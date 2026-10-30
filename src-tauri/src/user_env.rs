//! The person's own PATH, for the tools Juno starts on their behalf.
//!
//! A GUI-launched macOS app inherits launchd's minimal PATH
//! (`/usr/bin:/bin:/usr/sbin:/sbin`), so `npx`, `uvx`, `node` and friends
//! installed by Homebrew, nvm, volta, bun, asdf or mise are invisible to it.
//! This module asks the person's login shell for the PATH they actually have,
//! once, and caches it. If the shell cannot be asked (timeout, odd shell, no
//! `$SHELL`) it falls back to the launchd PATH plus well-known tool dirs that
//! exist.
//!
//! Nothing here mutates the process environment: callers set `PATH` on the
//! child they spawn.

use std::collections::HashMap;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::OnceLock;
use std::time::Duration;
use tracing::{debug, info, warn};
use wait_timeout::ChildExt;

const START_MARKER: &str = "__JUNO_PATH_START__";
const END_MARKER: &str = "__JUNO_PATH_END__";
const PROBE_TIMEOUT: Duration = Duration::from_secs(3);
const LAUNCHD_PATH: &str = "/usr/bin:/bin:/usr/sbin:/sbin";

/// Where the resolved PATH came from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PathSource {
    LoginShell,
    Fallback,
}

#[derive(Debug, Clone)]
pub struct UserPath {
    pub path: String,
    pub source: PathSource,
}

static USER_PATH: OnceLock<UserPath> = OnceLock::new();

/// Pull the PATH out of shell output. Rc files may print anything before or
/// after, so only the text between the last start marker and the next end
/// marker counts. Returns `None` if either marker is missing or the value is
/// empty.
pub fn parse_marked_path(output: &str) -> Option<String> {
    let start = output.rfind(START_MARKER)? + START_MARKER.len();
    let rest = &output[start..];
    let end = rest.find(END_MARKER)?;
    let value = rest[..end].trim();
    if value.is_empty() {
        None
    } else {
        Some(value.to_string())
    }
}

/// The launchd PATH plus well-known tool dirs that exist. Tool dirs come
/// first (they shadow stale system copies), launchd dirs last. `exists` is
/// injected so tests do not depend on the machine.
pub fn fallback_path(
    launchd_path: &str,
    home: Option<&Path>,
    exists: impl Fn(&Path) -> bool,
) -> String {
    let mut candidates: Vec<PathBuf> = vec![
        PathBuf::from("/opt/homebrew/bin"),
        PathBuf::from("/usr/local/bin"),
    ];
    if let Some(h) = home {
        for rel in [
            ".local/bin",
            ".bun/bin",
            ".volta/bin",
            ".cargo/bin",
            ".deno/bin",
            ".local/share/mise/shims",
            ".asdf/shims",
        ] {
            candidates.push(h.join(rel));
        }
    }
    let mut parts: Vec<String> = Vec::new();
    for c in candidates {
        if exists(&c) {
            push_unique(&mut parts, &c.to_string_lossy());
        }
    }
    for p in launchd_path.split(':').filter(|p| !p.is_empty()) {
        push_unique(&mut parts, p);
    }
    parts.join(":")
}

fn push_unique(parts: &mut Vec<String>, p: &str) {
    if !parts.iter().any(|x| x == p) {
        parts.push(p.to_string());
    }
}

fn build_fallback() -> String {
    let launchd = std::env::var("PATH")
        .ok()
        .filter(|p| !p.is_empty())
        .unwrap_or_else(|| LAUNCHD_PATH.to_string());
    let home = dirs::home_dir();
    fallback_path(&launchd, home.as_deref(), |p| p.is_dir())
}

/// Run the login shell and read PATH. Blocking, bounded by `PROBE_TIMEOUT`.
fn probe_login_shell() -> Option<String> {
    let shell = std::env::var("SHELL")
        .ok()
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| "/bin/zsh".to_string());
    let script = format!("printf '%s' \"{START_MARKER}${{PATH}}{END_MARKER}\"");
    let mut child = Command::new(&shell)
        .args(["-ilc", &script])
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|e| debug!("login shell probe: cannot spawn {}: {}", shell, e))
        .ok()?;

    // Drain stdout on its own thread so a chatty rc file cannot fill the pipe
    // and stall the shell before the timeout fires.
    let mut stdout = child.stdout.take()?;
    let (tx, rx) = std::sync::mpsc::channel::<String>();
    std::thread::spawn(move || {
        let mut buf = Vec::new();
        let _ = stdout.read_to_end(&mut buf);
        let _ = tx.send(String::from_utf8_lossy(&buf).into_owned());
    });

    match child.wait_timeout(PROBE_TIMEOUT) {
        Ok(Some(_)) => {}
        Ok(None) => {
            warn!("login shell probe timed out after {:?}", PROBE_TIMEOUT);
            let _ = child.kill();
            let _ = child.wait();
            return None;
        }
        Err(e) => {
            debug!("login shell probe wait failed: {}", e);
            let _ = child.kill();
            let _ = child.wait();
            return None;
        }
    }
    let out = rx.recv_timeout(Duration::from_millis(500)).ok()?;
    parse_marked_path(&out)
}

fn resolve_blocking() -> UserPath {
    let resolved = match probe_login_shell() {
        Some(path) => UserPath {
            path,
            source: PathSource::LoginShell,
        },
        None => UserPath {
            path: build_fallback(),
            source: PathSource::Fallback,
        },
    };
    info!("User PATH resolved from {:?}", resolved.source);
    debug!("User PATH: {}", resolved.path);
    resolved
}

/// The resolved PATH if the probe already finished, else a cheap fallback that
/// is not cached. Never blocks, so it is safe on any thread.
pub fn user_path_now() -> String {
    match USER_PATH.get() {
        Some(u) => u.path.clone(),
        None => build_fallback(),
    }
}

/// The resolved PATH, running the login shell probe on a blocking thread the
/// first time.
pub async fn user_path() -> String {
    if let Some(u) = USER_PATH.get() {
        return u.path.clone();
    }
    match tauri::async_runtime::spawn_blocking(|| {
        USER_PATH.get_or_init(resolve_blocking).path.clone()
    })
    .await
    {
        Ok(p) => p,
        Err(e) => {
            warn!("user PATH probe task failed: {}", e);
            build_fallback()
        }
    }
}

/// Start the probe in the background so the first spawn does not pay for it.
pub fn prewarm() {
    tauri::async_runtime::spawn_blocking(|| {
        USER_PATH.get_or_init(resolve_blocking);
    });
}

/// The PATH a child should get: a server's own `PATH` entry wins over the
/// person's login-shell PATH.
pub fn effective_path(user_path: &str, overrides: &HashMap<String, String>) -> String {
    match overrides.get("PATH") {
        Some(p) if !p.is_empty() => p.clone(),
        _ => user_path.to_string(),
    }
}

/// Resolve a bare command name (no `/`) against `path`. A command containing a
/// path separator is returned unchanged. `None` if a bare name is not found.
pub fn resolve_command(command: &str, path: &str) -> Option<PathBuf> {
    if command.contains('/') {
        return Some(PathBuf::from(command));
    }
    std::env::split_paths(path)
        .map(|d| d.join(command))
        .find(|c| is_executable_file(c))
}

#[cfg(unix)]
fn is_executable_file(p: &Path) -> bool {
    use std::os::unix::fs::PermissionsExt;
    std::fs::metadata(p)
        .map(|m| m.is_file() && m.permissions().mode() & 0o111 != 0)
        .unwrap_or(false)
}

#[cfg(not(unix))]
fn is_executable_file(p: &Path) -> bool {
    p.is_file()
}

/// Find a user-installed CLI by name on the person's PATH (non-blocking).
pub fn find_on_user_path(name: &str) -> Option<PathBuf> {
    resolve_command(name, &user_path_now())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_with_noise_before_and_after() {
        let out = format!("welcome!\nmotd\n{START_MARKER}/a:/b{END_MARKER}\nbye\n");
        assert_eq!(parse_marked_path(&out), Some("/a:/b".to_string()));
    }

    #[test]
    fn parser_uses_last_start_marker() {
        let out = format!("{START_MARKER}/old{START_MARKER}/new{END_MARKER}");
        assert_eq!(parse_marked_path(&out), Some("/new".to_string()));
    }

    #[test]
    fn parser_rejects_missing_or_empty() {
        assert_eq!(parse_marked_path("just noise"), None);
        assert_eq!(parse_marked_path(&format!("{START_MARKER}/a")), None);
        assert_eq!(parse_marked_path(&format!("{END_MARKER}/a")), None);
        assert_eq!(
            parse_marked_path(&format!("{START_MARKER}{END_MARKER}")),
            None
        );
    }

    #[test]
    fn fallback_includes_existing_dirs_then_launchd() {
        let home = PathBuf::from("/Users/x");
        let existing = ["/opt/homebrew/bin", "/Users/x/.bun/bin"];
        let p = fallback_path(LAUNCHD_PATH, Some(&home), |d| {
            existing.iter().any(|e| Path::new(e) == d)
        });
        assert_eq!(
            p,
            "/opt/homebrew/bin:/Users/x/.bun/bin:/usr/bin:/bin:/usr/sbin:/sbin"
        );
    }

    #[test]
    fn fallback_dedupes_and_survives_no_home() {
        let p = fallback_path("/usr/local/bin:/usr/bin", None, |_| true);
        assert_eq!(p, "/opt/homebrew/bin:/usr/local/bin:/usr/bin");
    }

    #[test]
    fn server_path_override_wins() {
        let mut env = HashMap::new();
        assert_eq!(effective_path("/user", &env), "/user");
        env.insert("PATH".to_string(), "/mine".to_string());
        assert_eq!(effective_path("/user", &env), "/mine");
        env.insert("PATH".to_string(), String::new());
        assert_eq!(effective_path("/user", &env), "/user");
    }

    #[cfg(unix)]
    #[test]
    fn resolves_bare_command_in_temp_path() {
        use std::os::unix::fs::PermissionsExt;
        let dir = tempfile::tempdir().expect("tempdir");
        let bin = dir.path().join("mytool");
        std::fs::write(&bin, "#!/bin/sh\n").expect("write");
        std::fs::set_permissions(&bin, std::fs::Permissions::from_mode(0o755)).expect("chmod");
        let plain = dir.path().join("notexec");
        std::fs::write(&plain, "x").expect("write");

        let path = format!("/nonexistent:{}", dir.path().display());
        assert_eq!(resolve_command("mytool", &path), Some(bin));
        assert_eq!(resolve_command("notexec", &path), None);
        assert_eq!(resolve_command("missing", &path), None);
        assert_eq!(
            resolve_command("/abs/tool", &path),
            Some(PathBuf::from("/abs/tool"))
        );
    }
}

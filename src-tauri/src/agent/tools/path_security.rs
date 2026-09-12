//! # Path Security Helpers
//!
//! Shared canonicalization and workspace-boundary enforcement for agent file
//! operations. This is the single implementation of the "resolve symlinks and
//! `..` segments, then require the result to live inside an allowed root"
//! logic that previously existed only in `basic_tools` (security audit
//! 2026-02-08, items #13/#14/#18).
//!
//! ## Used by:
//! - `basic_tools::validate_file_path` (read path canonicalization)
//! - `anthropic_computer_use::validate_file_path` (str_replace editor tool)
//! - `enhanced_coding_tools::create_file_with_content` (smart_create_file)

use std::fs::File;
use std::io::Read as _;
use std::path::{Component, Path, PathBuf};

/// Canonicalize a path, tolerating files that do not exist yet.
///
/// For an existing path this is `std::fs::canonicalize`. For a path whose
/// final component does not exist yet (e.g. a file about to be created), the
/// parent directory is canonicalized and the file name re-appended, so `..`
/// segments and symlinks in the directory portion are still resolved.
pub fn canonicalize_lenient(full_path: &Path) -> PathBuf {
    match full_path.canonicalize() {
        Ok(p) => p,
        Err(_) => {
            // If the file doesn't exist yet, canonicalize the parent directory
            if let Some(parent) = full_path.parent() {
                match parent.canonicalize() {
                    Ok(canonical_parent) => {
                        if let Some(file_name) = full_path.file_name() {
                            canonical_parent.join(file_name)
                        } else {
                            full_path.to_path_buf()
                        }
                    }
                    Err(_) => full_path.to_path_buf(),
                }
            } else {
                full_path.to_path_buf()
            }
        }
    }
}

/// Sensitive credential/key material the agent must never read or write,
/// regardless of workspace boundaries (security audit 2026-02-08, item #32).
///
/// Matching is case-insensitive and precise (exact file names, exact
/// extensions, exact directory components) so ordinary source files like
/// `keystore.rs` or `environment.md` are unaffected. Returns a short reason
/// string when the path is blocked.
pub fn sensitive_path_reason(path: &Path) -> Option<String> {
    // Directory components that are credential stores in their entirety
    for component in path.components() {
        if let Component::Normal(part) = component {
            let part = part.to_string_lossy().to_lowercase();
            if part == ".ssh" || part == ".gnupg" {
                return Some(format!("path is inside the {} credential directory", part));
            }
        }
    }

    let file_name = path.file_name()?.to_string_lossy().to_lowercase();

    // SSH private keys (id_rsa, id_ed25519, id_ecdsa, id_dsa, and their
    // derivatives like id_rsa.old); already covered when under ~/.ssh, but
    // copies elsewhere are just as sensitive
    for key_name in ["id_rsa", "id_ed25519", "id_ecdsa", "id_dsa"] {
        if file_name == key_name || file_name.starts_with(&format!("{}.", key_name)) {
            return Some(format!("'{}' is an SSH key file", file_name));
        }
    }

    // Credential-bearing dotfiles and wallet files, by exact name
    if matches!(
        file_name.as_str(),
        ".netrc" | "_netrc" | ".npmrc" | ".pgpass" | ".boto" | "wallet.dat" | ".htpasswd"
    ) {
        return Some(format!("'{}' is a credential file", file_name));
    }

    // AWS credentials: ~/.aws/credentials (or any credentials file under an
    // .aws directory)
    if file_name == "credentials" {
        let under_aws = path.components().any(|c| {
            matches!(c, Component::Normal(part) if part.to_string_lossy().to_lowercase() == ".aws")
        });
        if under_aws {
            return Some("AWS credentials file".to_string());
        }
    }

    // Dotenv family (.env, .env.local, ...)
    if file_name == ".env" || file_name.starts_with(".env.") {
        return Some(format!("'{}' is an environment secret file", file_name));
    }

    // Key-material extensions: certificates/keys, PKCS bundles, Java/mac
    // keystores and keychains
    if let Some(extension) = path.extension() {
        let ext = extension.to_string_lossy().to_lowercase();
        if matches!(
            ext.as_str(),
            "pem" | "key" | "p12" | "pfx" | "jks" | "keystore" | "keychain" | "keychain-db"
        ) {
            return Some(format!("'.{}' files hold key material", ext));
        }
    }

    None
}

/// Resolve `path_str` to an absolute, canonical path and require it to live
/// inside one of the allowed `roots`.
///
/// Relative paths are resolved against the current working directory before
/// canonicalization, matching the behavior of `basic_tools`. An empty `roots`
/// slice fails closed: no boundary can be established, so access is denied.
/// Sensitive credential/key files are denied even inside the boundary
/// (security audit 2026-02-08, item #32).
pub fn resolve_within_roots(path_str: &str, roots: &[PathBuf]) -> Result<PathBuf, String> {
    if path_str.is_empty() {
        return Err("Empty path not allowed".to_string());
    }

    let path = PathBuf::from(path_str);
    let full_path = if path.is_absolute() {
        path
    } else {
        let current_dir = std::env::current_dir()
            .map_err(|e| format!("Failed to get current directory: {}", e))?;
        current_dir.join(&path)
    };

    let canonical_path = canonicalize_lenient(&full_path);

    if let Some(reason) = sensitive_path_reason(&canonical_path) {
        return Err(format!(
            "Access denied: sensitive file is blocked ({})",
            reason
        ));
    }

    if roots.is_empty() {
        return Err(
            "Access denied: No workspace boundary is available for file access".to_string(),
        );
    }

    let allowed = roots.iter().any(|root| {
        let canonical_root = canonicalize_lenient(root);
        canonical_path.starts_with(&canonical_root)
    });

    if allowed {
        Ok(canonical_path)
    } else {
        Err(format!(
            "Access denied: Path is outside the workspace boundary. Workspace: {}",
            roots
                .iter()
                .map(|r| r.display().to_string())
                .collect::<Vec<_>>()
                .join(", ")
        ))
    }
}

/// Whether a process working directory is usable as a workspace boundary
/// root (security audit 2026-02-08, items #12/#27).
///
/// Packaged apps launched from Finder start with cwd `/`; a boundary rooted
/// there is no boundary at all (`starts_with("/")` matches everything). A
/// cwd the process cannot write to is equally suspect: it is some system
/// location, not a workspace the user pointed the app at.
pub fn is_usable_workspace_cwd(cwd: &Path) -> bool {
    // The filesystem root makes the boundary check unbounded
    if cwd.parent().is_none() {
        return false;
    }

    // A workspace root must be writable by this process
    is_writable_dir(cwd)
}

/// True when the process can write to `path` (POSIX `access(2)` with W_OK,
/// which honors the effective uid/gid, unlike permission-bit inspection).
fn is_writable_dir(path: &Path) -> bool {
    use std::ffi::CString;
    use std::os::unix::ffi::OsStrExt;

    let Ok(c_path) = CString::new(path.as_os_str().as_bytes()) else {
        return false;
    };

    // SAFETY: c_path is a valid NUL-terminated C string that outlives the
    // call; access() only reads the pointer and touches no other memory.
    unsafe { libc::access(c_path.as_ptr(), libc::W_OK) == 0 }
}

/// The process working directory, if it is usable as a workspace root.
/// Logs why the cwd was excluded so a Finder-launched app (`cwd == "/"`)
/// is diagnosable (security audit 2026-02-08, items #12/#27).
pub fn usable_cwd_workspace_root() -> Option<PathBuf> {
    match std::env::current_dir() {
        Ok(cwd) if is_usable_workspace_cwd(&cwd) => Some(cwd),
        Ok(cwd) => {
            log::warn!(
                "Workspace root resolution: cwd '{}' is the filesystem root or not writable \
                 (typical for a packaged app launched from Finder); excluding it from the \
                 workspace boundary and falling back to ~/Juno",
                cwd.display()
            );
            None
        }
        Err(e) => {
            log::warn!(
                "Workspace root resolution: could not determine cwd ({}); falling back to ~/Juno",
                e
            );
            None
        }
    }
}

/// Default allowed roots for agent file operations:
/// - the process working directory, only when it is a usable boundary
///   (not `/`, writable — see [`usable_cwd_workspace_root`]), and
/// - the agent's preferred output directory (`~/Juno`), where
///   `smart_create_file` places files given a bare filename.
///
/// When the cwd is unusable (packaged app launched from Finder), `~/Juno`
/// alone bounds the agent. If neither resolves, the list is empty and
/// [`resolve_within_roots`] fails closed.
pub fn default_workspace_roots() -> Vec<PathBuf> {
    let mut roots = Vec::new();
    if let Some(cwd) = usable_cwd_workspace_root() {
        roots.push(cwd);
    }
    if let Ok(juno_dir) = crate::utils::get_agent_preferred_directory() {
        roots.push(juno_dir);
    }
    log::debug!(
        "Workspace roots resolved: [{}]",
        roots
            .iter()
            .map(|r| r.display().to_string())
            .collect::<Vec<_>>()
            .join(", ")
    );
    roots
}

/// Resolve `path_str` against [`default_workspace_roots`]. Fails closed when
/// no workspace root can be determined.
pub fn resolve_within_default_roots(path_str: &str) -> Result<PathBuf, String> {
    resolve_within_roots(path_str, &default_workspace_roots())
}

// ---------------------------------------------------------------------------
// TOCTOU-hardened file I/O (security audit 2026-02-08, item #28)
//
// Canonicalize-then-open leaves a race: a symlink swapped in between the
// validation and the open redirects the operation outside the boundary. The
// helpers below close it by (a) opening the already-canonical path with
// O_NOFOLLOW, so a symlink swapped into the final component fails the open,
// and (b) asking the kernel for the opened handle's real path (fcntl
// F_GETPATH on macOS) and re-checking the boundary on THAT path, which also
// catches swapped intermediate directories. All I/O then goes through the
// verified handle.
// ---------------------------------------------------------------------------

/// The real filesystem path of an open file handle, from the kernel.
#[cfg(target_os = "macos")]
fn handle_real_path(file: &File) -> Result<PathBuf, String> {
    use std::os::unix::ffi::OsStrExt;
    use std::os::unix::io::AsRawFd;

    let mut buf = [0u8; libc::PATH_MAX as usize];
    // SAFETY: buf is PATH_MAX bytes as F_GETPATH requires, and the fd is a
    // valid open descriptor for the lifetime of the call.
    let rc = unsafe { libc::fcntl(file.as_raw_fd(), libc::F_GETPATH, buf.as_mut_ptr()) };
    if rc == -1 {
        return Err("Failed to resolve the real path of the open file handle".to_string());
    }

    let len = buf.iter().position(|&b| b == 0).unwrap_or(buf.len());
    Ok(PathBuf::from(std::ffi::OsStr::from_bytes(&buf[..len])))
}

/// Verify that an already-open handle really lives inside one of the allowed
/// roots, using the kernel's view of the handle rather than the path string
/// that was validated earlier.
fn verify_handle_within_roots(file: &File, roots: &[PathBuf]) -> Result<(), String> {
    #[cfg(target_os = "macos")]
    {
        let real_path = handle_real_path(file)?;
        let allowed = roots.iter().any(|root| {
            let canonical_root = canonicalize_lenient(root);
            real_path.starts_with(&canonical_root)
        });
        if !allowed {
            return Err(format!(
                "Access denied: the opened file resolved outside the workspace boundary ({})",
                real_path.display()
            ));
        }
        if let Some(reason) = sensitive_path_reason(&real_path) {
            return Err(format!(
                "Access denied: sensitive file is blocked ({})",
                reason
            ));
        }
        Ok(())
    }

    #[cfg(not(target_os = "macos"))]
    {
        // No cheap handle-path API off macOS; O_NOFOLLOW on the canonical
        // path still protects the final component. Documented residual.
        let _ = (file, roots);
        Ok(())
    }
}

/// Open an already-validated canonical path for reading without following a
/// symlink swapped into the final component.
fn open_no_follow_read(canonical: &Path) -> std::io::Result<File> {
    use std::os::unix::fs::OpenOptionsExt;
    std::fs::OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_NOFOLLOW)
        .open(canonical)
}

/// Read a validated file to a string through a boundary-verified handle.
/// `canonical` must come from [`resolve_within_roots`] with the same `roots`.
pub fn read_to_string_checked(canonical: &Path, roots: &[PathBuf]) -> Result<String, String> {
    let mut file = open_no_follow_read(canonical)
        .map_err(|e| format!("Failed to open file '{}': {}", canonical.display(), e))?;
    verify_handle_within_roots(&file, roots)?;

    let mut content = String::new();
    file.read_to_string(&mut content)
        .map_err(|e| format!("Failed to read file '{}': {}", canonical.display(), e))?;
    Ok(content)
}

/// Write (create or truncate) a validated file through a boundary-verified
/// handle. The file is only truncated after the handle passes verification.
/// `canonical` must come from [`resolve_within_roots`] with the same `roots`.
pub fn write_checked(canonical: &Path, contents: &[u8], roots: &[PathBuf]) -> Result<(), String> {
    use std::io::Write as _;
    use std::os::unix::fs::OpenOptionsExt;

    let mut file = std::fs::OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(false) // truncate only after the handle is verified
        .custom_flags(libc::O_NOFOLLOW)
        .open(canonical)
        .map_err(|e| format!("Failed to open file '{}': {}", canonical.display(), e))?;
    verify_handle_within_roots(&file, roots)?;

    file.set_len(0)
        .map_err(|e| format!("Failed to truncate file '{}': {}", canonical.display(), e))?;
    file.write_all(contents)
        .map_err(|e| format!("Failed to write file '{}': {}", canonical.display(), e))?;
    Ok(())
}

/// Create a brand-new validated file (fails if it already exists) through a
/// boundary-verified handle.
/// `canonical` must come from [`resolve_within_roots`] with the same `roots`.
pub fn create_new_checked(
    canonical: &Path,
    contents: &[u8],
    roots: &[PathBuf],
) -> Result<(), String> {
    use std::io::Write as _;

    let mut file = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true) // O_EXCL: never follows a symlink, fails if present
        .open(canonical)
        .map_err(|e| format!("Failed to create file '{}': {}", canonical.display(), e))?;

    if let Err(e) = verify_handle_within_roots(&file, roots) {
        // Best-effort cleanup of the just-created empty file
        let _ = std::fs::remove_file(canonical);
        return Err(e);
    }

    file.write_all(contents)
        .map_err(|e| format!("Failed to write file '{}': {}", canonical.display(), e))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_root() -> tempfile::TempDir {
        tempfile::tempdir().unwrap_or_else(|e| panic!("failed to create temp dir: {}", e))
    }

    #[test]
    fn allows_file_inside_root() {
        let dir = temp_root();
        let file = dir.path().join("notes.txt");
        std::fs::write(&file, "hello").unwrap_or_else(|e| panic!("write failed: {}", e));

        let result = resolve_within_roots(&file.to_string_lossy(), &[dir.path().to_path_buf()]);
        assert!(result.is_ok(), "expected Ok, got {:?}", result);
    }

    #[test]
    fn allows_nonexistent_file_in_existing_dir_inside_root() {
        let dir = temp_root();
        let file = dir.path().join("new_file.txt");

        let result = resolve_within_roots(&file.to_string_lossy(), &[dir.path().to_path_buf()]);
        assert!(result.is_ok(), "expected Ok, got {:?}", result);
    }

    #[test]
    fn blocks_absolute_path_outside_root() {
        let dir = temp_root();
        let result = resolve_within_roots("/etc/hosts", &[dir.path().to_path_buf()]);
        assert!(result.is_err(), "absolute path outside root must be denied");
    }

    #[test]
    fn blocks_traversal_escaping_root() {
        let dir = temp_root();
        let sneaky = dir
            .path()
            .join("sub")
            .join("..")
            .join("..")
            .join("etc")
            .join("passwd");

        let result = resolve_within_roots(&sneaky.to_string_lossy(), &[dir.path().to_path_buf()]);
        assert!(
            result.is_err(),
            "traversal escaping the root must be denied"
        );
    }

    #[test]
    fn empty_roots_fail_closed() {
        let result = resolve_within_roots("/tmp/anything.txt", &[]);
        assert!(result.is_err(), "empty roots must deny access");
    }

    #[test]
    fn empty_path_rejected() {
        let dir = temp_root();
        let result = resolve_within_roots("", &[dir.path().to_path_buf()]);
        assert!(result.is_err());
    }

    // --- Workspace root resolution (#12/#27) ---

    #[test]
    fn filesystem_root_is_not_a_usable_workspace_cwd() {
        // Packaged apps launched from Finder start with cwd `/`
        assert!(!is_usable_workspace_cwd(Path::new("/")));
    }

    #[test]
    fn writable_directory_is_a_usable_workspace_cwd() {
        let dir = temp_root();
        assert!(is_usable_workspace_cwd(dir.path()));
    }

    #[test]
    fn non_writable_directory_is_not_a_usable_workspace_cwd() {
        use std::os::unix::fs::PermissionsExt;
        let dir = temp_root();
        let locked = dir.path().join("locked");
        std::fs::create_dir(&locked).unwrap_or_else(|e| panic!("mkdir failed: {}", e));
        std::fs::set_permissions(&locked, std::fs::Permissions::from_mode(0o555))
            .unwrap_or_else(|e| panic!("chmod failed: {}", e));

        // Root bypasses permission checks; only assert for regular users
        // SAFETY: geteuid takes no arguments and only reads process state.
        if unsafe { libc::geteuid() } != 0 {
            assert!(!is_usable_workspace_cwd(&locked));
        }

        // Restore so TempDir cleanup can remove it
        let _ = std::fs::set_permissions(&locked, std::fs::Permissions::from_mode(0o755));
    }

    #[test]
    fn boundary_check_with_only_juno_root_denies_system_paths() {
        // Simulates a Finder launch (cwd `/` excluded): with only a ~/Juno
        // style root, absolute system paths must be denied instead of the
        // boundary being unbounded.
        let juno_like = temp_root();
        let result = resolve_within_roots("/etc/hosts", &[juno_like.path().to_path_buf()]);
        assert!(result.is_err(), "system path must be outside the boundary");
    }

    // --- Sensitive file blocklist (#32) ---

    #[test]
    fn ssh_keys_and_ssh_directory_blocked_case_insensitively() {
        assert!(sensitive_path_reason(Path::new("/home/user/id_rsa")).is_some());
        assert!(sensitive_path_reason(Path::new("/home/user/ID_RSA")).is_some());
        assert!(sensitive_path_reason(Path::new("/home/user/id_ed25519")).is_some());
        assert!(sensitive_path_reason(Path::new("/home/user/id_rsa.old")).is_some());
        assert!(sensitive_path_reason(Path::new("/home/user/.ssh/config")).is_some());
        assert!(sensitive_path_reason(Path::new("/home/user/.SSH/known_hosts")).is_some());
    }

    #[test]
    fn credential_dotfiles_blocked() {
        assert!(sensitive_path_reason(Path::new("/home/user/.aws/credentials")).is_some());
        assert!(sensitive_path_reason(Path::new("/home/user/.netrc")).is_some());
        assert!(sensitive_path_reason(Path::new("/home/user/_netrc")).is_some());
        assert!(sensitive_path_reason(Path::new("/home/user/.npmrc")).is_some());
        assert!(sensitive_path_reason(Path::new("/home/user/.pgpass")).is_some());
        assert!(sensitive_path_reason(Path::new("/project/.env")).is_some());
        assert!(sensitive_path_reason(Path::new("/project/.env.local")).is_some());
    }

    #[test]
    fn key_material_extensions_blocked() {
        assert!(sensitive_path_reason(Path::new("/tmp/cert.p12")).is_some());
        assert!(sensitive_path_reason(Path::new("/tmp/CERT.PFX")).is_some());
        assert!(sensitive_path_reason(Path::new("/tmp/server.pem")).is_some());
        assert!(sensitive_path_reason(Path::new("/tmp/private.key")).is_some());
        assert!(sensitive_path_reason(Path::new("/tmp/app.jks")).is_some());
        assert!(sensitive_path_reason(Path::new("/tmp/release.keystore")).is_some());
        assert!(sensitive_path_reason(Path::new("/Library/login.keychain-db")).is_some());
        assert!(sensitive_path_reason(Path::new("/home/user/wallet.dat")).is_some());
    }

    #[test]
    fn ordinary_files_not_flagged_as_sensitive() {
        assert!(sensitive_path_reason(Path::new("/project/src/keystore.rs")).is_none());
        assert!(sensitive_path_reason(Path::new("/project/environment.md")).is_none());
        assert!(sensitive_path_reason(Path::new("/project/envelope.txt")).is_none());
        assert!(sensitive_path_reason(Path::new("/project/monkey.png")).is_none());
        assert!(sensitive_path_reason(Path::new("/project/README.md")).is_none());
    }

    #[test]
    fn resolve_within_roots_blocks_sensitive_files_inside_boundary() {
        let dir = temp_root();
        let key = dir.path().join("id_rsa");
        std::fs::write(&key, "PRIVATE").unwrap_or_else(|e| panic!("write failed: {}", e));

        let result = resolve_within_roots(&key.to_string_lossy(), &[dir.path().to_path_buf()]);
        assert!(
            result.is_err(),
            "sensitive file inside the boundary must still be denied"
        );
    }

    // --- TOCTOU-hardened I/O (#28) ---

    #[test]
    fn read_to_string_checked_reads_file_inside_root() {
        let dir = temp_root();
        let file = dir.path().join("notes.txt");
        std::fs::write(&file, "hello world").unwrap_or_else(|e| panic!("write failed: {}", e));

        let roots = [dir.path().to_path_buf()];
        let canonical = resolve_within_roots(&file.to_string_lossy(), &roots)
            .unwrap_or_else(|e| panic!("resolve failed: {}", e));
        let content = read_to_string_checked(&canonical, &roots)
            .unwrap_or_else(|e| panic!("read failed: {}", e));
        assert_eq!(content, "hello world");
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn read_to_string_checked_rejects_symlink_swapped_after_validation() {
        let dir = temp_root();
        let outside = temp_root();
        let secret = outside.path().join("secret.txt");
        std::fs::write(&secret, "secret").unwrap_or_else(|e| panic!("write failed: {}", e));

        let roots = [dir.path().to_path_buf()];
        let target = dir.path().join("data.txt");
        std::fs::write(&target, "benign").unwrap_or_else(|e| panic!("write failed: {}", e));

        // Validate while the path is a benign regular file...
        let canonical = resolve_within_roots(&target.to_string_lossy(), &roots)
            .unwrap_or_else(|e| panic!("resolve failed: {}", e));

        // ...then swap in a symlink to a file outside the boundary (TOCTOU)
        std::fs::remove_file(&target).unwrap_or_else(|e| panic!("remove failed: {}", e));
        std::os::unix::fs::symlink(&secret, &target)
            .unwrap_or_else(|e| panic!("symlink failed: {}", e));

        let result = read_to_string_checked(&canonical, &roots);
        assert!(
            result.is_err(),
            "swapped symlink must not be followed, got {:?}",
            result
        );
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn write_checked_rejects_symlink_swapped_after_validation() {
        let dir = temp_root();
        let outside = temp_root();
        let victim = outside.path().join("victim.txt");
        std::fs::write(&victim, "do not clobber").unwrap_or_else(|e| panic!("write failed: {}", e));

        let roots = [dir.path().to_path_buf()];
        let target = dir.path().join("out.txt");
        std::fs::write(&target, "benign").unwrap_or_else(|e| panic!("write failed: {}", e));

        let canonical = resolve_within_roots(&target.to_string_lossy(), &roots)
            .unwrap_or_else(|e| panic!("resolve failed: {}", e));

        std::fs::remove_file(&target).unwrap_or_else(|e| panic!("remove failed: {}", e));
        std::os::unix::fs::symlink(&victim, &target)
            .unwrap_or_else(|e| panic!("symlink failed: {}", e));

        let result = write_checked(&canonical, b"pwned", &roots);
        assert!(
            result.is_err(),
            "swapped symlink must not be written through"
        );

        let victim_content = std::fs::read_to_string(&victim)
            .unwrap_or_else(|e| panic!("read victim failed: {}", e));
        assert_eq!(victim_content, "do not clobber");
    }

    #[test]
    fn write_checked_and_create_new_checked_work_inside_root() {
        let dir = temp_root();
        let roots = [dir.path().to_path_buf()];

        let fresh = dir.path().join("fresh.txt");
        let canonical_fresh = resolve_within_roots(&fresh.to_string_lossy(), &roots)
            .unwrap_or_else(|e| panic!("resolve failed: {}", e));
        create_new_checked(&canonical_fresh, b"created", &roots)
            .unwrap_or_else(|e| panic!("create failed: {}", e));
        assert!(
            create_new_checked(&canonical_fresh, b"again", &roots).is_err(),
            "create_new must fail on an existing file"
        );

        write_checked(&canonical_fresh, b"overwritten", &roots)
            .unwrap_or_else(|e| panic!("write failed: {}", e));
        let content = read_to_string_checked(&canonical_fresh, &roots)
            .unwrap_or_else(|e| panic!("read failed: {}", e));
        assert_eq!(content, "overwritten");
    }
}

//! Apps and websites: "open Safari", "quit Slack", "open github.com".
//!
//! The name the person said is never used as-is. It is matched against the
//! `.app` bundles actually installed on this Mac, and only an exact match
//! (after the same normalization on both sides, plus a few well-known
//! aliases) is acted on. "Open Safari and find flights" never reaches here
//! (the clause word refuses it), and "open the flight tracker" finds no app
//! called that, so both go to the agent.
//!
//! Opening runs `open -a <bundle path>` with the path as an argument, never
//! through a shell. Quitting passes the app name to AppleScript as `argv`, so
//! the script text itself is a constant.
//!
//! # The bundle match is the safeguard, not `argv`
//!
//! AppleScript resolves an application name when it compiles the script. A
//! name macOS cannot place shows the person a modal "Where is ...?" picker
//! listing every app on the Mac, and the script sits there blocked until
//! someone dismisses it. Passing the name through `argv` does not reliably
//! prevent that: `id of application (item 1 of argv)` was observed prompting
//! on 2026-10-01 with the name supplied as an argument.
//!
//! What keeps this module safe is the step before the script: a name is only
//! ever acted on after [`match_app`] matched it against an `.app` bundle
//! found on disk, so the app is known to exist and there is nothing for
//! macOS to ask about. `argv` keeps the person's words out of the script
//! text, which is a different guarantee (injection, not the picker), and both
//! are worth having.
//!
//! Anything new that addresses an app by name belongs behind the same match.
//! [`quit_installed`] takes an [`InstalledApp`] rather than a string for that
//! reason, and a test pins it.

use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::{Duration, Instant};

use once_cell::sync::Lazy;
use regex::Regex;
use tauri::AppHandle;

use super::{compile, osascript, run, Reply};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AppIntent {
    /// Open or bring forward the app whose normalized name is `query`.
    Open { query: String },
    /// Quit the app whose normalized name is `query`.
    Quit { query: String },
    /// Open `https://<domain>` in the default browser.
    Website { domain: String },
}

struct Patterns {
    website: Regex,
    domain: Regex,
    open: Regex,
    quit: Regex,
}

impl Patterns {
    fn compile() -> Result<Self, regex::Error> {
        Ok(Self {
            website: Regex::new(
                r"^(?:open|launch|visit|go to|pull up|bring up)(?: up)?(?: website)? (\S+(?: dot \S+)+|\S+\.\S+)$",
            )?,
            // Hostnames on a short list of common TLDs. The list is what keeps
            // "open main.rs" or "open notes.md" away from the browser.
            domain: Regex::new(
                r"^(?:[a-z0-9](?:[a-z0-9-]*[a-z0-9])?\.)+(?:com|org|net|io|dev|app|ai|co|edu|gov|me|tv|fm|gg|sh|xyz|info|news|so|us|uk|ca|de|fr)$",
            )?,
            open: Regex::new(
                r"^(?:open|launch|start|switch to|bring up|pull up)(?: up)? (.+?)(?: app| application)?$",
            )?,
            quit: Regex::new(r"^(?:quit|exit)(?: out of| from)? (.+?)(?: app| application)?$")?,
        })
    }
}

static PATTERNS: Lazy<Option<Patterns>> = Lazy::new(|| compile("apps", Patterns::compile));

pub fn parse(utterance: &str) -> Option<AppIntent> {
    let p = PATTERNS.as_ref()?;
    if let Some(caps) = p.website.captures(utterance) {
        let raw = caps.get(1)?.as_str().replace(" dot ", ".");
        let domain = raw.strip_prefix("www.").unwrap_or(&raw).to_string();
        return p
            .domain
            .is_match(&domain)
            .then_some(AppIntent::Website { domain });
    }
    if let Some(caps) = p.quit.captures(utterance) {
        let query = app_key(caps.get(1)?.as_str());
        return (!query.is_empty()).then_some(AppIntent::Quit { query });
    }
    if let Some(caps) = p.open.captures(utterance) {
        let query = app_key(caps.get(1)?.as_str());
        return (!query.is_empty()).then_some(AppIntent::Open { query });
    }
    None
}

/// Canonical form of an app name for matching: lower case, letters and
/// digits only, articles dropped. Applied identically to what was said and
/// to the bundle names on disk.
pub fn app_key(name: &str) -> String {
    name.to_lowercase()
        .chars()
        .map(|c| if c.is_alphanumeric() { c } else { ' ' })
        .collect::<String>()
        .split_whitespace()
        .filter(|w| !matches!(*w, "the" | "my" | "a" | "an"))
        .collect::<Vec<_>>()
        .join(" ")
}

/// Spoken names that differ from the bundle name.
const ALIASES: &[(&str, &[&str])] = &[
    ("settings", &["System Settings", "System Preferences"]),
    (
        "system preferences",
        &["System Settings", "System Preferences"],
    ),
    ("preferences", &["System Settings", "System Preferences"]),
    ("vs code", &["Visual Studio Code"]),
    ("vscode", &["Visual Studio Code"]),
    ("itunes", &["Music"]),
    ("apple music", &["Music"]),
    ("activity", &["Activity Monitor"]),
];

/// Vendor prefixes people leave off: "Chrome", "Word", "Photoshop".
const VENDOR_PREFIXES: &[&str] = &["google ", "microsoft ", "adobe "];

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InstalledApp {
    pub name: String,
    pub path: PathBuf,
}

/// Find the one installed app `query` names. Ambiguous or unknown is `None`.
pub fn match_app<'a>(query: &str, apps: &'a [InstalledApp]) -> Option<&'a InstalledApp> {
    if let Some((_, names)) = ALIASES.iter().find(|(alias, _)| *alias == query) {
        return names
            .iter()
            .find_map(|n| apps.iter().find(|a| a.name.eq_ignore_ascii_case(n)));
    }
    let mut hits = apps.iter().filter(|a| {
        let key = app_key(&a.name);
        key == query
            || VENDOR_PREFIXES
                .iter()
                .any(|v| key.strip_prefix(v) == Some(query))
    });
    let first = hits.next()?;
    // Two different apps answer to the same name: let the agent ask.
    if hits.any(|other| other.name != first.name) {
        return None;
    }
    // Never open or quit Juno itself from a voice command.
    if app_key(&first.name).starts_with("juno") {
        return None;
    }
    Some(first)
}

/// Folders scanned for `.app` bundles, in priority order.
fn app_dirs() -> Vec<PathBuf> {
    let mut folders = vec![
        PathBuf::from("/Applications"),
        PathBuf::from("/Applications/Utilities"),
        PathBuf::from("/System/Applications"),
        PathBuf::from("/System/Applications/Utilities"),
        PathBuf::from("/System/Library/CoreServices/Applications"),
    ];
    if let Some(home) = dirs::home_dir() {
        folders.push(home.join("Applications"));
    }
    folders.push(PathBuf::from("/System/Library/CoreServices/Finder.app"));
    folders
}

fn scan_installed_apps() -> Vec<InstalledApp> {
    let mut apps = Vec::new();
    for dir in app_dirs() {
        if dir.extension().is_some_and(|e| e == "app") {
            push_bundle(&mut apps, &dir);
            continue;
        }
        let Ok(entries) = std::fs::read_dir(&dir) else {
            continue;
        };
        for entry in entries.flatten() {
            push_bundle(&mut apps, &entry.path());
        }
    }
    apps
}

fn push_bundle(apps: &mut Vec<InstalledApp>, path: &Path) {
    if path.extension().is_none_or(|e| e != "app") {
        return;
    }
    if let Some(name) = path.file_stem().and_then(|s| s.to_str()) {
        apps.push(InstalledApp {
            name: name.to_string(),
            path: path.to_path_buf(),
        });
    }
}

/// How long a scan of the app folders is reused.
const APP_CACHE_TTL: Duration = Duration::from_secs(60);

type AppCache = Option<(Instant, Vec<InstalledApp>)>;
static APP_CACHE: Lazy<Mutex<AppCache>> = Lazy::new(|| Mutex::new(None));

async fn installed_apps() -> Vec<InstalledApp> {
    if let Ok(guard) = APP_CACHE.lock() {
        if let Some((at, apps)) = guard.as_ref() {
            if at.elapsed() < APP_CACHE_TTL {
                return apps.clone();
            }
        }
    }
    let apps = match tokio::task::spawn_blocking(scan_installed_apps).await {
        Ok(apps) => apps,
        Err(e) => {
            log::warn!("local_intents: app scan failed: {}", e);
            return Vec::new();
        }
    };
    if let Ok(mut guard) = APP_CACHE.lock() {
        *guard = Some((Instant::now(), apps.clone()));
    }
    apps
}

/// Quit `item 1 of argv` if it is running. Responses are ignored so an app
/// showing a "save changes?" sheet cannot stall the reply.
///
/// Reached only through [`quit_installed`]: this script addresses an app by
/// name, which is safe only for a name already matched against an installed
/// bundle. See the module docs.
const QUIT_SCRIPT: &str = "on run argv\n\
     set appName to item 1 of argv\n\
     if application appName is running then\n\
     ignoring application responses\n\
     tell application appName to quit\n\
     end ignoring\n\
     return \"quit\"\n\
     end if\n\
     return \"not_running\"\n\
     end run";

/// Quit an app that has already been matched against a bundle on disk.
///
/// Takes [`InstalledApp`] rather than a name on purpose. An `InstalledApp` is
/// built from an `.app` bundle the folder scan found, so holding one is proof
/// the app exists, and the modal app picker has nothing to ask about. A `&str`
/// here would let a future caller hand this script a guessed name.
async fn quit_installed(app: &InstalledApp) -> Result<String, String> {
    osascript(QUIT_SCRIPT, vec![app.name.clone()]).await
}

/// The installed app a name resolves to, or `None` when nothing on disk
/// answers to it.
///
/// Reuses the cached folder scan, so this costs nothing most of the time and
/// never asks macOS to place a name. Used by
/// [`crate::agent::app_observation`] to tell "installed but not running" from
/// "no such app", and to decide whether a command that names an app literally
/// is safe to run at all.
pub(crate) async fn resolve_installed(name: &str) -> Option<InstalledApp> {
    let apps = installed_apps().await;
    match_app(&app_key(name), &apps).cloned()
}

pub(super) async fn handle(_app_handle: &AppHandle, intent: AppIntent) -> Option<Reply> {
    match intent {
        AppIntent::Website { domain } => {
            let url = format!("https://{}", domain);
            Some(match run("open", vec![url.clone()]).await {
                Ok(_) => Reply::card(
                    format!("<LinkCard url=\"{}\" />", url),
                    format!("Opening {}.", domain),
                ),
                Err(e) => {
                    log::warn!("local_intents: open {} failed: {}", url, e);
                    Reply::failure(format!("I couldn't open {}.", domain))
                }
            })
        }
        AppIntent::Open { query } => {
            let apps = installed_apps().await;
            let app = match_app(&query, &apps)?;
            let path = app.path.to_string_lossy().into_owned();
            Some(match run("open", vec!["-a".into(), path]).await {
                Ok(_) => Reply::text(format!("Opening {}.", app.name)),
                Err(e) => {
                    log::warn!("local_intents: open -a {} failed: {}", app.name, e);
                    Reply::failure(format!("I couldn't open {}.", app.name))
                }
            })
        }
        AppIntent::Quit { query } => {
            let apps = installed_apps().await;
            let app = match_app(&query, &apps)?;
            Some(match quit_installed(app).await {
                Ok(out) if out.trim() == "not_running" => {
                    Reply::text(format!("{} isn't running.", app.name))
                }
                Ok(_) => Reply::text(format!("Quitting {}.", app.name)),
                Err(e) => {
                    log::warn!("local_intents: quit {} failed: {}", app.name, e);
                    Reply::failure(format!("I couldn't quit {}.", app.name))
                }
            })
        }
    }
}

#[cfg(test)]
mod tests {
    use super::super::utterance::normalize;
    use super::*;

    fn p(q: &str) -> Option<AppIntent> {
        normalize(q).and_then(|u| parse(&u))
    }

    fn open(q: &str) -> Option<AppIntent> {
        Some(AppIntent::Open { query: q.into() })
    }

    fn installed(names: &[&str]) -> Vec<InstalledApp> {
        names
            .iter()
            .map(|n| InstalledApp {
                name: n.to_string(),
                path: PathBuf::from(format!("/Applications/{}.app", n)),
            })
            .collect()
    }

    #[test]
    fn open_and_quit_phrasings() {
        assert_eq!(p("open Safari"), open("safari"));
        assert_eq!(p("Hey Juno, open Safari please."), open("safari"));
        assert_eq!(p("launch the Notes app"), open("notes"));
        assert_eq!(p("switch to Slack"), open("slack"));
        assert_eq!(p("open up Google Chrome"), open("google chrome"));
        assert_eq!(
            p("quit Slack"),
            Some(AppIntent::Quit {
                query: "slack".into()
            })
        );
        assert_eq!(
            p("exit out of Zoom"),
            Some(AppIntent::Quit {
                query: "zoom".into()
            })
        );
    }

    #[test]
    fn websites() {
        let site = |d: &str| {
            Some(AppIntent::Website {
                domain: d.to_string(),
            })
        };
        assert_eq!(p("open github.com"), site("github.com"));
        assert_eq!(p("go to www.nytimes.com"), site("nytimes.com"));
        assert_eq!(p("open github dot com"), site("github.com"));
        assert_eq!(
            p("visit news.ycombinator.com"),
            site("news.ycombinator.com")
        );
        // Files and unknown TLDs are not websites; they reach the agent.
        assert_eq!(p("open main.rs"), None);
        assert_eq!(p("open notes.md"), None);
        assert_eq!(p("go to github"), None);
    }

    #[test]
    fn compound_or_descriptive_requests_reach_the_agent() {
        for q in [
            "open Safari and find me flights to Denver",
            "open Safari then search for flights",
            "open a new tab",
            "how do I quit Vim",
            "quit",
            "open",
            "close Slack",
            "force quit Chrome",
            "go to sleep",
            "open github.com and star the repo",
        ] {
            let parsed = p(q);
            let acted = match &parsed {
                // "open a new tab" parses as an app query; it must not match
                // any installed app.
                Some(AppIntent::Open { query }) | Some(AppIntent::Quit { query }) => {
                    match_app(query, &installed(&["Safari", "Slack", "Google Chrome"])).is_some()
                }
                Some(AppIntent::Website { .. }) => true,
                None => false,
            };
            assert!(!acted, "{:?} should reach the agent, got {:?}", q, parsed);
        }
    }

    #[test]
    fn resolves_against_installed_apps_only() {
        let apps = installed(&[
            "Safari",
            "Google Chrome",
            "Microsoft Word",
            "Visual Studio Code",
            "System Settings",
            "1Password 7",
            "Juno",
            "Find My",
        ]);
        let hit = |q: &str| match_app(&app_key(q), &apps).map(|a| a.name.clone());
        assert_eq!(hit("safari"), Some("Safari".into()));
        assert_eq!(hit("SAFARI"), Some("Safari".into()));
        assert_eq!(hit("chrome"), Some("Google Chrome".into()));
        assert_eq!(hit("google chrome"), Some("Google Chrome".into()));
        assert_eq!(hit("word"), Some("Microsoft Word".into()));
        assert_eq!(hit("vs code"), Some("Visual Studio Code".into()));
        assert_eq!(hit("settings"), Some("System Settings".into()));
        assert_eq!(hit("1password 7"), Some("1Password 7".into()));
        assert_eq!(hit("find my"), Some("Find My".into()));
        assert_eq!(hit("juno"), None, "never quit or reopen Juno by voice");
        assert_eq!(hit("flight tracker"), None);
        assert_eq!(hit("safari find flights"), None);
    }

    /// The real safeguard, pinned.
    ///
    /// `QUIT_SCRIPT` addresses an app by name, and AppleScript resolves a name
    /// when it compiles the script: a name macOS cannot place puts a modal
    /// "Where is ...?" picker in front of the person. `argv` does not prevent
    /// that (`id of application (item 1 of argv)` was observed prompting), so
    /// what makes this call safe is that the name came from a bundle found on
    /// disk. `quit_installed` takes an `InstalledApp` to keep that true, and
    /// this test fails if the script is ever reached another way.
    #[test]
    fn the_quit_script_is_only_reachable_with_an_installed_app() {
        // Split so this test's own source is not a match for its needle.
        let needle = concat!("osascript(", "QUIT_SCRIPT");
        let src = include_str!("apps.rs");
        assert_eq!(
            src.matches(needle).count(),
            1,
            "the quit script must have exactly one call site"
        );
        let (before, _) = src
            .split_once(needle)
            .expect("the quit script has a call site");
        let enclosing = before
            .rsplit("\nasync fn ")
            .next()
            .expect("rsplit yields at least one piece");
        assert!(
            enclosing.starts_with("quit_installed(app: &InstalledApp)"),
            "the quit script must be called from quit_installed, which can only be \
             handed an app matched against a bundle on disk"
        );

        // And an InstalledApp cannot be conjured from a name that is not on
        // disk: this is the step that makes holding one proof of existence.
        let installed = installed(&["Spotify"]);
        assert!(match_app("spotify", &installed).is_some());
        assert!(match_app(&app_key("Spotfiy"), &installed).is_none());
        assert!(match_app(&app_key("Definitely Not Installed App"), &installed).is_none());
    }

    #[test]
    fn duplicate_names_resolve_to_the_first_folder_but_distinct_apps_are_ambiguous() {
        let mut apps = installed(&["Slack"]);
        apps.push(InstalledApp {
            name: "Slack".into(),
            path: PathBuf::from("/Users/x/Applications/Slack.app"),
        });
        assert_eq!(
            match_app("slack", &apps).map(|a| a.path.clone()),
            Some(PathBuf::from("/Applications/Slack.app"))
        );
        let clash = installed(&["Google Docs", "Microsoft Docs"]);
        assert_eq!(match_app("docs", &clash), None);
    }
}

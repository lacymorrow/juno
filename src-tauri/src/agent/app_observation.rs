//! What macOS says is true about one app, after Juno acted on it.
//!
//! # Why this exists
//!
//! A tool call that launches or focuses an app used to report that the call
//! was dispatched. Dispatched and done are not the same fact: `open -a
//! "Spotify"` exits 0 whether Spotify comes up, comes up behind the editor,
//! comes up minimized, or throws its own error and quits. The model was handed
//! "the command ran" and said "Spotify is open and playing", because nothing
//! in the result contradicted that sentence.
//!
//! So the loop asks macOS afterwards and puts the answer in the tool result.
//! The honest sentence becomes the only one the model's material supports.
//! This module is that question, and the vocabulary for the answer.
//!
//! # Why not a screenshot
//!
//! A screenshot is the most expensive thing in a turn. One of these checks is
//! a single `osascript` call, measured at about 0.8 s on zero, nearly all of
//! it process startup. That is cheap enough to run after a launch and before
//! an action that depends on the app, and nowhere else. A screenshot stays
//! reserved for questions these cannot answer.
//!
//! # Never resolve an app by name
//!
//! AppleScript resolves an application name when the script is compiled, not
//! when it runs, and a name macOS cannot place puts a modal "Where is ...?"
//! file picker in front of the person, listing every app on the Mac. Passing
//! the name through `argv` is not enough on its own: `id of application
//! (item 1 of argv)` prompts too.
//!
//! So nothing here resolves an app name. [`OBSERVE_SCRIPT`] talks only to
//! `System Events`, which is on every Mac, and asks it for processes by
//! bundle identifier or by process name. Existence comes from the `.app`
//! bundles on disk, through the folder scan [`crate::agent::local_intents::apps`]
//! already caches. Both answers are reached without LaunchServices being
//! asked to place a name, so there is no dialog to avoid.
//!
//! # What it cannot answer, and says so
//!
//! - An app named only by bundle id (`open -b com.spotify.client`) or launched
//!   by a command this module cannot parse gets no observation at all. Silence
//!   is correct there; a guess would be the same defect in a new costume.
//! - Window state comes from the accessibility API. Without that permission
//!   the window counts come back unknown and the wording drops to running or
//!   not running. [`Presence::Unknown`] means Juno could not look, and the
//!   sentence says that rather than implying success.

use std::time::Duration;

use serde_json::{json, Value};

/// How long any single `osascript` call may take before it is abandoned.
const SCRIPT_TIMEOUT: Duration = Duration::from_secs(5);

/// How long to keep asking before reporting what the last look found.
///
/// A cold app launch is normally on screen inside a second; three seconds is
/// slow but still a launch. Past that, reporting what is true now beats
/// waiting for a state that may never arrive.
const SETTLE_TIMEOUT: Duration = Duration::from_millis(3000);

/// Gap between settle polls.
const SETTLE_INTERVAL: Duration = Duration::from_millis(300);

/// What Juno found when it looked.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Presence {
    /// The look itself failed, or this is not macOS. Juno does not know.
    Unknown,
    /// Nothing on this Mac answers to that name: no `.app` bundle installed
    /// and no process running under it.
    NotFound,
    /// Installed, not running.
    NotRunning,
    /// Running and frontmost: the app the person is looking at.
    Frontmost,
    /// Running with at least one window the person could see, behind something.
    Background,
    /// Running, and every window it has is minimized.
    Minimized,
    /// Running with no windows at all (a menu-bar agent, or a window that
    /// closed while the process stayed alive).
    NoWindows,
}

impl Presence {
    /// Running, by Juno's own observation. `Unknown` is not running and not
    /// "not running"; it is no answer, so it is excluded here on purpose.
    pub fn is_running(self) -> bool {
        matches!(
            self,
            Presence::Frontmost | Presence::Background | Presence::Minimized | Presence::NoWindows
        )
    }

    /// Juno looked and the app was definitely not there.
    pub fn is_absent(self) -> bool {
        matches!(self, Presence::NotRunning | Presence::NotFound)
    }

    /// The short machine-readable name that goes in the tool result.
    pub fn as_str(self) -> &'static str {
        match self {
            Presence::Unknown => "unknown",
            Presence::NotFound => "no_such_app",
            Presence::NotRunning => "not_running",
            Presence::Frontmost => "running_frontmost",
            Presence::Background => "running_not_frontmost",
            Presence::Minimized => "running_minimized",
            Presence::NoWindows => "running_no_windows",
        }
    }
}

/// One look at one app.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Observation {
    /// The app name as the command named it. Never a path or a bundle id:
    /// this string reaches the person through what Juno says next.
    pub app: String,
    pub presence: Presence,
    /// Windows the app has, when the accessibility API answered.
    pub windows: Option<u32>,
    /// How many of those are minimized.
    pub minimized: Option<u32>,
}

impl Observation {
    fn unknown(app: &str) -> Self {
        Self {
            app: app.to_string(),
            presence: Presence::Unknown,
            windows: None,
            minimized: None,
        }
    }

    /// What Juno saw, as one plain sentence. No verdict on the task, just the
    /// state, so the model cannot round it up.
    pub fn sentence(&self) -> String {
        let app = &self.app;
        match self.presence {
            Presence::Unknown => {
                format!(
                    "Juno could not check whether {app} is running, so its state is unverified."
                )
            }
            Presence::NotFound => format!("Juno could not find an app called {app} on this Mac."),
            Presence::NotRunning => format!("{app} is not running."),
            Presence::Frontmost => format!("{app} is running and frontmost."),
            Presence::Background => {
                format!("{app} is running but is not frontmost, so it is not what the person is looking at.")
            }
            Presence::Minimized => {
                format!("{app} is running but minimized, so it is not on screen.")
            }
            Presence::NoWindows => format!("{app} is running but has no windows open."),
        }
    }

    /// The observation as tool-result fields.
    pub fn to_json(&self) -> Value {
        json!({
            "app": self.app,
            "state": self.presence.as_str(),
            "windows": self.windows,
            "minimized_windows": self.minimized,
            "observed": self.sentence(),
            "verified_by": "osascript",
        })
    }
}

/// How a command meant to touch the app, which decides what "settled" means.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AppCommand {
    /// Meant to put the app in front: `open -a`, `activate`.
    Activate,
    /// Meant to drive an app that may stay in the background:
    /// `tell application "Music" to play`.
    Control,
}

/// The app a tool call acts on, and how the command names it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AppTarget {
    pub app: String,
    pub kind: AppCommand,
    /// The command addresses the app by literal name inside `tell application
    /// "..."`. AppleScript resolves that name when it compiles the script, so
    /// a name macOS cannot place shows the person a modal app picker before
    /// any runtime check could stop it. Such a command is only safe to run
    /// once the app is known to exist.
    pub names_app_literally: bool,
}

/// The app a tool call acts on, when the call names one Juno can check.
///
/// Returns `None` for everything else, including an app named only by bundle
/// id. An unparsed command is left alone rather than guessed at.
pub fn app_command_target(tool_name: &str, input: &Value) -> Option<AppTarget> {
    if tool_name != "bash" {
        return None;
    }
    let command = input.get("command").and_then(Value::as_str)?;
    parse_app_command(command)
}

/// Apps whose state says nothing about the person's task. `System Events` is
/// the scripting bridge itself, running on every Mac.
const UNINTERESTING: &[&str] = &["system events"];

/// Pull the app a shell command activates or drives out of its text.
///
/// Deliberately narrow. It recognises the two forms the prompt teaches
/// (`open -a`, `osascript -e 'tell application "X" to ...'`) plus
/// `activate application "X"`, and nothing else.
pub fn parse_app_command(command: &str) -> Option<AppTarget> {
    let patterns = patterns()?;
    if let Some(caps) = patterns.open_a.captures(command) {
        let raw = caps
            .get(1)
            .or_else(|| caps.get(2))
            .or_else(|| caps.get(3))?
            .as_str();
        return name_of(raw).map(|app| AppTarget {
            app,
            kind: AppCommand::Activate,
            names_app_literally: false,
        });
    }
    if let Some(caps) = patterns.activate_application.captures(command) {
        return name_of(caps.get(1)?.as_str()).map(|app| AppTarget {
            app,
            kind: AppCommand::Activate,
            names_app_literally: true,
        });
    }
    if let Some(caps) = patterns.tell_application.captures(command) {
        let app = name_of(caps.get(1)?.as_str())?;
        let kind = if patterns.activates.is_match(command) {
            AppCommand::Activate
        } else {
            AppCommand::Control
        };
        return Some(AppTarget {
            app,
            kind,
            names_app_literally: true,
        });
    }
    None
}

/// An app name from a path or a bare name, or `None` when there is nothing
/// checkable there.
fn name_of(raw: &str) -> Option<String> {
    let trimmed = raw.trim().trim_matches(|c| c == '"' || c == '\'');
    // A path: use the bundle's own name. `/Applications/Spotify.app` -> Spotify.
    let name = if trimmed.contains('/') {
        std::path::Path::new(trimmed)
            .file_stem()
            .and_then(|s| s.to_str())?
    } else {
        trimmed
    };
    let name = name.trim_end_matches(".app").trim();
    if name.is_empty() || UNINTERESTING.contains(&name.to_lowercase().as_str()) {
        return None;
    }
    // A bundle id is not a name anyone can read, and it is not what the
    // observation script matches on. Leave it unobserved.
    if name.split('.').count() >= 3 && !name.contains(' ') {
        return None;
    }
    Some(name.to_string())
}

struct Patterns {
    open_a: regex::Regex,
    tell_application: regex::Regex,
    activate_application: regex::Regex,
    activates: regex::Regex,
}

impl Patterns {
    fn compile() -> Result<Self, regex::Error> {
        Ok(Self {
            // `-a` may arrive combined with other short flags (`open -na
            // ...`, `open -gja ...`), so the flag group only has to end in
            // `a`. `open -b <bundle id>` does not match, which is correct:
            // there is no app name in it to check.
            open_a: regex::Regex::new(
                r#"(?i)\bopen\s+(?:-[a-z]+\s+)*-[a-z]*a\s+(?:"([^"]+)"|'([^']+)'|([^\s;|&'"]+))"#,
            )?,
            tell_application: regex::Regex::new(r#"(?i)\btell\s+application\s+"([^"]+)""#)?,
            activate_application: regex::Regex::new(r#"(?i)\bactivate\s+application\s+"([^"]+)""#)?,
            activates: regex::Regex::new(r"(?i)\bactivate\b")?,
        })
    }
}

static PATTERNS: once_cell::sync::Lazy<Option<Patterns>> = once_cell::sync::Lazy::new(|| {
    match Patterns::compile() {
        Ok(p) => Some(p),
        Err(e) => {
            // Literal patterns, so a failure means a developer broke one. No
            // panic (house rule); observation simply switches off and the loop
            // keeps working exactly as it did before this module existed.
            tracing::error!("app_observation: regex failed to compile ({e}); observation disabled");
            None
        }
    }
});

fn patterns() -> Option<&'static Patterns> {
    PATTERNS.as_ref()
}

/// True when this tool call physically acts on whatever is on screen.
///
/// Reuses [`crate::agent::tools::anthropic_computer_use::is_ui_modifying_action`],
/// the single list of mutating actions, so a new action added there is covered
/// here for free. Toolset member calls arrive already routed to `computer`
/// with the member name in `action`, so one arm covers both wire shapes.
pub fn acts_on_screen(tool_name: &str, input: &Value) -> bool {
    use crate::agent::tools::anthropic_computer_use::is_ui_modifying_action;
    if tool_name == "computer" {
        return input
            .get("action")
            .and_then(Value::as_str)
            .is_some_and(is_ui_modifying_action);
    }
    is_ui_modifying_action(tool_name)
}

/// Read the pipe-delimited record the observation script returns.
///
/// Shape: `running|frontmost|windows|minimized`, with `?` for a field the
/// script could not read. `installed` comes from the on-disk bundle scan, not
/// from the script, and separates "installed but not running" from "no such
/// app". Pure, so the state naming is tested without a Mac in the loop.
pub fn parse_record(app: &str, raw: &str, installed: bool) -> Observation {
    let fields: Vec<&str> = raw.trim().split('|').collect();
    let field = |i: usize| fields.get(i).map(|s| s.trim()).unwrap_or("?");
    let number = |i: usize| field(i).parse::<u32>().ok();

    let windows = number(2);
    let minimized = number(3);

    let presence = match field(0) {
        "no" if installed => Presence::NotRunning,
        "no" => Presence::NotFound,
        "yes" => match (field(1), windows, minimized) {
            ("yes", _, _) => Presence::Frontmost,
            // No window information: say running, claim nothing about screen.
            ("no", None, _) => Presence::NoWindows,
            ("no", Some(0), _) => Presence::NoWindows,
            ("no", Some(w), Some(m)) if m >= w => Presence::Minimized,
            ("no", Some(_), _) => Presence::Background,
            _ => Presence::Unknown,
        },
        _ => Presence::Unknown,
    };

    Observation {
        app: app.to_string(),
        presence,
        windows,
        minimized,
    }
}

/// Running, frontmost, and window state for one app.
///
/// Talks only to `System Events`, which exists on every Mac, and matches the
/// process by bundle identifier when one is known and by process name
/// otherwise. No statement in here asks macOS to resolve an application name,
/// which is what keeps the modal app picker off the person's screen. Do not
/// add `tell application <name>`, `application <name> is running`, or `id of
/// application <name>` to this script: each of those resolves a name, and the
/// last one prompts even when the name arrives through `argv`.
const OBSERVE_SCRIPT: &str = r#"on run argv
	set wantId to item 1 of argv
	set wantName to item 2 of argv
	set isRunning to "no"
	set isFrontmost to "?"
	set windowCount to "?"
	set minimizedCount to "?"
	try
		tell application "System Events"
			set procs to {}
			if wantId is not "" then
				set procs to (every application process whose bundle identifier is wantId)
			end if
			if (count of procs) is 0 then
				set procs to (every application process whose name is wantName)
			end if
			if (count of procs) > 0 then
				set isRunning to "yes"
				set p to item 1 of procs
				if frontmost of p then
					set isFrontmost to "yes"
				else
					set isFrontmost to "no"
				end if
				try
					set ws to windows of p
					set windowCount to (count of ws) as text
					set m to 0
					repeat with w in ws
						try
							if value of attribute "AXMinimized" of w is true then set m to m + 1
						end try
					end repeat
					set minimizedCount to m as text
				end try
			end if
		end tell
	on error
		set isRunning to "?"
	end try
	return isRunning & "|" & isFrontmost & "|" & windowCount & "|" & minimizedCount
end run"#;

/// Run the constant observation script. The app's identifiers arrive as
/// `argv`, never spliced into the script text, and never through a shell.
async fn run_observe_script(bundle_id: String, name: String) -> Result<String, String> {
    let args = vec![
        "-e".to_string(),
        OBSERVE_SCRIPT.to_string(),
        bundle_id,
        name,
    ];
    let task = tokio::task::spawn_blocking(move || {
        let out = std::process::Command::new("osascript")
            .args(&args)
            .output()
            .map_err(|e| format!("osascript failed to start: {e}"))?;
        if out.status.success() {
            Ok(String::from_utf8_lossy(&out.stdout).trim().to_string())
        } else {
            Err(String::from_utf8_lossy(&out.stderr).trim().to_string())
        }
    });
    match tokio::time::timeout(SCRIPT_TIMEOUT, task).await {
        Ok(Ok(result)) => result,
        Ok(Err(e)) => Err(format!("osascript task failed: {e}")),
        Err(_) => Err("osascript timed out".to_string()),
    }
}

/// The bundle identifier in an `.app` bundle on disk.
///
/// Read straight out of `Info.plist` with `plutil`, so LaunchServices is never
/// asked to place a name. Roughly 50 ms.
async fn bundle_id_on_disk(app_path: &std::path::Path) -> Option<String> {
    let plist = app_path.join("Contents/Info.plist");
    let args = vec![
        "-extract".to_string(),
        "CFBundleIdentifier".to_string(),
        "raw".to_string(),
        "-o".to_string(),
        "-".to_string(),
        plist.to_string_lossy().into_owned(),
    ];
    let out = tokio::task::spawn_blocking(move || {
        std::process::Command::new("plutil").args(&args).output()
    })
    .await
    .ok()?
    .ok()?;
    if !out.status.success() {
        return None;
    }
    let id = String::from_utf8_lossy(&out.stdout).trim().to_string();
    (!id.is_empty()).then_some(id)
}

/// Who an app is on disk. Resolved once per question, not once per look, so a
/// settle loop does not rescan the app folders or re-read `Info.plist` ten
/// times while it waits.
#[derive(Debug, Clone, Default)]
struct AppIdentity {
    /// An `.app` bundle on this Mac answers to the name.
    installed: bool,
    /// Its bundle identifier, or empty when there is nothing to read. The
    /// script matches on this first, which is what makes a name that differs
    /// from the process name ("Visual Studio Code" runs as "Code") answer
    /// correctly.
    bundle_id: String,
}

async fn identify(app: &str) -> AppIdentity {
    match crate::agent::local_intents::apps::resolve_installed(app).await {
        Some(found) => AppIdentity {
            installed: true,
            bundle_id: bundle_id_on_disk(&found.path).await.unwrap_or_default(),
        },
        None => AppIdentity::default(),
    }
}

async fn observe_identified(app: &str, who: &AppIdentity) -> Observation {
    match run_observe_script(who.bundle_id.clone(), app.to_string()).await {
        Ok(raw) => parse_record(app, &raw, who.installed),
        Err(e) => {
            log::warn!("app_observation: could not observe {app}: {e}");
            Observation::unknown(app)
        }
    }
}

/// Look at one app now.
pub async fn observe(app: &str) -> Observation {
    if !cfg!(target_os = "macos") {
        return Observation::unknown(app);
    }
    let who = identify(app).await;
    observe_identified(app, &who).await
}

/// Whether a screen action may go ahead, given the watched app's state.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ScreenAction {
    /// The app is there, or Juno could not tell. Act.
    Act,
    /// The app is gone. Hand the model this instead of clicking.
    Ask(Value),
}

/// Decide what to do with a screen action after looking at the app the run has
/// been working in.
///
/// Pure, so the decision is tested without a Mac. `Unknown` acts: refusing on
/// a failed look would be its own invented answer, and the person would be
/// asked a question about nothing.
pub fn screen_action_for(app: &str, action: &str, presence: Presence) -> ScreenAction {
    if presence.is_absent() {
        ScreenAction::Ask(world_changed_result(app, action))
    } else {
        ScreenAction::Act
    }
}

/// What a launch or focus command actually did.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LaunchReport {
    pub before: Presence,
    pub after: Observation,
    /// Juno saw the app running at some point during the wait.
    pub seen_running: bool,
    pub waited_ms: u64,
}

impl LaunchReport {
    /// The one sentence that goes to the model, in the words the state
    /// deserves. It never says the task succeeded, only what is true.
    pub fn sentence(&self) -> String {
        let app = &self.after.app;
        let state = self.after.sentence();
        match self.after.presence {
            Presence::Unknown | Presence::NotFound => state,
            Presence::NotRunning => {
                if self.seen_running {
                    format!("{app} launched and then exited: {state}")
                } else {
                    format!("{state} The command ran, but nothing came up.")
                }
            }
            _ if self.before.is_running() => format!("{app} was already running. {state}"),
            Presence::Frontmost => format!("{app} launched and is now frontmost."),
            Presence::Background => format!("{app} launched but is not frontmost. {state}"),
            Presence::Minimized => format!("{app} launched but is minimized. {state}"),
            Presence::NoWindows => format!("{app} launched but has no windows open."),
        }
    }

    /// True when what Juno found is not what the command was asking for, so
    /// the model must not describe the step as done.
    pub fn fell_short(&self) -> bool {
        !matches!(self.after.presence, Presence::Frontmost)
    }

    /// The fields appended to the tool result.
    pub fn to_json(&self) -> Value {
        let mut value = self.after.to_json();
        if let Value::Object(map) = &mut value {
            map.insert("observed".to_string(), json!(self.sentence()));
            map.insert(
                "was_running_before".to_string(),
                json!(self.before.is_running()),
            );
            map.insert("waited_ms".to_string(), json!(self.waited_ms));
        }
        value
    }
}

/// Watch an app settle after a command that targeted it, then report.
///
/// `Activate` waits for frontmost, because that is what the command asked
/// for. `Control` waits only for running: `tell application "Music" to play`
/// has no business stealing focus, and waiting for focus it will never take
/// would burn the whole settle window on every call.
pub async fn settle(app: &str, command: AppCommand, before: Presence) -> LaunchReport {
    let started = std::time::Instant::now();
    let mut seen_running = before.is_running();
    if !cfg!(target_os = "macos") {
        return LaunchReport {
            before,
            after: Observation::unknown(app),
            seen_running,
            waited_ms: 0,
        };
    }
    // Resolved once. An app that was not installed a moment ago may have
    // finished installing, but not inside a three second launch wait.
    let who = identify(app).await;
    let mut last = observe_identified(app, &who).await;

    while started.elapsed() < SETTLE_TIMEOUT {
        if last.presence.is_running() {
            seen_running = true;
        }
        let settled = match command {
            AppCommand::Activate => matches!(last.presence, Presence::Frontmost),
            AppCommand::Control => last.presence.is_running(),
        };
        // Nothing to wait for: an app that is not there will not appear, and
        // an unreadable state will not become readable.
        if settled || matches!(last.presence, Presence::NotFound | Presence::Unknown) {
            break;
        }
        tokio::time::sleep(SETTLE_INTERVAL).await;
        last = observe_identified(app, &who).await;
    }

    if last.presence.is_running() {
        seen_running = true;
    }
    LaunchReport {
        before,
        after: last,
        seen_running,
        waited_ms: started.elapsed().as_millis() as u64,
    }
}

/// The result handed back instead of acting, when the app the run was working
/// in is gone.
///
/// An error shape on purpose: the runner already halts the rest of a batch on
/// a failed screen action, so the planned clicks behind this one never land on
/// whatever took the app's place.
pub fn world_changed_result(app: &str, action: &str) -> Value {
    crate::agent::tools::anthropic_computer_use::create_anthropic_error_response(format!(
        "{app} is no longer running. Juno opened it earlier in this task and it has since \
         closed, so the {action} was not performed. Tell the person {app} closed and ask \
         whether they still want this done. Do not reopen it or act on it again without an \
         answer."
    ))
}

/// The result handed back instead of running a command that addresses an app
/// macOS cannot place.
///
/// Running it would hand the person a modal file picker listing every app on
/// the Mac, which is not an error anyone can act on. So the command does not
/// run, and the model is told to ask in plain words. No path, no bundle id,
/// and no mention of AppleScript reaches the person.
pub fn unknown_app_result(app: &str) -> Value {
    crate::agent::tools::anthropic_computer_use::create_anthropic_error_response(format!(
        "Juno could not find an app called {app} on this Mac, so the command was not run. \
         Tell the person you could not find {app} and ask which app they meant. Do not \
         guess another name, and do not mention file paths, bundle identifiers or \
         AppleScript."
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn obs(raw: &str) -> Observation {
        parse_record("Spotify", raw, true)
    }

    /// The whole point of the script's shape. AppleScript resolves an app name
    /// at compile time, so any name but `System Events` inside `tell
    /// application "..."` risks the modal picker, and the three name-resolving
    /// forms must stay out entirely. Asserted on the text, never by running it
    /// at a missing app: that experiment is the harm.
    #[test]
    fn the_script_never_asks_macos_to_resolve_an_app_name() {
        let tells: Vec<&str> = OBSERVE_SCRIPT
            .match_indices("tell application")
            .map(|(i, _)| &OBSERVE_SCRIPT[i..])
            .collect();
        assert_eq!(tells.len(), 1, "exactly one tell block, to System Events");
        assert!(
            tells[0].starts_with("tell application \"System Events\""),
            "the only app addressed by name must be System Events"
        );
        for forbidden in [
            "id of application",
            "application appName",
            "application wantName",
            "application (item",
            "activate application",
        ] {
            assert!(
                !OBSERVE_SCRIPT.contains(forbidden),
                "{forbidden:?} resolves an app name and can show the person a modal picker"
            );
        }
        // The target arrives as data, the way `local_intents/apps.rs` does it.
        assert!(OBSERVE_SCRIPT.contains("item 1 of argv"));
        assert!(OBSERVE_SCRIPT.contains("whose bundle identifier is wantId"));
        assert!(OBSERVE_SCRIPT.contains("whose name is wantName"));
    }

    #[test]
    fn a_failed_launch_and_a_success_are_different_facts() {
        assert_eq!(
            obs("yes|yes|1|0").presence,
            Presence::Frontmost,
            "running and frontmost is the only success"
        );
        assert_eq!(obs("no|?|?|?").presence, Presence::NotRunning);
        assert_eq!(
            parse_record("Spotify", "no|?|?|?", false).presence,
            Presence::NotFound
        );
    }

    #[test]
    fn a_minimized_window_is_not_an_open_one() {
        let minimized = obs("yes|no|1|1");
        assert_eq!(minimized.presence, Presence::Minimized);
        assert!(minimized.sentence().contains("minimized"));
        assert!(!minimized.sentence().contains("frontmost."));

        let behind = obs("yes|no|2|1");
        assert_eq!(behind.presence, Presence::Background);
        assert!(behind.sentence().contains("not frontmost"));
    }

    #[test]
    fn unreadable_window_state_claims_nothing_about_the_screen() {
        // Accessibility refused, so windows are unknown. Running is still
        // known, and the sentence stops there.
        let o = obs("yes|no|?|?");
        assert_eq!(o.presence, Presence::NoWindows);
        assert!(o.sentence().contains("no windows"));

        let blind = parse_record("Spotify", "?|?|?|?", false);
        assert_eq!(blind.presence, Presence::Unknown);
        assert!(blind.sentence().contains("unverified"));
        assert!(!blind.presence.is_running());
        assert!(!blind.presence.is_absent());
    }

    #[test]
    fn launch_wording_names_what_was_seen() {
        let report = |before: Presence, raw: &str, seen: bool| LaunchReport {
            before,
            after: obs(raw),
            seen_running: seen,
            waited_ms: 500,
        };

        let exited = report(Presence::NotRunning, "no|?|?|?", true);
        assert!(exited.sentence().contains("launched and then exited"));
        assert!(exited.fell_short());

        let never = report(Presence::NotRunning, "no|?|?|?", false);
        assert!(never.sentence().contains("nothing came up"));

        let behind = report(Presence::NotRunning, "yes|no|1|0", true);
        assert!(behind.sentence().contains("launched but is not frontmost"));
        assert!(behind.fell_short());

        let hidden = report(Presence::NotRunning, "yes|no|1|1", true);
        assert!(hidden.sentence().contains("launched but is minimized"));

        let good = report(Presence::NotRunning, "yes|yes|1|0", true);
        assert_eq!(good.sentence(), "Spotify launched and is now frontmost.");
        assert!(!good.fell_short());

        let already = report(Presence::Background, "yes|yes|1|0", true);
        assert!(already
            .sentence()
            .starts_with("Spotify was already running."));
    }

    #[test]
    fn the_result_the_model_receives_carries_the_observation() {
        let report = LaunchReport {
            before: Presence::NotRunning,
            after: obs("no|?|?|?"),
            seen_running: true,
            waited_ms: 3100,
        };
        let json = report.to_json();
        assert_eq!(json["state"], "not_running");
        assert_eq!(json["was_running_before"], false);
        assert_eq!(json["waited_ms"], 3100);
        assert!(json["observed"]
            .as_str()
            .is_some_and(|s| s.contains("launched and then exited")));
    }

    #[test]
    fn launch_and_focus_commands_are_recognised() {
        let target = |c: &str| parse_app_command(c);
        let activate = |app: &str, literal: bool| {
            Some(AppTarget {
                app: app.into(),
                kind: AppCommand::Activate,
                names_app_literally: literal,
            })
        };
        assert_eq!(target("open -a \"Spotify\""), activate("Spotify", false));
        assert_eq!(target("open -a Spotify"), activate("Spotify", false));
        assert_eq!(
            target("open -na '/Applications/Visual Studio Code.app'"),
            activate("Visual Studio Code", false)
        );
        assert_eq!(
            target("osascript -e 'tell application \"Spotify\" to activate'"),
            activate("Spotify", true)
        );
        assert_eq!(
            target("osascript -e 'activate application \"Slack\"'"),
            activate("Slack", true)
        );
        assert_eq!(
            target("osascript -e 'tell application \"Music\" to play'"),
            Some(AppTarget {
                app: "Music".into(),
                kind: AppCommand::Control,
                names_app_literally: true,
            })
        );
    }

    /// `open -a` fails cleanly on a name macOS cannot place; `tell application
    /// "..."` shows the picker. Only the second form needs the existence gate,
    /// and this is the flag the runner reads to tell them apart.
    #[test]
    fn only_the_tell_form_is_flagged_as_naming_an_app_literally() {
        assert_eq!(
            parse_app_command("open -a Spotify").map(|t| t.names_app_literally),
            Some(false)
        );
        assert_eq!(
            parse_app_command("osascript -e 'tell application \"Spotify\" to pause'")
                .map(|t| t.names_app_literally),
            Some(true)
        );
    }

    #[test]
    fn commands_juno_cannot_check_get_no_observation() {
        for command in [
            // A bundle id is not a name the observation script matches on.
            "open -b com.spotify.client",
            // The scripting bridge itself is always running; its state says
            // nothing about the task.
            "osascript -e 'tell application \"System Events\" to keystroke \"n\"'",
            "ls /Applications",
            "echo open -a",
            "defaults write com.apple.finder AppleShowAllFiles true",
        ] {
            assert_eq!(
                parse_app_command(command),
                None,
                "{command:?} should be left unobserved"
            );
        }
    }

    #[test]
    fn only_bash_calls_are_scanned_for_app_commands() {
        let command = json!({ "command": "open -a Spotify" });
        assert!(app_command_target("bash", &command).is_some());
        assert_eq!(app_command_target("computer", &command), None);
        assert_eq!(
            app_command_target("bash", &json!({ "action": "key" })),
            None
        );
    }

    #[test]
    fn screen_actions_are_the_ones_the_app_has_to_be_there_for() {
        assert!(acts_on_screen(
            "computer",
            &json!({ "action": "left_click" })
        ));
        assert!(acts_on_screen("computer", &json!({ "action": "type" })));
        // Routed toolset member calls arrive in this same shape.
        assert!(acts_on_screen(
            "computer",
            &json!({ "action": "key", "toolset_name": "computer" })
        ));
        assert!(!acts_on_screen(
            "computer",
            &json!({ "action": "screenshot" })
        ));
        assert!(!acts_on_screen("bash", &json!({ "command": "ls" })));
        assert!(!acts_on_screen("read_file", &json!({ "path": "/tmp/x" })));
    }

    #[test]
    fn the_world_changed_result_asks_instead_of_acting() {
        let value = world_changed_result("Spotify", "left_click");
        assert!(crate::agent::tools::anthropic_computer_use::is_anthropic_error_response(&value));
        let text = serde_json::to_string(&value).unwrap_or_default();
        assert!(text.contains("no longer running"));
        assert!(text.contains("ask"));
        assert!(text.contains("not performed"));
    }

    /// Bug 20, in one assertion. Juno opened Spotify, the person closed it,
    /// and the next step in the plan was a click on the play button. The click
    /// must not happen, and what reaches the model must be the question.
    #[test]
    fn a_closed_app_turns_the_next_click_into_a_question() {
        let gone = screen_action_for("Spotify", "left_click", Presence::NotRunning);
        let ScreenAction::Ask(value) = gone else {
            panic!("a closed app must not be clicked in");
        };
        let text = serde_json::to_string(&value).unwrap_or_default();
        assert!(text.contains("Spotify is no longer running"));
        assert!(text.contains("ask whether they still want this done"));

        // Still there, in any of its running shapes: act.
        for presence in [
            Presence::Frontmost,
            Presence::Background,
            Presence::Minimized,
            Presence::NoWindows,
        ] {
            assert_eq!(
                screen_action_for("Spotify", "left_click", presence),
                ScreenAction::Act,
                "{presence:?} is still running, so the action proceeds"
            );
        }

        // Juno could not look. Acting is right: a failed check is not a
        // closed app, and asking about nothing is its own invention.
        assert_eq!(
            screen_action_for("Spotify", "left_click", Presence::Unknown),
            ScreenAction::Act
        );
    }

    /// Lacy, on the modal picker: "some users don't even know what a filepath
    /// is. we need to be user friendly." So the not-found ask says the app was
    /// not found and asks which one was meant, and nothing in it is machinery.
    #[test]
    fn the_unknown_app_ask_is_plain_and_mentions_no_machinery() {
        let value = unknown_app_result("Spotfiy");
        assert!(crate::agent::tools::anthropic_computer_use::is_anthropic_error_response(&value));
        let text = serde_json::to_string(&value).unwrap_or_default();
        assert!(text.contains("could not find an app called Spotfiy"));
        assert!(text.contains("was not run"));
        assert!(text.contains("ask which app they meant"));
        assert!(text.contains("do not mention file paths"));
    }
}

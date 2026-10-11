//! "Open settings" follows the app you were in.
//!
//! A bare "open settings" opens the settings of the app that was frontmost,
//! says which one it opened, and offers the other likely targets as chips. If
//! that was wrong, a short correction ("no, not that", "Mac settings", "the
//! other one") closes what Juno opened and opens the right one.
//!
//! # Which app was "in front"
//!
//! The floating bar never activates Juno, so while the person talks to the
//! bar the frontmost app is still the one they were using: reading it when the
//! command arrives gives the pre-Juno app without any tracking. When Juno
//! itself is frontmost (its chat or settings window), the person was in Juno,
//! and the answer is Juno's settings.
//!
//! # Resolution order
//!
//! 1. An explicit target wins: "Juno settings", "Mac settings" (a pane if one
//!    is named), "<App> settings".
//! 2. A bare phrase uses the frontmost regular app, by pressing its
//!    "Settings..." or "Preferences..." menu item (Cmd+, when the menu cannot
//!    be read).
//! 3. Finder, no frontmost app, a non-regular app, or an app with no settings
//!    item: System Settings.
//! 4. Juno frontmost: Juno's settings.
//!
//! # Corrections
//!
//! The last action is remembered for [`CORRECTION_WINDOW`], and forgotten by
//! the next unrelated turn. Correction phrases are anchored local intents like
//! every other grammar here, and match only while that memory is live, so
//! "no" at any other time still reaches the agent.
//!
//! # What closing may touch
//!
//! A correction or a tapped chip closes what Juno opened, and only what it can
//! identify exactly ([`close_plan`]):
//!
//! - Juno's own Settings window: closed.
//! - System Settings: quit, but only when Juno launched it (it was not running
//!   before the action). Never when it was already open.
//! - Another app's settings: only a window that was not in the app's window
//!   list before the action and is in it after, matched by its window number,
//!   and closed with its AX close button.
//! - Anything else, including a settings window that belongs to a different
//!   app than the one pressed (Ghostty's "Settings..." opens its config file in
//!   TextEdit): nothing is closed and the alternative simply opens.
//!
//! Never Cmd+Q, never an app quit for any app but System Settings Juno itself
//! launched, never a window that existed before. The first version closed
//! "the first window" of the pressed app. For Ghostty that was its terminal
//! window, and Ghostty answered by asking to quit all of Ghostty.
//!
//! The person's words never reach a script: app names for the Settings press
//! come from the running process list or from a bundle matched on disk, and
//! travel as `argv`.

use std::path::PathBuf;
use std::sync::Mutex;
use std::time::{Duration, Instant};

use once_cell::sync::Lazy;
use regex::Regex;
use serde::Serialize;
use tauri::{AppHandle, Emitter};

use super::{apps, compile, osascript, run, Reply};

/// How long after opening a settings window a correction still applies.
pub const CORRECTION_WINDOW: Duration = Duration::from_secs(60);

/* --------------------------------- panes --------------------------------- */

/// A System Settings pane that can be named aloud.
#[derive(Debug, PartialEq, Eq)]
pub struct Pane {
    /// Normalized words that name it.
    names: &'static [&'static str],
    /// Spoken and shown name.
    label: &'static str,
    /// `x-apple.systempreferences:` identifier.
    id: &'static str,
}

const PANES: &[Pane] = &[
    Pane {
        names: &["wifi", "wi fi", "wi-fi"],
        label: "Wi-Fi",
        id: "com.apple.wifi-settings-extension",
    },
    Pane {
        names: &["bluetooth"],
        label: "Bluetooth",
        id: "com.apple.BluetoothSettings",
    },
    Pane {
        names: &["network"],
        label: "Network",
        id: "com.apple.Network-Settings.extension",
    },
    Pane {
        names: &["sound", "audio"],
        label: "Sound",
        id: "com.apple.Sound-Settings.extension",
    },
    Pane {
        names: &["display", "displays"],
        label: "Displays",
        id: "com.apple.Displays-Settings.extension",
    },
    Pane {
        names: &["notification", "notifications"],
        label: "Notifications",
        id: "com.apple.Notifications-Settings.extension",
    },
    Pane {
        names: &["battery"],
        label: "Battery",
        id: "com.apple.Battery-Settings.extension",
    },
    Pane {
        names: &["keyboard"],
        label: "Keyboard",
        id: "com.apple.Keyboard-Settings.extension",
    },
    Pane {
        names: &["trackpad"],
        label: "Trackpad",
        id: "com.apple.Trackpad-Settings.extension",
    },
    Pane {
        names: &["privacy", "privacy and security"],
        label: "Privacy",
        id: "com.apple.settings.PrivacySecurity.extension",
    },
];

/* -------------------------------- grammar -------------------------------- */

/// What the person asked to open, before it is matched against anything.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Spec {
    Juno,
    /// System Settings, on a pane when one was named.
    Mac(Option<&'static Pane>),
    /// An app, by normalized name.
    App(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SettingsIntent {
    /// "open settings": the app the person was in.
    Bare,
    /// "open Juno settings", "open Mac settings", "open Music settings".
    Explicit(Spec),
}

/// Where a correction sends the person.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Switch {
    /// "the other one": the first alternative that was offered.
    Other,
    To(Spec),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Correction {
    /// "no", "not that", "close it": close what was opened.
    Close,
    /// "no, Mac settings", "the other one": close it, open this.
    Switch(Switch),
}

struct Patterns {
    bare: Regex,
    target: Regex,
    close: Regex,
    switch: Regex,
}

const OPEN_VERB: &str = r"(?:open|show|pull up|bring up|launch)(?: up)?";

impl Patterns {
    fn compile() -> Result<Self, regex::Error> {
        Ok(Self {
            bare: Regex::new(&format!(
                r"^{OPEN_VERB} (?:settings|preferences|prefs)(?: window)?$"
            ))?,
            target: Regex::new(&format!(
                r"^{OPEN_VERB} (.+?) (?:settings|preferences|prefs)(?: window)?$"
            ))?,
            close: Regex::new(
                r"^(?:(?:no|nope|nah) )?(?:no|nope|nah|wrong(?: one)?|not that(?: one)?|(?:that is|thats) (?:wrong|not it)|close (?:it|that|that one|them)|undo(?: that)?)$",
            )?,
            switch: Regex::new(&format!(
                r"^(?:(?:no|nope|nah)(?: not that(?: one)?)? )?(?:(?:{OPEN_VERB}|try|i meant|i said|i want|i wanted|use) )?(?:(other one|other ones|other settings|another one)|(.+?) (?:settings|preferences|prefs)(?: window)?)$"
            ))?,
        })
    }
}

static PATTERNS: Lazy<Option<Patterns>> = Lazy::new(|| compile("settings", Patterns::compile));

/// Turn the words before "settings" into a target.
fn classify(name: &str) -> Option<Spec> {
    let key = apps::app_key(name);
    if key.is_empty() || key.split_whitespace().count() > 3 {
        return None;
    }
    match key.as_str() {
        "juno" | "junos" | "your" => return Some(Spec::Juno),
        "mac" | "macos" | "mac os" | "system" | "computer" | "apple" => {
            return Some(Spec::Mac(None))
        }
        _ => {}
    }
    if let Some(pane) = PANES.iter().find(|p| p.names.contains(&key.as_str())) {
        return Some(Spec::Mac(Some(pane)));
    }
    Some(Spec::App(key))
}

/// Parse a normalized utterance. Pure.
pub fn parse(utterance: &str) -> Option<SettingsIntent> {
    let p = PATTERNS.as_ref()?;
    if p.bare.is_match(utterance) {
        return Some(SettingsIntent::Bare);
    }
    let caps = p.target.captures(utterance)?;
    classify(caps.get(1)?.as_str()).map(SettingsIntent::Explicit)
}

/// Parse a correction from a normalized utterance. Pure; whether one may
/// fire is [`correction_for`]'s question.
pub fn parse_correction(utterance: &str) -> Option<Correction> {
    let p = PATTERNS.as_ref()?;
    if p.close.is_match(utterance) {
        return Some(Correction::Close);
    }
    let caps = p.switch.captures(utterance)?;
    if caps.get(1).is_some() {
        return Some(Correction::Switch(Switch::Other));
    }
    classify(caps.get(2)?.as_str()).map(|s| Correction::Switch(Switch::To(s)))
}

/* ------------------------------- the targets ------------------------------ */

/// A settings window Juno can open and close.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Target {
    Juno,
    Mac(Option<&'static Pane>),
    App {
        /// The process name System Events knows it by (Ghostty's is "ghostty").
        name: String,
        /// The name the person knows it by ("Ghostty"): the app's localized
        /// display name, never the process or bundle name.
        display: String,
        /// Where to launch it from when it is not running yet.
        path: Option<PathBuf>,
    },
}

impl Target {
    /// "Music settings", "Mac settings", "Juno settings", "Wi-Fi settings".
    pub fn label(&self) -> String {
        match self {
            Target::Juno => "Juno settings".into(),
            Target::Mac(None) => "Mac settings".into(),
            Target::Mac(Some(p)) => format!("{} settings", p.label),
            Target::App { display, .. } => format!("{} settings", display),
        }
    }

    fn chip(&self) -> String {
        format!("Open {}", self.label())
    }
}

/// The frontmost process, as System Events reports it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Frontmost {
    pub name: String,
    /// The localized name shown in the Dock and Finder, when System Events
    /// has one.
    pub display: Option<String>,
    pub bundle_id: Option<String>,
    pub pid: i32,
    /// A regular Dock app, not a menu bar extra or a helper.
    pub regular: bool,
}

impl Frontmost {
    /// What to call the app aloud: its localized display name, falling back to
    /// the process name.
    pub fn display_name(&self) -> String {
        self.display
            .as_deref()
            .map(|d| d.trim().trim_end_matches(".app").trim())
            .filter(|d| !d.is_empty())
            .unwrap_or(&self.name)
            .to_string()
    }
}

/// What the frontmost app means for a bare "open settings".
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FrontKind {
    Juno,
    /// Finder, nothing, or something with no settings of its own.
    Mac,
    App(Frontmost),
}

/// Apps that have no settings worth opening: the answer is System Settings.
const NO_SETTINGS_BUNDLES: &[&str] = &[
    "com.apple.finder",
    "com.apple.dock",
    "com.apple.loginwindow",
    "com.apple.systemuiserver",
    "com.apple.controlcenter",
    "com.apple.Spotlight",
    "com.apple.systempreferences",
];

/// Classify the frontmost app. Pure, so resolution order is testable with a
/// fake frontmost app.
pub fn classify_front(front: Option<&Frontmost>, own_pid: i32) -> FrontKind {
    let Some(f) = front else {
        return FrontKind::Mac;
    };
    if f.pid == own_pid {
        return FrontKind::Juno;
    }
    let skip = f
        .bundle_id
        .as_deref()
        .is_none_or(|b| NO_SETTINGS_BUNDLES.contains(&b));
    if !f.regular || skip {
        return FrontKind::Mac;
    }
    FrontKind::App(f.clone())
}

/// The first target a bare "open settings" tries.
pub fn bare_target(kind: &FrontKind) -> Target {
    match kind {
        FrontKind::Juno => Target::Juno,
        FrontKind::Mac => Target::Mac(None),
        FrontKind::App(f) => Target::App {
            name: f.name.clone(),
            display: f.display_name(),
            path: None,
        },
    }
}

/// The app that was in front, when it is one with settings of its own.
fn previous_app(kind: &FrontKind) -> Option<Target> {
    match kind {
        FrontKind::App(_) => Some(bare_target(kind)),
        _ => None,
    }
}

/// The other likely targets, in the order they are offered.
pub fn alternatives(current: &Target, previous: Option<&Target>) -> Vec<Target> {
    let mut all: Vec<Target> = Vec::new();
    match current {
        Target::App { .. } => {
            all.push(Target::Mac(None));
            all.push(Target::Juno);
        }
        Target::Mac(_) => {
            all.extend(previous.cloned());
            all.push(Target::Juno);
        }
        Target::Juno => {
            all.extend(previous.cloned());
            all.push(Target::Mac(None));
        }
    }
    all.retain(|t| t != current);
    all.dedup();
    all
}

/* --------------------------- what was last opened -------------------------- */

#[derive(Debug, Clone)]
struct Session {
    target: Target,
    /// What the action changed, which is all closing may act on.
    opened: Opened,
    previous: Option<Target>,
    alternatives: Vec<Target>,
    at: Instant,
    generation: u64,
}

impl Session {
    fn is_live(&self, now: Instant) -> bool {
        now.saturating_duration_since(self.at) < CORRECTION_WINDOW
    }
}

struct Memory {
    session: Option<Session>,
    generation: u64,
}

static MEMORY: Lazy<Mutex<Memory>> = Lazy::new(|| {
    Mutex::new(Memory {
        session: None,
        generation: 0,
    })
});

/// A live session, if there is one.
fn live_session() -> Option<Session> {
    let guard = MEMORY.lock().ok()?;
    guard
        .session
        .as_ref()
        .filter(|s| s.is_live(Instant::now()))
        .cloned()
}

fn remember(mut session: Session) -> u64 {
    match MEMORY.lock() {
        Ok(mut guard) => {
            guard.generation += 1;
            session.generation = guard.generation;
            let g = session.generation;
            guard.session = Some(session);
            g
        }
        Err(_) => 0,
    }
}

fn take_session() -> Option<Session> {
    MEMORY.lock().ok()?.session.take()
}

/// A correction phrase, when one may fire: only with a live session.
pub fn correction_for(query: &str) -> Option<Correction> {
    live_session()?;
    correction_from(query)
}

/// The correction a query names, ignoring whether one may fire. Pure.
pub fn correction_from(query: &str) -> Option<Correction> {
    let utterance = super::utterance::normalize(query)?;
    // "no thanks" is a refusal of something else, not a correction.
    let bare_no = matches!(utterance.as_str(), "no" | "nope" | "nah");
    if bare_no && query.to_lowercase().contains("thank") {
        return None;
    }
    // The normalizer drops a leading "Juno" as courtesy, which leaves a bare
    // "Juno settings" as just "settings".
    if matches!(utterance.as_str(), "settings" | "preferences" | "prefs")
        && query.to_lowercase().contains("juno")
    {
        return Some(Correction::Switch(Switch::To(Spec::Juno)));
    }
    parse_correction(&utterance)
}

/* ---------------------------------- chips --------------------------------- */

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct Chip {
    pub id: String,
    pub label: String,
}

#[derive(Serialize, Clone)]
struct ChipsPayload {
    chips: Vec<Chip>,
}

fn chips_for(session: &Session) -> Vec<Chip> {
    session
        .alternatives
        .iter()
        .enumerate()
        .map(|(i, t)| Chip {
            id: format!("alt-{}", i),
            label: t.chip(),
        })
        .collect()
}

fn emit_chips(app: &AppHandle, chips: Vec<Chip>) {
    if let Err(e) = app.emit(
        crate::constants::events::chips::REPLY_CHIPS,
        ChipsPayload { chips },
    ) {
        log::warn!("settings_follow: could not publish chips: {}", e);
    }
}

/// Show the alternatives under the reply just given, and arrange for them to
/// go away when the correction window closes. Does nothing without a live
/// session, so it is safe to call after any local reply.
pub fn publish(app: &AppHandle) {
    let Some(session) = live_session() else {
        return;
    };
    emit_chips(app, chips_for(&session));
    let generation = session.generation;
    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        tokio::time::sleep(CORRECTION_WINDOW + Duration::from_secs(1)).await;
        let expired = match MEMORY.lock() {
            Ok(mut guard) => {
                let same = guard
                    .session
                    .as_ref()
                    .is_some_and(|s| s.generation == generation);
                if same {
                    guard.session = None;
                }
                same
            }
            Err(_) => false,
        };
        if expired {
            emit_chips(&app, Vec::new());
        }
    });
}

/// An unrelated turn: the chance to correct has passed.
pub fn forget(app: &AppHandle) {
    if take_session().is_some() {
        emit_chips(app, Vec::new());
    }
}

/* ------------------------- what an action opened ------------------------- */

/// One window of an app, identified by its AX window number. A window with no
/// number cannot be told apart from another, so it is never listed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WinRef {
    pub number: String,
    pub title: String,
}

/// What opening a target changed. Everything closing is allowed to do comes
/// from here and nowhere else.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Opened {
    /// System Settings was already running before the action.
    pub was_running: bool,
    /// The pressed app's windows before the action.
    pub before: Vec<WinRef>,
    /// The pressed app's windows after it.
    pub after: Vec<WinRef>,
}

/// The windows in `after` that were not in `before`. Pure.
pub fn new_windows(before: &[WinRef], after: &[WinRef]) -> Vec<WinRef> {
    after
        .iter()
        .filter(|w| !w.number.is_empty() && !before.iter().any(|b| b.number == w.number))
        .cloned()
        .collect()
}

/// What closing a session may do.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CloseAction {
    /// Close Juno's own Settings window.
    JunoWindow,
    /// Quit System Settings, which Juno launched.
    QuitSystemSettings,
    /// Close exactly these windows of the pressed app, by AX close button.
    CloseWindows(Vec<WinRef>),
    /// Close nothing.
    Nothing,
}

/// Decide what to close for `target`, given what opening it changed. Pure, and
/// the only place that decides: nothing quits an app but System Settings that
/// Juno launched, and no window that existed before is ever closed.
pub fn close_plan(target: &Target, opened: &Opened) -> CloseAction {
    match target {
        Target::Juno => CloseAction::JunoWindow,
        Target::Mac(_) if !opened.was_running => CloseAction::QuitSystemSettings,
        Target::Mac(_) => CloseAction::Nothing,
        Target::App { .. } => {
            let fresh = new_windows(&opened.before, &opened.after);
            if fresh.is_empty() {
                CloseAction::Nothing
            } else {
                CloseAction::CloseWindows(fresh)
            }
        }
    }
}

/* --------------------------------- native --------------------------------- */

const FRONTMOST_SCRIPT: &str = r#"tell application "System Events"
	set p to first application process whose frontmost is true
	set b to ""
	try
		set b to bundle identifier of p
	end try
	set d to ""
	try
		set d to displayed name of p
	end try
	return (name of p) & tab & b & tab & ((unix id of p) as text) & tab & ((background only of p) as text) & tab & d
end tell"#;

/// Parse [`FRONTMOST_SCRIPT`]'s output. Pure.
pub fn parse_frontmost(out: &str) -> Option<Frontmost> {
    let mut parts = out.trim_end_matches(['\n', '\r']).split('\t');
    let name = parts.next()?.trim().to_string();
    let bundle = parts.next()?.trim().to_string();
    let pid = parts.next()?.trim().parse::<i32>().ok()?;
    let background = parts.next()?.trim() == "true";
    let display = parts
        .next()
        .map(|d| d.trim().to_string())
        .filter(|d| !d.is_empty() && d != "missing value");
    if name.is_empty() {
        return None;
    }
    Some(Frontmost {
        name,
        display,
        bundle_id: (!bundle.is_empty()).then_some(bundle),
        pid,
        regular: !background,
    })
}

/// Build a [`Frontmost`] from what `NSRunningApplication` reports. Pure.
///
/// `name` is the executable's file name, which is what System Events calls
/// the process (and what its `process "..."` lookups take); the localized
/// name is the fallback and the display name. `policy` is
/// `NSApplicationActivationPolicy`: 0 is a regular Dock app.
#[cfg(any(target_os = "macos", test))]
fn frontmost_from_parts(
    localized: Option<String>,
    bundle_id: Option<String>,
    executable: Option<String>,
    pid: i32,
    policy: isize,
) -> Option<Frontmost> {
    let clean = |s: Option<String>| s.map(|s| s.trim().to_string()).filter(|s| !s.is_empty());
    let localized = clean(localized);
    let name = clean(executable).or_else(|| localized.clone())?;
    Some(Frontmost {
        name,
        display: localized,
        bundle_id: clean(bundle_id),
        pid,
        regular: policy == 0,
    })
}

/// The frontmost app straight from `NSWorkspace`: microseconds, where the
/// System Events script below takes about half a second.
#[cfg(target_os = "macos")]
fn native_frontmost() -> Option<Frontmost> {
    use objc::runtime::Object;
    use objc::{class, msg_send, sel, sel_impl};

    unsafe fn ns_string(obj: *mut Object) -> Option<String> {
        if obj.is_null() {
            return None;
        }
        let bytes: *const std::os::raw::c_char = msg_send![obj, UTF8String];
        if bytes.is_null() {
            return None;
        }
        std::ffi::CStr::from_ptr(bytes)
            .to_str()
            .ok()
            .map(str::to_string)
    }

    objc::rc::autoreleasepool(|| {
        // SAFETY: plain reads of NSWorkspace and NSRunningApplication
        // properties, each null-checked, inside an autorelease pool.
        unsafe {
            let workspace: *mut Object = msg_send![class!(NSWorkspace), sharedWorkspace];
            if workspace.is_null() {
                return None;
            }
            let front: *mut Object = msg_send![workspace, frontmostApplication];
            if front.is_null() {
                return None;
            }
            let localized: *mut Object = msg_send![front, localizedName];
            let bundle: *mut Object = msg_send![front, bundleIdentifier];
            let pid: i32 = msg_send![front, processIdentifier];
            let policy: isize = msg_send![front, activationPolicy];
            let url: *mut Object = msg_send![front, executableURL];
            let executable: *mut Object = if url.is_null() {
                std::ptr::null_mut()
            } else {
                msg_send![url, lastPathComponent]
            };
            frontmost_from_parts(
                ns_string(localized),
                ns_string(bundle),
                ns_string(executable),
                pid,
                policy,
            )
        }
    })
}

async fn frontmost() -> Option<Frontmost> {
    #[cfg(target_os = "macos")]
    if let Some(front) = native_frontmost() {
        return Some(front);
    }
    let out = osascript(FRONTMOST_SCRIPT, Vec::new()).await.ok()?;
    parse_frontmost(&out)
}

/// Press the app's Settings or Preferences menu item, or send Cmd+, when the
/// menu cannot be read. Lists the app's windows (AX window number and title)
/// before and after, so the caller can tell which window, if any, the press
/// created. Output: the way it was pressed, then `=B` and the windows before,
/// then `=A` and the windows after. Window fields are separated by ASCII 31.
const OPEN_SCRIPT: &str = r#"on listWindows(procName)
	set out to ""
	set sep to (ASCII character 31)
	tell application "System Events"
		tell process procName
			repeat with w in windows
				set n to ""
				try
					set n to (value of attribute "AXWindowNumber" of w) as text
				end try
				set t to ""
				try
					set t to name of w
				end try
				if t is missing value then set t to ""
				set out to out & n & sep & t & linefeed
			end repeat
		end tell
	end tell
	return out
end listWindows

on run argv
	set procName to item 1 of argv
	tell application "System Events"
		if not (exists process procName) then return "missing"
	end tell
	set beforeList to my listWindows(procName)
	tell application "System Events"
		tell process procName
			set frontmost to true
			delay 0.15
			set how to "noitem"
			set menuRead to false
			try
				set items_ to menu items of menu 1 of menu bar item 2 of menu bar 1
				set menuRead to true
				repeat with mi in items_
					set n to missing value
					try
						set n to name of mi
					end try
					if n is not missing value then
						if n starts with "Settings" or n starts with "Preferences" then
							click mi
							set how to "menu"
							exit repeat
						end if
					end if
				end repeat
			end try
			if not menuRead then
				keystroke "," using command down
				set how to "keys"
			end if
			if how is "noitem" then return "noitem"
			delay 0.5
		end tell
	end tell
	set afterList to my listWindows(procName)
	return how & linefeed & "=B" & linefeed & beforeList & "=A" & linefeed & afterList
end run"#;

/// Close the window of `argv[0]` whose AX window number is `argv[1]`, with its
/// close button. It matches by number only: there is no "first window", no
/// title match, no keystroke, and it never quits the app.
const CLOSE_SCRIPT: &str = r#"on run argv
	set procName to item 1 of argv
	set wantNumber to item 2 of argv
	if wantNumber is "" then return "refused"
	tell application "System Events"
		if not (exists process procName) then return "gone"
		tell process procName
			repeat with w in windows
				set n to ""
				try
					set n to (value of attribute "AXWindowNumber" of w) as text
				end try
				if n is wantNumber then
					try
						click (first button of w whose subrole is "AXCloseButton")
						return "closed"
					end try
					return "no_close_button"
				end if
			end repeat
		end tell
	end tell
	return "not_found"
end run"#;

/// Whether System Settings is running, by bundle id (its name is localized).
const SYSTEM_SETTINGS_RUNNING_SCRIPT: &str = r#"tell application "System Events"
	return ((count of (application processes whose bundle identifier is "com.apple.systempreferences")) > 0) as text
end tell"#;

/// Quit System Settings, and only System Settings: used when Juno launched it
/// for the person and they asked for something else. No other app is ever
/// addressed this way.
const QUIT_SYSTEM_SETTINGS_SCRIPT: &str =
    r#"tell application id "com.apple.systempreferences" to quit"#;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OpenError {
    /// The app has no Settings item, or pressing it opened nothing.
    NoItem,
    Failed,
}

/// Read [`OPEN_SCRIPT`]'s output: the windows either side of the press.
pub fn parse_open_result(out: &str) -> Result<Opened, OpenError> {
    let out = out.trim_end_matches(['\n', '\r']);
    if out == "noitem" {
        return Err(OpenError::NoItem);
    }
    let mut lines = out.lines();
    let how = lines.next().ok_or(OpenError::Failed)?.trim();
    if how != "menu" && how != "keys" {
        return Err(OpenError::Failed);
    }
    #[derive(PartialEq)]
    enum Section {
        None,
        Before,
        After,
    }
    let mut section = Section::None;
    let mut opened = Opened::default();
    for line in lines {
        match line {
            "=B" => section = Section::Before,
            "=A" => section = Section::After,
            row => {
                let mut fields = row.split('\u{1f}');
                let number = fields.next().unwrap_or("").trim();
                let title = fields.next().unwrap_or("");
                // No number, no identity: never listed, so never closed.
                if number.is_empty() {
                    continue;
                }
                let win = WinRef {
                    number: number.to_string(),
                    title: title.to_string(),
                };
                match section {
                    Section::Before => opened.before.push(win),
                    Section::After => opened.after.push(win),
                    Section::None => {}
                }
            }
        }
    }
    // A pressed menu item is trusted even when the window was already up. A
    // keystroke proves nothing by itself: only a new window counts.
    if how == "keys" && new_windows(&opened.before, &opened.after).is_empty() {
        return Err(OpenError::NoItem);
    }
    Ok(opened)
}

async fn system_settings_running() -> bool {
    // Unsure counts as running: Juno quits System Settings only when it is
    // certain it started it.
    !matches!(
        osascript(SYSTEM_SETTINGS_RUNNING_SCRIPT, Vec::new())
            .await
            .as_deref(),
        Ok("false")
    )
}

async fn open_target(app: &AppHandle, target: &Target) -> Result<Opened, OpenError> {
    match target {
        Target::Juno => crate::window_management::open_settings_window(app.clone())
            .await
            .map(|_| Opened::default())
            .map_err(|e| {
                log::warn!("settings_follow: Juno settings failed: {}", e);
                OpenError::Failed
            }),
        Target::Mac(pane) => {
            // Asked before opening: only a System Settings Juno starts is
            // Juno's to quit afterwards.
            let was_running = system_settings_running().await;
            let result = match pane {
                Some(p) => run("open", vec![format!("x-apple.systempreferences:{}", p.id)]).await,
                None => match apps::resolve_installed("settings").await {
                    Some(app) => {
                        run(
                            "open",
                            vec!["-a".into(), app.path.to_string_lossy().into_owned()],
                        )
                        .await
                    }
                    None => Err("System Settings is not installed".into()),
                },
            };
            result
                .map(|_| Opened {
                    was_running,
                    ..Opened::default()
                })
                .map_err(|e| {
                    log::warn!("settings_follow: System Settings failed: {}", e);
                    OpenError::Failed
                })
        }
        Target::App { name, path, .. } => {
            if let Some(path) = path {
                if let Err(e) = run(
                    "open",
                    vec!["-a".into(), path.to_string_lossy().into_owned()],
                )
                .await
                {
                    log::warn!("settings_follow: launching {} failed: {}", name, e);
                    return Err(OpenError::Failed);
                }
                tokio::time::sleep(Duration::from_millis(600)).await;
            }
            match osascript(OPEN_SCRIPT, vec![name.clone()]).await {
                Ok(out) => parse_open_result(&out),
                Err(e) => {
                    log::warn!("settings_follow: {} settings press failed: {}", name, e);
                    Err(OpenError::Failed)
                }
            }
        }
    }
}

/// Open `target`; an app with no settings falls back to System Settings.
async fn open_with_fallback(app: &AppHandle, target: Target) -> Result<(Target, Opened), Target> {
    match open_target(app, &target).await {
        Ok(opened) => Ok((target, opened)),
        Err(OpenError::NoItem) => {
            let mac = Target::Mac(None);
            match open_target(app, &mac).await {
                Ok(opened) => Ok((mac, opened)),
                Err(_) => Err(mac),
            }
        }
        Err(OpenError::Failed) => Err(target),
    }
}

async fn close_target(app: &AppHandle, target: &Target, opened: &Opened) {
    match close_plan(target, opened) {
        CloseAction::JunoWindow => {
            if let Err(e) = crate::window_management::close_settings_window(app.clone()).await {
                log::warn!("settings_follow: closing Juno settings failed: {}", e);
            }
        }
        CloseAction::QuitSystemSettings => {
            if let Err(e) = osascript(QUIT_SYSTEM_SETTINGS_SCRIPT, Vec::new()).await {
                log::warn!("settings_follow: quitting System Settings failed: {}", e);
            }
        }
        CloseAction::CloseWindows(windows) => {
            let Target::App { name, .. } = target else {
                return;
            };
            for win in windows {
                match osascript(CLOSE_SCRIPT, vec![name.clone(), win.number.clone()]).await {
                    Ok(result) => log::info!(
                        "settings_follow: closing {} window {} ({}): {}",
                        name,
                        win.number,
                        win.title,
                        result
                    ),
                    Err(e) => {
                        log::warn!("settings_follow: closing a settings window failed: {}", e)
                    }
                }
            }
        }
        CloseAction::Nothing => {
            log::info!(
                "settings_follow: nothing to close for {}; it opens the alternative only",
                target.label()
            );
        }
    }
}

/* -------------------------------- handlers -------------------------------- */

/// Resolve what was said to something that can be opened. `None` when an app
/// name matches nothing installed: the agent gets the request instead.
async fn resolve_spec(spec: &Spec) -> Option<Target> {
    match spec {
        Spec::Juno => Some(Target::Juno),
        Spec::Mac(pane) => Some(Target::Mac(*pane)),
        Spec::App(key) => {
            let app = apps::resolve_installed(key).await?;
            Some(Target::App {
                display: app.name.clone(),
                name: app.name,
                path: Some(app.path),
            })
        }
    }
}

fn own_pid() -> i32 {
    i32::try_from(std::process::id()).unwrap_or(-1)
}

/// The acknowledgement for opening `target`, said before it opens.
fn opening_line(target: &Target) -> String {
    format!("Opening {}.", target.label())
}

async fn open_and_remember(app: &AppHandle, wanted: Target, previous: Option<Target>) -> Reply {
    // Speak first: opening a settings window takes the better part of a
    // second, and the acknowledgement does not depend on how it went.
    super::speak_ahead(app, &opening_line(&wanted));
    match open_with_fallback(app, wanted).await {
        Ok((target, opened)) => {
            let alternatives = alternatives(&target, previous.as_ref());
            remember(Session {
                target: target.clone(),
                opened,
                previous,
                alternatives,
                at: Instant::now(),
                generation: 0,
            });
            Reply::text(opening_line(&target)).unspoken()
        }
        Err(target) => Reply::failure(format!("I couldn't open {}.", target.label())).unspoken(),
    }
}

pub(super) async fn handle(app: &AppHandle, intent: SettingsIntent) -> Option<Reply> {
    let kind = classify_front(frontmost().await.as_ref(), own_pid());
    let previous = previous_app(&kind);
    let wanted = match intent {
        SettingsIntent::Bare => bare_target(&kind),
        SettingsIntent::Explicit(spec) => resolve_spec(&spec).await?,
    };
    // The explicit app, once chosen, is a fine "previous" for later chips.
    let previous = match (&wanted, previous) {
        (Target::App { .. }, None) => Some(wanted.clone()),
        (_, p) => p,
    };
    Some(open_and_remember(app, wanted, previous).await)
}

pub(super) async fn handle_correction(app: &AppHandle, correction: Correction) -> Option<Reply> {
    let session = live_session()?;
    let destination = match &correction {
        Correction::Close => None,
        Correction::Switch(Switch::Other) => Some(session.alternatives.first()?.clone()),
        Correction::Switch(Switch::To(spec)) => Some(resolve_spec(spec).await?),
    };
    take_session();
    if destination.as_ref() != Some(&session.target) {
        close_target(app, &session.target, &session.opened).await;
    }
    match destination {
        None => Some(Reply::text(format!("Closed {}.", session.target.label()))),
        Some(target) => Some(open_and_remember(app, target, session.previous).await),
    }
}

/// A chip was tapped: the same as saying its alternative.
#[tauri::command]
pub async fn run_reply_chip(app: AppHandle, id: String) -> Result<(), String> {
    let session = live_session().ok_or("That suggestion has expired.")?;
    let index: usize = id
        .strip_prefix("alt-")
        .and_then(|n| n.parse().ok())
        .ok_or("Unknown suggestion.")?;
    let target = session
        .alternatives
        .get(index)
        .cloned()
        .ok_or("Unknown suggestion.")?;
    take_session();
    close_target(&app, &session.target, &session.opened).await;
    let reply = open_and_remember(&app, target, session.previous).await;
    // A chip is a new action, not the tail of the turn before it. That turn
    // has ended (and may have been stopped), and speech from a stopped turn is
    // dropped, so the chip opens its own turn to be heard.
    crate::tts::begin_turn();
    super::emit_reply(&app, reply).await;
    publish(&app);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::super::utterance::normalize;
    use super::*;

    #[test]
    fn the_acknowledgement_is_present_tense() {
        let ghostty = Target::App {
            name: "ghostty".to_string(),
            display: "Ghostty".to_string(),
            path: None,
        };
        assert_eq!(opening_line(&ghostty), "Opening Ghostty settings.");
        assert_eq!(opening_line(&Target::Mac(None)), "Opening Mac settings.");
        assert_eq!(opening_line(&Target::Juno), "Opening Juno settings.");
    }

    fn p(q: &str) -> Option<SettingsIntent> {
        normalize(q).and_then(|u| parse(&u))
    }

    fn c(q: &str) -> Option<Correction> {
        correction_from(q)
    }

    fn app_spec(name: &str) -> SettingsIntent {
        SettingsIntent::Explicit(Spec::App(name.into()))
    }

    fn front(name: &str, bundle: &str, pid: i32) -> Frontmost {
        Frontmost {
            name: name.into(),
            display: None,
            bundle_id: Some(bundle.into()),
            pid,
            regular: true,
        }
    }

    fn app_target(name: &str) -> Target {
        Target::App {
            name: name.into(),
            display: name.into(),
            path: None,
        }
    }

    fn win(number: &str, title: &str) -> WinRef {
        WinRef {
            number: number.into(),
            title: title.into(),
        }
    }

    #[test]
    fn bare_phrases() {
        for q in [
            "open settings",
            "Open preferences",
            "hey Juno, open the settings please",
            "pull up preferences",
            "open settings window",
        ] {
            assert_eq!(p(q), Some(SettingsIntent::Bare), "{q}");
        }
    }

    #[test]
    fn explicit_targets() {
        assert_eq!(
            p("open Juno settings"),
            Some(SettingsIntent::Explicit(Spec::Juno))
        );
        assert_eq!(
            p("open your settings"),
            Some(SettingsIntent::Explicit(Spec::Juno))
        );
        assert_eq!(
            p("open Mac settings"),
            Some(SettingsIntent::Explicit(Spec::Mac(None)))
        );
        assert_eq!(
            p("open system settings"),
            Some(SettingsIntent::Explicit(Spec::Mac(None)))
        );
        assert!(matches!(
            p("open Wi-Fi settings"),
            Some(SettingsIntent::Explicit(Spec::Mac(Some(pane)))) if pane.label == "Wi-Fi"
        ));
        assert_eq!(p("open Music settings"), Some(app_spec("music")));
        assert_eq!(p("show Safari preferences"), Some(app_spec("safari")));
    }

    #[test]
    fn near_misses_fall_through_to_the_agent() {
        for q in [
            "open settings and turn on dark mode",
            "open Music settings then play something",
            "how do I open settings",
            "what are my settings",
            "settings",
            "open",
            "change the settings",
            "open the settings for the new project and review them",
            "open a b c d e settings",
            "open privacy",
            "close settings",
        ] {
            assert_eq!(p(q), None, "{q} should reach the agent");
        }
    }

    #[test]
    fn bare_settings_resolution_order() {
        let own = 100;
        // 2: a regular app is where settings open.
        let music = front("Music", "com.apple.Music", 5);
        let kind = classify_front(Some(&music), own);
        assert_eq!(kind, FrontKind::App(music.clone()));
        assert_eq!(bare_target(&kind), app_target("Music"));
        // 3: Finder, nothing in front, a helper, or System Settings itself.
        let finder = front("Finder", "com.apple.finder", 6);
        assert_eq!(classify_front(Some(&finder), own), FrontKind::Mac);
        assert_eq!(classify_front(None, own), FrontKind::Mac);
        let mut helper = music.clone();
        helper.regular = false;
        assert_eq!(classify_front(Some(&helper), own), FrontKind::Mac);
        let mut unbundled = music.clone();
        unbundled.bundle_id = None;
        assert_eq!(classify_front(Some(&unbundled), own), FrontKind::Mac);
        let settings = front("System Settings", "com.apple.systempreferences", 7);
        assert_eq!(classify_front(Some(&settings), own), FrontKind::Mac);
        // 4: Juno's own window means Juno's settings, even with a Music-like bundle.
        let juno = front("Juno", "com.juno.app", own);
        assert_eq!(classify_front(Some(&juno), own), FrontKind::Juno);
        assert_eq!(bare_target(&FrontKind::Juno), Target::Juno);
        assert_eq!(bare_target(&FrontKind::Mac), Target::Mac(None));
    }

    #[test]
    fn alternatives_are_the_other_likely_targets() {
        let music = app_target("Music");
        assert_eq!(
            alternatives(&music, Some(&music)),
            vec![Target::Mac(None), Target::Juno]
        );
        assert_eq!(
            alternatives(&Target::Mac(None), Some(&music)),
            vec![music.clone(), Target::Juno]
        );
        assert_eq!(alternatives(&Target::Mac(None), None), vec![Target::Juno]);
        assert_eq!(
            alternatives(&Target::Juno, Some(&music)),
            vec![music.clone(), Target::Mac(None)]
        );
        assert_eq!(alternatives(&Target::Juno, None), vec![Target::Mac(None)]);
        let chips = chips_for(&Session {
            target: music.clone(),
            opened: Opened::default(),
            previous: Some(music),
            alternatives: vec![Target::Mac(None), Target::Juno],
            at: Instant::now(),
            generation: 1,
        });
        assert_eq!(chips[0].label, "Open Mac settings");
        assert_eq!(chips[1].label, "Open Juno settings");
        assert_eq!(chips[0].id, "alt-0");
    }

    #[test]
    fn close_corrections() {
        for q in [
            "no",
            "No.",
            "nope",
            "not that",
            "no, not that one",
            "wrong one",
            "that's wrong",
            "that's not it",
            "close it",
            "no close that",
            "undo",
        ] {
            assert_eq!(c(q), Some(Correction::Close), "{q}");
        }
    }

    #[test]
    fn switch_corrections() {
        let to = |s| Some(Correction::Switch(Switch::To(s)));
        assert_eq!(
            c("no, the other one"),
            Some(Correction::Switch(Switch::Other))
        );
        assert_eq!(
            c("no, open the other settings"),
            Some(Correction::Switch(Switch::Other))
        );
        assert_eq!(c("the other one"), Some(Correction::Switch(Switch::Other)));
        assert_eq!(c("Juno settings"), to(Spec::Juno));
        assert_eq!(c("no, Juno settings"), to(Spec::Juno));
        assert_eq!(c("no, open Mac settings"), to(Spec::Mac(None)));
        assert_eq!(c("Mac settings"), to(Spec::Mac(None)));
        assert_eq!(
            c("no, I meant Safari settings"),
            to(Spec::App("safari".into()))
        );
    }

    #[test]
    fn correction_near_misses_reach_the_agent() {
        for q in [
            "no and then open Safari",
            "no way",
            "not that one, the blue one",
            "close it and open Safari",
            "wrong one, try again with Mac settings and Juno settings",
            "no thanks I will do it later",
            "that is not it, check my email",
            "other",
            "settings",
            "the other one is better",
            "undo my last commit",
            "close the window",
            "close Safari",
        ] {
            assert_eq!(c(q), None, "{q} should not be a correction");
        }
    }

    #[test]
    fn corrections_expire_after_the_window() {
        let at = Instant::now();
        let session = Session {
            target: Target::Juno,
            opened: Opened::default(),
            previous: None,
            alternatives: vec![Target::Mac(None)],
            at,
            generation: 1,
        };
        assert!(session.is_live(at));
        assert!(session.is_live(at + CORRECTION_WINDOW - Duration::from_secs(1)));
        assert!(!session.is_live(at + CORRECTION_WINDOW));
        assert!(!session.is_live(at + CORRECTION_WINDOW + Duration::from_secs(30)));
    }

    #[test]
    fn corrections_need_a_live_session() {
        // Nothing has been opened in this test process, so no phrase fires.
        for q in [
            "no",
            "not that",
            "Mac settings",
            "the other one",
            "close it",
        ] {
            assert_eq!(correction_for(q), None, "{q}");
        }
    }

    #[test]
    fn native_frontmost_maps_like_system_events() {
        let f = frontmost_from_parts(
            Some("Ghostty".into()),
            Some("com.mitchellh.ghostty".into()),
            Some("ghostty".into()),
            512,
            0,
        )
        .expect("mapped");
        assert_eq!(f.name, "ghostty");
        assert_eq!(f.display.as_deref(), Some("Ghostty"));
        assert_eq!(f.bundle_id.as_deref(), Some("com.mitchellh.ghostty"));
        assert!(f.regular);
        assert_eq!(f.display_name(), "Ghostty");

        // Accessory apps are not regular; no executable falls back to the
        // localized name; nothing to call it is no app.
        let accessory =
            frontmost_from_parts(Some("Helper".into()), None, None, 9, 1).expect("mapped");
        assert_eq!(accessory.name, "Helper");
        assert!(!accessory.regular);
        assert_eq!(accessory.bundle_id, None);
        assert_eq!(
            frontmost_from_parts(None, None, Some(" ".into()), 1, 0),
            None
        );
    }

    #[test]
    fn frontmost_output_parses() {
        let f = parse_frontmost("Music\tcom.apple.Music\t512\tfalse\n");
        assert_eq!(f, Some(front("Music", "com.apple.Music", 512)));
        let helper = parse_frontmost("Helper\t\t9\ttrue");
        assert_eq!(helper.as_ref().map(|h| h.regular), Some(false));
        assert_eq!(helper.and_then(|h| h.bundle_id), None);
        assert_eq!(parse_frontmost(""), None);
        assert_eq!(parse_frontmost("Music\tcom.apple.Music\tnope\tfalse"), None);
    }

    const US: char = '\u{1f}';

    fn open_output(how: &str, before: &[(&str, &str)], after: &[(&str, &str)]) -> String {
        let rows = |list: &[(&str, &str)]| {
            list.iter()
                .map(|(n, t)| format!("{n}{US}{t}\n"))
                .collect::<String>()
        };
        format!("{how}\n=B\n{}=A\n{}", rows(before), rows(after))
    }

    #[test]
    fn open_results_list_the_windows_either_side() {
        let out = open_output(
            "menu",
            &[("10", "Music")],
            &[("10", "Music"), ("11", "General")],
        );
        let opened = parse_open_result(&out).expect("opened");
        assert_eq!(opened.before, vec![win("10", "Music")]);
        assert_eq!(opened.after, vec![win("10", "Music"), win("11", "General")]);
        assert_eq!(
            new_windows(&opened.before, &opened.after),
            vec![win("11", "General")]
        );
        // A pressed menu item is trusted even when nothing new appeared.
        let same = open_output("menu", &[("10", "Music")], &[("10", "Music")]);
        assert!(parse_open_result(&same).is_ok());
        // Cmd+, that changed nothing is not a success.
        let silent = open_output("keys", &[("10", "Music")], &[("10", "Music")]);
        assert_eq!(parse_open_result(&silent), Err(OpenError::NoItem));
        let loud = open_output(
            "keys",
            &[("10", "Music")],
            &[("10", "Music"), ("12", "Prefs")],
        );
        assert!(parse_open_result(&loud).is_ok());
        // No Settings item: System Settings is next.
        assert_eq!(parse_open_result("noitem"), Err(OpenError::NoItem));
        assert_eq!(parse_open_result("missing"), Err(OpenError::Failed));
        assert_eq!(parse_open_result("garbage"), Err(OpenError::Failed));
    }

    #[test]
    fn a_window_without_a_number_has_no_identity() {
        // Titles change (a terminal retitles itself), so a window with no
        // number is never listed, and so never counted as new.
        let out = open_output("menu", &[("", "zsh")], &[("", "vim"), ("", "zsh")]);
        let opened = parse_open_result(&out).expect("opened");
        assert!(opened.before.is_empty() && opened.after.is_empty());
        assert_eq!(
            close_plan(&app_target("ghostty"), &opened),
            CloseAction::Nothing
        );
    }

    fn opened(was_running: bool, before: &[WinRef], after: &[WinRef]) -> Opened {
        Opened {
            was_running,
            before: before.to_vec(),
            after: after.to_vec(),
        }
    }

    #[test]
    fn closing_juno_settings_closes_its_window() {
        assert_eq!(
            close_plan(&Target::Juno, &Opened::default()),
            CloseAction::JunoWindow
        );
    }

    #[test]
    fn system_settings_is_quit_only_when_juno_launched_it() {
        let launched = opened(false, &[], &[]);
        let already_open = opened(true, &[], &[]);
        for target in [Target::Mac(None), Target::Mac(PANES.first())] {
            assert_eq!(
                close_plan(&target, &launched),
                CloseAction::QuitSystemSettings
            );
            assert_eq!(close_plan(&target, &already_open), CloseAction::Nothing);
        }
    }

    #[test]
    fn another_apps_settings_close_only_a_window_the_action_created() {
        let target = app_target("Music");
        // The press opened a new window: close exactly it.
        let plan = close_plan(
            &target,
            &opened(
                true,
                &[win("1", "Music")],
                &[win("1", "Music"), win("2", "General")],
            ),
        );
        assert_eq!(plan, CloseAction::CloseWindows(vec![win("2", "General")]));
        // The press created nothing: nothing is closed.
        let plan = close_plan(
            &target,
            &opened(true, &[win("1", "Music")], &[win("1", "Music")]),
        );
        assert_eq!(plan, CloseAction::Nothing);
        // A window that was already there is never closed, even retitled.
        let plan = close_plan(
            &target,
            &opened(true, &[win("1", "Music")], &[win("1", "Settings")]),
        );
        assert_eq!(plan, CloseAction::Nothing);
    }

    /// The reported defect: tapping a chip in Ghostty tried to quit all of
    /// Ghostty. Ghostty's Settings opens its config file in TextEdit, so the
    /// press adds no Ghostty window; its terminal window was there before and
    /// must not be touched.
    #[test]
    fn ghostty_settings_press_closes_nothing() {
        let windows = [win("201", "~/repo/juno")];
        let plan = close_plan(&app_target("ghostty"), &opened(true, &windows, &windows));
        assert_eq!(plan, CloseAction::Nothing);
    }

    #[test]
    fn only_new_windows_are_ever_in_a_plan() {
        let before = [win("1", "a"), win("2", "b")];
        let after = [win("1", "a"), win("2", "b"), win("3", "c"), win("4", "d")];
        match close_plan(&app_target("X"), &opened(true, &before, &after)) {
            CloseAction::CloseWindows(list) => {
                assert_eq!(list, vec![win("3", "c"), win("4", "d")]);
                assert!(list.iter().all(|w| !before.contains(w)));
            }
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn nothing_quits_an_app_but_system_settings_juno_launched() {
        let source = include_str!("settings_follow.rs");
        let production = source.split("#[cfg(test)]").next().unwrap_or(source);
        // No synthesized Cmd+Q, no `quit` addressed to a named process, no
        // process kill. The one quit is the fixed System Settings script.
        assert!(!production.contains("keystroke \"q\""));
        assert!(!production.contains("kill"));
        assert_eq!(production.matches("to quit\"#").count(), 1);
        assert!(QUIT_SYSTEM_SETTINGS_SCRIPT.contains("com.apple.systempreferences"));
        // The close script closes by number, never "the first window".
        assert!(CLOSE_SCRIPT.contains("AXWindowNumber"));
        assert!(!CLOSE_SCRIPT.contains("quit"));
        assert!(!CLOSE_SCRIPT.contains("keystroke"));
        assert!(!CLOSE_SCRIPT.contains("window 1"));
    }

    #[test]
    fn apps_are_called_by_their_display_name() {
        let f = parse_frontmost("ghostty\tcom.mitchellh.ghostty\t512\tfalse\tGhostty\n")
            .expect("parsed");
        assert_eq!(f.name, "ghostty");
        assert_eq!(f.display_name(), "Ghostty");
        let kind = classify_front(Some(&f), 1);
        let target = bare_target(&kind);
        assert_eq!(target.label(), "Ghostty settings");
        assert_eq!(target.chip(), "Open Ghostty settings");
        // No display name: the process name is the fallback.
        let plain = parse_frontmost("Music\tcom.apple.Music\t5\tfalse\n").expect("parsed");
        assert_eq!(plain.display_name(), "Music");
        // A ".app" suffix is not part of the name.
        let suffixed = Frontmost {
            display: Some("Ghostty.app".into()),
            ..plain
        };
        assert_eq!(suffixed.display_name(), "Ghostty");
    }

    #[test]
    fn settings_beat_the_open_app_grammar() {
        use super::super::{parse_local_intent, LocalIntent};
        assert!(matches!(
            parse_local_intent("open settings"),
            Some(LocalIntent::Settings(SettingsIntent::Bare))
        ));
        assert!(matches!(
            parse_local_intent("open Music settings"),
            Some(LocalIntent::Settings(_))
        ));
        assert!(matches!(
            parse_local_intent("open Music"),
            Some(LocalIntent::App(_))
        ));
    }

    #[test]
    fn scripts_take_names_only_as_argv() {
        assert!(OPEN_SCRIPT.contains("item 1 of argv"));
        assert!(CLOSE_SCRIPT.contains("item 2 of argv"));
        assert!(!OPEN_SCRIPT.contains("application procName"));
    }
}

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
        /// Process and display name.
        name: String,
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
            Target::App { name, .. } => format!("{} settings", name),
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
    pub bundle_id: Option<String>,
    pub pid: i32,
    /// A regular Dock app, not a menu bar extra or a helper.
    pub regular: bool,
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
    /// Title of the window the Settings press opened, when one was seen.
    window: Option<String>,
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

/* --------------------------------- native --------------------------------- */

const FRONTMOST_SCRIPT: &str = r#"tell application "System Events"
	set p to first application process whose frontmost is true
	set b to ""
	try
		set b to bundle identifier of p
	end try
	return (name of p) & tab & b & tab & ((unix id of p) as text) & tab & ((background only of p) as text)
end tell"#;

/// Parse [`FRONTMOST_SCRIPT`]'s output. Pure.
pub fn parse_frontmost(out: &str) -> Option<Frontmost> {
    let mut parts = out.trim_end_matches(['\n', '\r']).split('\t');
    let name = parts.next()?.trim().to_string();
    let bundle = parts.next()?.trim().to_string();
    let pid = parts.next()?.trim().parse::<i32>().ok()?;
    let background = parts.next()?.trim() == "true";
    if name.is_empty() {
        return None;
    }
    Some(Frontmost {
        name,
        bundle_id: (!bundle.is_empty()).then_some(bundle),
        pid,
        regular: !background,
    })
}

async fn frontmost() -> Option<Frontmost> {
    let out = osascript(FRONTMOST_SCRIPT, Vec::new()).await.ok()?;
    parse_frontmost(&out)
}

/// Press the app's Settings or Preferences menu item, or send Cmd+, when the
/// menu cannot be read. Reports window counts and titles either side so the
/// caller can tell whether anything opened. Fields are tab separated.
const OPEN_SCRIPT: &str = r#"on run argv
	set procName to item 1 of argv
	tell application "System Events"
		if not (exists process procName) then return "missing"
		tell process procName
			set frontmost to true
			delay 0.15
			set beforeCount to count of windows
			set beforeTitle to ""
			try
				set beforeTitle to name of window 1
			end try
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
			set afterCount to count of windows
			set afterTitle to ""
			try
				set afterTitle to name of window 1
			end try
			return how & tab & (beforeCount as text) & tab & (afterCount as text) & tab & beforeTitle & tab & afterTitle
		end tell
	end tell
end run"#;

/// Close the window of `argv[0]` titled `argv[1]` (the first window when the
/// title is empty) with its close button. Never quits the app.
const CLOSE_SCRIPT: &str = r#"on run argv
	set procName to item 1 of argv
	set wantTitle to item 2 of argv
	tell application "System Events"
		if not (exists process procName) then return "gone"
		tell process procName
			repeat with w in windows
				set t to ""
				try
					set t to name of w
				end try
				if wantTitle is "" or t is wantTitle then
					try
						click (first button of w whose subrole is "AXCloseButton")
						return "closed"
					end try
				end if
			end repeat
		end tell
	end tell
	return "not_found"
end run"#;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OpenError {
    /// The app has no Settings item, or pressing it opened nothing.
    NoItem,
    Failed,
}

/// Read [`OPEN_SCRIPT`]'s output: the title of the window that opened.
pub fn parse_open_result(out: &str) -> Result<Option<String>, OpenError> {
    let out = out.trim_end_matches(['\n', '\r']);
    if out == "noitem" {
        return Err(OpenError::NoItem);
    }
    let mut parts = out.splitn(5, '\t');
    let how = parts.next().ok_or(OpenError::Failed)?;
    let before: i64 = parts
        .next()
        .and_then(|s| s.trim().parse().ok())
        .ok_or(OpenError::Failed)?;
    let after: i64 = parts
        .next()
        .and_then(|s| s.trim().parse().ok())
        .ok_or(OpenError::Failed)?;
    let before_title = parts.next().unwrap_or("");
    let after_title = parts.next().unwrap_or("");
    let changed = after > before || before_title != after_title;
    match how {
        // A pressed menu item is trusted even when the window was already up.
        "menu" => Ok((!after_title.is_empty()).then(|| after_title.to_string())),
        // A keystroke proves nothing by itself: only a visible change counts.
        "keys" if changed => Ok((!after_title.is_empty()).then(|| after_title.to_string())),
        "keys" => Err(OpenError::NoItem),
        _ => Err(OpenError::Failed),
    }
}

async fn open_target(app: &AppHandle, target: &Target) -> Result<Option<String>, OpenError> {
    match target {
        Target::Juno => crate::window_management::open_settings_window(app.clone())
            .await
            .map(|_| None)
            .map_err(|e| {
                log::warn!("settings_follow: Juno settings failed: {}", e);
                OpenError::Failed
            }),
        Target::Mac(pane) => {
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
            result.map(|_| None).map_err(|e| {
                log::warn!("settings_follow: System Settings failed: {}", e);
                OpenError::Failed
            })
        }
        Target::App { name, path } => {
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
async fn open_with_fallback(
    app: &AppHandle,
    target: Target,
) -> Result<(Target, Option<String>), Target> {
    match open_target(app, &target).await {
        Ok(window) => Ok((target, window)),
        Err(OpenError::NoItem) => {
            let mac = Target::Mac(None);
            match open_target(app, &mac).await {
                Ok(window) => Ok((mac, window)),
                Err(_) => Err(mac),
            }
        }
        Err(OpenError::Failed) => Err(target),
    }
}

async fn close_target(app: &AppHandle, target: &Target, window: Option<&str>) {
    match target {
        Target::Juno => {
            if let Err(e) = crate::window_management::close_settings_window(app.clone()).await {
                log::warn!("settings_follow: closing Juno settings failed: {}", e);
            }
        }
        Target::Mac(_) | Target::App { .. } => {
            let process = match target {
                Target::App { name, .. } => name.clone(),
                _ => "System Settings".to_string(),
            };
            let title = window.unwrap_or("").to_string();
            if let Err(e) = osascript(CLOSE_SCRIPT, vec![process, title]).await {
                log::warn!("settings_follow: closing a settings window failed: {}", e);
            }
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
                name: app.name,
                path: Some(app.path),
            })
        }
    }
}

fn own_pid() -> i32 {
    i32::try_from(std::process::id()).unwrap_or(-1)
}

async fn open_and_remember(app: &AppHandle, wanted: Target, previous: Option<Target>) -> Reply {
    match open_with_fallback(app, wanted).await {
        Ok((target, window)) => {
            let alternatives = alternatives(&target, previous.as_ref());
            remember(Session {
                target: target.clone(),
                window,
                previous,
                alternatives,
                at: Instant::now(),
                generation: 0,
            });
            Reply::text(format!("Opened {}.", target.label()))
        }
        Err(target) => Reply::failure(format!("I couldn't open {}.", target.label())),
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
        close_target(app, &session.target, session.window.as_deref()).await;
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
    close_target(&app, &session.target, session.window.as_deref()).await;
    let reply = open_and_remember(&app, target, session.previous).await;
    super::emit_reply(&app, reply).await;
    publish(&app);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::super::utterance::normalize;
    use super::*;

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
            bundle_id: Some(bundle.into()),
            pid,
            regular: true,
        }
    }

    fn app_target(name: &str) -> Target {
        Target::App {
            name: name.into(),
            path: None,
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
            window: None,
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
            window: None,
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
    fn frontmost_output_parses() {
        let f = parse_frontmost("Music\tcom.apple.Music\t512\tfalse\n");
        assert_eq!(f, Some(front("Music", "com.apple.Music", 512)));
        let helper = parse_frontmost("Helper\t\t9\ttrue");
        assert_eq!(helper.as_ref().map(|h| h.regular), Some(false));
        assert_eq!(helper.and_then(|h| h.bundle_id), None);
        assert_eq!(parse_frontmost(""), None);
        assert_eq!(parse_frontmost("Music\tcom.apple.Music\tnope\tfalse"), None);
    }

    #[test]
    fn open_results_decide_whether_a_window_opened() {
        // A pressed menu item opened something; its title is remembered.
        assert_eq!(
            parse_open_result("menu\t1\t2\tMusic\tGeneral"),
            Ok(Some("General".into()))
        );
        // Cmd+, that changed nothing is not a success.
        assert_eq!(
            parse_open_result("keys\t1\t1\tMusic\tMusic"),
            Err(OpenError::NoItem)
        );
        assert_eq!(
            parse_open_result("keys\t1\t2\tMusic\tGeneral"),
            Ok(Some("General".into()))
        );
        // No Settings item: System Settings is next.
        assert_eq!(parse_open_result("noitem"), Err(OpenError::NoItem));
        assert_eq!(parse_open_result("missing"), Err(OpenError::Failed));
        assert_eq!(parse_open_result("garbage"), Err(OpenError::Failed));
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

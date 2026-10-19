//! The permission preflight: look before a Mac app tool runs, ask in words
//! before any system dialog, and never surprise the person.
//!
//! Decided by Lacy 2026-10-10 (LAC-4234, `docs/plans/siri-parity.md`
//! "Permissions, progressively, right before the act"). Before a tool runs,
//! every permission it needs is read silently. Three outcomes:
//!
//! 1. **Granted.** [`Preflight::Clear`]. The tool runs. Nothing is said.
//! 2. **Not yet asked.** [`Preflight::Ask`]. Juno says one line ("I need
//!    Reminders for that.") and shows one button, Continue. The system dialog
//!    is brought up only when the person taps Continue or says "continue"
//!    ([`classify_reply`]); then [`continue_after_ask`] requests each
//!    permission in order and checks again.
//! 3. **Declined, restricted, or no system dialog exists** (Full Disk Access,
//!    Automation once denied). [`Preflight::Blocked`] carries the existing
//!    denied answer: one sentence and the one button that opens the right
//!    Privacy pane.
//!
//! One table ([`needs`]) says what each tool needs, so the API path
//! (`agent_runner`) and the MCP path (`juno_mcp`) cannot drift apart. The
//! status source is a trait so the whole decision is tested without a Mac.

use serde_json::{json, Value};

use crate::constants::agent::tool_names;
use crate::state::{RiskLevel, ToolApprovalRequest};

use super::script::App;
use super::{chat_db, full_disk_access_result, Access, Domain};

/// How long a Continue waits for an answer, in seconds.
pub const ASK_TIMEOUT_SECS: u64 = 120;

/// One thing macOS can say no to, as a person names it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Permission {
    Reminders,
    Calendars,
    Contacts,
    /// Controlling one app (Messages, Mail, Notes, Music).
    Automation(App),
    /// Reading the Messages history. macOS has no dialog for this one.
    FullDiskAccess,
}

impl Permission {
    /// What the person calls it.
    pub fn label(self) -> &'static str {
        match self {
            Permission::Reminders => "Reminders",
            Permission::Calendars => "Calendar",
            Permission::Contacts => "Contacts",
            Permission::Automation(app) => app.label(),
            Permission::FullDiskAccess => "Full Disk Access",
        }
    }

    /// The words that follow "I need": "Reminders", "to control Messages".
    fn need_phrase(self) -> String {
        match self {
            Permission::Automation(app) => format!("to control {}", app.label()),
            other => other.label().to_string(),
        }
    }

    /// The no: one sentence and one button, the answer each tool already gave.
    pub fn denied_result(self) -> Value {
        match self {
            Permission::Reminders => super::denied_result(Domain::Reminders),
            Permission::Calendars => super::denied_result(Domain::Calendar),
            Permission::Contacts => super::denied_result(Domain::Contacts),
            Permission::Automation(app) => super::script::not_permitted_result(app),
            Permission::FullDiskAccess => full_disk_access_result(),
        }
    }

    /// A policy on this Mac says no. Nothing the person can switch, so no button.
    pub fn restricted_result(self) -> Value {
        json!({
            "ok": false,
            "access": "restricted",
            "summary": format!("This Mac does not let Juno use {}.", self.label()),
        })
    }

    /// The dialog was shown and not answered.
    fn unanswered_result(self) -> Value {
        json!({
            "ok": false,
            "access": "asking",
            "summary": format!(
                "I did not get an answer about {}, so I did not do that. Ask me again when you are ready.",
                self.label()
            ),
        })
    }

    /// What macOS says right now. Silent: never shows a dialog.
    #[cfg(target_os = "macos")]
    pub fn status(self) -> Access {
        match self {
            Permission::Reminders => super::eventkit::status(Domain::Reminders),
            Permission::Calendars => super::eventkit::status(Domain::Calendar),
            Permission::Contacts => super::contacts::status(),
            Permission::Automation(app) => automation::status(app.bundle_id()),
            Permission::FullDiskAccess => full_disk_status(),
        }
    }

    #[cfg(not(target_os = "macos"))]
    pub fn status(self) -> Access {
        Access::Granted
    }

    /// Bring up the real system dialog where one exists and wait for the
    /// answer. `None` when it was not answered in time. Full Disk Access has
    /// no dialog, so this only reports where it stands.
    #[cfg(target_os = "macos")]
    pub fn request(self) -> Option<Access> {
        match self {
            Permission::Reminders => super::eventkit::request(Domain::Reminders),
            Permission::Calendars => super::eventkit::request(Domain::Calendar),
            Permission::Contacts => super::contacts::request(),
            Permission::Automation(app) => automation::request(app.bundle_id()),
            Permission::FullDiskAccess => Some(full_disk_status()),
        }
    }

    #[cfg(not(target_os = "macos"))]
    pub fn request(self) -> Option<Access> {
        Some(Access::Granted)
    }
}

/// Full Disk Access by the one honest test: macOS refuses the open of the
/// Messages history when it is off. No history at all is not a refusal.
fn full_disk_status() -> Access {
    match chat_db::access() {
        chat_db::Access::Readable(_) | chat_db::Access::Missing => Access::Granted,
        chat_db::Access::NoAccess => Access::Denied,
    }
}

/// Can this running process read the Messages history now? The Full Disk
/// Access card re-asks this when Juno is reactivated after the person visited
/// System Settings. macOS applies the switch to a process only when it starts,
/// so "still no" after they added Juno means a restart is the missing step.
pub fn full_disk_access_granted() -> bool {
    full_disk_status() == Access::Granted
}

// ---------------------------------------------------------------------------
// Automation, read silently
// ---------------------------------------------------------------------------

/// What `AEDeterminePermissionToAutomateTarget` said, as an access state.
/// `None` means the target app is not running, so macOS cannot answer yet.
pub fn access_from_automation(code: i32) -> Option<Access> {
    match code {
        0 => Some(Access::Granted),
        // errAEEventWouldRequireUserConsent: never asked.
        -1744 => Some(Access::NotDetermined),
        // errAEEventNotPermitted: the person said no.
        -1743 => Some(Access::Denied),
        // procNotFound: the app is not running.
        -600 => None,
        _ => Some(Access::Denied),
    }
}

#[cfg(target_os = "macos")]
mod automation {
    //! `AEDeterminePermissionToAutomateTarget` through plain C, so the check is
    //! made by Juno's own process (the dialog names Juno) and costs one call.

    use std::ffi::c_void;
    use std::time::Duration;

    use super::{access_from_automation, Access};

    /// 'bund': an Apple event target named by bundle id.
    const TYPE_APPLICATION_BUNDLE_ID: u32 = u32::from_be_bytes(*b"bund");
    /// '****': any event class, any event id.
    const TYPE_WILD_CARD: u32 = u32::from_be_bytes(*b"****");
    /// A code `access_from_automation` reads as a no, for a call that could
    /// not be made at all.
    const COULD_NOT_ASK: i32 = -1;
    /// How long a just-launched app gets to come up before the answer is
    /// called "not asked yet".
    const LAUNCH_POLLS: u32 = 10;
    const LAUNCH_POLL_MS: u64 = 300;

    #[repr(C)]
    struct AeDesc {
        descriptor_type: u32,
        data_handle: *mut c_void,
    }

    #[link(name = "CoreServices", kind = "framework")]
    extern "C" {
        fn AECreateDesc(
            type_code: u32,
            data_ptr: *const c_void,
            data_size: isize,
            result: *mut AeDesc,
        ) -> i16;
        fn AEDisposeDesc(desc: *mut AeDesc) -> i16;
        fn AEDeterminePermissionToAutomateTarget(
            target: *const AeDesc,
            event_class: u32,
            event_id: u32,
            ask_user_if_needed: u8,
        ) -> i32;
    }

    /// The raw status for one bundle id. `ask` is whether macOS may show its
    /// dialog; it blocks until the person answers when it does.
    fn determine(bundle: &str, ask: bool) -> i32 {
        let bytes = bundle.as_bytes();
        let Ok(size) = isize::try_from(bytes.len()) else {
            return COULD_NOT_ASK;
        };
        let mut desc = AeDesc {
            descriptor_type: 0,
            data_handle: std::ptr::null_mut(),
        };
        // SAFETY: `bytes` is valid for `size` bytes for the call, `desc` is a
        // zeroed descriptor the call fills in, and it is disposed below.
        let created = unsafe {
            AECreateDesc(
                TYPE_APPLICATION_BUNDLE_ID,
                bytes.as_ptr().cast::<c_void>(),
                size,
                &mut desc,
            )
        };
        if created != 0 {
            return COULD_NOT_ASK;
        }
        // SAFETY: `desc` was created above and is still live.
        let status = unsafe {
            AEDeterminePermissionToAutomateTarget(
                &desc,
                TYPE_WILD_CARD,
                TYPE_WILD_CARD,
                u8::from(ask),
            )
        };
        // SAFETY: disposes the descriptor created above, once.
        unsafe { AEDisposeDesc(&mut desc) };
        status
    }

    /// Start the app hidden and in the background so macOS has a process to
    /// answer about. It does not take focus.
    fn launch_quietly(bundle: &str) {
        let launched = std::process::Command::new("/usr/bin/open")
            .args(["-g", "-j", "-b", bundle])
            .status();
        if let Err(e) = launched {
            tracing::warn!("Could not start {bundle} in the background: {e}");
        }
    }

    /// Silent: never shows a dialog.
    pub fn status(bundle: &str) -> Access {
        if let Some(access) = access_from_automation(determine(bundle, false)) {
            return access;
        }
        launch_quietly(bundle);
        for _ in 0..LAUNCH_POLLS {
            std::thread::sleep(Duration::from_millis(LAUNCH_POLL_MS));
            if let Some(access) = access_from_automation(determine(bundle, false)) {
                return access;
            }
        }
        Access::NotDetermined
    }

    /// Show the system dialog and wait for the answer.
    pub fn request(bundle: &str) -> Option<Access> {
        // Makes sure the app is up, so the ask has a process to name.
        let _ = status(bundle);
        access_from_automation(determine(bundle, true))
    }
}

// ---------------------------------------------------------------------------
// What each tool needs: the one table
// ---------------------------------------------------------------------------

use Permission::{Automation, Calendars, Contacts, FullDiskAccess, Reminders};

const NONE: &[Permission] = &[];

/// Every Mac app tool and the permissions it needs, in the order they are
/// asked. A test walks `tool_definitions()` against this, so a new tool
/// cannot ship without saying what it needs.
const NEEDS: &[(&str, &[Permission])] = &[
    (tool_names::REMINDERS_LIST, &[Reminders]),
    (tool_names::REMINDERS_CREATE, &[Reminders]),
    (tool_names::REMINDERS_COMPLETE, &[Reminders]),
    (tool_names::CALENDAR_EVENTS, &[Calendars]),
    (tool_names::CALENDAR_CREATE_EVENT, &[Calendars]),
    (tool_names::CALENDAR_MOVE_EVENT, &[Calendars]),
    (tool_names::CALENDAR_DELETE_EVENT, &[Calendars]),
    (tool_names::CONTACTS_FIND, &[Contacts]),
    (
        tool_names::MESSAGES_SEND,
        &[Contacts, Automation(App::Messages)],
    ),
    (tool_names::MESSAGES_RECENT, &[Contacts, FullDiskAccess]),
    // A call opens a facetime:// link; it resolves the person, nothing more.
    (tool_names::FACETIME_CALL, &[Contacts]),
    (tool_names::MAIL_UNREAD, &[Automation(App::Mail)]),
    (tool_names::MAIL_SEARCH, &[Automation(App::Mail)]),
    (tool_names::MAIL_SEND, &[Contacts, Automation(App::Mail)]),
    (tool_names::MAIL_DRAFT, &[Automation(App::Mail)]),
    (tool_names::NOTES_CREATE, &[Automation(App::Notes)]),
    (tool_names::NOTES_APPEND, &[Automation(App::Notes)]),
    (tool_names::NOTES_SEARCH, &[Automation(App::Notes)]),
    (tool_names::MUSIC_PLAY, &[Automation(App::Music)]),
    (tool_names::MAPS_DIRECTIONS, NONE),
    (tool_names::SHORTCUTS_LIST, NONE),
    (tool_names::SHORTCUTS_RUN, NONE),
    (tool_names::FOCUS_SET, NONE),
];

/// Whether the table has a row for this tool, even an empty one.
#[cfg(test)]
fn has_entry(name: &str) -> bool {
    NEEDS.iter().any(|(tool, _)| *tool == name)
}

/// The permissions a tool needs. Empty for a tool that needs none and for a
/// name that is not a Mac app tool.
pub fn needs(name: &str, _input: &Value) -> Vec<Permission> {
    NEEDS
        .iter()
        .find(|(tool, _)| *tool == name)
        .map(|(_, perms)| perms.to_vec())
        .unwrap_or_default()
}

// ---------------------------------------------------------------------------
// The decision
// ---------------------------------------------------------------------------

/// What to do before the tool runs.
#[derive(Debug, Clone, PartialEq)]
pub enum Preflight {
    /// Everything is already allowed. Run, say nothing.
    Clear,
    /// These have never been asked. Say so, show Continue, wait.
    Ask(Vec<Permission>),
    /// Stop with this answer. Nothing runs.
    Blocked(Value),
}

/// Where statuses come from and where dialogs are brought up. The real one is
/// [`System`]; a test supplies its own and counts the calls.
pub trait Source {
    fn status(&self, permission: Permission) -> Access;
    fn request(&self, permission: Permission) -> Option<Access>;
}

/// The Mac itself.
pub struct System;

impl Source for System {
    fn status(&self, permission: Permission) -> Access {
        permission.status()
    }

    fn request(&self, permission: Permission) -> Option<Access> {
        permission.request()
    }
}

/// Read every permission the tool needs, silently. A decline anywhere wins
/// over an ask, so nobody is asked for one thing and then told no about
/// another. Pure given the source.
pub fn preflight_with(source: &impl Source, name: &str, input: &Value) -> Preflight {
    let mut ask = Vec::new();
    for permission in needs(name, input) {
        match source.status(permission) {
            Access::Granted => {}
            Access::NotDetermined => ask.push(permission),
            Access::Denied => return Preflight::Blocked(permission.denied_result()),
            Access::Restricted => return Preflight::Blocked(permission.restricted_result()),
        }
    }
    if ask.is_empty() {
        Preflight::Clear
    } else {
        Preflight::Ask(ask)
    }
}

/// After "continue": bring each dialog up in order, then check again. The
/// answer is `Clear` or `Blocked`, never `Ask`.
pub fn continue_with(
    source: &impl Source,
    name: &str,
    input: &Value,
    asked: &[Permission],
) -> Preflight {
    for permission in asked {
        match source.request(*permission) {
            Some(Access::Granted) => {}
            Some(Access::Restricted) => return Preflight::Blocked(permission.restricted_result()),
            Some(_) => return Preflight::Blocked(permission.denied_result()),
            None => return Preflight::Blocked(permission.unanswered_result()),
        }
    }
    match preflight_with(source, name, input) {
        Preflight::Ask(still) => match still.first() {
            Some(permission) => Preflight::Blocked(permission.unanswered_result()),
            None => Preflight::Clear,
        },
        settled => settled,
    }
}

/// What the model is told when the person did not continue.
pub const NOT_CONTINUED: &str = "The person did not continue, so Juno did not do this. \
Tell them it was not done; they can ask again if they change their mind.";
/// What the model is told when nobody answered the ask.
pub const NOT_ANSWERED: &str = "No answer arrived, so Juno did not do this. \
Tell the person it was not done; they can ask again.";
/// What the model is told when a permission is still missing after the ask.
pub const NOT_ALLOWED_YET: &str = "Juno does not have what it needs for this yet, so it did not \
do it. Tell the person it was not done; they can ask again.";

fn check_failed() -> Preflight {
    Preflight::Blocked(json!({
        "ok": false,
        "summary": "I could not check that just now. Ask me again in a moment.",
    }))
}

/// The preflight both paths call. Tools that need nothing skip the thread hop.
pub async fn preflight(name: &str, input: &Value) -> Preflight {
    if needs(name, input).is_empty() {
        return Preflight::Clear;
    }
    let (name, input) = (name.to_string(), input.clone());
    tauri::async_runtime::spawn_blocking(move || preflight_with(&System, &name, &input))
        .await
        .unwrap_or_else(|e| {
            tracing::warn!("The permission check did not finish: {e}");
            check_failed()
        })
}

/// After the person said "continue": the dialogs, then the check again.
pub async fn continue_after_ask(name: &str, input: &Value, asked: Vec<Permission>) -> Preflight {
    let (name, input) = (name.to_string(), input.clone());
    tauri::async_runtime::spawn_blocking(move || continue_with(&System, &name, &input, &asked))
        .await
        .unwrap_or_else(|e| {
            tracing::warn!("The permission request did not finish: {e}");
            check_failed()
        })
}

// ---------------------------------------------------------------------------
// The ask: one line, one action
// ---------------------------------------------------------------------------

/// "I need Reminders for that." / "I need Contacts and to control Messages for that."
pub fn ask_line(permissions: &[Permission]) -> String {
    let phrases: Vec<String> = permissions.iter().map(|p| p.need_phrase()).collect();
    let joined = match phrases.as_slice() {
        [] => "access".to_string(),
        [one] => one.clone(),
        [init @ .., last] => format!("{} and {last}", init.join(", ")),
    };
    format!("I need {joined} for that.")
}

/// The pending ask, held the way a pending send is: a tool approval request
/// whose payload is the card's line and the spoken prompt. The Continue button
/// and the spoken "continue" both resolve it.
pub fn ask_request(name: &str, input: &Value, permissions: &[Permission]) -> ToolApprovalRequest {
    let line = ask_line(permissions);
    let labels: Vec<&str> = permissions.iter().map(|p| p.label()).collect();
    let payload = json!({
        "line": line,
        "prompt": format!("{line} Say continue."),
        "permissions": labels,
    });
    ToolApprovalRequest::new(
        uuid::Uuid::new_v4().to_string(),
        name.to_string(),
        input.clone(),
        line,
    )
    .with_risk(RiskLevel::Low)
    .with_timeout(ASK_TIMEOUT_SECS)
    .with_permission(payload)
}

/// What the person's reply did to a waiting ask.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ContinueReply {
    /// "continue": bring the dialog up.
    Continue,
    /// A clear no: drop the ask.
    Cancel,
    /// Anything else. Spoken on purpose, it is a new request and the ask is
    /// dropped; overheard, it is ignored.
    Other,
}

/// The whole utterance has to be one of these. "continue later" and "yes but
/// not yet" are not "continue". A bare yes is fine here: nothing is sent.
const CONTINUE_PHRASES: &[&str] = &[
    "continue",
    "please continue",
    "go ahead",
    "ok",
    "okay",
    "ok go ahead",
    "okay go ahead",
    "yes",
    "yes please",
    "yes continue",
    "yes go ahead",
    "yeah",
    "yep",
];

fn normalize(text: &str) -> String {
    let spaced: String = text
        .to_lowercase()
        .chars()
        .map(|c| {
            if c.is_alphanumeric() || c == '\'' {
                c
            } else {
                ' '
            }
        })
        .collect();
    spaced.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// Whether the whole utterance means "continue".
pub fn is_continue_phrase(text: &str) -> bool {
    if text.contains('?') {
        return false;
    }
    let said = normalize(text);
    CONTINUE_PHRASES.iter().any(|phrase| *phrase == said)
}

/// Read a reply to a waiting ask. `parse` is the general yes/no parser, used
/// only to recognise a clear no.
pub fn classify_reply(text: &str, parse: impl Fn(&str) -> Option<bool>) -> ContinueReply {
    if is_continue_phrase(text) {
        ContinueReply::Continue
    } else if parse(text) == Some(false) {
        ContinueReply::Cancel
    } else {
        ContinueReply::Other
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agent::tools::mac_apps::tool_definitions;
    use std::cell::RefCell;

    /// A Mac that answers from a script and remembers every dialog it was
    /// asked to show.
    struct Fake {
        statuses: RefCell<Vec<(Permission, Access)>>,
        answers: Vec<(Permission, Option<Access>)>,
        dialogs: RefCell<Vec<Permission>>,
    }

    impl Fake {
        fn new(statuses: &[(Permission, Access)]) -> Self {
            Self {
                statuses: RefCell::new(statuses.to_vec()),
                answers: Vec::new(),
                dialogs: RefCell::new(Vec::new()),
            }
        }

        fn answering(mut self, answers: &[(Permission, Option<Access>)]) -> Self {
            self.answers = answers.to_vec();
            self
        }
    }

    impl Source for Fake {
        fn status(&self, permission: Permission) -> Access {
            self.statuses
                .borrow()
                .iter()
                .find(|(p, _)| *p == permission)
                .map(|(_, a)| *a)
                .unwrap_or(Access::Granted)
        }

        fn request(&self, permission: Permission) -> Option<Access> {
            self.dialogs.borrow_mut().push(permission);
            let answer = self
                .answers
                .iter()
                .find(|(p, _)| *p == permission)
                .and_then(|(_, a)| *a);
            if let Some(access) = answer {
                for entry in self.statuses.borrow_mut().iter_mut() {
                    if entry.0 == permission {
                        entry.1 = access;
                    }
                }
            }
            answer
        }
    }

    fn none() -> Value {
        json!({})
    }

    #[test]
    fn every_tool_says_what_it_needs() {
        for definition in tool_definitions() {
            assert!(
                has_entry(&definition.name),
                "{} has no row in the permission table",
                definition.name
            );
        }
        let names: Vec<String> = tool_definitions().into_iter().map(|d| d.name).collect();
        for (tool, _) in NEEDS {
            assert!(
                names.iter().any(|n| n == tool),
                "{tool} is in the permission table but is not a tool"
            );
        }
        let mut seen: Vec<&str> = NEEDS.iter().map(|(t, _)| *t).collect();
        seen.sort_unstable();
        seen.dedup();
        assert_eq!(seen.len(), NEEDS.len(), "a tool is in the table twice");
    }

    #[test]
    fn the_table_says_what_the_decision_listed() {
        let want = |name: &str| needs(name, &none());
        assert_eq!(want("reminders_create"), [Reminders]);
        assert_eq!(want("calendar_events"), [Calendars]);
        assert_eq!(want("contacts_find"), [Contacts]);
        assert_eq!(want("messages_send"), [Contacts, Automation(App::Messages)]);
        assert_eq!(want("messages_recent"), [Contacts, FullDiskAccess]);
        assert_eq!(want("mail_send"), [Contacts, Automation(App::Mail)]);
        assert_eq!(want("mail_unread"), [Automation(App::Mail)]);
        assert_eq!(want("notes_create"), [Automation(App::Notes)]);
        assert_eq!(want("music_play"), [Automation(App::Music)]);
        for free in [
            "shortcuts_run",
            "shortcuts_list",
            "focus_set",
            "maps_directions",
        ] {
            assert!(want(free).is_empty(), "{free}");
        }
        assert!(want("not_a_tool").is_empty());
    }

    #[test]
    fn granted_runs_and_says_nothing() {
        let mac = Fake::new(&[]);
        for name in ["reminders_create", "messages_send", "messages_recent"] {
            assert_eq!(preflight_with(&mac, name, &none()), Preflight::Clear);
        }
        assert!(mac.dialogs.borrow().is_empty());
    }

    #[test]
    fn never_asked_returns_an_ask_and_shows_no_dialog() {
        let mac = Fake::new(&[(Reminders, Access::NotDetermined)]);
        assert_eq!(
            preflight_with(&mac, "reminders_create", &none()),
            Preflight::Ask(vec![Reminders])
        );
        assert!(
            mac.dialogs.borrow().is_empty(),
            "a status check showed a dialog"
        );

        // Only what is actually missing is asked about.
        let mac = Fake::new(&[
            (Contacts, Access::Granted),
            (Automation(App::Messages), Access::NotDetermined),
        ]);
        assert_eq!(
            preflight_with(&mac, "messages_send", &none()),
            Preflight::Ask(vec![Automation(App::Messages)])
        );
        let mac = Fake::new(&[
            (Contacts, Access::NotDetermined),
            (Automation(App::Messages), Access::NotDetermined),
        ]);
        assert_eq!(
            preflight_with(&mac, "messages_send", &none()),
            Preflight::Ask(vec![Contacts, Automation(App::Messages)])
        );
        assert!(mac.dialogs.borrow().is_empty());
    }

    #[test]
    fn declined_is_blocked_with_the_settings_action() {
        let cases = [
            (Reminders, "reminders_list", "Privacy_Reminders"),
            (Calendars, "calendar_events", "Privacy_Calendars"),
            (Contacts, "contacts_find", "Privacy_Contacts"),
            (Automation(App::Notes), "notes_search", "Privacy_Automation"),
            (FullDiskAccess, "messages_recent", "Privacy_AllFiles"),
        ];
        for (permission, tool, pane) in cases {
            let mac = Fake::new(&[(permission, Access::Denied)]);
            let Preflight::Blocked(answer) = preflight_with(&mac, tool, &none()) else {
                panic!("{tool} should be blocked");
            };
            assert_eq!(answer["ok"], false, "{tool}");
            let card = answer["card"].as_str().unwrap_or_default();
            assert!(card.contains(pane), "{tool}: {card}");
            assert!(mac.dialogs.borrow().is_empty(), "{tool}");
        }
    }

    #[test]
    fn a_no_wins_over_an_ask_and_restricted_has_no_button() {
        let mac = Fake::new(&[
            (Contacts, Access::NotDetermined),
            (FullDiskAccess, Access::Denied),
        ]);
        assert!(matches!(
            preflight_with(&mac, "messages_recent", &none()),
            Preflight::Blocked(_)
        ));
        let mac = Fake::new(&[(Reminders, Access::Restricted)]);
        let Preflight::Blocked(answer) = preflight_with(&mac, "reminders_list", &none()) else {
            panic!("restricted should block");
        };
        assert_eq!(answer["access"], "restricted");
        assert!(answer.get("card").is_none());
    }

    #[test]
    fn continue_asks_in_order_then_clears() {
        let mac = Fake::new(&[
            (Contacts, Access::NotDetermined),
            (Automation(App::Messages), Access::NotDetermined),
        ])
        .answering(&[
            (Contacts, Some(Access::Granted)),
            (Automation(App::Messages), Some(Access::Granted)),
        ]);
        let asked = [Contacts, Automation(App::Messages)];
        assert_eq!(
            continue_with(&mac, "messages_send", &none(), &asked),
            Preflight::Clear
        );
        assert_eq!(*mac.dialogs.borrow(), asked);
    }

    #[test]
    fn a_dialog_that_ends_denied_stops_and_skips_the_rest() {
        let mac = Fake::new(&[
            (Contacts, Access::NotDetermined),
            (Automation(App::Messages), Access::NotDetermined),
        ])
        .answering(&[(Contacts, Some(Access::Denied))]);
        let asked = [Contacts, Automation(App::Messages)];
        let Preflight::Blocked(answer) = continue_with(&mac, "messages_send", &none(), &asked)
        else {
            panic!("a denied dialog should block");
        };
        assert_eq!(answer["access"], "denied");
        assert_eq!(*mac.dialogs.borrow(), [Contacts]);
    }

    #[test]
    fn an_unanswered_dialog_does_not_run_the_tool() {
        let mac = Fake::new(&[(Reminders, Access::NotDetermined)]).answering(&[(Reminders, None)]);
        let Preflight::Blocked(answer) =
            continue_with(&mac, "reminders_create", &none(), &[Reminders])
        else {
            panic!("no answer should block");
        };
        assert_eq!(answer["access"], "asking");
    }

    #[test]
    fn automation_codes_map_to_one_state_each() {
        assert_eq!(access_from_automation(0), Some(Access::Granted));
        assert_eq!(access_from_automation(-1744), Some(Access::NotDetermined));
        assert_eq!(access_from_automation(-1743), Some(Access::Denied));
        assert_eq!(access_from_automation(-600), None);
        assert_eq!(access_from_automation(-1), Some(Access::Denied));
    }

    #[test]
    fn the_ask_is_one_line_in_plain_words() {
        assert_eq!(ask_line(&[Reminders]), "I need Reminders for that.");
        assert_eq!(
            ask_line(&[Automation(App::Messages)]),
            "I need to control Messages for that."
        );
        assert_eq!(
            ask_line(&[Contacts, Automation(App::Messages)]),
            "I need Contacts and to control Messages for that."
        );
        let request = ask_request("reminders_create", &none(), &[Reminders]);
        assert!(request.is_permission());
        assert!(!request.is_send());
        assert_eq!(request.description, "I need Reminders for that.");
        let payload = request.permission.unwrap_or_default();
        assert_eq!(payload["line"], "I need Reminders for that.");
        assert_eq!(
            payload["prompt"],
            "I need Reminders for that. Say continue."
        );
        for banned in ["TCC", "EventKit", "plist", "framework", "Apple event"] {
            assert!(!payload["line"]
                .as_str()
                .unwrap_or_default()
                .contains(banned));
        }
    }

    #[test]
    fn a_messages_send_ask_is_not_mistaken_for_a_send() {
        let request = ask_request(
            "messages_send",
            &json!({"to": "Katie", "body": "hi"}),
            &[Contacts],
        );
        assert!(request.is_permission());
        assert!(!request.is_send(), "Continue must not be answered as Send");
    }

    #[test]
    fn only_the_whole_utterance_continues() {
        for yes in [
            "continue",
            "Continue.",
            "go ahead",
            "Go ahead!",
            "ok",
            "OK",
            "okay",
            "yes",
            "Yeah",
            "please continue",
        ] {
            assert!(is_continue_phrase(yes), "{yes}");
        }
        for near in [
            "continue later",
            "don't continue",
            "do not continue",
            "yes but not yet",
            "yes but...",
            "continue?",
            "ok so what about tomorrow",
            "go ahead and cancel it",
            "no",
            "",
            "remind me to call Katie",
        ] {
            assert!(!is_continue_phrase(near), "{near}");
        }
    }

    #[test]
    fn replies_sort_into_continue_cancel_or_new_request() {
        let parse = |text: &str| {
            if text.contains("don't") || text == "no" {
                Some(false)
            } else {
                None
            }
        };
        assert_eq!(classify_reply("continue", parse), ContinueReply::Continue);
        assert_eq!(
            classify_reply("don't continue", parse),
            ContinueReply::Cancel
        );
        assert_eq!(classify_reply("no", parse), ContinueReply::Cancel);
        assert_eq!(
            classify_reply("continue later", parse),
            ContinueReply::Other
        );
        assert_eq!(
            classify_reply("what's on my calendar", parse),
            ContinueReply::Other
        );
    }
}

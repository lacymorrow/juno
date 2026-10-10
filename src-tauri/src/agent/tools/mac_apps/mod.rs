//! # Mac apps: Reminders, Calendar, Contacts, Messages, FaceTime, Mail, Notes
//!
//! Typed tools for the apps the Mac already syncs, so the model fills slots
//! and Rust does the work. The model no longer writes AppleScript for these
//! (Siri parity slices 1 and 2, `docs/plans/siri-parity.md`).
//!
//! Slice 2 adds `messages`, `mail`, `notes` (constant AppleScript through
//! `script`, every value as `argv`), `chat_db` (the Messages history, read
//! only), `recipients` (who a message goes to) and `send` (the send gate's
//! card, phrase and read-back wording).
//!
//! ## Shape
//!
//! - `reminders`, `calendar`, `contacts`: the EventKit and Contacts calls,
//!   macOS only. Each tool is one synchronous function that runs on a blocking
//!   thread, because Objective-C objects are not `Send` and must not be held
//!   across an await.
//! - This file: everything that can be said without a Mac. The tool catalog,
//!   the one date format, the relative "when" a person hears, the permission
//!   states and what each one says, and the card the answer is shown on. All of
//!   it is pure so tests can pin it.
//!
//! ## Permission, in the moment
//!
//! Nothing is asked at install. The first call that needs an app checks its
//! authorization. Not determined: the tool says what it is asking for, brings
//! the system dialog up and waits for the answer. Denied: one sentence and one
//! button. Nothing here names a framework, a plist key or the privacy database.
//!
//! ## What a write reports
//!
//! Every write reads the object back from a fresh store and reports what it
//! saw (title, when, where it lives, id). A write that cannot be read back is
//! an error, never "created".

#![cfg_attr(not(target_os = "macos"), allow(dead_code, unused_imports))]

use std::collections::HashMap;

use chrono::{
    DateTime, Duration, Local, NaiveDate, NaiveDateTime, NaiveTime, SecondsFormat, TimeZone,
};
use serde::Serialize;
use serde_json::{json, Value};

use crate::agent::core::ToolDefinition;
use crate::constants::agent::tool_names;
use crate::constants::permissions::urls;

#[cfg(target_os = "macos")]
mod calendar;
#[cfg(target_os = "macos")]
mod contacts;
#[cfg(target_os = "macos")]
mod reminders;

#[cfg(target_os = "macos")]
mod mail;
#[cfg(target_os = "macos")]
mod messages;
#[cfg(target_os = "macos")]
mod notes;

pub mod chat_db;
pub mod recipients;
pub mod script;
pub mod send;

// Music, Maps, Shortcuts and Focus: external commands, not frameworks, so the
// pure parts (matching, URLs, parsing) are built and tested everywhere.
mod maps;
mod music;
mod proc;
mod shortcuts;

// ---------------------------------------------------------------------------
// The catalog
// ---------------------------------------------------------------------------

/// Tools that only read. Classified Low and listed as deliberately ungated in
/// `risk_classifier`; `risk_classifier` has a test that walks this list.
pub const READ_TOOLS: &[&str] = &[
    tool_names::REMINDERS_LIST,
    tool_names::CALENDAR_EVENTS,
    tool_names::CONTACTS_FIND,
    tool_names::MESSAGES_RECENT,
    tool_names::MAIL_UNREAD,
    tool_names::MAIL_SEARCH,
    tool_names::NOTES_SEARCH,
    tool_names::SHORTCUTS_LIST,
];

/// How dates are written in and out of every tool.
const DATE_NOTE: &str =
    "Dates are ISO 8601 with the local offset, for example 2026-10-09T09:00:00-04:00. \
A bare date such as 2026-10-09 means the whole day.";

fn definition(
    name: &str,
    description: String,
    properties: Value,
    required: &[&str],
) -> ToolDefinition {
    ToolDefinition {
        name: name.to_string(),
        description,
        input_schema: json!({
            "type": "object",
            "properties": properties,
            "required": required,
        }),
        api_type: None,
        beta_flag: None,
    }
}

/// Every Mac app tool, with no app handle needed. Registration, the default
/// tool configuration and the risk gate's drift tests all read this one list.
pub fn tool_definitions() -> Vec<ToolDefinition> {
    let mut tools = app_tool_definitions();
    tools.extend(command_tool_definitions());
    tools
}

/// Reminders, Calendar and Contacts.
fn app_tool_definitions() -> Vec<ToolDefinition> {
    let note = DATE_NOTE;
    vec![
        definition(
            tool_names::REMINDERS_LIST,
            format!(
                "List the person's open reminders in the Reminders app, soonest first. Optionally only one list, \
                 or only those due in a window. Use this instead of AppleScript. {note}"
            ),
            json!({
                "list": {"type": "string", "description": "Name of one Reminders list. Leave out for all lists."},
                "due_from": {"type": "string", "description": "Only reminders due at or after this."},
                "due_to": {"type": "string", "description": "Only reminders due before this."}
            }),
            &[],
        ),
        definition(
            tool_names::REMINDERS_CREATE,
            format!(
                "Add a reminder to the Reminders app. Reads it back and reports what was saved. Use this instead of AppleScript. {note}"
            ),
            json!({
                "title": {"type": "string", "description": "What to be reminded of, in the person's words."},
                "due": {"type": "string", "description": "When it is due. A bare date means that day with no time."},
                "list": {"type": "string", "description": "Reminders list name. Defaults to the person's default list."},
                "notes": {"type": "string", "description": "Extra detail."}
            }),
            &["title"],
        ),
        definition(
            tool_names::REMINDERS_COMPLETE,
            "Mark a reminder done. The id comes from reminders_list or reminders_create. Reads it back.".to_string(),
            json!({"id": {"type": "string", "description": "Reminder id."}}),
            &["id"],
        ),
        definition(
            tool_names::CALENDAR_EVENTS,
            format!(
                "List events on the person's Calendar between two moments, earliest first. Covers every account \
                 Calendar has, Google and iCloud included. Use this instead of AppleScript or a cloud calendar tool. {note}"
            ),
            json!({
                "from": {"type": "string", "description": "Start of the window."},
                "to": {"type": "string", "description": "End of the window. A bare date includes that whole day."}
            }),
            &["from", "to"],
        ),
        definition(
            tool_names::CALENDAR_CREATE_EVENT,
            format!(
                "Add an event to the person's default Calendar. Reads it back and reports what was saved. Invitees \
                 cannot be sent from here: their names are written into the event's notes and the result says so. {note}"
            ),
            json!({
                "title": {"type": "string", "description": "Event title."},
                "start": {"type": "string", "description": "When it starts. A bare date makes an all-day event."},
                "end": {"type": "string", "description": "When it ends. Defaults to one hour after start."},
                "location": {"type": "string", "description": "Where it happens."},
                "invitees": {"type": "array", "items": {"type": "string"}, "description": "Names or emails of people it is with."}
            }),
            &["title", "start"],
        ),
        definition(
            tool_names::CALENDAR_MOVE_EVENT,
            format!(
                "Move a Calendar event to a new start, keeping its length. The id comes from calendar_events or \
                 calendar_create_event. Reads it back. {note}"
            ),
            json!({
                "id": {"type": "string", "description": "Event id."},
                "new_start": {"type": "string", "description": "New start. A bare date keeps the time of day."}
            }),
            &["id", "new_start"],
        ),
        definition(
            tool_names::CALENDAR_DELETE_EVENT,
            "Delete a Calendar event. This cannot be undone, so the person is always asked first. The id comes from calendar_events."
                .to_string(),
            json!({"id": {"type": "string", "description": "Event id."}}),
            &["id"],
        ),
        definition(
            tool_names::CONTACTS_FIND,
            "Find a person in Contacts by name or by relationship (\"my wife\", \"Mom\"). Returns name, phone numbers, \
             email addresses and related people. Use this instead of AppleScript."
                .to_string(),
            json!({"query": {"type": "string", "description": "A name, or a relationship such as \"my wife\"."}}),
            &["query"],
        ),
        definition(
            tool_names::MESSAGES_SEND,
            "Send a text in Messages (iMessage, or SMS when that is the only way to reach them). The person is \
             always shown the message and must say \"send it\" first; you do not ask them yourself. Pass the \
             recipient as the person said it (a name, \"my wife\", or a number) and the body in their words. \
             Use this instead of AppleScript. If more than one person matches, nothing is sent and the names \
             come back: ask which one."
                .to_string(),
            json!({
                "to": {"type": "string", "description": "Who to text: a name, a relationship, or a phone number or email."},
                "body": {"type": "string", "description": "The message, exactly as it should arrive."}
            }),
            &["to", "body"],
        ),
        definition(
            tool_names::MESSAGES_RECENT,
            "Read recent messages the person received in Messages, newest first. Optionally only from one person. \
             Use this for \"what did Sam text me\". Never use AppleScript or the screen for this."
                .to_string(),
            json!({
                "from": {"type": "string", "description": "Only messages from this person (a name or an address)."},
                "limit": {"type": "integer", "description": "How many, 1 to 20. Defaults to 5."}
            }),
            &[],
        ),
        definition(
            tool_names::FACETIME_CALL,
            "Start a FaceTime call to a person in Contacts, right away. Use this for \"FaceTime Mom\" or \"call Doug on FaceTime\"."
                .to_string(),
            json!({
                "contact": {"type": "string", "description": "Who to call: a name, a relationship, or a number or email."},
                "audio": {"type": "boolean", "description": "True for an audio-only call. Defaults to video."}
            }),
            &["contact"],
        ),
        definition(
            tool_names::MAIL_UNREAD,
            "List unread mail in the Mail app's inbox: who it is from, the subject, when. Use this instead of AppleScript."
                .to_string(),
            json!({"limit": {"type": "integer", "description": "How many, 1 to 20. Defaults to 5."}}),
            &[],
        ),
        definition(
            tool_names::MAIL_SEARCH,
            format!(
                "Find mail in the Mail app's inbox by subject or sender. Optionally only from one sender, or only \
                 since a date. Use this instead of AppleScript. {note}"
            ),
            json!({
                "query": {"type": "string", "description": "Words in the subject or the sender."},
                "from": {"type": "string", "description": "Only mail whose sender contains this."},
                "since": {"type": "string", "description": "Only mail received on or after this."}
            }),
            &["query"],
        ),
        definition(
            tool_names::MAIL_SEND,
            "Send an email from the Mail app. The person is always shown the email and must say \"send it\" \
             first; you do not ask them yourself. Use mail_draft instead when they only want it written. Use \
             this instead of AppleScript or a cloud mail tool for mail set up on this Mac."
                .to_string(),
            json!({
                "to": {"type": "string", "description": "Who to email: a name, a relationship, or an email address."},
                "subject": {"type": "string", "description": "The subject line."},
                "body": {"type": "string", "description": "The email, exactly as it should arrive."}
            }),
            &["to", "subject", "body"],
        ),
        definition(
            tool_names::MAIL_DRAFT,
            "Write an email in the Mail app and leave it open, unsent, for the person to finish. Nothing is sent."
                .to_string(),
            json!({
                "to": {"type": "string", "description": "Who it is for: a name, a relationship, or an email address."},
                "subject": {"type": "string", "description": "The subject line."},
                "body": {"type": "string", "description": "The email."}
            }),
            &["to", "body"],
        ),
        definition(
            tool_names::NOTES_CREATE,
            "Make a note in the Notes app. Reads it back. Use this instead of AppleScript.".to_string(),
            json!({
                "title": {"type": "string", "description": "The note's title, its first line."},
                "body": {"type": "string", "description": "The rest of the note."},
                "folder": {"type": "string", "description": "Notes folder. Defaults to the person's default folder."}
            }),
            &["title"],
        ),
        definition(
            tool_names::NOTES_APPEND,
            "Add text to the end of an existing note, found by its title. Reads it back. If more than one note \
             matches, nothing is added and the titles come back."
                .to_string(),
            json!({
                "note": {"type": "string", "description": "The note's title, or part of it."},
                "text": {"type": "string", "description": "What to add."}
            }),
            &["note", "text"],
        ),
        definition(
            tool_names::NOTES_SEARCH,
            "Find notes in the Notes app whose title or text contains some words.".to_string(),
            json!({"query": {"type": "string", "description": "Words to look for."}}),
            &["query"],
        ),
    ]
}

/// Music, Maps, Shortcuts and Focus.
fn command_tool_definitions() -> Vec<ToolDefinition> {
    vec![
        definition(
            tool_names::MUSIC_PLAY,
            "Play something from the person's Apple Music library by name and report what is now playing. \
             Apple Music only: Spotify cannot be searched, so if Spotify is in front the result says so and \
             nothing plays. Use this instead of AppleScript. When it returns a `card`, finish with that tag exactly as given."
                .to_string(),
            json!({
                "query": {"type": "string", "description": "What to play: a song, album, artist or playlist name, as the person said it."},
                "kind": {"type": "string", "enum": ["song", "album", "artist", "playlist"], "description": "Narrow the search when the person said which. Leave out when they did not."}
            }),
            &["query"],
        ),
        definition(
            tool_names::MAPS_DIRECTIONS,
            "Open Maps with directions to a place. Starts from where the person is unless `from` is given. \
             When it returns a `card`, finish with that tag exactly as given."
                .to_string(),
            json!({
                "to": {"type": "string", "description": "The destination, as the person said it."},
                "from": {"type": "string", "description": "Where to start. Leave out for the person's location."},
                "mode": {"type": "string", "enum": ["driving", "walking", "transit"], "description": "How they are getting there."}
            }),
            &["to"],
        ),
        definition(
            tool_names::SHORTCUTS_LIST,
            "List the names of the shortcuts in the person's Shortcuts app. Anything they built there, \
             such as lights, scenes or routines, can be run with shortcuts_run."
                .to_string(),
            json!({}),
            &[],
        ),
        definition(
            tool_names::SHORTCUTS_RUN,
            "Run one of the person's own shortcuts by name. The name is matched to their list, ignoring case \
             and punctuation; if more than one fits, nothing runs and the candidates come back so the person \
             can choose. Optional text is given to the shortcut as its input. Stops waiting after 30 seconds. \
             Use this for \"turn on the porch lights\" and anything else they have built."
                .to_string(),
            json!({
                "name": {"type": "string", "description": "The shortcut's name, as the person said it."},
                "input": {"type": "string", "description": "Text to give the shortcut, when it takes some."}
            }),
            &["name"],
        ),
        definition(
            tool_names::FOCUS_SET,
            "Turn Do Not Disturb on or off. The first time, Juno adds its own Focus shortcut to Shortcuts, which \
             takes one click from the person; say so, then ask nothing else."
                .to_string(),
            json!({
                "state": {"type": "string", "enum": ["on", "off"], "description": "Whether Do Not Disturb should be on or off."}
            }),
            &["state"],
        ),
    ]
}

/// Register all Mac app tools on a provider.
pub async fn register_mac_apps_tools(
    provider: &mut crate::agent::implementations::tool_provider::LocalToolProvider,
) {
    for def in tool_definitions() {
        let name = def.name.clone();
        let exec = move |input: Value| {
            let name = name.clone();
            async move { dispatch(name, input).await }
        };
        provider.register_async_tool(def, exec).await;
    }
    tracing::info!(
        "Registered Mac app tools (Reminders, Calendar, Contacts, Messages, Mail, Notes)"
    );
}

/// Run one tool on a blocking thread and hand back what it observed.
///
/// Also the executor behind the same tools on Juno's MCP server
/// (`agent::providers::juno_mcp`), so the Claude CLI runs this exact path.
pub(crate) async fn dispatch(name: String, input: Value) -> Result<Value, String> {
    tauri::async_runtime::spawn_blocking(move || execute(&name, &input))
        .await
        .map_err(|e| format!("The Mac app request did not finish: {e}"))?
}

#[cfg(target_os = "macos")]
fn execute(name: &str, input: &Value) -> Result<Value, String> {
    match name {
        tool_names::REMINDERS_LIST => reminders::list(input),
        tool_names::REMINDERS_CREATE => reminders::create(input),
        tool_names::REMINDERS_COMPLETE => reminders::complete(input),
        tool_names::CALENDAR_EVENTS => calendar::events(input),
        tool_names::CALENDAR_CREATE_EVENT => calendar::create(input),
        tool_names::CALENDAR_MOVE_EVENT => calendar::move_event(input),
        tool_names::CALENDAR_DELETE_EVENT => calendar::delete(input),
        tool_names::CONTACTS_FIND => contacts::find(input),
        tool_names::MESSAGES_SEND => messages::send(input),
        tool_names::MESSAGES_RECENT => messages::recent(input),
        tool_names::FACETIME_CALL => messages::facetime(input),
        tool_names::MAIL_UNREAD => mail::unread(input),
        tool_names::MAIL_SEARCH => mail::search(input),
        tool_names::MAIL_SEND => mail::send(input),
        tool_names::MAIL_DRAFT => mail::draft(input),
        tool_names::NOTES_CREATE => notes::create(input),
        tool_names::NOTES_APPEND => notes::append(input),
        tool_names::NOTES_SEARCH => notes::search(input),
        tool_names::MUSIC_PLAY => music::play(input),
        tool_names::MAPS_DIRECTIONS => maps::directions(input),
        tool_names::SHORTCUTS_LIST => shortcuts::list(input),
        tool_names::SHORTCUTS_RUN => shortcuts::run(input),
        tool_names::FOCUS_SET => shortcuts::focus(input),
        other => Err(format!("{other} is not a Mac app tool")),
    }
}

#[cfg(not(target_os = "macos"))]
fn execute(_name: &str, _input: &Value) -> Result<Value, String> {
    Err("The Mac app tools are only available on a Mac.".to_string())
}

/// A send as it would go out, recipient resolved through Contacts, for the
/// gate to draw and ask about before anything is sent.
///
/// `Ok(draft)`: show it and wait for "send it". `Err(answer)`: there is
/// nothing to ask about (ambiguous name, no address, Contacts declined, not a
/// send tool); hand this back as the tool's answer. Nothing was sent.
pub async fn preview_send(name: String, input: Value) -> Result<send::SendDraft, Value> {
    tauri::async_runtime::spawn_blocking(move || preview_blocking(&name, &input))
        .await
        .unwrap_or_else(|e| {
            Err(json!({"ok": false, "summary": format!("The message was not sent: {e}")}))
        })
}

#[cfg(target_os = "macos")]
fn preview_blocking(name: &str, input: &Value) -> Result<send::SendDraft, Value> {
    let found = match name {
        tool_names::MESSAGES_SEND => messages::preview(input),
        tool_names::MAIL_SEND => mail::preview(input),
        other => Err(format!("{other} does not send anything")),
    };
    match found {
        Ok(inner) => inner,
        Err(e) => Err(json!({"ok": false, "sent": false, "summary": e})),
    }
}

#[cfg(not(target_os = "macos"))]
fn preview_blocking(_name: &str, _input: &Value) -> Result<send::SendDraft, Value> {
    Err(
        json!({"ok": false, "sent": false, "summary": "Messages and Mail are only available on a Mac."}),
    )
}

/// Whether the person has already allowed this app, asked without showing a
/// dialog. Local intents use it: they answer only what is already allowed and
/// leave the asking to the agent, in the moment.
#[cfg(target_os = "macos")]
pub(crate) fn access_granted(domain: Domain) -> bool {
    eventkit::status(domain) == Access::Granted
}

#[cfg(not(target_os = "macos"))]
pub(crate) fn access_granted(_domain: Domain) -> bool {
    false
}

/// Whether the Juno Focus shortcut is in the person's Shortcuts. Blocking.
pub(crate) fn focus_shortcut_installed() -> bool {
    shortcuts::focus_installed()
}

// ---------------------------------------------------------------------------
// Arguments
// ---------------------------------------------------------------------------

/// A non-empty, trimmed string argument.
pub(crate) fn text_arg<'a>(input: &'a Value, key: &str) -> Option<&'a str> {
    input
        .get(key)
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|s| !s.is_empty())
}

pub(crate) fn required_text<'a>(input: &'a Value, key: &str) -> Result<&'a str, String> {
    text_arg(input, key).ok_or_else(|| format!("Missing {key}."))
}

/// A list of strings, accepting one string as a list of one.
pub(crate) fn list_arg(input: &Value, key: &str) -> Vec<String> {
    match input.get(key) {
        Some(Value::Array(items)) => items
            .iter()
            .filter_map(Value::as_str)
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect(),
        Some(Value::String(s)) if !s.trim().is_empty() => vec![s.trim().to_string()],
        _ => Vec::new(),
    }
}

// ---------------------------------------------------------------------------
// Dates: in and out
// ---------------------------------------------------------------------------

/// What a date argument meant.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum When {
    /// A moment.
    Moment(DateTime<Local>),
    /// A whole day, no time.
    Day(NaiveDate),
}

fn local_from_naive(naive: NaiveDateTime) -> Option<DateTime<Local>> {
    Local.from_local_datetime(&naive).earliest()
}

/// Read an ISO 8601 date. Anything with an offset keeps it; without one the
/// Mac's own time zone is assumed, which is what the person means.
pub fn parse_when(input: &str) -> Result<When, String> {
    let text = input.trim();
    if let Ok(dt) = DateTime::parse_from_rfc3339(text) {
        return Ok(When::Moment(dt.with_timezone(&Local)));
    }
    for format in [
        "%Y-%m-%dT%H:%M:%S%.f",
        "%Y-%m-%dT%H:%M:%S",
        "%Y-%m-%dT%H:%M",
        "%Y-%m-%d %H:%M:%S",
        "%Y-%m-%d %H:%M",
    ] {
        if let Ok(naive) = NaiveDateTime::parse_from_str(text, format) {
            return local_from_naive(naive)
                .map(When::Moment)
                .ok_or_else(|| format!("{text} does not exist on this Mac's clock."));
        }
    }
    if let Ok(day) = NaiveDate::parse_from_str(text, "%Y-%m-%d") {
        return Ok(When::Day(day));
    }
    Err(format!(
        "Could not read the date {text:?}. Use ISO 8601 with the local offset, like 2026-10-09T09:00:00-04:00."
    ))
}

/// The start of a day on this Mac's clock.
pub fn start_of_day(day: NaiveDate) -> Result<DateTime<Local>, String> {
    local_from_naive(day.and_time(NaiveTime::MIN))
        .ok_or_else(|| format!("{day} has no midnight on this Mac's clock."))
}

impl When {
    /// The earliest moment this covers.
    pub fn start(self) -> Result<DateTime<Local>, String> {
        match self {
            When::Moment(dt) => Ok(dt),
            When::Day(day) => start_of_day(day),
        }
    }

    /// The first moment after this: a day ends at the next midnight.
    pub fn end_exclusive(self) -> Result<DateTime<Local>, String> {
        match self {
            When::Moment(dt) => Ok(dt),
            When::Day(day) => {
                let next = day
                    .succ_opt()
                    .ok_or_else(|| format!("{day} is the last day there is."))?;
                start_of_day(next)
            }
        }
    }
}

/// Write a moment the one way every tool writes it: ISO 8601, local offset.
pub fn format_iso(dt: DateTime<Local>) -> String {
    dt.to_rfc3339_opts(SecondsFormat::Secs, false)
}

/// What a time label should carry besides the day.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Clock {
    /// A time of day.
    Timed,
    /// A day with no time, such as a reminder due Friday.
    DateOnly,
    /// An all-day event.
    AllDay,
}

/// "Tomorrow 9:00 AM", "Friday 3:30 PM", "Oct 20 9:00 AM". The label a person
/// hears and the card shows.
pub fn relative_when(dt: DateTime<Local>, now: DateTime<Local>, clock: Clock) -> String {
    let days = (dt.date_naive() - now.date_naive()).num_days();
    let day = match days {
        0 => "Today".to_string(),
        1 => "Tomorrow".to_string(),
        -1 => "Yesterday".to_string(),
        2..=6 => dt.format("%A").to_string(),
        _ => dt.format("%b %-d").to_string(),
    };
    match clock {
        Clock::Timed => format!("{day} {}", dt.format("%-I:%M %p")),
        Clock::DateOnly => day,
        Clock::AllDay => format!("{day}, all day"),
    }
}

/// Calendar components to a local moment. EventKit marks an unset field with
/// the largest integer, which arrives here as `None`.
pub fn from_components(
    year: i64,
    month: i64,
    day: i64,
    hour: Option<i64>,
    minute: Option<i64>,
) -> Option<(NaiveDate, Option<NaiveTime>)> {
    let date = NaiveDate::from_ymd_opt(
        i32::try_from(year).ok()?,
        u32::try_from(month).ok()?,
        u32::try_from(day).ok()?,
    )?;
    match hour {
        None => Some((date, None)),
        Some(h) => {
            let time = NaiveTime::from_hms_opt(
                u32::try_from(h).ok()?,
                u32::try_from(minute.unwrap_or(0)).ok()?,
                0,
            )?;
            Some((date, Some(time)))
        }
    }
}

/// A date and an optional time of day, as a moment on this Mac's clock.
pub fn components_to_local(date: NaiveDate, time: Option<NaiveTime>) -> Option<DateTime<Local>> {
    local_from_naive(date.and_time(time.unwrap_or(NaiveTime::MIN)))
}

/// Keep a moment's time of day and put it on another day.
pub fn on_day(moment: DateTime<Local>, day: NaiveDate) -> Result<DateTime<Local>, String> {
    local_from_naive(day.and_time(moment.time())).ok_or_else(|| {
        format!(
            "{day} has no {} on this Mac's clock.",
            moment.format("%-I:%M %p")
        )
    })
}

/// Where a moved event lands: the new start, with its length kept.
pub fn moved_range(
    old_start: DateTime<Local>,
    old_end: DateTime<Local>,
    new_start: When,
) -> Result<(DateTime<Local>, DateTime<Local>), String> {
    let length: Duration = old_end - old_start;
    let start = match new_start {
        When::Moment(dt) => dt,
        When::Day(day) => on_day(old_start, day)?,
    };
    Ok((start, start + length))
}

/// An event id is its EventKit identifier plus the start of the occurrence, so
/// one occurrence of a repeating event can be told from the next.
pub fn event_id(identifier: &str, start: DateTime<Local>) -> String {
    format!("{identifier}@{}", start.timestamp())
}

/// The reverse of [`event_id`]. A bare identifier has no occurrence.
pub fn split_event_id(id: &str) -> (&str, Option<i64>) {
    match id.rsplit_once('@') {
        Some((identifier, stamp)) => match stamp.parse::<i64>() {
            Ok(secs) => (identifier, Some(secs)),
            Err(_) => (id, None),
        },
        None => (id, None),
    }
}

/// The hour either side of a moment, as seconds, used to find an occurrence.
pub const OCCURRENCE_SLACK_SECS: i64 = 60;

/// Lower-case, trimmed, for comparing names a person typed.
pub fn fold(text: &str) -> String {
    text.trim().to_lowercase()
}

// ---------------------------------------------------------------------------
// One shape for events and reminders
// ---------------------------------------------------------------------------

/// An event or a reminder, as the model and the card both see it.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct AgendaItem {
    /// `"event"` or `"reminder"`.
    pub kind: &'static str,
    /// Pass this back to move, complete or delete.
    pub id: String,
    pub title: String,
    /// The label a person hears: "Tomorrow 9:00 AM". Empty when there is no date.
    pub when: String,
    /// ISO 8601, when there is a date.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub start: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub end: Option<String>,
    /// The calendar or list it lives in.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub list: Option<String>,
    #[serde(rename = "where", skip_serializing_if = "Option::is_none")]
    pub place: Option<String>,
    pub done: bool,
}

/// The part of an item the card draws. The card is display only.
#[derive(Serialize)]
struct CardItem<'a> {
    title: &'a str,
    when: &'a str,
    #[serde(rename = "where", skip_serializing_if = "Option::is_none")]
    place: Option<&'a str>,
    done: bool,
    kind: &'a str,
}

/// Soonest first; undated last; stable inside a tie.
pub fn sort_items(items: &mut [AgendaItem]) {
    items.sort_by(|a, b| match (&a.start, &b.start) {
        (Some(x), Some(y)) => x.cmp(y),
        (Some(_), None) => std::cmp::Ordering::Less,
        (None, Some(_)) => std::cmp::Ordering::Greater,
        (None, None) => std::cmp::Ordering::Equal,
    });
}

/// The JSX the renderer expects, in the way `<NowPlayingCard>` is emitted: the
/// tool hands back the finished tag and the model passes it through.
pub fn agenda_card(items: &[AgendaItem], empty: &str) -> String {
    let card: Vec<CardItem<'_>> = items
        .iter()
        .map(|i| CardItem {
            title: &i.title,
            when: &i.when,
            place: i.place.as_deref(),
            done: i.done,
            kind: i.kind,
        })
        .collect();
    let items_json = serde_json::to_string(&card).unwrap_or_else(|_| "[]".to_string());
    let empty_json = serde_json::to_string(empty).unwrap_or_else(|_| "\"\"".to_string());
    format!("<AgendaCard items={{{items_json}}} empty={{{empty_json}}} />")
}

/// The answer to a list: what was found, as data, as a sentence and as a card.
pub fn listing(items: Vec<AgendaItem>, empty: &str, asked: Option<String>) -> Value {
    let summary = if items.is_empty() {
        empty.to_string()
    } else {
        items
            .iter()
            .map(|i| match (i.when.is_empty(), &i.place) {
                (true, _) => i.title.clone(),
                (false, Some(place)) => format!("{} at {} ({})", i.title, place, i.when),
                (false, None) => format!("{} ({})", i.title, i.when),
            })
            .collect::<Vec<_>>()
            .join("; ")
    };
    let mut out = json!({
        "ok": true,
        "count": items.len(),
        "summary": summary,
        "items": items,
        "card": agenda_card(&items, empty),
    });
    attach_asked(&mut out, asked);
    out
}

/// The answer to a write: the item as it was read back.
pub fn observed(
    verb: &str,
    item: AgendaItem,
    extra: Option<(&str, Value)>,
    asked: Option<String>,
) -> Value {
    let place = item
        .place
        .as_deref()
        .map(|p| format!(" at {p}"))
        .unwrap_or_default();
    let list = item
        .list
        .as_deref()
        .map(|l| format!(" in {l}"))
        .unwrap_or_default();
    let when = if item.when.is_empty() {
        String::new()
    } else {
        format!(" for {}", item.when)
    };
    let summary = format!("{verb}: {}{when}{place}{list}.", item.title);
    let card = agenda_card(std::slice::from_ref(&item), "");
    let mut out = json!({
        "ok": true,
        "summary": summary,
        "observed": item,
        "card": card,
    });
    if let (Some((key, value)), Some(map)) = (extra, out.as_object_mut()) {
        map.insert(key.to_string(), value);
    }
    attach_asked(&mut out, asked);
    out
}

fn attach_asked(out: &mut Value, asked: Option<String>) {
    if let (Some(line), Some(map)) = (asked, out.as_object_mut()) {
        map.insert("asked".to_string(), Value::String(line));
    }
}

// ---------------------------------------------------------------------------
// Permission states
// ---------------------------------------------------------------------------

/// The three apps, as a person names them.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Domain {
    Reminders,
    Calendar,
    Contacts,
}

impl Domain {
    /// What the person calls it.
    pub fn label(self) -> &'static str {
        match self {
            Domain::Reminders => "Reminders",
            Domain::Calendar => "Calendar",
            Domain::Contacts => "Contacts",
        }
    }

    /// Where the one switch lives.
    pub fn settings_url(self) -> &'static str {
        match self {
            Domain::Reminders => urls::REMINDERS_PANEL,
            Domain::Calendar => urls::CALENDARS_PANEL,
            Domain::Contacts => urls::CONTACTS_PANEL,
        }
    }
}

/// What macOS currently says about one app.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Access {
    /// Allowed.
    Granted,
    /// Never asked.
    NotDetermined,
    /// The person said no, or only allowed part of what is needed.
    Denied,
    /// A policy on this Mac forbids it. The person cannot change that here.
    Restricted,
}

/// EventKit's `EKAuthorizationStatus` raw value. Write-only is not enough to
/// read, so it is treated as a no.
pub fn access_from_eventkit(raw: isize) -> Access {
    match raw {
        3 => Access::Granted,
        0 => Access::NotDetermined,
        1 => Access::Restricted,
        _ => Access::Denied,
    }
}

/// Contacts' `CNAuthorizationStatus` raw value. Limited access still reads.
pub fn access_from_contacts(raw: isize) -> Access {
    match raw {
        3 | 4 => Access::Granted,
        0 => Access::NotDetermined,
        1 => Access::Restricted,
        _ => Access::Denied,
    }
}

/// What to do next, given what macOS says.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Gate {
    /// Go ahead.
    Proceed,
    /// Bring the system dialog up and wait for the answer.
    Ask,
    /// Stop with the denied answer.
    Denied,
    /// Stop with the restricted answer.
    Restricted,
}

pub fn gate(access: Access) -> Gate {
    match access {
        Access::Granted => Gate::Proceed,
        Access::NotDetermined => Gate::Ask,
        Access::Denied => Gate::Denied,
        Access::Restricted => Gate::Restricted,
    }
}

/// The line that says what Juno is asking for, in plain words.
pub fn ask_line(domain: Domain) -> String {
    format!("Asking for access to {}.", domain.label())
}

/// The no: one sentence, one action. The card carries the button.
pub fn denied_result(domain: Domain) -> Value {
    let label = domain.label();
    json!({
        "ok": false,
        "access": "denied",
        "summary": format!("Juno does not have access to your {label}. You can turn it on in System Settings."),
        "card": format!(
            "<AgendaCard needsAccess=\"{label}\" settingsUrl=\"{}\" />",
            domain.settings_url()
        ),
    })
}

/// A Mac policy says no. Nothing the person can switch, so no button.
pub fn restricted_result(domain: Domain) -> Value {
    json!({
        "ok": false,
        "access": "restricted",
        "summary": format!("This Mac does not let Juno use {}.", domain.label()),
    })
}

/// The dialog was shown and not answered in time.
pub fn waiting_result(domain: Domain) -> Value {
    json!({
        "ok": false,
        "access": "asking",
        "summary": format!("{} Answer the box on screen, then ask me again.", ask_line(domain)),
    })
}

/// What a platform check came to, once the dialog (if any) has been through.
#[derive(Debug)]
pub enum Cleared {
    /// Use the app. `asked` carries the line to repeat when a dialog just ran.
    Go { asked: Option<String> },
    /// Return this as the tool result.
    Stop(Value),
}

/// Fold the status before and after asking into the next step. Pure, so the
/// whole decision table is tested without a Mac.
pub fn clear(
    domain: Domain,
    before: Access,
    after_asking: impl FnOnce() -> Option<Access>,
) -> Cleared {
    match gate(before) {
        Gate::Proceed => Cleared::Go { asked: None },
        Gate::Denied => Cleared::Stop(denied_result(domain)),
        Gate::Restricted => Cleared::Stop(restricted_result(domain)),
        Gate::Ask => match after_asking() {
            Some(Access::Granted) => Cleared::Go {
                asked: Some(ask_line(domain)),
            },
            Some(Access::Restricted) => Cleared::Stop(restricted_result(domain)),
            Some(_) => Cleared::Stop(denied_result(domain)),
            None => Cleared::Stop(waiting_result(domain)),
        },
    }
}

/// Map names to ids without allocating twice. Used to look a list up by name.
pub fn find_by_name<'a, T>(named: &'a [(String, T)], wanted: &str) -> Option<&'a T> {
    let wanted = fold(wanted);
    named
        .iter()
        .find(|(name, _)| fold(name) == wanted)
        .map(|(_, v)| v)
}

/// Group names for an error the model can use: "Lists: Home, Work".
pub fn name_list(names: &[String]) -> String {
    let mut seen: HashMap<String, ()> = HashMap::new();
    names
        .iter()
        .filter(|n| seen.insert(fold(n), ()).is_none())
        .cloned()
        .collect::<Vec<_>>()
        .join(", ")
}

/// The EventKit plumbing Reminders and Calendar share.
///
/// Objective-C objects are not `Send`, so nothing here crosses an await: every
/// tool runs start to finish on one blocking thread, and the two things that
/// call back from another queue (the access dialog and the reminders fetch)
/// hand plain Rust data through a oneshot channel.
#[cfg(target_os = "macos")]
pub(crate) mod eventkit {
    use std::sync::Mutex;
    use std::time::Duration as StdDuration;

    use block2::RcBlock;
    use objc2::rc::Retained;
    use objc2::runtime::Bool;
    use objc2_event_kit::{EKAuthorizationStatus, EKEntityType, EKEventStore};
    use objc2_foundation::{NSDate, NSError};
    use tokio::sync::oneshot;

    use super::*;

    /// How long to wait for a person to answer the system dialog.
    const DIALOG_WAIT_SECS: u64 = 120;

    /// Wait for a callback's answer. `None` when it never came.
    pub fn wait<T>(rx: oneshot::Receiver<T>, secs: u64) -> Option<T> {
        tauri::async_runtime::block_on(async move {
            tokio::time::timeout(StdDuration::from_secs(secs), rx)
                .await
                .ok()
                .and_then(Result::ok)
        })
    }

    /// A store to read or write through. Hold it as long as anything read from it.
    pub fn new_store() -> Retained<EKEventStore> {
        // SAFETY: `+new` on a plain NSObject subclass.
        unsafe { EKEventStore::new() }
    }

    pub fn ns_date(dt: DateTime<Local>) -> Retained<NSDate> {
        // Whole seconds are all a reminder or an event needs.
        NSDate::dateWithTimeIntervalSince1970(dt.timestamp() as f64)
    }

    pub fn from_ns_date(date: &NSDate) -> Option<DateTime<Local>> {
        Local
            .timestamp_opt(date.timeIntervalSince1970().floor() as i64, 0)
            .single()
    }

    pub fn ns_error(error: &NSError) -> String {
        error.localizedDescription().to_string()
    }

    fn entity(domain: Domain) -> EKEntityType {
        match domain {
            Domain::Reminders => EKEntityType::Reminder,
            _ => EKEntityType::Event,
        }
    }

    /// What macOS says right now.
    pub fn status(domain: Domain) -> Access {
        // SAFETY: a class method that only reads the current answer.
        let raw: EKAuthorizationStatus =
            unsafe { EKEventStore::authorizationStatusForEntityType(entity(domain)) };
        access_from_eventkit(raw.0)
    }

    /// Show the system dialog and wait for the answer.
    fn request(domain: Domain) -> Option<Access> {
        let (tx, rx) = oneshot::channel::<bool>();
        let slot = Mutex::new(Some(tx));
        let store = new_store();
        let block = RcBlock::new(move |granted: Bool, _error: *mut NSError| {
            if let Ok(mut guard) = slot.lock() {
                if let Some(tx) = guard.take() {
                    let _ = tx.send(granted.as_bool());
                }
            }
        });
        let handler = &*block as *const _ as *mut _;
        // SAFETY: the block outlives the wait below, and the store stays alive
        // until the answer is in.
        unsafe {
            match domain {
                Domain::Reminders => store.requestFullAccessToRemindersWithCompletion(handler),
                _ => store.requestFullAccessToEventsWithCompletion(handler),
            }
        }
        wait(rx, DIALOG_WAIT_SECS)?;
        Some(status(domain))
    }

    /// Check the app's access; if it was never asked, ask now.
    pub fn clear_access(domain: Domain) -> Cleared {
        clear(domain, status(domain), || request(domain))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Timelike;

    fn at(y: i32, m: u32, d: u32, h: u32, min: u32) -> DateTime<Local> {
        local_from_naive(
            NaiveDate::from_ymd_opt(y, m, d)
                .unwrap_or_default()
                .and_hms_opt(h, min, 0)
                .unwrap_or_default(),
        )
        .unwrap_or_else(Local::now)
    }

    #[test]
    fn iso_with_an_offset_keeps_the_moment() {
        let When::Moment(dt) =
            parse_when("2026-10-09T09:00:00-04:00").unwrap_or(When::Day(NaiveDate::MIN))
        else {
            panic!("expected a moment");
        };
        assert_eq!(dt.timestamp(), 1_791_550_800);
    }

    #[test]
    fn iso_without_an_offset_is_this_macs_clock() {
        let parsed = parse_when("2026-10-09T09:00");
        assert_eq!(parsed, Ok(When::Moment(at(2026, 10, 9, 9, 0))));
        let parsed = parse_when("2026-10-09 15:30:00");
        assert_eq!(parsed, Ok(When::Moment(at(2026, 10, 9, 15, 30))));
    }

    #[test]
    fn a_bare_date_is_a_whole_day() {
        let day = NaiveDate::from_ymd_opt(2026, 10, 9).unwrap_or_default();
        assert_eq!(parse_when("2026-10-09"), Ok(When::Day(day)));
        let start = When::Day(day).start().unwrap_or_else(|_| Local::now());
        let end = When::Day(day)
            .end_exclusive()
            .unwrap_or_else(|_| Local::now());
        assert_eq!(end.date_naive(), day.succ_opt().unwrap_or_default());
        assert_eq!(start.hour(), 0);
        assert_eq!(end.hour(), 0);
    }

    #[test]
    fn nonsense_dates_say_what_to_send_instead() {
        let err = parse_when("tomorrow at nine").err().unwrap_or_default();
        assert!(err.contains("ISO 8601"), "{err}");
        assert!(parse_when("").is_err());
    }

    #[test]
    fn dates_go_out_with_the_local_offset() {
        let out = format_iso(at(2026, 10, 9, 9, 0));
        assert!(out.starts_with("2026-10-09T09:00:00"), "{out}");
        assert!(
            !out.ends_with('Z') || Local::now().offset().local_minus_utc() == 0,
            "{out}"
        );
        assert_eq!(parse_when(&out), Ok(When::Moment(at(2026, 10, 9, 9, 0))));
    }

    #[test]
    fn when_reads_the_way_a_person_says_it() {
        let now = at(2026, 10, 8, 14, 0);
        assert_eq!(
            relative_when(at(2026, 10, 8, 15, 0), now, Clock::Timed),
            "Today 3:00 PM"
        );
        assert_eq!(
            relative_when(at(2026, 10, 9, 9, 0), now, Clock::Timed),
            "Tomorrow 9:00 AM"
        );
        assert_eq!(
            relative_when(at(2026, 10, 7, 9, 5), now, Clock::Timed),
            "Yesterday 9:05 AM"
        );
        assert_eq!(
            relative_when(at(2026, 10, 10, 0, 30), now, Clock::Timed),
            "Saturday 12:30 AM"
        );
        assert_eq!(
            relative_when(at(2026, 10, 20, 9, 0), now, Clock::Timed),
            "Oct 20 9:00 AM"
        );
        assert_eq!(
            relative_when(at(2026, 10, 9, 0, 0), now, Clock::DateOnly),
            "Tomorrow"
        );
        assert_eq!(
            relative_when(at(2026, 10, 9, 0, 0), now, Clock::AllDay),
            "Tomorrow, all day"
        );
    }

    #[test]
    fn unset_components_mean_no_time() {
        let day = NaiveDate::from_ymd_opt(2026, 10, 9).unwrap_or_default();
        assert_eq!(from_components(2026, 10, 9, None, None), Some((day, None)));
        let nine = NaiveTime::from_hms_opt(9, 0, 0);
        assert_eq!(
            from_components(2026, 10, 9, Some(9), None),
            Some((day, nine))
        );
        assert_eq!(from_components(2026, 13, 9, None, None), None);
        assert_eq!(from_components(2026, 10, 9, Some(25), Some(0)), None);
        assert_eq!(from_components(2026, 0, 9, None, None), None);
    }

    #[test]
    fn a_moved_event_keeps_its_length() {
        let start = at(2026, 10, 9, 15, 0);
        let end = at(2026, 10, 9, 16, 30);
        let (s, e) =
            moved_range(start, end, When::Moment(at(2026, 10, 9, 16, 0))).unwrap_or((start, end));
        assert_eq!(s, at(2026, 10, 9, 16, 0));
        assert_eq!(e, at(2026, 10, 9, 17, 30));
        let day = NaiveDate::from_ymd_opt(2026, 10, 12).unwrap_or_default();
        let (s, e) = moved_range(start, end, When::Day(day)).unwrap_or((start, end));
        assert_eq!(s, at(2026, 10, 12, 15, 0));
        assert_eq!(e, at(2026, 10, 12, 16, 30));
    }

    #[test]
    fn event_ids_round_trip() {
        let start = at(2026, 10, 9, 15, 0);
        let id = event_id("ABC:123", start);
        assert_eq!(split_event_id(&id), ("ABC:123", Some(start.timestamp())));
        assert_eq!(split_event_id("ABC"), ("ABC", None));
        assert_eq!(split_event_id("a@b"), ("a@b", None));
    }

    fn item(title: &str, start: Option<&str>) -> AgendaItem {
        AgendaItem {
            kind: "event",
            id: title.to_string(),
            title: title.to_string(),
            when: String::new(),
            start: start.map(str::to_string),
            end: None,
            list: None,
            place: None,
            done: false,
        }
    }

    #[test]
    fn items_sort_soonest_first_with_undated_last() {
        let mut items = vec![
            item("none", None),
            item("b", Some("2026-10-09T10:00:00-04:00")),
            item("a", Some("2026-10-09T09:00:00-04:00")),
        ];
        sort_items(&mut items);
        let titles: Vec<&str> = items.iter().map(|i| i.title.as_str()).collect();
        assert_eq!(titles, ["a", "b", "none"]);
    }

    #[test]
    fn the_card_is_the_tag_the_renderer_expects() {
        let mut one = item("Call Katie", Some("2026-10-09T09:00:00-04:00"));
        one.when = "Tomorrow 9:00 AM".to_string();
        one.kind = "reminder";
        let tag = agenda_card(&[one], "Nothing due.");
        assert!(tag.starts_with("<AgendaCard items={[{"), "{tag}");
        assert!(tag.contains("\"title\":\"Call Katie\""), "{tag}");
        assert!(tag.contains("\"when\":\"Tomorrow 9:00 AM\""), "{tag}");
        assert!(tag.contains("\"done\":false"), "{tag}");
        assert!(tag.ends_with("empty={\"Nothing due.\"} />"), "{tag}");
        assert!(!tag.contains("\"id\""), "the card never carries ids: {tag}");
    }

    #[test]
    fn an_empty_list_says_so_in_one_line() {
        let out = listing(Vec::new(), "Nothing on your calendar today.", None);
        assert_eq!(out["count"], 0);
        assert_eq!(out["summary"], "Nothing on your calendar today.");
        assert!(out["card"]
            .as_str()
            .unwrap_or_default()
            .contains("items={[]}"));
    }

    #[test]
    fn a_write_reports_what_was_read_back() {
        let mut saved = item("Dentist", Some("2026-10-09T15:00:00-04:00"));
        saved.when = "Tomorrow 3:00 PM".to_string();
        saved.list = Some("Home".to_string());
        saved.place = Some("Main St".to_string());
        let out = observed("Added to your calendar", saved, None, None);
        assert_eq!(
            out["summary"],
            "Added to your calendar: Dentist for Tomorrow 3:00 PM at Main St in Home."
        );
        assert_eq!(out["observed"]["id"], "Dentist");
        assert!(out.get("asked").is_none());
    }

    #[test]
    fn authorization_status_maps_to_one_gate_each() {
        for (raw, want) in [
            (0, Gate::Ask),
            (1, Gate::Restricted),
            (2, Gate::Denied),
            (3, Gate::Proceed),
            (4, Gate::Denied),
            (99, Gate::Denied),
        ] {
            assert_eq!(gate(access_from_eventkit(raw)), want, "eventkit {raw}");
        }
        for (raw, want) in [
            (0, Gate::Ask),
            (1, Gate::Restricted),
            (2, Gate::Denied),
            (3, Gate::Proceed),
            (4, Gate::Proceed),
        ] {
            assert_eq!(gate(access_from_contacts(raw)), want, "contacts {raw}");
        }
    }

    #[test]
    fn not_determined_says_what_is_being_asked_then_follows_the_answer() {
        assert_eq!(
            ask_line(Domain::Reminders),
            "Asking for access to Reminders."
        );

        let yes = clear(Domain::Reminders, Access::NotDetermined, || {
            Some(Access::Granted)
        });
        assert!(
            matches!(yes, Cleared::Go { asked: Some(ref l) } if l == "Asking for access to Reminders.")
        );

        let no = clear(Domain::Calendar, Access::NotDetermined, || {
            Some(Access::Denied)
        });
        assert!(matches!(no, Cleared::Stop(ref v) if v["access"] == "denied"));

        let silent = clear(Domain::Contacts, Access::NotDetermined, || None);
        assert!(
            matches!(silent, Cleared::Stop(ref v) if v["access"] == "asking"
            && v["summary"].as_str().unwrap_or_default().starts_with("Asking for access to Contacts."))
        );

        let known = clear(Domain::Reminders, Access::Granted, || None);
        assert!(
            matches!(known, Cleared::Go { asked: None }),
            "a granted app is never asked again"
        );
    }

    #[test]
    fn a_dialog_is_never_shown_for_an_answer_already_given() {
        let never = || -> Option<Access> { panic!("asked again") };
        assert!(matches!(
            clear(Domain::Reminders, Access::Denied, never),
            Cleared::Stop(_)
        ));
        assert!(matches!(
            clear(Domain::Reminders, Access::Restricted, never),
            Cleared::Stop(_)
        ));
        assert!(matches!(
            clear(Domain::Reminders, Access::Granted, never),
            Cleared::Go { .. }
        ));
    }

    #[test]
    fn denied_is_one_sentence_and_one_button_and_names_no_framework() {
        for domain in [Domain::Reminders, Domain::Calendar, Domain::Contacts] {
            let out = denied_result(domain);
            let summary = out["summary"].as_str().unwrap_or_default();
            assert_eq!(
                summary.matches('.').count(),
                2,
                "two short sentences at most: {summary}"
            );
            let card = out["card"].as_str().unwrap_or_default();
            assert!(card.contains(domain.settings_url()), "{card}");
            assert!(
                card.contains(&format!("needsAccess=\"{}\"", domain.label())),
                "{card}"
            );
            for banned in ["EventKit", "TCC", "NS", "plist", "framework", "Privacy_"] {
                assert!(!summary.contains(banned), "{banned} in {summary}");
            }
        }
        assert_eq!(
            Domain::Reminders.settings_url(),
            "x-apple.systempreferences:com.apple.preference.security?Privacy_Reminders"
        );
        assert_eq!(
            Domain::Calendar.settings_url(),
            "x-apple.systempreferences:com.apple.preference.security?Privacy_Calendars"
        );
        assert_eq!(
            Domain::Contacts.settings_url(),
            "x-apple.systempreferences:com.apple.preference.security?Privacy_Contacts"
        );
    }

    #[test]
    fn restricted_offers_no_button() {
        assert!(restricted_result(Domain::Calendar).get("card").is_none());
    }

    #[test]
    fn the_catalog_matches_the_names_the_gate_knows() {
        let names: Vec<String> = tool_definitions().into_iter().map(|d| d.name).collect();
        assert_eq!(names.len(), 18);
        for read in READ_TOOLS {
            assert!(names.iter().any(|n| n == read), "{read}");
        }
    }

    #[test]
    fn list_arguments_accept_one_string_or_many() {
        assert_eq!(
            list_arg(&json!({"invitees": ["Doug", " ", "Katie"]}), "invitees"),
            ["Doug", "Katie"]
        );
        assert_eq!(list_arg(&json!({"invitees": "Doug"}), "invitees"), ["Doug"]);
        assert!(list_arg(&json!({}), "invitees").is_empty());
    }

    #[test]
    fn list_names_are_found_without_regard_to_case() {
        let lists = vec![("Groceries".to_string(), 1), ("Work".to_string(), 2)];
        assert_eq!(find_by_name(&lists, " work "), Some(&2));
        assert_eq!(find_by_name(&lists, "play"), None);
        assert_eq!(
            name_list(&["Work".to_string(), "work".to_string(), "Home".to_string()]),
            "Work, Home"
        );
    }
}

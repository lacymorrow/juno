//! Messages and FaceTime.
//!
//! Sending is constant AppleScript (`send` to a participant of the iMessage or
//! SMS account, both in the Tahoe dictionary) with the address, body and
//! service as `argv`. Reading is `chat.db`, read only. After a send the
//! conversation is read back and "Sent." is said only when the row is there and
//! marked sent.
//!
//! The send itself is gated before it gets here (the Send class in
//! `risk_classifier`, asked in every mode, answered only by "send it" or the
//! button). This module never decides whether to ask.

use std::collections::HashMap;
use std::path::PathBuf;
use std::time::Duration as StdDuration;

use serde_json::{json, Value};

use super::chat_db;
use super::contacts;
use super::recipients::{self, Channel, Recipient, Resolved, Service};
use super::script::{self, App, Outcome};
use super::send::{self, Evidence, SendDraft, SendKind};
use super::*;

/// How long to look for the sent message in the conversation.
const READ_BACK_TRIES: u32 = 16;
const READ_BACK_EVERY: StdDuration = StdDuration::from_millis(500);

/// Most messages one read hands back.
const MAX_RECENT: usize = 20;

/// `send` to a participant of the account for the chosen service.
/// argv: address, body, service ("iMessage" or "SMS").
const SEND_BODY: &str = r#"set theAddress to item 1 of argv
set theBody to item 2 of argv
set theService to item 3 of argv
tell application "Messages"
if theService is "SMS" then
set theAccount to first account whose service type is SMS
else
set theAccount to first account whose service type is iMessage
end if
set theParticipant to participant theAddress of theAccount
send theBody to theParticipant
end tell
return "OK""#;

// ---------------------------------------------------------------------------
// The history, when it can be read
// ---------------------------------------------------------------------------

/// The readable history and every handle in it.
struct History {
    path: PathBuf,
    handles: Vec<(i64, String, String)>,
}

fn history() -> Option<History> {
    match chat_db::access() {
        chat_db::Access::Readable(path) => {
            let handles = chat_db::handles(&path).ok()?;
            Some(History { path, handles })
        }
        _ => None,
    }
}

/// Which services Messages has used with each address, keyed by
/// [`recipients::address_key`]. Empty when the history cannot be read.
fn known_services(history: Option<&History>) -> HashMap<String, Vec<Service>> {
    let mut known: HashMap<String, Vec<Service>> = HashMap::new();
    if let Some(history) = history {
        for (_, address, service) in &history.handles {
            if let Some(service) = Service::from_db(service) {
                known
                    .entry(recipients::address_key(address))
                    .or_default()
                    .push(service);
            }
        }
    }
    known
}

/// The handle rows that are this address, in any spelling.
fn handle_ids_for(history: &History, address: &str) -> Vec<i64> {
    let key = recipients::address_key(address);
    history
        .handles
        .iter()
        .filter(|(_, a, _)| recipients::address_key(a) == key)
        .map(|(id, _, _)| *id)
        .collect()
}

// ---------------------------------------------------------------------------
// Recipients
// ---------------------------------------------------------------------------

/// Settle who a message or call goes to. `Ok(Err(answer))` is a finished
/// answer: ambiguous, unknown, no address, or Contacts declined. Nothing was
/// sent.
pub(crate) fn resolve_recipient(
    to: &str,
    channel: Channel,
) -> Result<Result<Recipient, Value>, String> {
    let history = history();
    let known = known_services(history.as_ref());

    if recipients::address_kind(to).is_some() {
        return Ok(
            match recipients::from_typed_address(to, channel, &known) {
                Some(recipient) => Ok(recipient),
                None => Err(recipients::unresolved_result(
                    to,
                    &Resolved::NoAddress(to.to_string()),
                    channel,
                )),
            },
        );
    }

    let (people, _asked) = match contacts::people_for(to)? {
        Ok(found) => found,
        Err(answer) => return Ok(Err(answer)),
    };
    let candidates = people.iter().map(contacts::candidate).collect();
    Ok(match recipients::resolve(to, candidates, channel, &known) {
        Resolved::One(recipient) => Ok(recipient),
        other => Err(recipients::unresolved_result(to, &other, channel)),
    })
}

/// The text as it would go out, recipient resolved. Used by the gate to draw
/// the card and by [`send`] to send exactly that.
pub(crate) fn preview(input: &Value) -> Result<Result<SendDraft, Value>, String> {
    let to = required_text(input, "to")?;
    let body = required_text(input, "body")?;
    Ok(resolve_recipient(to, Channel::Text)?.map(|recipient| SendDraft {
        kind: SendKind::Text,
        recipient,
        subject: None,
        body: body.to_string(),
    }))
}

// ---------------------------------------------------------------------------
// messages_send
// ---------------------------------------------------------------------------

/// Look for the message in the conversation until it shows, or say what was seen.
fn read_back(draft: &SendDraft, started_unix: i64) -> Evidence {
    let mut last = None;
    for attempt in 0..READ_BACK_TRIES {
        if attempt > 0 {
            std::thread::sleep(READ_BACK_EVERY);
        }
        let Some(history) = history() else {
            return Evidence::Unseen(
                "Messages took it. I can't see your conversations to confirm it went out."
                    .to_string(),
            );
        };
        let ids = handle_ids_for(&history, &draft.recipient.address);
        let rows = chat_db::sent_since(&history.path, &ids, started_unix - 5).unwrap_or_default();
        match chat_db::evidence_for(&rows, &draft.body) {
            Some(Evidence::Waiting) => last = Some(Evidence::Waiting),
            Some(done) => return done,
            None => {}
        }
    }
    last.unwrap_or_else(|| {
        Evidence::Unseen(
            "Messages took it, but it has not shown up in the conversation yet.".to_string(),
        )
    })
}

/// `messages_send`. Only ever reached after the person said "send it" or
/// pressed Send.
pub fn send(input: &Value) -> Result<Value, String> {
    let draft = match preview(input)? {
        Ok(draft) => draft,
        Err(answer) => return Ok(answer),
    };
    let service = draft
        .recipient
        .service
        .unwrap_or(Service::IMessage)
        .as_str()
        .to_string();
    let started = Local::now().timestamp();
    let argv = vec![
        draft.recipient.address.clone(),
        draft.body.clone(),
        service,
    ];
    match script::run(&script::wrap(SEND_BODY), argv)? {
        Outcome::Ok(_) => {}
        Outcome::Err { number, message } => {
            return Ok(script::error_result(App::Messages, number, &message))
        }
    }
    Ok(send::sent_result(&draft, &read_back(&draft, started)))
}

// ---------------------------------------------------------------------------
// messages_recent
// ---------------------------------------------------------------------------

/// The one ask for reading messages: say it, open the pane, show one button.
fn needs_full_disk_access() -> Value {
    // There is no system dialog for this one, so Juno opens the row itself, at
    // the moment it is needed, and the card carries the same action.
    if let Err(e) = std::process::Command::new("/usr/bin/open")
        .arg(urls::FULL_DISK_ACCESS_PANEL)
        .status()
    {
        tracing::warn!("Could not open the Full Disk Access row: {}", e);
    }
    full_disk_access_result()
}

/// One sentence, one action.
pub(crate) fn full_disk_access_result() -> Value {
    json!({
        "ok": false,
        "access": "denied",
        "summary": "I need Full Disk Access to read your messages.",
        "card": format!(
            "<MessageCard needsAccess=\"Full Disk Access\" reason={{\"I need Full Disk Access to read your messages.\"}} settingsUrl=\"{}\" />",
            urls::FULL_DISK_ACCESS_PANEL
        ),
    })
}

/// `messages_recent`
pub fn recent(input: &Value) -> Result<Value, String> {
    let limit = input
        .get("limit")
        .and_then(Value::as_u64)
        .and_then(|n| usize::try_from(n).ok())
        .unwrap_or(5)
        .clamp(1, MAX_RECENT);
    let from = text_arg(input, "from");

    let path = match chat_db::access() {
        chat_db::Access::Readable(path) => path,
        chat_db::Access::NoAccess => return Ok(needs_full_disk_access()),
        chat_db::Access::Missing => {
            return Ok(json!({
                "ok": true,
                "count": 0,
                "summary": "There are no messages on this Mac.",
                "messages": [],
            }))
        }
    };
    let handles = chat_db::handles(&path)?;
    let history = History { path, handles };

    // Who "from" is: a typed address, or everyone Contacts says that name is.
    let mut from_name: Option<String> = None;
    let mut ids: Vec<i64> = Vec::new();
    if let Some(from) = from {
        let addresses: Vec<String> = if recipients::address_kind(from).is_some() {
            vec![from.to_string()]
        } else {
            let (people, _asked) = match contacts::people_for(from)? {
                Ok(found) => found,
                Err(answer) => return Ok(answer),
            };
            if people.is_empty() {
                return Ok(json!({
                    "ok": true,
                    "count": 0,
                    "summary": format!("Nobody in Contacts matches {from}."),
                    "messages": [],
                }));
            }
            from_name = people.first().map(|p| p.name.clone());
            people
                .iter()
                .flat_map(|p| p.phones.iter().chain(p.emails.iter()))
                .map(|l| l.value.clone())
                .collect()
        };
        for address in &addresses {
            ids.extend(handle_ids_for(&history, address));
        }
        if ids.is_empty() {
            let who = from_name.unwrap_or_else(|| from.to_string());
            return Ok(json!({
                "ok": true,
                "count": 0,
                "summary": format!("No messages from {who} on this Mac."),
                "messages": [],
            }));
        }
    }

    let rows = chat_db::received(&history.path, &ids, limit)?;
    let by_id: HashMap<i64, &str> = history
        .handles
        .iter()
        .map(|(id, address, _)| (*id, address.as_str()))
        .collect();
    let mut names: HashMap<i64, String> = HashMap::new();
    let now = Local::now();

    let messages: Vec<Value> = rows
        .iter()
        .filter(|r| !r.text.trim().is_empty())
        .map(|row| {
            let sender = names
                .entry(row.handle_id)
                .or_insert_with(|| {
                    let address = by_id.get(&row.handle_id).copied().unwrap_or_default();
                    contacts::name_for_address(address).unwrap_or_else(|| address.to_string())
                })
                .clone();
            let when = Local
                .timestamp_opt(row.at, 0)
                .single()
                .map(|dt| relative_when(dt, now, Clock::Timed))
                .unwrap_or_default();
            json!({ "from": sender, "when": when, "text": row.text })
        })
        .collect();

    let summary = if messages.is_empty() {
        "No new messages.".to_string()
    } else {
        messages
            .iter()
            .map(|m| {
                format!(
                    "{} ({}): {}",
                    m["from"].as_str().unwrap_or_default(),
                    m["when"].as_str().unwrap_or_default(),
                    m["text"].as_str().unwrap_or_default()
                )
            })
            .collect::<Vec<_>>()
            .join("\n")
    };
    Ok(json!({
        "ok": true,
        "count": messages.len(),
        "summary": summary,
        "messages": messages,
    }))
}

// ---------------------------------------------------------------------------
// facetime_call
// ---------------------------------------------------------------------------

/// Only these characters reach the FaceTime URL: a phone number or an email.
fn safe_for_url(address: &str) -> bool {
    !address.is_empty()
        && address
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '+' | '@' | '.' | '-' | '_'))
}

/// `facetime_call`. Acts at once, the way Siri does.
pub fn facetime(input: &Value) -> Result<Value, String> {
    let to = text_arg(input, "contact")
        .or_else(|| text_arg(input, "to"))
        .ok_or_else(|| "Missing contact.".to_string())?;
    let audio = input.get("audio").and_then(Value::as_bool).unwrap_or(false);
    let recipient = match resolve_recipient(to, Channel::Call)? {
        Ok(recipient) => recipient,
        Err(mut answer) => {
            // The shared wording says "Nothing was sent"; for a call, nothing was dialled.
            if let Some(summary) = answer.get("summary").and_then(Value::as_str) {
                let fixed = summary.replace("Nothing was sent.", "No call was made.");
                answer["summary"] = Value::String(fixed);
            }
            return Ok(answer);
        }
    };
    let address = if recipient.address.contains('@') {
        recipient.address.clone()
    } else {
        recipients::normalize_phone(&recipient.address)
    };
    if !safe_for_url(&address) {
        return Ok(json!({
            "ok": false,
            "summary": format!("{} cannot be called with FaceTime.", recipient.name),
        }));
    }
    let scheme = if audio { "facetime-audio" } else { "facetime" };
    let url = format!("{scheme}://{address}");
    let status = std::process::Command::new("/usr/bin/open")
        .arg(&url)
        .status()
        .map_err(|e| format!("FaceTime did not open: {e}"))?;
    if !status.success() {
        return Ok(json!({
            "ok": false,
            "summary": "FaceTime did not open.",
        }));
    }
    let name = recipient.name;
    Ok(json!({
        "ok": true,
        "summary": format!("FaceTime is calling {name}."),
        "to": name,
        "address": address,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_an_address_reaches_the_facetime_url() {
        assert!(safe_for_url("+17045550100"));
        assert!(safe_for_url("doug@example.com"));
        assert!(!safe_for_url("doug@example.com?x=1"));
        assert!(!safe_for_url("a b"));
        assert!(!safe_for_url(""));
    }

    #[test]
    fn the_full_disk_access_answer_is_one_sentence_and_one_action() {
        let out = full_disk_access_result();
        assert_eq!(
            out["summary"],
            "I need Full Disk Access to read your messages."
        );
        let card = out["card"].as_str().unwrap_or_default();
        assert!(card.contains("Privacy_AllFiles"), "{card}");
        for banned in ["TCC", "plist", "chat.db", "framework", "sqlite"] {
            assert!(!out["summary"].as_str().unwrap_or_default().contains(banned));
            assert!(!card.contains(banned), "{banned} in {card}");
        }
    }

    #[test]
    fn the_send_script_takes_everything_as_argv() {
        let script = script::wrap(SEND_BODY);
        assert!(script.contains("item 1 of argv"));
        assert!(script.contains("send theBody to theParticipant"));
        assert!(!script.contains("{}"), "nothing is spliced in");
    }
}

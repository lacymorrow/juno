//! Mail: what is unread, finding a message, a draft, and a send.
//!
//! Constant AppleScript through Mail's own dictionary, every value as `argv`.
//! Dates cross as "seconds from now" integers, so nothing depends on how this
//! Mac writes a date. A send is read back from the Sent mailbox and called
//! "Sent." only when it is there. The send is gated before it gets here.

use serde_json::{json, Value};

use super::messages::resolve_recipient;
use super::recipients::Channel;
use super::script::{self, App, Outcome};
use super::send::{self, Evidence, SendDraft, SendKind};
use super::*;

/// Most messages one answer carries.
const MAX_MESSAGES: i64 = 20;

/// argv: limit. First record: the unread count. Then sender, subject, seconds
/// from now it arrived (negative).
const UNREAD_BODY: &str = r#"set lim to (item 1 of argv) as integer
set out to "OK" & US
tell application "Mail"
set msgs to (messages of inbox whose read status is false)
set total to count of msgs
set out to out & (total as text) & RS
set n to total
if n > lim then set n to lim
repeat with i from 1 to n
set m to item i of msgs
set out to out & (sender of m) & US & (subject of m) & US & ((((date received of m) - (current date)) as integer) as text) & RS
end repeat
end tell
return out"#;

/// argv: query, from, seconds back, limit. Records as in [`UNREAD_BODY`],
/// without the count. An empty `from` matches every sender.
const SEARCH_BODY: &str = r#"set q to item 1 of argv
set f to item 2 of argv
set secs to (item 3 of argv) as integer
set lim to (item 4 of argv) as integer
set sinceDate to (current date) - secs
set out to "OK" & US
tell application "Mail"
set msgs to (messages of inbox whose ((subject contains q) or (sender contains q)) and (sender contains f) and (date received > sinceDate))
set n to count of msgs
if n > lim then set n to lim
repeat with i from 1 to n
set m to item i of msgs
set out to out & (sender of m) & US & (subject of m) & US & ((((date received of m) - (current date)) as integer) as text) & RS
end repeat
end tell
return out"#;

/// argv: address, subject, body. Sends, then watches Sent for it.
/// Answers "sent", "unseen" or "refused".
const SEND_BODY: &str = r#"set a to item 1 of argv
set s to item 2 of argv
set b to item 3 of argv
set startDate to (current date) - 5
tell application "Mail"
set m to make new outgoing message with properties {subject:s, content:b, visible:false}
tell m to make new to recipient at end of to recipients with properties {address:a}
set didSend to send m
end tell
if didSend is false then return "OK" & US & "refused"
repeat 15 times
delay 1
tell application "Mail"
set hits to (messages of sent mailbox whose subject is s and date sent > startDate)
end tell
if (count of hits) > 0 then return "OK" & US & "sent"
end repeat
return "OK" & US & "unseen""#;

/// argv: address, subject, body. Opens the message in a window, unsent, and
/// answers how many open messages have that subject.
const DRAFT_BODY: &str = r#"set a to item 1 of argv
set s to item 2 of argv
set b to item 3 of argv
tell application "Mail"
set m to make new outgoing message with properties {subject:s, content:b, visible:true}
tell m to make new to recipient at end of to recipients with properties {address:a}
activate
set n to count of (outgoing messages whose subject is s)
end tell
return "OK" & US & (n as text)"#;

/// One message in a list, worded for a person.
fn listed(records: &[Vec<String>]) -> Vec<Value> {
    let now = Local::now();
    records
        .iter()
        .filter(|r| r.len() >= 3)
        .map(|r| {
            let offset = r[2].trim().parse::<i64>().unwrap_or(0);
            let when = Local
                .timestamp_opt(now.timestamp() + offset, 0)
                .single()
                .map(|dt| relative_when(dt, now, Clock::Timed))
                .unwrap_or_default();
            json!({ "from": sender_name(&r[0]), "subject": r[1], "when": when })
        })
        .collect()
}

/// "Doug Keesler <doug@example.com>" reads as "Doug Keesler".
pub fn sender_name(raw: &str) -> String {
    let raw = raw.trim();
    match raw.split_once('<') {
        Some((name, _)) if !name.trim().is_empty() => name.trim().trim_matches('"').to_string(),
        _ => raw
            .trim_start_matches('<')
            .trim_end_matches('>')
            .to_string(),
    }
}

fn summary_of(messages: &[Value]) -> String {
    messages
        .iter()
        .map(|m| {
            format!(
                "{}: {} ({})",
                m["from"].as_str().unwrap_or_default(),
                m["subject"].as_str().unwrap_or_default(),
                m["when"].as_str().unwrap_or_default()
            )
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn limit(input: &Value, default: i64) -> i64 {
    input
        .get("limit")
        .and_then(Value::as_i64)
        .unwrap_or(default)
        .clamp(1, MAX_MESSAGES)
}

/// `mail_unread`
pub fn unread(input: &Value) -> Result<Value, String> {
    let records = match script::run(
        &script::wrap(UNREAD_BODY),
        vec![limit(input, 5).to_string()],
    )? {
        Outcome::Ok(records) => records,
        Outcome::Err { number, message } => {
            return Ok(script::error_result(App::Mail, number, &message))
        }
    };
    let total = records
        .first()
        .and_then(|r| r.first())
        .and_then(|n| n.trim().parse::<usize>().ok())
        .unwrap_or(0);
    let messages = listed(records.get(1..).unwrap_or_default());
    let summary = match total {
        0 => "No unread mail.".to_string(),
        n if n > messages.len() => format!(
            "{n} unread. The newest {}:\n{}",
            messages.len(),
            summary_of(&messages)
        ),
        n => format!("{n} unread:\n{}", summary_of(&messages)),
    };
    Ok(json!({ "ok": true, "unread": total, "summary": summary, "messages": messages }))
}

/// `mail_search`
pub fn search(input: &Value) -> Result<Value, String> {
    let query = text_arg(input, "query").unwrap_or_default();
    let from = text_arg(input, "from").unwrap_or_default();
    if query.is_empty() && from.is_empty() {
        return Err("Missing query.".to_string());
    }
    // How far back, as whole seconds from now. No `since` means ten years.
    let seconds_back = match text_arg(input, "since") {
        Some(since) => {
            let start = parse_when(since)?.start()?;
            (Local::now() - start).num_seconds().max(0)
        }
        None => 10 * 365 * 24 * 3600,
    };
    let argv = vec![
        query.to_string(),
        from.to_string(),
        seconds_back.to_string(),
        limit(input, 10).to_string(),
    ];
    let records = match script::run(&script::wrap(SEARCH_BODY), argv)? {
        Outcome::Ok(records) => records,
        Outcome::Err { number, message } => {
            return Ok(script::error_result(App::Mail, number, &message))
        }
    };
    let messages = listed(&records);
    let summary = if messages.is_empty() {
        "No mail matches that.".to_string()
    } else {
        summary_of(&messages)
    };
    Ok(json!({ "ok": true, "count": messages.len(), "summary": summary, "messages": messages }))
}

/// The email as it would go out, recipient resolved.
pub(crate) fn preview(input: &Value) -> Result<Result<SendDraft, Value>, String> {
    let to = required_text(input, "to")?;
    let body = required_text(input, "body")?;
    let subject = text_arg(input, "subject").map(str::to_string);
    Ok(
        resolve_recipient(to, Channel::Email)?.map(|recipient| SendDraft {
            kind: SendKind::Email,
            recipient,
            subject,
            body: body.to_string(),
        }),
    )
}

fn argv_for(draft: &SendDraft) -> Vec<String> {
    vec![
        draft.recipient.address.clone(),
        draft.subject.clone().unwrap_or_default(),
        draft.body.clone(),
    ]
}

/// What Mail answered after a send, as evidence.
pub fn send_evidence(answer: &str) -> Evidence {
    match answer.trim() {
        "sent" => Evidence::Confirmed,
        "refused" => Evidence::Failed,
        _ => Evidence::Unseen("Mail took it, but it has not shown up in Sent yet.".to_string()),
    }
}

/// `mail_send`. Only ever reached after "send it" or the Send button.
pub fn send(input: &Value) -> Result<Value, String> {
    let draft = match preview(input)? {
        Ok(draft) => draft,
        Err(answer) => return Ok(answer),
    };
    let answer = match script::run(&script::wrap(SEND_BODY), argv_for(&draft))? {
        Outcome::Ok(records) => records
            .first()
            .and_then(|r| r.first())
            .cloned()
            .unwrap_or_default(),
        Outcome::Err { number, message } => {
            return Ok(script::error_result(App::Mail, number, &message))
        }
    };
    Ok(send::sent_result(&draft, &send_evidence(&answer)))
}

/// `mail_draft`. Free: it sends nothing and stays open in Mail.
pub fn draft(input: &Value) -> Result<Value, String> {
    let draft = match preview(input)? {
        Ok(draft) => draft,
        Err(answer) => {
            let mut answer = answer;
            if let Some(summary) = answer.get("summary").and_then(Value::as_str) {
                let fixed = summary.replace("Nothing was sent.", "No draft was made.");
                answer["summary"] = Value::String(fixed);
            }
            return Ok(answer);
        }
    };
    let open = match script::run(&script::wrap(DRAFT_BODY), argv_for(&draft))? {
        Outcome::Ok(records) => records
            .first()
            .and_then(|r| r.first())
            .and_then(|n| n.trim().parse::<usize>().ok())
            .unwrap_or(0),
        Outcome::Err { number, message } => {
            return Ok(script::error_result(App::Mail, number, &message))
        }
    };
    if open == 0 {
        return Ok(json!({
            "ok": false,
            "summary": "Mail did not open the draft.",
        }));
    }
    Ok(json!({
        "ok": true,
        "sent": false,
        "summary": format!("The draft to {} is open in Mail. Nothing was sent.", draft.recipient.name),
        "to": draft.recipient.name,
        "address": draft.recipient.address,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_sender_reads_as_a_name() {
        assert_eq!(
            sender_name("Doug Keesler <doug@example.com>"),
            "Doug Keesler"
        );
        assert_eq!(sender_name("\"Katie B\" <k@example.com>"), "Katie B");
        assert_eq!(sender_name("<k@example.com>"), "k@example.com");
        assert_eq!(sender_name("k@example.com"), "k@example.com");
    }

    #[test]
    fn sent_comes_only_from_the_sent_mailbox() {
        assert_eq!(send_evidence("sent"), Evidence::Confirmed);
        assert_eq!(send_evidence("refused"), Evidence::Failed);
        assert!(matches!(send_evidence("unseen"), Evidence::Unseen(_)));
        assert!(matches!(send_evidence(""), Evidence::Unseen(_)));
    }

    #[test]
    fn every_mail_script_takes_its_values_as_argv() {
        for body in [UNREAD_BODY, SEARCH_BODY, SEND_BODY, DRAFT_BODY] {
            let script = script::wrap(body);
            assert!(script.contains("item 1 of argv"), "{body}");
            assert!(!script.contains("{}"), "{body}");
        }
        assert!(!DRAFT_BODY.contains("send m"), "a draft never sends");
    }

    #[test]
    fn a_list_reads_newest_first_as_written() {
        let rows = vec![vec![
            "Doug <d@example.com>".to_string(),
            "Invoice".to_string(),
            "-120".to_string(),
        ]];
        let out = listed(&rows);
        assert_eq!(out[0]["from"], "Doug");
        assert_eq!(out[0]["subject"], "Invoice");
        assert!(
            out[0]["when"]
                .as_str()
                .unwrap_or_default()
                .starts_with("Today")
                || out[0]["when"]
                    .as_str()
                    .unwrap_or_default()
                    .starts_with("Yesterday")
        );
    }
}

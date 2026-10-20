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

/// argv: address, subject, body, then any file paths to attach. Sends, then
/// watches Sent for it. Answers "sent", "unseen" or "refused".
const SEND_BODY: &str = r#"set a to item 1 of argv
set s to item 2 of argv
set b to item 3 of argv
set startDate to (current date) - 5
tell application "Mail"
set m to make new outgoing message with properties {subject:s, content:b, visible:false}
tell m to make new to recipient at end of to recipients with properties {address:a}
if (count of argv) > 3 then
repeat with i from 4 to (count of argv)
tell content of m to make new attachment with properties {file name:(POSIX file (item i of argv))} at after last paragraph
end repeat
end if
-- Mail adds attachments asynchronously; a send before they land goes without them.
set want to (count of argv) - 3
repeat 40 times
if (count of attachments of content of m) >= want then exit repeat
delay 0.25
end repeat
if (count of attachments of content of m) < want then return "OK" & US & "unattached"
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

/// argv: address, subject, body, "1" to bring Mail forward, then any file
/// paths to attach. Opens the message in a window, unsent, and answers how
/// many open messages have that subject and how many files it holds.
///
/// The compose window comes from `visible:true` alone. `activate` is what
/// brings Mail to the front, and in background mode that is focus theft: the
/// draft lands on top of whatever the person is typing in (LAC-5007). So it
/// is a separate argument, and background mode passes "0".
const DRAFT_BODY: &str = r#"set a to item 1 of argv
set s to item 2 of argv
set b to item 3 of argv
set bringToFront to item 4 of argv
tell application "Mail"
set m to make new outgoing message with properties {subject:s, content:b, visible:true}
tell m to make new to recipient at end of to recipients with properties {address:a}
if (count of argv) > 4 then
repeat with i from 5 to (count of argv)
tell content of m to make new attachment with properties {file name:(POSIX file (item i of argv))} at after last paragraph
end repeat
end if
if bringToFront is "1" then activate
set n to count of (outgoing messages whose subject is s)
set k to count of attachments of content of m
end tell
return "OK" & US & (n as text) & US & (k as text)"#;

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
    let attachments = attachment_paths(input)?;
    Ok(
        resolve_recipient(to, Channel::Email)?.map(|recipient| SendDraft {
            kind: SendKind::Email,
            recipient,
            subject,
            body: body.to_string(),
            attachments,
        }),
    )
}

/// The `attachments` argument as absolute paths of files that exist. `~/` is
/// the home folder. A path that is not a file fails the whole call, so nothing
/// is drafted or sent without a file the person asked for.
pub fn attachment_paths(input: &Value) -> Result<Vec<String>, String> {
    let Some(list) = input.get("attachments").and_then(Value::as_array) else {
        return Ok(Vec::new());
    };
    let home = dirs::home_dir();
    let mut paths = Vec::new();
    let mut missing = Vec::new();
    for raw in list.iter().filter_map(Value::as_str).map(str::trim) {
        if raw.is_empty() {
            continue;
        }
        let path = match (raw.strip_prefix("~/"), &home) {
            (Some(rest), Some(home)) => home.join(rest),
            _ => std::path::PathBuf::from(raw),
        };
        if path.is_absolute() && path.is_file() {
            paths.push(path.to_string_lossy().into_owned());
        } else {
            missing.push(raw.to_string());
        }
    }
    if !missing.is_empty() {
        return Err(format!(
            "No file at {}. Give the full path of each file to attach.",
            missing.join(", ")
        ));
    }
    Ok(paths)
}

fn argv_for(draft: &SendDraft) -> Vec<String> {
    let mut argv = vec![
        draft.recipient.address.clone(),
        draft.subject.clone().unwrap_or_default(),
        draft.body.clone(),
    ];
    argv.extend(draft.attachments.iter().cloned());
    argv
}

/// The same values with the foreground flag in fourth place, for [`DRAFT_BODY`].
///
/// `front` is read from the platform mirror of the background-mode setting,
/// which is the one source synchronous code can ask. A draft made while the
/// person is working appears in Mail without Mail coming forward.
fn draft_argv(draft: &SendDraft, front: bool) -> Vec<String> {
    let mut argv = vec![
        draft.recipient.address.clone(),
        draft.subject.clone().unwrap_or_default(),
        draft.body.clone(),
        if front { "1" } else { "0" }.to_string(),
    ];
    argv.extend(draft.attachments.iter().cloned());
    argv
}

/// What Mail answered after a send, as evidence.
pub fn send_evidence(answer: &str) -> Evidence {
    match answer.trim() {
        "sent" => Evidence::Confirmed,
        "refused" => Evidence::Failed,
        "unattached" => Evidence::Failed,
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
    let front = !computer_use_ai_sdk::background::is_background_mode();
    let (open, attached) = match script::run(&script::wrap(DRAFT_BODY), draft_argv(&draft, front))?
    {
        Outcome::Ok(records) => {
            let field = |i: usize| {
                records
                    .first()
                    .and_then(|r| r.get(i))
                    .and_then(|n| n.trim().parse::<usize>().ok())
                    .unwrap_or(0)
            };
            (field(0), field(1))
        }
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
    let files = match (draft.attachments.len(), attached) {
        (0, _) => String::new(),
        (want, got) if got >= want => format!(" {want} attached."),
        (want, got) => {
            format!(" Only {got} of {want} files attached; add the rest in the Mail window.")
        }
    };
    // Where the window is, said plainly. In background mode Mail was never
    // brought forward, so "open in Mail" on its own would read as "on your
    // screen" and send the model looking for a window that is behind.
    let where_it_is = if front {
        "is open in Mail."
    } else {
        "is open in Mail, behind what you are working in."
    };
    Ok(json!({
        "ok": true,
        "sent": false,
        "summary": format!(
            "The draft to {} {where_it_is}{files} Nothing was sent.",
            draft.recipient.name
        ),
        "to": draft.recipient.name,
        "address": draft.recipient.address,
        "observed": { "attachments": attached, "brought_to_front": front },
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
        // Files reach both scripts only as argv items after the body.
        for body in [SEND_BODY, DRAFT_BODY] {
            assert!(body.contains("item i of argv"), "{body}");
            assert!(body.contains("make new attachment"), "{body}");
        }
    }

    /// Every `set <name> to item N of argv` line actually binds.
    ///
    /// This exists because a grep-only test cannot see a reserved word.
    /// `set bringToFront to item 4 of argv` reads as `set beginning to ...` and fails
    /// at runtime with -10006, so every `mail_draft` call would have returned
    /// "Mail did not do it" while `cargo test` stayed green. Only the argv
    /// prelude is run, so no mail app is touched and nothing is drafted.
    #[cfg(target_os = "macos")]
    #[test]
    fn every_argv_binding_is_a_name_applescript_will_accept() {
        for (name, body) in [
            ("UNREAD_BODY", UNREAD_BODY),
            ("SEARCH_BODY", SEARCH_BODY),
            ("SEND_BODY", SEND_BODY),
            ("DRAFT_BODY", DRAFT_BODY),
        ] {
            let prelude: Vec<&str> = body
                .lines()
                .take_while(|line| line.trim_start().starts_with("set "))
                .filter(|line| line.contains("of argv"))
                .collect();
            assert!(!prelude.is_empty(), "{name} binds nothing from argv");

            let script = script::wrap(&format!("{}\nreturn \"OK\"", prelude.join("\n")));
            let argv = vec!["1".to_string(); 8];
            let out = std::process::Command::new("osascript")
                .arg("-e")
                .arg(&script)
                .args(&argv)
                .output()
                .expect("osascript must run on macOS");
            let stdout = String::from_utf8_lossy(&out.stdout);
            assert!(
                matches!(script::parse(&stdout), script::Outcome::Ok(_)),
                "{name} has a binding AppleScript will not accept: {}{}",
                stdout.trim(),
                String::from_utf8_lossy(&out.stderr).trim()
            );
        }
    }

    /// LAC-5007. The compose window comes from `visible:true`; `activate` is
    /// what puts Mail on top of the person's work. It only runs when the
    /// caller asked for the front, and background mode never does.
    #[test]
    fn a_draft_only_takes_the_front_when_it_was_asked_to() {
        assert!(
            !DRAFT_BODY.contains("\nactivate"),
            "an unconditional activate steals focus from whatever they are typing in"
        );
        assert!(DRAFT_BODY.contains("if bringToFront is \"1\" then activate"));
        assert!(
            DRAFT_BODY.contains("visible:true"),
            "the window still opens"
        );

        let draft = SendDraft {
            kind: SendKind::Email,
            recipient: crate::agent::tools::mac_apps::recipients::Recipient {
                name: "Jenny".to_string(),
                address: "jenny@example.com".to_string(),
                service: None,
            },
            subject: Some("Receipts".to_string()),
            body: "Both attached.".to_string(),
            attachments: vec!["/tmp/a.pdf".to_string(), "/tmp/b.pdf".to_string()],
        };
        // The flag is argv item 4, so the files start at 5 and the script's
        // `repeat with i from 5` lines up with them.
        assert_eq!(draft_argv(&draft, false)[3], "0");
        assert_eq!(draft_argv(&draft, true)[3], "1");
        assert_eq!(
            &draft_argv(&draft, false)[4..],
            ["/tmp/a.pdf", "/tmp/b.pdf"]
        );
        // A send takes no flag, so its files still start at item 4.
        assert_eq!(&argv_for(&draft)[3..], ["/tmp/a.pdf", "/tmp/b.pdf"]);
        assert!(SEND_BODY.contains("repeat with i from 4"));
        assert!(DRAFT_BODY.contains("repeat with i from 5"));
        // A send never goes out before its files are in the message.
        let wait = SEND_BODY.find("count of attachments").unwrap_or(usize::MAX);
        let send = SEND_BODY.find("send m").unwrap_or(0);
        assert!(wait < send);
    }

    #[test]
    fn attachments_must_be_real_files() {
        assert_eq!(attachment_paths(&json!({})), Ok(Vec::new()));
        let here = std::env::current_exe()
            .map(|p| p.to_string_lossy().into_owned())
            .unwrap_or_default();
        assert_eq!(
            attachment_paths(&json!({"attachments": [here.clone()]})),
            Ok(vec![here])
        );
        let err = attachment_paths(&json!({"attachments": ["/no/such/quote.pdf"]}));
        assert!(err.is_err_and(|e| e.contains("/no/such/quote.pdf")));
        assert!(attachment_paths(&json!({"attachments": ["relative.pdf"]})).is_err());
    }

    #[test]
    fn an_attachment_that_never_lands_is_not_a_send() {
        assert_eq!(send_evidence("unattached"), Evidence::Failed);
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

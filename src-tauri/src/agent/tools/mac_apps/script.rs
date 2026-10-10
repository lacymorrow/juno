//! Constant AppleScript for Messages, Mail and Notes.
//!
//! The rule from `docs/plans/local-intents.md`: a script is constant text, and
//! everything the person said reaches it only as `argv` (`on run argv`). No
//! name, body or subject is ever spliced into a script.
//!
//! Every script here answers in one shape, so nothing reads an error message
//! to decide what happened. A script ends in one of:
//!
//! - `OK` then fields: success. Fields are separated by the ASCII unit
//!   separator (31) and records by the record separator (30), which no person
//!   types into a message.
//! - `ERR`, the AppleScript error number, the message: a failure. The number
//!   is what is matched on (-1743 is "not allowed to control this app").
//!
//! The wrapper every script shares is [`wrap`], so the protocol is written once.

use std::time::Duration;

use serde_json::{json, Value};

use crate::constants::permissions::urls;

/// Field separator inside a record.
pub const US: char = '\u{1f}';
/// Record separator.
pub const RS: char = '\u{1e}';

/// AppleScript's "not allowed to send Apple events to this app".
pub const ERR_NOT_PERMITTED: i64 = -1743;
/// "Can't get" an object: no such note, folder, account.
pub const ERR_NO_SUCH_OBJECT: i64 = -1728;
/// The app did not answer in time, which is what an unanswered dialog looks like.
pub const ERR_TIMED_OUT: i64 = -1712;

/// How long a script may run. Long, because the first run of each app sits
/// behind the system's "Juno wants to control ..." dialog until it is answered.
pub const SCRIPT_TIMEOUT: Duration = Duration::from_secs(90);

/// The AppleScript prelude every script opens with: separators as variables.
const PRELUDE: &str = "set US to (character id 31)\nset RS to (character id 30)\n";

/// Wrap a script body in `on run argv`, the separators, and the one error shape.
///
/// `body` must `return "OK" & ...` on success. It may read `argv`, `US`, `RS`.
pub fn wrap(body: &str) -> String {
    format!(
        "on run argv\n{PRELUDE}try\n{body}\non error errMsg number errNum\nreturn \"ERR\" & US & (errNum as text) & US & errMsg\nend try\nend run"
    )
}

/// What a script came back with.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Outcome {
    /// Records, each a list of fields. The leading `OK` is removed.
    Ok(Vec<Vec<String>>),
    /// An AppleScript error.
    Err { number: i64, message: String },
}

/// Read a script's stdout. Anything that is not the protocol is an error with
/// number 0, carrying the text, so a broken script is visible, never "OK".
pub fn parse(stdout: &str) -> Outcome {
    let text = stdout.trim_end_matches(['\n', '\r']);
    if let Some(rest) = text.strip_prefix("ERR") {
        let mut fields = rest.trim_start_matches(US).splitn(2, US);
        let number = fields
            .next()
            .and_then(|n| n.trim().parse::<i64>().ok())
            .unwrap_or(0);
        let message = fields.next().unwrap_or_default().trim().to_string();
        return Outcome::Err { number, message };
    }
    match text.strip_prefix("OK") {
        Some(rest) => {
            let rest = rest.strip_prefix(US).unwrap_or(rest);
            if rest.is_empty() {
                return Outcome::Ok(Vec::new());
            }
            Outcome::Ok(
                rest.split(RS)
                    .filter(|r| !r.is_empty())
                    .map(|record| record.split(US).map(str::to_string).collect())
                    .collect(),
            )
        }
        None => Outcome::Err {
            number: 0,
            message: text.trim().to_string(),
        },
    }
}

/// The app a script drives, as a person names it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum App {
    Messages,
    Mail,
    Notes,
}

impl App {
    pub fn label(self) -> &'static str {
        match self {
            App::Messages => "Messages",
            App::Mail => "Mail",
            App::Notes => "Notes",
        }
    }
}

/// The answer when macOS says Juno may not control an app: one sentence, one
/// button that opens the Automation row.
pub fn not_permitted_result(app: App) -> Value {
    let label = app.label();
    json!({
        "ok": false,
        "access": "denied",
        "summary": format!("Juno is not allowed to control {label}. You can allow it in System Settings."),
        "card": format!(
            "<MessageCard needsAccess=\"{label}\" settingsUrl=\"{}\" />",
            urls::AUTOMATION_PANEL
        ),
    })
}

/// The answer when the "Juno wants to control ..." dialog was not answered.
pub fn waiting_result(app: App) -> Value {
    json!({
        "ok": false,
        "access": "asking",
        "summary": format!(
            "I need to control {} for that. Answer the box on screen, then ask me again.",
            app.label()
        ),
    })
}

/// Turn an error outcome into the tool's answer. Callers that expect a
/// particular error (such as "no such note") match its number first.
pub fn error_result(app: App, number: i64, message: &str) -> Value {
    match number {
        ERR_NOT_PERMITTED => not_permitted_result(app),
        ERR_TIMED_OUT => waiting_result(app),
        _ => json!({
            "ok": false,
            "summary": format!("{} did not do it: {}", app.label(), message.trim()),
        }),
    }
}

/// Run a constant script with its argv, from Juno's process, and read the
/// protocol back. Blocking: call from the tool's blocking thread.
pub fn run(script: &str, argv: Vec<String>) -> Result<Outcome, String> {
    let script = script.to_string();
    let stdout = tauri::async_runtime::block_on(async move {
        crate::agent::local_intents::osascript_for(&script, argv, SCRIPT_TIMEOUT).await
    })?;
    Ok(parse(&stdout))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_wrapper_keeps_variable_input_out_of_the_script() {
        let script = wrap("return \"OK\" & US & (item 1 of argv)");
        assert!(script.starts_with("on run argv\n"));
        assert!(script.contains("on error errMsg number errNum"));
        assert!(script.ends_with("end run"));
    }

    #[test]
    fn ok_records_and_fields_come_apart() {
        let out = format!("OK{US}a{US}b{RS}c{US}d{RS}\n");
        assert_eq!(
            parse(&out),
            Outcome::Ok(vec![
                vec!["a".to_string(), "b".to_string()],
                vec!["c".to_string(), "d".to_string()],
            ])
        );
        assert_eq!(parse("OK\n"), Outcome::Ok(Vec::new()));
        // A body with newlines and tabs survives.
        let body = "line one\n\tline two";
        assert_eq!(
            parse(&format!("OK{US}{body}")),
            Outcome::Ok(vec![vec![body.to_string()]])
        );
    }

    #[test]
    fn errors_are_matched_by_number_not_by_text() {
        let out = format!("ERR{US}-1743{US}Not authorized to send Apple events to Messages.");
        assert_eq!(
            parse(&out),
            Outcome::Err {
                number: ERR_NOT_PERMITTED,
                message: "Not authorized to send Apple events to Messages.".to_string()
            }
        );
        let denied = error_result(App::Messages, ERR_NOT_PERMITTED, "anything at all");
        assert_eq!(denied["access"], "denied");
        assert!(denied["card"]
            .as_str()
            .unwrap_or_default()
            .contains("Privacy_Automation"));
        for banned in ["TCC", "Apple events", "plist", "framework", "osascript"] {
            assert!(
                !denied["summary"].as_str().unwrap_or_default().contains(banned),
                "{banned}"
            );
        }
        assert_eq!(error_result(App::Mail, ERR_TIMED_OUT, "")["access"], "asking");
    }

    #[test]
    fn anything_off_protocol_is_an_error_never_ok() {
        assert_eq!(
            parse("true"),
            Outcome::Err {
                number: 0,
                message: "true".to_string()
            }
        );
    }
}

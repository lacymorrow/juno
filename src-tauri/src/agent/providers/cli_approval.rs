//! # Per-send approval for the Claude CLI provider (LAC-4058)
//!
//! The Claude CLI used to run under `--dangerously-skip-permissions`. Once
//! account connectors load (LAC-4056), a spoken "email Cameron" would send
//! mail with nothing in the way but prompt text. This module puts the person
//! back in the loop, the way Juno already asks for other permissions:
//!
//! - The CLI is pointed at an `approve` tool on Juno's loopback MCP server
//!   (`--permission-prompt-tool mcp__juno__approve`) instead of skipping
//!   permissions.
//! - Everyday tools stay pre-approved via `--allowedTools`, so only calls
//!   that are not covered there reach `approve`.
//! - `approve` auto-allows read-only connector calls and prompts for
//!   anything that sends, schedules, deletes or otherwise writes, using the
//!   same `tool-approval-request` sheet the in-process provider uses.
//! - A denial returns the reason text to the CLI, so the model tells the
//!   person "not sent, you declined" instead of failing silently.
//!
//! The "Ask before Juno sends" setting (default on) gates the whole path;
//! off restores the old skip-permissions behaviour for testing.

use std::collections::HashMap;
use std::sync::{LazyLock, Mutex};
use std::time::{Duration, Instant};

use serde_json::{json, Value};
use tauri::{Emitter, Manager};
use tracing::{info, warn};

use crate::constants::events;
use crate::constants::settings::{store_keys, SETTINGS_STORE_FILE};
use crate::state::{AppState, RiskLevel, ToolApprovalRequest};

/// The approve tool's name on Juno's MCP server, as the CLI addresses it.
pub const APPROVE_TOOL_CLI_NAME: &str = "mcp__juno__approve";

/// The approve tool's local name on the server.
pub const APPROVE_TOOL_NAME: &str = "approve";

/// Tools the CLI may use without a per-call prompt. The CLI's own toolset
/// keeps working exactly as it did under skip-permissions; what changes is
/// connector writes. Juno's `computer` is listed so desktop automation never
/// round-trips through the prompt either.
pub const ALLOWED_TOOLS: &str =
    "Bash,Read,Edit,Write,Glob,Grep,WebFetch,WebSearch,NotebookEdit,TodoWrite,Task,mcp__juno__computer";

/// How long the sheet waits for an answer before denying.
const APPROVAL_TIMEOUT_SECS: u64 = 60;

/// After a denial, an identical ask inside this window is denied without a
/// new sheet, the way `permission_gate.rs` stops a tool loop from nagging.
const DENY_COOLDOWN: Duration = Duration::from_secs(60);

/// Is "Ask before Juno sends" on? Default on: a person who has never seen
/// the setting gets the prompt, not the silent send. Read fresh from the
/// store so flipping it needs no restart.
pub fn is_enabled(app: &tauri::AppHandle) -> bool {
    use tauri_plugin_store::StoreExt;
    app.store(SETTINGS_STORE_FILE)
        .ok()
        .and_then(|store| store.get(store_keys::CLI_ASK_BEFORE_SEND_ENABLED))
        .and_then(|value| value.as_bool())
        .unwrap_or(true)
}

/// Read the "Ask before Juno sends" flag.
#[tauri::command]
pub async fn get_cli_ask_before_send_enabled(app_handle: tauri::AppHandle) -> Result<bool, String> {
    Ok(is_enabled(&app_handle))
}

/// Turn "Ask before Juno sends" on or off.
#[tauri::command]
pub async fn set_cli_ask_before_send_enabled(
    app_handle: tauri::AppHandle,
    enabled: bool,
) -> Result<(), String> {
    use tauri_plugin_store::StoreExt;
    let store = app_handle
        .store(SETTINGS_STORE_FILE)
        .map_err(|e| format!("Failed to access settings store: {e}"))?;
    store.set(
        store_keys::CLI_ASK_BEFORE_SEND_ENABLED,
        Value::Bool(enabled),
    );
    store
        .save()
        .map_err(|e| format!("Failed to save settings store: {e}"))?;
    info!(
        "[CliApproval] Ask before Juno sends {}",
        if enabled { "enabled" } else { "disabled" }
    );
    Ok(())
}

/// The permission arguments for a CLI spawn.
///
/// With the setting on and Juno's MCP server wired in, the CLI routes
/// permission prompts to `approve` and pre-approves the everyday tools.
/// Without the server there is nothing to serve the prompt, so the old
/// skip-permissions behaviour stays (that is the headless and test path,
/// where no connectors load under `--strict-mcp-config` anyway).
pub fn permission_args(ask_before_send: bool, has_mcp_server: bool) -> Vec<String> {
    if ask_before_send && has_mcp_server {
        vec![
            "--permission-prompt-tool".to_string(),
            APPROVE_TOOL_CLI_NAME.to_string(),
            "--allowedTools".to_string(),
            ALLOWED_TOOLS.to_string(),
        ]
    } else {
        vec!["--dangerously-skip-permissions".to_string()]
    }
}

/// What `approve` decides before any sheet is shown.
#[derive(Debug, PartialEq, Eq)]
pub enum Verdict {
    /// Runs without asking: Juno's own tools and read-only connector calls.
    Allow,
    /// Shows the approval sheet and waits for the person.
    Prompt,
}

/// Verbs that read. A connector tool whose name carries one of these — and
/// none of the write verbs — runs without a prompt.
const READ_VERBS: &[&str] = &[
    "get", "list", "search", "read", "fetch", "find", "lookup", "view", "show", "describe",
    "status", "history", "query", "retrieve", "count", "check", "download", "export",
];

/// Verbs that change something on the other side. Any of these anywhere in
/// the name wins over a read verb: `mark_as_read` is a write, whatever
/// "read" says.
const WRITE_VERBS: &[&str] = &[
    "send", "create", "update", "delete", "post", "reply", "draft", "schedule", "move", "archive",
    "upload", "add", "remove", "set", "write", "edit", "insert", "invite", "cancel", "mark",
    "publish", "submit", "execute", "run", "trigger", "patch", "put", "share", "forward", "react",
    "pin", "star", "assign", "close", "merge", "approve",
];

/// Decide whether a call the CLI could not resolve on its own runs or asks.
///
/// Everything unrecognized prompts. A prompt too many costs a click; a
/// silent send cannot be taken back.
pub fn verdict_for(tool_name: &str) -> Verdict {
    // Juno's own server: the computer tool is in `--allowedTools`, but a
    // call that lands here anyway is Juno driving its own desktop.
    if tool_name.starts_with("mcp__juno__") {
        return Verdict::Allow;
    }

    if let Some(rest) = tool_name.strip_prefix("mcp__") {
        // `mcp__<server>__<tool>` — split off the server, judge the tool name.
        if let Some((_server, tool)) = rest.split_once("__") {
            let lowered: Vec<String> = tool
                .split(['_', '-'])
                .filter(|s| !s.is_empty())
                .map(str::to_lowercase)
                .collect();
            let has_write = lowered.iter().any(|s| WRITE_VERBS.contains(&s.as_str()));
            let has_read = lowered.iter().any(|s| READ_VERBS.contains(&s.as_str()));
            if has_read && !has_write {
                return Verdict::Allow;
            }
        }
        return Verdict::Prompt;
    }

    // A built-in that is not in `--allowedTools` reached the prompt path.
    Verdict::Prompt
}

/// When each tool was last denied, so a retry loop cannot nag.
static RECENT_DENIALS: LazyLock<Mutex<HashMap<String, Instant>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

fn recently_denied(tool_name: &str, now: Instant) -> bool {
    let Ok(denials) = RECENT_DENIALS.lock() else {
        return false;
    };
    denials
        .get(tool_name)
        .is_some_and(|at| now.duration_since(*at) < DENY_COOLDOWN)
}

fn record_denial(tool_name: &str) {
    if let Ok(mut denials) = RECENT_DENIALS.lock() {
        denials.insert(tool_name.to_string(), Instant::now());
    }
}

fn clear_denial(tool_name: &str) {
    if let Ok(mut denials) = RECENT_DENIALS.lock() {
        denials.remove(tool_name);
    }
}

#[cfg(test)]
fn clear_denials_for_test() {
    if let Ok(mut denials) = RECENT_DENIALS.lock() {
        denials.clear();
    }
}

/// The CLI's permission-prompt payload: `{"behavior":"allow",...}` or
/// `{"behavior":"deny",...}`, JSON-stringified into the MCP text content.
fn allow_result(input: &Value) -> Value {
    prompt_tool_result(&json!({ "behavior": "allow", "updatedInput": input }))
}

fn deny_result(message: &str) -> Value {
    prompt_tool_result(&json!({ "behavior": "deny", "message": message }))
}

fn prompt_tool_result(payload: &Value) -> Value {
    let text = serde_json::to_string(payload).unwrap_or_else(|_| payload.to_string());
    json!({
        "content": [{ "type": "text", "text": text }],
        "isError": false
    })
}

/// A one-line, human description of what is about to go out: which
/// connector, who it is going to, the subject or first line.
fn describe(tool_name: &str, input: &Value) -> String {
    let pick = |keys: &[&str]| -> Option<String> {
        keys.iter().find_map(|k| {
            input.get(*k).and_then(|v| match v {
                Value::String(s) if !s.is_empty() => Some(s.clone()),
                Value::Array(a) => Some(
                    a.iter()
                        .filter_map(Value::as_str)
                        .collect::<Vec<_>>()
                        .join(", "),
                ),
                _ => None,
            })
        })
    };

    let recipient = pick(&[
        "to",
        "recipient",
        "recipients",
        "channel",
        "channel_id",
        "email",
        "address",
        "user",
        "user_id",
        "username",
        "phone",
    ]);
    let subject = pick(&["subject", "title", "name"]);
    let body = pick(&["text", "body", "message", "content", "description", "query"]);

    let action = tool_name.strip_prefix("mcp__").unwrap_or(tool_name);
    let mut line = action.replace("__", ": ").replace('_', " ");
    if let Some(recipient) = recipient {
        line.push_str(&format!(" — to {recipient}"));
    }
    if let Some(subject) = subject {
        line.push_str(&format!(" — {subject}"));
    }
    if let Some(body) = body {
        let first: String = body.lines().next().unwrap_or("").chars().take(80).collect();
        if !first.is_empty() {
            line.push_str(&format!(" — “{first}”"));
        }
    }
    line
}

/// Handle a `tools/call` for `approve`: decide, maybe show the sheet, wait,
/// and answer in the CLI's permission-prompt shape.
pub async fn handle_approve(app: &tauri::AppHandle, arguments: &Value) -> Value {
    let tool_name = arguments
        .get("tool_name")
        .and_then(Value::as_str)
        .unwrap_or("");
    let input = arguments.get("input").cloned().unwrap_or_else(|| json!({}));

    if tool_name.is_empty() {
        return deny_result("Malformed permission request: no tool_name.");
    }

    // Off means off: the old behaviour, everything runs.
    if !is_enabled(app) {
        return allow_result(&input);
    }

    if verdict_for(tool_name) == Verdict::Allow {
        return allow_result(&input);
    }

    // A tool loop retrying a fresh denial gets the same answer without a
    // new sheet (same idea as permission_gate's ask rate limit).
    if recently_denied(tool_name, Instant::now()) {
        return deny_result(
            "This was declined moments ago and was not sent. Do not retry; \
             tell the person it was not sent, and that they can ask again if they change their mind.",
        );
    }

    let description = describe(tool_name, &input);
    let request_id = uuid::Uuid::new_v4().to_string();
    let request = ToolApprovalRequest::new(
        request_id.clone(),
        tool_name.to_string(),
        input.clone(),
        description.clone(),
    )
    .with_risk(RiskLevel::High)
    .with_timeout(APPROVAL_TIMEOUT_SECS);

    let app_state = app.state::<AppState>();
    app_state.add_pending_tool_approval(request.clone()).await;

    let approval_event = json!({
        "tool_name": request.tool_name,
        "tool_id": request.tool_id,
        "tool_input": request.tool_input,
        "description": request.description,
        "timestamp": request.timestamp,
        "risk_level": request.risk_level,
        "target_app": request.target_app,
        "timeout_seconds": request.timeout_seconds,
        "is_batch": false,
        "batch_size": 1
    });
    if let Err(e) = app.emit(events::tools::APPROVAL_REQUEST, approval_event) {
        warn!("[CliApproval] Failed to emit approval request: {}", e);
    }
    info!("[CliApproval] Waiting for approval: {}", description);

    // On voice, say what is being asked; the sheet stays on screen as the
    // answer surface either way. `invoke_tts` already no-ops when the TTS
    // provider is off or something else is speaking.
    {
        let app = app.clone();
        let spoken = format!("{description}. Allow?");
        tauri::async_runtime::spawn(async move {
            let state = app.state::<AppState>();
            let _ = crate::tts::invoke_tts(spoken, state, app.clone()).await;
        });
    }

    // Poll at 50 ms, the same cadence as the in-process approval wait.
    let mut remaining = (APPROVAL_TIMEOUT_SECS * 1000 / 50) as i64;
    let mut decision: Option<bool> = None;
    while remaining > 0 {
        if let Some(answer) = app_state.get_tool_approval_status(&request_id).await {
            decision = Some(answer);
            break;
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
        remaining -= 1;
    }
    app_state.remove_tool_approval(&request_id).await;

    match decision {
        Some(true) => {
            clear_denial(tool_name);
            info!("[CliApproval] Approved: {}", description);
            allow_result(&input)
        }
        Some(false) => {
            record_denial(tool_name);
            info!("[CliApproval] Declined: {}", description);
            deny_result(
                "The person declined, so it was not sent. Tell them it was not sent \
                 and move on; they can ask again if they change their mind.",
            )
        }
        None => {
            record_denial(tool_name);
            info!(
                "[CliApproval] No answer in {}s: {}",
                APPROVAL_TIMEOUT_SECS, description
            );
            deny_result(
                "No answer within 60 seconds, so nothing was sent. Tell the person their \
                 approval was needed and no answer arrived; they can ask again.",
            )
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // The pin the ticket asks for (juno-dead-control-pattern): with the
    // setting on and the MCP server present, the prompt tool is wired in and
    // skip-permissions is gone. `claude_cli::tests` pins the same through
    // `build_args`; this covers the seam both spawn paths share.
    #[test]
    fn asking_replaces_skip_permissions() {
        let args = permission_args(true, true);
        assert!(args.contains(&"--permission-prompt-tool".to_string()));
        assert!(args.contains(&APPROVE_TOOL_CLI_NAME.to_string()));
        assert!(!args.contains(&"--dangerously-skip-permissions".to_string()));
        assert!(args.contains(&"--allowedTools".to_string()));
    }

    #[test]
    fn setting_off_restores_skip_permissions() {
        let args = permission_args(false, true);
        assert_eq!(args, vec!["--dangerously-skip-permissions".to_string()]);
        assert!(!args.contains(&"--permission-prompt-tool".to_string()));
    }

    #[test]
    fn no_mcp_server_means_no_prompt_tool_to_serve() {
        // Without Juno's server there is nothing to answer the prompt, so
        // pointing the CLI at it would hang every gated call.
        let args = permission_args(true, false);
        assert_eq!(args, vec!["--dangerously-skip-permissions".to_string()]);
    }

    #[test]
    fn read_only_connector_calls_run_without_asking() {
        for name in [
            "mcp__gmail__search_messages",
            "mcp__slack__slack_read_channel",
            "mcp__gdrive__get_file",
            "mcp__calendar__list_events",
            "mcp__github__fetch-issue",
        ] {
            assert_eq!(verdict_for(name), Verdict::Allow, "{name} should run");
        }
    }

    #[test]
    fn connector_writes_prompt() {
        for name in [
            "mcp__gmail__send_email",
            "mcp__slack__slack_send_message",
            "mcp__docs__create_document",
            "mcp__calendar__delete_event",
            "mcp__gmail__create_draft",
        ] {
            assert_eq!(verdict_for(name), Verdict::Prompt, "{name} should ask");
        }
    }

    #[test]
    fn a_write_verb_beats_a_read_verb() {
        // "read" appears in the name, but the call changes state.
        assert_eq!(verdict_for("mcp__gmail__mark_as_read"), Verdict::Prompt);
        // "get" + "put" — the write wins.
        assert_eq!(verdict_for("mcp__x__get_and_put_thing"), Verdict::Prompt);
    }

    #[test]
    fn unknown_names_prompt_rather_than_run() {
        assert_eq!(verdict_for("mcp__mystery__frobnicate"), Verdict::Prompt);
        assert_eq!(verdict_for("SomeBuiltIn"), Verdict::Prompt);
    }

    #[test]
    fn junos_own_tools_never_prompt() {
        assert_eq!(verdict_for("mcp__juno__computer"), Verdict::Allow);
    }

    #[test]
    fn results_carry_the_clis_permission_shape_as_text() {
        let allow = allow_result(&json!({ "to": "cameron@example.com" }));
        let text = allow["content"][0]["text"].as_str().expect("text content");
        let parsed: Value = serde_json::from_str(text).expect("stringified JSON");
        assert_eq!(parsed["behavior"], "allow");
        assert_eq!(parsed["updatedInput"]["to"], "cameron@example.com");

        let deny = deny_result("because");
        let text = deny["content"][0]["text"].as_str().expect("text content");
        let parsed: Value = serde_json::from_str(text).expect("stringified JSON");
        assert_eq!(parsed["behavior"], "deny");
        assert_eq!(parsed["message"], "because");
    }

    #[test]
    fn a_fresh_denial_is_not_asked_again_immediately() {
        clear_denials_for_test();
        let now = Instant::now();
        assert!(!recently_denied("mcp__gmail__send_email", now));
        record_denial("mcp__gmail__send_email");
        assert!(recently_denied("mcp__gmail__send_email", Instant::now()));
        // A different tool is its own question.
        assert!(!recently_denied(
            "mcp__slack__slack_send_message",
            Instant::now()
        ));
        // Approval clears the cooldown so the next ask is a real ask.
        clear_denial("mcp__gmail__send_email");
        assert!(!recently_denied("mcp__gmail__send_email", Instant::now()));
        clear_denials_for_test();
    }

    #[test]
    fn the_description_names_recipient_and_first_line() {
        let line = describe(
            "mcp__slack__slack_send_message",
            &json!({ "channel": "#dev", "text": "site is up\nsecond line" }),
        );
        assert!(line.contains("slack"), "{line}");
        assert!(line.contains("#dev"), "{line}");
        assert!(line.contains("site is up"), "{line}");
        assert!(!line.contains("second line"), "{line}");
    }
}

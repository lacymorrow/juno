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
/// round-trips through the prompt either, and so is `settings`, whose
/// guardrail settings are refused by construction
/// (`settings::registry::authorize_set`).
pub const ALLOWED_TOOLS: &str = "Bash,Read,Edit,Write,Glob,Grep,WebFetch,WebSearch,NotebookEdit,\
     TodoWrite,Task,mcp__juno__computer,mcp__juno__settings";

/// How long the sheet waits for an answer before denying.
pub const APPROVAL_TIMEOUT_SECS: u64 = 60;

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

/// Acts that leave this machine. Sending, publishing, sharing and inviting are
/// one consequence under four verbs: data reaches a person who could not see it
/// before, and it cannot be recalled.
///
/// Lacy's rule: "it should not send communications out without approval but it
/// should draft emails freely."
const SEND_VERBS: &[&str] = &[
    "send",
    "post",
    "reply",
    "forward",
    "publish",
    "share",
    "invite",
    "submit",
    "broadcast",
];

/// Acts that cost money. Irrecoverable in the way that matters most to the
/// person paying.
const SPEND_VERBS: &[&str] = &[
    "pay",
    "purchase",
    "buy",
    "checkout",
    "charge",
    "subscribe",
    "transfer",
    "refund",
    "payout",
];

/// Emptying the Trash is the one act that defeats the construction every other
/// delete now relies on: it turns every recoverable delete Juno made into a
/// permanent one, retroactively and in bulk. Nobody asking Juno to tidy a
/// folder is asking for that.
///
/// It costs almost nothing to gate, because nobody asks Juno to empty the
/// Trash in the course of ordinary work.
const EMPTY_VERBS: &[&str] = &["empty", "purge"];

/// Acts that destroy something the Trash cannot catch: a remote record, a
/// cloud file, a calendar entry, a database row.
///
/// **These are held, not settled.** The directive names three gated acts:
/// sending, spending, and emptying the Trash. A remote delete is none of them,
/// so following the directive literally would let it through.
///
/// It is kept gated because the standing rule across this whole workstream is
/// that a prompt is only removed once the construction that makes the act safe
/// exists. `rm` stopped asking because deletes go to the Trash. Nothing makes a
/// deleted calendar event come back, and Juno has no remote undo to build on,
/// so there is no construction here to remove the prompt in favour of.
///
/// This is deliberately not a silent addition: it is the one protection in this
/// change that the directive did not ask for, and removing it is a one-line
/// edit to this constant if that is the call. See
/// `docs/plans/permissions-by-consequence.md`.
const DESTROY_REMOTE_VERBS: &[&str] = &["delete", "destroy", "revoke", "wipe"];

/// Whether a connector tool name reads as one of the gated acts.
///
/// Matching is on exact tokens, which is what keeps the singular verb forms
/// from colliding with plural nouns: `list_transfers` and `get_posts` carry
/// `transfers` and `posts`, not `transfer` and `post`, so a listing is not
/// mistaken for a payment or a publish.
///
/// A gated verb wins outright. There is no read-verb override, because the safe
/// direction is now the opposite of what it used to be: under a permissive
/// default the thing to be careful about is the small gated set, not everything
/// else, so `search_and_send` asks.
fn names_a_gated_act(tool: &str) -> bool {
    tool.split(['_', '-'])
        .filter(|segment| !segment.is_empty())
        .map(str::to_lowercase)
        .any(|segment| {
            let segment = segment.as_str();
            SEND_VERBS.contains(&segment)
                || SPEND_VERBS.contains(&segment)
                || EMPTY_VERBS.contains(&segment)
                || DESTROY_REMOTE_VERBS.contains(&segment)
        })
}

/// Decide whether a call the CLI could not resolve on its own runs or asks.
///
/// **The default allows.** Lacy, 2026-10-01: "I prefer to start by allowing
/// Juno to do everything and then we can rein in permissions as users privacy
/// concerns arise." A computer-use app that interrupts is one nobody keeps, and
/// the person this default is for will not know how to answer a prompt, so
/// every default that asks is a default that stops her.
///
/// This used to be the other way round: a tool whose name carried any write
/// verb prompted, and so did anything unrecognised. That asked about
/// `create_document`, `update_event`, `mark_as_read`, `move_message`,
/// `archive_thread` and `add_label`, none of which leaves the machine and all
/// of which a person can undo.
///
/// Now a connector call runs unless its name reads as one of the gated acts,
/// and the conservative direction moved with it: an unrecognised name runs,
/// while a name that reads as a send, a spend or an empty asks even when
/// nothing else about it is recognised.
///
/// This is name matching, and name matching is the degraded path. It is here
/// because a third-party connector's parameters are not a contract Juno can
/// read, so the tool name is the only structural signal available. Where Juno
/// owns the tool, the gate belongs on the call's parameters instead. The known
/// gap is a spend whose verb Juno does not carry, such as `place_order`; the
/// fix for that is a structural spend signal, not a longer verb list.
pub fn verdict_for(tool_name: &str) -> Verdict {
    // Juno's own server: the computer tool is in `--allowedTools`, but a
    // call that lands here anyway is Juno driving its own desktop. The Mac app
    // tools are not waved through by this: `juno_mcp` runs Juno's own
    // permission gate on them before they execute.
    if tool_name.starts_with("mcp__juno__") {
        return Verdict::Allow;
    }

    if let Some(rest) = tool_name.strip_prefix("mcp__") {
        // `mcp__<server>__<tool>` — split off the server, judge the tool name.
        // A name that does not split is judged whole rather than waved
        // through: `mcp__weird` gets the same reading as a tool name.
        let tool = rest
            .split_once("__")
            .map(|(_server, tool)| tool)
            .unwrap_or(rest);
        if names_a_gated_act(tool) {
            return Verdict::Prompt;
        }
        return Verdict::Allow;
    }

    // A built-in that is not in `--allowedTools`. The CLI's own toolset is
    // local work: Bash, Read, Edit, Write, Glob, Grep, WebFetch, WebSearch,
    // NotebookEdit, TodoWrite, Task. None of it sends and none of it spends,
    // so one arriving here runs. `the_clis_builtins_are_all_local_work` pins
    // that assumption to the list, so a future built-in that sends shows up as
    // a failing test rather than as a silent send.
    Verdict::Allow
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

/// Show the approval sheet for one request, wait, and return the answer.
///
/// `Some(true)` approved, `Some(false)` declined, `None` no answer before the
/// request's timeout. This is the one sheet both CLI gates use: `approve`
/// (the CLI's permission prompt) and the gate in front of Juno's own Mac app
/// tools on the MCP server (`juno_mcp`), which the CLI never prompts for.
pub async fn ask_person(app: &tauri::AppHandle, request: ToolApprovalRequest) -> Option<bool> {
    let request_id = request.tool_id.clone();
    let description = request.description.clone();
    let timeout_secs = request.timeout_seconds;

    let app_state = app.state::<AppState>();
    app_state.add_pending_tool_approval(request.clone()).await;

    let mut approval_event = json!({
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
    // A send is drawn as the message card and answered only by "send it".
    if let (Some(message), Some(map)) = (&request.message, approval_event.as_object_mut()) {
        map.insert("message".to_string(), message.clone());
        map.insert("consequence".to_string(), json!("send"));
        map.insert("always_allow_label".to_string(), Value::Null);
    }
    if let Err(e) = app.emit(events::tools::APPROVAL_REQUEST, approval_event) {
        warn!("[CliApproval] Failed to emit approval request: {}", e);
    }
    info!("[CliApproval] Waiting for approval: {}", description);

    // On voice, say what is being asked; the sheet stays on screen as the
    // answer surface either way. `invoke_tts` already no-ops when the TTS
    // provider is off or something else is speaking.
    {
        let app = app.clone();
        // A send says the message and the phrase; anything else asks Allow.
        let spoken = request
            .message
            .as_ref()
            .and_then(|m| m.get("prompt"))
            .and_then(Value::as_str)
            .map(str::to_string)
            .unwrap_or_else(|| format!("{description}. Allow?"));
        tauri::async_runtime::spawn(async move {
            let state = app.state::<AppState>();
            let _ = crate::tts::invoke_tts(spoken, state, app.clone()).await;
        });
    }

    // Poll at 50 ms, the same cadence as the in-process approval wait.
    let mut remaining = (timeout_secs * 1000 / 50) as i64;
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
    decision
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
    let request = ToolApprovalRequest::new(
        uuid::Uuid::new_v4().to_string(),
        tool_name.to_string(),
        input.clone(),
        description.clone(),
    )
    .with_risk(RiskLevel::High)
    .with_timeout(APPROVAL_TIMEOUT_SECS);

    let decision = ask_person(app, request).await;

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

/// Classify a short spoken reply to a pending approval prompt as yes/no
/// (LAC-4066). The sheet also speaks "... Allow?" on voice, so a person
/// answers out loud; this turns that answer into approve/deny.
///
/// Deliberately strict, and it errs toward not sending: a wrong "yes" sends a
/// real message. It only matches tiny, unambiguous affirm/deny utterances, so
/// ordinary talk near an open approval sheet does not silently send or cancel.
/// Anything longer than a few words, or with no clear affirm/deny word, returns
/// `None` and falls through to normal voice handling.
///
/// - **Deny wins and stays broad** — "no, don't allow" is a no, because a false
///   "no" only costs a click.
/// - **Yes must be clean and unhedged** — a trailing/embedded `?`, or any hedge
///   ("not", "maybe", "probably", "i think", "wait", "hold on", "hmm", ...)
///   makes an affirm fall through to `None`, so "not sure", "yes?", "hmm yes"
///   and "probably yes" never approve. The sheet and the 60 s timeout decide.
pub fn parse_spoken_approval(text: &str) -> Option<bool> {
    // A question is never a clean answer, and the normalisation below folds '?'
    // to a space (turning "yes?" into "yes"), so catch it in the raw text
    // first: any question mark makes this a non-answer that falls through to the
    // sheet and the 60 s timeout (LAC-4066).
    if text.contains('?') {
        return None;
    }

    // Fold punctuation to spaces, keep apostrophes so "don't" stays one word.
    let normalized: String = text
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
    let words: Vec<&str> = normalized.split_whitespace().collect();

    // A yes/no answer is a few words at most. Longer than this is a sentence,
    // i.e. almost certainly not an answer to the sheet, so let it fall through.
    if words.is_empty() || words.len() > 6 {
        return None;
    }
    let joined = words.join(" ");

    // Multi-word entries are matched as substrings of the whole utterance;
    // single words must appear as a standalone token so "no" does not fire on
    // "nobody" and "ok" does not fire on "okra".
    const DENY: &[&str] = &[
        "no",
        "nope",
        "nah",
        "don't",
        "dont",
        "do not",
        "deny",
        "decline",
        "declined",
        "cancel",
        "reject",
        "negative",
        "no thanks",
        "never mind",
        "nevermind",
    ];
    const AFFIRM: &[&str] = &[
        "yes",
        "yeah",
        "yep",
        "yup",
        "sure",
        "ok",
        "okay",
        "allow",
        "allowed",
        "approve",
        "approved",
        "confirm",
        "confirmed",
        "affirmative",
        "go ahead",
        "send it",
        "do it",
        "please do",
        "sounds good",
    ];

    let matches = |phrases: &[&str]| -> bool {
        phrases.iter().any(|phrase| {
            if phrase.contains(' ') {
                joined.contains(phrase)
            } else {
                words.contains(phrase)
            }
        })
    };

    if matches(DENY) {
        return Some(false);
    }

    // A hedge or negation turns an affirm into a non-answer. A wrong "yes"
    // sends a real message, so yes needs a clean, unhedged affirm: any of these
    // words or phrases makes the utterance fall through instead of approving
    // ("not sure", "hmm yes", "probably yes", "yes but wait", "hold on, ok",
    // "i think so"). The sheet stays up and the 60 s timeout denies (LAC-4066).
    // "don't"/"do not" are denials, handled above; "dunno" is the hedge
    // spelling that carries no deny word. Multi-word hedges match as substrings
    // and single words as standalone tokens, same as DENY/AFFIRM.
    const HEDGES: &[&str] = &[
        "not",
        "maybe",
        "perhaps",
        "unsure",
        "probably",
        "possibly",
        "wait",
        "hmm",
        "hm",
        "um",
        "uh",
        "er",
        "dunno",
        "what",
        "why",
        "who",
        "which",
        "how",
        "but",
        "i guess",
        "i think",
        "i suppose",
        "hold on",
        "hang on",
        "one sec",
        "let me",
    ];
    if matches(HEDGES) {
        return None;
    }

    if matches(AFFIRM) {
        Some(true)
    } else {
        None
    }
}

/// Resolve a pending per-send approval (LAC-4058) from a short spoken or typed
/// reply (LAC-4066). Returns `true` when a clear yes/no answered one or more
/// pending approvals, so the caller stops instead of treating the utterance as
/// a new query. A no-op (returns `false`) when nothing is pending or the text
/// is not a clear affirm/deny, so it is safe to call on every voice/query
/// entry point.
pub async fn try_answer_pending_approval(app_state: &AppState, text: &str) -> bool {
    !matches!(
        answer_pending_approval(app_state, text, AnswerSource::Ambient).await,
        PendingAnswer::NotAnAnswer
    )
}

/// Where a reply came from, which decides whether it can be a correction.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AnswerSource {
    /// A query the person deliberately spoke or typed (`submit_query`). While
    /// a send waits, anything that is not "send it" or a no is a correction.
    Deliberate,
    /// Always-listening speech. Only "send it" or a clear no touch a waiting
    /// send; other talk in the room never cancels it.
    Ambient,
}

/// What a reply did to the pending approvals.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PendingAnswer {
    /// Not an answer to anything; carry on with the query.
    NotAnAnswer,
    /// It answered (approved, denied, or changed a waiting send).
    Answered,
    /// A clean yes to a waiting send. Nothing was sent; say this line.
    SayThePhrase(String),
}

/// Answer pending approvals from a spoken or typed reply.
///
/// A waiting send (a text or an email) is answered first and on its own
/// terms (`mac_apps::send::classify_reply`): "send it" (or "send") approves
/// it, the same as the Send button; a clear no denies it; a bare "yes" leaves
/// it waiting and asks for the phrase; anything else said deliberately is a
/// correction, recorded for the waiting gate, which then hands the words to
/// the agent to redo the message. Every other approval keeps the yes/no rules
/// below, and a bare "yes" never reaches a send through them.
pub async fn answer_pending_approval(
    app_state: &AppState,
    text: &str,
    source: AnswerSource,
) -> PendingAnswer {
    use crate::agent::tools::mac_apps::send::{self, SendReply};

    let pending = app_state.get_pending_tool_approvals().await;
    if pending.is_empty() {
        return PendingAnswer::NotAnAnswer;
    }
    let (sends, others): (Vec<_>, Vec<_>) = pending.into_iter().partition(|r| r.is_send());

    if !sends.is_empty() {
        let Some(reply) = send::classify_reply(text, parse_spoken_approval) else {
            return PendingAnswer::NotAnAnswer;
        };
        match reply {
            SendReply::Send => {
                for request in &sends {
                    app_state.approve_tool(&request.tool_id).await;
                }
                info!(
                    "[CliApproval] '{}' sent {} waiting message(s)",
                    text,
                    sends.len()
                );
                return PendingAnswer::Answered;
            }
            SendReply::Cancel => {
                for request in &sends {
                    app_state.deny_tool(&request.tool_id).await;
                }
                return PendingAnswer::Answered;
            }
            SendReply::SayThePhrase => {
                return PendingAnswer::SayThePhrase(send::SAY_SEND_IT.to_string());
            }
            SendReply::Correction(words) => {
                if source == AnswerSource::Ambient {
                    return PendingAnswer::NotAnAnswer;
                }
                for request in &sends {
                    send::record_correction(&request.tool_id, &words);
                    app_state.deny_tool(&request.tool_id).await;
                }
                info!("[CliApproval] '{}' changed a waiting message", text);
                return PendingAnswer::Answered;
            }
        }
    }

    let decision = match parse_spoken_approval(text) {
        Some(decision) => decision,
        None => return PendingAnswer::NotAnAnswer,
    };
    let pending = others;
    if pending.is_empty() {
        return PendingAnswer::NotAnAnswer;
    }

    // There is at most one CLI approval waiting at a time (handle_approve
    // blocks on its own request), but answer every pending request the same
    // way so a spoken yes/no is never applied to only some of them.
    for request in &pending {
        if decision {
            app_state.approve_tool(&request.tool_id).await;
        } else {
            app_state.deny_tool(&request.tool_id).await;
        }
    }

    info!(
        "[CliApproval] Voice/typed '{}' answered {} pending approval(s) as {}",
        text,
        pending.len(),
        if decision { "allow" } else { "deny" }
    );
    PendingAnswer::Answered
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

    /// Was `connector_writes_prompt`, which asserted that any connector write
    /// asks. Under the permissive default that is the wrong question: a write
    /// is not a consequence, it is a verb tense. Only three consequences ask.
    ///
    /// `mcp__docs__create_document` moved out of this list and into
    /// `local_connector_writes_no_longer_interrupt` below. Creating a document
    /// does not leave the machine and the person can delete it.
    #[test]
    fn sending_and_spending_ask() {
        for name in [
            // Sending, in its four verbs.
            "mcp__gmail__send_email",
            "mcp__slack__slack_send_message",
            "mcp__blog__publish_article",
            "mcp__gdrive__share_file",
            "mcp__github__invite_collaborator",
            "mcp__forms__submit_response",
            "mcp__slack__post_to_channel",
            "mcp__gmail__forward_thread",
            "mcp__x__broadcast_update",
            // Spending.
            "mcp__stripe__charge_card",
            "mcp__shop__buy_item",
            "mcp__shop__checkout_cart",
            "mcp__bank__transfer_funds",
            "mcp__billing__pay_invoice",
            "mcp__plans__subscribe_to_plan",
            "mcp__stripe__refund_payment",
            "mcp__stripe__create_payout",
            "mcp__shop__purchase_license",
        ] {
            assert_eq!(
                verdict_for(name),
                Verdict::Prompt,
                "{name} leaves the machine or costs money"
            );
        }
    }

    /// Emptying the Trash is gated because it defeats the construction every
    /// other delete relies on: it turns every recoverable delete Juno made
    /// into a permanent one, retroactively. See [`EMPTY_VERBS`].
    #[test]
    fn emptying_the_trash_asks() {
        for name in ["mcp__files__empty_trash", "mcp__mail__purge_deleted_items"] {
            assert_eq!(verdict_for(name), Verdict::Prompt, "{name} should ask");
        }
    }

    /// The point of the whole change. Each of these asked before and does not
    /// now, and for each one the reason is the same: it stays on this machine
    /// and the person can undo it.
    #[test]
    fn local_connector_writes_no_longer_interrupt() {
        for name in [
            "mcp__docs__create_document",
            "mcp__calendar__update_event",
            "mcp__gmail__mark_as_read",
            "mcp__gmail__move_message",
            "mcp__gmail__archive_thread",
            "mcp__gmail__add_label",
            "mcp__gmail__create_draft",
            "mcp__notion__set_property",
            "mcp__notion__insert_block",
            "mcp__jira__assign_issue",
            "mcp__github__close_issue",
            "mcp__github__merge_pull_request",
            "mcp__slack__react_to_message",
            "mcp__slack__pin_message",
            "mcp__calendar__cancel_event",
            "mcp__calendar__schedule_meeting",
            "mcp__gdrive__upload_file",
            "mcp__x__patch_record",
            "mcp__x__put_record",
            "mcp__x__edit_thing",
            "mcp__x__write_thing",
            "mcp__x__trigger_build",
        ] {
            assert_eq!(
                verdict_for(name),
                Verdict::Allow,
                "{name} is local and undoable, so it must not interrupt anyone"
            );
        }
    }

    /// `mcp__gmail__create_draft` used to assert that drafting asks. That was
    /// the wrong test. Drafting stays on this machine and can be thrown away;
    /// sending is what cannot be recalled, and the gate belongs on the
    /// consequence rather than on the word "create". Lacy: "it should not send
    /// communications out without approval but it should draft emails freely."
    /// Fleet rules 12 and 13 already make draft-only the sanctioned path for
    /// mail.
    #[test]
    fn drafting_runs_but_sending_asks() {
        for name in [
            "mcp__gmail__create_draft",
            "mcp__gmail__update_draft",
            "mcp__slack__create_draft_message",
        ] {
            assert_eq!(
                verdict_for(name),
                Verdict::Allow,
                "{name} only writes a draft, so it should not interrupt anyone"
            );
        }

        for name in [
            "mcp__gmail__send_draft",
            "mcp__gmail__forward_draft",
            // Ambiguous on purpose: a name carrying `reply` is far more often
            // posting one than composing one, so it fails closed.
            "mcp__gmail__draft_reply",
            "mcp__blog__publish_draft",
            "mcp__forms__submit_draft",
            "mcp__gmail__send_email",
        ] {
            assert_eq!(
                verdict_for(name),
                Verdict::Prompt,
                "{name} leaves the machine, draft or not"
            );
        }
    }

    /// The one protection in this change the directive did not ask for.
    ///
    /// The gated set is sending, spending and emptying the Trash. A remote
    /// delete is none of those, so a literal reading would let it through. It
    /// stays gated because the rule across this workstream is that a prompt is
    /// removed only once the construction that makes the act safe exists.
    /// `rm` stopped asking because deletes go to the Trash; nothing brings a
    /// deleted calendar event back.
    ///
    /// Deleting this test and [`DESTROY_REMOTE_VERBS`] is the one-line change
    /// if the call goes the other way. It is a test rather than a comment so
    /// that going the other way is a deliberate act.
    #[test]
    fn a_remote_delete_still_asks_because_nothing_undoes_it() {
        for name in [
            "mcp__calendar__delete_event",
            "mcp__gdrive__delete_file",
            "mcp__gmail__delete_draft",
            "mcp__db__destroy_record",
            "mcp__auth__revoke_token",
        ] {
            assert_eq!(
                verdict_for(name),
                Verdict::Prompt,
                "{name} cannot be undone and Juno has no remote undo to offer"
            );
        }
    }

    /// Replaces `unknown_names_prompt_rather_than_run`, whose name is the old
    /// posture. The conservative direction moved: under a permissive default
    /// the thing to be careful about is the small gated set, not everything
    /// else. An unrecognised connector call runs.
    ///
    /// The care did not disappear, it moved. The second half of this test is
    /// where it went: a name Juno cannot otherwise read still asks the moment
    /// it carries a gated verb.
    #[test]
    fn unknown_names_run_rather_than_prompt() {
        assert_eq!(verdict_for("mcp__mystery__frobnicate"), Verdict::Allow);
        assert_eq!(verdict_for("mcp__weird"), Verdict::Allow);

        // Unrecognised in every respect except the part that matters.
        assert_eq!(
            verdict_for("mcp__mystery__frobnicate_and_send"),
            Verdict::Prompt
        );
        assert_eq!(
            verdict_for("mcp__mystery__pay_the_frobnicator"),
            Verdict::Prompt
        );
    }

    /// A gated verb wins outright, with no read-verb override. The old rule
    /// was "a write verb beats a read verb"; the new one is narrower and
    /// sharper, so `search_and_send` asks even though it searches.
    #[test]
    fn a_gated_verb_beats_a_read_verb() {
        assert_eq!(verdict_for("mcp__x__search_and_send"), Verdict::Prompt);
        assert_eq!(verdict_for("mcp__x__get_and_pay"), Verdict::Prompt);
        // And a read that merely sounds like one does not ask.
        assert_eq!(verdict_for("mcp__gmail__get_message"), Verdict::Allow);
    }

    /// Exact-token matching is what keeps plural nouns from reading as verbs.
    /// `list_transfers` carries `transfers`, not `transfer`, so a listing is
    /// not mistaken for a payment. This is the property that lets the verb
    /// lists stay short.
    #[test]
    fn plural_nouns_do_not_read_as_verbs() {
        for name in [
            "mcp__bank__list_transfers",
            "mcp__blog__get_posts",
            "mcp__plans__list_subscriptions",
            "mcp__gdrive__list_shared_files",
            "mcp__shop__get_purchases",
            "mcp__stripe__list_charges",
        ] {
            assert_eq!(
                verdict_for(name),
                Verdict::Allow,
                "{name} reads things, it does not do them"
            );
        }
    }

    /// The assumption behind allowing an unlisted built-in: every tool the CLI
    /// brings of its own is local work. If that stops being true, this fails
    /// here rather than sending something silently.
    #[test]
    fn the_clis_builtins_are_all_local_work() {
        for builtin in ALLOWED_TOOLS.split(',') {
            assert!(
                !names_a_gated_act(builtin),
                "{builtin} reads as a gated act but is pre-approved in                  ALLOWED_TOOLS; either it does not belong there or                  verdict_for must stop allowing unlisted built-ins"
            );
        }
        assert_eq!(verdict_for("SomeBuiltIn"), Verdict::Allow);
    }

    #[test]
    fn junos_own_tools_never_prompt() {
        assert_eq!(verdict_for("mcp__juno__computer"), Verdict::Allow);
        assert_eq!(verdict_for("mcp__juno__settings"), Verdict::Allow);
        assert!(ALLOWED_TOOLS.split(',').any(|t| t == "mcp__juno__settings"));
        // Juno's own send tools are asked about once, by `juno_mcp::run_gated`
        // with the message card, not a second time by the CLI's prompt.
        assert_eq!(verdict_for("mcp__juno__messages_send"), Verdict::Allow);
        assert_eq!(verdict_for("mcp__juno__mail_send"), Verdict::Allow);
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
    fn a_spoken_yes_approves_and_a_spoken_no_denies() {
        for yes in [
            "yes",
            "Yes.",
            "yeah",
            "sure",
            "ok",
            "okay",
            "allow",
            "approve it",
            "go ahead",
            "send it",
            "do it",
        ] {
            assert_eq!(parse_spoken_approval(yes), Some(true), "{yes} should allow");
        }
        for no in [
            "no",
            "No.",
            "nope",
            "nah",
            "deny",
            "cancel",
            "don't",
            "do not send it",
            "no thanks",
            "never mind",
        ] {
            assert_eq!(parse_spoken_approval(no), Some(false), "{no} should deny");
        }
    }

    #[test]
    fn deny_wins_when_both_words_appear() {
        // "no, don't allow" carries both an affirm and a deny word; it is a no.
        assert_eq!(parse_spoken_approval("no don't allow"), Some(false));
        assert_eq!(parse_spoken_approval("no, do not send it"), Some(false));
    }

    #[test]
    fn ambient_speech_falls_through() {
        // Not an answer: no clear affirm/deny word, or too long to be one.
        // These return None so the utterance goes to normal voice handling
        // instead of silently approving or cancelling the pending send.
        for ambient in [
            "what time is it",
            "email cameron about the launch",
            "nobody has replied yet",
            "that is okra",
            "not sure",
            "i'm not sure",
            "maybe",
            "maybe later",
            "yes i think we should rewrite the whole onboarding flow tomorrow",
            "",
        ] {
            assert_eq!(
                parse_spoken_approval(ambient),
                None,
                "'{ambient}' should fall through"
            );
        }
    }

    #[test]
    fn a_clean_affirm_approves_and_a_clear_deny_denies() {
        // Lacy's required sets on PR #641. Must approve: a clean, unhedged
        // affirm. Must deny: a clear no.
        for yes in ["yes", "yeah", "ok", "send it", "go ahead"] {
            assert_eq!(
                parse_spoken_approval(yes),
                Some(true),
                "'{yes}' must approve"
            );
        }
        for no in ["no", "nope", "don't"] {
            assert_eq!(parse_spoken_approval(no), Some(false), "'{no}' must deny");
        }
    }

    #[test]
    fn hedged_or_questioned_affirms_never_approve() {
        // A wrong "yes" sends a real message, so anything short of a clean,
        // unhedged affirm must fall through to `None`: the sheet stays up and
        // the 60 s timeout denies (LAC-4066). Every case carries an affirm word
        // or is a bare hedge; none may return `Some(true)`. This is the
        // required must-not-approve set from PR #641's review.
        for hedged in [
            "not sure",
            "i'm not sure",
            "maybe",
            "sure?",
            "yes?",
            "send it?",
            "i guess so",
            "i guess so, send it",
            "i think so",
            "yes but wait",
            "hmm yes",
            "probably",
            "probably yes",
            "hold on, ok",
            "wait",
            "let me think",
        ] {
            assert_eq!(
                parse_spoken_approval(hedged),
                None,
                "'{hedged}' must not approve"
            );
        }
    }

    #[tokio::test]
    async fn try_answer_resolves_a_pending_approval_by_voice() {
        // The hotkey / push-to-talk path funnels through
        // `anthropic::submit_query`, which calls `try_answer_pending_approval`;
        // the always-listening path calls it too. Both share this resolver, so
        // exercising it covers every voice entry point (LAC-4066).
        let state = AppState::new(None);
        let request = ToolApprovalRequest::new(
            "tool-1".to_string(),
            "mcp__slack__slack_send_message".to_string(),
            json!({ "channel": "#dev", "text": "site is up" }),
            "slack: send message".to_string(),
        );

        // A spoken "yes" approves the pending tool.
        state.add_pending_tool_approval(request.clone()).await;
        assert!(try_answer_pending_approval(&state, "yes").await);
        assert_eq!(state.get_tool_approval_status("tool-1").await, Some(true));

        // A spoken "no" denies it.
        state.clear_pending_tool_approvals().await;
        state.add_pending_tool_approval(request.clone()).await;
        assert!(try_answer_pending_approval(&state, "no").await);
        assert_eq!(state.get_tool_approval_status("tool-1").await, Some(false));

        // A hedge is not an answer: nothing is resolved, the request stays
        // pending for the sheet and the timeout to decide.
        state.clear_pending_tool_approvals().await;
        state.add_pending_tool_approval(request.clone()).await;
        assert!(!try_answer_pending_approval(&state, "not sure").await);
        assert_eq!(state.get_tool_approval_status("tool-1").await, None);

        // Nothing pending: even a clear "yes" is a no-op, so it falls through to
        // be handled as a normal query.
        state.clear_pending_tool_approvals().await;
        assert!(!try_answer_pending_approval(&state, "yes").await);
    }

    /// The voice path into a waiting send: "send it" approves the same pending
    /// approval the Send button does; a bare "yes" never does.
    #[tokio::test]
    async fn only_send_it_answers_a_waiting_text() {
        let state = AppState::new(None);
        let text = || {
            ToolApprovalRequest::new(
                "send-1".to_string(),
                "messages_send".to_string(),
                json!({ "to": "Doug", "body": "running late" }),
                "Text Doug Keesler: running late".to_string(),
            )
            .with_message(json!({ "to": "Doug Keesler", "body": "running late" }))
        };

        // A bare yes leaves it waiting and asks for the phrase.
        for yes in ["yes", "ok", "yeah", "yes send"] {
            state.clear_pending_tool_approvals().await;
            state.add_pending_tool_approval(text()).await;
            let answer = answer_pending_approval(&state, yes, AnswerSource::Deliberate).await;
            assert_eq!(
                answer,
                PendingAnswer::SayThePhrase("Say send it.".to_string()),
                "{yes}"
            );
            assert_eq!(
                state.get_tool_approval_status("send-1").await,
                None,
                "{yes}"
            );
        }

        // "send it" approves it.
        state.clear_pending_tool_approvals().await;
        state.add_pending_tool_approval(text()).await;
        assert!(try_answer_pending_approval(&state, "Send it.").await);
        assert_eq!(state.get_tool_approval_status("send-1").await, Some(true));

        // "don't send it" cancels; nothing is sent.
        state.clear_pending_tool_approvals().await;
        state.add_pending_tool_approval(text()).await;
        assert!(try_answer_pending_approval(&state, "don't send it").await);
        assert_eq!(state.get_tool_approval_status("send-1").await, Some(false));

        // A deliberate correction denies it and hands the words over.
        state.clear_pending_tool_approvals().await;
        state.add_pending_tool_approval(text()).await;
        let answer =
            answer_pending_approval(&state, "make it twenty minutes", AnswerSource::Deliberate)
                .await;
        assert_eq!(answer, PendingAnswer::Answered);
        assert_eq!(state.get_tool_approval_status("send-1").await, Some(false));
        assert_eq!(
            crate::agent::tools::mac_apps::send::take_correction("send-1").as_deref(),
            Some("make it twenty minutes")
        );

        // Talk in the room never touches it.
        state.clear_pending_tool_approvals().await;
        state.add_pending_tool_approval(text()).await;
        assert!(!try_answer_pending_approval(&state, "make it twenty minutes").await);
        // "send it later" is a clean affirm to the general parser, so it only
        // asks for the phrase again; the message has not gone.
        assert_eq!(
            answer_pending_approval(&state, "send it later", AnswerSource::Ambient).await,
            PendingAnswer::SayThePhrase("Say send it.".to_string())
        );
        assert_eq!(state.get_tool_approval_status("send-1").await, None);
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

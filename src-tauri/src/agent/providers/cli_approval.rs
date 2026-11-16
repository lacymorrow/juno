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
//! off restores the old skip-permissions behaviour for testing. With one
//! exception, added by LAC-8366 and written down here by LAC-8384: **"Ask me
//! first" in Security and Privacy forces the asking path on whatever the send
//! setting says.** It is the stronger promise, since the person asked to be
//! told before anything changes their Mac, and keeping it needs the same
//! `approve` route, so a person who chose it is asked about sends too.
//! [`routes_permission_prompts`] is that decision, and
//! [`skips_the_send_gate`] is its other half.
//!
//! Neither of those is the last word, because the CLI merges the person's own
//! `~/.claude/settings.json` into every spawn Juno makes and a `defaultMode`
//! there can switch the gate off before Juno is asked anything.
//! [`PERMISSION_PIN`] is what stops that.

use std::collections::HashMap;
use std::path::Path;
use std::sync::{LazyLock, Mutex};
use std::time::{Duration, Instant};

use serde_json::{json, Value};
use tauri::{Emitter, Manager};
use tracing::{info, warn};

use crate::agent::background_shell::{policy_for, ShellPolicy};
use crate::agent::tools::permission_policy::{self, PermissionMode};
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
///
/// **`Bash` is deliberately not here** (LAC-5007). It used to be. The CLI's
/// shell runs outside Juno's tool layer, so `open -a Mail` inside it took the
/// person's screen while they were typing, with every one of background
/// mode's guards looking the other way. Routing it through `approve` is what
/// lets [`background_shell_policy`] read a command before it runs.
///
/// It costs one loopback round trip per shell call and asks the person
/// nothing: with background mode off, or on a command that leaves the front
/// alone, `verdict_for` allows it on the spot. Deciding per call rather than
/// per spawn is also what makes toggling background mode take effect in the
/// conversation already running, instead of at the next cold start.
///
/// **`Edit`, `Write` and `NotebookEdit` are not here either** (LAC-8366). They
/// were, which meant "Ask me first" in Security and Privacy did nothing on this
/// provider: a file changed with no card. They now reach `approve` like `Bash`,
/// and [`ask_first_risk`] decides there, per call, against the mode read fresh
/// from settings. In every other mode `verdict_for` allows them on the spot, so
/// the only cost to a person who did not choose "Ask me first" is one loopback
/// round trip per edit.
///
/// **`Task` is here deliberately, and it was measured rather than assumed**
/// (LAC-8384). The open question was whether a subagent's `Edit` and `Write`
/// calls come back through the parent's permission prompt or run unseen inside
/// the subagent, which would make a pre-approved `Task` a way around every
/// gate in this module. Measured against `claude` 2.1.325 with a stub MCP
/// server standing in for Juno's: a `Task` subagent's `Write` arrived at
/// `approve` as `tool_name: "Write"` carrying its own `file_path`, the denial
/// stopped it, and the subagent reported the refusal back verbatim. A
/// subagent's tool calls are gated per call exactly like the parent's, so
/// pre-approving the delegation itself gates nothing away.
pub const ALLOWED_TOOLS: &str = "Read,Glob,Grep,WebFetch,WebSearch,\
     TodoWrite,Task,mcp__juno__computer,mcp__juno__settings";

/// The CLI's own shell tool, as the permission prompt names it, and as the
/// hook's `matcher` names it.
pub const BASH_TOOL: &str = "Bash";

/// The hook event the shell screen answers. Claude Code routes a hook answer
/// by this name, so a wrong one is silently ignored.
pub const HOOK_EVENT: &str = "PreToolUse";

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

/// Whether a spawn routes the CLI's permission prompts to `approve` instead of
/// skipping them.
///
/// Two settings, one answer. "Ask before Juno sends" asks about what leaves
/// the machine; "Ask me first" asks about anything that changes the Mac.
/// Either one needs the CLI to stop and ask, and there is one flag for that,
/// so either one turns it on.
///
/// The second half is a behaviour change worth stating plainly: **"Ask me
/// first" forces the asking path on even with "Ask before Juno sends" off.**
/// The stronger promise wins, so a person who chose to be asked before
/// anything changes their Mac is asked before a send too, whatever the send
/// setting says. Nobody can choose "ask me about everything" and "do not ask
/// me about sends" at once, and of the two readings this is the safe one.
///
/// Pure, because the `||` is the whole wiring: flipped to `&&`, "Ask me first"
/// silently stops working for anyone with the send gate off, and nothing else
/// in this module would notice (LAC-8384 item 2).
pub fn routes_permission_prompts(ask_first: bool, send_gate_enabled: bool) -> bool {
    ask_first || send_gate_enabled
}

/// Whether `approve` answers allow without reading the call at all.
///
/// "Ask before Juno sends" off means off: the old behaviour, everything runs.
/// It does not mean off under "Ask me first", and the reason is not that the
/// mode outranks the setting. It is that by the time this is asked,
/// [`ask_first_answer`] has already answered every tool that mode owns. What
/// reaches here is a name it has no opinion about, which in practice is an
/// `mcp__*` connector call, and the send gate is the right reader for one.
///
/// Pure for the same reason as [`routes_permission_prompts`]: dropping the
/// `mode != AskFirst` half hands "Ask me first" a silent exemption for every
/// connector send, and nothing else would redden.
pub fn skips_the_send_gate(mode: PermissionMode, send_gate_enabled: bool) -> bool {
    mode != PermissionMode::AskFirst && !send_gate_enabled
}

/// The permission arguments for a CLI spawn.
///
/// With the setting on and Juno's MCP server wired in, the CLI routes
/// permission prompts to `approve` and pre-approves the everyday tools.
/// Without the server there is nothing to serve the prompt, so the old
/// skip-permissions behaviour stays (that is the headless and test path,
/// where no connectors load under `--strict-mcp-config` anyway).
///
/// Background mode's shell screen (LAC-5007) rides this same route on the
/// asking path, because `ALLOWED_TOOLS` no longer pre-approves `Bash`.
///
/// It used to stop there. With "Ask before Juno sends" off there was no
/// `approve` call to read a command, so `open -a Mail` took the front again
/// (LAC-5010), on the configuration that is Lacy's default. `shell_screen` is
/// the repair: a `--settings` file installing a `PreToolUse` hook on `Bash`,
/// which Juno answers on its own loopback route.
///
/// The hook is **added to** skip-permissions rather than replacing it.
/// Swapping in `--permission-prompt-tool` would have been the obvious fix and
/// is the wrong one: skip-permissions suppresses more than the prompt tool
/// covers (trust dialogs, writes outside cwd), so the person who opted out of
/// the send gate would start being asked about things they never agreed to
/// see. A hook leaves `permissionMode` at `bypassPermissions` and reads only
/// `Bash`. Measured on `claude` 2.1.311: a `PreToolUse` hook fires under
/// skip-permissions, and its `updatedInput` and `deny` both take effect.
///
/// With no MCP server there is nothing to serve a prompt and nothing to
/// answer a hook, so that spawn still has no shell screen. It is the headless
/// and test path, where there is no desktop to take, and the gap is named
/// rather than papered over.
///
/// The asking path also carries [`PERMISSION_PIN`], without which the person's
/// own `claude` settings can switch the whole gate off (LAC-8384).
pub fn permission_args(
    ask_before_send: bool,
    has_mcp_server: bool,
    shell_screen: Option<&Path>,
) -> Vec<String> {
    if ask_before_send && has_mcp_server {
        return vec![
            "--permission-prompt-tool".to_string(),
            APPROVE_TOOL_CLI_NAME.to_string(),
            "--allowedTools".to_string(),
            ALLOWED_TOOLS.to_string(),
            // Pinned last, so it overrides the person's own settings rather
            // than being overridden by them.
            SETTINGS_FLAG.to_string(),
            PERMISSION_PIN.to_string(),
        ];
    }

    let mut args = vec![SKIP_PERMISSIONS.to_string()];
    if let Some(settings) = shell_screen {
        args.push(SETTINGS_FLAG.to_string());
        args.push(settings.display().to_string());
    }
    args
}

/// The flag that waves every permission check through.
const SKIP_PERMISSIONS: &str = "--dangerously-skip-permissions";

/// The flag that carries background mode's shell screen into the spawn.
const SETTINGS_FLAG: &str = "--settings";

/// Juno's own `claude` settings for the asking path, as an inline JSON string
/// (`--settings` takes "a settings JSON file or a JSON string", so this needs
/// no file and no temporary path).
///
/// It exists because the gate above is not the last word on whether the CLI
/// asks. The CLI merges the person's own `~/.claude/settings.json` into every
/// spawn Juno makes, and a `permissions.defaultMode` there that accepts edits
/// means `Write` and `Edit` never reach `approve` at all: no card, and "Ask me
/// first" does nothing, which is the bug LAC-8366 set out to fix arriving by a
/// second route. Measured on `claude` 2.1.325 (LAC-8384): with
/// `defaultMode: "acceptEdits"` a `Write` ran and `approve` was never called;
/// with this pinned the same `Write` reached `approve` and the denial held.
/// Lacy's own settings carry `defaultMode: "auto"`, which behaves the same
/// way, so without this the fix would have been inert on the machine it was
/// reviewed on.
///
/// Two levers, and they are **not** redundant. Each closes a bypass the other
/// leaves open, which is why neither line may be deleted as belt-and-braces:
///
/// - `defaultMode` is pinned to `default`, the mode that asks. The CLI's own
///   name for it is now `manual` (`--permission-mode` lists `manual`, not
///   `default`), and both spellings were measured to work, so the legacy one
///   is sent because older builds know only that. This is what closes the
///   `acceptEdits` / `auto` bypass above.
/// - `ask` names the tools by hand. It beats `defaultMode` (measured with
///   `defaultMode: "acceptEdits"` *and* this list, the `Write` still reached
///   `approve`), so it is also the backstop if a future build stops
///   understanding the legacy spelling.
///
///   **And it is the only thing that closes the hook bypass**, which is a
///   second, independent way to switch this gate off. A `PreToolUse` hook
///   answering `permissionDecision: "allow"` pre-empts
///   `--permission-prompt-tool` entirely: `approve` is never called and the
///   tool runs. Measured on 2.1.325, one variable changed between the two
///   runs: hook alone, `approve` not consulted and `echo hi` ran; the same
///   hook plus this `ask` list, `approve` consulted and the denial held.
///
///   Hooks load from the person's own `~/.claude/settings.json` **and** from
///   a project-level `.claude/settings.json` in the directory the CLI spawns
///   in, and `-p` skips the workspace trust dialog, so Juno never gets asked
///   about the directory. A project hook was measured firing from the spawn's
///   cwd.
///
///   That cwd is **always** the home directory, never a repo Juno happens to
///   be looking at: [`super::claude_cli::claude_command`] sets it on every
///   spawn, asserted by `claude_spawns_from_the_home_directory`. So both
///   halves of the bypass are the person's own `~/.claude` config rather than
///   third-party content, and at cwd `$HOME` the project path and the user
///   path are the same file. Juno reading an untrusted repo cannot plant a
///   hook on this path. What is left is a local process writing that file
///   between spawns, and the CLI's own `Bash` is one such process: see
///   LAC-8382 for the boundary call on that, which this PR does not make.
///   `--bare` would skip hooks and is unusable here: it also skips the
///   keychain reads this provider authenticates with.
///
/// `MultiEdit` is deliberately absent even though [`ask_first_risk`] maps it:
/// 2.1.325 prints "Permission ask rule "MultiEdit" matches no known tool" and
/// the person reads that in their chat.
///
/// What this cannot pin is the opted-out path, which spawns with
/// `--dangerously-skip-permissions` by the person's own choice and asks about
/// nothing by design.
const PERMISSION_PIN: &str = concat!(
    r#"{"permissions":{"defaultMode":"default","#,
    r#""ask":["Write","Edit","NotebookEdit","Bash"]}}"#
);

/// Why this spawn must not happen, when it would run the CLI's own shell with
/// nothing reading it and background mode is on (LAC-7421).
///
/// `None` is the normal answer: either something reads every `Bash` call (the
/// `approve` tool on the asking path, the `PreToolUse` hook on the opted-out
/// one), or background mode is off and there is no promise to keep.
///
/// Some, and the caller refuses to start the turn. The alternative, which is
/// what shipped in v0.8.269, is to degrade to a bare
/// `--dangerously-skip-permissions` on a failed screen write and say so in one
/// log line: exactly the pre-LAC-5010 state, where `open -a Mail` takes the
/// front off the person mid-sentence. Background mode is a promise that Juno
/// will stay out of their way while they work, and a turn that cannot keep it
/// should not run.
///
/// Read off the arguments [`permission_args`] actually returns, so the guard
/// and the spawn cannot drift apart.
pub fn unscreened_spawn_refusal(
    background_mode: bool,
    ask_before_send: bool,
    has_mcp_server: bool,
    shell_screen: Option<&Path>,
) -> Option<String> {
    if !background_mode {
        return None;
    }
    let args = permission_args(ask_before_send, has_mcp_server, shell_screen);
    let skips = args.iter().any(|a| a == SKIP_PERMISSIONS);
    let screened = args.iter().any(|a| a == SETTINGS_FLAG);
    if !skips || screened {
        return None;
    }
    // Read by the person, in the chat, in place of the answer they asked
    // for. So it says what Juno could not do in their terms, not in this
    // module's: no shell screen, no arming, no CLI. The log line beside it
    // carries the write error (LAC-7421).
    Some(
        "Juno could not keep its own shell commands off your screen, so it \
         did not run that. Restarting Juno usually fixes it. You can also \
         turn background mode off, which lets Juno's commands take the front."
            .to_string(),
    )
}

/// Read one `Bash` call against background mode.
///
/// `None` when this is not a shell call, when background mode is off, or when
/// the command leaves the front alone. `Some` is a refusal or a rewrite, and
/// which of the two it is decides when the caller may act on it: see
/// [`handle_approve`].
///
/// This used to answer in the CLI's permission shape and be the first word in
/// `handle_approve`, which is how a rewrite came to run with no card under
/// "Ask me first" (LAC-8384). A rewrite is not a decision, so it no longer
/// gets to be one.
///
/// A rewrite travels as `updatedInput`, which the CLI runs in place of what
/// the model wrote. A denial travels as the reason text, which the model does
/// read. Nothing else is written into the input: a permission allow has no
/// channel back to the model, so what it should say about a window that opened
/// behind the person's work is taught once in `MCP_TOOL_GUIDANCE` instead of
/// smuggled through a field meant for something else.
async fn background_shell_policy(
    app: &tauri::AppHandle,
    tool_name: &str,
    input: &Value,
) -> Option<ShellPolicy> {
    // The tool name first: it costs a string compare and answers most calls,
    // where reading the setting is an await.
    if tool_name != BASH_TOOL {
        return None;
    }
    let background_mode = crate::input_control::background_mode_enabled(app).await;
    shell_verdict(background_mode, tool_name, input)
}

/// What background mode says about one tool call, or `None` when it has
/// nothing to say: not the shell, background mode off, or a command that
/// leaves the front alone.
///
/// Pure in the one piece of live state it needs, so both envelopes below are
/// pinned in tests without an app in the loop.
fn shell_verdict(background_mode: bool, tool_name: &str, input: &Value) -> Option<ShellPolicy> {
    if !background_mode || tool_name != BASH_TOOL {
        return None;
    }
    let command = input.get("command").and_then(Value::as_str)?;
    match policy_for(command) {
        ShellPolicy::Run => None,
        judged => Some(judged),
    }
}

/// The same call with its command replaced.
///
/// Everything else the model wrote is carried through. Both envelopes replace
/// the whole input rather than merging into it, so a field dropped here is a
/// field the model loses: its own `description` and `timeout` among them.
fn with_command(input: &Value, command: &str) -> Value {
    let mut updated = input.clone();
    if let Some(map) = updated.as_object_mut() {
        map.insert("command".to_string(), json!(command));
    }
    updated
}

/// Read one `Bash` call against background mode, in the PreToolUse hook's
/// answer shape (LAC-5010).
///
/// Same verdict as [`background_shell_policy`], different envelope. The
/// `approve` route only exists when the CLI spawns with
/// `--permission-prompt-tool`; with "Ask before Juno sends" off it spawns with
/// `--dangerously-skip-permissions` instead, and a hook is the one thing that
/// still runs there. Measured on `claude` 2.1.311: a `PreToolUse` hook fires
/// under skip-permissions, its `updatedInput` replaces what the model wrote,
/// and a `deny` stops the call with the reason reaching the model.
///
/// `{}` means "nothing to say", which the CLI treats as "carry on". That is
/// the answer for a tool that is not the shell, for background mode off, and
/// for a command that leaves the front alone.
///
/// Pure in the one piece of live state it needs, so every shape is pinned in
/// tests without an app in the loop.
fn hook_answer_for(background_mode: bool, tool_name: &str, input: &Value) -> Value {
    match shell_verdict(background_mode, tool_name, input) {
        None | Some(ShellPolicy::Run) => json!({}),
        Some(ShellPolicy::Rewritten { command, note }) => json!({
            "hookSpecificOutput": {
                "hookEventName": HOOK_EVENT,
                "permissionDecision": "allow",
                "permissionDecisionReason": note,
                "updatedInput": with_command(input, &command),
            }
        }),
        Some(ShellPolicy::Refused(reason)) => json!({
            "hookSpecificOutput": {
                "hookEventName": HOOK_EVENT,
                "permissionDecision": "deny",
                "permissionDecisionReason": reason,
            }
        }),
    }
}

/// The hook route's answer for one `PreToolUse` payload from the CLI.
pub async fn background_shell_hook_answer(app: &tauri::AppHandle, payload: &Value) -> Value {
    let background_mode = crate::input_control::background_mode_enabled(app).await;
    let tool_name = payload
        .get("tool_name")
        .and_then(Value::as_str)
        .unwrap_or_default();
    let input = payload
        .get("tool_input")
        .cloned()
        .unwrap_or_else(|| json!({}));
    hook_answer_for(background_mode, tool_name, &input)
}

/// The risk of one CLI built-in under "Ask me first", or `None` when the tool is
/// not one the CLI changes the Mac with (reads, search, planning, connectors).
///
/// The CLI's `Edit`, `Write`, `NotebookEdit` and `Bash` are Juno's `edit_file`,
/// `write_file` and `bash` under other names, so they are judged by the same
/// classifier the in-process runner uses rather than by a second list that
/// could drift from it. A path with `..` or under a system directory is
/// Critical there, and Critical asks in every mode.
pub fn ask_first_risk(tool_name: &str, input: &Value) -> Option<(&'static str, Value, RiskLevel)> {
    let (mapped, mapped_input): (&'static str, Value) = match tool_name {
        "Write" => ("write_file", input.clone()),
        // `MultiEdit` is mapped although `claude` 2.1.325 has no such tool
        // (the pin's `ask` list leaves it out for exactly that reason, since a
        // rule naming no known tool prints a warning into the person's chat).
        // Mapping costs nothing and an older build that still emits it is
        // gated rather than waved through.
        "Edit" | "MultiEdit" => ("edit_file", input.clone()),
        "NotebookEdit" => {
            // The classifier reads `path` or `file_path`; a notebook names its
            // file `notebook_path`, and an unread path is a Medium that should
            // have been Critical.
            let mut notebook = input.clone();
            if let (Some(map), Some(path)) = (
                notebook.as_object_mut(),
                input.get("notebook_path").cloned(),
            ) {
                map.entry("path").or_insert(path);
            }
            ("edit_file", notebook)
        }
        BASH_TOOL => ("bash", input.clone()),
        _ => return None,
    };
    let risk = crate::agent::tools::risk_classifier::classify_risk(mapped, &mapped_input);
    Some((mapped, mapped_input, risk))
}

/// The cooldown key for one call, so a retrying loop cannot nag.
///
/// The send gate keys on the tool name alone, and should: one connector tool
/// asked about once is the same question on a retry, and a loop that varied
/// the recipient would otherwise raise a fresh sheet per attempt. "Ask me
/// first" asks per file, so its key carries the path or the command too.
/// Keyed on the name alone, declining one edit would auto-deny an edit to
/// every other file for a minute, which is a wrong answer rather than a quiet
/// one (LAC-8384 item 3).
fn denial_key(tool_name: &str, input: &Value) -> String {
    let target = ["file_path", "path", "notebook_path", "command"]
        .iter()
        .find_map(|key| input.get(*key).and_then(Value::as_str));
    match target {
        // Bounded: a `Bash` heredoc can be arbitrarily long and this is held
        // for the cooldown. 120 chars still tells two files or two commands
        // apart, which is all the key is for, and `chars` keeps it off a
        // UTF-8 boundary panic. Two commands sharing a 120-char prefix share
        // a cooldown, which costs one extra card after a decline (LAC-8384).
        Some(target) => {
            let target: String = target.chars().take(120).collect();
            format!("{tool_name}:{target}")
        }
        None => tool_name.to_string(),
    }
}

/// Ask the person about one CLI built-in under "Ask me first".
///
/// `None` means this call is not gated by the mode (a read, or a tool this
/// module does not own) and the ordinary path continues. `Some` is the answer
/// to hand the CLI: allow when the policy lets it through, otherwise the
/// person's decision. A tool that reaches here and cannot be shown to the
/// person is denied, never allowed.
///
/// `approved` is what runs if the answer is yes, which is not always `input`:
/// background mode may have rewritten the command, and that rewrite is applied
/// after the person agrees rather than instead of asking them (LAC-8384).
async fn ask_first_answer(
    app: &tauri::AppHandle,
    tool_name: &str,
    input: &Value,
    approved: &Value,
) -> Option<Value> {
    let (mapped, mapped_input, risk) = ask_first_risk(tool_name, input)?;
    if !permission_policy::requires_approval_for(mapped, PermissionMode::AskFirst, &risk, false) {
        return Some(allow_result(approved));
    }

    // A loop retrying the same declined call gets the same answer without a
    // fresh sheet 60 seconds in a row.
    let key = denial_key(tool_name, input);
    if recently_denied(&key, Instant::now()) {
        return Some(deny_result(
            "Juno asked about this moments ago and it was not done: the person declined, \
             or did not answer. Do not retry; tell them it was not done, and that they can \
             ask again if they change their mind.",
        ));
    }

    // The card describes what the model asked for, not `approved`. Under
    // background mode those differ by the rewrite (`open` against `open -g`),
    // and the person recognises the command they caused. The rewrite is
    // background mode keeping its own promise about the screen, which is not a
    // second thing to agree to, and it is taught to the model once in
    // `MCP_TOOL_GUIDANCE`. Deliberate: do not "fix" the mismatch.
    let description = permission_policy::describe_action(mapped, &mapped_input);
    let request = ToolApprovalRequest::new(
        uuid::Uuid::new_v4().to_string(),
        tool_name.to_string(),
        input.clone(),
        description.clone(),
    )
    .with_risk(risk)
    .with_timeout(APPROVAL_TIMEOUT_SECS);

    Some(match ask_person(app, request).await {
        Some(true) => {
            clear_denial(&key);
            info!("[CliApproval] Ask me first, approved: {description}");
            allow_result(approved)
        }
        Some(false) => {
            record_denial(&key);
            info!("[CliApproval] Ask me first, declined: {description}");
            deny_result(
                "The person chose to be asked first and declined, so Juno did not do this. \
                 Tell them it was not done; they can ask again if they change their mind.",
            )
        }
        None => {
            record_denial(&key);
            info!(
                "[CliApproval] Ask me first, no answer in {APPROVAL_TIMEOUT_SECS}s: {description}"
            );
            deny_result(
                "No answer within 60 seconds, so Juno did not do this. Tell the person \
                 their approval was needed and no answer arrived; they can ask again.",
            )
        }
    })
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
///
/// `WebFetch` and `WebSearch` are the same gap wearing the CLI's own names, and
/// they are in `ALLOWED_TOOLS` as well, so they do not even reach here: a URL
/// can carry data out of the machine in its query string and nothing above
/// reads it as a send. That is not an oversight to be patched with another
/// list. It is the reason the direction of travel is to gate by consequence
/// where the parameters are known, which is `docs/plans/permissions-by-consequence.md`.
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
        let now = Instant::now();
        // Swept here rather than on read, because the keys stopped being a
        // closed set: `denial_key` carries the path or the command, so a long
        // session declining many calls would otherwise hold every one of them
        // for as long as the app runs. An entry past its cooldown answers
        // nothing, so dropping it changes no decision (LAC-8384).
        denials.retain(|_, at| now.duration_since(*at) < DENY_COOLDOWN);
        denials.insert(tool_name.to_string(), now);
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
    // A permission ask is drawn as the one-line card with Continue.
    if let (Some(permission), Some(map)) = (&request.permission, approval_event.as_object_mut()) {
        map.insert("permission".to_string(), permission.clone());
        map.insert("consequence".to_string(), json!("permission"));
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
            .or(request.permission.as_ref())
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

/// What [`handle_approve`] may do with background mode's verdict.
///
/// The type is the point, and it is doing more work than the test beside it. A
/// refusal is an answer and carries the sentence the model reads. A rewrite is
/// not an answer, and there is no variant in which it could be one: it is only
/// ever a replacement for the input, handed on to the gates below. The
/// LAC-8384 bug was one `return` on the rewrite path, so it is worth making
/// unexpressible rather than only asserted. A `return` is cheap to write.
#[derive(Debug, PartialEq, Eq)]
enum BackgroundRead {
    /// Do not run it, whatever anyone says. Nobody is asked.
    Refuse(String),
    /// Nothing is answered yet. `Some` replaces the input if, and only if, a
    /// gate below allows the call.
    Pending(Option<String>),
}

/// Read background mode's verdict the way [`handle_approve`] reads it.
///
/// Pure, so the one thing that must never happen is asserted with no app in
/// the loop: see `a_background_rewrite_is_never_an_answer`.
fn read_background(policy: Option<ShellPolicy>) -> BackgroundRead {
    match policy {
        Some(ShellPolicy::Refused(reason)) => BackgroundRead::Refuse(reason),
        Some(ShellPolicy::Rewritten { command, .. }) => BackgroundRead::Pending(Some(command)),
        None | Some(ShellPolicy::Run) => BackgroundRead::Pending(None),
    }
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

    // Background mode reads the shell call first, and independent of the send
    // gate (LAC-5007): "Ask before Juno sends" is about what leaves the
    // machine, this is about whose screen it is, and turning one off must not
    // turn the other off.
    //
    // Only a refusal is an answer on its own. Juno will not run that command
    // whatever anyone says, so putting a card in front of it would be a
    // question with one answer.
    //
    // A rewrite is not an answer. It changes *what* runs, not *whether* it may
    // run, and it used to return allow from right here, so under "Ask me
    // first" a `Bash` `open` was rewritten to `open -g` and ran with no card
    // at all. Background mode is on by default, which made that the default
    // configuration (LAC-8384). The rewrite is now carried past the gates
    // below and applied only once something has allowed the call.
    let rewrite = match read_background(background_shell_policy(app, tool_name, &input).await) {
        BackgroundRead::Refuse(reason) => {
            let command = input
                .get("command")
                .and_then(Value::as_str)
                .unwrap_or_default();
            info!("[CliApproval] Background mode refused `{command}`");
            return deny_result(&reason);
        }
        BackgroundRead::Pending(rewrite) => {
            if let Some(command) = &rewrite {
                info!("[CliApproval] Background mode: running `{command}` instead");
            }
            rewrite
        }
    };

    // What runs if this call is allowed. Every allow below answers with this
    // rather than with the raw input, so no gate can drop the rewrite on the
    // way out, including the one that turns the send gate off entirely, which
    // is the configuration background mode has to keep working on.
    let approved = match &rewrite {
        Some(command) => with_command(&input, command),
        None => input.clone(),
    };

    // "Ask me first" is a separate promise from "Ask before Juno sends", and
    // it is the stronger one: the person chose to be asked before anything
    // changes the Mac. It is read fresh per call, so a change in Settings
    // applies to the conversation already running (LAC-8366).
    let mode = super::juno_mcp::permission_mode(app).await;
    if mode == PermissionMode::AskFirst {
        if let Some(answer) = ask_first_answer(app, tool_name, &input, &approved).await {
            return answer;
        }
    }

    // Off means off: the old behaviour, everything runs. Not under "Ask me
    // first", and the reason is not that the mode outranks the setting. It is
    // that `ask_first_answer` has already answered every tool that mode owns,
    // so what reaches here is a name it has no opinion about (an `mcp__*`
    // connector call), and the send gate is the right reader for one. See
    // [`skips_the_send_gate`], which is where that is written down and tested.
    if skips_the_send_gate(mode, is_enabled(app)) {
        return allow_result(&approved);
    }

    if verdict_for(tool_name) == Verdict::Allow {
        return allow_result(&approved);
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
            allow_result(&approved)
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
    use crate::agent::tools::mac_apps::preflight::{self as permission, ContinueReply};
    use crate::agent::tools::mac_apps::send::{self, SendReply};

    let pending = app_state.get_pending_tool_approvals().await;
    if pending.is_empty() {
        return PendingAnswer::NotAnAnswer;
    }
    let (mut asks, pending): (Vec<_>, Vec<_>) =
        pending.into_iter().partition(|r| r.is_permission());
    // One already answered is the waiter's to collect, not to answer again.
    asks.retain(|r| r.approved.is_none());

    // A waiting permission ask ("I need Reminders for that.") is answered on
    // its own terms: "continue" brings the system dialog up, a clear no drops
    // it, and anything else said on purpose is a new request, so the ask is
    // dropped and the words carry on as a query. Overheard talk changes
    // nothing.
    if !asks.is_empty() {
        match permission::classify_reply(text, parse_spoken_approval) {
            ContinueReply::Continue => {
                for request in &asks {
                    app_state.approve_tool(&request.tool_id).await;
                }
                info!(
                    "[CliApproval] '{}' continued {} permission ask(s)",
                    text,
                    asks.len()
                );
                return PendingAnswer::Answered;
            }
            ContinueReply::Cancel => {
                for request in &asks {
                    app_state.deny_tool(&request.tool_id).await;
                }
                return PendingAnswer::Answered;
            }
            ContinueReply::Other => {
                if source == AnswerSource::Deliberate {
                    for request in &asks {
                        app_state.deny_tool(&request.tool_id).await;
                    }
                    info!("[CliApproval] '{}' replaced a waiting permission ask", text);
                }
                if pending.is_empty() {
                    return PendingAnswer::NotAnAnswer;
                }
            }
        }
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
    /// LAC-5010. The two settings that decide a spawn's permission posture,
    /// in all four combinations, so moving one of them can never quietly
    /// move another.
    #[test]
    fn all_four_permission_postures_are_pinned() {
        let screen = Path::new("/tmp/juno-cli-hooks-1.json");

        // Asking, with a server to ask through: the prompt tool, and the
        // shell screen rides it because `Bash` is not pre-approved.
        assert_eq!(
            permission_args(true, true, Some(screen)),
            vec![
                "--permission-prompt-tool".to_string(),
                APPROVE_TOOL_CLI_NAME.to_string(),
                "--allowedTools".to_string(),
                ALLOWED_TOOLS.to_string(),
                // The person's own settings cannot switch this gate off
                // (LAC-8384). The shell screen is deliberately *not* here:
                // see `the_shell_screen_never_rides_the_asking_path`.
                "--settings".to_string(),
                PERMISSION_PIN.to_string(),
            ]
        );

        // Not asking, but the server is there: permissions are skipped, as
        // the person asked, and the shell screen arrives as a PreToolUse
        // hook instead of through `approve`.
        assert_eq!(
            permission_args(false, true, Some(screen)),
            vec![
                "--dangerously-skip-permissions".to_string(),
                "--settings".to_string(),
                screen.display().to_string(),
            ]
        );

        // No server: nothing to serve a prompt, and nothing to answer a
        // hook either. The headless and test path, named rather than
        // papered over.
        assert_eq!(
            permission_args(true, false, None),
            vec!["--dangerously-skip-permissions".to_string()]
        );
        assert_eq!(
            permission_args(false, false, None),
            vec!["--dangerously-skip-permissions".to_string()]
        );
    }

    /// LAC-5010's second acceptance line: nothing that skip-permissions used
    /// to wave through starts prompting.
    ///
    /// The hook is additive. It carries no `--permission-prompt-tool` and no
    /// `--allowedTools`, so the CLI stays in `bypassPermissions` and the
    /// trust dialogs and out-of-cwd writes the person opted out of stay
    /// waved through. Only `Bash` is read, and only to keep the screen.
    #[test]
    fn the_shell_screen_adds_no_prompt_to_the_opted_out_path() {
        let args = permission_args(false, true, Some(Path::new("/tmp/juno-cli-hooks-1.json")));
        assert!(args.contains(&"--dangerously-skip-permissions".to_string()));
        assert!(!args.contains(&"--permission-prompt-tool".to_string()));
        assert!(!args.contains(&"--allowedTools".to_string()));
    }

    /// LAC-7421: a failed screen write used to come back as `None`, and
    /// `None` spawned a bare `--dangerously-skip-permissions`, which is the
    /// pre-LAC-5010 state where `open -a Mail` takes the front. With
    /// background mode on, that spawn is refused instead.
    #[test]
    fn an_unscreened_spawn_is_refused_while_background_mode_is_on() {
        let screen = Path::new("/tmp/juno-cli-hooks-1.json");

        // The failure this is here for: the tool server is up, so the
        // `computer` tool works and nothing else looks wrong, but the screen
        // did not get written.
        let why = unscreened_spawn_refusal(true, false, true, None)
            .expect("an unscreened spawn is refused");
        // The person reads this in place of an answer, so it has to say what
        // did not happen and how to get the turn to run.
        let said = why.to_lowercase();
        assert!(said.contains("did not run"), "{why}");
        assert!(said.contains("background mode"), "{why}");
        assert!(
            !said.contains("shell screen") && !said.contains("--settings"),
            "the refusal is read by a person, not by this module: {why}"
        );

        // Both of the other ways in: no tool server at all, and the asking
        // path falling back to skip-permissions because there is nothing to
        // serve a prompt.
        assert!(unscreened_spawn_refusal(true, false, false, None).is_some());
        assert!(unscreened_spawn_refusal(true, true, false, None).is_some());

        // Screened: the hook reads every `Bash` call on the opted-out path...
        assert_eq!(
            unscreened_spawn_refusal(true, false, true, Some(screen)),
            None
        );
        // ...and `approve` reads it on the asking path, where `Bash` is not
        // pre-approved, so no `--settings` file is needed.
        assert_eq!(unscreened_spawn_refusal(true, true, true, None), None);

        // Background mode off: nothing was promised about the front, so an
        // unscreened spawn is what the person asked for.
        assert_eq!(unscreened_spawn_refusal(false, false, true, None), None);
        assert_eq!(unscreened_spawn_refusal(false, false, false, None), None);
    }

    #[test]
    fn the_clis_shell_is_never_pre_approved() {
        assert!(
            !ALLOWED_TOOLS.split(',').any(|t| t == BASH_TOOL),
            "pre-approving {BASH_TOOL} puts the CLI's shell back outside \
             background mode's reach (LAC-5007)"
        );
    }

    fn hook_decision(answer: &Value) -> Option<&str> {
        answer
            .get("hookSpecificOutput")?
            .get("permissionDecision")?
            .as_str()
    }

    fn hook_command(answer: &Value) -> Option<&str> {
        answer
            .get("hookSpecificOutput")?
            .get("updatedInput")?
            .get("command")?
            .as_str()
    }

    /// LAC-5010 regression. The two commands from the 2026-10-19 transcript,
    /// verbatim, down the skip-permissions path this time: the hook has to
    /// background them exactly as `approve` does on the asking path.
    #[test]
    fn the_two_commands_that_took_lacys_screen_are_backgrounded_by_the_hook_too() {
        for (before, after) in [
            (
                r#"cd ~/Desktop/"Jenny's Folder" && open -a Mail "Headphones receipt.pdf" "Kitchen remodel quote.pdf""#,
                r#"cd ~/Desktop/"Jenny's Folder" && open -g -a Mail "Headphones receipt.pdf" "Kitchen remodel quote.pdf""#,
            ),
            ("open -a Mail", "open -g -a Mail"),
        ] {
            let answer = hook_answer_for(true, BASH_TOOL, &json!({ "command": before }));
            assert_eq!(hook_decision(&answer), Some("allow"), "{before:?}");
            assert_eq!(hook_command(&answer), Some(after), "{before:?}");
            assert_eq!(
                answer["hookSpecificOutput"]["hookEventName"],
                json!("PreToolUse")
            );
        }
    }

    /// Only `command` changes. The CLI runs `updatedInput` in place of what
    /// the model wrote, so dropping a field here would silently drop the
    /// model's timeout or its description from the transcript.
    #[test]
    fn a_rewrite_keeps_the_rest_of_the_call() {
        let answer = hook_answer_for(
            true,
            BASH_TOOL,
            &json!({ "command": "open -a Mail", "description": "Open Mail", "timeout": 5000 }),
        );
        let updated = &answer["hookSpecificOutput"]["updatedInput"];
        assert_eq!(updated["command"], json!("open -g -a Mail"));
        assert_eq!(updated["description"], json!("Open Mail"));
        assert_eq!(updated["timeout"], json!(5000));
    }

    /// An activation has no background form, so the hook refuses it and the
    /// reason is the sentence the model acts on. Measured: a hook `deny`
    /// reaches the model as text under skip-permissions.
    #[test]
    fn an_activation_is_denied_with_the_reason_the_model_reads() {
        let answer = hook_answer_for(
            true,
            BASH_TOOL,
            &json!({ "command": r#"osascript -e 'tell application "Mail" to activate'"# }),
        );
        assert_eq!(hook_decision(&answer), Some("deny"));
        let reason = answer["hookSpecificOutput"]["permissionDecisionReason"]
            .as_str()
            .unwrap_or_default();
        assert!(reason.contains("Mail"), "{reason}");
        assert!(reason.contains("where it is"), "{reason}");
        // A denial has nothing to run, so it carries no rewritten input.
        assert!(answer["hookSpecificOutput"].get("updatedInput").is_none());
    }

    /// `{}` is "carry on", and it is the answer to everything the screen has
    /// no business touching. Background mode off is first: that is the person
    /// having said they are not working alongside Juno right now.
    #[test]
    fn the_hook_stays_silent_when_it_has_nothing_to_say() {
        let nothing = json!({});
        for (background_mode, tool, input) in [
            (false, BASH_TOOL, json!({ "command": "open -a Mail" })),
            (true, "Read", json!({ "file_path": "/tmp/x" })),
            (
                true,
                BASH_TOOL,
                json!({ "command": "ls -la && git status" }),
            ),
            (true, BASH_TOOL, json!({ "command": "open -g -a Mail" })),
            // A payload with no command at all must not panic or guess.
            (true, BASH_TOOL, json!({})),
        ] {
            assert_eq!(
                hook_answer_for(background_mode, tool, &input),
                nothing,
                "{tool} {input} with background mode {background_mode}"
            );
        }
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

    /// LAC-8366: the file tools are not pre-approved, so they reach `approve`.
    #[test]
    fn file_writing_builtins_are_not_pre_approved() {
        for tool in ["Edit", "Write", "NotebookEdit", "MultiEdit", BASH_TOOL] {
            assert!(
                !ALLOWED_TOOLS.split(',').any(|t| t == tool),
                "{tool} must reach approve or Ask me first cannot see it"
            );
        }
    }

    /// LAC-8366: what "Ask me first" stops and what it lets through.
    #[test]
    fn ask_first_asks_for_every_change_and_waves_reads_through() {
        let asks = |tool: &str, input: Value| {
            ask_first_risk(tool, &input).is_some_and(|(name, _, risk)| {
                permission_policy::requires_approval_for(
                    name,
                    PermissionMode::AskFirst,
                    &risk,
                    false,
                )
            })
        };
        assert!(asks(
            "Write",
            json!({"file_path": "/Users/a/notes.md", "content": "x"})
        ));
        assert!(asks("Edit", json!({"file_path": "/Users/a/notes.md"})));
        assert!(asks("MultiEdit", json!({"file_path": "/Users/a/notes.md"})));
        assert!(asks(
            "NotebookEdit",
            json!({"notebook_path": "/Users/a/n.ipynb"})
        ));
        assert!(asks("Bash", json!({"command": "rm old.txt"})));
        // Reads, and an inert command, do not stop the person.
        assert!(!asks("Bash", json!({"command": "ls -la"})));
        for tool in ["Read", "Glob", "Grep", "WebSearch", "TodoWrite", "Task"] {
            assert!(ask_first_risk(tool, &json!({})).is_none(), "{tool}");
        }
    }

    /// A notebook's path must reach the classifier, or a system path is Medium.
    #[test]
    fn ask_first_reads_a_notebooks_path_for_the_critical_floor() {
        let (_, _, risk) =
            ask_first_risk("NotebookEdit", &json!({"notebook_path": "/etc/x.ipynb"}))
                .expect("a notebook edit is gated");
        assert_eq!(risk, RiskLevel::Critical);
    }

    /// LAC-8384 item 2: the first of the two wiring decisions, as a truth
    /// table. Written inline in `turn_environment` it was unreachable by any
    /// test, so flipping its `||` to `&&` reddened nothing.
    #[test]
    fn ask_me_first_routes_prompts_whatever_the_send_setting_says() {
        assert!(routes_permission_prompts(true, true));
        // The assert `&&` turns red, and the only one that does: the mode has
        // to ask even though "Ask before Juno sends" is off.
        assert!(
            routes_permission_prompts(true, false),
            "Ask me first must route prompts with the send gate off"
        );
        assert!(routes_permission_prompts(false, true));
        // Neither promise: the old skip-permissions spawn.
        assert!(!routes_permission_prompts(false, false));
    }

    /// LAC-8384 item 2: the second decision, which used to be an inline
    /// `mode != AskFirst && !is_enabled(app)` no test could reach.
    #[test]
    fn only_an_off_send_gate_outside_ask_me_first_skips_reading_the_call() {
        use PermissionMode::{AskFirst, AskWhenRisky, DontAsk};

        // Off means off, in the modes that promise nothing more.
        assert!(skips_the_send_gate(AskWhenRisky, false));
        assert!(skips_the_send_gate(DontAsk, false));

        // The assert that reddens when the `mode != AskFirst` half is dropped:
        // "Ask me first" keeps reading connector calls with the gate off.
        assert!(
            !skips_the_send_gate(AskFirst, false),
            "Ask me first must not inherit the send gate's off switch"
        );

        // With the gate on, nothing is skipped in any mode.
        for mode in [AskFirst, AskWhenRisky, DontAsk] {
            assert!(!skips_the_send_gate(mode, true), "{mode:?}");
        }
    }

    /// LAC-8384: the gate above is not the last word on whether the CLI asks.
    /// The person's own `~/.claude/settings.json` rides every spawn Juno
    /// makes, and a `permissions.defaultMode` there that accepts edits means
    /// `Write` never reaches `approve` at all. Measured on `claude` 2.1.325;
    /// see [`PERMISSION_PIN`].
    #[test]
    fn the_asking_path_pins_the_permission_mode() {
        let args = permission_args(true, true, None);
        let at = args
            .iter()
            .position(|a| a == SETTINGS_FLAG)
            .expect("the asking path pins the mode");
        let pin: Value = serde_json::from_str(&args[at + 1]).expect("the pin is JSON");

        assert_eq!(pin["permissions"]["defaultMode"], json!("default"));

        // The second lever, which beats `defaultMode` outright.
        let ask = pin["permissions"]["ask"]
            .as_array()
            .expect("the pin names the tools")
            .clone();
        for tool in ["Write", "Edit", "NotebookEdit", BASH_TOOL] {
            assert!(ask.iter().any(|t| t == tool), "{tool} must be pinned");
        }
        // 2.1.325 prints "matches no known tool" into the person's chat for a
        // rule naming one, so this stays out even though `ask_first_risk`
        // maps it.
        assert!(!ask.iter().any(|t| t == "MultiEdit"));

        // Do not "simplify" this list away as a backstop for `defaultMode`.
        // It is the only half of the pin that survives an allow-hook, and a
        // hook can arrive from a project-level `.claude/settings.json` in the
        // directory the CLI spawns in. Measured on 2.1.325: hook alone, the
        // permission prompt was never consulted; hook plus this list, it was.
    }

    /// The shell screen must never ride the asking path, and this is a
    /// security property rather than tidiness. Measured on `claude` 2.1.325
    /// (LAC-8384): a `PreToolUse` hook answering `permissionDecision: "allow"`
    /// pre-empts `--permission-prompt-tool` entirely, and `approve` was never
    /// called for the `Bash` the hook allowed. So carrying the screen here
    /// would hand every shell call back to the hook and take "Ask me first"
    /// out of the loop again, which is the bug this ticket exists for.
    #[test]
    fn the_shell_screen_never_rides_the_asking_path() {
        let screen = Path::new("/tmp/juno-cli-hooks-1.json");
        let args = permission_args(true, true, Some(screen));
        assert!(
            !args.iter().any(|a| a == &screen.display().to_string()),
            "a hook allow pre-empts `approve`, so the screen cannot ride this path"
        );

        // And the opted-out path still carries it, where it is the only
        // reader a `Bash` call gets.
        let opted_out = permission_args(false, true, Some(screen));
        let at = opted_out
            .iter()
            .position(|a| a == SETTINGS_FLAG)
            .expect("the screen rides the opted-out path");
        assert_eq!(opted_out[at + 1], screen.display().to_string());
        assert!(!opted_out.iter().any(|a| a == PERMISSION_PIN));
    }

    /// LAC-8384 item 1, the blocking one, half 1: the premise.
    ///
    /// Background mode rewrites `open` so it does not take the front, and
    /// "Ask me first" asks before anything changes the Mac. Both are true of
    /// the same call, which is what made the order matter: the rewrite used to
    /// answer first and the card was never raised. Background mode is on by
    /// default, so that was the default configuration.
    #[test]
    fn a_rewritten_shell_command_is_still_one_ask_me_first_asks_about() {
        let call = json!({"command": "open https://example.com"});

        let policy =
            shell_verdict(true, BASH_TOOL, &call).expect("background mode rewrites `open`");
        assert!(
            matches!(policy, ShellPolicy::Rewritten { .. }),
            "{policy:?}: this test needs a command background mode rewrites"
        );

        let (mapped, _, risk) = ask_first_risk(BASH_TOOL, &call).expect("a shell call is gated");
        assert!(
            permission_policy::requires_approval_for(
                mapped,
                PermissionMode::AskFirst,
                &risk,
                false
            ),
            "`open` is not inert, so Ask me first asks about it"
        );
    }

    /// LAC-8384 item 1, half 1b: a rewrite is never an answer.
    ///
    /// This is the shape of the bug, as a value. A refusal answers the call; a
    /// rewrite is `Pending`, which is the type saying "nothing is settled, and
    /// here is what runs if something below settles it yes".
    #[test]
    fn a_background_rewrite_is_never_an_answer() {
        let rewritten = ShellPolicy::Rewritten {
            command: "open -g https://example.com".to_string(),
            note: "opened behind your work".to_string(),
        };
        assert_eq!(
            read_background(Some(rewritten)),
            BackgroundRead::Pending(Some("open -g https://example.com".to_string())),
            "a rewrite changes what runs, it does not decide that it may"
        );

        // A refusal is the one verdict that answers on its own.
        assert_eq!(
            read_background(Some(ShellPolicy::Refused("no".to_string()))),
            BackgroundRead::Refuse("no".to_string())
        );

        // Nothing to say, in both of the ways there are to say nothing.
        assert_eq!(read_background(None), BackgroundRead::Pending(None));
        assert_eq!(
            read_background(Some(ShellPolicy::Run)),
            BackgroundRead::Pending(None)
        );
    }

    /// LAC-8384 item 1, half 2: the order itself.
    ///
    /// Pinned by reading the source, which is the only way statement order in
    /// an `AppHandle`-bound async function can be. `handle_approve` cannot be
    /// called from a unit test: it wants a running app, a settings store, a
    /// window to put the sheet in and a person to answer it. What can be
    /// asserted is the shape of the window between reading background mode's
    /// verdict and raising the card: nothing in it may answer the call with
    /// an allow. The bug was exactly one `return` in that window, and every
    /// other test in this file stayed green.
    #[test]
    fn nothing_answers_allow_between_background_mode_and_the_ask_me_first_card() {
        // Comments are stripped first. Both review passes said the same thing
        // about the first version of this test, and they were right: a comment
        // in that window mentioning `allow_result` would have failed it with
        // nothing wrong, which is how a tripwire gets deleted (LAC-8384).
        //
        // Trailing comments go too, not just whole-line ones. Dropping a line
        // at its first `//` would truncate real code when that `//` is inside
        // a string literal (a URL), and a truncation is the dangerous
        // direction: it makes the tripwire blind rather than noisy. So the cut
        // only happens when nothing quoted precedes the `//`.
        //
        // The two marker names below are load-bearing. If you rename
        // `background_shell_policy` or `ask_first_answer`, update them here:
        // both are `expect`ed, so a rename fails this test loudly instead of
        // passing it over a window that is not there.
        let source: String = include_str!("cli_approval.rs")
            .lines()
            .map(|line| match line.split_once("//") {
                Some((before, _)) if !before.contains('"') => before,
                _ => line,
            })
            .collect::<Vec<_>>()
            .join("\n");
        let body = source
            .split_once("pub async fn handle_approve")
            .expect("handle_approve is declared in this file")
            .1;

        // Positive control: a rename must break this test loudly rather than
        // make it pass over a window that is not there.
        let read_at = body
            .find("background_shell_policy(")
            .expect("handle_approve reads background mode");
        let ask_at = body
            .find("ask_first_answer(")
            .expect("handle_approve raises the Ask me first card");
        assert!(
            read_at < ask_at,
            "background mode is read first, so the card knows what would run"
        );

        let window = &body[read_at..ask_at];
        assert!(
            !window.is_empty(),
            "the window between the two is where the bug lived"
        );
        assert!(
            !window.contains("allow_result"),
            "a background rewrite must not answer allow before the card: {window}"
        );
    }

    /// LAC-8384 item 4: the Critical floor is the security-relevant part of
    /// the mapping, and only the notebook case was tested. A system path or a
    /// traversal asks in every mode, so these are the calls that must not come
    /// back Medium.
    #[test]
    fn a_system_path_or_a_traversal_is_critical_for_every_file_tool() {
        let critical = |tool: &str, input: Value| {
            let (_, _, risk) = ask_first_risk(tool, &input).expect("a file write is gated");
            assert_eq!(risk, RiskLevel::Critical, "{tool} {input}");
        };

        critical(
            "Write",
            json!({"file_path": "/etc/sudoers", "content": "x"}),
        );
        critical("Edit", json!({"file_path": "/etc/hosts"}));
        critical("MultiEdit", json!({"file_path": "/etc/hosts"}));
        critical(
            "Write",
            json!({"file_path": "/tmp/../../etc/passwd", "content": "x"}),
        );
        critical("Edit", json!({"file_path": "/Users/a/../../etc/passwd"}));
        critical(
            "NotebookEdit",
            json!({"notebook_path": "/tmp/../../etc/x.ipynb"}),
        );
    }

    /// The sheet must name the notebook, which is not free: `NotebookEdit` is
    /// judged and described under the name `edit_file`, and the sentence is
    /// built from the mapped input rather than the original. Drop the
    /// `notebook_path` copy in `ask_first_risk` and the person is asked to
    /// approve "Change a file" (LAC-8384).
    #[test]
    fn the_sheet_names_the_notebook_it_is_about_to_change() {
        let (mapped, mapped_input, _) = ask_first_risk(
            "NotebookEdit",
            &json!({"notebook_path": "/Users/a/analysis.ipynb"}),
        )
        .expect("a notebook edit is gated");
        assert_eq!(
            permission_policy::describe_action(mapped, &mapped_input),
            "Change analysis.ipynb"
        );
    }

    /// LAC-8384 item 3: the cooldown key.
    ///
    /// Two gates, two keys, on purpose. The send gate asks about a connector
    /// tool, where a retry is the same question; "Ask me first" asks about a
    /// file, where it is not.
    #[test]
    fn the_cooldown_key_tells_two_files_apart() {
        assert_eq!(
            denial_key("Edit", &json!({"file_path": "/Users/a/one.md"})),
            "Edit:/Users/a/one.md"
        );
        assert_ne!(
            denial_key("Edit", &json!({"file_path": "/Users/a/one.md"})),
            denial_key("Edit", &json!({"file_path": "/Users/a/two.md"})),
            "declining one edit must not auto-deny every other file"
        );
        assert_eq!(
            denial_key(BASH_TOOL, &json!({"command": "rm old.txt"})),
            "Bash:rm old.txt"
        );
        assert_eq!(
            denial_key(
                "NotebookEdit",
                &json!({"notebook_path": "/Users/a/n.ipynb"})
            ),
            "NotebookEdit:/Users/a/n.ipynb"
        );
        // Nothing to key on: the tool name is the whole key, which is what
        // the send gate uses anyway.
        assert_eq!(denial_key("Write", &json!({})), "Write");
    }

    /// LAC-8384 item 6: `Task` stays pre-approved, and this is the measurement
    /// that settles it rather than an assumption.
    ///
    /// The question was whether a subagent's `Edit` calls come back through
    /// the parent's permission prompt, or run unseen inside the subagent.
    /// Measured against `claude` 2.1.325 with a stub MCP server standing in
    /// for Juno's: a `Task` subagent's `Write` arrived at `approve` as
    /// `tool_name: "Write"` with its own `file_path`, and the denial stopped
    /// it, and the subagent reported the refusal back verbatim. So the subagent's
    /// file edits are gated per call exactly like the parent's, and `Task`
    /// being pre-approved gates nothing away.
    #[test]
    fn task_is_pre_approved_because_its_subagents_tools_are_gated_per_call() {
        assert!(ALLOWED_TOOLS.split(',').any(|t| t == "Task"));
        // Which is only safe because the tools a subagent reaches for are the
        // same names this module gates.
        for tool in ["Write", "Edit", BASH_TOOL] {
            assert!(
                ask_first_risk(tool, &json!({"file_path": "/Users/a/x", "command": "rm x"}))
                    .is_some(),
                "{tool} must be gated for a subagent as well as the parent"
            );
        }
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

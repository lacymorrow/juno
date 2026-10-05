//! # One long-lived `claude` process per conversation (on by default)
//!
//! The one-shot path in [`super::claude_cli`] spawns a whole `claude` process per
//! message. Measured on an M-series Mac against CLI 2.1.278, that costs 1.6–3.1
//! seconds of Node startup, settings cascade, plugin and skill discovery, CLAUDE.md
//! discovery, MCP handshake and transcript read — on every single follow-up, before
//! the model has seen a token. Keeping one process alive and feeding it messages on
//! stdin drops that to roughly zero.
//!
//! The full spike, with numbers and the protocol evidence, is in
//! `docs/plans/cli-persistent-session-spike.md`. The three things worth repeating
//! here, because they are what the code below is shaped around:
//!
//! ## 1. Cancelling no longer means killing
//!
//! A `control_request` with subtype `interrupt` ends the running turn and leaves the
//! process alive and immediately reusable — measured at 10–35 ms, mid-generation and
//! mid-tool-call alike. That is strictly better than today, where escape destroys the
//! process and the next message pays a cold boot plus a `--resume`.
//!
//! ## 2. A persistent process starts turns nobody asked for
//!
//! This is the one genuinely new hazard, and it does not exist in the one-shot model.
//! A background `Bash` task finishing wakes the CLI up on its own: it runs a turn,
//! spends tokens, and writes a `result` frame with no user message behind it. A reader
//! that waits for "the next result" will hand that to the user as the answer to their
//! next question. It did, in the spike.
//!
//! So every message Juno sends carries its own `uuid`, and the CLI echoes it back as
//! `command_uuid` on `command_lifecycle` frames. A turn is Juno's if and only if such
//! a frame opened it. Everything else on the stream is dropped. Unsolicited turns
//! carry no lifecycle frames at all, which is what makes the test exact.
//!
//! ## 3. Nothing is lost when this fails
//!
//! The process is pinned to a session id — `--session-id` for a fresh one, `--resume`
//! for one that already exists — and the CLI still writes that session's transcript to
//! disk. So a dead, reaped, or disabled persistent session degrades to the existing
//! one-shot `--resume` path with the full conversation intact. Every failure here
//! returns [`TurnOutcome::Unavailable`] and the caller falls back.
//!
//! ## Shape
//!
//! One process per conversation, never one global process: the CLI session *is* the
//! conversation, messages in one process run strictly in order, and `interrupt` is
//! process-wide — so a shared process would leak history between chats, serialize
//! every conversation behind every other, and let escape cancel the wrong one.
//!
//! Each session owns two tasks and exactly one async lock:
//!
//! - a writer task owning the child's stdin, fed by an unbounded channel, so nothing
//!   ever has to hold a lock to send;
//! - a reader task owning the child's stdout, parsing NDJSON into a channel;
//! - `inbox`, the receiving half, behind the session's only `TokioMutex`. Locking it
//!   both serializes turns and hands the turn its frames, so there is no second lock
//!   to acquire and no ordering to get wrong.

use std::collections::HashMap;
use std::path::Path;
use std::process::Stdio;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, OnceLock};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use serde_json::{json, Value};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::sync::{mpsc, Mutex as TokioMutex};
use tracing::{debug, info, warn};

use super::claude_cli::claude_command;
use crate::agent::core::AgentError;
use crate::constants::settings::{store_keys, SETTINGS_STORE_FILE};

/// How long to wait for the CLI to acknowledge a message with a
/// `command_lifecycle` frame carrying our own `command_uuid`.
///
/// Generous, because the first turn of a session pays the CLI's whole cold boot
/// before its input loop answers. If nothing arrives the session is unusable —
/// an older CLI without `msg_lifecycle_v1`, or a wedged process — and the turn
/// falls back to the one-shot path having emitted nothing.
const LIFECYCLE_ACK_TIMEOUT: Duration = Duration::from_secs(45);

/// Ceiling on one turn. Matches the one-shot path's `CLI_TIMEOUT`, but applies to
/// the turn rather than the process: a turn that overruns is interrupted, not killed.
const TURN_TIMEOUT: Duration = Duration::from_secs(300);

/// How long to wait for the aborted `result` after sending an interrupt before
/// giving up and killing the process. Measured at 10–35 ms in practice.
const INTERRUPT_GRACE: Duration = Duration::from_secs(10);

/// Kill a session that has not run a turn in this long.
///
/// Bounds both the process leak and the window in which an idle process can wake
/// itself up and act unasked (see the module docs).
const IDLE_REAP_AFTER: Duration = Duration::from_secs(600);

/// How often the reaper looks.
const REAP_INTERVAL: Duration = Duration::from_secs(60);

/// How many `claude` processes may be alive at once.
///
/// Small on purpose. Each one is a Node process holding an MCP connection, and the
/// session id is what actually carries the conversation — evicting a process costs
/// one cold boot on the next message, not any context.
const MAX_LIVE_SESSIONS: usize = 3;

/// Environment that turns off the CLI's own background machinery for a
/// persistent process. Verified against `claude` 2.1.289: `DISABLE_CRON`
/// removes the Cron tools from the session; `DISABLE_BACKGROUND_TASKS` turns
/// off `run_in_background` and background agent launches.
const UNSOLICITED_TURN_ENV: [(&str, &str); 2] = [
    ("CLAUDE_CODE_DISABLE_BACKGROUND_TASKS", "1"),
    ("CLAUDE_CODE_DISABLE_CRON", "1"),
];

/// Tools that exist to wake a session later, which is exactly what a
/// persistent process must not do on its own. `--disallowedTools` removes
/// them from the session (verified against `claude` 2.1.289).
const WAKE_TOOLS: &str = "ScheduleWakeup,Monitor";

/// What a turn produced, or that there was no persistent session to run it in.
pub enum TurnOutcome {
    /// The turn ran to completion in the persistent process. Display text, tags stripped.
    Completed(String),
    /// No persistent session was available. The caller should use the one-shot path,
    /// which resumes the same session id, so nothing has been lost. Nothing was
    /// emitted to the UI.
    Unavailable,
}

/// Everything a turn needs. Borrowed, because the caller owns all of it already.
pub struct TurnRequest<'a> {
    pub binary: &'a Path,
    pub model: &'a str,
    /// The CLI's `--effort`, resolved the same way the one-shot path does.
    pub effort: &'a str,
    /// Ask for raw streaming events, so text reaches the screen and the TTS
    /// splitter a few words at a time instead of one whole block at a time.
    /// False only for a `claude` build the one-shot path has already seen
    /// reject the flag.
    pub include_partial_messages: bool,
    pub system_prompt: Option<&'a str>,
    pub mcp_config: Option<&'a Path>,
    /// Extra system-prompt guidance, appended only when `mcp_config` is present.
    pub mcp_guidance: &'a str,
    /// Whether the CLI may load the MCP servers on the person's own Claude
    /// account. When false, `spawn_args` passes `--strict-mcp-config` so only
    /// what `mcp_config` names loads. Spawn-time: part of the session
    /// signature, so flipping the setting replaces the process (LAC-4056).
    pub load_account_mcp: bool,
    pub conversation_id: &'a str,
    /// The CLI session this conversation runs in.
    pub session_id: &'a str,
    /// True when no session on disk holds `session_id` yet, so the process pins it
    /// with `--session-id`. An id that already exists must be continued with
    /// `--resume` instead: `--session-id` on an existing session is a hard error
    /// ("Session ID <id> is already in use"), verified against CLI 2.1.278.
    pub session_is_new: bool,
    pub query: &'a str,
    pub app_handle: &'a tauri::AppHandle,
    pub message_id: Option<String>,
    pub cancel_rx: Option<crate::state::CancelReceiver>,
    /// Per-send approval (LAC-4058): route the CLI's permission prompts into
    /// Juno's approval sheet instead of skipping permissions.
    pub ask_before_send: bool,
}

/// Is the persistent-session path turned on?
///
/// Default true. A person who has never touched the switch has no value in the
/// store and gets the default; one who turned it off has an explicit `false`
/// that is respected. Read fresh from the store rather than cached, so turning
/// it off needs no restart.
pub fn is_enabled(app: &tauri::AppHandle) -> bool {
    use tauri_plugin_store::StoreExt;
    let stored = app
        .store(SETTINGS_STORE_FILE)
        .ok()
        .and_then(|store| store.get(store_keys::CLI_PERSISTENT_SESSION_ENABLED));
    enabled_from_store(stored.as_ref())
}

/// The flag's value given what the store holds. Anything but an explicit
/// boolean, including nothing at all, is the default.
fn enabled_from_store(stored: Option<&Value>) -> bool {
    stored
        .and_then(Value::as_bool)
        .unwrap_or(crate::constants::settings::defaults::CLI_PERSISTENT_SESSION_ENABLED)
}

/// Read the persistent-session flag.
#[tauri::command]
pub async fn get_cli_persistent_session_enabled(
    app_handle: tauri::AppHandle,
) -> Result<bool, String> {
    Ok(is_enabled(&app_handle))
}

/// Turn the persistent-session path on or off.
///
/// Turning it off kills every live process immediately rather than waiting for the
/// reaper, because "off" should mean off. Nothing is lost: each conversation's CLI
/// session id is already in the one-shot path's registry, so the next message
/// resumes it.
#[tauri::command]
pub async fn set_cli_persistent_session_enabled(
    app_handle: tauri::AppHandle,
    enabled: bool,
) -> Result<(), String> {
    use tauri_plugin_store::StoreExt;
    let store = app_handle
        .store(SETTINGS_STORE_FILE)
        .map_err(|e| format!("Failed to access settings store: {e}"))?;
    store.set(
        store_keys::CLI_PERSISTENT_SESSION_ENABLED,
        Value::Bool(enabled),
    );
    store
        .save()
        .map_err(|e| format!("Failed to save settings store: {e}"))?;

    if !enabled {
        shutdown_all().await;
    }
    info!(
        "[CliSession] Persistent Claude CLI sessions {}",
        if enabled { "enabled" } else { "disabled" }
    );
    Ok(())
}

/// A live `claude` process, and the conversation it is pinned to.
struct CliSession {
    /// The `--session-id` this process was spawned with. Shared with the one-shot
    /// path's registry, so a fallback resumes exactly this conversation.
    session_id: String,
    /// Model, system prompt and MCP config are spawn-time arguments — a live process
    /// keeps whatever it was born with. When they change, the session is torn down
    /// and respawned, which costs one cold boot and loses nothing.
    signature: String,
    /// Lines to write to the child's stdin. Cloneable, so sending needs no lock.
    to_child: mpsc::UnboundedSender<String>,
    /// Frames from the child, and the lock that serializes turns. The session's
    /// only async mutex.
    inbox: TokioMutex<mpsc::UnboundedReceiver<Value>>,
    /// Held so the process can be health-checked and killed. A `std` mutex because
    /// both `try_wait` and `start_kill` are non-blocking and never held across an await.
    child: std::sync::Mutex<tokio::process::Child>,
    /// Unix seconds of the last turn, for the idle reaper.
    last_used: AtomicU64,
}

impl CliSession {
    /// Has the process exited?
    fn is_dead(&self) -> bool {
        match self.child.lock() {
            Ok(mut child) => !matches!(child.try_wait(), Ok(None)),
            // A poisoned lock means a panic happened while holding it. Treat the
            // session as unusable rather than reasoning about what state it is in.
            Err(_) => true,
        }
    }

    fn kill(&self) {
        if let Ok(mut child) = self.child.lock() {
            let _ = child.start_kill();
        }
    }

    fn touch(&self) {
        self.last_used.store(now_secs(), Ordering::Relaxed);
    }

    fn idle_for(&self) -> Duration {
        Duration::from_secs(now_secs().saturating_sub(self.last_used.load(Ordering::Relaxed)))
    }
}

fn now_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_else(|_| Duration::from_secs(0))
        .as_secs()
}

type Registry = TokioMutex<HashMap<String, Arc<CliSession>>>;

static SESSIONS: OnceLock<Registry> = OnceLock::new();

fn sessions() -> &'static Registry {
    SESSIONS.get_or_init(|| TokioMutex::new(HashMap::new()))
}

/// Kill every live session. Call from the app's exit path.
///
/// `kill_on_drop` covers a drop on a live runtime; it does not cover the app being
/// torn down around these processes, which is what this is for.
pub async fn shutdown_all() {
    let mut map = sessions().lock().await;
    for (conversation, session) in map.drain() {
        debug!("[CliSession] Shutting down session for conversation {conversation}");
        session.kill();
    }
}

static REAPER_STARTED: OnceLock<()> = OnceLock::new();

/// Start the idle reaper, once, the first time a persistent session is wanted.
///
/// Self-starting rather than wired into app setup so that a feature nobody has
/// turned on costs nothing and touches nothing.
fn ensure_reaper() {
    if REAPER_STARTED.set(()).is_ok() {
        tauri::async_runtime::spawn(async {
            loop {
                tokio::time::sleep(REAP_INTERVAL).await;
                reap_idle().await;
            }
        });
    }
}

/// Kill sessions that have gone quiet. Safe to call on a timer.
pub async fn reap_idle() {
    let mut map = sessions().lock().await;
    map.retain(|conversation, session| {
        let stale = session.idle_for() >= IDLE_REAP_AFTER || session.is_dead();
        if stale {
            info!("[CliSession] Reaping idle session for conversation {conversation}");
            session.kill();
        }
        !stale
    });
}

/// Forget (and kill) the session for one conversation.
async fn evict(conversation_id: &str) {
    let mut map = sessions().lock().await;
    if let Some(session) = map.remove(conversation_id) {
        session.kill();
    }
}

/// The spawn-time arguments a live process cannot be talked out of.
///
/// Separated by unit separators so a model named `a` with the prompt `b` cannot
/// collide with a model literally named `a<US>b`.
fn signature_parts(
    model: &str,
    system_prompt: Option<&str>,
    mcp_config: Option<&Path>,
    load_account_mcp: bool,
    ask_before_send: bool,
) -> String {
    format!(
        "{}\u{1f}{}\u{1f}{}\u{1f}{}\u{1f}{}",
        model,
        system_prompt.unwrap_or_default(),
        mcp_config
            .map(|p| p.to_string_lossy().into_owned())
            .unwrap_or_default(),
        // Both are spawn-time postures a live process cannot be talked out
        // of: --strict-mcp-config (LAC-4056) and the permission flags
        // (LAC-4058). Flipping either must replace the process.
        load_account_mcp,
        ask_before_send
    )
}

fn signature_of(req: &TurnRequest<'_>) -> String {
    // Effort and the partial-messages flag are spawn-time too. Appended rather
    // than threaded through `signature_parts`, whose shape the tests pin.
    format!(
        "{}\u{1f}{}\u{1f}{}",
        signature_parts(
            req.model,
            req.system_prompt,
            req.mcp_config,
            req.load_account_mcp,
            req.ask_before_send,
        ),
        req.effort,
        req.include_partial_messages
    )
}

/// The session for this conversation, spawning one if needed, and whether it was
/// already running (warm) rather than started for this turn.
///
/// Returns `None` when a persistent session could not be had, which is never fatal:
/// the caller falls back to the one-shot path.
async fn acquire(req: &TurnRequest<'_>) -> Option<(Arc<CliSession>, bool)> {
    ensure_reaper();
    let signature = signature_of(req);

    // Check-then-spawn, with the registry lock released across the spawn. The lock
    // is reacquired and rechecked afterwards so two turns racing on a cold
    // conversation do not leave an orphan process behind.
    {
        let map = sessions().lock().await;
        if let Some(existing) = map.get(req.conversation_id) {
            let usable = existing.signature == signature
                && existing.session_id == req.session_id
                && !existing.is_dead();
            if usable {
                return Some((Arc::clone(existing), true));
            }
        }
    }

    // Whatever was there is stale, dead, or configured differently.
    evict(req.conversation_id).await;

    let spawned = match spawn_session(req, signature.clone()) {
        Ok(session) => Arc::new(session),
        Err(e) => {
            warn!("[CliSession] Could not start a persistent Claude CLI session: {e}");
            return None;
        }
    };

    let mut map = sessions().lock().await;
    if let Some(existing) = map.get(req.conversation_id) {
        if existing.signature == signature && !existing.is_dead() {
            // Another turn won the race. Ours is surplus.
            spawned.kill();
            return Some((Arc::clone(existing), false));
        }
    }

    if map.len() >= MAX_LIVE_SESSIONS {
        // Evict the least recently used. Costs that conversation one cold boot on
        // its next message; its session id, and so its history, is untouched.
        let victim = map
            .iter()
            .max_by_key(|(_, s)| s.idle_for())
            .map(|(id, _)| id.clone());
        if let Some(id) = victim {
            if let Some(session) = map.remove(&id) {
                info!("[CliSession] Evicting least recently used session ({id})");
                session.kill();
            }
        }
    }

    map.insert(req.conversation_id.to_string(), Arc::clone(&spawned));
    Some((spawned, false))
}

/// Spawn the process and the two tasks that own its pipes.
fn spawn_session(req: &TurnRequest<'_>, signature: String) -> Result<CliSession, AgentError> {
    let args = spawn_args(req);
    info!(
        "[CliSession] Starting persistent Claude CLI session {} ({}) for conversation {}",
        req.session_id,
        if req.session_is_new { "new" } else { "resumed" },
        req.conversation_id
    );
    debug!("[CliSession] args: {}", args.join(" "));

    let mut child = claude_command(req.binary)
        .args(&args)
        // A process that outlives its turn can be woken by its own background
        // work: a finished background Bash task or subagent, a cron job, a
        // scheduled wakeup. Each starts a turn nobody asked for, which lifecycle
        // correlation hides but cannot stop from spending tokens or acting on
        // the desktop (spike, Q2a). Switch those sources off at the root.
        // Unknown to an older CLI, which simply ignores them.
        .envs(UNSOLICITED_TURN_ENV)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true)
        .spawn()
        .map_err(|e| AgentError::LlmError(format!("Failed to spawn Claude CLI: {e}")))?;

    let stdin = child
        .stdin
        .take()
        .ok_or_else(|| AgentError::LlmError("Claude CLI gave us no stdin".to_string()))?;
    let stdout = child
        .stdout
        .take()
        .ok_or_else(|| AgentError::LlmError("Claude CLI gave us no stdout".to_string()))?;
    let stderr = child.stderr.take();

    let (to_child, mut outbox) = mpsc::unbounded_channel::<String>();
    let (frames_tx, frames_rx) = mpsc::unbounded_channel::<Value>();

    // Writer: owns stdin so nothing else ever needs a lock to send a message.
    tauri::async_runtime::spawn(async move {
        let mut stdin = stdin;
        while let Some(line) = outbox.recv().await {
            if stdin.write_all(line.as_bytes()).await.is_err() || stdin.flush().await.is_err() {
                debug!("[CliSession] stdin closed; writer stopping");
                break;
            }
        }
        // Dropping stdin is how the CLI is asked to shut down cleanly (it exits 0).
    });

    // Reader: NDJSON off stdout into the inbox. EOF drops the sender, which is how
    // a turn learns the process is gone rather than waiting forever.
    tauri::async_runtime::spawn(async move {
        let mut lines = BufReader::new(stdout).lines();
        loop {
            match lines.next_line().await {
                Ok(Some(line)) => {
                    if line.trim().is_empty() {
                        continue;
                    }
                    match serde_json::from_str::<Value>(&line) {
                        Ok(value) => {
                            if frames_tx.send(value).is_err() {
                                break;
                            }
                        }
                        Err(e) => debug!(
                            "[CliSession] Non-JSON line: {} ({e})",
                            line.chars().take(80).collect::<String>()
                        ),
                    }
                }
                Ok(None) => {
                    debug!("[CliSession] stdout EOF");
                    break;
                }
                Err(e) => {
                    warn!("[CliSession] Error reading stdout: {e}");
                    break;
                }
            }
        }
    });

    // Drain stderr or a >64KB write there would block the child forever.
    if let Some(stderr) = stderr {
        tauri::async_runtime::spawn(async move {
            let mut lines = BufReader::new(stderr).lines();
            while let Ok(Some(line)) = lines.next_line().await {
                if !line.trim().is_empty() {
                    debug!(
                        "[CliSession] stderr: {}",
                        line.chars().take(300).collect::<String>()
                    );
                }
            }
        });
    }

    Ok(CliSession {
        session_id: req.session_id.to_string(),
        signature,
        to_child,
        inbox: TokioMutex::new(frames_rx),
        child: std::sync::Mutex::new(child),
        last_used: AtomicU64::new(now_secs()),
    })
}

/// The spawn arguments. Deliberately close to the one-shot path's `build_args`. The
/// only real differences are the stdin input format and how the session is named.
fn spawn_args(req: &TurnRequest<'_>) -> Vec<String> {
    let mut args = vec![
        "-p".to_string(),
        // Messages arrive on stdin for the life of the process rather than as a
        // positional argument for the life of one query.
        "--input-format".to_string(),
        "stream-json".to_string(),
        "--output-format".to_string(),
        "stream-json".to_string(),
        "--model".to_string(),
        req.model.to_string(),
        // Same per-turn effort as the one-shot path. Without it a persistent
        // turn would run at the CLI's own default instead of Juno's.
        "--effort".to_string(),
        req.effort.to_string(),
        "--disallowedTools".to_string(),
        WAKE_TOOLS.to_string(),
    ];

    // Raw streaming events, as on the one-shot path. Without them a persistent
    // turn delivers each text block whole, once the model has finished it, so
    // speech cannot start until the whole first block exists.
    if req.include_partial_messages {
        args.push(super::claude_cli::PARTIAL_MESSAGES_FLAG.to_string());
    }

    // Mirrors the one-shot path's build_args (LAC-4056): with "Load account
    // MCP connectors" off, --strict-mcp-config keeps the person's claude.ai
    // connectors and user-level servers out; on, the default, omits the flag.
    // Same extra case as `build_args`: with no MCP server there is nothing
    // to answer a permission prompt, so connectors must not load into a
    // spawn that falls back to --dangerously-skip-permissions.
    if !req.load_account_mcp || (req.ask_before_send && req.mcp_config.is_none()) {
        args.push("--strict-mcp-config".to_string());
    }

    // Permission posture (LAC-4058): same rule as the one-shot path's
    // `build_args`, through the same seam.
    args.extend(super::cli_approval::permission_args(
        req.ask_before_send,
        req.mcp_config.is_some(),
    ));

    // A fresh id is pinned; one that already exists must be resumed. Getting this
    // backwards is a hard error from the CLI, not a fallback: it refuses to start
    // with "Session ID <id> is already in use". Either way the CLI keeps writing the
    // transcript to disk, so the one-shot path can `--resume` this exact id if the
    // process dies. `--no-session-persistence` would remove that safety net and must
    // never be passed here.
    if req.session_is_new {
        args.push("--session-id".to_string());
    } else {
        args.push("--resume".to_string());
    }
    args.push(req.session_id.to_string());

    if let Some(mcp_path) = req.mcp_config {
        args.push("--mcp-config".to_string());
        args.push(mcp_path.to_string_lossy().into_owned());
        args.push("--append-system-prompt".to_string());
        args.push(req.mcp_guidance.to_string());
    }

    if let Some(prompt) = req.system_prompt {
        args.push("--system-prompt".to_string());
        args.push(prompt.to_string());
    }

    args
}

/// Run one turn in this conversation's persistent process.
///
/// Emits the same Tauri streaming events as the one-shot path. Returns
/// [`TurnOutcome::Unavailable`], having emitted nothing, whenever a persistent
/// session could not be used — the caller then runs the turn the old way.
pub async fn run_turn(req: TurnRequest<'_>) -> Result<TurnOutcome, AgentError> {
    let Some((session, warm)) = acquire(&req).await else {
        return Ok(TurnOutcome::Unavailable);
    };

    // Locking the inbox is what serializes turns: one at a time per process, which
    // is also the only order the CLI itself will run them in.
    let mut inbox = session.inbox.lock().await;

    if session.is_dead() {
        drop(inbox);
        evict(req.conversation_id).await;
        return Ok(TurnOutcome::Unavailable);
    }

    // Anything still on the stream belongs to a turn that is over, or to a turn the
    // CLI started by itself. Neither is ours. Clear it so the lifecycle scan below
    // starts from a clean stream.
    let mut discarded = 0usize;
    while inbox.try_recv().is_ok() {
        discarded += 1;
    }
    if discarded > 0 {
        debug!("[CliSession] Discarded {discarded} stale frame(s) before this turn");
    }

    let command_uuid = uuid::Uuid::new_v4().to_string();
    let message = json!({
        "type": "user",
        // The CLI echoes this back as `command_uuid` on command_lifecycle frames.
        // It is the only thing that distinguishes this turn from one the CLI
        // decided to run on its own. Without it there is no safe reader.
        "uuid": command_uuid,
        "message": { "role": "user", "content": [{ "type": "text", "text": req.query }] }
    });

    let line = match serde_json::to_string(&message) {
        Ok(line) => format!("{line}\n"),
        Err(e) => {
            return Err(AgentError::LlmError(format!(
                "Could not encode the query for the Claude CLI: {e}"
            )))
        }
    };

    if session.to_child.send(line).is_err() {
        drop(inbox);
        evict(req.conversation_id).await;
        return Ok(TurnOutcome::Unavailable);
    }
    session.touch();
    // A cold process pays its whole boot before the first token; a warm one
    // does not. The timing line says which, so the two are never averaged.
    crate::turn_timing::note_llm(
        if warm {
            "claude_cli/persistent-warm"
        } else {
            "claude_cli/persistent-cold"
        },
        req.model,
    );
    crate::turn_timing::mark(crate::turn_timing::Stage::LlmRequestSent);

    let outcome = stream_turn(&session, &mut inbox, &req, &command_uuid).await;
    session.touch();
    drop(inbox);

    // One rule, because the alternative is a class of bug rather than a bug: if the
    // persistent path did not carry this turn to completion, the process dies.
    //
    // It is not only that a session which failed once is not to be trusted again.
    // The message has already been written to its stdin, so a process left alive
    // could still run it — after the one-shot fallback has answered the same query,
    // to a user who asked once. Killing the process is what makes the fallback safe.
    match outcome {
        Ok(TurnOutcome::Completed(text)) => Ok(TurnOutcome::Completed(text)),
        Ok(TurnOutcome::Unavailable) => {
            evict(req.conversation_id).await;
            Ok(TurnOutcome::Unavailable)
        }
        Err(e) => {
            evict(req.conversation_id).await;
            Err(e)
        }
    }
}

/// Whether a frame is the `command_lifecycle` acknowledgement of *our* turn.
///
/// This is the correlation the whole persistent path hangs on: a turn is rendered
/// if and only if a `command_lifecycle` frame carrying the exact `uuid` we put on
/// the user message opens it. Anything else — a different uuid, a missing uuid, a
/// non-string uuid, another frame type — is a turn the CLI started on its own
/// (observed: a finishing background Bash task) and must stay invisible.
fn lifecycle_frame_is_ours(frame: &Value, command_uuid: &str) -> bool {
    frame.get("type").and_then(Value::as_str) == Some("command_lifecycle")
        && frame.get("command_uuid").and_then(Value::as_str) == Some(command_uuid)
}

/// How a turn that Juno owns ended, when it did not complete.
struct TurnFailure {
    error: AgentError,
    /// What the bubble is left showing.
    shown: &'static str,
}

impl TurnFailure {
    fn cancelled() -> Self {
        Self {
            error: AgentError::Terminated,
            shown: "Cancelled",
        }
    }
}

/// Read the turn off the stream, emitting as it goes.
///
/// Split out from [`run_turn`] so every exit path there can decide whether the
/// session survives, in one place.
///
/// The frames inside a turn are the one-shot path's frames, so they go through
/// the one-shot path's handlers: `stream_event` text deltas reach the screen and
/// the TTS splitter as they arrive, tool and reasoning indicators rise and fall
/// the same way, and the whole-message `assistant` frame is only a fallback.
async fn stream_turn(
    session: &CliSession,
    inbox: &mut mpsc::UnboundedReceiver<Value>,
    req: &TurnRequest<'_>,
    command_uuid: &str,
) -> Result<TurnOutcome, AgentError> {
    use super::claude_cli::{clear_finished_tools, clear_pending_tools, close_open_thinking};
    use super::claude_cli::{ClaudeCliBrain, StreamBlock};
    use std::collections::HashSet;

    // Prefer the run's own cancellation channel. For session-tracked runs that is
    // the merged session+global receiver — escape cancels only the focused session,
    // whose token the global channel never sees (LAC-3697) — and the global AppState
    // channel is the fallback for callers that thread none through.
    //
    // A never-firing channel stands in when there is neither, so the select below
    // needs no second shape. `_keep` holds its sender alive.
    let (_keep, never_cancels) = tokio::sync::watch::channel(false);
    let mut cancel_rx = req
        .cancel_rx
        .clone()
        .or_else(|| {
            use tauri::Manager;
            req.app_handle
                .try_state::<crate::state::AppState>()
                .map(|state| state.cancel_rx.clone())
        })
        .unwrap_or(never_cancels);

    let msg_id = req
        .message_id
        .clone()
        .unwrap_or_else(|| uuid::Uuid::new_v4().to_string());
    // The shared handlers take the one-shot path's optional handle.
    let app = Some(req.app_handle.clone());

    // The bubble. Opened only at our `started` frame, and closed however this
    // function is left once it is open, including a process that dies mid-turn.
    let mut surface = TurnSurface::new(req.app_handle, msg_id.clone());

    // "Ours" begins at the command_lifecycle `started` frame carrying our uuid, and
    // nothing before it is rendered. Until then the turn can still fall back to the
    // one-shot path without the UI having seen anything.
    let mut ours = false;
    // Set once, when the interrupt goes out, so the grace window cannot be pushed
    // forward forever by a process that keeps talking but never ends the turn.
    let mut cancel_deadline: Option<Instant> = None;

    let mut accumulated = String::new();
    let mut final_result: Option<String> = None;
    let mut tts = crate::agent::tts_tags::TtsTagStream::new();
    let mut spoken: Vec<String> = Vec::new();

    // Partial-message bookkeeping, exactly as the one-shot `process_stream`
    // keeps it. Fresh per turn: block indices and message ids restart.
    let mut blocks: HashMap<u64, StreamBlock> = HashMap::new();
    let mut pending_tools: HashMap<String, String> = HashMap::new();
    let mut streamed_messages: HashSet<String> = HashSet::new();
    let mut partial_message_id: Option<String> = None;
    let mut saw_text_delta = false;
    // Whole-message fallback: `assistant` frames are cumulative per message, so
    // the char cursor restarts for each new message in a tool loop.
    let mut fallback_message_id: Option<String> = None;
    let mut previous_chars: usize = 0;

    let ack_deadline = Instant::now() + LIFECYCLE_ACK_TIMEOUT;
    let turn_deadline = Instant::now() + TURN_TIMEOUT;

    let failure: Option<TurnFailure> = loop {
        let deadline = match (cancel_deadline, ours) {
            (Some(deadline), _) => deadline,
            (None, true) => turn_deadline,
            (None, false) => ack_deadline,
        };

        let received = if cancel_deadline.is_some() {
            // Already interrupted: drain to our aborted result, nothing to race.
            tokio::time::timeout_at(deadline.into(), inbox.recv()).await
        } else {
            tokio::select! {
                frame = tokio::time::timeout_at(deadline.into(), inbox.recv()) => frame,
                // Loop on changed() + borrow() rather than wait_for(), to avoid
                // holding a non-Send guard across the select boundary.
                _ = async {
                    loop {
                        if *cancel_rx.borrow() { return; }
                        if cancel_rx.changed().await.is_err() {
                            std::future::pending::<()>().await;
                        }
                    }
                } => {
                    info!("[CliSession] Cancelled; interrupting the turn (process stays alive)");
                    send_interrupt(session);
                    cancel_deadline = Some(Instant::now() + INTERRUPT_GRACE);
                    continue;
                }
            }
        };

        let frame = match received {
            Ok(Some(frame)) => frame,
            // The sender was dropped: the reader saw EOF, so the process is gone.
            Ok(None) => {
                if !ours {
                    return Ok(TurnOutcome::Unavailable);
                }
                break Some(TurnFailure {
                    error: AgentError::LlmError(
                        "The Claude CLI session ended mid-turn".to_string(),
                    ),
                    shown: "Error: The Claude CLI session ended mid-turn",
                });
            }
            Err(_elapsed) => {
                if cancel_deadline.is_some() {
                    // The interrupt was not honoured. Killing is the fallback, and
                    // the conversation survives it via the session id.
                    warn!("[CliSession] Interrupt went unanswered; killing the process");
                    session.kill();
                    break Some(TurnFailure::cancelled());
                }
                if !ours {
                    // No lifecycle frame for our uuid within the window: an older CLI
                    // without msg_lifecycle_v1, or a wedged process. Nothing was
                    // emitted, so the one-shot path can take this turn — but only
                    // because run_turn kills this process on the way out. The message
                    // is already on its stdin, and a live process would still run it.
                    warn!(
                        "[CliSession] No command_lifecycle frame after {}s; falling back",
                        LIFECYCLE_ACK_TIMEOUT.as_secs()
                    );
                    return Ok(TurnOutcome::Unavailable);
                }
                // A turn that overran is interrupted, not killed.
                warn!(
                    "[CliSession] Turn exceeded {}s; interrupting",
                    TURN_TIMEOUT.as_secs()
                );
                send_interrupt(session);
                break Some(TurnFailure {
                    error: AgentError::Timeout(format!(
                        "Claude CLI timed out after {} seconds",
                        TURN_TIMEOUT.as_secs()
                    )),
                    shown: "Claude CLI timed out",
                });
            }
        };

        let frame_type = frame.get("type").and_then(Value::as_str).unwrap_or("");

        if frame_type == "command_lifecycle" {
            if !lifecycle_frame_is_ours(&frame, command_uuid) {
                continue;
            }
            match frame.get("state").and_then(Value::as_str) {
                Some("started") => {
                    ours = true;
                    surface.open();
                }
                Some("cancelled") => break Some(TurnFailure::cancelled()),
                _ => {}
            }
            continue;
        }

        if frame_type == "control_response" {
            debug!("[CliSession] control_response: {}", frame);
            continue;
        }

        if !ours {
            // A turn the CLI started by itself, or the tail of an earlier one.
            // Never rendered; see the module docs.
            if frame_type == "result" {
                debug!("[CliSession] Ignoring a result from a turn Juno did not ask for");
            }
            continue;
        }

        match frame_type {
            "stream_event" => {
                if cancel_deadline.is_some() {
                    // The user asked to stop. Nothing more reaches the screen.
                    continue;
                }
                if let Some(event) = frame.get("event") {
                    ClaudeCliBrain::handle_stream_event(
                        event,
                        &app,
                        &msg_id,
                        &mut blocks,
                        &mut pending_tools,
                        &mut streamed_messages,
                        &mut partial_message_id,
                        &mut saw_text_delta,
                        &mut tts,
                        &mut accumulated,
                        &mut spoken,
                    );
                }
            }
            // Tool results come back as a `user` turn; their indicators come down.
            "user" => clear_finished_tools(&frame, &app, &msg_id, &mut pending_tools),
            "assistant" => {
                if cancel_deadline.is_some() {
                    continue;
                }
                crate::turn_timing::mark(crate::turn_timing::Stage::LlmFirstToken);
                let Some(message) = frame.get("message") else {
                    continue;
                };
                let message_id = message
                    .get("id")
                    .and_then(Value::as_str)
                    .map(str::to_string);
                // When the partial stream is live it has already shown and
                // spoken every word of this message. Diffing it again would
                // render each word twice and speak the answer twice over.
                let already_streamed = match message_id.as_ref() {
                    Some(id) => streamed_messages.contains(id),
                    None => saw_text_delta,
                };
                if already_streamed {
                    continue;
                }
                if fallback_message_id != message_id {
                    fallback_message_id = message_id;
                    previous_chars = 0;
                }
                let text = super::claude_cli::extract_text_from_message(message);
                let chars = text.chars().count();
                if chars <= previous_chars {
                    continue;
                }
                // Char offsets, never byte offsets: slicing bytes panics on
                // multi-byte UTF-8.
                let delta: String = text.chars().skip(previous_chars).collect();
                previous_chars = chars;
                ClaudeCliBrain::emit_display_text(
                    &app,
                    &msg_id,
                    &delta,
                    &mut tts,
                    &mut accumulated,
                    &mut spoken,
                );
            }
            "result" => {
                let subtype = frame.get("subtype").and_then(Value::as_str).unwrap_or("");
                let terminal = frame
                    .get("terminal_reason")
                    .and_then(Value::as_str)
                    .unwrap_or("");

                if let Some(ms) = frame.get("duration_ms").and_then(Value::as_u64) {
                    info!("[CliSession] Turn duration: {ms}ms");
                }
                if let Some(cost) = frame.get("total_cost_usd").and_then(Value::as_f64) {
                    info!("[CliSession] Turn cost: ${cost:.4}");
                }

                // `terminal_reason` is a closed protocol field, not an error message:
                // an interrupt ends a turn as `aborted_streaming` (mid-generation) or
                // `aborted_tools` (mid-tool-call). Both mean the user's stop landed
                // and nothing from this turn should be shown or spoken (LAC-3697).
                let aborted = matches!(terminal, "aborted_streaming" | "aborted_tools");
                if cancel_deadline.is_some() || aborted {
                    info!("[CliSession] Turn ended as {subtype}/{terminal}");
                    break Some(TurnFailure::cancelled());
                }

                if let Some(text) = frame.get("result").and_then(Value::as_str) {
                    final_result = Some(text.to_string());
                }
                break None;
            }
            // `system/init` arrives at the head of every turn, not once per process.
            // Nothing to do with it beyond noting the subtype.
            "system" => {
                // Hoisted out of the macro: tracing's expansion shadows `Value`
                // with its own field trait of the same name (E0782).
                let subtype = frame.get("subtype").and_then(Value::as_str).unwrap_or("?");
                debug!("[CliSession] system/{subtype}");
            }
            other => debug!("[CliSession] frame '{other}' skipped"),
        }
    };

    // Nothing may be left spinning however the turn ended: a thinking block the
    // CLI never stopped, or a tool whose result never came.
    close_open_thinking(&app, &mut blocks);
    clear_pending_tools(&app, &msg_id, &mut pending_tools);

    if let Some(failure) = failure {
        surface.close(failure.shown.to_string());
        return Err(failure.error);
    }

    // Flush the tag parser: a partial tag becomes display text, an unterminated
    // block is still spoken.
    let (tail_display, tail_spoken) = tts.finish();
    if !tail_display.is_empty() || !tail_spoken.is_empty() {
        accumulated.push_str(&tail_display);
        emit_chunk(req.app_handle, tail_display, &msg_id, &tail_spoken);
        spoken.extend(tail_spoken);
    }

    // The `result` frame carries the raw final text, tags included. Strip it for
    // display and speak any block the streamed text did not already cover.
    let final_result = final_result.map(|raw| {
        let (display, blocks) = crate::agent::tts_tags::split_tts_tags(&raw);
        let unspoken: Vec<String> = blocks.into_iter().filter(|b| !spoken.contains(b)).collect();
        if !unspoken.is_empty() {
            emit_chunk(req.app_handle, String::new(), &msg_id, &unspoken);
        }
        display
    });

    let complete = final_result.unwrap_or(accumulated);
    surface.close(complete.clone());
    Ok(TurnOutcome::Completed(complete))
}

/// Ask the CLI to end the running turn. The process stays alive and is immediately
/// reusable — that is the whole point of this module.
fn send_interrupt(session: &CliSession) {
    let request = json!({
        "type": "control_request",
        "request_id": format!("juno_int_{}", uuid::Uuid::new_v4()),
        "request": { "subtype": "interrupt" }
    });
    match serde_json::to_string(&request) {
        Ok(line) => {
            if session.to_child.send(format!("{line}\n")).is_err() {
                warn!("[CliSession] Could not deliver the interrupt; killing instead");
                session.kill();
            }
        }
        Err(e) => warn!("[CliSession] Could not encode the interrupt: {e}"),
    }
}

/// The assistant bubble for one persistent turn.
///
/// Opened late, at the `started` frame, because a turn that falls back before it
/// must leave no trace (the one-shot path opens its own). Once open it is closed
/// on every way out, including the ones nobody remembered: the frontend only
/// stops a spinner on `stream_end`.
struct TurnSurface<'a> {
    app_handle: &'a tauri::AppHandle,
    msg_id: String,
    started: bool,
    open: bool,
}

impl<'a> TurnSurface<'a> {
    fn new(app_handle: &'a tauri::AppHandle, msg_id: String) -> Self {
        Self {
            app_handle,
            msg_id,
            started: false,
            open: false,
        }
    }

    /// Open the bubble, once. A second `started` frame must not append a second
    /// empty bubble above the answer.
    fn open(&mut self) {
        if self.started {
            return;
        }
        self.started = true;
        self.open = true;
        crate::agent::tool_logger::emit_stream_start(self.app_handle, self.msg_id.clone());
    }

    /// Close it on the text the person should be left looking at. A no-op if it
    /// was never opened or is already closed.
    fn close(&mut self, text: String) {
        if !self.open {
            return;
        }
        self.open = false;
        crate::agent::tool_logger::emit_stream_end(self.app_handle, self.msg_id.clone(), text);
    }
}

impl Drop for TurnSurface<'_> {
    fn drop(&mut self) {
        if self.open {
            warn!("[CliSession] Turn ended without closing its stream; closing it empty");
            self.close(String::new());
        }
    }
}

/// One display chunk plus every spoken block, matching the one-shot path's event
/// shape: the first block rides on the text chunk, extras go out TTS-only.
fn emit_chunk(handle: &tauri::AppHandle, display: String, msg_id: &str, blocks: &[String]) {
    if display.is_empty() && blocks.is_empty() {
        return;
    }
    crate::agent::tool_logger::emit_streaming_text_chunk(
        handle,
        display,
        Some(msg_id.to_string()),
        blocks.first().cloned(),
    );
    for spoken in blocks.iter().skip(1) {
        crate::agent::tool_logger::emit_streaming_text_chunk(
            handle,
            String::new(),
            Some(msg_id.to_string()),
            Some(spoken.clone()),
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn signature_separates_model_from_prompt() {
        // A model named "a" with the prompt "b" must not collide with a model
        // literally named "a<US>b", or a prompt change would go unnoticed.
        assert_ne!(
            signature_parts("a", Some("b"), None, true, true),
            signature_parts("a\u{1f}b", None, None, true, true)
        );
    }

    #[test]
    fn signature_changes_with_every_spawn_time_argument() {
        let base = signature_parts("sonnet", None, None, true, true);
        assert_ne!(base, signature_parts("opus", None, None, true, true));
        assert_ne!(
            base,
            signature_parts("sonnet", Some("be brief"), None, true, true)
        );
        assert_ne!(
            base,
            signature_parts("sonnet", None, Some(Path::new("/tmp/a.json")), true, true)
        );
        // Toggling "Load account MCP connectors" changes --strict-mcp-config,
        // which a live process cannot be talked out of, so the session must
        // be replaced rather than reused (LAC-4056).
        assert_ne!(base, signature_parts("sonnet", None, None, false, true));
        // Same for "Ask before Juno sends": a live session would otherwise
        // keep the permission posture it was born with (LAC-4058).
        assert_ne!(base, signature_parts("sonnet", None, None, true, false));
    }

    #[test]
    fn signature_is_stable_for_identical_arguments() {
        assert_eq!(
            signature_parts(
                "sonnet",
                Some("p"),
                Some(Path::new("/tmp/a.json")),
                true,
                true
            ),
            signature_parts(
                "sonnet",
                Some("p"),
                Some(Path::new("/tmp/a.json")),
                true,
                true
            )
        );
    }

    #[test]
    fn the_flag_is_on_unless_someone_turned_it_off() {
        // Never touched: nothing in the store.
        assert!(enabled_from_store(None));
        // Turned off: respected.
        assert!(!enabled_from_store(Some(&Value::Bool(false))));
        assert!(enabled_from_store(Some(&Value::Bool(true))));
        // Junk in the store is not a decision.
        assert!(enabled_from_store(Some(&json!("false"))));
        assert!(enabled_from_store(Some(&Value::Null)));
    }

    #[test]
    fn a_persistent_process_cannot_wake_itself() {
        // The sources of turns nobody asked for are switched off at spawn.
        let env: Vec<&str> = UNSOLICITED_TURN_ENV.iter().map(|(k, _)| *k).collect();
        assert!(env.contains(&"CLAUDE_CODE_DISABLE_BACKGROUND_TASKS"));
        assert!(env.contains(&"CLAUDE_CODE_DISABLE_CRON"));
        assert!(UNSOLICITED_TURN_ENV.iter().all(|(_, v)| *v == "1"));
        let tools: Vec<&str> = WAKE_TOOLS.split(',').collect();
        assert!(tools.contains(&"ScheduleWakeup"));
        assert!(tools.contains(&"Monitor"));
    }

    #[test]
    fn now_secs_never_goes_backwards() {
        let first = now_secs();
        assert!(first > 0);
        assert!(now_secs() >= first);
    }

    #[test]
    fn lifecycle_correlation_accepts_only_our_exact_uuid() {
        let uuid = "3f2a77aa-0000-4000-8000-000000000001";
        let ours = json!({ "type": "command_lifecycle", "command_uuid": uuid, "state": "started" });
        assert!(lifecycle_frame_is_ours(&ours, uuid));

        // An unsolicited turn: same shape, someone else's uuid.
        let theirs =
            json!({ "type": "command_lifecycle", "command_uuid": "other", "state": "started" });
        assert!(!lifecycle_frame_is_ours(&theirs, uuid));

        // An older CLI without msg_lifecycle_v1 omits the field entirely.
        let missing = json!({ "type": "command_lifecycle", "state": "started" });
        assert!(!lifecycle_frame_is_ours(&missing, uuid));

        // A uuid that is present but not a string must not match.
        let wrong_type =
            json!({ "type": "command_lifecycle", "command_uuid": 42, "state": "started" });
        assert!(!lifecycle_frame_is_ours(&wrong_type, uuid));

        // Our uuid on a non-lifecycle frame opens nothing.
        let not_lifecycle = json!({ "type": "assistant", "command_uuid": uuid });
        assert!(!lifecycle_frame_is_ours(&not_lifecycle, uuid));
    }

    #[test]
    fn interrupt_request_has_the_shape_the_cli_expects() {
        // Verified against claude 2.1.278: subtype "interrupt" under `request`,
        // with a request_id the control_response echoes back.
        let request = json!({
            "type": "control_request",
            "request_id": "juno_int_test",
            "request": { "subtype": "interrupt" }
        });
        assert_eq!(request["type"], "control_request");
        assert_eq!(request["request"]["subtype"], "interrupt");
    }
}

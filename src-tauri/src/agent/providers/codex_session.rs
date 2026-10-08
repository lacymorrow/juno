//! # One warm `codex app-server` for the Codex CLI provider
//!
//! The one-shot path in [`super::codex_cli`] spawns `codex exec` per message:
//! about 2.2s of process start before the model sees a token, and no memory of
//! the previous message. This keeps one `codex app-server` alive instead and
//! talks JSON-RPC 2.0 to it over stdio, one JSON object per line.
//!
//! Measured on a ChatGPT plan, codex-cli 0.159.0:
//!
//! | | first text |
//! |---|---|
//! | cold `codex exec` | ~5.9s |
//! | new thread, first turn | 3.0-4.3s |
//! | thread started early and left to settle, first turn | ~2.5s |
//! | follow-up turn on the same thread | 1.0-2.0s |
//!
//! Most of a new thread's first-turn cost is Codex starting the person's own
//! MCP servers for that thread. So a **warm thread** is started ahead of time
//! (at launch, and again after each new conversation takes it) and the next
//! new conversation adopts it. No throwaway turn is sent to warm it: that
//! would spend plan quota on every launch for no answer, and settling alone
//! got most of the gain.
//!
//! ## Shape
//!
//! - One process, many threads. A Juno conversation maps to one Codex thread,
//!   which holds the history, so a follow-up is a follow-up.
//! - A writer task owns stdin; a reader task owns stdout and routes responses
//!   to the request waiting on them and notifications into one channel.
//! - That channel's receiver sits behind the only async lock. A turn holds it
//!   for its whole life, which serializes turns and hands the turn its events.
//!   A new turn first interrupts the one in flight (`turn/interrupt`), so it
//!   never waits behind an answer nobody wants any more.
//! - Escape interrupts with `turn/interrupt` (measured: the turn reports
//!   `interrupted` 8ms later) and the process stays warm.
//!
//! ## Nothing is lost when this fails
//!
//! `app-server` is labelled experimental. Every failure before the turn has
//! shown anything returns [`TurnOutcome::Unavailable`], and the caller runs the
//! same message through `codex exec` instead. Nobody is told. A protocol error
//! (unknown method, invalid params: a Codex that speaks a different version)
//! switches this path off until settings change; anything else gets
//! [`MAX_FAILURES`] tries.
//!
//! ## Juno's computer tool
//!
//! The process is started with `-c mcp_servers.juno.*` pointing at Juno's own
//! loopback MCP server ([`super::juno_mcp`]). The bearer token reaches Codex
//! through an environment variable on the child (`bearer_token_env_var`), so
//! it is never on a command line. Only `computer` is enabled, and it is
//! pre-approved: with `approvalPolicy: "never"` an MCP call that needs
//! approval fails instead of asking, which is also why the person's own
//! connectors cannot send anything unattended from here.

use std::collections::HashMap;
use std::path::PathBuf;
use std::process::Stdio;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, OnceLock};
use std::time::Duration;

use serde_json::{json, Value};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::sync::{mpsc, oneshot, Mutex as TokioMutex};
use tracing::{debug, info, warn};

use super::claude_cli::ClaudeCliBrain;
use super::juno_mcp;
use crate::agent::core::AgentError;

/// How long a single request (initialize, thread/start, turn/start) may take.
const REQUEST_TIMEOUT: Duration = Duration::from_secs(15);
/// Upper bound on one turn, matching the one-shot path.
const TURN_TIMEOUT: Duration = Duration::from_secs(300);
/// How long an interrupted turn has to report back before the process is
/// killed instead.
const INTERRUPT_GRACE: Duration = Duration::from_secs(5);
/// Delay before a replacement warm thread is started, so it does not compete
/// with the turn that just took the last one.
const SPARE_REFILL_DELAY: Duration = Duration::from_secs(3);
/// Conversations kept bound to a live thread. Each thread runs its own copy of
/// the person's stdio MCP servers, so this stays small.
const MAX_BOUND_THREADS: usize = 2;
/// Transient failures tolerated before this path is left alone until settings
/// change.
pub const MAX_FAILURES: u32 = 3;
/// The environment variable Codex reads Juno's MCP bearer token from.
pub const MCP_TOKEN_ENV: &str = "JUNO_MCP_TOKEN";
/// The name Juno's MCP server goes by inside Codex.
const MCP_SERVER_NAME: &str = "juno";
const STORE_FILE: &str = "codex_session.json";
const INSTRUCTIONS_KEY: &str = "spare_developer_instructions";

// ---------------------------------------------------------------------------
// Pure pieces: framing, parsing, the fallback decision. Unit-tested below.
// ---------------------------------------------------------------------------

/// `-c` overrides that hand Codex Juno's computer tool. The URL is not a
/// secret; the token travels in [`MCP_TOKEN_ENV`].
pub fn mcp_config_args(url: &str) -> Vec<String> {
    let name = MCP_SERVER_NAME;
    vec![
        "-c".to_string(),
        format!("mcp_servers.{name}.url={}", toml_string(url)),
        "-c".to_string(),
        format!(
            "mcp_servers.{name}.bearer_token_env_var={}",
            toml_string(MCP_TOKEN_ENV)
        ),
        "-c".to_string(),
        format!("mcp_servers.{name}.default_tools_approval_mode=\"approve\""),
        "-c".to_string(),
        format!("mcp_servers.{name}.enabled_tools=[\"computer\"]"),
    ]
}

/// A TOML basic string. JSON string escaping is a valid subset of it.
fn toml_string(s: &str) -> String {
    Value::String(s.to_string()).to_string()
}

/// One request, framed: a JSON object and a newline.
pub(crate) fn encode_request(id: u64, method: &str, params: Value) -> String {
    let mut line =
        json!({ "jsonrpc": "2.0", "id": id, "method": method, "params": params }).to_string();
    line.push('\n');
    line
}

/// A notification: no id, no answer expected.
pub(crate) fn encode_notification(method: &str) -> String {
    let mut line = json!({ "jsonrpc": "2.0", "method": method }).to_string();
    line.push('\n');
    line
}

/// The answer to a server-initiated request Juno does not handle.
fn encode_error_response(id: &Value, code: i64, message: &str) -> String {
    let mut line = json!({
        "jsonrpc": "2.0",
        "id": id,
        "error": { "code": code, "message": message }
    })
    .to_string();
    line.push('\n');
    line
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct RpcError {
    pub code: i64,
    pub message: String,
}

/// What one line from the server is.
#[derive(Debug, PartialEq)]
pub(crate) enum Frame {
    /// The answer to one of Juno's requests.
    Response {
        id: u64,
        outcome: Result<Value, RpcError>,
    },
    /// Something happened. No answer expected.
    Notification { method: String, params: Value },
    /// The server is asking Juno something (an approval, an elicitation).
    Request { id: Value, method: String },
    /// Not JSON-RPC at all.
    Junk,
}

pub(crate) fn parse_frame(line: &str) -> Frame {
    let Ok(v) = serde_json::from_str::<Value>(line.trim()) else {
        return Frame::Junk;
    };
    let method = v.get("method").and_then(Value::as_str);
    let id = v.get("id").filter(|id| !id.is_null());
    match (method, id) {
        (Some(method), Some(id)) => Frame::Request {
            id: id.clone(),
            method: method.to_string(),
        },
        (Some(method), None) => Frame::Notification {
            method: method.to_string(),
            params: v.get("params").cloned().unwrap_or(Value::Null),
        },
        (None, Some(id)) => {
            let Some(id) = id.as_u64() else {
                return Frame::Junk;
            };
            let outcome = match v.get("error") {
                Some(err) => Err(RpcError {
                    code: err.get("code").and_then(Value::as_i64).unwrap_or(0),
                    message: err
                        .get("message")
                        .and_then(Value::as_str)
                        .unwrap_or("")
                        .to_string(),
                }),
                None => Ok(v.get("result").cloned().unwrap_or(Value::Null)),
            };
            Frame::Response { id, outcome }
        }
        (None, None) => Frame::Junk,
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum TurnStatus {
    Completed,
    Interrupted,
    Failed,
}

/// What a notification means to the turn that is waiting on it.
#[derive(Debug, PartialEq)]
pub(crate) enum TurnEvent {
    /// More of an assistant message. `item_id` changes between messages.
    Delta { item_id: String, text: String },
    /// The turn is over.
    Completed {
        status: TurnStatus,
        error: Option<String>,
    },
    /// Something went wrong; `will_retry` means Codex is handling it.
    Error { message: String, will_retry: bool },
    /// Not ours, or nothing to act on.
    Ignore,
}

pub(crate) fn turn_event(
    method: &str,
    params: &Value,
    thread_id: &str,
    turn_id: &str,
) -> TurnEvent {
    if params.get("threadId").and_then(Value::as_str) != Some(thread_id) {
        return TurnEvent::Ignore;
    }
    fn at<'p>(params: &'p Value, pointer: &str) -> Option<&'p str> {
        params.pointer(pointer).and_then(Value::as_str)
    }
    let str_at = |pointer| at(params, pointer);
    match method {
        "item/agentMessage/delta" if str_at("/turnId") == Some(turn_id) => TurnEvent::Delta {
            item_id: str_at("/itemId").unwrap_or("").to_string(),
            text: str_at("/delta").unwrap_or("").to_string(),
        },
        "turn/completed" if str_at("/turn/id") == Some(turn_id) => TurnEvent::Completed {
            status: match str_at("/turn/status") {
                Some("completed") => TurnStatus::Completed,
                Some("interrupted") => TurnStatus::Interrupted,
                _ => TurnStatus::Failed,
            },
            error: str_at("/turn/error/message").map(str::to_string),
        },
        "error" if str_at("/turnId") == Some(turn_id) => TurnEvent::Error {
            message: str_at("/error/message")
                .unwrap_or("Codex reported an error")
                .to_string(),
            will_retry: params
                .get("willRetry")
                .and_then(Value::as_bool)
                .unwrap_or(false),
        },
        _ => TurnEvent::Ignore,
    }
}

/// Why the persistent path could not serve a turn.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum SessionFailure {
    /// `codex app-server` would not start.
    Spawn(String),
    /// This Codex speaks a different protocol. Not worth retrying.
    Incompatible(String),
    /// Any other error answer.
    Rpc(String),
    /// No answer in time.
    Timeout,
    /// The process went away.
    Died,
}

impl SessionFailure {
    fn from_rpc(e: RpcError) -> Self {
        // Method not found, invalid params, invalid request: the server does
        // not know the protocol Juno speaks, and will not learn it by retrying.
        if matches!(e.code, -32601 | -32602 | -32600) {
            Self::Incompatible(format!("{} ({})", e.message, e.code))
        } else {
            Self::Rpc(format!("{} ({})", e.message, e.code))
        }
    }

    fn describe(&self) -> String {
        match self {
            Self::Spawn(e) => format!("could not start app-server: {e}"),
            Self::Incompatible(e) => format!("incompatible app-server: {e}"),
            Self::Rpc(e) => format!("app-server error: {e}"),
            Self::Timeout => "app-server did not answer in time".to_string(),
            Self::Died => "app-server exited".to_string(),
        }
    }
}

/// Whether the persistent path is worth trying. The fallback decision.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Health {
    incompatible: bool,
    failures: u32,
}

impl Health {
    pub(crate) fn usable(&self) -> bool {
        !self.incompatible && self.failures < MAX_FAILURES
    }

    pub(crate) fn record(&mut self, failure: &SessionFailure) {
        match failure {
            SessionFailure::Incompatible(_) => self.incompatible = true,
            _ => self.failures = self.failures.saturating_add(1),
        }
    }

    pub(crate) fn record_success(&mut self) {
        self.failures = 0;
    }
}

// ---------------------------------------------------------------------------
// The process.
// ---------------------------------------------------------------------------

type Pending = Arc<std::sync::Mutex<HashMap<u64, oneshot::Sender<Result<Value, RpcError>>>>>;

/// What a process is born with. A turn that needs anything else gets a new one.
#[derive(Clone, Debug)]
pub struct Launch {
    pub binary: PathBuf,
    pub mcp: Option<juno_mcp::Endpoint>,
}

impl Launch {
    fn signature(&self) -> String {
        let (url, token) = match &self.mcp {
            Some(mcp) => (mcp.url.as_str(), mcp.token.as_str()),
            None => ("", ""),
        };
        format!("{}\u{1f}{url}\u{1f}{token}", self.binary.display())
    }
}

/// What a thread is born with. The model is sent again on every turn, so a
/// model change does not need a new thread; the instructions do.
#[derive(Clone, Debug)]
pub struct ThreadConfig {
    pub model: String,
    pub instructions: String,
}

struct AppServer {
    signature: String,
    to_child: mpsc::UnboundedSender<String>,
    pending: Pending,
    /// Notifications, in order. Locking it is what serializes turns.
    events: TokioMutex<mpsc::UnboundedReceiver<(String, Value)>>,
    next_id: AtomicU64,
    dead: Arc<AtomicBool>,
    child: std::sync::Mutex<Option<tokio::process::Child>>,
}

impl AppServer {
    fn is_dead(&self) -> bool {
        self.dead.load(Ordering::SeqCst)
    }

    fn kill(&self) {
        self.dead.store(true, Ordering::SeqCst);
        if let Ok(mut child) = self.child.lock() {
            if let Some(child) = child.as_mut() {
                let _ = child.start_kill();
            }
        }
    }

    async fn request(&self, method: &str, params: Value) -> Result<Value, SessionFailure> {
        if self.is_dead() {
            return Err(SessionFailure::Died);
        }
        let id = self.next_id.fetch_add(1, Ordering::SeqCst) + 1;
        let (tx, rx) = oneshot::channel();
        if let Ok(mut pending) = self.pending.lock() {
            pending.insert(id, tx);
        }
        if self
            .to_child
            .send(encode_request(id, method, params))
            .is_err()
        {
            self.forget(id);
            return Err(SessionFailure::Died);
        }
        match tokio::time::timeout(REQUEST_TIMEOUT, rx).await {
            Ok(Ok(Ok(result))) => Ok(result),
            Ok(Ok(Err(e))) => Err(SessionFailure::from_rpc(e)),
            Ok(Err(_)) => Err(SessionFailure::Died),
            Err(_) => {
                self.forget(id);
                Err(SessionFailure::Timeout)
            }
        }
    }

    /// Fire a request and ignore its answer (interrupt, unsubscribe).
    fn send_and_forget(&self, method: &str, params: Value) {
        let id = self.next_id.fetch_add(1, Ordering::SeqCst) + 1;
        if self
            .to_child
            .send(encode_request(id, method, params))
            .is_err()
        {
            debug!("[CodexSession] Could not send {method}; the process is gone");
        }
    }

    fn forget(&self, id: u64) {
        if let Ok(mut pending) = self.pending.lock() {
            pending.remove(&id);
        }
    }
}

async fn spawn_server(launch: &Launch) -> Result<Arc<AppServer>, SessionFailure> {
    let mut command = super::codex_cli::codex_command(&launch.binary);
    command.arg("app-server");
    if let Some(mcp) = &launch.mcp {
        command.args(mcp_config_args(&mcp.url));
        command.env(MCP_TOKEN_ENV, &mcp.token);
    }
    let mut child = command
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true)
        .spawn()
        .map_err(|e| SessionFailure::Spawn(e.to_string()))?;

    let (Some(mut stdin), Some(stdout)) = (child.stdin.take(), child.stdout.take()) else {
        let _ = child.start_kill();
        return Err(SessionFailure::Spawn("no stdio pipes".to_string()));
    };
    if let Some(stderr) = child.stderr.take() {
        // Drained so a chatty server can never fill the pipe and stall.
        tauri::async_runtime::spawn(async move {
            let mut lines = BufReader::new(stderr).lines();
            while let Ok(Some(line)) = lines.next_line().await {
                debug!(
                    "[CodexSession] stderr: {}",
                    line.chars().take(300).collect::<String>()
                );
            }
        });
    }

    let (to_child, mut outbox) = mpsc::unbounded_channel::<String>();
    tauri::async_runtime::spawn(async move {
        while let Some(line) = outbox.recv().await {
            if stdin.write_all(line.as_bytes()).await.is_err() || stdin.flush().await.is_err() {
                break;
            }
        }
    });

    let pending: Pending = Arc::new(std::sync::Mutex::new(HashMap::new()));
    let dead = Arc::new(AtomicBool::new(false));
    let (events_tx, events_rx) = mpsc::unbounded_channel::<(String, Value)>();
    {
        let pending = Arc::clone(&pending);
        let dead = Arc::clone(&dead);
        let replies = to_child.clone();
        tauri::async_runtime::spawn(async move {
            let mut lines = BufReader::new(stdout).lines();
            while let Ok(Some(line)) = lines.next_line().await {
                match parse_frame(&line) {
                    Frame::Response { id, outcome } => {
                        let waiter = pending.lock().ok().and_then(|mut p| p.remove(&id));
                        if let Some(waiter) = waiter {
                            let _ = waiter.send(outcome);
                        }
                    }
                    Frame::Notification { method, params } => {
                        let _ = events_tx.send((method, params));
                    }
                    Frame::Request { id, method } => {
                        // Approval policy is "never", so these should not come.
                        // Answer anyway: an unanswered request stalls the turn.
                        debug!("[CodexSession] Declining server request {method}");
                        let _ = replies.send(encode_error_response(
                            &id,
                            -32601,
                            "Juno does not handle this request",
                        ));
                    }
                    Frame::Junk => {}
                }
            }
            dead.store(true, Ordering::SeqCst);
            if let Ok(mut pending) = pending.lock() {
                // Dropping the senders wakes every waiter with "died".
                pending.clear();
            }
        });
    }

    let server = Arc::new(AppServer {
        signature: launch.signature(),
        to_child,
        pending,
        events: TokioMutex::new(events_rx),
        next_id: AtomicU64::new(0),
        dead,
        child: std::sync::Mutex::new(Some(child)),
    });

    let init = server
        .request(
            "initialize",
            json!({
                "clientInfo": {
                    "name": "juno",
                    "title": "Juno",
                    "version": env!("CARGO_PKG_VERSION")
                }
            }),
        )
        .await;
    if let Err(e) = init {
        server.kill();
        return Err(e);
    }
    let _ = server.to_child.send(encode_notification("initialized"));
    info!("[CodexSession] app-server ready");
    Ok(server)
}

async fn start_thread(server: &AppServer, config: &ThreadConfig) -> Result<String, SessionFailure> {
    let mut params = json!({
        "ephemeral": true,
        "sandbox": "read-only",
        "approvalPolicy": "never",
        "model": config.model,
        "developerInstructions": config.instructions,
    });
    if let (Some(home), Some(obj)) = (dirs::home_dir(), params.as_object_mut()) {
        obj.insert("cwd".to_string(), json!(home.to_string_lossy()));
    }
    let result = server.request("thread/start", params).await?;
    result
        .pointer("/thread/id")
        .and_then(Value::as_str)
        .map(str::to_string)
        .ok_or_else(|| SessionFailure::Incompatible("thread/start returned no thread id".into()))
}

// ---------------------------------------------------------------------------
// The registry: the process, the threads bound to conversations, the spare.
// ---------------------------------------------------------------------------

struct Spare {
    thread_id: String,
    instructions: String,
}

#[derive(Default)]
struct Registry {
    server: Option<Arc<AppServer>>,
    /// (conversation id, thread id), least recently used first.
    threads: Vec<(String, String)>,
    spare: Option<Spare>,
    spare_pending: bool,
    health: Health,
    /// (thread id, turn id) of the turn in flight.
    active_turn: Option<(String, String)>,
}

static REGISTRY: OnceLock<std::sync::Mutex<Registry>> = OnceLock::new();

fn registry() -> std::sync::MutexGuard<'static, Registry> {
    REGISTRY
        .get_or_init(|| std::sync::Mutex::new(Registry::default()))
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

fn record_failure(failure: &SessionFailure) {
    debug!("[CodexSession] {}", failure.describe());
    registry().health.record(failure);
}

/// Kill the process and forget every thread. Safe to call any time.
pub fn shutdown() {
    let server = {
        let mut r = registry();
        r.threads.clear();
        r.spare = None;
        r.active_turn = None;
        r.server.take()
    };
    if let Some(server) = server {
        server.kill();
    }
}

async fn get_server(launch: &Launch) -> Result<Arc<AppServer>, SessionFailure> {
    let signature = launch.signature();
    let stale = {
        let mut r = registry();
        match &r.server {
            Some(s) if !s.is_dead() && s.signature == signature => return Ok(Arc::clone(s)),
            _ => {}
        }
        // Threads live in the process; a new process starts with none.
        r.threads.clear();
        r.spare = None;
        r.active_turn = None;
        r.server.take()
    };
    if let Some(stale) = stale {
        stale.kill();
    }

    let server = spawn_server(launch).await?;
    let mut r = registry();
    if let Some(existing) = &r.server {
        if !existing.is_dead() && existing.signature == signature {
            // Another task got there first; keep one.
            let existing = Arc::clone(existing);
            drop(r);
            server.kill();
            return Ok(existing);
        }
    }
    r.server = Some(Arc::clone(&server));
    Ok(server)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Origin {
    /// The conversation's own thread, already used.
    Warm,
    /// A thread started ahead of time and adopted now.
    Spare,
    /// A thread started for this turn.
    Cold,
}

impl Origin {
    fn timing_label(self) -> &'static str {
        match self {
            Origin::Warm => "codex_cli/persistent-warm",
            Origin::Spare => "codex_cli/persistent-spare",
            Origin::Cold => "codex_cli/persistent-cold",
        }
    }
}

async fn acquire_thread(
    server: &AppServer,
    conversation_id: &str,
    config: &ThreadConfig,
) -> Result<(String, Origin), SessionFailure> {
    let (bound, spare) = {
        let mut r = registry();
        let bound = r
            .threads
            .iter()
            .position(|(conversation, _)| conversation == conversation_id)
            .map(|index| {
                let entry = r.threads.remove(index);
                let thread_id = entry.1.clone();
                r.threads.push(entry);
                thread_id
            });
        let spare = if bound.is_none() {
            r.spare.take()
        } else {
            None
        };
        (bound, spare)
    };
    if let Some(thread_id) = bound {
        return Ok((thread_id, Origin::Warm));
    }

    let (thread_id, origin) = match spare {
        Some(spare) if spare.instructions == config.instructions => {
            info!("[CodexSession] Adopted warm thread");
            (spare.thread_id, Origin::Spare)
        }
        other => {
            if let Some(stale) = other {
                debug!("[CodexSession] Warm thread was born with other instructions; dropping it");
                server
                    .send_and_forget("thread/unsubscribe", json!({ "threadId": stale.thread_id }));
            }
            (start_thread(server, config).await?, Origin::Cold)
        }
    };

    let evicted = {
        let mut r = registry();
        r.threads
            .push((conversation_id.to_string(), thread_id.clone()));
        let mut evicted = Vec::new();
        while r.threads.len() > MAX_BOUND_THREADS {
            evicted.push(r.threads.remove(0).1);
        }
        evicted
    };
    for old in evicted {
        server.send_and_forget("thread/unsubscribe", json!({ "threadId": old }));
    }
    Ok((thread_id, origin))
}

/// Interrupt the turn in flight, if there is one. A new turn calls this first.
fn interrupt_active_turn() {
    let (server, active) = {
        let r = registry();
        (r.server.clone(), r.active_turn.clone())
    };
    if let (Some(server), Some((thread_id, turn_id))) = (server, active) {
        info!("[CodexSession] A new turn interrupts the one in flight");
        server.send_and_forget(
            "turn/interrupt",
            json!({ "threadId": thread_id, "turnId": turn_id }),
        );
    }
}

/// Start a warm thread for the next new conversation, unless one is ready or
/// on its way. Never sends a turn.
async fn fill_spare(launch: Launch, config: ThreadConfig) {
    {
        let mut r = registry();
        if !r.health.usable() || r.spare_pending {
            return;
        }
        if r.spare
            .as_ref()
            .is_some_and(|s| s.instructions == config.instructions)
        {
            return;
        }
        r.spare_pending = true;
    }

    let result = async {
        let server = get_server(&launch).await?;
        let thread_id = start_thread(&server, &config).await?;
        Ok::<_, SessionFailure>((server, thread_id))
    }
    .await;

    match result {
        Ok((server, thread_id)) => {
            let replaced = {
                let mut r = registry();
                r.spare_pending = false;
                r.spare.replace(Spare {
                    thread_id,
                    instructions: config.instructions,
                })
            };
            if let Some(old) = replaced {
                server.send_and_forget("thread/unsubscribe", json!({ "threadId": old.thread_id }));
            }
            debug!("[CodexSession] Warm thread ready");
        }
        Err(failure) => {
            registry().spare_pending = false;
            record_failure(&failure);
        }
    }
}

fn schedule_spare(launch: Launch, config: ThreadConfig) {
    if registry().spare.is_some() {
        return;
    }
    tauri::async_runtime::spawn(async move {
        tokio::time::sleep(SPARE_REFILL_DELAY).await;
        fill_spare(launch, config).await;
    });
}

// ---------------------------------------------------------------------------
// Remembering the instructions, so a warm thread can be ready at launch.
// ---------------------------------------------------------------------------

fn remembered_instructions(app: &tauri::AppHandle) -> Option<String> {
    use tauri_plugin_store::StoreExt;
    app.store(STORE_FILE)
        .ok()?
        .get(INSTRUCTIONS_KEY)?
        .as_str()
        .map(str::to_string)
}

fn remember_instructions(app: &tauri::AppHandle, instructions: &str) {
    use tauri_plugin_store::StoreExt;
    let Ok(store) = app.store(STORE_FILE) else {
        return;
    };
    if store.get(INSTRUCTIONS_KEY).as_ref().and_then(Value::as_str) == Some(instructions) {
        return;
    }
    store.set(INSTRUCTIONS_KEY, Value::String(instructions.to_string()));
    if let Err(e) = store.save() {
        debug!("[CodexSession] Could not save {STORE_FILE}: {e}");
    }
}

/// Keep a warm thread ready while Codex is the provider: prime one now, and
/// re-prime (or shut down) whenever provider settings change. Never blocks.
pub fn start_warm(app: tauri::AppHandle) {
    use tauri::Listener;

    let generation = Arc::new(AtomicU64::new(0));
    let listener_app = app.clone();
    app.listen_any(
        crate::constants::settings::events::PROVIDER_SETTINGS_CHANGED,
        move |_| {
            let mine = generation.fetch_add(1, Ordering::Relaxed) + 1;
            let generation = Arc::clone(&generation);
            let app = listener_app.clone();
            tauri::async_runtime::spawn(async move {
                tokio::time::sleep(Duration::from_millis(1500)).await;
                if generation.load(Ordering::Relaxed) == mine {
                    // A settings change is a fresh start for the fallback rule.
                    registry().health = Health::default();
                    prime(app).await;
                }
            });
        },
    );
    tauri::async_runtime::spawn(prime(app));
}

async fn prime(app: tauri::AppHandle) {
    use crate::agent::providers::factory::BrainFactory;
    use crate::agent::providers::types::Provider;

    let model = match BrainFactory::active_provider_and_model(Some(&app)) {
        Some((Provider::CodexCli, model)) => model,
        _ => {
            shutdown();
            return;
        }
    };
    let Some(instructions) = remembered_instructions(&app) else {
        debug!("[CodexSession] No turn has run yet; no warm thread until one has");
        return;
    };
    if !super::codex_cli::cli_status().await.is_signed_in() {
        return;
    }
    let Ok(binary) = super::codex_cli::detect_codex_cli() else {
        return;
    };
    let mcp = juno_mcp::ensure_running(&app).await.ok();
    fill_spare(
        Launch { binary, mcp },
        ThreadConfig {
            model,
            instructions,
        },
    )
    .await;
}

// ---------------------------------------------------------------------------
// A turn.
// ---------------------------------------------------------------------------

pub enum TurnOutcome {
    /// The turn ran here. The text is what the person was shown.
    Completed(String),
    /// Nothing was shown or spent; run the message some other way.
    Unavailable,
}

pub struct TurnRequest<'a> {
    pub launch: Launch,
    pub thread: ThreadConfig,
    pub conversation_id: &'a str,
    pub query: &'a str,
    pub app_handle: &'a tauri::AppHandle,
    pub message_id: Option<String>,
    pub cancel_rx: Option<crate::state::CancelReceiver>,
}

pub async fn run_turn(req: TurnRequest<'_>) -> Result<TurnOutcome, AgentError> {
    if !registry().health.usable() {
        return Ok(TurnOutcome::Unavailable);
    }
    interrupt_active_turn();

    let server = match get_server(&req.launch).await {
        Ok(server) => server,
        Err(failure) => {
            record_failure(&failure);
            return Ok(TurnOutcome::Unavailable);
        }
    };
    let (thread_id, origin) = match acquire_thread(&server, req.conversation_id, &req.thread).await
    {
        Ok(found) => found,
        Err(failure) => {
            record_failure(&failure);
            return Ok(TurnOutcome::Unavailable);
        }
    };
    remember_instructions(req.app_handle, &req.thread.instructions);

    let mut events = server.events.lock().await;
    // Whatever arrived while no turn was listening belongs to nobody.
    while events.try_recv().is_ok() {}

    crate::turn_timing::note_llm(origin.timing_label(), &req.thread.model);
    crate::turn_timing::mark(crate::turn_timing::Stage::LlmRequestSent);
    let started = server
        .request(
            "turn/start",
            json!({
                "threadId": thread_id,
                "input": [{ "type": "text", "text": req.query }],
                "model": req.thread.model,
            }),
        )
        .await;
    let turn_id = match started.map(|v| {
        v.pointer("/turn/id")
            .and_then(Value::as_str)
            .map(str::to_string)
    }) {
        Ok(Some(turn_id)) => turn_id,
        Ok(None) => {
            record_failure(&SessionFailure::Incompatible(
                "turn/start returned no turn id".into(),
            ));
            return Ok(TurnOutcome::Unavailable);
        }
        Err(failure) => {
            record_failure(&failure);
            return Ok(TurnOutcome::Unavailable);
        }
    };
    registry().active_turn = Some((thread_id.clone(), turn_id.clone()));

    let outcome = stream_turn(&server, &mut events, &req, &thread_id, &turn_id).await;
    drop(events);

    {
        let mut r = registry();
        if r.active_turn.as_ref().is_some_and(|(_, t)| *t == turn_id) {
            r.active_turn = None;
        }
        if matches!(outcome, Ok(TurnOutcome::Completed(_))) {
            r.health.record_success();
        }
    }
    schedule_spare(req.launch.clone(), req.thread.clone());
    outcome
}

enum Step {
    Event(Option<(String, Value)>),
    Cancel,
    CancelClosed,
    Deadline,
}

async fn stream_turn(
    server: &AppServer,
    events: &mut mpsc::UnboundedReceiver<(String, Value)>,
    req: &TurnRequest<'_>,
    thread_id: &str,
    turn_id: &str,
) -> Result<TurnOutcome, AgentError> {
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
        .unwrap_or_else(|| never_cancels.clone());

    let msg_id = req
        .message_id
        .clone()
        .unwrap_or_else(|| uuid::Uuid::new_v4().to_string());
    let app = Some(req.app_handle.clone());
    let mut surface = Surface::new(req.app_handle, msg_id.clone());

    let mut tts_stream = crate::agent::tts_tags::TtsTagStream::new();
    let mut text = String::new();
    let mut spoken: Vec<String> = Vec::new();
    let mut last_item: Option<String> = None;
    let mut last_error: Option<String> = None;
    let deadline = tokio::time::Instant::now() + TURN_TIMEOUT;
    let mut interrupted_at: Option<tokio::time::Instant> = None;

    let interrupt = |server: &AppServer| {
        server.send_and_forget(
            "turn/interrupt",
            json!({ "threadId": thread_id, "turnId": turn_id }),
        );
    };

    loop {
        if interrupted_at.is_none() && *cancel_rx.borrow() {
            info!("[CodexSession] Cancelled; interrupting the turn");
            interrupt(server);
            interrupted_at = Some(tokio::time::Instant::now());
        }
        let wait_until = match interrupted_at {
            Some(at) => at + INTERRUPT_GRACE,
            None => deadline,
        };
        let step = tokio::select! {
            event = events.recv() => Step::Event(event),
            changed = cancel_rx.changed(), if interrupted_at.is_none() => match changed {
                Ok(()) => Step::Cancel,
                Err(_) => Step::CancelClosed,
            },
            _ = tokio::time::sleep_until(wait_until) => Step::Deadline,
        };

        match step {
            Step::Cancel => continue, // the check at the top acts on it
            Step::CancelClosed => {
                // The sender is gone; it will never cancel again.
                cancel_rx = never_cancels.clone();
            }
            Step::Deadline => {
                if interrupted_at.is_some() {
                    warn!("[CodexSession] Interrupt went unanswered; killing app-server");
                    server.kill();
                    surface.close("Cancelled".to_string());
                    return Err(AgentError::Terminated);
                }
                interrupt(server);
                surface.close("Codex timed out".to_string());
                return Err(AgentError::Timeout(format!(
                    "Codex timed out after {} seconds",
                    TURN_TIMEOUT.as_secs()
                )));
            }
            Step::Event(None) => {
                // The process died under the turn.
                server.kill();
                if interrupted_at.is_some() {
                    surface.close("Cancelled".to_string());
                    return Err(AgentError::Terminated);
                }
                if !surface.opened {
                    record_failure(&SessionFailure::Died);
                    return Ok(TurnOutcome::Unavailable);
                }
                surface.close(text.clone());
                return Err(AgentError::LlmError(
                    "Codex stopped before finishing its answer".to_string(),
                ));
            }
            Step::Event(Some((method, params))) => {
                match turn_event(&method, &params, thread_id, turn_id) {
                    TurnEvent::Delta {
                        item_id,
                        text: delta,
                    } => {
                        if delta.is_empty() {
                            continue;
                        }
                        crate::turn_timing::mark(crate::turn_timing::Stage::LlmFirstToken);
                        surface.open();
                        let new_item = last_item.as_deref() != Some(item_id.as_str());
                        let delta = if new_item && last_item.is_some() {
                            // A second message is a second paragraph.
                            format!("\n\n{delta}")
                        } else {
                            delta
                        };
                        last_item = Some(item_id);
                        ClaudeCliBrain::emit_display_text(
                            &app,
                            &msg_id,
                            &delta,
                            &mut tts_stream,
                            &mut text,
                            &mut spoken,
                        );
                    }
                    TurnEvent::Error {
                        message,
                        will_retry,
                    } => {
                        debug!("[CodexSession] error (will retry: {will_retry}): {message}");
                        if !will_retry {
                            last_error = Some(message);
                        }
                    }
                    TurnEvent::Completed { status, error } => {
                        surface.open();
                        ClaudeCliBrain::flush_display_text(
                            &app,
                            &msg_id,
                            &mut tts_stream,
                            &mut text,
                            &mut spoken,
                        );
                        return match status {
                            TurnStatus::Completed => {
                                surface.close(text.clone());
                                Ok(TurnOutcome::Completed(text))
                            }
                            TurnStatus::Interrupted => {
                                surface.close("Cancelled".to_string());
                                Err(AgentError::Terminated)
                            }
                            TurnStatus::Failed => {
                                let message = error
                                    .or(last_error)
                                    .unwrap_or_else(|| "Codex could not answer".to_string());
                                warn!("[CodexSession] Turn failed: {message}");
                                surface.close(format!("Error: {message}"));
                                Err(AgentError::LlmError(message))
                            }
                        };
                    }
                    TurnEvent::Ignore => {}
                }
            }
        }
    }
}

/// The assistant bubble for one turn. Opened at the first thing worth showing,
/// so a turn that falls back before then leaves no trace; once open it is
/// closed on every way out.
struct Surface<'a> {
    app_handle: &'a tauri::AppHandle,
    msg_id: String,
    opened: bool,
    closed: bool,
}

impl<'a> Surface<'a> {
    fn new(app_handle: &'a tauri::AppHandle, msg_id: String) -> Self {
        Self {
            app_handle,
            msg_id,
            opened: false,
            closed: false,
        }
    }

    fn open(&mut self) {
        if !self.opened {
            self.opened = true;
            crate::agent::tool_logger::emit_stream_start(self.app_handle, self.msg_id.clone());
        }
    }

    fn close(&mut self, text: String) {
        if self.closed {
            return;
        }
        self.open();
        self.closed = true;
        crate::agent::tool_logger::emit_stream_end(self.app_handle, self.msg_id.clone(), text);
    }
}

impl Drop for Surface<'_> {
    fn drop(&mut self) {
        if self.opened && !self.closed {
            self.close(String::new());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn requests_are_one_json_object_per_line() {
        let line = encode_request(7, "turn/start", json!({ "threadId": "t" }));
        assert!(line.ends_with('\n'));
        assert_eq!(line.matches('\n').count(), 1);
        let v: Value = serde_json::from_str(line.trim()).unwrap();
        assert_eq!(v["jsonrpc"], "2.0");
        assert_eq!(v["id"], 7);
        assert_eq!(v["method"], "turn/start");
        assert_eq!(v["params"]["threadId"], "t");

        let note: Value = serde_json::from_str(encode_notification("initialized").trim()).unwrap();
        assert_eq!(note["method"], "initialized");
        assert!(note.get("id").is_none(), "a notification carries no id");
    }

    #[test]
    fn responses_notifications_and_requests_are_told_apart() {
        assert_eq!(
            parse_frame(r#"{"id":2,"result":{"thread":{"id":"th"}}}"#),
            Frame::Response {
                id: 2,
                outcome: Ok(json!({ "thread": { "id": "th" } }))
            }
        );
        assert_eq!(
            parse_frame(r#"{"id":3,"error":{"code":-32601,"message":"unknown method"}}"#),
            Frame::Response {
                id: 3,
                outcome: Err(RpcError {
                    code: -32601,
                    message: "unknown method".to_string()
                })
            }
        );
        assert!(matches!(
            parse_frame(r#"{"method":"turn/started","params":{"threadId":"th"}}"#),
            Frame::Notification { ref method, .. } if method == "turn/started"
        ));
        assert_eq!(
            parse_frame(r#"{"id":"srv-1","method":"item/tool/requestUserInput","params":{}}"#),
            Frame::Request {
                id: json!("srv-1"),
                method: "item/tool/requestUserInput".to_string()
            }
        );
        assert_eq!(parse_frame("WARN something on stdout"), Frame::Junk);
        assert_eq!(parse_frame(""), Frame::Junk);
    }

    #[test]
    fn a_declined_server_request_is_answered_with_its_own_id() {
        let line = encode_error_response(&json!("srv-1"), -32601, "no");
        let v: Value = serde_json::from_str(line.trim()).unwrap();
        assert_eq!(v["id"], "srv-1");
        assert_eq!(v["error"]["code"], -32601);
    }

    // Shapes captured from codex-cli 0.159.0 on a ChatGPT login.
    const DELTA: &str = r#"{"method":"item/agentMessage/delta","params":{"delta":"<TTS>Hi","itemId":"msg_1","threadId":"th","turnId":"tu"}}"#;
    const DONE: &str = r#"{"method":"turn/completed","params":{"threadId":"th","turn":{"id":"tu","items":[],"status":"completed","error":null}}}"#;
    const INTERRUPTED: &str = r#"{"method":"turn/completed","params":{"threadId":"th","turn":{"id":"tu","items":[],"status":"interrupted","error":null}}}"#;
    const FAILED: &str = r#"{"method":"turn/completed","params":{"threadId":"th","turn":{"id":"tu","status":"failed","error":{"message":"usage limit reached"}}}}"#;
    const ERROR: &str = r#"{"method":"error","params":{"threadId":"th","turnId":"tu","willRetry":false,"error":{"message":"stream disconnected"}}}"#;

    fn event(line: &str, thread: &str, turn: &str) -> TurnEvent {
        match parse_frame(line) {
            Frame::Notification { method, params } => turn_event(&method, &params, thread, turn),
            other => panic!("not a notification: {other:?}"),
        }
    }

    #[test]
    fn deltas_and_completion_are_read_for_our_turn() {
        assert_eq!(
            event(DELTA, "th", "tu"),
            TurnEvent::Delta {
                item_id: "msg_1".to_string(),
                text: "<TTS>Hi".to_string()
            }
        );
        assert_eq!(
            event(DONE, "th", "tu"),
            TurnEvent::Completed {
                status: TurnStatus::Completed,
                error: None
            }
        );
        assert_eq!(
            event(INTERRUPTED, "th", "tu"),
            TurnEvent::Completed {
                status: TurnStatus::Interrupted,
                error: None
            }
        );
        assert_eq!(
            event(FAILED, "th", "tu"),
            TurnEvent::Completed {
                status: TurnStatus::Failed,
                error: Some("usage limit reached".to_string())
            }
        );
        assert_eq!(
            event(ERROR, "th", "tu"),
            TurnEvent::Error {
                message: "stream disconnected".to_string(),
                will_retry: false
            }
        );
    }

    #[test]
    fn another_thread_or_turn_is_never_mistaken_for_ours() {
        // The warm thread and other conversations share the process.
        assert_eq!(event(DELTA, "other-thread", "tu"), TurnEvent::Ignore);
        assert_eq!(event(DELTA, "th", "older-turn"), TurnEvent::Ignore);
        assert_eq!(event(DONE, "th", "older-turn"), TurnEvent::Ignore);
        let mcp_status = r#"{"method":"mcpServer/startupStatus/updated","params":{"threadId":"th","name":"juno","status":"ready"}}"#;
        assert_eq!(event(mcp_status, "th", "tu"), TurnEvent::Ignore);
    }

    #[test]
    fn a_protocol_mismatch_stops_trying_at_once() {
        let mut health = Health::default();
        assert!(health.usable());
        health.record(&SessionFailure::from_rpc(RpcError {
            code: -32601,
            message: "method not found".to_string(),
        }));
        assert!(
            !health.usable(),
            "a Codex that does not speak this protocol never will"
        );
        health.record_success();
        assert!(
            !health.usable(),
            "a later success does not undo incompatibility"
        );
    }

    #[test]
    fn transient_failures_get_a_few_tries_and_success_resets_them() {
        let mut health = Health::default();
        for _ in 0..MAX_FAILURES - 1 {
            health.record(&SessionFailure::Timeout);
        }
        assert!(health.usable());
        health.record_success();
        for _ in 0..MAX_FAILURES - 1 {
            health.record(&SessionFailure::Died);
        }
        assert!(health.usable());
        health.record(&SessionFailure::Spawn("ENOENT".to_string()));
        assert!(
            !health.usable(),
            "then it falls back to codex exec for good"
        );
        assert_eq!(
            SessionFailure::from_rpc(RpcError {
                code: -32000,
                message: "busy".to_string()
            }),
            SessionFailure::Rpc("busy (-32000)".to_string())
        );
    }

    #[test]
    fn the_computer_tool_is_offered_with_the_token_kept_off_the_command_line() {
        let args = mcp_config_args("http://127.0.0.1:51234/mcp");
        let joined = args.join(" ");
        assert!(joined.contains(r#"mcp_servers.juno.url="http://127.0.0.1:51234/mcp""#));
        assert!(joined.contains(r#"mcp_servers.juno.bearer_token_env_var="JUNO_MCP_TOKEN""#));
        assert!(joined.contains(r#"mcp_servers.juno.default_tools_approval_mode="approve""#));
        assert!(joined.contains(r#"mcp_servers.juno.enabled_tools=["computer"]"#));
        assert_eq!(args.iter().filter(|a| *a == "-c").count(), 4);
    }

    #[test]
    fn a_process_is_replaced_when_the_tool_server_changes() {
        let bare = Launch {
            binary: PathBuf::from("/opt/homebrew/bin/codex"),
            mcp: None,
        };
        let with_tool = Launch {
            mcp: Some(juno_mcp::Endpoint {
                url: "http://127.0.0.1:1/mcp".to_string(),
                token: "t".to_string(),
            }),
            ..bare.clone()
        };
        assert_ne!(bare.signature(), with_tool.signature());
        assert_eq!(with_tool.signature(), with_tool.clone().signature());
    }

    #[test]
    fn turns_are_labelled_by_how_warm_they_started() {
        assert_eq!(Origin::Warm.timing_label(), "codex_cli/persistent-warm");
        assert_eq!(Origin::Spare.timing_label(), "codex_cli/persistent-spare");
        assert_eq!(Origin::Cold.timing_label(), "codex_cli/persistent-cold");
    }
}

//! Claude CLI provider — uses the `claude` binary (Claude Code) as a subprocess-based
//! AI provider. This allows users with a Claude Max/Pro subscription to use Juno
//! without a direct Anthropic API key.
//!
//! The Claude CLI handles its own agent loop (with Bash, Read, Edit tools), so this
//! provider spawns a subprocess per query and streams the response back to the UI
//! via the same Tauri events as the Anthropic provider.
//!
//! Desktop automation (mouse, keyboard, screenshots) is Juno's own tool,
//! served to the CLI over MCP from inside this process — see
//! [`crate::agent::providers::juno_mcp`]. It used to be delegated to
//! `juno-cua`, a separate binary, which meant the mouse moved without Juno
//! knowing: the smooth-movement setting went unread and the cursor overlay
//! went untold, so nothing on screen said who was driving.
//!
//! NOTE: This provider is macOS-only (matching Juno's platform target). The binary
//! detection paths are Unix-specific.

use async_trait::async_trait;
use serde_json::Value;
use std::path::PathBuf;
use std::process::Stdio;
use std::sync::atomic::{AtomicBool, Ordering};
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::time::Duration;
use tracing::{debug, error, info, warn};

/// Session-level auth cache — once verified, skip re-checking.
/// Reset on app restart (static lifetime).
static AUTH_VERIFIED: AtomicBool = AtomicBool::new(false);

/// Set once a `claude` build has rejected [`PARTIAL_MESSAGES_FLAG`], so the
/// rest of the session skips the flag rather than burning a failed spawn on
/// every single query.
///
/// Session-scoped like [`AUTH_VERIFIED`]: someone who upgrades their CLI gets
/// the fast path back on the next app start, and we never have to probe
/// `--help` up front to find out.
static PARTIAL_MESSAGES_UNSUPPORTED: AtomicBool = AtomicBool::new(false);

/// Claude CLI session ids, keyed by the Juno conversation they belong to.
///
/// The CLI is a subprocess: one spawn is one session, and a spawn with no
/// `--resume` starts from nothing. Every turn was therefore a brand new
/// conversation, which is why Juno kept answering follow-ups with "this is a
/// fresh session, I don't have the previous turn" while the chat window
/// showed the whole history above it. The history was never reaching her.
///
/// Keyed by conversation rather than held on the brain, because the brain is
/// rebuilt from settings on every single query. Starting a new chat mints a
/// new conversation id, so a new chat gets no entry here and correctly starts
/// the CLI fresh; nothing has to be explicitly cleared.
static CLI_SESSIONS: std::sync::OnceLock<
    std::sync::Mutex<std::collections::HashMap<String, String>>,
> = std::sync::OnceLock::new();

/// Conversations to remember a session id for. Well past what one run of the
/// app will revisit, and bounded so a long-lived process cannot grow this
/// forever.
const MAX_TRACKED_SESSIONS: usize = 64;

fn cli_sessions() -> &'static std::sync::Mutex<std::collections::HashMap<String, String>> {
    CLI_SESSIONS.get_or_init(|| std::sync::Mutex::new(std::collections::HashMap::new()))
}

/// The CLI session to continue for `conversation_id`, if we have seen one.
fn resume_id_for(conversation_id: &str) -> Option<String> {
    match cli_sessions().lock() {
        Ok(map) => map.get(conversation_id).cloned(),
        Err(e) => {
            warn!("[ClaudeCLI] Session registry poisoned: {}", e);
            None
        }
    }
}

/// Remember the CLI session this conversation is now running in.
fn remember_session(conversation_id: &str, session_id: &str) {
    match cli_sessions().lock() {
        Ok(mut map) => {
            if map.len() >= MAX_TRACKED_SESSIONS && !map.contains_key(conversation_id) {
                // Nothing here is worth an LRU. Conversations are revisited in
                // the near term or not at all, and losing one only costs a
                // fresh session on the next message.
                map.clear();
            }
            map.insert(conversation_id.to_string(), session_id.to_string());
        }
        Err(e) => warn!("[ClaudeCLI] Session registry poisoned: {}", e),
    }
}

/// Forget a session the CLI has told us it will not resume.
fn forget_session(conversation_id: &str) {
    if let Ok(mut map) = cli_sessions().lock() {
        map.remove(conversation_id);
    }
}

/// Which Juno conversation this run belongs to.
async fn conversation_id_for(app_handle: &Option<tauri::AppHandle>) -> Option<String> {
    use tauri::Manager;
    let handle = app_handle.as_ref()?;
    let state = handle.try_state::<crate::state::AppState>()?;
    let id = state.current_conversation_id.lock().await.clone();
    Some(id)
}

use crate::agent::core::{AgentAction, AgentError, Message, Role, ToolDefinition};
use crate::agent::providers::{claude_cli_session, juno_mcp};
use crate::agent::traits::AgentBrain;
use crate::settings::ProviderConfig as CentralizedProviderConfig;

/// Claude CLI model aliases — these map directly to `--model` flag values.
pub mod model_aliases {
    pub const OPUS: &str = "opus";
    pub const SONNET: &str = "sonnet";
    pub const HAIKU: &str = "haiku";
}

/// Maximum time to wait for the Claude CLI subprocess before killing it.
/// Claude CLI may run multi-step agent loops, so this is generous.
const CLI_TIMEOUT: Duration = Duration::from_secs(300);

/// Asks the CLI for the raw streaming events rather than finished messages
/// only. Everything the user sees while Juno works depends on it.
///
/// Older `claude` builds do not know this flag and exit non-zero on it, which
/// is why `run_streaming` retries once without it — Juno users install the
/// CLI themselves, so there is no floor version to assume.
const PARTIAL_MESSAGES_FLAG: &str = "--include-partial-messages";

/// Detect the Claude CLI binary on PATH.
/// Returns the path if found, or an error describing what to do.
pub fn detect_claude_cli() -> Result<PathBuf, AgentError> {
    // Check common locations in order of preference (macOS-specific paths)
    let candidates = [
        // User-local install (most common for Claude Code)
        dirs::home_dir().map(|h| h.join(".local/bin/claude")),
        // Homebrew on macOS
        Some(PathBuf::from("/usr/local/bin/claude")),
        Some(PathBuf::from("/opt/homebrew/bin/claude")),
        // System-wide
        Some(PathBuf::from("/usr/bin/claude")),
    ];

    for candidate in candidates.into_iter().flatten() {
        if candidate.exists() {
            info!("Found Claude CLI at: {}", candidate.display());
            return Ok(candidate);
        }
    }

    // Fall back to manual PATH search (avoids spawning a subprocess
    // and works even when `which` isn't available)
    if let Ok(path_var) = std::env::var("PATH") {
        for dir in std::env::split_paths(&path_var) {
            let candidate = dir.join("claude");
            if candidate.is_file() {
                info!("Found Claude CLI via PATH: {}", candidate.display());
                return Ok(candidate);
            }
        }
    }

    Err(AgentError::ConfigurationError(
        "Claude CLI (claude) not found. Install it from https://claude.ai/code".to_string(),
    ))
}

/// Quick check: is the Claude CLI binary available? (No auth check — just existence.)
pub fn is_claude_cli_available() -> bool {
    detect_claude_cli().is_ok()
}

/// What the last `claude auth status` said, for callers that cannot await.
///
/// Provider listing is synchronous and runs on every Settings render, so it
/// cannot spawn a subprocess to ask. Before anything has asked, the answer is
/// `Unknown` rather than "signed out" — claiming someone is logged out because
/// nobody has checked is a lie the UI would repeat.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum SignIn {
    /// Nobody has asked yet, or the CLI answered in a way we could not read.
    #[default]
    Unknown,
    /// `loggedIn: true`, in those words.
    SignedIn,
    /// `loggedIn: false`, in those words.
    SignedOut,
}

/// `SignIn` as a `u8`, because atomics do not carry enums.
static LAST_SIGN_IN: std::sync::atomic::AtomicU8 = std::sync::atomic::AtomicU8::new(0);

fn remember_sign_in(state: SignIn) {
    let encoded = match state {
        SignIn::Unknown => 0,
        SignIn::SignedIn => 1,
        SignIn::SignedOut => 2,
    };
    LAST_SIGN_IN.store(encoded, Ordering::Relaxed);
}

/// The most recent answer, without asking again.
pub fn last_known_sign_in() -> SignIn {
    match LAST_SIGN_IN.load(Ordering::Relaxed) {
        1 => SignIn::SignedIn,
        2 => SignIn::SignedOut,
        _ => SignIn::Unknown,
    }
}

/// Everything Juno knows about the local CLI, gathered in one pass.
#[derive(Debug, Clone, Default)]
pub struct CliStatus {
    /// The `claude` binary exists.
    pub installed: bool,
    /// What `claude auth status` said, including that it said nothing usable.
    pub sign_in: SignIn,
    /// Which account, when it told us. Settings shows this so the person can
    /// see whose quota Juno is spending.
    pub email: Option<String>,
}

impl CliStatus {
    /// Proof of a login. What Juno requires before choosing this provider for
    /// somebody who never asked for it.
    pub fn is_signed_in(&self) -> bool {
        self.sign_in == SignIn::SignedIn
    }

    /// Not a *known* bad login — which is what an actual query accepts, since
    /// [`check_auth_status`] lets an unreadable answer through rather than
    /// blocking someone whose CLI works.
    ///
    /// This is the reading the UI uses. Greying the provider out on an answer
    /// we merely could not parse would tell somebody they are signed out while
    /// their queries keep succeeding, which is a worse lie than the one this
    /// status was added to fix.
    pub fn could_run(&self) -> bool {
        self.installed && self.sign_in != SignIn::SignedOut
    }
}

/// Ask the CLI about itself.
///
/// Reports three answers, not two, because the ambiguous one is real and the
/// two callers want opposite things from it. An automatic switch onto this
/// provider demands proof ([`CliStatus::is_signed_in`]); the UI demands only
/// the absence of a confirmed negative ([`CliStatus::could_run`]). Collapsing
/// "could not tell" into either one is what makes a status display lie.
///
/// Never returns an error: everything that could go wrong is one of the three
/// answers.
pub async fn cli_status() -> CliStatus {
    let Ok(binary_path) = detect_claude_cli() else {
        // Not installed is not the same as signed out, and recording it as
        // signed out would outlive the fact: install the CLI, and until
        // something probed again the provider list would say "sign in" to
        // someone who already is.
        remember_sign_in(SignIn::Unknown);
        return CliStatus::default();
    };

    let output = tokio::process::Command::new(&binary_path)
        .args(["auth", "status", "--json"])
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .await;

    // `None` means the CLI did not give us an answer we can read: a build too
    // old to have `auth status --json` (non-zero exit), output that is not
    // JSON, or a spawn that failed outright. None of those is evidence of
    // being logged out.
    let parsed = match output {
        Ok(output) if output.status.success() => {
            serde_json::from_slice::<Value>(&output.stdout).ok()
        }
        Ok(output) => {
            debug!(
                "[ClaudeCLI] `claude auth status` exited {}: {}",
                output.status,
                String::from_utf8_lossy(&output.stderr)
                    .chars()
                    .take(200)
                    .collect::<String>()
            );
            None
        }
        Err(e) => {
            warn!("[ClaudeCLI] Could not run `claude auth status`: {}", e);
            None
        }
    };

    let sign_in = match parsed
        .as_ref()
        .and_then(|json| json.get("loggedIn"))
        .and_then(Value::as_bool)
    {
        Some(true) => SignIn::SignedIn,
        Some(false) => SignIn::SignedOut,
        None => SignIn::Unknown,
    };
    remember_sign_in(sign_in);

    CliStatus {
        installed: true,
        sign_in,
        email: parsed
            .as_ref()
            .and_then(|json| json.get("email"))
            .and_then(Value::as_str)
            .map(str::to_string),
    }
}

/// Write the `--mcp-config` file pointing the CLI at Juno's own tool server.
///
/// With `--strict-mcp-config` this is the only MCP server the CLI loads, so
/// the person's own user-level servers stay out of Juno's agent. The file is
/// pid-scoped so two Junos in a dev session cannot clobber each other, and it
/// carries the bearer token, which is why it goes to a file the CLI reads
/// rather than onto a command line every `ps` on the machine can see.
fn write_mcp_config(endpoint: &juno_mcp::Endpoint) -> Result<PathBuf, AgentError> {
    let path = std::env::temp_dir().join(format!("juno-mcp-{}.json", std::process::id()));
    let bytes = serde_json::to_vec(&juno_mcp::mcp_config(endpoint)).map_err(|e| {
        AgentError::ConfigurationError(format!("Failed to serialize MCP config: {}", e))
    })?;
    std::fs::write(&path, bytes).map_err(|e| {
        AgentError::ConfigurationError(format!(
            "Failed to write MCP config to {}: {}",
            path.display(),
            e
        ))
    })?;
    Ok(path)
}

/// System-prompt guidance appended when Juno's tool server is wired in.
/// Without this, models tend to fall back to `cliclick`/`screencapture` via
/// Bash even when MCP computer-use tools are available (LAC-3692).
const MCP_TOOL_GUIDANCE: &str = "You have a desktop automation tool from the \"juno\" MCP \
server: `computer`, which takes screenshots and moves, clicks, types, scrolls and presses keys. \
For ANY desktop or GUI automation use it. Do NOT use shell commands like cliclick, \
screencapture, or osascript for desktop automation: they bypass Juno, so the pointer moves with \
nothing on screen saying that Juno is the one moving it.";

/// Check if Claude CLI is both installed and authenticated.
/// Runs `claude auth status --json` and returns Ok(()) if logged in.
pub async fn check_cli_auth_status() -> Result<(), AgentError> {
    let binary_path = detect_claude_cli()?;
    check_auth_status(&binary_path).await
}

/// Check Claude CLI authentication status by running `claude auth status`.
/// Caches the result for the session — only runs the subprocess once.
/// Returns Ok(()) if authenticated, or an error with details.
async fn check_auth_status(binary_path: &PathBuf) -> Result<(), AgentError> {
    // Fast path: already verified this session
    if AUTH_VERIFIED.load(Ordering::Relaxed) {
        return Ok(());
    }

    let output = tokio::process::Command::new(binary_path)
        .args(["auth", "status", "--json"])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .await
        .map_err(|e| {
            AgentError::ConfigurationError(format!("Failed to run claude auth status: {}", e))
        })?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(AgentError::ConfigurationError(format!(
            "Claude CLI auth check failed: {}",
            stderr.chars().take(200).collect::<String>()
        )));
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    match serde_json::from_str::<Value>(&stdout) {
        Ok(json) => {
            let logged_in = json
                .get("loggedIn")
                .and_then(|v| v.as_bool())
                .unwrap_or(false);
            if logged_in {
                let email = json
                    .get("email")
                    .and_then(|v| v.as_str())
                    .unwrap_or("unknown");
                info!("Claude CLI authenticated as: {}", email);
                AUTH_VERIFIED.store(true, Ordering::Relaxed);
                remember_sign_in(SignIn::SignedIn);
                Ok(())
            } else {
                remember_sign_in(SignIn::SignedOut);
                Err(AgentError::ConfigurationError(
                    "Claude CLI is not logged in. Run `claude login` to authenticate.".to_string(),
                ))
            }
        }
        Err(_) => {
            // If we can't parse JSON but the command succeeded, assume OK.
            // Clear any confirmed-negative the cache is holding: we are about
            // to let a query run, so the UI must not go on saying this person
            // is signed out.
            warn!("Could not parse claude auth status output, assuming authenticated");
            AUTH_VERIFIED.store(true, Ordering::Relaxed);
            remember_sign_in(SignIn::Unknown);
            Ok(())
        }
    }
}

/// Claude CLI-based AgentBrain implementation.
///
/// Spawns `claude -p --output-format=stream-json` as a subprocess for each query,
/// streaming the response back through Tauri events. The CLI handles its own tool
/// execution: its built-in tools (Bash, Read, Edit, etc.) plus Juno's computer-use
/// Juno's own `computer` tool, served in-process over MCP (LAC-3696).
///
/// The subprocess is cancellable via Juno's escape key: the `run_streaming` method
/// races the streaming loop against the run's cancellation channel and kills the
/// child process immediately when cancellation is detected. Session-tracked runs
/// pass a merged session+global receiver through `decide_next_action_streaming`
/// (escape cancels only the FOCUSED session since LAC-1432, so the global AppState
/// channel alone would never fire — LAC-3697); the global channel remains the
/// fallback for legacy/headless callers that don't thread one through.
pub struct ClaudeCliBrain {
    binary_path: PathBuf,
    model: String,
    system_prompt: Option<String>,
    /// How hard the CLI thinks per turn — its `--effort` flag. Resolved once
    /// at construction from the hidden `providers[].effort` setting, so the
    /// stream loop never has to reach for the store.
    effort: String,
    /// The session id the CLI reported during the current run.
    ///
    /// Filled by the stream reader and read once the run finishes, so the
    /// next message in this conversation can continue the same session. Kept
    /// here rather than threaded through the return type because the stream
    /// loop already borrows `&self` and the brain lives exactly one run.
    observed_session: std::sync::Mutex<Option<String>>,
}

impl ClaudeCliBrain {
    /// Create from centralized provider config.
    /// Validates that the Claude CLI binary exists. Auth is checked lazily
    /// at query time to avoid blocking initialization.
    pub fn from_config(config: &CentralizedProviderConfig) -> Result<Self, AgentError> {
        let binary_path = detect_claude_cli()?;
        let model = config
            .model
            .clone()
            .unwrap_or_else(|| model_aliases::SONNET.to_string());

        let effort = resolve_effort(config.effort.as_deref());

        info!(
            "Initializing Claude CLI brain (binary: {}, model: {}, effort: {})",
            binary_path.display(),
            model,
            effort
        );

        Ok(Self {
            binary_path,
            model,
            system_prompt: config.system_prompt.clone(),
            effort,
            observed_session: std::sync::Mutex::new(None),
        })
    }

    /// Build the subprocess command arguments.
    ///
    /// `resume` continues an existing CLI session rather than starting a new
    /// one, which is what makes a second message in the same chat a follow-up
    /// instead of a cold open.
    fn build_args(
        &self,
        query: &str,
        resume: Option<&str>,
        mcp_config: Option<&std::path::Path>,
    ) -> Vec<String> {
        let mut args = vec![
            "-p".to_string(),
            "--output-format".to_string(),
            "stream-json".to_string(),
            "--model".to_string(),
            self.model.clone(),
            // How hard to think per turn. Hidden advanced setting; see
            // `resolve_effort`.
            "--effort".to_string(),
            self.effort.clone(),
            // Ask for the raw streaming events, not just the finished
            // messages. Without this the CLI says nothing between "thinking"
            // and the final answer: on a measured 8.5s task the first visible
            // pixel landed at t+7.1s, because a tool-only `assistant` message
            // extracts to the empty string and everything before it — the
            // tool call, its arguments, the reasoning — was never sent.
            // Stripped again by `run_streaming` for a CLI too old to take it.
            PARTIAL_MESSAGES_FLAG.to_string(),
            // --strict-mcp-config limits MCP servers to exactly what we pass
            // via --mcp-config (or none) — user-level servers never load.
            // We can't use --bare because it blocks OAuth/keychain auth.
            "--strict-mcp-config".to_string(),
            // In -p mode with stdin null, the CLI can't prompt for permission.
            // Allow tool execution since the user explicitly chose this provider.
            // Note: this also lets Juno's own MCP tool run without prompting
            // — matching this provider's existing trust model.
            "--dangerously-skip-permissions".to_string(),
            // The CLI's own toolset (Bash, Read, Edit, WebFetch) stays on
            // deliberately. It augments Juno's computer tool rather than
            // competing with it — reading a file beats screenshotting a text
            // editor — so it is not worth narrowing with `--tools ""` today.
            // May be revisited if the model starts reaching for Bash to drive
            // the desktop despite MCP_TOOL_GUIDANCE.
        ];

        // Continue the session this conversation is already running in. The
        // CLI keeps the full transcript and its own tool results on its side,
        // so resuming carries far more than replaying our message list could.
        if let Some(session) = resume {
            args.push("--resume".to_string());
            args.push(session.to_string());
        }

        if let Some(mcp_path) = mcp_config {
            args.push("--mcp-config".to_string());
            args.push(mcp_path.to_string_lossy().to_string());
            args.push("--append-system-prompt".to_string());
            args.push(MCP_TOOL_GUIDANCE.to_string());
        }

        if let Some(ref prompt) = self.system_prompt {
            args.push("--system-prompt".to_string());
            args.push(prompt.clone());
        }

        // The query itself is the final positional argument
        args.push(query.to_string());
        args
    }

    /// Extract the latest user query from the message history.
    /// Falls back to "Hello" if no user message is found.
    fn extract_query(messages: &[Message]) -> String {
        messages
            .iter()
            .rev()
            .find(|m| m.role == Role::User)
            .map(|m| m.content.clone())
            .unwrap_or_else(|| "Hello".to_string())
    }

    /// Run the Claude CLI subprocess and stream output to the Tauri frontend.
    ///
    /// Validates auth, spawns the subprocess with a timeout, drains stderr
    /// concurrently with stdout to prevent pipe buffer deadlocks, and emits
    /// Tauri streaming events for the UI.
    async fn run_streaming(
        &self,
        query: &str,
        app_handle: Option<tauri::AppHandle>,
        message_id: Option<String>,
        cancel_rx: Option<crate::state::CancelReceiver>,
    ) -> Result<String, AgentError> {
        // Validate auth before spawning the query subprocess
        check_auth_status(&self.binary_path).await?;

        // Continue this conversation's CLI session if it has one.
        let conversation = conversation_id_for(&app_handle).await;
        let resume = conversation.as_deref().and_then(resume_id_for);

        // Hand the CLI Juno's own computer tool. Done per run rather than when
        // the brain is built, because the server lives in the running app and
        // the brain is constructed without an app handle (and rebuilt on every
        // query). Without a handle there is no desktop to drive anyway, which
        // is the headless and test case, so the CLI simply runs toolless.
        let mcp_config = match app_handle.as_ref() {
            Some(handle) => match juno_mcp::ensure_running(handle).await {
                Ok(endpoint) => write_mcp_config(&endpoint)
                    .map_err(|e| warn!("Juno's tool server is up but its config is not: {e}"))
                    .ok(),
                Err(e) => {
                    // Non-fatal, and deliberately loud: the CLI still answers,
                    // it just cannot touch the desktop.
                    warn!("Could not offer Juno's computer tool to the CLI: {e}");
                    None
                }
            },
            None => None,
        };

        // Experimental (off by default): run this turn in one long-lived process
        // kept alive for the conversation, instead of spawning a fresh one here.
        // Worth ~1.6-3.1s per follow-up — see docs/plans/cli-persistent-session-spike.md.
        //
        // Every failure path returns `Unavailable` having emitted nothing, and falls
        // through to the one-shot spawn below. That fallback resumes the same session
        // id the persistent process was pinned to, so no context is lost either way.
        if let (Some(handle), Some(conversation_id)) = (app_handle.as_ref(), conversation.as_ref())
        {
            if claude_cli_session::is_enabled(handle) {
                // Continue this conversation's CLI session if it has one, otherwise
                // mint an id for the process to pin. The distinction matters: the CLI
                // refuses to start if handed --session-id for a session that already
                // exists, so an existing id has to arrive as --resume instead.
                let (session_id, session_is_new) = match resume.clone() {
                    Some(existing) => (existing, false),
                    None => (uuid::Uuid::new_v4().to_string(), true),
                };

                let request = claude_cli_session::TurnRequest {
                    binary: &self.binary_path,
                    model: &self.model,
                    system_prompt: self.system_prompt.as_deref(),
                    mcp_config: mcp_config.as_deref(),
                    mcp_guidance: MCP_TOOL_GUIDANCE,
                    conversation_id,
                    session_id: &session_id,
                    session_is_new,
                    query,
                    app_handle: handle,
                    message_id: message_id.clone(),
                    cancel_rx: cancel_rx.clone(),
                };

                match claude_cli_session::run_turn(request).await {
                    Ok(claude_cli_session::TurnOutcome::Completed(text)) => {
                        remember_session(conversation_id, &session_id);
                        return Ok(text);
                    }
                    Ok(claude_cli_session::TurnOutcome::Unavailable) => {
                        debug!("Persistent Claude CLI session unavailable; spawning one-shot");
                    }
                    Err(e) => {
                        // The turn actually started and then failed. Re-running it on
                        // the one-shot path would bill the user twice for one message.
                        remember_session(conversation_id, &session_id);
                        return Err(e);
                    }
                }
            }
        }

        let msg_id = message_id.unwrap_or_else(|| uuid::Uuid::new_v4().to_string());

        // Open the bubble once, before any spawn. The old-CLI retry below can
        // run the query twice, and the frontend appends a fresh assistant
        // message on every start it sees — a second one would leave an empty
        // bubble stranded above the answer. Being above the spawn also puts
        // it above two failure exits, which is why this is a guard: it closes
        // on drop, so no exit from here on can strand a spinner.
        let mut stream = StreamSurface::open(&app_handle, msg_id.clone());

        // Prefer the run's cancellation channel (for session-tracked runs this
        // is the merged session+global receiver — escape cancels only the
        // focused session's token, which the global channel never sees;
        // LAC-3697). Fall back to the legacy global AppState channel for
        // callers that don't thread a receiver through.
        let cancel_rx = cancel_rx.or_else(|| {
            app_handle.as_ref().and_then(|handle| {
                use tauri::Manager;
                handle
                    .try_state::<crate::state::AppState>()
                    .map(|state| state.cancel_rx.clone())
            })
        });

        // Keep a copy for the post-stream check below — a cancel can land in
        // the instant between the stream finishing and the race resolving.
        let post_cancel_rx = cancel_rx.clone();

        // Ask for partial messages unless an earlier query this session
        // already discovered this `claude` build will not take the flag.
        let mut include_partial_messages = !PARTIAL_MESSAGES_UNSUPPORTED.load(Ordering::Relaxed);
        // Both fallbacks below are one-shot, and both guards live here rather
        // than inside the loop body, so one query can never turn into an
        // unbounded chain of respawns however they interleave.
        let mut partial_fallback_used = false;
        let mut resume_fallback_used = false;
        let mut resume = resume;

        let (accumulated_text, final_result, status) = loop {
            let mut args = self.build_args(query, resume.as_deref(), mcp_config.as_deref());
            if !include_partial_messages {
                strip_partial_messages(&mut args);
            }

            match resume.as_deref() {
                Some(session) => info!(
                    "Spawning Claude CLI (resuming session {}): {}",
                    session,
                    self.binary_path.display()
                ),
                None => info!(
                    "Spawning Claude CLI (new session): {} {}",
                    self.binary_path.display(),
                    args.iter().take(6).cloned().collect::<Vec<_>>().join(" ")
                ),
            }

            let spawn = |args: &Vec<String>| {
                tokio::process::Command::new(&self.binary_path)
                    .args(args)
                    .stdout(Stdio::piped())
                    .stderr(Stdio::piped())
                    .stdin(Stdio::null())
                    .kill_on_drop(true)
                    .spawn()
            };

            let mut child = match spawn(&args) {
                Ok(child) => child,
                // A session id we stored can stop being resumable: the CLI
                // prunes its history, or the transcript is gone. Falling back
                // to a fresh session costs the thread's context, which is
                // exactly what we had before this existed, and beats failing
                // the message outright.
                Err(e) if resume.is_some() && !resume_fallback_used => {
                    resume_fallback_used = true;
                    warn!(
                        "Claude CLI would not resume ({}); starting a fresh session",
                        e
                    );
                    if let Some(ref id) = conversation {
                        forget_session(id);
                    }
                    resume = None;
                    let mut fresh = self.build_args(query, None, mcp_config.as_deref());
                    if !include_partial_messages {
                        strip_partial_messages(&mut fresh);
                    }
                    match spawn(&fresh) {
                        Ok(child) => child,
                        Err(e) => {
                            let failure = format!("Failed to spawn Claude CLI: {}", e);
                            stream.close(format!("Error: {}", failure));
                            return Err(AgentError::LlmError(failure));
                        }
                    }
                }
                Err(e) => {
                    let failure = format!("Failed to spawn Claude CLI: {}", e);
                    stream.close(format!("Error: {}", failure));
                    return Err(AgentError::LlmError(failure));
                }
            };

            let stdout = child.stdout.take().ok_or_else(|| {
                AgentError::LlmError("Failed to capture Claude CLI stdout".to_string())
            })?;

            // Drain stderr concurrently to prevent pipe buffer deadlocks.
            // If the child writes >64KB to stderr while we only read stdout,
            // both processes would block forever. It is also the only place
            // an argument the CLI refused is ever reported.
            let stderr = child.stderr.take();
            let stderr_handle = stderr.map(|se| {
                tauri::async_runtime::spawn(async move {
                    let mut buf = String::new();
                    let mut reader = BufReader::new(se);
                    let _ = tokio::io::AsyncReadExt::read_to_string(&mut reader, &mut buf).await;
                    buf
                })
            });

            // Run the streaming loop with a timeout, cancellable via escape
            // key. tokio::select! races the stream against the cancellation
            // signal — whichever completes first wins, and the other branch
            // is dropped.
            let stream_future = tokio::time::timeout(CLI_TIMEOUT, async {
                self.process_stream(stdout, &app_handle, &msg_id).await
            });

            let (accumulated_text, final_result) = if let Some(mut rx) = cancel_rx.clone() {
                tokio::select! {
                    result = stream_future => {
                        match result {
                            Ok(Ok(r)) => r,
                            Ok(Err(e)) => {
                                stream.close(format!("Error: {}", e));
                                return Err(e);
                            }
                            Err(_elapsed) => {
                                stream.close("Claude CLI timed out".to_string());
                                return Err(AgentError::Timeout(format!(
                                    "Claude CLI timed out after {} seconds",
                                    CLI_TIMEOUT.as_secs()
                                )));
                            }
                        }
                    }
                    // Loop on changed() + borrow() instead of wait_for() to avoid
                    // holding a non-Send RwLockReadGuard across the select! boundary.
                    _ = async {
                        loop {
                            if *rx.borrow() { return; }
                            if rx.changed().await.is_err() {
                                // Sender dropped — will never cancel
                                std::future::pending::<()>().await;
                            }
                        }
                    } => {
                        info!("Claude CLI cancelled via escape key, killing subprocess");
                        let _ = child.kill().await;
                        stream.close("Cancelled".to_string());
                        return Err(AgentError::Terminated);
                    }
                }
            } else {
                // No AppState available (headless/test) — fall back to timeout-only
                match stream_future.await {
                    Ok(Ok(r)) => r,
                    Ok(Err(e)) => {
                        stream.close(format!("Error: {}", e));
                        return Err(e);
                    }
                    Err(_elapsed) => {
                        stream.close("Claude CLI timed out".to_string());
                        return Err(AgentError::Timeout(format!(
                            "Claude CLI timed out after {} seconds",
                            CLI_TIMEOUT.as_secs()
                        )));
                    }
                }
            };

            // If cancellation landed after the stream completed but before the
            // race resolved, discard the response — the user asked to stop, so
            // nothing should be rendered or spoken (LAC-3697).
            if post_cancel_rx
                .as_ref()
                .map(|rx| *rx.borrow())
                .unwrap_or(false)
            {
                info!("Claude CLI cancelled after stream completion, discarding response");
                let _ = child.kill().await;
                stream.close("Cancelled".to_string());
                return Err(AgentError::Terminated);
            }

            // Wait for subprocess to finish
            let status = child
                .wait()
                .await
                .map_err(|e| AgentError::LlmError(format!("Claude CLI process error: {}", e)))?;

            // Collect stderr — logged either way for debuggability, and read
            // below to tell "this CLI is too old" from a real failure.
            let stderr_buf = match stderr_handle {
                Some(handle) => match handle.await {
                    Ok(buf) => buf,
                    Err(e) => {
                        error!("Failed to read Claude CLI stderr: {}", e);
                        String::new()
                    }
                },
                None => String::new(),
            };
            if !stderr_buf.is_empty() {
                if status.success() {
                    warn!(
                        "Claude CLI stderr (success): {}",
                        stderr_buf.chars().take(500).collect::<String>()
                    );
                } else {
                    error!(
                        "Claude CLI stderr (failure): {}",
                        stderr_buf.chars().take(500).collect::<String>()
                    );
                }
            }

            // A CLI too old for --include-partial-messages spawns fine, then
            // exits non-zero having printed nothing. Without this every query
            // would fail outright until the user upgraded, so retry once with
            // the flag stripped: the whole-message `assistant` path is still
            // in `process_stream` and gives exactly the pre-branch behaviour.
            let produced_nothing = accumulated_text.is_empty()
                && final_result.as_deref().map(str::is_empty).unwrap_or(true);
            if include_partial_messages
                && !partial_fallback_used
                && !status.success()
                && produced_nothing
                && rejects_partial_messages(&stderr_buf)
            {
                partial_fallback_used = true;
                include_partial_messages = false;
                PARTIAL_MESSAGES_UNSUPPORTED.store(true, Ordering::Relaxed);
                warn!(
                    "This `claude` build rejected {}; retrying without it. Update your Claude CLI to see Juno's reasoning and tool actions live instead of only the final answer.",
                    PARTIAL_MESSAGES_FLAG
                );
                continue;
            }

            break (accumulated_text, final_result, status);
        };

        // Remember the session this conversation is now in, so the next
        // message continues it. Recorded here rather than mid-stream because
        // a cancelled or failed run has already returned above, and a session
        // that produced nothing is not one worth resuming into.
        if let Some(ref id) = conversation {
            let observed = self
                .observed_session
                .lock()
                .ok()
                .and_then(|slot| slot.clone());
            match observed {
                Some(session) => remember_session(id, &session),
                // The CLI always reports one, so this means the stream ended
                // before any usable line arrived. Dropping the old id is the
                // safe move: resuming into a session we cannot confirm is
                // worse than starting cleanly.
                None => forget_session(id),
            }
        }

        // Use final_result if available, otherwise accumulated_text
        let complete_text = final_result.unwrap_or(accumulated_text);

        // Close the bubble on the answer. The guard would close it empty if
        // we did not, which is the whole point of it being a guard.
        stream.close(complete_text.clone());

        if complete_text.is_empty() && !status.success() {
            return Err(AgentError::LlmError(format!(
                "Claude CLI exited with status {} and no output",
                status
            )));
        }

        Ok(complete_text)
    }

    /// Process the stdout stream from the Claude CLI subprocess.
    /// Returns (accumulated_text, final_result).
    ///
    /// Uses character count (not byte count) for delta tracking to avoid
    /// panics on multi-byte UTF-8 content (emoji, CJK, accented chars).
    async fn process_stream(
        &self,
        stdout: tokio::process::ChildStdout,
        app_handle: &Option<tauri::AppHandle>,
        msg_id: &str,
    ) -> Result<(String, Option<String>), AgentError> {
        let reader = BufReader::new(stdout);
        let mut lines = reader.lines();

        // Display text with <TTS> blocks removed; what the UI has been shown.
        let mut accumulated_text = String::new();
        let mut previous_char_count: usize = 0;
        let mut final_result: Option<String> = None;
        // Shared <TTS> extraction so the CLI speaks like the API provider.
        let mut tts_stream = crate::agent::tts_tags::TtsTagStream::new();
        let mut spoken_blocks: Vec<String> = Vec::new();

        // Open content blocks in the partial-message stream, keyed by the
        // index the CLI stamps on every delta belonging to them.
        let mut blocks: std::collections::HashMap<u64, StreamBlock> =
            std::collections::HashMap::new();
        // Assistant message ids whose text has already gone out one
        // `text_delta` at a time. The whole-message `assistant` event for
        // those ids must stay silent — see the `assistant` arm below.
        let mut streamed_messages: std::collections::HashSet<String> =
            std::collections::HashSet::new();
        // The message the partial stream is currently inside. Deltas do not
        // carry a message id, only the enclosing `message_start` does.
        let mut partial_message_id: Option<String> = None;
        // Has the partial stream produced any text at all? Belt and braces
        // for the id-less case: if a `text_delta` ever arrived, the fast path
        // is working, so an `assistant` message we cannot match by id must
        // still be assumed already shown rather than emitted twice.
        let mut saw_text_delta = false;
        // Announced tools with no result yet: tool_use_id -> display label.
        let mut pending_tools: std::collections::HashMap<String, String> =
            std::collections::HashMap::new();
        // Which assistant message `previous_char_count` is counting. A run of
        // tool calls produces several, each starting from zero.
        let mut fallback_message_id: Option<String> = None;

        loop {
            match lines.next_line().await {
                Ok(Some(line)) => {
                    if line.trim().is_empty() {
                        continue;
                    }

                    let parsed: Value = match serde_json::from_str(&line) {
                        Ok(v) => v,
                        Err(e) => {
                            debug!(
                                "Non-JSON line from Claude CLI: {} ({})",
                                line.chars().take(80).collect::<String>(),
                                e
                            );
                            continue;
                        }
                    };

                    let event_type = parsed.get("type").and_then(|v| v.as_str()).unwrap_or("");

                    // Every stream-json line carries the session this run is
                    // in. Recorded from whichever line shows up first, so the
                    // next message in this conversation can resume it.
                    if let Some(session) = parsed.get("session_id").and_then(|v| v.as_str()) {
                        if let Ok(mut slot) = self.observed_session.lock() {
                            if slot.as_deref() != Some(session) {
                                debug!("Claude CLI session: {}", session);
                                *slot = Some(session.to_string());
                            }
                        }
                    }

                    match event_type {
                        // The raw streaming events, one step ahead of the
                        // finished messages. This is where reasoning, tool
                        // names and tool arguments become visible while the
                        // model is still working rather than after it stops.
                        "stream_event" => {
                            if let Some(event) = parsed.get("event") {
                                Self::handle_stream_event(
                                    event,
                                    app_handle,
                                    msg_id,
                                    &mut blocks,
                                    &mut pending_tools,
                                    &mut streamed_messages,
                                    &mut partial_message_id,
                                    &mut saw_text_delta,
                                    &mut tts_stream,
                                    &mut accumulated_text,
                                    &mut spoken_blocks,
                                );
                            }
                        }
                        // Tool results come back as a `user` turn. The action
                        // is over, so whatever indicator it raised comes down
                        // — otherwise a finished tool spins forever.
                        "user" => {
                            if let Some(content) = parsed
                                .get("message")
                                .and_then(|m| m.get("content"))
                                .and_then(|c| c.as_array())
                            {
                                for block in content {
                                    if block.get("type").and_then(|v| v.as_str())
                                        != Some("tool_result")
                                    {
                                        continue;
                                    }
                                    let tool_use_id = block
                                        .get("tool_use_id")
                                        .and_then(|v| v.as_str())
                                        .unwrap_or("");
                                    let label = pending_tools
                                        .remove(tool_use_id)
                                        .unwrap_or_else(|| "tool".to_string());
                                    if let Some(ref handle) = app_handle {
                                        crate::agent::tool_logger::emit_tool_pending_cleared(
                                            handle,
                                            msg_id,
                                            tool_use_id,
                                            &label,
                                        );
                                    }
                                }
                            }
                        }
                        "assistant" => {
                            // Fallback path. Kept for CLI builds that do not
                            // support --include-partial-messages, where this
                            // whole-message diff is the only source of text.
                            if let Some(message) = parsed.get("message") {
                                let message_id = message
                                    .get("id")
                                    .and_then(|v| v.as_str())
                                    .map(|s| s.to_string());

                                // When the partial stream is live it has
                                // already put every word of this message on
                                // screen. Running the diff as well would
                                // render each word twice and, worse, push the
                                // whole text through the TTS splitter a
                                // second time — the answer spoken twice over.
                                let already_streamed = match message_id.as_ref() {
                                    Some(id) => streamed_messages.contains(id),
                                    // No id to match on. If the partial
                                    // stream has produced text at all, it is
                                    // the live path and this is a replay.
                                    None => saw_text_delta,
                                };

                                if already_streamed {
                                    debug!(
                                        "Claude CLI assistant message already streamed as deltas; skipping whole-message diff"
                                    );
                                } else {
                                    // Each assistant message diffs from its
                                    // own zero. Without this reset the second
                                    // message in a tool loop is measured
                                    // against the first one's length and its
                                    // opening words never appear.
                                    if fallback_message_id != message_id {
                                        fallback_message_id = message_id.clone();
                                        previous_char_count = 0;
                                    }

                                    let text = extract_text_from_message(message);
                                    let char_count = text.chars().count();

                                    if char_count > previous_char_count {
                                        // Char offsets, not bytes — safe for
                                        // multi-byte UTF-8.
                                        let delta: String =
                                            text.chars().skip(previous_char_count).collect();
                                        previous_char_count = char_count;
                                        Self::emit_display_text(
                                            app_handle,
                                            msg_id,
                                            &delta,
                                            &mut tts_stream,
                                            &mut accumulated_text,
                                            &mut spoken_blocks,
                                        );
                                    }
                                }
                            }
                        }
                        "result" => {
                            // Final result — extract the result text
                            if let Some(result_text) = parsed.get("result").and_then(|v| v.as_str())
                            {
                                final_result = Some(result_text.to_string());
                            }

                            // Log cost info if available
                            if let Some(cost) = parsed.get("cost_usd").and_then(|v| v.as_f64()) {
                                info!("Claude CLI query cost: ${:.4}", cost);
                            }
                            if let Some(duration) =
                                parsed.get("duration_ms").and_then(|v| v.as_u64())
                            {
                                info!("Claude CLI query duration: {}ms", duration);
                            }
                        }
                        "system" => {
                            debug!(
                                "Claude CLI system event: {}",
                                parsed
                                    .get("subtype")
                                    .and_then(|v| v.as_str())
                                    .unwrap_or("unknown")
                            );
                        }
                        other => {
                            debug!("Claude CLI event type '{}': skipped", other);
                        }
                    }
                }
                Ok(None) => break, // EOF
                Err(e) => {
                    warn!("Error reading Claude CLI stdout: {}", e);
                    break;
                }
            }
        }

        // Nothing may be left spinning once the stream is over. A tool that
        // was cancelled, errored, or whose result never arrived has no
        // `tool_result` to clear it, and a thinking block the CLI never
        // stopped would stay open in the UI forever.
        close_open_thinking(app_handle, &mut blocks);
        clear_pending_tools(app_handle, msg_id, &mut pending_tools);

        // Flush the parser: a partial tag becomes display text, an unterminated
        // block is still spoken.
        let (tail_display, tail_spoken) = tts_stream.finish();
        if !tail_display.is_empty() || !tail_spoken.is_empty() {
            accumulated_text.push_str(&tail_display);
            if let Some(ref handle) = app_handle {
                Self::emit_chunk_with_tts(handle, tail_display, msg_id, &tail_spoken);
            }
            spoken_blocks.extend(tail_spoken);
        }

        // The `result` event carries the raw final text, tags included. Strip
        // it for display and speak any block the assistant events did not
        // already cover (older CLI builds emit no assistant events).
        let final_result = final_result.map(|raw| {
            // Named apart from the content-block map above, which is a
            // different `blocks` entirely.
            let (display, tts_blocks) = crate::agent::tts_tags::split_tts_tags(&raw);
            let unspoken: Vec<String> = tts_blocks
                .into_iter()
                .filter(|b| !spoken_blocks.contains(b))
                .collect();
            if !unspoken.is_empty() {
                info!(
                    "Speaking {} TTS block(s) found only in the Claude CLI result",
                    unspoken.len()
                );
                if let Some(ref handle) = app_handle {
                    Self::emit_chunk_with_tts(handle, String::new(), msg_id, &unspoken);
                }
            }
            display
        });

        Ok((accumulated_text, final_result))
    }

    /// Handle one inner event from a `stream_event` line.
    ///
    /// The inner event mirrors the Anthropic streaming API exactly, so the
    /// shapes here are the same ones `anthropic.rs` parses.
    #[allow(clippy::too_many_arguments)]
    fn handle_stream_event(
        event: &Value,
        app_handle: &Option<tauri::AppHandle>,
        msg_id: &str,
        blocks: &mut std::collections::HashMap<u64, StreamBlock>,
        pending_tools: &mut std::collections::HashMap<String, String>,
        streamed_messages: &mut std::collections::HashSet<String>,
        partial_message_id: &mut Option<String>,
        saw_text_delta: &mut bool,
        tts_stream: &mut crate::agent::tts_tags::TtsTagStream,
        accumulated_text: &mut String,
        spoken_blocks: &mut Vec<String>,
    ) {
        let inner_type = event.get("type").and_then(|v| v.as_str()).unwrap_or("");
        let index = event.get("index").and_then(|v| v.as_u64()).unwrap_or(0);

        match inner_type {
            "message_start" => {
                *partial_message_id = event
                    .get("message")
                    .and_then(|m| m.get("id"))
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string());
                // Block indices restart at zero for every message, so
                // anything still open belongs to the previous one.
                close_open_thinking(app_handle, blocks);
            }
            "content_block_start" => {
                let block = event.get("content_block");
                let block_type = block
                    .and_then(|b| b.get("type"))
                    .and_then(|v| v.as_str())
                    .unwrap_or("");

                match block_type {
                    "thinking" | "redacted_thinking" => {
                        // A fresh id, never `msg_id`: the frontend's
                        // text-stream handler matches on message id alone,
                        // so sharing one would append the answer's text to
                        // the reasoning surface as well.
                        let thinking_id = uuid::Uuid::new_v4().to_string();
                        if let Some(handle) = app_handle {
                            crate::agent::tool_logger::emit_thinking_start(
                                handle,
                                thinking_id.clone(),
                            );
                        }
                        blocks.insert(
                            index,
                            StreamBlock::Thinking {
                                thinking_id,
                                text: String::new(),
                            },
                        );
                    }
                    "tool_use" | "server_tool_use" | "mcp_tool_use" => {
                        let name = block
                            .and_then(|b| b.get("name"))
                            .and_then(|v| v.as_str())
                            .unwrap_or("tool")
                            .to_string();
                        let tool_use_id = block
                            .and_then(|b| b.get("id"))
                            .and_then(|v| v.as_str())
                            .map(|s| s.to_string())
                            .unwrap_or_else(|| format!("block-{}", index));
                        let label = friendly_tool_name(&name).to_string();
                        // The name arrives a full beat before the arguments.
                        // Say what is coming now, sharpen it as the
                        // arguments land.
                        let announced = format!("Using {}", label);
                        if let Some(handle) = app_handle {
                            crate::agent::tool_logger::emit_tool_pending(
                                handle,
                                msg_id,
                                &tool_use_id,
                                &label,
                                &announced,
                            );
                        }
                        pending_tools.insert(tool_use_id.clone(), label.clone());
                        blocks.insert(
                            index,
                            StreamBlock::ToolUse {
                                tool_use_id,
                                name,
                                label,
                                partial_json: String::new(),
                                announced,
                            },
                        );
                    }
                    // Plain text needs no bookkeeping: its deltas go straight
                    // out, and nothing has to be closed when it stops.
                    _ => {}
                }
            }
            "content_block_delta" => {
                let delta = match event.get("delta") {
                    Some(delta) => delta,
                    None => return,
                };

                match delta.get("type").and_then(|v| v.as_str()).unwrap_or("") {
                    "text_delta" => {
                        if let Some(text) = delta.get("text").and_then(|v| v.as_str()) {
                            // Mark the message as served by this path so the
                            // whole-message `assistant` event stays quiet.
                            *saw_text_delta = true;
                            if let Some(id) = partial_message_id.as_ref() {
                                streamed_messages.insert(id.clone());
                            }
                            Self::emit_display_text(
                                app_handle,
                                msg_id,
                                text,
                                tts_stream,
                                accumulated_text,
                                spoken_blocks,
                            );
                        }
                    }
                    "thinking_delta" => {
                        if let Some(text) = delta.get("thinking").and_then(|v| v.as_str()) {
                            if let Some(StreamBlock::Thinking {
                                thinking_id,
                                text: accumulated,
                            }) = blocks.get_mut(&index)
                            {
                                accumulated.push_str(text);
                                if let Some(handle) = app_handle {
                                    crate::agent::tool_logger::emit_thinking_chunk(
                                        handle,
                                        text.to_string(),
                                        Some(thinking_id.clone()),
                                    );
                                }
                            }
                        }
                    }
                    "input_json_delta" => {
                        if let Some(fragment) = delta.get("partial_json").and_then(|v| v.as_str()) {
                            if let Some(StreamBlock::ToolUse {
                                tool_use_id,
                                name,
                                label,
                                partial_json,
                                announced,
                            }) = blocks.get_mut(&index)
                            {
                                partial_json.push_str(fragment);
                                // Read the arguments while they are still
                                // arriving, so "Clicking 640, 60" shows
                                // before the pointer moves rather than after.
                                if let Some(input) = parse_partial_json(partial_json.as_str()) {
                                    if let Some(description) =
                                        describe_tool_call(name.as_str(), &input)
                                    {
                                        if description != *announced {
                                            announced.clear();
                                            announced.push_str(&description);
                                            if let Some(handle) = app_handle {
                                                crate::agent::tool_logger::emit_tool_pending(
                                                    handle,
                                                    msg_id,
                                                    tool_use_id.as_str(),
                                                    label.as_str(),
                                                    &description,
                                                );
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                    // `signature_delta` and friends carry nothing the UI
                    // shows; the Anthropic provider ignores them here too.
                    other => {
                        debug!("Claude CLI stream delta '{}': skipped", other);
                    }
                }
            }
            "content_block_stop" => {
                // A tool block stays in `pending_tools` on purpose: its
                // arguments are complete but the action has not run yet.
                if let Some(StreamBlock::Thinking { thinking_id, text }) = blocks.remove(&index) {
                    if let Some(handle) = app_handle {
                        crate::agent::tool_logger::emit_thinking_end(handle, thinking_id, text);
                    }
                }
            }
            "message_stop" => {
                close_open_thinking(app_handle, blocks);
                *partial_message_id = None;
            }
            _ => {}
        }
    }

    /// The single funnel for assistant display text.
    ///
    /// Both the `text_delta` fast path and the whole-message `assistant`
    /// fallback come through here, in stream order, so the spoken channel is
    /// split out of the text exactly once and in sequence. Routing either one
    /// around this would silently break voice output: `TtsTagStream` is a
    /// running parser, and text that skips it takes its `<TTS>` tags along
    /// into the visible answer.
    fn emit_display_text(
        app_handle: &Option<tauri::AppHandle>,
        msg_id: &str,
        delta: &str,
        tts_stream: &mut crate::agent::tts_tags::TtsTagStream,
        accumulated_text: &mut String,
        spoken_blocks: &mut Vec<String>,
    ) {
        if delta.is_empty() {
            return;
        }

        let (display_delta, tts_blocks) = tts_stream.push(delta);
        for spoken in &tts_blocks {
            info!("Extracted TTS content from Claude CLI: '{}'", spoken);
        }
        accumulated_text.push_str(&display_delta);
        if let Some(handle) = app_handle {
            Self::emit_chunk_with_tts(handle, display_delta, msg_id, &tts_blocks);
        }
        spoken_blocks.extend(tts_blocks);
    }

    /// Emit one display chunk plus every spoken block. The first block rides on
    /// the text chunk; extra blocks go out as TTS-only chunks, matching the
    /// Anthropic provider's event shape.
    fn emit_chunk_with_tts(
        handle: &tauri::AppHandle,
        display: String,
        msg_id: &str,
        tts_blocks: &[String],
    ) {
        if display.is_empty() && tts_blocks.is_empty() {
            return;
        }
        crate::agent::tool_logger::emit_streaming_text_chunk(
            handle,
            display,
            Some(msg_id.to_string()),
            tts_blocks.first().cloned(),
        );
        for spoken in tts_blocks.iter().skip(1) {
            crate::agent::tool_logger::emit_streaming_text_chunk(
                handle,
                String::new(),
                Some(msg_id.to_string()),
                Some(spoken.clone()),
            );
        }
    }
}

#[async_trait]
impl AgentBrain for ClaudeCliBrain {
    async fn decide_next_action(
        &self,
        messages: &[Message],
        _available_tools: &[ToolDefinition],
    ) -> Result<AgentAction, AgentError> {
        let query = Self::extract_query(messages);
        let result = self.run_streaming(&query, None, None, None).await?;
        Ok(AgentAction::Finish(result))
    }

    fn supports_streaming(&self) -> bool {
        true
    }

    async fn decide_next_action_streaming(
        &self,
        messages: &[Message],
        _available_tools: &[ToolDefinition],
        app_handle: Option<tauri::AppHandle>,
        message_id: Option<String>,
        cancel_rx: Option<crate::state::CancelReceiver>,
    ) -> Result<AgentAction, AgentError> {
        let query = Self::extract_query(messages);
        let result = self
            .run_streaming(&query, app_handle, message_id, cancel_rx)
            .await?;
        Ok(AgentAction::Finish(result))
    }
}

/// The assistant bubble for one run: opened on construction, guaranteed to be
/// closed however the run ends.
///
/// `run_streaming` leaves by roughly eight different doors — cancellation
/// before and after the stream, timeout, stream error, two spawn failures,
/// success, and whatever gets added next. The frontend appends a message on
/// `stream_start` and only stops its spinner on `stream_end`, so a door that
/// forgets to close leaves a bubble spinning in the conversation forever.
/// Pairing them by hand worked while the start sat below the spawn; once it
/// moved above the retry loop it stopped working, and the fix that scales is
/// to make the close impossible to forget rather than to remember it in eight
/// places.
struct StreamSurface {
    /// A clone, so the caller keeps using its own `app_handle` freely.
    app_handle: Option<tauri::AppHandle>,
    msg_id: String,
    open: bool,
}

impl StreamSurface {
    /// Open the bubble. From here on it closes no matter how the run ends.
    fn open(app_handle: &Option<tauri::AppHandle>, msg_id: String) -> Self {
        if let Some(handle) = app_handle {
            crate::agent::tool_logger::emit_stream_start(handle, msg_id.clone());
        }
        Self {
            app_handle: app_handle.clone(),
            msg_id,
            open: true,
        }
    }

    /// Close it on the text the person should be left looking at. Idempotent,
    /// so a path that closes explicitly and then drops emits once.
    fn close(&mut self, text: String) {
        if !self.open {
            return;
        }
        self.open = false;
        if let Some(ref handle) = self.app_handle {
            crate::agent::tool_logger::emit_stream_end(handle, self.msg_id.clone(), text);
        }
    }
}

impl Drop for StreamSurface {
    fn drop(&mut self) {
        if self.open {
            // An exit that did not say what to show. Better an empty bubble
            // than one that spins for the rest of the session.
            warn!("Claude CLI run ended without closing its stream; closing it empty");
            self.close(String::new());
        }
    }
}

/// One open content block in the partial-message stream.
///
/// `content_block_stop` carries only an index, never the kind of block it is
/// closing, so the kind has to be remembered from its `content_block_start`.
/// Plain text blocks are not tracked: their deltas go straight to the UI and
/// there is nothing to close.
enum StreamBlock {
    Thinking {
        /// The reasoning surface this block is streaming into. Always its own
        /// id, never the run's `msg_id`.
        thinking_id: String,
        /// Everything streamed so far, replayed on `thinking_end`.
        text: String,
    },
    ToolUse {
        tool_use_id: String,
        /// The tool as the CLI names it, e.g. `mcp__juno__computer`.
        name: String,
        /// The same tool as a person would say it, e.g. `computer`.
        label: String,
        /// Arguments so far. Arrives a few characters per delta.
        partial_json: String,
        /// The last description sent for this tool, so a sharper reading of
        /// the same arguments replaces it and an identical one does not.
        announced: String,
    },
}

/// Close every reasoning surface still open.
///
/// Called when a message ends and when the stream does: a thinking block the
/// CLI never stopped would otherwise sit spinning in the UI for good.
fn close_open_thinking(
    app_handle: &Option<tauri::AppHandle>,
    blocks: &mut std::collections::HashMap<u64, StreamBlock>,
) {
    let open: Vec<StreamBlock> = blocks.drain().map(|(_, block)| block).collect();
    if let Some(handle) = app_handle {
        for block in open {
            if let StreamBlock::Thinking { thinking_id, text } = block {
                crate::agent::tool_logger::emit_thinking_end(handle, thinking_id, text);
            }
        }
    }
}

/// Take down every pending-tool indicator that never got a result — a
/// cancelled run, a tool that errored, a stream that simply ended.
fn clear_pending_tools(
    app_handle: &Option<tauri::AppHandle>,
    msg_id: &str,
    pending_tools: &mut std::collections::HashMap<String, String>,
) {
    let stale: Vec<(String, String)> = pending_tools.drain().collect();
    if let Some(handle) = app_handle {
        for (tool_use_id, label) in stale {
            crate::agent::tool_logger::emit_tool_pending_cleared(
                handle,
                msg_id,
                &tool_use_id,
                &label,
            );
        }
    }
}

/// Resolve the `--effort` level to pass the CLI.
///
/// Hidden advanced setting: `providers[].effort` in the Tauri settings store,
/// with no UI that reads or writes it. A value the CLI would not accept is
/// dropped rather than passed through, so a stale or hand-edited store cannot
/// make every single spawn fail on an unknown argument.
fn resolve_effort(configured: Option<&str>) -> String {
    use crate::constants::settings::defaults::{CLAUDE_CLI_EFFORT, CLAUDE_CLI_EFFORT_LEVELS};

    match configured {
        Some(level) if CLAUDE_CLI_EFFORT_LEVELS.contains(&level) => level.to_string(),
        Some(other) => {
            warn!(
                "Ignoring unknown Claude CLI effort level '{}'; using '{}'",
                other, CLAUDE_CLI_EFFORT
            );
            CLAUDE_CLI_EFFORT.to_string()
        }
        None => CLAUDE_CLI_EFFORT.to_string(),
    }
}

/// Drop [`PARTIAL_MESSAGES_FLAG`] from an argument list, for a `claude` build
/// too old to accept it. The flag is standalone, so nothing follows it to
/// remove as well.
fn strip_partial_messages(args: &mut Vec<String>) {
    args.retain(|arg| arg != PARTIAL_MESSAGES_FLAG);
}

/// Did the CLI refuse [`PARTIAL_MESSAGES_FLAG`]?
///
/// A subprocess's stderr is the only signal there is — the CLI has no
/// structured channel for "I do not know that argument", so the usual
/// no-string-matching-on-errors rule has nothing better to offer here.
///
/// The match is on the flag name alone, deliberately. Every version phrases
/// the rejection differently ("unknown option", "unrecognized option", a bare
/// usage dump) and that wording will keep drifting, whereas the flag name is
/// the thing we passed and the thing being complained about. The caller pairs
/// this with a non-zero exit and completely empty output, which is what makes
/// it safe to be this loose: a build that supports the flag has no reason to
/// name it on stderr while failing to produce a single line on stdout. A
/// false positive costs one extra spawn on a query that was already failing.
fn rejects_partial_messages(stderr: &str) -> bool {
    stderr.contains(PARTIAL_MESSAGES_FLAG)
}

/// The tool as a person would say it.
///
/// MCP tools reach the CLI as `mcp__<server>__<tool>`; nobody needs to read
/// the plumbing, they need to read "computer".
fn friendly_tool_name(raw: &str) -> &str {
    raw.rsplit("__").next().unwrap_or(raw)
}

/// Shorten text for a one-line indicator without ever byte-slicing it.
fn truncate_for_display(text: &str, max_chars: usize) -> String {
    let trimmed = text.trim();
    if trimmed.chars().count() <= max_chars {
        return trimmed.to_string();
    }
    format!(
        "{}...",
        trimmed
            .chars()
            .take(max_chars.saturating_sub(3))
            .collect::<String>()
    )
}

/// The last path segment of a file path, for a shorter indicator.
fn file_label(path: &str) -> String {
    let name = path.rsplit('/').next().unwrap_or(path);
    let name = if name.is_empty() { path } else { name };
    truncate_for_display(name, 40)
}

/// Best-effort parse of tool arguments that are still arriving.
///
/// `input_json_delta` hands the arguments over a few characters at a time,
/// and waiting for the closing brace is waiting for the action itself. So we
/// close whatever is still open and parse that. A fragment too early to mean
/// anything yields None, and the next delta is tried instead.
fn parse_partial_json(partial: &str) -> Option<Value> {
    let trimmed = partial.trim();
    if trimmed.is_empty() {
        return None;
    }
    if let Ok(value) = serde_json::from_str::<Value>(trimmed) {
        return Some(value);
    }
    if let Some(value) = close_open_json(trimmed) {
        return Some(value);
    }
    // A field cut mid-value cannot be closed into anything valid, so fall
    // back to everything before it: `"action":"left_click","coordinate":[64`
    // still tells us a click is coming.
    let shorter = truncate_at_last_comma(trimmed)?;
    close_open_json(&shorter)
}

/// Append the closers for every string, array and object left open, then
/// parse. Returns None if the result is still not valid JSON.
fn close_open_json(fragment: &str) -> Option<Value> {
    let mut stack: Vec<char> = Vec::new();
    let mut in_string = false;
    let mut escaped = false;

    for ch in fragment.chars() {
        if in_string {
            if escaped {
                escaped = false;
            } else if ch == '\\' {
                escaped = true;
            } else if ch == '"' {
                in_string = false;
            }
            continue;
        }
        match ch {
            '"' => in_string = true,
            '{' => stack.push('}'),
            '[' => stack.push(']'),
            '}' | ']' => {
                stack.pop();
            }
            _ => {}
        }
    }

    let mut repaired = fragment.to_string();
    // A trailing backslash would escape the quote we are about to add.
    if escaped {
        repaired.push('\\');
    }
    if in_string {
        repaired.push('"');
    }
    while let Some(closer) = stack.pop() {
        repaired.push(closer);
    }

    serde_json::from_str::<Value>(&repaired).ok()
}

/// Everything before the last comma that is not inside a string — where the
/// half-arrived field begins.
fn truncate_at_last_comma(fragment: &str) -> Option<String> {
    let mut in_string = false;
    let mut escaped = false;
    let mut last_comma: Option<usize> = None;

    for (position, ch) in fragment.chars().enumerate() {
        if in_string {
            if escaped {
                escaped = false;
            } else if ch == '\\' {
                escaped = true;
            } else if ch == '"' {
                in_string = false;
            }
            continue;
        }
        match ch {
            '"' => in_string = true,
            ',' => last_comma = Some(position),
            _ => {}
        }
    }

    last_comma.map(|position| fragment.chars().take(position).collect())
}

/// Say what a tool is about to do, in the words a person would use.
///
/// Returns None only when there is nothing useful to say yet, which leaves
/// whatever was announced at `content_block_start` standing.
fn describe_tool_call(tool_name: &str, input: &Value) -> Option<String> {
    let label = friendly_tool_name(tool_name);

    if label == "computer" {
        return describe_computer_action(input);
    }

    fn field<'a>(input: &'a Value, key: &str) -> Option<&'a str> {
        input.get(key).and_then(|v| v.as_str())
    }

    let described = match label {
        "Bash" => field(input, "command")
            .map(|command| format!("Running {}", truncate_for_display(command, 60))),
        "Read" => field(input, "file_path").map(|path| format!("Reading {}", file_label(path))),
        "Write" => field(input, "file_path").map(|path| format!("Writing {}", file_label(path))),
        "Edit" | "NotebookEdit" => {
            field(input, "file_path").map(|path| format!("Editing {}", file_label(path)))
        }
        "Glob" | "Grep" => field(input, "pattern")
            .map(|pattern| format!("Searching for {}", truncate_for_display(pattern, 40))),
        "WebFetch" => {
            field(input, "url").map(|url| format!("Fetching {}", truncate_for_display(url, 60)))
        }
        "WebSearch" => field(input, "query")
            .map(|query| format!("Searching the web for {}", truncate_for_display(query, 40))),
        "Task" => field(input, "description").map(|what| truncate_for_display(what, 60)),
        _ => None,
    };

    // No recognised arguments is not nothing: the tool's own name is still
    // more than the blank screen this replaces.
    Some(described.unwrap_or_else(|| format!("Using {}", label)))
}

/// Say what Juno's own desktop tool is about to do. Action names match the
/// vocabulary in `agent::tools::anthropic_computer_use`.
fn describe_computer_action(input: &Value) -> Option<String> {
    let action = input.get("action").and_then(|v| v.as_str())?;

    let point = |key: &str| -> Option<String> {
        let coordinate = input.get(key)?.as_array()?;
        let x = coordinate.first()?.as_f64()?;
        let y = coordinate.get(1)?.as_f64()?;
        Some(format!("({}, {})", x.round() as i64, y.round() as i64))
    };
    let at = point("coordinate");
    // "Clicking (640, 60)" once the point has arrived, plain "Clicking"
    // until then.
    let with_point = |verb: &str| match &at {
        Some(place) => format!("{} {}", verb, place),
        None => verb.to_string(),
    };
    let text = input.get("text").and_then(|v| v.as_str());

    let described = match action {
        "screenshot" => "Taking a screenshot".to_string(),
        "cursor_position" => "Finding the cursor".to_string(),
        "mouse_move" => with_point("Moving to"),
        "left_click" => with_point("Clicking"),
        "right_click" => with_point("Right-clicking"),
        "middle_click" => with_point("Middle-clicking"),
        "double_click" => with_point("Double-clicking"),
        "triple_click" => with_point("Triple-clicking"),
        "left_mouse_down" => with_point("Pressing the mouse at"),
        "left_mouse_up" => with_point("Releasing the mouse at"),
        "left_click_drag" => {
            let from = point("start_coordinate").or_else(|| point("coordinate"));
            match (from, point("end_coordinate")) {
                (Some(start), Some(end)) => format!("Dragging {} to {}", start, end),
                _ => with_point("Dragging to"),
            }
        }
        "scroll" => {
            let direction = input
                .get("scroll_direction")
                .and_then(|v| v.as_str())
                .unwrap_or("down");
            format!("Scrolling {}", direction)
        }
        "type" => match text {
            Some(typed) => format!("Typing \"{}\"", truncate_for_display(typed, 30)),
            None => "Typing".to_string(),
        },
        "key" => match text {
            Some(key) => format!("Pressing {}", truncate_for_display(key, 30)),
            None => "Pressing a key".to_string(),
        },
        "hold_key" => match text {
            Some(key) => format!("Holding {}", truncate_for_display(key, 30)),
            None => "Holding a key".to_string(),
        },
        "wait" => "Waiting".to_string(),
        "zoom" => "Zooming in".to_string(),
        other => format!("Running {}", other),
    };

    Some(described)
}

/// Extract text content from a Claude CLI assistant message JSON object.
/// Handles both `content` array format and direct `content` string.
pub(super) fn extract_text_from_message(message: &Value) -> String {
    // Try content array format: {"content": [{"type": "text", "text": "..."}]}
    if let Some(content_array) = message.get("content").and_then(|v| v.as_array()) {
        let mut text = String::new();
        for block in content_array {
            if block.get("type").and_then(|v| v.as_str()) == Some("text") {
                if let Some(t) = block.get("text").and_then(|v| v.as_str()) {
                    text.push_str(t);
                }
            }
        }
        if !text.is_empty() {
            return text;
        }
    }

    // Try direct content string: {"content": "..."}
    if let Some(content_str) = message.get("content").and_then(|v| v.as_str()) {
        return content_str.to_string();
    }

    String::new()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A `CliStatus` in a given state, without running anything.
    fn status(installed: bool, sign_in: SignIn) -> CliStatus {
        CliStatus {
            installed,
            sign_in,
            email: None,
        }
    }

    #[test]
    fn only_a_confirmed_login_is_proof() {
        assert!(status(true, SignIn::SignedIn).is_signed_in());
        assert!(!status(true, SignIn::SignedOut).is_signed_in());
        // The case the distinction exists for: Juno must not move somebody
        // onto this provider on the strength of an answer it could not read.
        assert!(!status(true, SignIn::Unknown).is_signed_in());
        assert!(!status(false, SignIn::Unknown).is_signed_in());
    }

    #[test]
    fn only_a_confirmed_logout_stops_the_ui_offering_it() {
        // `check_auth_status` lets an unreadable answer through and runs the
        // query, so the provider list must not grey the CLI out on one.
        assert!(status(true, SignIn::Unknown).could_run());
        assert!(status(true, SignIn::SignedIn).could_run());
        assert!(!status(true, SignIn::SignedOut).could_run());
        assert!(!status(false, SignIn::SignedIn).could_run());
    }

    #[test]
    fn an_unasked_question_is_not_a_no() {
        // The provider listing is synchronous and reads this before anything
        // has probed. Defaulting to SignedOut would tell every user with a
        // working CLI to go and sign in.
        assert_eq!(SignIn::default(), SignIn::Unknown);
        assert!(status(true, SignIn::default()).could_run());
    }

    #[test]
    fn the_sign_in_cache_round_trips_every_state() {
        // It crosses an AtomicU8, so a bad encoding would silently read back
        // as Unknown and quietly disable the whole distinction.
        for state in [SignIn::Unknown, SignIn::SignedIn, SignIn::SignedOut] {
            remember_sign_in(state);
            assert_eq!(last_known_sign_in(), state);
        }
    }

    #[test]
    fn test_detect_claude_cli() {
        // This test is environment-dependent — just verify it doesn't panic
        let result = detect_claude_cli();
        // On CI without claude installed, this will be Err — that's fine
        match result {
            Ok(path) => assert!(path.exists()),
            Err(e) => assert!(e.to_string().contains("not found")),
        }
    }

    #[test]
    fn test_extract_text_from_content_array() {
        let message = serde_json::json!({
            "content": [
                {"type": "text", "text": "Hello "},
                {"type": "text", "text": "world!"}
            ]
        });
        assert_eq!(extract_text_from_message(&message), "Hello world!");
    }

    #[test]
    fn test_extract_text_from_content_string() {
        let message = serde_json::json!({
            "content": "Hello world!"
        });
        assert_eq!(extract_text_from_message(&message), "Hello world!");
    }

    #[test]
    fn test_extract_text_empty() {
        let message = serde_json::json!({});
        assert_eq!(extract_text_from_message(&message), "");
    }

    #[test]
    fn test_extract_text_with_multibyte() {
        let message = serde_json::json!({
            "content": [
                {"type": "text", "text": "Hello 🌍 世界!"}
            ]
        });
        assert_eq!(extract_text_from_message(&message), "Hello 🌍 世界!");
    }

    #[test]
    fn test_extract_query_from_messages() {
        let messages = vec![
            Message {
                role: Role::User,
                content: "first question".to_string(),
                tool_calls: None,
                tool_call_id: None,
                name: None,
                images: None,
            },
            Message {
                role: Role::Assistant,
                content: "response".to_string(),
                tool_calls: None,
                tool_call_id: None,
                name: None,
                images: None,
            },
            Message {
                role: Role::User,
                content: "second question".to_string(),
                tool_calls: None,
                tool_call_id: None,
                name: None,
                images: None,
            },
        ];
        assert_eq!(ClaudeCliBrain::extract_query(&messages), "second question");
    }

    #[test]
    fn test_extract_query_no_user_message() {
        let messages = vec![Message {
            role: Role::Assistant,
            content: "response".to_string(),
            tool_calls: None,
            tool_call_id: None,
            name: None,
            images: None,
        }];
        assert_eq!(ClaudeCliBrain::extract_query(&messages), "Hello");
    }

    #[test]
    fn test_build_args_basic() {
        let brain = ClaudeCliBrain {
            binary_path: PathBuf::from("/usr/bin/claude"),
            model: "sonnet".to_string(),
            system_prompt: None,
            effort: "high".to_string(),
            observed_session: std::sync::Mutex::new(None),
        };
        let args = brain.build_args("test query", None, None);
        assert!(args.contains(&"-p".to_string()));
        assert!(args.contains(&"stream-json".to_string()));
        assert!(args.contains(&"sonnet".to_string()));
        assert!(args.contains(&"test query".to_string()));
        // No --system-prompt when None
        assert!(!args.contains(&"--system-prompt".to_string()));
        // No MCP flags when Juno's tool server isn't wired
        assert!(!args.contains(&"--mcp-config".to_string()));
        assert!(!args.contains(&"--append-system-prompt".to_string()));
        // User-level MCP servers stay disabled either way
        assert!(args.contains(&"--strict-mcp-config".to_string()));
    }

    #[test]
    fn test_build_args_with_system_prompt() {
        let brain = ClaudeCliBrain {
            binary_path: PathBuf::from("/usr/bin/claude"),
            model: "opus".to_string(),
            system_prompt: Some("You are helpful.".to_string()),
            effort: "high".to_string(),
            observed_session: std::sync::Mutex::new(None),
        };
        let args = brain.build_args("test query", None, None);
        assert!(args.contains(&"--system-prompt".to_string()));
        assert!(args.contains(&"You are helpful.".to_string()));
    }

    /// Regression test for LAC-3696: when Juno's tool server is up, the CLI
    /// must be given our MCP config (the `computer` tool) while
    /// --strict-mcp-config still blocks user-level servers, and the model must
    /// be steered toward it via --append-system-prompt.
    #[test]
    fn resuming_passes_the_session_to_the_cli() {
        // Without this flag every message was a new CLI session, so Juno
        // answered follow-ups as if the chat above her had never happened.
        let brain = ClaudeCliBrain {
            binary_path: PathBuf::from("/usr/bin/claude"),
            model: "sonnet".to_string(),
            system_prompt: None,
            effort: "high".to_string(),
            observed_session: std::sync::Mutex::new(None),
        };
        let args = brain.build_args("and what about the other one?", Some("abc-123"), None);
        let idx = args
            .iter()
            .position(|a| a == "--resume")
            .expect("--resume is passed when a session is known");
        assert_eq!(args[idx + 1], "abc-123");
    }

    #[test]
    fn a_first_message_starts_a_fresh_session() {
        let brain = ClaudeCliBrain {
            binary_path: PathBuf::from("/usr/bin/claude"),
            model: "sonnet".to_string(),
            system_prompt: None,
            effort: "high".to_string(),
            observed_session: std::sync::Mutex::new(None),
        };
        let args = brain.build_args("hello", None, None);
        assert!(!args.iter().any(|a| a == "--resume"));
    }

    #[test]
    fn a_conversation_remembers_its_session() {
        let convo = "conversation-under-test";
        assert_eq!(resume_id_for(convo), None, "nothing known yet");
        remember_session(convo, "session-1");
        assert_eq!(resume_id_for(convo), Some("session-1".to_string()));
        // A new chat is a new id, so it gets no session and starts clean.
        assert_eq!(resume_id_for("a-different-conversation"), None);
        forget_session(convo);
        assert_eq!(resume_id_for(convo), None);
    }

    #[test]
    fn the_cli_is_pointed_at_junos_own_tool_server() {
        let brain = ClaudeCliBrain {
            binary_path: PathBuf::from("/usr/bin/claude"),
            model: "sonnet".to_string(),
            system_prompt: None,
            effort: "high".to_string(),
            observed_session: std::sync::Mutex::new(None),
        };
        let config = PathBuf::from("/tmp/juno-mcp-test.json");
        let args = brain.build_args("move the mouse", None, Some(&config));

        let idx = args
            .iter()
            .position(|a| a == "--mcp-config")
            .expect("--mcp-config flag present");
        assert_eq!(args[idx + 1], "/tmp/juno-mcp-test.json");
        assert!(
            args.iter().any(|a| a == "--strict-mcp-config"),
            "the person's own MCP servers must stay out of Juno's agent"
        );
        assert!(
            args.iter().any(|a| a.contains("juno")),
            "the guidance has to name the server the tool is on"
        );
    }

    #[test]
    fn without_a_desktop_the_cli_runs_toolless() {
        // Headless and test runs have no app handle, so there is no tool
        // server and nothing to point at. The CLI still answers.
        let brain = ClaudeCliBrain {
            binary_path: PathBuf::from("/usr/bin/claude"),
            model: "sonnet".to_string(),
            system_prompt: None,
            effort: "high".to_string(),
            observed_session: std::sync::Mutex::new(None),
        };
        let args = brain.build_args("hello", None, None);
        assert!(!args.iter().any(|a| a == "--mcp-config"));
    }

    /// Write an executable shell script that stands in for the `claude` binary.
    /// Answers the `auth status` pre-check with a logged-in response, then runs
    /// `body` for the actual query invocation.
    fn write_fake_cli_script(body: &str) -> PathBuf {
        use std::io::Write;
        use std::os::unix::fs::PermissionsExt;
        let path = std::env::temp_dir().join(format!(
            "juno-fake-claude-{}-{}.sh",
            std::process::id(),
            uuid::Uuid::new_v4()
        ));
        let mut file = std::fs::File::create(&path).expect("create fake CLI script");
        writeln!(
            file,
            "#!/bin/sh\nif [ \"$1\" = \"auth\" ]; then echo '{{\"loggedIn\": true}}'; exit 0; fi\n{}",
            body
        )
        .expect("write fake CLI script");
        let mut perms = file.metadata().expect("stat fake CLI script").permissions();
        perms.set_mode(0o755);
        std::fs::set_permissions(&path, perms).expect("chmod fake CLI script");
        path
    }

    fn test_brain(binary_path: PathBuf) -> ClaudeCliBrain {
        ClaudeCliBrain {
            binary_path,
            model: "sonnet".to_string(),
            system_prompt: None,
            effort: "high".to_string(),
            observed_session: std::sync::Mutex::new(None),
        }
    }

    /// Regression test for LAC-3697: a session-scoped cancel arrives via the
    /// `cancel_rx` parameter (the merged session+global receiver), NOT the
    /// global AppState channel. `run_streaming` must observe it, kill the
    /// subprocess promptly, and surface `AgentError::Terminated` so the
    /// response is never rendered or spoken.
    #[tokio::test]
    async fn test_run_streaming_session_cancel_kills_subprocess() {
        let script =
            write_fake_cli_script("sleep 30\necho '{\"type\":\"result\",\"result\":\"too late\"}'");
        let brain = test_brain(script.clone());

        let (cancel_tx, cancel_rx) = tokio::sync::watch::channel(false);
        let start = std::time::Instant::now();

        // Simulate the focused-session escape landing shortly after spawn.
        let canceller = tokio::spawn(async move {
            tokio::time::sleep(Duration::from_millis(300)).await;
            let _ = cancel_tx.send(true);
        });

        let result = brain
            .run_streaming("test query", None, None, Some(cancel_rx))
            .await;
        let _ = canceller.await;

        assert!(
            matches!(result, Err(AgentError::Terminated)),
            "expected Terminated after session cancel, got {:?}",
            result
        );
        assert!(
            start.elapsed() < Duration::from_secs(5),
            "cancel must kill the subprocess promptly (took {:?}, script sleeps 30s)",
            start.elapsed()
        );
        let _ = std::fs::remove_file(&script);
    }

    /// A cancel that is already signalled before the query starts must
    /// short-circuit without waiting on the subprocess.
    #[tokio::test]
    async fn test_run_streaming_pre_cancelled_returns_terminated() {
        let script = write_fake_cli_script("sleep 30");
        let brain = test_brain(script.clone());

        let (_cancel_tx, cancel_rx) = tokio::sync::watch::channel(true);
        let start = std::time::Instant::now();

        let result = brain
            .run_streaming("test query", None, None, Some(cancel_rx))
            .await;

        assert!(
            matches!(result, Err(AgentError::Terminated)),
            "expected Terminated for pre-cancelled run, got {:?}",
            result
        );
        assert!(
            start.elapsed() < Duration::from_secs(5),
            "pre-cancelled run must return promptly (took {:?})",
            start.elapsed()
        );
        let _ = std::fs::remove_file(&script);
    }

    /// Sanity check: an uncancelled run with a live session receiver streams
    /// to completion and returns the CLI's result text.
    #[tokio::test]
    async fn test_run_streaming_completes_when_not_cancelled() {
        let script =
            write_fake_cli_script("echo '{\"type\":\"result\",\"result\":\"hello from cli\"}'");
        let brain = test_brain(script.clone());

        let (_cancel_tx, cancel_rx) = tokio::sync::watch::channel(false);
        let result = brain
            .run_streaming("test query", None, None, Some(cancel_rx))
            .await;

        assert_eq!(result.expect("run should succeed"), "hello from cli");
        let _ = std::fs::remove_file(&script);
    }

    /// The flag that makes any of this possible. Without it the CLI reports
    /// only finished messages and the user watches a blank pane.
    #[test]
    fn partial_messages_are_requested() {
        let brain = test_brain(PathBuf::from("/usr/bin/claude"));
        let args = brain.build_args("what is on screen", None, None);
        assert!(args.contains(&"--include-partial-messages".to_string()));
        // Only valid alongside --print and stream-json, both of which we pass.
        assert!(args.contains(&"-p".to_string()));
        assert!(args.contains(&"stream-json".to_string()));
    }

    /// The bubble opens once and closes once, however many times it is asked.
    ///
    /// The idempotence is what lets every exit path close explicitly while
    /// `Drop` still backstops the ones that cannot (the `?` returns). Drop
    /// the guard and a path that closes explicitly would emit a second,
    /// empty `stream_end` and wipe the answer off the screen.
    #[test]
    fn a_stream_surface_closes_exactly_once() {
        // No AppHandle in tests, so nothing is emitted; the state machine
        // that decides whether to emit is the part under test.
        let mut surface = StreamSurface::open(&None, "msg-1".to_string());
        assert!(surface.open, "opening arms the guard");

        surface.close("the answer".to_string());
        assert!(!surface.open, "closing disarms it");

        // A second close, and the one Drop would attempt, must both no-op.
        surface.close(String::new());
        assert!(!surface.open);
    }

    #[test]
    fn an_old_cli_is_recognised_by_the_flag_it_names() {
        // Wording drifts between versions; the flag name does not.
        assert!(rejects_partial_messages(
            "error: unknown option '--include-partial-messages'"
        ));
        assert!(rejects_partial_messages(
            "unrecognized option: --include-partial-messages\nUsage: claude [options]"
        ));
        // Real failures that have nothing to do with the flag must not
        // trigger a pointless respawn.
        assert!(!rejects_partial_messages("Error: network unreachable"));
        assert!(!rejects_partial_messages(""));
    }

    #[test]
    fn stripping_the_flag_leaves_the_rest_of_the_command_intact() {
        let brain = test_brain(PathBuf::from("/usr/bin/claude"));
        let mut args = brain.build_args("hello", Some("abc-123"), None);
        let before = args.len();
        strip_partial_messages(&mut args);

        assert!(!args.contains(&PARTIAL_MESSAGES_FLAG.to_string()));
        assert_eq!(before - 1, args.len(), "only the one standalone flag goes");
        // Everything the run depends on survives.
        assert!(args.contains(&"stream-json".to_string()));
        assert!(args.contains(&"--strict-mcp-config".to_string()));
        assert!(args.contains(&"abc-123".to_string()));
        assert_eq!(args.last().map(String::as_str), Some("hello"));
    }

    /// The whole point of the fallback: a `claude` too old for
    /// --include-partial-messages must still answer, not fail every query.
    #[tokio::test]
    async fn an_old_cli_degrades_instead_of_failing() {
        let log = std::env::temp_dir().join(format!("juno-old-cli-{}.log", uuid::Uuid::new_v4()));
        let script = write_fake_cli_script(&format!(
            r#"echo run >> {log}
for arg in "$@"; do
if [ "$arg" = "--include-partial-messages" ]; then
echo "error: unknown option '--include-partial-messages'" >&2
exit 1
fi
done
echo '{{"type":"result","result":"answered the old way"}}'"#,
            log = log.display()
        ));

        // A previous test may have latched this; the retry is what we are
        // measuring, so start from the fast path.
        PARTIAL_MESSAGES_UNSUPPORTED.store(false, Ordering::Relaxed);

        let brain = test_brain(script.clone());
        let result = brain.run_streaming("test query", None, None, None).await;

        assert_eq!(
            result.expect("an old CLI must still answer"),
            "answered the old way"
        );

        let runs = std::fs::read_to_string(&log).unwrap_or_default();
        assert_eq!(
            runs.lines().count(),
            2,
            "exactly one retry: the rejected run, then the stripped one"
        );
        assert!(
            PARTIAL_MESSAGES_UNSUPPORTED.load(Ordering::Relaxed),
            "the rest of the session must skip the flag, not re-discover it"
        );

        // Leave the static as the rest of the suite expects to find it.
        PARTIAL_MESSAGES_UNSUPPORTED.store(false, Ordering::Relaxed);
        let _ = std::fs::remove_file(&script);
        let _ = std::fs::remove_file(&log);
    }

    #[test]
    fn effort_is_passed_and_defaults_sanely() {
        let brain = test_brain(PathBuf::from("/usr/bin/claude"));
        let args = brain.build_args("hello", None, None);
        let idx = args
            .iter()
            .position(|a| a == "--effort")
            .expect("--effort is always passed");
        assert_eq!(args[idx + 1], "high");
    }

    #[test]
    fn an_unknown_effort_level_is_dropped_not_forwarded() {
        // A hand-edited or stale store must not make every spawn fail on an
        // argument the CLI rejects.
        assert_eq!(resolve_effort(Some("xhigh")), "xhigh");
        assert_eq!(resolve_effort(Some("banana")), "high");
        assert_eq!(resolve_effort(None), "high");
    }

    #[test]
    fn mcp_plumbing_is_stripped_from_tool_names() {
        assert_eq!(friendly_tool_name("mcp__juno__computer"), "computer");
        assert_eq!(friendly_tool_name("Bash"), "Bash");
    }

    #[test]
    fn a_half_arrived_tool_call_still_reads() {
        // The exact shape the CLI streams: arguments a few characters at a
        // time. Each prefix must say as much as it can.
        let complete = r#"{"action":"left_click","coordinate":[640,60]}"#;
        let parsed = parse_partial_json(complete).expect("complete JSON parses");
        assert_eq!(
            describe_tool_call("mcp__juno__computer", &parsed).as_deref(),
            Some("Clicking (640, 60)")
        );

        // Mid-coordinate: the point is not knowable yet, the action is.
        let mid = r#"{"action":"left_click","coordinate":[64"#;
        let parsed = parse_partial_json(mid).expect("partial JSON is repaired");
        assert_eq!(
            describe_tool_call("mcp__juno__computer", &parsed).as_deref(),
            Some("Clicking")
        );

        // Mid-string: the open quote is closed before parsing.
        let mid_string = r#"{"action":"scroll","scroll_direction":"do"#;
        let parsed = parse_partial_json(mid_string).expect("open string is closed");
        assert_eq!(
            describe_tool_call("mcp__juno__computer", &parsed).as_deref(),
            Some("Scrolling do")
        );

        // Too early to mean anything: a key with no value cannot be closed
        // into valid JSON, so we say nothing rather than guess.
        assert!(parse_partial_json("{\"ac").is_none());
        assert!(parse_partial_json("").is_none());
    }

    #[test]
    fn partial_json_never_panics_on_multibyte_text() {
        // A truncation that lands inside a multi-byte character would panic
        // if any of this byte-sliced.
        let fragment = r#"{"action":"type","text":"héllo 🌍 世界"#;
        let parsed = parse_partial_json(fragment).expect("multibyte fragment is repaired");
        let described = describe_tool_call("mcp__juno__computer", &parsed)
            .expect("a type action always describes");
        assert!(described.starts_with("Typing"), "got {}", described);
    }

    #[test]
    fn the_cli_tools_are_described_too() {
        let bash = serde_json::json!({"command": "ls -la /tmp"});
        assert_eq!(
            describe_tool_call("Bash", &bash).as_deref(),
            Some("Running ls -la /tmp")
        );

        let read = serde_json::json!({"file_path": "/Users/x/repo/src/main.rs"});
        assert_eq!(
            describe_tool_call("Read", &read).as_deref(),
            Some("Reading main.rs")
        );

        // An unrecognised tool still beats a blank screen.
        assert_eq!(
            describe_tool_call("Mystery", &serde_json::json!({})).as_deref(),
            Some("Using Mystery")
        );
    }

    #[test]
    fn long_text_is_truncated_by_characters_not_bytes() {
        let long = "🌍".repeat(50);
        let shortened = truncate_for_display(&long, 10);
        assert_eq!(shortened.chars().count(), 10);
        assert!(shortened.ends_with("..."));
    }

    #[test]
    fn test_char_count_delta_with_emoji() {
        // Simulate the delta computation logic
        let text1 = "Hello 🌍".to_string();
        let text2 = "Hello 🌍 world!".to_string();

        let prev_char_count = text1.chars().count(); // 7
        let curr_char_count = text2.chars().count(); // 15

        assert!(curr_char_count > prev_char_count);
        let delta: String = text2.chars().skip(prev_char_count).collect();
        assert_eq!(delta, " world!");
    }
}

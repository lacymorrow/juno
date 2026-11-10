//! Copilot CLI provider: uses the `copilot` binary (GitHub Copilot CLI) as a
//! subprocess-based AI provider, so Juno runs on the Copilot plan a person
//! already pays for instead of asking for an API key on top (LAC-7611).
//!
//! This is the *native* path from the spike (`docs/plans/cli-providers-spike.md`,
//! measurement 6): `copilot -p <query> --output-format json`, one process per
//! turn, JSONL on stdout, no new dependency. Follow-up turns continue the same
//! Copilot session with `--resume=<id>`, where the id comes from the
//! `session.start` event of the first turn. ACP comes back with the second row.
//!
//! Juno's computer tool is mounted with `--additional-mcp-config @<file>`, a
//! pid-scoped 0600 file, so the loopback bearer token never reaches argv.
//! `-p` mode refuses every tool not on `--allow-tool`, and only `juno` is on
//! it: Copilot's own shell and file tools stay off, the desktop goes through
//! Juno. Copilot's built-in GitHub MCP server is off (`--disable-builtin-mcps`).
//!
//! Bring-your-own-key (Azure AI Foundry and friends) is Copilot's own config.
//! Juno spawns `copilot`; `copilot` reads `~/.copilot/providers.json`.
//!
//! NOTE: macOS-only, matching Juno's platform target.

use async_trait::async_trait;
use serde_json::{json, Value};
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::sync::atomic::{AtomicU8, Ordering};
use std::sync::Mutex;
use tokio::io::{AsyncBufReadExt, BufReader};
use tracing::{debug, error, info, warn};

use super::claude_cli::{ClaudeCliBrain, CLI_IDLE_TIMEOUT};
use super::juno_mcp;
use crate::agent::core::{AgentAction, AgentError, Message, Role, ToolDefinition};
use crate::agent::traits::AgentBrain;
use crate::settings::ProviderConfig as CentralizedProviderConfig;

/// Copilot model ids, passed as `--model`. The catalog lives in
/// `Provider::model_definitions`; this is only the fallback when a config
/// entry has no model at all.
pub mod model_aliases {
    /// Copilot picks within the person's plan.
    pub const DEFAULT: &str = "auto";
}

/// The name Juno's server goes by in Copilot's MCP config, and so in
/// `--allow-tool`. Copilot surfaces its tool as `juno-computer`.
const MCP_SERVER_NAME: &str = "juno";

/// Shown when the model cannot see images. A bring-your-own-key deployment is
/// text-only unless `COPILOT_PROVIDER_VISION=true`, and Juno's computer tool
/// returns screenshots.
pub const NO_VISION_MESSAGE: &str =
    "This model can't see the screen. Pick another model, or run `copilot` and `/model`.";

/// Shown when Copilot reports it has no login.
pub const SIGNED_OUT_MESSAGE: &str = "Copilot signed out. Run `copilot login`, then try again.";

/// What Juno knows about the Copilot login.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum SignIn {
    /// Nobody has asked yet, or the evidence was unreadable. Listed as runnable:
    /// the first turn finds out.
    #[default]
    Unknown,
    /// Proof of a login: a token in the environment, a user recorded in
    /// `~/.copilot/config.json`, or a `gh` login Copilot falls back to.
    SignedIn,
    /// A turn failed asking for a login.
    SignedOut,
}

static LAST_SIGN_IN: AtomicU8 = AtomicU8::new(0);

fn remember_sign_in(state: SignIn) {
    let encoded = match state {
        SignIn::Unknown => 0,
        SignIn::SignedIn => 1,
        SignIn::SignedOut => 2,
    };
    LAST_SIGN_IN.store(encoded, Ordering::Relaxed);
}

/// The most recent answer, without touching the disk, so a synchronous
/// Settings render can read it.
pub fn last_known_sign_in() -> SignIn {
    match LAST_SIGN_IN.load(Ordering::Relaxed) {
        1 => SignIn::SignedIn,
        2 => SignIn::SignedOut,
        _ => SignIn::Unknown,
    }
}

/// Everything Juno knows about the local Copilot CLI, gathered in one pass.
#[derive(Debug, Clone, Copy, Default)]
pub struct CliStatus {
    /// The `copilot` binary exists.
    pub installed: bool,
    pub sign_in: SignIn,
}

impl CliStatus {
    /// Proof of a login, which is what the default-provider rule treats as a
    /// credential this provider can run on.
    pub fn is_signed_in(&self) -> bool {
        self.installed && self.sign_in == SignIn::SignedIn
    }
}

/// Find the `copilot` binary: common install locations first (cheap
/// `exists()` calls), then the person's login-shell PATH, then ours. Juno
/// launched from the Dock does not have the shell's PATH.
pub fn detect_copilot_cli() -> Result<PathBuf, AgentError> {
    let home = dirs::home_dir();
    let candidates = [
        home.as_ref().map(|h| h.join(".local/bin/copilot")),
        Some(PathBuf::from("/opt/homebrew/bin/copilot")),
        Some(PathBuf::from("/usr/local/bin/copilot")),
        home.as_ref().map(|h| h.join(".bun/bin/copilot")),
        home.as_ref().map(|h| h.join(".npm-global/bin/copilot")),
    ];
    for candidate in candidates.into_iter().flatten() {
        if candidate.exists() {
            debug!("Found Copilot CLI at: {}", candidate.display());
            return Ok(candidate);
        }
    }
    if let Some(found) = crate::user_env::find_on_user_path("copilot") {
        debug!("Found Copilot CLI via user PATH: {}", found.display());
        return Ok(found);
    }
    if let Ok(path_var) = std::env::var("PATH") {
        for dir in std::env::split_paths(&path_var) {
            let candidate = dir.join("copilot");
            if candidate.is_file() {
                return Ok(candidate);
            }
        }
    }
    Err(AgentError::ConfigurationError(
        "Copilot CLI (copilot) not found. Install it from https://github.com/github/copilot-cli"
            .to_string(),
    ))
}

/// Quick check: is the Copilot CLI binary available? (No auth check.)
pub fn is_copilot_cli_available() -> bool {
    detect_copilot_cli().is_ok()
}

/// Does `~/.copilot/config.json` record a logged-in user?
///
/// The file is JSON with `//` comment lines. Key spelling has varied between
/// releases, so keys are compared ignoring case and underscores.
fn config_records_a_login(bytes: &[u8]) -> bool {
    let text = String::from_utf8_lossy(bytes);
    let stripped: String = text
        .lines()
        .filter(|l| !l.trim_start().starts_with("//"))
        .collect::<Vec<_>>()
        .join("\n");
    let Ok(Value::Object(map)) = serde_json::from_str::<Value>(&stripped) else {
        return false;
    };
    map.iter().any(|(key, value)| {
        let key = key.to_ascii_lowercase().replace('_', "");
        let is_login_key = key == "lastloggedinuser" || key == "loggedinusers";
        is_login_key
            && match value {
                Value::Null => false,
                Value::Array(a) => !a.is_empty(),
                Value::String(s) => !s.is_empty(),
                Value::Object(o) => !o.is_empty(),
                _ => true,
            }
    })
}

/// Read what the machine says about the Copilot login. Never errors: the worst
/// answer is `Unknown`, which leaves the Settings listing alone.
pub async fn cli_status() -> CliStatus {
    if detect_copilot_cli().is_err() {
        remember_sign_in(SignIn::Unknown);
        return CliStatus::default();
    }

    let token_in_env = ["COPILOT_GITHUB_TOKEN", "GH_TOKEN", "GITHUB_TOKEN"]
        .iter()
        .any(|name| std::env::var(name).is_ok_and(|v| !v.is_empty()));

    let in_config = match dirs::home_dir() {
        Some(home) => tokio::fs::read(home.join(".copilot/config.json"))
            .await
            .map(|bytes| config_records_a_login(&bytes))
            .unwrap_or(false),
        None => false,
    };

    // Cheapest evidence first: `gh` is only asked when nothing else proved it.
    let proven = token_in_env || in_config || gh_has_a_login().await;
    let sign_in = if proven {
        SignIn::SignedIn
    } else if last_known_sign_in() == SignIn::SignedOut {
        // A turn already failed asking for a login, and nothing since has shown one.
        SignIn::SignedOut
    } else {
        SignIn::Unknown
    };
    remember_sign_in(sign_in);
    CliStatus {
        installed: true,
        sign_in,
    }
}

/// Copilot falls back to the `gh` CLI's login. `gh auth token` exits 0 and
/// prints a token only when there is one; the token is never read here, only
/// its existence.
async fn gh_has_a_login() -> bool {
    let Some(gh) = crate::user_env::find_on_user_path("gh") else {
        return false;
    };
    let run = tokio::process::Command::new(gh)
        .args(["auth", "token"])
        .stdin(Stdio::null())
        .stderr(Stdio::null())
        .kill_on_drop(true)
        .output();
    match tokio::time::timeout(std::time::Duration::from_secs(3), run).await {
        Ok(Ok(out)) => out.status.success() && !out.stdout.iter().all(u8::is_ascii_whitespace),
        _ => false,
    }
}

/// Pre-query check: refuse early when there is a *known* sign-out. An
/// `Unknown` answer runs the turn, which finds out for itself.
pub async fn check_cli_auth_status() -> Result<(), AgentError> {
    detect_copilot_cli()?;
    match cli_status().await.sign_in {
        SignIn::SignedOut => Err(AgentError::ConfigurationError(
            SIGNED_OUT_MESSAGE.to_string(),
        )),
        _ => Ok(()),
    }
}

/// Juno conversation id to Copilot session id, so a second message continues
/// the same Copilot session. Bounded: a stale id only costs a cold turn.
static SESSIONS: Mutex<Option<HashMap<String, String>>> = Mutex::new(None);
const MAX_REMEMBERED_SESSIONS: usize = 64;

fn remember_session(conversation_id: &str, session_id: &str) {
    if let Ok(mut guard) = SESSIONS.lock() {
        let map = guard.get_or_insert_with(HashMap::new);
        if map.len() >= MAX_REMEMBERED_SESSIONS && !map.contains_key(conversation_id) {
            map.clear();
        }
        map.insert(conversation_id.to_string(), session_id.to_string());
    }
}

fn session_for(conversation_id: &str) -> Option<String> {
    SESSIONS
        .lock()
        .ok()
        .and_then(|guard| guard.as_ref().and_then(|m| m.get(conversation_id).cloned()))
}

/// The `--additional-mcp-config` contents pointing Copilot at Juno's server.
/// Copilot requires `tools` on a remote server entry.
fn mcp_config(endpoint: &juno_mcp::Endpoint) -> Value {
    json!({
        "mcpServers": {
            MCP_SERVER_NAME: {
                "type": "http",
                "url": endpoint.url,
                "headers": { "Authorization": format!("Bearer {}", endpoint.token) },
                "tools": ["*"]
            }
        }
    })
}

/// Write the MCP config to a pid-scoped 0600 file. It carries the bearer
/// token, which is why Copilot reads it from a file (`@path`) and not argv.
fn write_mcp_config(endpoint: &juno_mcp::Endpoint) -> Result<PathBuf, AgentError> {
    write_mcp_config_in(&std::env::temp_dir(), endpoint)
}

fn write_mcp_config_in(dir: &Path, endpoint: &juno_mcp::Endpoint) -> Result<PathBuf, AgentError> {
    let path = dir.join(format!("juno-copilot-mcp-{}.json", std::process::id()));
    let bytes = serde_json::to_vec(&mcp_config(endpoint)).map_err(|e| {
        AgentError::ConfigurationError(format!("Failed to serialize MCP config: {e}"))
    })?;
    super::claude_cli::write_private_file(&path, &bytes)?;
    Ok(path)
}

/// Names of the MCP servers in `~/.copilot/mcp-config.json`, for turning the
/// "Load account MCP connectors" setting off: Copilot has no blanket switch,
/// only `--disable-mcp-server <name>`.
fn user_mcp_server_names(bytes: &[u8]) -> Vec<String> {
    serde_json::from_slice::<Value>(bytes)
        .ok()
        .and_then(|v| v.get("mcpServers").and_then(Value::as_object).cloned())
        .map(|servers| servers.keys().cloned().collect())
        .unwrap_or_default()
}

/// Copilot CLI brain. One subprocess per turn, JSONL back through Tauri events
/// in the same shape the other CLI providers use.
pub struct CopilotCliBrain {
    binary_path: PathBuf,
    model: String,
    system_prompt: Option<String>,
    /// `providers[].load_account_mcp`: whether the person's own Copilot MCP
    /// servers load alongside Juno's.
    load_account_mcp: bool,
}

impl CopilotCliBrain {
    /// Build from centralized provider config. Checks the binary exists; auth
    /// is checked lazily at query time so init never blocks on I/O.
    pub fn from_config(config: &CentralizedProviderConfig) -> Result<Self, AgentError> {
        let binary_path = detect_copilot_cli()?;
        let model = config
            .model
            .clone()
            .unwrap_or_else(|| model_aliases::DEFAULT.to_string());
        info!(
            "Initializing Copilot CLI brain (binary: {}, model: {})",
            binary_path.display(),
            model
        );
        Ok(Self {
            binary_path,
            model,
            system_prompt: config.system_prompt.clone(),
            load_account_mcp: config.load_account_mcp,
        })
    }

    fn extract_query(messages: &[Message]) -> String {
        messages
            .iter()
            .rev()
            .find(|m| m.role == Role::User)
            .map(|m| m.content.clone())
            .unwrap_or_else(|| "Hello".to_string())
    }

    /// `copilot -p` has no system-prompt flag, so the instructions ride in
    /// front of the query. Sent on every turn, not only the first: Juno
    /// rebuilds its prompt per turn (voice policy, tools), and a resumed
    /// session would otherwise keep the first turn's.
    fn prompt_for(&self, query: &str, with_computer_tool: bool) -> String {
        let mut instructions = self.system_prompt.clone().unwrap_or_default();
        if with_computer_tool {
            if !instructions.is_empty() {
                instructions.push_str("\n\n");
            }
            instructions.push_str(super::claude_cli::MCP_TOOL_GUIDANCE);
        }
        if instructions.is_empty() {
            query.to_string()
        } else {
            format!("System: {instructions}\n\nUser: {query}")
        }
    }

    fn build_args(
        &self,
        prompt: &str,
        resume: Option<&str>,
        mcp_config_file: Option<&Path>,
        disabled_user_servers: &[String],
    ) -> Vec<String> {
        let mut args = vec![
            "-p".to_string(),
            prompt.to_string(),
            "--output-format".to_string(),
            "json".to_string(),
            "--model".to_string(),
            self.model.clone(),
            // Nobody is there to answer a question mid-turn.
            "--no-ask-user".to_string(),
            // The GitHub MCP server loads repository tools into a desktop
            // assistant and cost 2 s of a 6 s session in the spike.
            "--disable-builtin-mcps".to_string(),
            // An update swapping the binary mid-turn is not Juno's to allow.
            "--no-auto-update".to_string(),
        ];
        if let Some(session) = resume {
            args.push(format!("--resume={session}"));
        }
        if let Some(file) = mcp_config_file {
            args.push("--additional-mcp-config".to_string());
            args.push(format!("@{}", file.display()));
            // Pre-approve Juno's own server and nothing else: in `-p` mode a
            // tool that is not allowed is refused.
            args.push(format!("--allow-tool={MCP_SERVER_NAME}"));
        }
        for name in disabled_user_servers {
            args.push("--disable-mcp-server".to_string());
            args.push(name.clone());
        }
        args
    }

    /// Spawn Copilot, stream its output back to the UI, return the final text.
    async fn run_streaming(
        &self,
        query: &str,
        app_handle: Option<tauri::AppHandle>,
        message_id: Option<String>,
        cancel_rx: Option<crate::state::CancelReceiver>,
    ) -> Result<String, AgentError> {
        check_cli_auth_status().await?;

        let mcp_file = match app_handle.as_ref() {
            Some(handle) => match juno_mcp::ensure_running(handle).await {
                Ok(endpoint) => write_mcp_config(&endpoint)
                    .map_err(|e| warn!("[CopilotCLI] Could not write Juno's tool config: {e}"))
                    .ok(),
                Err(e) => {
                    warn!("[CopilotCLI] Could not offer Juno's computer tool: {e}");
                    None
                }
            },
            None => None,
        };

        let disabled = if self.load_account_mcp {
            Vec::new()
        } else {
            match dirs::home_dir() {
                Some(home) => tokio::fs::read(home.join(".copilot/mcp-config.json"))
                    .await
                    .map(|b| user_mcp_server_names(&b))
                    .unwrap_or_default(),
                None => Vec::new(),
            }
        };

        let conversation_id = super::claude_cli::conversation_id_for(&app_handle).await;
        let resume = conversation_id.as_deref().and_then(session_for);

        let msg_id = message_id.unwrap_or_else(|| uuid::Uuid::new_v4().to_string());
        if let Some(ref handle) = app_handle {
            crate::agent::tool_logger::emit_stream_start(handle, msg_id.clone());
        }

        let prompt = self.prompt_for(query, mcp_file.is_some());
        let args = self.build_args(&prompt, resume.as_deref(), mcp_file.as_deref(), &disabled);
        info!(
            "Spawning Copilot CLI: {} (model {}, resume: {})",
            self.binary_path.display(),
            self.model,
            resume.is_some()
        );

        crate::turn_timing::note_llm("copilot_cli/oneshot", &self.model);
        crate::turn_timing::mark(crate::turn_timing::Stage::LlmRequestSent);
        let spawn_result = copilot_command(&self.binary_path)
            .args(&args)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .stdin(Stdio::null())
            .kill_on_drop(true)
            .spawn();

        let end_with = |text: String| {
            if let Some(ref handle) = app_handle {
                crate::agent::tool_logger::emit_stream_end(handle, msg_id.clone(), text);
            }
        };

        let mut child = match spawn_result {
            Ok(child) => child,
            Err(e) => {
                let failure = format!("Failed to spawn Copilot CLI: {e}");
                end_with(format!("Error: {failure}"));
                return Err(AgentError::LlmError(failure));
            }
        };

        let Some(stdout) = child.stdout.take() else {
            end_with("Error: Failed to capture Copilot CLI stdout".to_string());
            return Err(AgentError::LlmError(
                "Failed to capture Copilot CLI stdout".to_string(),
            ));
        };
        // Drain stderr concurrently so a full pipe cannot stall the child, and
        // so we can say why it refused.
        let stderr_handle = child.stderr.take().map(|se| {
            tauri::async_runtime::spawn(async move {
                let mut buf = String::new();
                let mut reader = BufReader::new(se);
                let _ = tokio::io::AsyncReadExt::read_to_string(&mut reader, &mut buf).await;
                buf
            })
        });

        let stream = process_stream(stdout, &app_handle, &msg_id);
        let outcome = match cancel_rx {
            Some(mut rx) => {
                tokio::select! {
                    result = stream => Some(result),
                    _ = async {
                        loop {
                            if *rx.borrow() { return; }
                            if rx.changed().await.is_err() {
                                std::future::pending::<()>().await;
                            }
                        }
                    } => None,
                }
            }
            None => Some(stream.await),
        };

        let Some(outcome) = outcome else {
            info!("Copilot CLI cancelled via escape key, killing subprocess");
            let _ = child.kill().await;
            end_with("Cancelled".to_string());
            return Err(AgentError::Terminated);
        };

        let StreamResult {
            text,
            session_id,
            exit_code,
            failure,
            idle_timed_out,
        } = outcome;

        if idle_timed_out {
            let _ = child.kill().await;
            end_with("Copilot CLI timed out".to_string());
            return Err(AgentError::Timeout(format!(
                "Copilot CLI went silent for {} minutes",
                CLI_IDLE_TIMEOUT.as_secs() / 60
            )));
        }

        let status = child
            .wait()
            .await
            .map_err(|e| AgentError::LlmError(format!("Copilot CLI process error: {e}")))?;
        let stderr_buf = match stderr_handle {
            Some(handle) => handle.await.unwrap_or_else(|e| {
                error!("Failed to read Copilot CLI stderr: {e}");
                String::new()
            }),
            None => String::new(),
        };
        if !stderr_buf.is_empty() {
            let snippet = stderr_buf.chars().take(500).collect::<String>();
            if status.success() {
                warn!("Copilot CLI stderr (success): {snippet}");
            } else {
                error!("Copilot CLI stderr (failure): {snippet}");
            }
        }

        if let (Some(conversation), Some(session)) = (conversation_id.as_deref(), session_id) {
            remember_session(conversation, &session);
        }

        let failed = !status.success() || exit_code.is_some_and(|c| c != 0);
        if failed && text.trim().is_empty() {
            let reason = failure.unwrap_or_else(|| stderr_buf.clone());
            let error = classify_failure(&reason, &stderr_buf, &status);
            end_with(format!("Error: {error}"));
            return Err(AgentError::LlmError(error));
        }

        remember_sign_in(SignIn::SignedIn);
        end_with(text.clone());
        Ok(text)
    }
}

/// Map a failed turn to the sentence the bar shows.
///
/// This is the one place Juno reads Copilot's own words: the CLI has no
/// structured error code for either case, so a login request and an image
/// refusal are recognised by the terms Copilot uses for them. Anything else is
/// passed through verbatim, which is also how a spent quota is surfaced.
fn classify_failure(reason: &str, stderr: &str, status: &std::process::ExitStatus) -> String {
    let haystack = format!("{reason}\n{stderr}").to_ascii_lowercase();
    if haystack.contains("copilot login") || haystack.contains("not authenticated") {
        remember_sign_in(SignIn::SignedOut);
        return SIGNED_OUT_MESSAGE.to_string();
    }
    if haystack.contains("vision") || haystack.contains("image input") {
        return NO_VISION_MESSAGE.to_string();
    }
    let said = reason.trim();
    if said.is_empty() {
        format!(
            "Copilot CLI exited with status {} and no output",
            super::claude_cli::exit_code_label(status)
        )
    } else {
        said.chars().take(500).collect()
    }
}

/// What one JSONL line means to Juno.
#[derive(Debug, PartialEq, Eq)]
enum Event {
    /// `session.start`: the id `--resume` takes.
    SessionStart(String),
    /// A streamed piece of an assistant message.
    Delta { message_id: String, text: String },
    /// A finished assistant message.
    Message { message_id: String, text: String },
    /// The final `result` line.
    Result { exit_code: i64 },
    /// A session error, with Copilot's message.
    Failure(String),
    /// Everything else: lifecycle, usage, ephemeral noise, non-JSON.
    Other,
}

fn parse_line(line: &str) -> Event {
    let Ok(parsed) = serde_json::from_str::<Value>(line.trim()) else {
        return Event::Other;
    };
    let data = parsed.get("data");
    let str_at = |v: Option<&Value>, key: &str| -> Option<String> {
        v.and_then(|d| d.get(key))
            .and_then(Value::as_str)
            .map(str::to_string)
    };
    match parsed.get("type").and_then(Value::as_str).unwrap_or("") {
        "session.start" => match str_at(data, "sessionId") {
            Some(id) => Event::SessionStart(id),
            None => Event::Other,
        },
        "assistant.message_delta" => match str_at(data, "deltaContent") {
            Some(text) if !text.is_empty() => Event::Delta {
                message_id: str_at(data, "messageId").unwrap_or_default(),
                text,
            },
            _ => Event::Other,
        },
        "assistant.message" => match str_at(data, "content") {
            Some(text) if !text.is_empty() => Event::Message {
                message_id: str_at(data, "messageId").unwrap_or_default(),
                text,
            },
            _ => Event::Other,
        },
        "result" => Event::Result {
            exit_code: parsed.get("exitCode").and_then(Value::as_i64).unwrap_or(0),
        },
        "session.error" | "error" => {
            Event::Failure(str_at(data, "message").unwrap_or_else(|| "Copilot turn failed".into()))
        }
        _ => Event::Other,
    }
}

#[derive(Default)]
struct StreamResult {
    text: String,
    session_id: Option<String>,
    exit_code: Option<i64>,
    failure: Option<String>,
    idle_timed_out: bool,
}

/// Read Copilot's JSONL and emit text as it arrives.
///
/// Deltas stream as they come; a finished `assistant.message` is only emitted
/// when no delta carried its text, so nothing is shown twice. Everything goes
/// through the `<TTS>` splitter the Claude CLI uses, so a voice turn speaks and
/// the tags never reach the visible answer.
async fn process_stream(
    stdout: tokio::process::ChildStdout,
    app_handle: &Option<tauri::AppHandle>,
    msg_id: &str,
) -> StreamResult {
    let mut lines = BufReader::new(stdout).lines();
    let mut result = StreamResult::default();
    let mut accumulated = String::new();
    let mut tts_stream = crate::agent::tts_tags::TtsTagStream::new();
    let mut spoken_blocks: Vec<String> = Vec::new();
    let mut streamed: HashSet<String> = HashSet::new();
    let mut current_message: Option<String> = None;
    let mut first_token_marked = false;

    loop {
        let next = match tokio::time::timeout(CLI_IDLE_TIMEOUT, lines.next_line()).await {
            Ok(next) => next,
            Err(_) => {
                result.idle_timed_out = true;
                break;
            }
        };
        let line = match next {
            Ok(Some(line)) => line,
            Ok(None) => break,
            Err(e) => {
                warn!("Error reading Copilot CLI stdout: {e}");
                break;
            }
        };
        let mut show = |id: &str, text: &str| {
            if !first_token_marked {
                first_token_marked = true;
                crate::turn_timing::mark(crate::turn_timing::Stage::LlmFirstToken);
            }
            if current_message.as_deref() != Some(id) {
                if current_message.is_some() {
                    ClaudeCliBrain::separate_text_block(
                        app_handle,
                        msg_id,
                        &mut tts_stream,
                        &mut accumulated,
                        &mut spoken_blocks,
                    );
                }
                current_message = Some(id.to_string());
            }
            ClaudeCliBrain::emit_display_text(
                app_handle,
                msg_id,
                text,
                &mut tts_stream,
                &mut accumulated,
                &mut spoken_blocks,
            );
        };
        match parse_line(&line) {
            Event::SessionStart(id) => result.session_id = Some(id),
            Event::Delta { message_id, text } => {
                streamed.insert(message_id.clone());
                show(&message_id, &text);
            }
            Event::Message { message_id, text } => {
                if !streamed.contains(&message_id) {
                    show(&message_id, &text);
                }
                // A `<TTS>` this message left open must not read out the next.
                ClaudeCliBrain::end_text_block(
                    app_handle,
                    msg_id,
                    &mut tts_stream,
                    &mut spoken_blocks,
                );
            }
            Event::Result { exit_code } => result.exit_code = Some(exit_code),
            Event::Failure(message) => {
                warn!("[CopilotCLI] Turn failed: {message}");
                result.failure = Some(message);
            }
            Event::Other => {}
        }
    }

    ClaudeCliBrain::flush_display_text(
        app_handle,
        msg_id,
        &mut tts_stream,
        &mut accumulated,
        &mut spoken_blocks,
    );
    result.text = accumulated;
    result
}

/// A `copilot` command rooted in the home directory, the way `claude_command`
/// roots `claude`: a GUI app starts in `/`, and a cwd is the first thing a
/// coding agent reads.
pub(crate) fn copilot_command(binary: impl AsRef<std::ffi::OsStr>) -> tokio::process::Command {
    let mut command = tokio::process::Command::new(binary);
    if let Some(home) = dirs::home_dir() {
        command.current_dir(home);
    }
    command
}

#[async_trait]
impl AgentBrain for CopilotCliBrain {
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

#[cfg(test)]
mod tests {
    use super::*;

    /// `LAST_SIGN_IN` is process-wide; tests that touch it take turns.
    static SIGN_IN_LOCK: Mutex<()> = Mutex::new(());

    fn brain(system_prompt: Option<&str>, load_account_mcp: bool) -> CopilotCliBrain {
        CopilotCliBrain {
            binary_path: PathBuf::from("/usr/bin/false"),
            model: "auto".to_string(),
            system_prompt: system_prompt.map(str::to_string),
            load_account_mcp,
        }
    }

    fn endpoint() -> juno_mcp::Endpoint {
        juno_mcp::Endpoint {
            url: "http://127.0.0.1:51234/mcp".to_string(),
            hook_url: "http://127.0.0.1:51234/hook/pretooluse".to_string(),
            token: "secret".to_string(),
        }
    }

    #[test]
    fn turn_args_are_headless_toolless_except_juno_and_keep_the_token_off_argv() {
        let file = PathBuf::from("/tmp/juno-copilot-mcp-1.json");
        let args = brain(None, true).build_args("hi", None, Some(&file), &[]);
        let joined = args.join(" ");
        assert_eq!(args[0], "-p");
        assert_eq!(args[1], "hi");
        assert!(joined.contains("--output-format json"));
        assert!(joined.contains("--model auto"));
        assert!(joined.contains("--disable-builtin-mcps"));
        assert!(joined.contains("--no-ask-user"));
        assert!(joined.contains("--additional-mcp-config @/tmp/juno-copilot-mcp-1.json"));
        assert!(joined.contains("--allow-tool=juno"));
        assert!(!joined.contains("secret"));
        assert!(!joined.contains("--allow-all"));
    }

    #[test]
    fn without_juno_s_server_nothing_is_allowed_and_nothing_resumes() {
        let args = brain(None, true).build_args("hi", None, None, &[]);
        assert!(!args.iter().any(|a| a.starts_with("--allow-tool")));
        assert!(!args.iter().any(|a| a.starts_with("--resume")));
    }

    #[test]
    fn a_follow_up_resumes_the_same_session() {
        let args = brain(None, true).build_args("again", Some("abc-123"), None, &[]);
        assert!(args.contains(&"--resume=abc-123".to_string()));
    }

    #[test]
    fn account_servers_are_disabled_by_name_when_the_setting_is_off() {
        let names = user_mcp_server_names(br#"{"mcpServers":{"slack":{},"linear":{}}}"#);
        assert_eq!(names.len(), 2);
        let args = brain(None, false).build_args("hi", None, None, &names);
        assert_eq!(
            args.iter().filter(|a| *a == "--disable-mcp-server").count(),
            2
        );
        assert!(user_mcp_server_names(b"not json").is_empty());
    }

    #[test]
    fn the_prompt_carries_juno_s_instructions_and_the_tool_steer() {
        let b = brain(Some("You are Juno."), true);
        let with_tool = b.prompt_for("hi", true);
        assert!(with_tool.starts_with("System: You are Juno."));
        assert!(with_tool.contains("`computer`"));
        assert!(with_tool.ends_with("User: hi"));
        assert_eq!(brain(None, true).prompt_for("hi", false), "hi");
    }

    #[test]
    fn mcp_config_names_the_server_juno_with_tools_and_a_bearer_header() {
        let config = mcp_config(&endpoint());
        let server = &config["mcpServers"]["juno"];
        assert_eq!(server["type"], "http");
        assert_eq!(server["tools"], json!(["*"]));
        assert_eq!(server["headers"]["Authorization"], "Bearer secret");
    }

    #[cfg(unix)]
    #[test]
    fn the_config_file_is_private() {
        use std::os::unix::fs::PermissionsExt;
        let dir = std::env::temp_dir().join(format!("juno-copilot-test-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).expect("temp dir");
        let path = write_mcp_config_in(&dir, &endpoint()).expect("written");
        let mode = std::fs::metadata(&path).expect("meta").permissions().mode() & 0o777;
        assert_eq!(mode, 0o600);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn real_jsonl_lines_parse() {
        assert_eq!(
            parse_line(r#"{"type":"session.start","data":{"sessionId":"s-1","version":1}}"#),
            Event::SessionStart("s-1".to_string())
        );
        assert_eq!(
            parse_line(
                r#"{"type":"assistant.message_delta","data":{"messageId":"m","deltaContent":"Hi"},"ephemeral":true}"#
            ),
            Event::Delta {
                message_id: "m".to_string(),
                text: "Hi".to_string()
            }
        );
        assert_eq!(
            parse_line(
                r#"{"type":"assistant.message","data":{"messageId":"m","content":"Hi","toolRequests":[]}}"#
            ),
            Event::Message {
                message_id: "m".to_string(),
                text: "Hi".to_string()
            }
        );
        assert_eq!(
            parse_line(r#"{"type":"result","exitCode":1,"usage":{}}"#),
            Event::Result { exit_code: 1 }
        );
        assert_eq!(
            parse_line(r#"{"type":"session.usage_checkpoint","data":{}}"#),
            Event::Other
        );
        // A tool-call-only message has empty content and shows nothing.
        assert_eq!(
            parse_line(r#"{"type":"assistant.message","data":{"messageId":"m","content":""}}"#),
            Event::Other
        );
        assert_eq!(parse_line("Warning: not json"), Event::Other);
    }

    #[test]
    fn config_login_evidence_is_read_leniently() {
        assert!(config_records_a_login(
            br#"{"lastLoggedInUser":{"host":"https://github.com","login":"a"}}"#
        ));
        assert!(config_records_a_login(
            b"// User settings\n{\"logged_in_users\":[{\"login\":\"a\"}]}"
        ));
        assert!(!config_records_a_login(
            br#"{"firstLaunchAt":"2026-11-10","loggedInUsers":[]}"#
        ));
        assert!(!config_records_a_login(b"garbage"));
    }

    #[test]
    fn sign_in_round_trips_and_only_proof_counts() {
        let _turn = SIGN_IN_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        remember_sign_in(SignIn::SignedIn);
        assert_eq!(last_known_sign_in(), SignIn::SignedIn);
        remember_sign_in(SignIn::SignedOut);
        assert_eq!(last_known_sign_in(), SignIn::SignedOut);
        remember_sign_in(SignIn::Unknown);
        let unknown = CliStatus {
            installed: true,
            sign_in: SignIn::Unknown,
        };
        assert!(!unknown.is_signed_in(), "unknown is not proof");
        let proven = CliStatus {
            installed: true,
            sign_in: SignIn::SignedIn,
        };
        assert!(proven.is_signed_in());
    }

    #[test]
    fn sessions_are_remembered_per_conversation() {
        remember_session("conv-a", "sess-1");
        remember_session("conv-b", "sess-2");
        assert_eq!(session_for("conv-a").as_deref(), Some("sess-1"));
        assert_eq!(session_for("conv-b").as_deref(), Some("sess-2"));
        assert_eq!(session_for("conv-none"), None);
    }

    #[cfg(unix)]
    #[test]
    fn failures_map_to_the_one_sentence_the_bar_shows() {
        use std::os::unix::process::ExitStatusExt;
        let _turn = SIGN_IN_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let status = std::process::ExitStatus::from_raw(1 << 8);
        assert_eq!(
            classify_failure("Run `copilot login` first", "", &status),
            SIGNED_OUT_MESSAGE
        );
        assert_eq!(last_known_sign_in(), SignIn::SignedOut);
        remember_sign_in(SignIn::Unknown);
        assert_eq!(
            classify_failure("model does not support vision", "", &status),
            NO_VISION_MESSAGE
        );
        // A spent quota is the agent's own text, verbatim.
        assert_eq!(
            classify_failure("You have used all premium requests", "", &status),
            "You have used all premium requests"
        );
    }
}

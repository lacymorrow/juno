//! Codex CLI provider: uses the `codex` binary (OpenAI Codex CLI) as a
//! subprocess-based AI provider. Mirrors [`super::claude_cli`] in shape: the
//! user already pays for a ChatGPT plan, so Juno should run on it rather than
//! ask for an OpenAI API key on top.
//!
//! The ChatGPT login lives in `~/.codex/auth.json`: `auth_mode: "chatgpt"` is
//! the proof of a plan login, which is what the default-provider rule picks up
//! as a credential. An `apikey` auth is still a login, but it is the API-key
//! case Juno already covers via the OpenAI provider, so this provider is
//! deliberately tied to the ChatGPT auth mode.
//!
//! A turn runs on a warm `codex app-server` thread ([`super::codex_session`])
//! when it can, and on a one-shot `codex exec --json -s read-only` when it
//! cannot. Both paths get Juno's computer tool over MCP, stream through the
//! same `<TTS>` funnel and Tauri events as the Claude CLI, and leave the shell
//! read-only: the desktop goes through Juno, not through Codex's sandbox.
//!
//! NOTE: macOS-only, matching Juno's platform target.

use async_trait::async_trait;
use serde::Deserialize;
use serde_json::Value;
use std::path::PathBuf;
use std::process::Stdio;
use std::sync::atomic::{AtomicU8, Ordering};
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::time::Duration;
use tracing::{debug, error, info, warn};

use super::claude_cli::ClaudeCliBrain;
use super::codex_session;
use super::juno_mcp;
use crate::agent::core::{AgentAction, AgentError, Message, Role, ToolDefinition};
use crate::agent::traits::AgentBrain;
use crate::settings::ProviderConfig as CentralizedProviderConfig;

/// Codex CLI model ids, passed as `-m`. The catalog lives in
/// `Provider::model_definitions`; this is only the fallback when a config
/// entry has no model at all.
pub mod model_aliases {
    /// The ChatGPT plan's default model as of codex-cli 0.159.0.
    pub const DEFAULT: &str = "gpt-6-luna";
}

/// Upper bound on how long one `codex exec` call runs before we give up. Codex
/// chat turns are generally fast; this is the same budget the Claude CLI
/// provider uses, kept generous because somebody's ChatGPT plan is on the
/// other end of the request.
const CODEX_TIMEOUT: Duration = Duration::from_secs(300);

/// Which login the local CLI has, if any.
///
/// Separate variants for the two login kinds, because they mean different
/// things to the default-provider rule: a ChatGPT login is a plan credential
/// that Juno should pick up; an API-key login is already the OpenAI-provider
/// case and must not be read as a reason to switch to the CLI.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum AuthMode {
    /// Nobody has asked yet, or the auth file was unreadable.
    #[default]
    Unknown,
    /// `auth_mode: "chatgpt"` in `~/.codex/auth.json`.
    ChatGPT,
    /// `auth_mode: "apikey"`: a login, but not the one this provider is for.
    ApiKey,
    /// The file is missing, empty, or has `auth_mode: null`.
    SignedOut,
}

/// `AuthMode` as a `u8`, so a synchronous Settings render can read the last
/// answer without spawning a subprocess.
static LAST_AUTH_MODE: AtomicU8 = AtomicU8::new(0);

fn remember_auth(mode: AuthMode) {
    let encoded = match mode {
        AuthMode::Unknown => 0,
        AuthMode::ChatGPT => 1,
        AuthMode::ApiKey => 2,
        AuthMode::SignedOut => 3,
    };
    LAST_AUTH_MODE.store(encoded, Ordering::Relaxed);
}

/// The most recent auth answer, without re-reading the file.
pub fn last_known_auth_mode() -> AuthMode {
    match LAST_AUTH_MODE.load(Ordering::Relaxed) {
        1 => AuthMode::ChatGPT,
        2 => AuthMode::ApiKey,
        3 => AuthMode::SignedOut,
        _ => AuthMode::Unknown,
    }
}

/// Everything Juno knows about the local Codex CLI, gathered in one pass.
#[derive(Debug, Clone, Default)]
pub struct CliStatus {
    /// The `codex` binary exists.
    pub installed: bool,
    /// What `~/.codex/auth.json` said about this install.
    pub auth_mode: AuthMode,
    /// The ChatGPT account's email, when the id_token decoded. Settings uses
    /// this so the person can see whose quota Juno is spending.
    pub email: Option<String>,
}

impl CliStatus {
    /// Proof of a ChatGPT-plan login, which is what the default-provider rule
    /// treats as a credential this provider can run on.
    pub fn is_signed_in(&self) -> bool {
        self.auth_mode == AuthMode::ChatGPT
    }

    /// Not a *known* bad login. What the Settings listing uses, to avoid
    /// greying out a provider on an answer that was merely unreadable.
    pub fn could_run(&self) -> bool {
        self.installed && !matches!(self.auth_mode, AuthMode::SignedOut)
    }
}

/// Find the `codex` binary.
///
/// Checks common install locations first (cheap `exists()` calls), then falls
/// back to a manual PATH walk. The npm-npx cache path is included because
/// `npx codex` leaves a resolved binary there, and it is a common install
/// path people reach for before Homebrew.
pub fn detect_codex_cli() -> Result<PathBuf, AgentError> {
    let home = dirs::home_dir();
    let candidates = [
        home.as_ref().map(|h| h.join(".local/bin/codex")),
        Some(PathBuf::from("/opt/homebrew/bin/codex")),
        Some(PathBuf::from("/usr/local/bin/codex")),
        Some(PathBuf::from("/usr/bin/codex")),
        home.as_ref().map(|h| h.join(".bun/bin/codex")),
    ];

    for candidate in candidates.into_iter().flatten() {
        if candidate.exists() {
            debug!("Found Codex CLI at: {}", candidate.display());
            return Ok(candidate);
        }
    }

    if let Ok(path_var) = std::env::var("PATH") {
        for dir in std::env::split_paths(&path_var) {
            let candidate = dir.join("codex");
            if candidate.is_file() {
                debug!("Found Codex CLI via PATH: {}", candidate.display());
                return Ok(candidate);
            }
        }
    }

    Err(AgentError::ConfigurationError(
        "Codex CLI (codex) not found. Install it from https://github.com/openai/codex".to_string(),
    ))
}

/// Quick check: is the Codex CLI binary available? (No auth check.)
pub fn is_codex_cli_available() -> bool {
    detect_codex_cli().is_ok()
}

/// Shape of the fields this module reads out of `~/.codex/auth.json`. Codex
/// writes a larger structure (tokens, refresh timestamp, …) but Juno only
/// needs the auth mode and, when present, the email claim inside the id token.
#[derive(Debug, Deserialize)]
struct AuthFile {
    auth_mode: Option<String>,
    #[serde(default)]
    tokens: Option<AuthTokens>,
    #[serde(rename = "OPENAI_API_KEY")]
    #[allow(dead_code)]
    openai_api_key: Option<String>,
}

#[derive(Debug, Deserialize)]
struct AuthTokens {
    id_token: Option<String>,
}

/// Read `~/.codex/auth.json` once and report what it says.
///
/// Never returns an error: everything that could go wrong is one of the
/// [`AuthMode`] variants. A missing or malformed file reads as `SignedOut`
/// rather than `Unknown`, because that is what it operationally means: the
/// CLI has no login to use. `Unknown` is reserved for "we have not asked
/// yet", which only matters synchronously in Settings.
pub async fn cli_status() -> CliStatus {
    if detect_codex_cli().is_err() {
        remember_auth(AuthMode::Unknown);
        return CliStatus::default();
    }

    let Some(home) = dirs::home_dir() else {
        remember_auth(AuthMode::Unknown);
        return CliStatus {
            installed: true,
            auth_mode: AuthMode::Unknown,
            email: None,
        };
    };
    let path = home.join(".codex/auth.json");

    let bytes = match tokio::fs::read(&path).await {
        Ok(bytes) => bytes,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            remember_auth(AuthMode::SignedOut);
            return CliStatus {
                installed: true,
                auth_mode: AuthMode::SignedOut,
                email: None,
            };
        }
        Err(e) => {
            warn!("[CodexCLI] Could not read {}: {}", path.display(), e);
            remember_auth(AuthMode::Unknown);
            return CliStatus {
                installed: true,
                auth_mode: AuthMode::Unknown,
                email: None,
            };
        }
    };

    let parsed: Option<AuthFile> = serde_json::from_slice(&bytes).ok();
    let mode = parsed
        .as_ref()
        .and_then(|a| a.auth_mode.as_deref())
        .map(|m| match m {
            "chatgpt" => AuthMode::ChatGPT,
            "apikey" => AuthMode::ApiKey,
            _ => AuthMode::SignedOut,
        })
        .unwrap_or(AuthMode::SignedOut);
    remember_auth(mode);

    let email = parsed
        .as_ref()
        .and_then(|a| a.tokens.as_ref())
        .and_then(|t| t.id_token.as_deref())
        .and_then(email_from_id_token);

    CliStatus {
        installed: true,
        auth_mode: mode,
        email,
    }
}

/// Decode the `email` claim out of a JWT without verifying it. We never trust
/// this for authorization (the CLI has already); we only use it to show
/// whose account Juno is spending in Settings. Returns `None` on anything
/// unexpected so a broken token is a missing email, not a crash.
fn email_from_id_token(id_token: &str) -> Option<String> {
    use base64::Engine;
    let payload_b64 = id_token.split('.').nth(1)?;
    let bytes = base64::engine::general_purpose::URL_SAFE_NO_PAD
        .decode(payload_b64)
        .ok()?;
    let json: Value = serde_json::from_slice(&bytes).ok()?;
    json.get("email")
        .and_then(Value::as_str)
        .map(str::to_string)
}

/// Pre-query auth check. The CLI validates its own token on every call, so
/// Juno's job is only to refuse early when there is no ChatGPT login at all.
pub async fn check_cli_auth_status() -> Result<(), AgentError> {
    detect_codex_cli()?;
    let status = cli_status().await;
    if status.is_signed_in() {
        if let Some(email) = status.email {
            info!("Codex CLI authenticated as: {}", email);
        }
        Ok(())
    } else {
        Err(AgentError::ConfigurationError(
            "Codex CLI is not logged in with ChatGPT. Run `codex login` to authenticate."
                .to_string(),
        ))
    }
}

/// Codex CLI brain. One subprocess per query, streaming NDJSON back through
/// Tauri events in the same shape the Anthropic provider uses.
pub struct CodexCliBrain {
    binary_path: PathBuf,
    model: String,
    system_prompt: Option<String>,
}

impl CodexCliBrain {
    /// Build from centralized provider config. Checks the binary exists; auth
    /// is checked lazily at query time so init never blocks on filesystem I/O.
    pub fn from_config(config: &CentralizedProviderConfig) -> Result<Self, AgentError> {
        let binary_path = detect_codex_cli()?;
        let model = config
            .model
            .clone()
            .unwrap_or_else(|| model_aliases::DEFAULT.to_string());

        info!(
            "Initializing Codex CLI brain (binary: {}, model: {})",
            binary_path.display(),
            model,
        );

        Ok(Self {
            binary_path,
            model,
            system_prompt: config.system_prompt.clone(),
        })
    }

    /// Extract the latest user query from the message history. Codex has no
    /// in-process concept of a conversation, so each turn is sent on its own;
    /// prior turns live in the memory the orchestrator already stitches into
    /// its prompt.
    fn extract_query(messages: &[Message]) -> String {
        messages
            .iter()
            .rev()
            .find(|m| m.role == Role::User)
            .map(|m| m.content.clone())
            .unwrap_or_else(|| "Hello".to_string())
    }

    /// Build the subprocess command line. Read-only sandbox is deliberate:
    /// Codex has its own shell tools and this provider is chat-only in v1, so
    /// any filesystem touch would be surprising. If somebody later wants
    /// write access, that is an explicit decision, not a default.
    /// What Codex is told about Juno: the system prompt, plus, when Juno's
    /// computer tool is on offer, the same steer toward it the Claude CLI gets.
    fn developer_instructions(&self, with_computer_tool: bool) -> String {
        let mut instructions = self.system_prompt.clone().unwrap_or_default();
        if with_computer_tool {
            if !instructions.is_empty() {
                instructions.push_str("\n\n");
            }
            instructions.push_str(super::claude_cli::MCP_TOOL_GUIDANCE);
        }
        instructions
    }

    fn build_args(&self, query: &str, mcp: Option<&juno_mcp::Endpoint>) -> Vec<String> {
        let mut args = vec![
            "exec".to_string(),
            "--json".to_string(),
            // Juno spawns from the home directory, which is not a git repo
            // and, for most people, not a directory Codex has been told to
            // trust. Without this flag `codex exec` refuses to start there.
            "--skip-git-repo-check".to_string(),
            // Juno keeps its own conversation memory. A rollout file per
            // spoken question would only clutter the person's Codex history.
            "--ephemeral".to_string(),
            "-s".to_string(),
            "read-only".to_string(),
            "-m".to_string(),
            self.model.clone(),
        ];
        if let Some(mcp) = mcp {
            args.extend(codex_session::mcp_config_args(&mcp.url));
        }
        let instructions = self.developer_instructions(mcp.is_some());
        if instructions.is_empty() {
            args.push(query.to_string());
        } else {
            // `codex exec` has no system-prompt flag, so the instructions ride
            // in front of the query. The persistent path sends them properly,
            // as `developerInstructions`; this is only its fallback.
            args.push(format!("System: {}\n\nUser: {}", instructions, query));
        }
        args
    }

    /// Spawn Codex, stream its output back to the UI, return the final text.
    async fn run_streaming(
        &self,
        query: &str,
        app_handle: Option<tauri::AppHandle>,
        message_id: Option<String>,
        cancel_rx: Option<crate::state::CancelReceiver>,
    ) -> Result<String, AgentError> {
        check_cli_auth_status().await?;

        // Juno's own computer tool, served in-process. Without an app handle
        // (headless, tests) there is no desktop and Codex runs toolless.
        let mcp = match app_handle.as_ref() {
            Some(handle) => juno_mcp::ensure_running(handle)
                .await
                .map_err(|e| warn!("[CodexCLI] Could not offer Juno's computer tool: {e}"))
                .ok(),
            None => None,
        };

        // The warm app-server path first. Anything it cannot do comes back as
        // `Unavailable` having shown nothing, and the one-shot path below runs
        // the same message. Nobody is told.
        if let Some(handle) = app_handle.as_ref() {
            if let Some(conversation_id) = super::claude_cli::conversation_id_for(&app_handle).await
            {
                let request = codex_session::TurnRequest {
                    launch: codex_session::Launch {
                        binary: self.binary_path.clone(),
                        mcp: mcp.clone(),
                    },
                    thread: codex_session::ThreadConfig {
                        model: self.model.clone(),
                        instructions: self.developer_instructions(mcp.is_some()),
                    },
                    conversation_id: &conversation_id,
                    query,
                    app_handle: handle,
                    message_id: message_id.clone(),
                    cancel_rx: cancel_rx.clone(),
                };
                match codex_session::run_turn(request).await {
                    Ok(codex_session::TurnOutcome::Completed(text)) => return Ok(text),
                    Ok(codex_session::TurnOutcome::Unavailable) => {
                        debug!("[CodexCLI] app-server unavailable; running codex exec");
                    }
                    // The turn ran and failed or was cancelled. Running it
                    // again would spend the plan twice for one message.
                    Err(e) => return Err(e),
                }
            }
        }

        let msg_id = message_id.unwrap_or_else(|| uuid::Uuid::new_v4().to_string());
        if let Some(ref handle) = app_handle {
            crate::agent::tool_logger::emit_stream_start(handle, msg_id.clone());
        }

        let args = self.build_args(query, mcp.as_ref());
        info!(
            "Spawning Codex CLI: {} {}",
            self.binary_path.display(),
            args.iter().take(5).cloned().collect::<Vec<_>>().join(" ")
        );

        crate::turn_timing::note_llm("codex_cli/oneshot", &self.model);
        crate::turn_timing::mark(crate::turn_timing::Stage::LlmRequestSent);
        let spawn_result = codex_command(&self.binary_path)
            .args(&args)
            // The bearer token for Juno's tool server, by name, never on argv.
            .envs(
                mcp.as_ref()
                    .map(|m| (codex_session::MCP_TOKEN_ENV, m.token.clone())),
            )
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .stdin(Stdio::null())
            .kill_on_drop(true)
            .spawn();

        let mut child = match spawn_result {
            Ok(child) => child,
            Err(e) => {
                let failure = format!("Failed to spawn Codex CLI: {}", e);
                if let Some(ref handle) = app_handle {
                    crate::agent::tool_logger::emit_stream_end(
                        handle,
                        msg_id.clone(),
                        format!("Error: {}", failure),
                    );
                }
                return Err(AgentError::LlmError(failure));
            }
        };

        let stdout = child.stdout.take().ok_or_else(|| {
            AgentError::LlmError("Failed to capture Codex CLI stdout".to_string())
        })?;

        // Drain stderr concurrently to prevent pipe-buffer deadlocks and so we
        // can log why Codex refused when it did.
        let stderr = child.stderr.take();
        let stderr_handle = stderr.map(|se| {
            tauri::async_runtime::spawn(async move {
                let mut buf = String::new();
                let mut reader = BufReader::new(se);
                let _ = tokio::io::AsyncReadExt::read_to_string(&mut reader, &mut buf).await;
                buf
            })
        });

        let stream_future = tokio::time::timeout(
            CODEX_TIMEOUT,
            Self::process_stream(stdout, &app_handle, &msg_id),
        );

        let final_text = if let Some(mut rx) = cancel_rx {
            tokio::select! {
                result = stream_future => match result {
                    Ok(Ok(text)) => text,
                    Ok(Err(e)) => {
                        if let Some(ref handle) = app_handle {
                            crate::agent::tool_logger::emit_stream_end(
                                handle,
                                msg_id.clone(),
                                format!("Error: {}", e),
                            );
                        }
                        return Err(e);
                    }
                    Err(_elapsed) => {
                        let _ = child.kill().await;
                        if let Some(ref handle) = app_handle {
                            crate::agent::tool_logger::emit_stream_end(
                                handle,
                                msg_id.clone(),
                                "Codex CLI timed out".to_string(),
                            );
                        }
                        return Err(AgentError::Timeout(format!(
                            "Codex CLI timed out after {} seconds",
                            CODEX_TIMEOUT.as_secs()
                        )));
                    }
                },
                _ = async {
                    loop {
                        if *rx.borrow() { return; }
                        if rx.changed().await.is_err() {
                            std::future::pending::<()>().await;
                        }
                    }
                } => {
                    info!("Codex CLI cancelled via escape key, killing subprocess");
                    let _ = child.kill().await;
                    if let Some(ref handle) = app_handle {
                        crate::agent::tool_logger::emit_stream_end(
                            handle,
                            msg_id.clone(),
                            "Cancelled".to_string(),
                        );
                    }
                    return Err(AgentError::Terminated);
                }
            }
        } else {
            match stream_future.await {
                Ok(Ok(text)) => text,
                Ok(Err(e)) => {
                    if let Some(ref handle) = app_handle {
                        crate::agent::tool_logger::emit_stream_end(
                            handle,
                            msg_id.clone(),
                            format!("Error: {}", e),
                        );
                    }
                    return Err(e);
                }
                Err(_elapsed) => {
                    let _ = child.kill().await;
                    if let Some(ref handle) = app_handle {
                        crate::agent::tool_logger::emit_stream_end(
                            handle,
                            msg_id.clone(),
                            "Codex CLI timed out".to_string(),
                        );
                    }
                    return Err(AgentError::Timeout(format!(
                        "Codex CLI timed out after {} seconds",
                        CODEX_TIMEOUT.as_secs()
                    )));
                }
            }
        };

        let status = child
            .wait()
            .await
            .map_err(|e| AgentError::LlmError(format!("Codex CLI process error: {}", e)))?;

        let stderr_buf = match stderr_handle {
            Some(handle) => match handle.await {
                Ok(buf) => buf,
                Err(e) => {
                    error!("Failed to read Codex CLI stderr: {}", e);
                    String::new()
                }
            },
            None => String::new(),
        };
        if !stderr_buf.is_empty() {
            let level_msg = stderr_buf.chars().take(500).collect::<String>();
            if status.success() {
                warn!("Codex CLI stderr (success): {}", level_msg);
            } else {
                error!("Codex CLI stderr (failure): {}", level_msg);
            }
        }

        if let Some(ref handle) = app_handle {
            crate::agent::tool_logger::emit_stream_end(handle, msg_id.clone(), final_text.clone());
        }

        if final_text.is_empty() && !status.success() {
            return Err(AgentError::LlmError(format!(
                "Codex CLI exited with status {} and no output",
                status
            )));
        }

        Ok(final_text)
    }

    /// Parse Codex NDJSON and emit text as it arrives.
    ///
    /// `codex exec --json` reports whole items, not token deltas, so each
    /// `item.completed { type: "agent_message" }` lands as one chunk. It goes
    /// through the same `<TTS>` splitter the Claude CLI uses, so a voice turn
    /// speaks and the tags never reach the visible answer.
    async fn process_stream(
        stdout: tokio::process::ChildStdout,
        app_handle: &Option<tauri::AppHandle>,
        msg_id: &str,
    ) -> Result<String, AgentError> {
        let reader = BufReader::new(stdout);
        let mut lines = reader.lines();
        let mut accumulated = String::new();
        let mut tts_stream = crate::agent::tts_tags::TtsTagStream::new();
        let mut spoken_blocks: Vec<String> = Vec::new();
        let mut failure: Option<String> = None;
        let mut messages_seen = 0usize;

        loop {
            match lines.next_line().await {
                Ok(Some(line)) => match parse_exec_line(&line) {
                    ExecEvent::AgentMessage(text) => {
                        crate::turn_timing::mark(crate::turn_timing::Stage::LlmFirstToken);
                        // Separate messages are separate paragraphs, not one
                        // run-on sentence.
                        let text = if messages_seen > 0 {
                            format!("\n\n{text}")
                        } else {
                            text
                        };
                        messages_seen += 1;
                        ClaudeCliBrain::emit_display_text(
                            app_handle,
                            msg_id,
                            &text,
                            &mut tts_stream,
                            &mut accumulated,
                            &mut spoken_blocks,
                        );
                    }
                    ExecEvent::Failed(message) => {
                        warn!("[CodexCLI] Turn failed: {}", message);
                        failure = Some(message);
                    }
                    ExecEvent::Other => {}
                },
                Ok(None) => break,
                Err(e) => {
                    warn!("Error reading Codex CLI stdout: {}", e);
                    break;
                }
            }
        }

        ClaudeCliBrain::flush_display_text(
            app_handle,
            msg_id,
            &mut tts_stream,
            &mut accumulated,
            &mut spoken_blocks,
        );

        match failure {
            Some(message) if accumulated.trim().is_empty() && spoken_blocks.is_empty() => {
                Err(AgentError::LlmError(message))
            }
            _ => Ok(accumulated),
        }
    }
}

/// What one `codex exec --json` line means to Juno.
#[derive(Debug, PartialEq, Eq)]
enum ExecEvent {
    /// A finished assistant message.
    AgentMessage(String),
    /// The turn failed, with the CLI's message.
    Failed(String),
    /// Anything else: lifecycle, reasoning, usage, non-JSON noise.
    Other,
}

fn parse_exec_line(line: &str) -> ExecEvent {
    let Ok(parsed) = serde_json::from_str::<Value>(line.trim()) else {
        return ExecEvent::Other;
    };
    match parsed.get("type").and_then(Value::as_str).unwrap_or("") {
        "item.completed" => {
            let Some(item) = parsed.get("item") else {
                return ExecEvent::Other;
            };
            if item.get("type").and_then(Value::as_str) != Some("agent_message") {
                return ExecEvent::Other;
            }
            match item.get("text").and_then(Value::as_str) {
                Some(text) if !text.is_empty() => ExecEvent::AgentMessage(text.to_string()),
                _ => ExecEvent::Other,
            }
        }
        "turn.failed" => ExecEvent::Failed(
            parsed
                .pointer("/error/message")
                .and_then(Value::as_str)
                .unwrap_or("Codex turn failed")
                .to_string(),
        ),
        other => {
            debug!("Codex CLI event '{}': skipped", other);
            ExecEvent::Other
        }
    }
}

/// A `codex` command rooted in the home directory, the way `claude_command`
/// roots `claude`: a GUI app starts in `/`, and a cwd is the first thing a
/// coding agent reads.
pub(crate) fn codex_command(binary: impl AsRef<std::ffi::OsStr>) -> tokio::process::Command {
    let mut command = tokio::process::Command::new(binary);
    if let Some(home) = dirs::home_dir() {
        command.current_dir(home);
    }
    command
}

#[async_trait]
impl AgentBrain for CodexCliBrain {
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

    #[test]
    fn chatgpt_auth_mode_counts_as_signed_in() {
        let status = CliStatus {
            installed: true,
            auth_mode: AuthMode::ChatGPT,
            email: Some("user@example.com".to_string()),
        };
        assert!(status.is_signed_in());
        assert!(status.could_run());
    }

    #[test]
    fn apikey_auth_is_not_the_login_this_provider_is_for() {
        // The OpenAI provider owns that path. A `codex login` with an API key
        // must not look like a ChatGPT-plan credential, or the default-provider
        // rule would move someone onto this provider when the OpenAI provider
        // was already going to run with the same key.
        let status = CliStatus {
            installed: true,
            auth_mode: AuthMode::ApiKey,
            email: None,
        };
        assert!(!status.is_signed_in());
        // Still "could run": the UI should not grey it out, because the CLI
        // can answer with that key; it just is not the default-rule case.
        assert!(status.could_run());
    }

    #[test]
    fn signed_out_is_a_known_bad_login() {
        let status = CliStatus {
            installed: true,
            auth_mode: AuthMode::SignedOut,
            email: None,
        };
        assert!(!status.is_signed_in());
        assert!(!status.could_run());
    }

    #[test]
    fn unknown_leaves_the_ui_alone() {
        // Unreadable auth file. Not "signed out"; we don't know. The UI
        // reads this as runnable so the person is not greyed out mid-query.
        let status = CliStatus {
            installed: true,
            auth_mode: AuthMode::Unknown,
            email: None,
        };
        assert!(!status.is_signed_in());
        assert!(status.could_run());
    }

    #[test]
    fn email_decodes_from_the_id_token() {
        // Minimal JWT: header.payload.sig. Only the payload is read.
        use base64::Engine;
        let payload = serde_json::json!({ "email": "demo@example.com" });
        let payload_b64 = base64::engine::general_purpose::URL_SAFE_NO_PAD
            .encode(serde_json::to_vec(&payload).unwrap());
        let token = format!("header.{}.sig", payload_b64);
        assert_eq!(
            email_from_id_token(&token).as_deref(),
            Some("demo@example.com")
        );
    }

    #[test]
    fn email_is_absent_when_the_token_is_junk() {
        assert_eq!(email_from_id_token("not a jwt"), None);
    }

    #[test]
    fn exec_runs_outside_a_git_repo_and_leaves_no_history() {
        let brain = CodexCliBrain {
            binary_path: PathBuf::from("/usr/bin/false"),
            model: "gpt-6-luna".to_string(),
            system_prompt: None,
        };
        let args = brain.build_args("hi", None);
        assert!(args.contains(&"--skip-git-repo-check".to_string()));
        assert!(args.contains(&"--ephemeral".to_string()));
        assert_eq!(args.last().map(String::as_str), Some("hi"));
    }

    #[test]
    fn the_fallback_gets_the_computer_tool_too() {
        let brain = CodexCliBrain {
            binary_path: PathBuf::from("/usr/bin/false"),
            model: "gpt-6-luna".to_string(),
            system_prompt: Some("You are Juno.".to_string()),
        };
        let endpoint = juno_mcp::Endpoint {
            url: "http://127.0.0.1:51234/mcp".to_string(),
            token: "secret".to_string(),
        };
        let args = brain.build_args("hi", Some(&endpoint));
        let joined = args.join(" ");
        assert!(joined.contains("mcp_servers.juno.url"));
        assert!(!joined.contains("secret"), "the token never reaches argv");
        let last = args.last().cloned().unwrap_or_default();
        assert!(last.starts_with("System: You are Juno."));
        assert!(last.contains("`computer`"), "steered toward Juno's tool");
        assert!(last.ends_with("User: hi"));
    }

    #[test]
    fn exec_lines_parse_to_messages_and_failures() {
        assert_eq!(
            parse_exec_line(
                r#"{"type":"item.completed","item":{"id":"item_1","type":"agent_message","text":"<TTS>Hi.</TTS>"}}"#
            ),
            ExecEvent::AgentMessage("<TTS>Hi.</TTS>".to_string())
        );
        assert_eq!(
            parse_exec_line(r#"{"type":"turn.failed","error":{"message":"model not supported"}}"#),
            ExecEvent::Failed("model not supported".to_string())
        );
        assert_eq!(
            parse_exec_line(
                r#"{"type":"item.completed","item":{"type":"reasoning","text":"thinking"}}"#
            ),
            ExecEvent::Other
        );
        assert_eq!(
            parse_exec_line("Reading additional input from stdin..."),
            ExecEvent::Other
        );
    }

    #[test]
    fn last_known_auth_mode_round_trips() {
        remember_auth(AuthMode::ChatGPT);
        assert_eq!(last_known_auth_mode(), AuthMode::ChatGPT);
        remember_auth(AuthMode::SignedOut);
        assert_eq!(last_known_auth_mode(), AuthMode::SignedOut);
        remember_auth(AuthMode::ApiKey);
        assert_eq!(last_known_auth_mode(), AuthMode::ApiKey);
    }
}

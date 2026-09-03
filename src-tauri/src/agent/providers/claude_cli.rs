//! Claude CLI provider — uses the `claude` binary (Claude Code) as a subprocess-based
//! AI provider. This allows users with a Claude Max/Pro subscription to use Juno
//! without a direct Anthropic API key.
//!
//! The Claude CLI handles its own agent loop (with Bash, Read, Edit tools), so this
//! provider spawns a subprocess per query and streams the response back to the UI
//! via the same Tauri events as the Anthropic provider.
//!
//! Desktop automation (mouse, keyboard, screenshots) is wired in via the
//! `juno-cua` stdio MCP server when the binary is available (LAC-3696) —
//! see [`detect_juno_cua`] and [`write_mcp_config`].
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

use crate::agent::core::{AgentAction, AgentError, Message, Role, ToolDefinition};
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

/// Detect the `juno-cua` binary — Juno's headless computer-use CLI, which
/// doubles as a stdio MCP server (`juno-cua serve-mcp`).
///
/// Checked in order:
/// 1. Next to the current executable — covers dev builds (the cargo workspace
///    puts `juno-cua` in the same `target/` dir as the app) and a future
///    bundled `externalBin` (Tauri places sidecars next to the main binary).
/// 2. Common install locations (npm/Homebrew distribute it).
/// 3. Manual PATH scan.
///
/// Returns `None` when not found — the provider degrades gracefully to the
/// CLI's built-in tools.
pub fn detect_juno_cua() -> Option<PathBuf> {
    if let Ok(exe) = std::env::current_exe() {
        if let Some(dir) = exe.parent() {
            let candidate = dir.join("juno-cua");
            if candidate.is_file() {
                info!("Found juno-cua next to app binary: {}", candidate.display());
                return Some(candidate);
            }
        }
    }

    let candidates = [
        dirs::home_dir().map(|h| h.join(".local/bin/juno-cua")),
        Some(PathBuf::from("/opt/homebrew/bin/juno-cua")),
        Some(PathBuf::from("/usr/local/bin/juno-cua")),
    ];
    for candidate in candidates.into_iter().flatten() {
        if candidate.is_file() {
            info!("Found juno-cua at: {}", candidate.display());
            return Some(candidate);
        }
    }

    if let Ok(path_var) = std::env::var("PATH") {
        for dir in std::env::split_paths(&path_var) {
            let candidate = dir.join("juno-cua");
            if candidate.is_file() {
                info!("Found juno-cua via PATH: {}", candidate.display());
                return Some(candidate);
            }
        }
    }

    None
}

/// Write the MCP config JSON pointing the Claude CLI at the juno-cua stdio
/// MCP server. Combined with `--strict-mcp-config`, this makes juno-cua the
/// ONLY MCP server the CLI loads (user-level servers stay disabled).
///
/// The file is pid-scoped in the temp dir so concurrent Juno instances
/// (multi-instance dev) can't clobber each other's configs.
fn write_mcp_config(cua_path: &std::path::Path) -> Result<PathBuf, AgentError> {
    let config = serde_json::json!({
        "mcpServers": {
            "juno-cua": {
                "command": cua_path.to_string_lossy(),
                "args": ["serve-mcp"]
            }
        }
    });
    let path = std::env::temp_dir().join(format!("juno-cua-mcp-{}.json", std::process::id()));
    let bytes = serde_json::to_vec(&config).map_err(|e| {
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

/// System-prompt guidance appended when the juno-cua MCP server is wired in.
/// Without this, models tend to fall back to `cliclick`/`screencapture` via
/// Bash even when MCP computer-use tools are available (LAC-3692).
const MCP_TOOL_GUIDANCE: &str = "You have desktop automation tools from the \"juno-cua\" MCP \
server (screenshot capture, mouse movement, clicking, typing, scrolling, key presses, \
clipboard, and accessibility queries). For ANY desktop or GUI automation — moving the mouse, \
clicking, taking screenshots, typing into apps — use these MCP tools. Do NOT use shell \
commands like cliclick, screencapture, or osascript for desktop automation.";

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
                Ok(())
            } else {
                Err(AgentError::ConfigurationError(
                    "Claude CLI is not logged in. Run `claude login` to authenticate.".to_string(),
                ))
            }
        }
        Err(_) => {
            // If we can't parse JSON but the command succeeded, assume OK
            warn!("Could not parse claude auth status output, assuming authenticated");
            AUTH_VERIFIED.store(true, Ordering::Relaxed);
            Ok(())
        }
    }
}

/// Claude CLI-based AgentBrain implementation.
///
/// Spawns `claude -p --output-format=stream-json` as a subprocess for each query,
/// streaming the response back through Tauri events. The CLI handles its own tool
/// execution: its built-in tools (Bash, Read, Edit, etc.) plus Juno's computer-use
/// tools exposed through the juno-cua MCP server when installed (LAC-3696).
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
    /// Path to the MCP config JSON wiring in juno-cua's computer-use tools.
    /// `None` when juno-cua isn't installed — the CLI then runs with its
    /// built-in tools only (LAC-3696).
    mcp_config_path: Option<PathBuf>,
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

        // Wire in Juno's computer-use tools via the juno-cua MCP server when
        // available. Failure to set this up is non-fatal — the provider still
        // works with the CLI's built-in tools (LAC-3696).
        let mcp_config_path = match detect_juno_cua() {
            Some(cua_path) => match write_mcp_config(&cua_path) {
                Ok(path) => Some(path),
                Err(e) => {
                    warn!(
                        "Found juno-cua but failed to write MCP config ({}); \
                         computer-use tools unavailable for Claude CLI provider",
                        e
                    );
                    None
                }
            },
            None => {
                info!(
                    "juno-cua not found — Claude CLI provider will run without \
                     computer-use MCP tools (install juno-cua to enable mouse/screen control)"
                );
                None
            }
        };

        info!(
            "Initializing Claude CLI brain (binary: {}, model: {}, computer-use MCP: {})",
            binary_path.display(),
            model,
            mcp_config_path.is_some()
        );

        Ok(Self {
            binary_path,
            model,
            system_prompt: config.system_prompt.clone(),
            mcp_config_path,
        })
    }

    /// Build the subprocess command arguments.
    fn build_args(&self, query: &str) -> Vec<String> {
        let mut args = vec![
            "-p".to_string(),
            "--output-format".to_string(),
            "stream-json".to_string(),
            "--model".to_string(),
            self.model.clone(),
            // --strict-mcp-config limits MCP servers to exactly what we pass
            // via --mcp-config (or none) — user-level servers never load.
            // We can't use --bare because it blocks OAuth/keychain auth.
            "--strict-mcp-config".to_string(),
            // In -p mode with stdin null, the CLI can't prompt for permission.
            // Allow tool execution since the user explicitly chose this provider.
            // Note: this also lets juno-cua MCP tools run without prompting —
            // matching this provider's existing trust model.
            "--dangerously-skip-permissions".to_string(),
        ];

        if let Some(ref mcp_path) = self.mcp_config_path {
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

        let args = self.build_args(query);

        info!(
            "Spawning Claude CLI: {} {}",
            self.binary_path.display(),
            args.iter().take(6).cloned().collect::<Vec<_>>().join(" ")
        );

        let mut child = tokio::process::Command::new(&self.binary_path)
            .args(&args)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .stdin(Stdio::null())
            .kill_on_drop(true)
            .spawn()
            .map_err(|e| AgentError::LlmError(format!("Failed to spawn Claude CLI: {}", e)))?;

        let stdout = child.stdout.take().ok_or_else(|| {
            AgentError::LlmError("Failed to capture Claude CLI stdout".to_string())
        })?;

        // Drain stderr concurrently to prevent pipe buffer deadlocks.
        // If the child writes >64KB to stderr while we only read stdout,
        // both processes would block forever.
        let stderr = child.stderr.take();
        let stderr_handle = stderr.map(|se| {
            tauri::async_runtime::spawn(async move {
                let mut buf = String::new();
                let mut reader = BufReader::new(se);
                let _ = tokio::io::AsyncReadExt::read_to_string(&mut reader, &mut buf).await;
                buf
            })
        });

        let msg_id = message_id.unwrap_or_else(|| uuid::Uuid::new_v4().to_string());

        // Emit stream start
        if let Some(ref handle) = app_handle {
            crate::agent::tool_logger::emit_stream_start(handle, msg_id.clone());
        }

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

        // Run the streaming loop with a timeout, cancellable via escape key.
        // tokio::select! races the stream against the cancellation signal —
        // whichever completes first wins, and the other branch is dropped.
        let stream_future = tokio::time::timeout(CLI_TIMEOUT, async {
            self.process_stream(stdout, &app_handle, &msg_id).await
        });

        let (accumulated_text, final_result) = if let Some(mut rx) = cancel_rx {
            tokio::select! {
                result = stream_future => {
                    match result {
                        Ok(Ok(r)) => r,
                        Ok(Err(e)) => {
                            if let Some(ref handle) = app_handle {
                                crate::agent::tool_logger::emit_stream_end(
                                    handle,
                                    msg_id,
                                    format!("Error: {}", e),
                                );
                            }
                            return Err(e);
                        }
                        Err(_elapsed) => {
                            if let Some(ref handle) = app_handle {
                                crate::agent::tool_logger::emit_stream_end(
                                    handle,
                                    msg_id,
                                    "Claude CLI timed out".to_string(),
                                );
                            }
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
                    if let Some(ref handle) = app_handle {
                        crate::agent::tool_logger::emit_stream_end(
                            handle,
                            msg_id,
                            "Cancelled".to_string(),
                        );
                    }
                    return Err(AgentError::Terminated);
                }
            }
        } else {
            // No AppState available (headless/test) — fall back to timeout-only
            match stream_future.await {
                Ok(Ok(r)) => r,
                Ok(Err(e)) => {
                    if let Some(ref handle) = app_handle {
                        crate::agent::tool_logger::emit_stream_end(
                            handle,
                            msg_id,
                            format!("Error: {}", e),
                        );
                    }
                    return Err(e);
                }
                Err(_elapsed) => {
                    if let Some(ref handle) = app_handle {
                        crate::agent::tool_logger::emit_stream_end(
                            handle,
                            msg_id,
                            "Claude CLI timed out".to_string(),
                        );
                    }
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
        if post_cancel_rx.map(|rx| *rx.borrow()).unwrap_or(false) {
            info!("Claude CLI cancelled after stream completion, discarding response");
            let _ = child.kill().await;
            if let Some(ref handle) = app_handle {
                crate::agent::tool_logger::emit_stream_end(handle, msg_id, "Cancelled".to_string());
            }
            return Err(AgentError::Terminated);
        }

        // Wait for subprocess to finish
        let status = child
            .wait()
            .await
            .map_err(|e| AgentError::LlmError(format!("Claude CLI process error: {}", e)))?;

        // Collect stderr output — log on both success and failure for debuggability
        if let Some(handle) = stderr_handle {
            match handle.await {
                Ok(stderr_buf) if !stderr_buf.is_empty() => {
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
                Err(e) => error!("Failed to read Claude CLI stderr: {}", e),
                _ => {}
            }
        }

        // Use final_result if available, otherwise accumulated_text
        let complete_text = final_result.unwrap_or(accumulated_text);

        // Emit stream end
        if let Some(ref handle) = app_handle {
            crate::agent::tool_logger::emit_stream_end(handle, msg_id, complete_text.clone());
        }

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

        let mut accumulated_text = String::new();
        let mut previous_char_count: usize = 0;
        let mut final_result: Option<String> = None;

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

                    match event_type {
                        "assistant" => {
                            // Extract text content from the assistant message
                            if let Some(message) = parsed.get("message") {
                                let text = extract_text_from_message(message);
                                let char_count = text.chars().count();

                                if char_count > previous_char_count {
                                    // Compute delta using char offsets (safe for multi-byte UTF-8)
                                    let delta: String =
                                        text.chars().skip(previous_char_count).collect();

                                    if !delta.is_empty() {
                                        if let Some(ref handle) = app_handle {
                                            crate::agent::tool_logger::emit_streaming_text_chunk(
                                                handle,
                                                delta,
                                                Some(msg_id.to_string()),
                                                None,
                                            );
                                        }
                                    }
                                    previous_char_count = char_count;
                                    accumulated_text = text;
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

        Ok((accumulated_text, final_result))
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

/// Extract text content from a Claude CLI assistant message JSON object.
/// Handles both `content` array format and direct `content` string.
fn extract_text_from_message(message: &Value) -> String {
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
            },
            Message {
                role: Role::Assistant,
                content: "response".to_string(),
                tool_calls: None,
                tool_call_id: None,
                name: None,
            },
            Message {
                role: Role::User,
                content: "second question".to_string(),
                tool_calls: None,
                tool_call_id: None,
                name: None,
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
        }];
        assert_eq!(ClaudeCliBrain::extract_query(&messages), "Hello");
    }

    #[test]
    fn test_build_args_basic() {
        let brain = ClaudeCliBrain {
            binary_path: PathBuf::from("/usr/bin/claude"),
            model: "sonnet".to_string(),
            system_prompt: None,
            mcp_config_path: None,
        };
        let args = brain.build_args("test query");
        assert!(args.contains(&"-p".to_string()));
        assert!(args.contains(&"stream-json".to_string()));
        assert!(args.contains(&"sonnet".to_string()));
        assert!(args.contains(&"test query".to_string()));
        // No --system-prompt when None
        assert!(!args.contains(&"--system-prompt".to_string()));
        // No MCP flags when juno-cua isn't wired
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
            mcp_config_path: None,
        };
        let args = brain.build_args("test query");
        assert!(args.contains(&"--system-prompt".to_string()));
        assert!(args.contains(&"You are helpful.".to_string()));
    }

    /// Regression test for LAC-3696: when juno-cua is available, the CLI must
    /// be given our MCP config (computer-use tools) while --strict-mcp-config
    /// still blocks user-level servers, and the model must be steered toward
    /// the MCP tools via --append-system-prompt.
    #[test]
    fn test_build_args_with_mcp_config() {
        let brain = ClaudeCliBrain {
            binary_path: PathBuf::from("/usr/bin/claude"),
            model: "sonnet".to_string(),
            system_prompt: None,
            mcp_config_path: Some(PathBuf::from("/tmp/juno-cua-mcp-test.json")),
        };
        let args = brain.build_args("move the mouse");

        let mcp_flag_idx = args
            .iter()
            .position(|a| a == "--mcp-config")
            .expect("--mcp-config flag present");
        assert_eq!(args[mcp_flag_idx + 1], "/tmp/juno-cua-mcp-test.json");

        // Strict mode must remain — explicit config + strict = ONLY our server
        assert!(args.contains(&"--strict-mcp-config".to_string()));

        // Guidance prompt steers the model away from cliclick/Bash fallbacks
        let guidance_idx = args
            .iter()
            .position(|a| a == "--append-system-prompt")
            .expect("--append-system-prompt flag present");
        assert!(args[guidance_idx + 1].contains("juno-cua"));
    }

    /// The MCP config file must contain the serve-mcp invocation for the
    /// detected binary, keyed under mcpServers.juno-cua (the server name
    /// becomes the mcp__juno-cua__* tool prefix).
    #[test]
    fn test_write_mcp_config_shape() {
        let cua_path = PathBuf::from("/opt/homebrew/bin/juno-cua");
        let config_path = write_mcp_config(&cua_path).expect("write mcp config");

        let content = std::fs::read_to_string(&config_path).expect("read mcp config");
        let parsed: Value = serde_json::from_str(&content).expect("valid JSON");

        let server = &parsed["mcpServers"]["juno-cua"];
        assert_eq!(server["command"], "/opt/homebrew/bin/juno-cua");
        assert_eq!(server["args"], serde_json::json!(["serve-mcp"]));

        let _ = std::fs::remove_file(&config_path);
    }

    #[test]
    fn test_detect_juno_cua_does_not_panic() {
        // Environment-dependent — on machines without juno-cua this is None
        let result = detect_juno_cua();
        if let Some(path) = result {
            assert!(path.is_file());
        }
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
            mcp_config_path: None,
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

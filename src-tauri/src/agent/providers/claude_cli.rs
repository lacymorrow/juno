//! Claude CLI provider — uses the `claude` binary (Claude Code) as a subprocess-based
//! AI provider. This allows users with a Claude Max/Pro subscription to use Juno
//! without a direct Anthropic API key.
//!
//! The Claude CLI handles its own agent loop (with Bash, Read, Edit tools), so this
//! provider spawns a subprocess per query and streams the response back to the UI
//! via the same Tauri events as the Anthropic provider.
//!
//! NOTE: This provider is macOS-only (matching Juno's platform target). The binary
//! detection paths are Unix-specific.

use async_trait::async_trait;
use serde_json::Value;
use std::path::PathBuf;
use std::process::Stdio;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::time::Duration;
use tracing::{debug, error, info, warn};

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

    // Fall back to PATH lookup via `which` (blocking, but only runs if
    // all static paths miss — acceptable for a one-time detection)
    match std::process::Command::new("which")
        .arg("claude")
        .output()
    {
        Ok(output) if output.status.success() => {
            let path_str = String::from_utf8_lossy(&output.stdout).trim().to_string();
            if !path_str.is_empty() {
                let path = PathBuf::from(&path_str);
                info!("Found Claude CLI via PATH: {}", path.display());
                return Ok(path);
            }
        }
        _ => {}
    }

    Err(AgentError::ConfigurationError(
        "Claude CLI (claude) not found. Install it from https://claude.ai/code".to_string(),
    ))
}

/// Quick check: is the Claude CLI binary available? (No auth check — just existence.)
pub fn is_claude_cli_available() -> bool {
    detect_claude_cli().is_ok()
}

/// Check Claude CLI authentication status by running `claude auth status`.
/// Returns Ok(()) if authenticated, or an error with details.
async fn check_auth_status(binary_path: &PathBuf) -> Result<(), AgentError> {
    let output = tokio::process::Command::new(binary_path)
        .args(["auth", "status", "--output", "json"])
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
            Ok(())
        }
    }
}

/// Claude CLI-based AgentBrain implementation.
///
/// Spawns `claude -p --output-format=stream-json` as a subprocess for each query,
/// streaming the response back through Tauri events. The CLI handles its own tool
/// execution (Bash, Read, Edit, etc.) so Juno doesn't need to provide tools.
///
/// Note: The user cannot cancel a running Claude CLI query via Juno's escape key —
/// the subprocess runs to completion or until the timeout (300s). The `kill_on_drop`
/// flag ensures cleanup if the `Child` handle is dropped.
pub struct ClaudeCliBrain {
    binary_path: PathBuf,
    model: String,
    system_prompt: Option<String>,
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

        info!(
            "Initializing Claude CLI brain (binary: {}, model: {})",
            binary_path.display(),
            model
        );

        Ok(Self {
            binary_path,
            model,
            system_prompt: config.system_prompt.clone(),
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
            "--verbose".to_string(),
            "false".to_string(),
        ];

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
    ) -> Result<String, AgentError> {
        // Validate auth before spawning the query subprocess
        check_auth_status(&self.binary_path).await?;

        let args = self.build_args(query);

        info!(
            "Spawning Claude CLI: {} {}",
            self.binary_path.display(),
            args.iter()
                .take(6)
                .cloned()
                .collect::<Vec<_>>()
                .join(" ")
        );

        let mut child = tokio::process::Command::new(&self.binary_path)
            .args(&args)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .stdin(Stdio::null())
            .kill_on_drop(true)
            .spawn()
            .map_err(|e| {
                AgentError::LlmError(format!("Failed to spawn Claude CLI: {}", e))
            })?;

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

        // Run the streaming loop with a timeout
        let streaming_result = tokio::time::timeout(CLI_TIMEOUT, async {
            self.process_stream(stdout, &app_handle, &msg_id).await
        })
        .await;

        let (accumulated_text, final_result) = match streaming_result {
            Ok(Ok(result)) => result,
            Ok(Err(e)) => {
                // Emit stream end with error before returning
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
                // Timeout — child is killed via kill_on_drop when dropped
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
        };

        // Wait for subprocess to finish
        let status = child.wait().await.map_err(|e| {
            AgentError::LlmError(format!("Claude CLI process error: {}", e))
        })?;

        // Collect stderr output
        if let Some(handle) = stderr_handle {
            match handle.await {
                Ok(stderr_buf) if !stderr_buf.is_empty() && !status.success() => {
                    error!(
                        "Claude CLI stderr: {}",
                        stderr_buf.chars().take(500).collect::<String>()
                    );
                }
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

                    let event_type = parsed
                        .get("type")
                        .and_then(|v| v.as_str())
                        .unwrap_or("");

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
                            if let Some(result_text) =
                                parsed.get("result").and_then(|v| v.as_str())
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
        let result = self.run_streaming(&query, None, None).await?;
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
    ) -> Result<AgentAction, AgentError> {
        let query = Self::extract_query(messages);
        let result = self
            .run_streaming(&query, app_handle, message_id)
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
        };
        let args = brain.build_args("test query");
        assert!(args.contains(&"-p".to_string()));
        assert!(args.contains(&"stream-json".to_string()));
        assert!(args.contains(&"sonnet".to_string()));
        assert!(args.contains(&"test query".to_string()));
        // No --system-prompt when None
        assert!(!args.contains(&"--system-prompt".to_string()));
    }

    #[test]
    fn test_build_args_with_system_prompt() {
        let brain = ClaudeCliBrain {
            binary_path: PathBuf::from("/usr/bin/claude"),
            model: "opus".to_string(),
            system_prompt: Some("You are helpful.".to_string()),
        };
        let args = brain.build_args("test query");
        assert!(args.contains(&"--system-prompt".to_string()));
        assert!(args.contains(&"You are helpful.".to_string()));
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

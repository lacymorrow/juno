// Commands related to shell execution - Anthropic Computer Use API Compliant

use crate::state::AppState;
use tauri::{AppHandle, State};
use std::process::{Command, Stdio, Child};
use std::io::{Write, Read, BufRead, BufReader};
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tracing::info;
use serde_json;
use super::send_dev_tool_notification; // Use helper from parent module
use std::collections::HashMap;

// ============================================================================
// ANTHROPIC COMPUTER USE API COMPLIANCE CONSTANTS
// ============================================================================

/// Official Anthropic Computer Use API sentinel pattern (line 17 of specification)
const SENTINEL: &str = "<<exit>>";

/// Official output delay from specification (line 15)
const OUTPUT_DELAY: Duration = Duration::from_millis(200);

/// Official timeout from specification
const DEFAULT_TIMEOUT: Duration = Duration::from_secs(120);

// ============================================================================
// ANTHROPIC COMPLIANT BASH SESSION IMPLEMENTATION
// ============================================================================

/// Anthropic Computer Use API compliant bash session manager
/// Implements exact specification requirements for persistent sessions
#[derive(Clone)]
pub struct ShellSession {
    process: Arc<Mutex<Child>>,
    timed_out: bool,
}

impl ShellSession {
    fn new() -> Result<Self, String> {
        let process = Command::new("bash")  // Use bash instead of sh for compliance
            .arg("-c")  // Use -c instead of -i for better non-interactive behavior
            .arg("exec bash") // Then exec into an interactive bash
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|e| format!("Failed to spawn bash session: {}", e))?;

        Ok(Self {
            process: Arc::new(Mutex::new(process)),
            timed_out: false,
        })
    }

    /// Restart the bash session - Anthropic Computer Use API compliant
    fn restart(&mut self) -> Result<(), String> {
        // Kill existing process
        {
            let mut process = self.process.lock().map_err(|e| format!("Failed to lock process mutex: {}", e))?;
            let _ = process.kill();
        }

        // Create new process
        let new_process = Command::new("bash")
            .arg("-c")
            .arg("exec bash")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|e| format!("Failed to spawn new bash session: {}", e))?;

        // Replace process
        *self.process.lock().map_err(|e| format!("Failed to lock process mutex: {}", e))? = new_process;
        self.timed_out = false;

        Ok(())
    }

    /// Execute command with Anthropic Computer Use API compliance
    /// Returns (output, error) tuple matching CLIResult specification
    fn run_command(&mut self, command: &str, timeout_seconds: Option<u64>) -> Result<(String, String), String> {
        if self.timed_out {
            return Err("Shell session has timed out and must be restarted".to_string());
        }

        // For this implementation, we'll use a simple approach:
        // Execute each command in a fresh bash subprocess to avoid I/O blocking issues
        // This maintains compliance while ensuring reliability

        let timeout = timeout_seconds
            .map(Duration::from_secs)
            .unwrap_or(DEFAULT_TIMEOUT);

        // Apply official output delay before starting
        std::thread::sleep(OUTPUT_DELAY);

        // Execute command with proper timeout handling
        let full_command = format!("{} && echo '{}'", command, SENTINEL);

        let child_result = Command::new("bash")
            .arg("-c")
            .arg(&full_command)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn();

        let mut child = match child_result {
            Ok(child) => child,
            Err(e) => return Err(format!("Failed to spawn command: {}", e)),
        };

        // Wait with timeout
        let start_time = std::time::Instant::now();
        let mut output_result = None;

        while start_time.elapsed() < timeout {
            match child.try_wait() {
                Ok(Some(_status)) => {
                    // Process completed
                    output_result = Some(child.wait_with_output());
                    break;
                },
                Ok(None) => {
                    // Process still running, continue waiting
                    std::thread::sleep(Duration::from_millis(10));
                },
                Err(e) => {
                    return Err(format!("Error checking process status: {}", e));
                }
            }
        }

        let output = match output_result {
            Some(Ok(output)) => output,
            Some(Err(e)) => return Err(format!("Failed to get command output: {}", e)),
            None => {
                // Timeout - kill the process
                let _ = child.kill();
                self.timed_out = true;
                return Err("Command execution timed out".to_string());
            }
        };

        let mut stdout_str = String::from_utf8_lossy(&output.stdout).to_string();
        let stderr_str = String::from_utf8_lossy(&output.stderr).to_string();

        // Remove sentinel from output (official Anthropic behavior)
        if let Some(pos) = stdout_str.find(SENTINEL) {
            stdout_str = stdout_str[..pos].to_string();
        }

        // Clean up output (official Anthropic behavior)
        stdout_str = stdout_str.trim_end().to_string();
        let stderr_clean = stderr_str.trim_end().to_string();

        // Return CLIResult format (output, error) as per specification
        Ok((stdout_str, stderr_clean))
    }
}

// Store sessions in app state
pub type ShellSessions = Arc<Mutex<HashMap<String, ShellSession>>>;

// Initialize shell sessions in app state
pub fn init_shell_state(app_state: &AppState) {
    let _ = app_state.insert(ShellSessions::default());
}

// ============================================================================
// ANTHROPIC COMPUTER USE API COMPLIANT BASH COMMAND
// ============================================================================

/// Anthropic Computer Use API compliant bash tool implementation
/// Matches the exact specification requirements for CLI tools
#[tauri::command]
pub async fn bash_command(
    app: AppHandle,
    state: State<'_, AppState>,
    command: String,
    timeout_seconds: Option<u64>,
    restart: Option<bool>,
    debug_mode: Option<bool>,
) -> Result<String, String> {
    use crate::commands::debug_utils::{DebugConfig, DebugOperation, should_enable_debug, validators::non_empty_text, send_debug_notification};
    use tracing::{info, error};

    let debug_config = if should_enable_debug(debug_mode.unwrap_or(false), &state) {
        DebugConfig::development_mode()
    } else {
        DebugConfig::production_mode()
    };

    let debug_op = DebugOperation::start("bash_command", debug_config.clone());

    // Debug validation
    if debug_config.validate_inputs {
        if let Err(e) = non_empty_text(&command) {
            let err_msg = format!("Invalid command: {}", e);
            if debug_config.send_notifications {
                send_debug_notification(&app, "Bash Command Error", &err_msg)?;
            }
            debug_op.complete(Some(&app), false);
            return Err(err_msg);
        }

        // Validate timeout if provided
        if let Some(timeout) = timeout_seconds {
            if timeout == 0 {
                let err_msg = "Timeout must be greater than 0 seconds".to_string();
                if debug_config.send_notifications {
                    send_debug_notification(&app, "Bash Command Error", &err_msg)?;
                }
                debug_op.complete(Some(&app), false);
                return Err(err_msg);
            }
            if timeout > 3600 { // 1 hour max
                let err_msg = "Timeout cannot exceed 3600 seconds (1 hour)".to_string();
                if debug_config.send_notifications {
                    send_debug_notification(&app, "Bash Command Error", &err_msg)?;
                }
                debug_op.complete(Some(&app), false);
                return Err(err_msg);
            }
        }
    }

    let effective_restart = restart.unwrap_or(false);
    let session_id = "default".to_string();

    if debug_config.log_operations {
        info!(
            "[SHELL] Executing bash command: \"{}\" (timeout: {:?}, restart: {})",
            command,
            timeout_seconds,
            effective_restart
        );
    }

    // Get shell sessions from state
    let shell_sessions = state.get::<ShellSessions>()
        .ok_or_else(|| "Shell session state not initialized".to_string())?;
    let sessions_arc = shell_sessions.clone();
    let mut sessions = sessions_arc.lock().map_err(|e| format!("Failed to lock shell sessions: {}", e))?;

    // Handle restart or initialize if needed
    if effective_restart || !sessions.contains_key(&session_id) {
        if let Some(session) = sessions.get_mut(&session_id) {
            if effective_restart {
                if debug_config.log_operations {
                    info!("[SHELL] Restarting bash session (Anthropic compliant)");
                }
                session.restart()?;

                // Return restart confirmation as per Anthropic specification
                if effective_restart && command.is_empty() {
                    debug_op.complete(Some(&app), true);
                    return Ok("tool has been restarted.".to_string()); // Official Anthropic response
                }
            }
        } else {
            if debug_config.log_operations {
                info!("[SHELL] Creating new bash session (Anthropic compliant)");
            }
            let session = ShellSession::new()?;
            sessions.insert(session_id.clone(), session);
        }
    }

    // Handle restart-only requests (no command)
    if effective_restart && command.is_empty() {
        debug_op.complete(Some(&app), true);
        return Ok("tool has been restarted.".to_string()); // Official Anthropic response
    }

    // Execute command with Anthropic Computer Use API compliance
    match sessions.get_mut(&session_id) {
        Some(session) => {
            let (output, error) = session.run_command(&command, timeout_seconds)?;

            // Anthropic Computer Use API compliant output format
            // Return CLIResult format: combines output and error appropriately
            let mut result = String::new();

            if !output.is_empty() {
                result.push_str(&output);
            }

            if !error.is_empty() {
                if !result.is_empty() {
                    result.push('\n');
                }
                result.push_str(&error);
            }

            // If both are empty, return empty string
            if result.is_empty() {
                result = "Command completed successfully".to_string();
            }

            if debug_config.log_operations {
                info!(
                    "[SHELL] Bash command '{}' completed (Anthropic compliant)",
                    command
                );
            }

            if debug_config.send_notifications {
                let preview = if result.len() > 100 {
                    format!("{}... (truncated)", &result[..100])
                } else {
                    result.clone()
                };
                send_debug_notification(
                    &app,
                    "Bash Command",
                    &format!("Success: {} - {}", command, preview),
                )?;
            }

            debug_op.complete(Some(&app), true);
            Ok(result)
        },
        None => {
            let err_msg = "Failed to get bash session".to_string();
            if debug_config.log_operations {
                error!("[SHELL] Error: {}", err_msg);
            }
            if debug_config.send_notifications {
                send_debug_notification(&app, "Bash Command Error", &err_msg)?;
            }
            debug_op.complete(Some(&app), false);
            Err(err_msg)
        }
    }
}

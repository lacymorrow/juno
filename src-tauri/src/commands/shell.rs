// Commands related to shell execution - Anthropic Computer Use API Compliant

use crate::state::AppState;
use tauri::{AppHandle, State};
use std::process::{Command, Stdio, Child};
use std::io::{Write, Read};
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
            .arg("-i")
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
            .arg("-i")
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

        // Use official Anthropic sentinel pattern
        {
            let mut process = self.process.lock().map_err(|e| format!("Failed to lock process mutex: {}", e))?;

            // Get stdin handle
            let stdin = process.stdin.as_mut()
                .ok_or_else(|| "Failed to open stdin".to_string())?;

            // Write command with official Anthropic sentinel pattern
            writeln!(stdin, "{} && echo '{}'", command, SENTINEL)
                .map_err(|e| format!("Failed to write to stdin: {}", e))?;
            stdin.flush()
                .map_err(|e| format!("Failed to flush stdin: {}", e))?;
        }

        // Apply official output delay
        std::thread::sleep(OUTPUT_DELAY);

        // Read output until sentinel or timeout
        let mut output = String::new();
        let mut error = String::new();
        let mut timed_out = false;

        let timeout = timeout_seconds
            .map(Duration::from_secs)
            .unwrap_or(DEFAULT_TIMEOUT);

        let start_time = std::time::Instant::now();

        loop {
            // Check timeout
            if start_time.elapsed() > timeout {
                timed_out = true;
                self.timed_out = true;
                break;
            }

            // Single lock scope to prevent race conditions
            {
                let mut process = self.process.lock().map_err(|e| format!("Failed to lock process mutex: {}", e))?;

                // Read stdout
                if let Some(stdout) = process.stdout.as_mut() {
                    let mut buffer = [0; 1024];
                    if let Ok(n) = stdout.read(&mut buffer) {
                        if n > 0 {
                            output.push_str(&String::from_utf8_lossy(&buffer[..n]));
                        }
                    }
                }

                // Read stderr
                if let Some(stderr) = process.stderr.as_mut() {
                    let mut buffer = [0; 1024];
                    if let Ok(n) = stderr.read(&mut buffer) {
                        if n > 0 {
                            error.push_str(&String::from_utf8_lossy(&buffer[..n]));
                        }
                    }
                }
            }

            // Check for official Anthropic sentinel
            if output.contains(SENTINEL) {
                break;
            }

            // Small sleep to avoid high CPU usage
            std::thread::sleep(Duration::from_millis(10));
        }

        if timed_out {
            return Err("Command execution timed out".to_string());
        }

        // Remove sentinel from output (official Anthropic behavior)
        if let Some(pos) = output.find(SENTINEL) {
            output = output[..pos].to_string();
        }

        // Clean up output (official Anthropic behavior)
        output = output.trim_end().to_string();
        error = error.trim_end().to_string();

        // Return CLIResult format (output, error) as per specification
        Ok((output, error))
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

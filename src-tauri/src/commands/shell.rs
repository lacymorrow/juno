// Commands related to shell execution

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

// Shell session manager to maintain persistent sessions
#[derive(Clone)]
pub struct ShellSession {
    process: Arc<Mutex<Child>>,
    timed_out: bool,
}

impl ShellSession {
    fn new() -> Result<Self, String> {
        let process = Command::new("sh")
            .arg("-i")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|e| format!("Failed to spawn shell session: {}", e))?;

        Ok(Self {
            process: Arc::new(Mutex::new(process)),
            timed_out: false,
        })
    }

    fn run_command(&mut self, command: &str, timeout_seconds: Option<u64>) -> Result<(String, String, Option<i32>, bool), String> {
        if self.timed_out {
            return Err("Shell session has timed out and must be restarted".to_string());
        }

        // Write the command with sentinel
        let sentinel = "<<COMMAND_COMPLETE>>";
        {
            let mut process = self.process.lock().map_err(|e| format!("Failed to lock process mutex: {}", e))?;

            // Get stdin handle
            let stdin = process.stdin.as_mut()
                .ok_or_else(|| "Failed to open stdin".to_string())?;

            // Write command with sentinel
            writeln!(stdin, "{} && echo \"{}\"", command, sentinel)
                .map_err(|e| format!("Failed to write to stdin: {}", e))?;
            stdin.flush()
                .map_err(|e| format!("Failed to flush stdin: {}", e))?;
        }

        // Read output until sentinel or timeout
        let mut output = String::new();
        let mut error = String::new();
        let mut timed_out = false;

        if let Some(seconds) = timeout_seconds {
            let timeout = Duration::from_secs(seconds);
            let start_time = std::time::Instant::now();

            loop {
                // Check if we've exceeded timeout
                if start_time.elapsed() > timeout {
                    timed_out = true;
                    self.timed_out = true;
                    break;
                }

                // FIXED: Single lock scope to prevent race conditions between stdout/stderr reads
                // This eliminates the risk of other operations interleaving between reads
                {
                    let mut process = self.process.lock().map_err(|e| format!("Failed to lock process mutex: {}", e))?;

                    // Read both stdout and stderr in single critical section
                    if let Some(stdout) = process.stdout.as_mut() {
                        let mut buffer = [0; 1024];
                        if let Ok(n) = stdout.read(&mut buffer) {
                            if n > 0 {
                                output.push_str(&String::from_utf8_lossy(&buffer[..n]));
                            }
                        }
                    }

                    if let Some(stderr) = process.stderr.as_mut() {
                        let mut buffer = [0; 1024];
                        if let Ok(n) = stderr.read(&mut buffer) {
                            if n > 0 {
                                error.push_str(&String::from_utf8_lossy(&buffer[..n]));
                            }
                        }
                    }
                }

                // Check for sentinel
                if output.contains(sentinel) {
                    break;
                }

                // Small sleep to avoid high CPU usage
                std::thread::sleep(Duration::from_millis(10));
            }
        } else {
            // No timeout, read until sentinel
            loop {
                // FIXED: Single lock scope to prevent race conditions between stdout/stderr reads
                // This eliminates the risk of other operations interleaving between reads
                {
                    let mut process = self.process.lock().map_err(|e| format!("Failed to lock process mutex: {}", e))?;

                    // Read both stdout and stderr in single critical section
                    if let Some(stdout) = process.stdout.as_mut() {
                        let mut buffer = [0; 1024];
                        if let Ok(n) = stdout.read(&mut buffer) {
                            if n > 0 {
                                output.push_str(&String::from_utf8_lossy(&buffer[..n]));
                            }
                        }
                    }

                    if let Some(stderr) = process.stderr.as_mut() {
                        let mut buffer = [0; 1024];
                        if let Ok(n) = stderr.read(&mut buffer) {
                            if n > 0 {
                                error.push_str(&String::from_utf8_lossy(&buffer[..n]));
                            }
                        }
                    }
                }

                if output.contains(sentinel) {
                    break;
                }

                // Small sleep to avoid high CPU usage
                std::thread::sleep(Duration::from_millis(10));
            }
        }

        // Remove sentinel from output
        if let Some(pos) = output.find(sentinel) {
            output = output[..pos].to_string();
        }

        // Trim trailing newlines
        output = output.trim_end().to_string();
        error = error.trim_end().to_string();

        // Check if process is still alive
        let exit_code = {
            let mut process = self.process.lock().map_err(|e| format!("Failed to lock process mutex: {}", e))?;
            match process.try_wait() {
                Ok(Some(status)) => status.code(),
                Ok(None) => None, // Process still running
                Err(e) => return Err(format!("Failed to check process status: {}", e)),
            }
        };

        Ok((output, error, exit_code, timed_out))
    }
}

// Store sessions in app state
pub type ShellSessions = Arc<Mutex<HashMap<String, ShellSession>>>;

// Initialize shell sessions in app state
pub fn init_shell_state(app_state: &AppState) {
    let _ = app_state.insert(ShellSessions::default());
}

// ============================================================================
// CONSOLIDATED: dev_ functions removed - use production functions instead
// ============================================================================

// CONSOLIDATED: dev_bash_command removed - use bash_command production function
// Located in this same file with debug capabilities enabled via debug_mode parameter

// This function replaces the dev_ prefixed function by incorporating debug features conditionally

// ============================================================================
// PRODUCTION SHELL FUNCTION WITH UNIFIED DEBUG SYSTEM
// ============================================================================

/// Production function to execute bash commands with optional debug features
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
        if sessions.contains_key(&session_id) {
            if debug_config.log_operations {
                info!("[SHELL] Restarting shell session");
            }
            let _ = sessions.remove(&session_id);
        } else {
            if debug_config.log_operations {
                info!("[SHELL] Creating new shell session");
            }
        }

        // Create new session
        let session = ShellSession::new()?;
        sessions.insert(session_id.clone(), session);
    }

    // Get the session and run the command
    match sessions.get_mut(&session_id) {
        Some(session) => {
            let (stdout, stderr, exit_code, timed_out) = session.run_command(&command, timeout_seconds)?;

            let success = exit_code.map_or(true, |code| code == 0);

            let result_json = serde_json::json!({
                "success": success,
                "stdout": stdout,
                "stderr": stderr,
                "exit_code": exit_code,
                "timed_out": timed_out
            });

            let result_str = serde_json::to_string(&result_json)
                .map_err(|e| format!("Failed to serialize bash command result: {}", e))?;

            if debug_config.log_operations {
                info!(
                    "[SHELL] Bash command '{}' finished. Success: {}, Timed out: {}",
                    command,
                    success,
                    timed_out
                );
            }

            if debug_config.send_notifications {
                let status = if success { "Success" } else { "Failed" };
                let preview = if stdout.len() > 100 {
                    format!("{}... (truncated)", &stdout[..100])
                } else {
                    stdout.clone()
                };
                send_debug_notification(
                    &app,
                    "Bash Command",
                    &format!("{}: {} - {}", status, command, preview),
                )?;
            }

            debug_op.complete(Some(&app), success);
            Ok(result_str)
        },
        None => {
            let err_msg = "Failed to get shell session".to_string();
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

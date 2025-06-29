// Commands related to shell execution - Anthropic Computer Use API Compliant

use crate::state::AppState;
use tauri::{AppHandle, State};
use std::process::{Command, Stdio, Child};
use std::io::{Write, BufRead, BufReader};
use std::sync::{Arc, Mutex, mpsc};
use std::time::Duration;
use std::thread;
use tracing::info;
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
/// Fixed: Uses session directory and script files for reliable I/O
#[derive(Clone)]
pub struct ShellSession {
    session_dir: std::path::PathBuf,
    session_id: String,
    timed_out: bool,
}

impl ShellSession {
    fn new() -> Result<Self, String> {
        use std::fs;

        // Create a unique session ID and directory
        let session_id = format!("bash_session_{}", std::process::id());
        let session_dir = std::env::temp_dir().join(format!("juno_shell_{}", session_id));

        // Create session directory
        fs::create_dir_all(&session_dir)
            .map_err(|e| format!("Failed to create session directory: {}", e))?;

        // Create session state files
        let bashrc_path = session_dir.join(".bashrc");
        let history_path = session_dir.join(".bash_history");
        let env_path = session_dir.join(".env");

        // Initialize session state files
        fs::write(&bashrc_path, "# Juno shell session\nexport HISTFILE=~/.bash_history\nset +H\n")
            .map_err(|e| format!("Failed to create session bashrc: {}", e))?;
        fs::write(&history_path, "")
            .map_err(|e| format!("Failed to create session history: {}", e))?;
        fs::write(&env_path, "")
            .map_err(|e| format!("Failed to create session env: {}", e))?;

        Ok(Self {
            session_dir,
            session_id,
            timed_out: false,
        })
    }

    /// Restart the bash session - Anthropic Computer Use API compliant
    fn restart(&mut self) -> Result<(), String> {
        use std::fs;

        // Clear session state files
        let history_path = self.session_dir.join(".bash_history");
        let env_path = self.session_dir.join(".env");

        fs::write(&history_path, "")
            .map_err(|e| format!("Failed to clear session history: {}", e))?;
        fs::write(&env_path, "")
            .map_err(|e| format!("Failed to clear session env: {}", e))?;

        self.timed_out = false;
        Ok(())
    }

        /// Execute command with Anthropic Computer Use API compliance
    /// Returns (output, error) tuple matching CLIResult specification
    /// Fixed: Uses session-based execution with proper non-blocking I/O
    fn run_command(&mut self, command: &str, timeout_seconds: Option<u64>) -> Result<(String, String), String> {
        use std::fs;

        if self.timed_out {
            return Err("Shell session has timed out and must be restarted".to_string());
        }

        let timeout = timeout_seconds
            .map(Duration::from_secs)
            .unwrap_or(DEFAULT_TIMEOUT);

        // Apply official output delay before starting
        std::thread::sleep(OUTPUT_DELAY);

        // Create session script with state persistence
        let script_path = self.session_dir.join("command.sh");
        let history_path = self.session_dir.join(".bash_history");
        let env_path = self.session_dir.join(".env");

        // Build script that maintains session state
        let script_content = format!(
            r#"#!/bin/bash
set +H  # Disable history expansion
cd "{session_dir}"
export HISTFILE="{history_path}"
export HOME="{session_dir}"

# Source any previous environment variables
if [ -f "{env_path}" ]; then
    source "{env_path}"
fi

# Execute the command and capture environment changes
({command}) && echo '{sentinel}' || echo '{sentinel}'

# Save environment variables for next command
declare -x | grep -v '^declare -x _' > "{env_path}"
"#,
            session_dir = self.session_dir.display(),
            history_path = history_path.display(),
            env_path = env_path.display(),
            command = command,
            sentinel = SENTINEL
        );

        // Write script content
        fs::write(&script_path, script_content)
            .map_err(|e| format!("Failed to write session script: {}", e))?;

        // Execute script with proper I/O handling (no need to make executable)
        let mut child = Command::new("bash")
            .arg(&script_path)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .current_dir(&self.session_dir)
            .spawn()
            .map_err(|e| format!("Failed to spawn command process: {}", e))?;

        // Take handles for thread-based reading
        let stdout = child.stdout.take()
            .ok_or_else(|| "Failed to take stdout handle".to_string())?;
        let stderr = child.stderr.take()
            .ok_or_else(|| "Failed to take stderr handle".to_string())?;

        // Create channels for non-blocking communication
        let (stdout_tx, stdout_rx) = mpsc::channel();
        let (stderr_tx, stderr_rx) = mpsc::channel();
        let (done_tx, done_rx) = mpsc::channel();

        // Spawn stdout reading thread
        let stdout_thread = {
            let stdout_tx = stdout_tx.clone();
            let done_tx = done_tx.clone();
            thread::spawn(move || {
                let mut reader = BufReader::new(stdout);
                let mut buffer = String::new();

                loop {
                    buffer.clear();
                    match reader.read_line(&mut buffer) {
                        Ok(0) => {
                            // EOF reached
                            let _ = done_tx.send(());
                            break;
                        },
                        Ok(_) => {
                            if stdout_tx.send(buffer.clone()).is_err() {
                                break; // Main thread dropped receiver
                            }
                        },
                        Err(_) => {
                            // Error reading
                            let _ = done_tx.send(());
                            break;
                        }
                    }
                }
            })
        };

        // Spawn stderr reading thread
        let stderr_thread = {
            let done_tx = done_tx.clone();
            thread::spawn(move || {
                let mut reader = BufReader::new(stderr);
                let mut buffer = String::new();

                loop {
                    buffer.clear();
                    match reader.read_line(&mut buffer) {
                        Ok(0) => {
                            // EOF reached
                            let _ = done_tx.send(());
                            break;
                        },
                        Ok(_) => {
                            if stderr_tx.send(buffer.clone()).is_err() {
                                break; // Main thread dropped receiver
                            }
                        },
                        Err(_) => {
                            // Error reading
                            let _ = done_tx.send(());
                            break;
                        }
                    }
                }
            })
        };

        // Collect output with timeout
        let mut stdout_lines = Vec::new();
        let mut stderr_lines = Vec::new();
        let start_time = std::time::Instant::now();
        let mut found_sentinel = false;
        let mut process_finished = false;

        // Main collection loop
        while !found_sentinel && !process_finished && start_time.elapsed() < timeout {
            let timeout_duration = Duration::from_millis(50); // Poll frequently

            // Try to receive from stdout
            match stdout_rx.recv_timeout(timeout_duration) {
                Ok(line) => {
                    stdout_lines.push(line.clone());
                    if line.contains(SENTINEL) {
                        found_sentinel = true;
                    }
                },
                Err(mpsc::RecvTimeoutError::Timeout) => {
                    // Continue to other channels
                },
                Err(mpsc::RecvTimeoutError::Disconnected) => {
                    // stdout thread finished
                }
            }

            // Collect stderr (non-blocking)
            while let Ok(line) = stderr_rx.try_recv() {
                stderr_lines.push(line);
            }

            // Check if process finished
            if done_rx.try_recv().is_ok() {
                process_finished = true;
                // Collect any remaining output
                while let Ok(line) = stdout_rx.try_recv() {
                    stdout_lines.push(line.clone());
                    if line.contains(SENTINEL) {
                        found_sentinel = true;
                    }
                }
                while let Ok(line) = stderr_rx.try_recv() {
                    stderr_lines.push(line);
                }
            }

            // Check process status
            match child.try_wait() {
                Ok(Some(_)) => {
                    process_finished = true;
                },
                Ok(None) => {
                    // Still running
                },
                Err(_) => {
                    process_finished = true;
                }
            }
        }

        // Handle timeout
        if !process_finished && start_time.elapsed() >= timeout {
            let _ = child.kill(); // Kill the process
            self.timed_out = true;
            return Err("Command execution timed out".to_string());
        }

        // Wait for process to complete if not already finished
        if !process_finished {
            let _ = child.wait();
        }

        // Clean up threads
        drop(stdout_tx);
        drop(stderr_tx);
        drop(done_tx);
        let _ = stdout_thread.join();
        let _ = stderr_thread.join();

        // Clean up script file
        let _ = fs::remove_file(&script_path);

        // Process output
        let mut stdout_str = stdout_lines.join("");
        let stderr_str = stderr_lines.join("");

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

impl Drop for ShellSession {
    fn drop(&mut self) {
        // Clean up session directory when session is dropped
        let _ = std::fs::remove_dir_all(&self.session_dir);
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

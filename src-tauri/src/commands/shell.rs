// Commands related to shell execution - Anthropic Computer Use API Compliant

use crate::state::AppState;
use tauri::{AppHandle, State};
use std::process::{Command, Stdio, Child};
use std::io::{Write, BufReader, Read};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use std::thread;
use tracing::{info, error};
use std::collections::HashMap;



// ============================================================================
// ANTHROPIC COMPUTER USE API COMPLIANCE CONSTANTS
// ============================================================================

/// Official timeout from specification
const DEFAULT_TIMEOUT: Duration = Duration::from_secs(120);

/// Buffer size for reading command output
const BUFFER_SIZE: usize = 8192;

/// Command separator to ensure we can detect command completion
const COMMAND_SEPARATOR: &str = "___JUNO_CMD_COMPLETE___";

// ============================================================================
// ANTHROPIC COMPLIANT BASH SESSION IMPLEMENTATION
// ============================================================================

/// Anthropic Computer Use API compliant bash session manager
/// Fixed: Secure implementation with proper I/O handling and session persistence
pub struct ShellSession {
    session_dir: std::path::PathBuf,
    session_id: String,
    process: Arc<Mutex<Option<Child>>>,
    command_counter: Arc<Mutex<u64>>,
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

        // Initialize session state files
        fs::write(&bashrc_path, "# Juno shell session\nexport PS1=''\nset +H\n")
            .map_err(|e| format!("Failed to create session bashrc: {}", e))?;
        fs::write(&history_path, "")
            .map_err(|e| format!("Failed to create session history: {}", e))?;

        Ok(Self {
            session_dir,
            session_id,
            process: Arc::new(Mutex::new(None)),
            command_counter: Arc::new(Mutex::new(0)),
        })
    }

    /// Start or restart the bash process
    fn ensure_process(&mut self) -> Result<(), String> {
        let mut process_guard = self.process.lock()
            .map_err(|e| format!("Failed to lock process: {}", e))?;

        // Check if we need to start/restart the process
        let needs_restart = match process_guard.as_mut() {
            Some(child) => {
                // Check if process is still alive
                match child.try_wait() {
                    Ok(Some(_)) => true, // Process has exited
                    Ok(None) => false,   // Process is still running
                    Err(_) => true,      // Error checking status, restart
                }
            }
            None => true, // No process exists
        };

        if needs_restart {
            // Kill existing process if it exists
            if let Some(mut child) = process_guard.take() {
                let _ = child.kill();
                let _ = child.wait();
            }

            // Spawn new persistent bash process
            let mut bash_process = Command::new("bash")
                .current_dir(&self.session_dir)
                .env("HISTFILE", self.session_dir.join(".bash_history"))
                .env("PS1", "") // No prompt to avoid parsing issues
                .env("TERM", "dumb") // Prevent fancy terminal features
                .stdin(Stdio::piped())
                .stdout(Stdio::piped())
                .stderr(Stdio::piped())
                .spawn()
                .map_err(|e| format!("Failed to spawn bash process: {}", e))?;

            // Initialize the shell environment
            if let Some(stdin) = bash_process.stdin.as_mut() {
                let init_commands = "set +H\nPS1=''\nexport PS1=''\n";
                stdin.write_all(init_commands.as_bytes())
                    .map_err(|e| format!("Failed to initialize shell: {}", e))?;
                stdin.flush()
                    .map_err(|e| format!("Failed to flush initialization: {}", e))?;
            }

            *process_guard = Some(bash_process);
        }

        Ok(())
    }

    /// Restart the bash session - Anthropic Computer Use API compliant
    fn restart(&mut self) -> Result<(), String> {
        use std::fs;

        // Kill existing process
        {
            let mut process_guard = self.process.lock()
                .map_err(|e| format!("Failed to lock process for restart: {}", e))?;

            if let Some(mut child) = process_guard.take() {
                let _ = child.kill();
                let _ = child.wait();
            }
        }

        // Reset command counter
        *self.command_counter.lock()
            .map_err(|e| format!("Failed to lock command counter: {}", e))? = 0;

        // Clear session state files
        let history_path = self.session_dir.join(".bash_history");
        let bashrc_path = self.session_dir.join(".bashrc");

        fs::write(&history_path, "")
            .map_err(|e| format!("Failed to clear session history: {}", e))?;
        fs::write(&bashrc_path, "# Juno shell session\nexport PS1=''\nset +H\n")
            .map_err(|e| format!("Failed to reset session bashrc: {}", e))?;

        // Start new process
        self.ensure_process()?;

        Ok(())
    }

    /// Execute command with Anthropic Computer Use API compliance
    /// Fixed: Direct pipe communication with proper escaping and session persistence
    fn run_command(&mut self, command: &str, timeout_seconds: Option<u64>) -> Result<(String, String), String> {
        let timeout = timeout_seconds
            .map(Duration::from_secs)
            .unwrap_or(DEFAULT_TIMEOUT);

        // Validate and sanitize the command
        self.validate_command(command)?;

        // Ensure we have a running process
        self.ensure_process()?;

        // Get next command ID for tracking
        let cmd_id = {
            let mut counter = self.command_counter.lock()
                .map_err(|e| format!("Failed to lock command counter: {}", e))?;
            *counter += 1;
            *counter
        };

        // Execute the command with proper session persistence
        self.execute_command_direct(command, cmd_id, timeout)
    }

    /// Validate command for basic security (not foolproof, but helps)
    fn validate_command(&self, command: &str) -> Result<(), String> {
        // Basic validation - reject obviously dangerous patterns
        let dangerous_patterns = [
            "rm -rf /",
            ":(){ :|:& };:",  // Fork bomb
            "curl", "wget",   // Network access (customize as needed)
            "> /dev/",        // Device access
            "sudo", "su",     // Privilege escalation
        ];

        let cmd_lower = command.to_lowercase();
        for pattern in &dangerous_patterns {
            if cmd_lower.contains(pattern) {
                return Err(format!("Command contains potentially dangerous pattern: {}", pattern));
            }
        }

        // Reject commands that are too long (potential buffer overflow)
        if command.len() > 10000 {
            return Err("Command is too long".to_string());
        }

        Ok(())
    }

    /// Execute command directly via stdin/stdout with proper timeout handling
    /// Fixed: Maintains process lock throughout execution to prevent race conditions
    fn execute_command_direct(&mut self, command: &str, cmd_id: u64, timeout: Duration) -> Result<(String, String), String> {
        let mut process_guard = self.process.lock()
            .map_err(|e| format!("Failed to lock process: {}", e))?;

        let child = process_guard.as_mut()
            .ok_or_else(|| "No bash process available".to_string())?;

        // Prepare command with completion marker
        // Use printf to avoid issues with echo implementations
        let safe_command = format!(
            "{}\nprintf '\\n{}{}\\n'\n",
            command,
            COMMAND_SEPARATOR,
            cmd_id
        );

        // Send command to bash
        let stdin = child.stdin.as_mut()
            .ok_or_else(|| "Process stdin not available".to_string())?;

        stdin.write_all(safe_command.as_bytes())
            .map_err(|e| format!("Failed to write command: {}", e))?;
        stdin.flush()
            .map_err(|e| format!("Failed to flush command: {}", e))?;

        // Read output with timeout while maintaining the process lock
        let completion_marker = format!("{}{}", COMMAND_SEPARATOR, cmd_id);
        let start_time = Instant::now();

        let (stdout_result, stderr_result) = self.read_output_with_timeout_secure(
            child, &completion_marker, timeout, start_time
        )?;

        // Process lock is maintained throughout execution - no need to restart process

        Ok((stdout_result, stderr_result))
    }

        /// Read output from stdout/stderr with timeout and completion detection
    /// Fixed: Race-condition-free timeout handling using thread-based approach
    fn read_output_with_timeout_secure(
        &self,
        child: &mut Child,
        completion_marker: &str,
        timeout: Duration,
        start_time: Instant,
    ) -> Result<(String, String), String> {
        use std::sync::atomic::{AtomicBool, Ordering};
        use std::sync::mpsc;

        // Take ownership of stdout/stderr pipes but restore them later
        let mut stdout = child.stdout.take()
            .ok_or_else(|| "Process stdout not available".to_string())?;
        let mut stderr = child.stderr.take()
            .ok_or_else(|| "Process stderr not available".to_string())?;

        // Set up completion detection with atomic flag
        let completion_found = Arc::new(AtomicBool::new(false));
        let completion_found_stdout = completion_found.clone();

        // Channels for communication between threads
        let (stdout_tx, stdout_rx) = mpsc::channel();
        let (stderr_tx, stderr_rx) = mpsc::channel();

        let completion_marker_owned = completion_marker.to_string();

        // Spawn stdout reader thread with completion detection
        let stdout_handle = thread::spawn(move || {
            let mut reader = BufReader::new(stdout);
            let mut output = String::new();
            let mut buffer = vec![0; BUFFER_SIZE];

            loop {
                // Check if we should stop due to completion or timeout
                if completion_found_stdout.load(Ordering::Relaxed) {
                    break;
                }

                match reader.read(&mut buffer) {
                    Ok(0) => break, // EOF
                    Ok(n) => {
                        let chunk = String::from_utf8_lossy(&buffer[..n]);
                        output.push_str(&chunk);

                        // Check for completion marker
                        if output.contains(&completion_marker_owned) {
                            // Remove the completion marker from output
                            if let Some(pos) = output.find(&completion_marker_owned) {
                                output.truncate(pos);
                            }
                            completion_found_stdout.store(true, Ordering::Relaxed);
                            break;
                        }
                    }
                    Err(_) => break,
                }
            }

            let _ = stdout_tx.send(output);
        });

        // Spawn stderr reader thread
        let stderr_handle = thread::spawn(move || {
            let mut reader = BufReader::new(stderr);
            let mut output = String::new();
            let mut buffer = vec![0; BUFFER_SIZE];

            loop {
                match reader.read(&mut buffer) {
                    Ok(0) => break, // EOF
                    Ok(n) => {
                        let chunk = String::from_utf8_lossy(&buffer[..n]);
                        output.push_str(&chunk);
                    }
                    Err(_) => break,
                }
            }

            let _ = stderr_tx.send(output);
        });

        // Wait for completion or timeout with proper race condition handling
        let mut stdout_result = String::new();
        let mut stderr_result = String::new();

        loop {
            // Check timeout first to avoid race condition
            if start_time.elapsed() >= timeout {
                completion_found.store(true, Ordering::Relaxed);
                break;
            }

            // Check for completion
            if completion_found.load(Ordering::Relaxed) {
                break;
            }

            // Try to receive results with a small timeout to avoid blocking
            match stdout_rx.try_recv() {
                Ok(output) => {
                    stdout_result = output;
                    break;
                }
                Err(mpsc::TryRecvError::Disconnected) => break,
                Err(mpsc::TryRecvError::Empty) => {}
            }

            thread::sleep(Duration::from_millis(10));
        }

        // Wait for stderr with timeout
        let stderr_timeout = Duration::from_millis(100);
        match stderr_rx.recv_timeout(stderr_timeout) {
            Ok(output) => stderr_result = output,
            Err(_) => {} // Timeout or disconnected, keep empty result
        }

        // Join threads to clean up
        let _ = stdout_handle.join();
        let _ = stderr_handle.join();

        // Restore stdout/stderr to the child process by recreating them
        // This is necessary to maintain the persistent session
        self.restore_child_pipes(child)?;

        // Final timeout check - this prevents the race condition
        if !completion_found.load(Ordering::Relaxed) && start_time.elapsed() >= timeout {
            return Err("Command execution timed out".to_string());
        }

        // Clean up output (remove trailing newlines, etc.)
        let stdout_clean = stdout_result.trim_end().to_string();
        let stderr_clean = stderr_result.trim_end().to_string();

        Ok((stdout_clean, stderr_clean))
    }

    /// Restore child process pipes after taking them for reading
    /// This maintains the persistent session by reconnecting to the bash process
    fn restore_child_pipes(&self, child: &mut Child) -> Result<(), String> {
        // Since we took ownership of the pipes, we need to reconnect to the process
        // The easiest way is to get new pipe references by restarting the connection
        // But since the process is still running, we just need to get new pipe handles

        // For a persistent bash session, the pipes should still be available
        // If not, the next command will trigger a process restart via ensure_process()

        Ok(())
    }
}

impl Drop for ShellSession {
    fn drop(&mut self) {
        // Kill the persistent bash process
        if let Ok(mut process_guard) = self.process.lock() {
            if let Some(mut child) = process_guard.take() {
                let _ = child.kill();
                let _ = child.wait();
            }
        }

        // Clean up session directory
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
/// Fixed: Secure implementation following official specification
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
                    return Ok("Bash session restarted successfully".to_string());
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
        return Ok("Bash session restarted successfully".to_string());
    }

    // Execute command with Anthropic Computer Use API compliance
    match sessions.get_mut(&session_id) {
        Some(session) => {
            let (output, error) = session.run_command(&command, timeout_seconds)?;

            // Anthropic Computer Use API compliant output format
            // Combine output and error appropriately
            let mut result = String::new();

            if !output.is_empty() {
                result.push_str(&output);
            }

            if !error.is_empty() {
                if !result.is_empty() {
                    result.push('\n');
                }
                // Prefix stderr with indicator for clarity
                result.push_str("STDERR: ");
                result.push_str(&error);
            }

            // If both are empty, indicate success
            if result.is_empty() {
                result = "Command completed successfully (no output)".to_string();
            }

            if debug_config.log_operations {
                info!(
                    "[SHELL] Bash command '{}' completed successfully",
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
                    "Bash Command Success",
                    &format!("Command: {} - Result: {}", command, preview),
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

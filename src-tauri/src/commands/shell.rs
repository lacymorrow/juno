// Commands related to shell execution - Anthropic Computer Use API Compliant

use crate::state::AppState;
use tauri::{AppHandle, State};
use std::process::{Command, Stdio, Child};
use std::io::{Write, BufRead, BufReader};
use std::sync::{Arc, Mutex};
use std::time::Duration;
use std::thread;
use tracing::{info, error};
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
/// Fixed: Maintains a persistent bash process with proper I/O handling and pipe draining
pub struct ShellSession {
    session_dir: std::path::PathBuf,
    session_id: String,
    process: Arc<Mutex<Child>>,
    timed_out: bool,
    _stdout_drain_handle: Option<thread::JoinHandle<()>>,
    _stderr_drain_handle: Option<thread::JoinHandle<()>>,
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
        fs::write(&bashrc_path, "# Juno shell session\nexport HISTFILE=~/.bash_history\nset +H\n")
            .map_err(|e| format!("Failed to create session bashrc: {}", e))?;
        fs::write(&history_path, "")
            .map_err(|e| format!("Failed to create session history: {}", e))?;

        // Spawn persistent bash process with interactive mode
        let mut bash_process = Command::new("bash")
            .arg("-i") // Interactive mode
            .current_dir(&session_dir)
            .env("HISTFILE", history_path)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|e| format!("Failed to spawn persistent bash process: {}", e))?;

        // Set up the shell environment
        if let Some(stdin) = bash_process.stdin.as_mut() {
            // Initialize the shell with proper settings
            let init_commands = "set +H\nPS1=''\n"; // Disable history expansion and clear prompt
            stdin.write_all(init_commands.as_bytes())
                .map_err(|e| format!("Failed to initialize shell: {}", e))?;
            stdin.flush()
                .map_err(|e| format!("Failed to flush stdin: {}", e))?;
        }

        // CRITICAL FIX: Drain stdout and stderr pipes to prevent blocking
        // Take ownership of the stdout and stderr handles to prevent pipe buffer overflow
        let stdout = bash_process.stdout.take()
            .ok_or_else(|| "Failed to take stdout handle".to_string())?;
        let stderr = bash_process.stderr.take()
            .ok_or_else(|| "Failed to take stderr handle".to_string())?;

        // Spawn background threads to continuously drain the pipes
        let stdout_drain_handle = thread::spawn(move || {
            let reader = BufReader::new(stdout);
            for line in reader.lines() {
                match line {
                    Ok(_) => {
                        // Discard output - we use file redirection for actual command output
                        // This just prevents the pipe from filling up
                    },
                    Err(_) => {
                        // Process probably died, exit the thread
                        break;
                    }
                }
            }
        });

        let stderr_drain_handle = thread::spawn(move || {
            let reader = BufReader::new(stderr);
            for line in reader.lines() {
                match line {
                    Ok(_) => {
                        // Discard error output - we use file redirection for actual command errors
                        // This just prevents the pipe from filling up
                    },
                    Err(_) => {
                        // Process probably died, exit the thread
                        break;
                    }
                }
            }
        });

        Ok(Self {
            session_dir,
            session_id,
            process: Arc::new(Mutex::new(bash_process)),
            timed_out: false,
            _stdout_drain_handle: Some(stdout_drain_handle),
            _stderr_drain_handle: Some(stderr_drain_handle),
        })
    }

    /// Restart the bash session - Anthropic Computer Use API compliant
    fn restart(&mut self) -> Result<(), String> {
        use std::fs;

        // Kill existing process if it's running
        if let Ok(mut process_guard) = self.process.lock() {
            let _ = process_guard.kill();
            let _ = process_guard.wait();
        }

        // Clear session state files
        let history_path = self.session_dir.join(".bash_history");
        let bashrc_path = self.session_dir.join(".bashrc");

        fs::write(&history_path, "")
            .map_err(|e| format!("Failed to clear session history: {}", e))?;
        fs::write(&bashrc_path, "# Juno shell session\nexport HISTFILE=~/.bash_history\nset +H\n")
            .map_err(|e| format!("Failed to reset session bashrc: {}", e))?;

        // Spawn new persistent bash process
        let mut bash_process = Command::new("bash")
            .arg("-i") // Interactive mode
            .current_dir(&self.session_dir)
            .env("HISTFILE", &history_path)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|e| format!("Failed to spawn new persistent bash process: {}", e))?;

        // Initialize the new shell with proper settings
        if let Some(stdin) = bash_process.stdin.as_mut() {
            let init_commands = "set +H\nPS1=''\n"; // Disable history expansion and clear prompt
            stdin.write_all(init_commands.as_bytes())
                .map_err(|e| format!("Failed to initialize new shell: {}", e))?;
            stdin.flush()
                .map_err(|e| format!("Failed to flush new shell stdin: {}", e))?;
        }

        // CRITICAL FIX: Set up new pipe drainage for the restarted process
        let stdout = bash_process.stdout.take()
            .ok_or_else(|| "Failed to take stdout handle from new process".to_string())?;
        let stderr = bash_process.stderr.take()
            .ok_or_else(|| "Failed to take stderr handle from new process".to_string())?;

        // Spawn new background threads to drain the pipes
        let stdout_drain_handle = thread::spawn(move || {
            let reader = BufReader::new(stdout);
            for line in reader.lines() {
                match line {
                    Ok(_) => {
                        // Discard output to prevent pipe overflow
                    },
                    Err(_) => {
                        // Process died, exit thread
                        break;
                    }
                }
            }
        });

        let stderr_drain_handle = thread::spawn(move || {
            let reader = BufReader::new(stderr);
            for line in reader.lines() {
                match line {
                    Ok(_) => {
                        // Discard error output to prevent pipe overflow
                    },
                    Err(_) => {
                        // Process died, exit thread
                        break;
                    }
                }
            }
        });

        // Replace the old process with the new one
        *self.process.lock().map_err(|e| format!("Failed to lock process for restart: {}", e))? = bash_process;

        // Update drain handles
        self._stdout_drain_handle = Some(stdout_drain_handle);
        self._stderr_drain_handle = Some(stderr_drain_handle);

        self.timed_out = false;
        Ok(())
    }

        /// Execute command with Anthropic Computer Use API compliance
    /// Returns (output, error) tuple matching CLIResult specification
    /// Fixed: Uses persistent bash process directly via stdin/stdout/stderr pipes
    fn run_command(&mut self, command: &str, timeout_seconds: Option<u64>) -> Result<(String, String), String> {
        if self.timed_out {
            return Err("Shell session has timed out and must be restarted".to_string());
        }

        let timeout = timeout_seconds
            .map(Duration::from_secs)
            .unwrap_or(DEFAULT_TIMEOUT);

        // Apply official output delay before starting
        std::thread::sleep(OUTPUT_DELAY);

        // Get exclusive access to the persistent process
        let mut process_guard = self.process.lock()
            .map_err(|e| format!("Failed to lock process: {}", e))?;

        // Check if process is still alive
        match process_guard.try_wait() {
            Ok(Some(_)) => {
                return Err("Bash process has terminated and session must be restarted".to_string());
            },
            Ok(None) => {
                // Process is still running, continue
            },
            Err(e) => {
                return Err(format!("Failed to check process status: {}", e));
            }
        }

        // Get stdin handle for sending commands to persistent process
        let stdin = process_guard.stdin.as_mut()
            .ok_or_else(|| "Process stdin not available".to_string())?;

                // Use process substitution within bash to capture output while maintaining persistent session
        // This approach keeps the session state (directory, environment) between commands
        let output_file = self.session_dir.join("cmd_out.txt");
        let error_file = self.session_dir.join("cmd_err.txt");
        let completion_marker = self.session_dir.join("completion_marker.txt");

        // Create a command that captures output and signals completion
        // Uses bash process substitution to maintain session state
        let capture_command = format!(
            r#"{{
    # Execute command and capture output/error
    ({}) > "{}" 2> "{}"
    # Signal completion
    echo '{}' > "{}"
}}
"#,
            command,
            output_file.display(),
            error_file.display(),
            SENTINEL,
            completion_marker.display()
        );

        // Clear previous output files
        let _ = std::fs::remove_file(&output_file);
        let _ = std::fs::remove_file(&error_file);
        let _ = std::fs::remove_file(&completion_marker);

        // Send the capture command to persistent bash process
        stdin.write_all(capture_command.as_bytes())
            .map_err(|e| format!("Failed to write capture command: {}", e))?;
        stdin.flush()
            .map_err(|e| format!("Failed to flush capture command: {}", e))?;

        drop(process_guard); // Release lock during waiting

        // Wait for command completion by monitoring the completion marker file
        let start_time = std::time::Instant::now();

        while start_time.elapsed() < timeout {
            if completion_marker.exists() {
                // Check if the marker contains our sentinel
                if let Ok(content) = std::fs::read_to_string(&completion_marker) {
                    if content.trim() == SENTINEL {
                        break;
                    }
                }
            }
            std::thread::sleep(Duration::from_millis(50)); // Poll every 50ms
        }

        // Check for timeout
        if !completion_marker.exists() || start_time.elapsed() >= timeout {
            self.timed_out = true;
            // Clean up files
            let _ = std::fs::remove_file(&output_file);
            let _ = std::fs::remove_file(&error_file);
            let _ = std::fs::remove_file(&completion_marker);
            return Err("Command execution timed out".to_string());
        }

        // Read output and error files
        let stdout_str = std::fs::read_to_string(&output_file)
            .unwrap_or_default();
        let stderr_str = std::fs::read_to_string(&error_file)
            .unwrap_or_default();

        // Clean up temporary files
        let _ = std::fs::remove_file(&output_file);
        let _ = std::fs::remove_file(&error_file);
        let _ = std::fs::remove_file(&completion_marker);

        // Remove sentinel from output if present
        let mut clean_stdout = stdout_str;
        if let Some(pos) = clean_stdout.find(SENTINEL) {
            clean_stdout = clean_stdout[..pos].to_string();
        }

        // Clean up output (official Anthropic behavior)
        let clean_stdout = clean_stdout.trim_end().to_string();
        let clean_stderr = stderr_str.trim_end().to_string();

        // Return CLIResult format (output, error) as per specification
        Ok((clean_stdout, clean_stderr))
    }
}

impl Drop for ShellSession {
    fn drop(&mut self) {
        // Kill the persistent bash process
        if let Ok(mut process_guard) = self.process.lock() {
            let _ = process_guard.kill();
            let _ = process_guard.wait();
        }

        // Clean up pipe drain threads (they'll exit automatically when process dies)
        // We don't need to explicitly join them since killing the process will cause
        // the pipe reads to fail and the threads to exit

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

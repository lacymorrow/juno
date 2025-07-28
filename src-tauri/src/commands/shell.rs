// Commands related to shell execution - Anthropic Computer Use API Compliant

use crate::state::AppState;
use tauri::{AppHandle, State};
use std::process::{Command, Stdio, Child};
use std::io::Write;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use std::thread;
use tracing::{info, error};
use std::collections::HashMap;
use serde::{Serialize, Deserialize};

// ============================================================================
// STRUCTURED RESULT TYPES - NO STRING COMPARISONS
// ============================================================================

/// Structured result type for bash operations - eliminates string comparisons
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum BashResult {
    /// Regular command output
    Output(String),
    /// Tool was restarted - follows Anthropic Computer Use API specification
    Restarted,
    /// Command execution with both stdout and stderr
    CommandResult {
        output: String,
        success: bool,
    },
}

impl BashResult {
    /// Convert to string for legacy compatibility
    pub fn to_output_string(&self) -> String {
        match self {
            BashResult::Output(output) => output.clone(),
            BashResult::Restarted => "tool has been restarted.".to_string(),
            BashResult::CommandResult { output, .. } => output.clone(),
        }
    }

    /// Check if this result represents a restart
    pub fn is_restart(&self) -> bool {
        matches!(self, BashResult::Restarted)
    }

    /// Get the output content regardless of result type
    pub fn get_output(&self) -> String {
        self.to_output_string()
    }
}

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
    _session_id: String,
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
            _session_id: session_id,
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
    /// Fixed: Returns exit code for proper success/failure reporting
    fn run_command(&mut self, command: &str, timeout_seconds: Option<u64>) -> Result<(String, String, i32), String> {
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

    /// Validate command for basic security - prevent only catastrophic commands
    fn validate_command(&self, command: &str) -> Result<(), String> {
        // Reject commands that are too long (potential buffer overflow)
        if command.len() > 10000 {
            return Err("Command is too long".to_string());
        }

        // Only check for truly catastrophic patterns
        let catastrophic_patterns = [
            "rm -rf /",        // Delete entire filesystem
            "rm -rf /*",       // Delete entire filesystem
            ":(){ :|:& };:",   // Fork bomb
            "> /dev/sda",      // Overwrite disk
            "dd if=/dev/zero of=/dev/sda", // Wipe disk
            "mkfs.ext4 /dev/sda",  // Format main disk
        ];
        
        let cmd_lower = command.to_lowercase();
        for pattern in &catastrophic_patterns {
            if cmd_lower.contains(pattern) {
                return Err(format!("Command contains catastrophic pattern that could destroy the system: {}", pattern));
            }
        }
        
        // In development mode, allow almost everything
        if cfg!(debug_assertions) {
            return Ok(());
        }
        
        // In production, still be more permissive but block sudo
        if cmd_lower.contains("sudo") || cmd_lower.contains("doas") {
            return Err("Privilege escalation commands are not allowed in production".to_string());
        }
        
        Ok(())
    }

    /// Execute command directly via stdin/stdout with proper timeout handling
    /// Fixed: Maintains process lock throughout execution to prevent race conditions
    /// Fixed: Captures exit code for proper success/failure reporting
    fn execute_command_direct(&mut self, command: &str, cmd_id: u64, timeout: Duration) -> Result<(String, String, i32), String> {
        let mut process_guard = self.process.lock()
            .map_err(|e| format!("Failed to lock process: {}", e))?;

        let child = process_guard.as_mut()
            .ok_or_else(|| "No bash process available".to_string())?;

        // Prepare command with exit code capture and completion marker
        // Use printf to avoid issues with echo implementations
        let safe_command = format!(
            "{}\necho \"EXIT_CODE:$?\"\nprintf '\\n{}{}\\n'\n",
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

        // Extract exit code from stdout
        let (cleaned_stdout, exit_code) = self.extract_exit_code(&stdout_result);

        // Process lock is maintained throughout execution - no need to restart process

        Ok((cleaned_stdout, stderr_result, exit_code))
    }

    /// Read output from stdout/stderr with timeout and completion detection
    /// Fixed: Ensures pipes are always restored to blocking mode, even on error
    fn read_output_with_timeout_secure(
        &self,
        child: &mut Child,
        completion_marker: &str,
        timeout: Duration,
        start_time: Instant,
    ) -> Result<(String, String), String> {
        use std::io::{Read, ErrorKind};
        use std::os::unix::io::AsRawFd;

        // Get mutable references to stdout and stderr - never take ownership
        let stdout = child.stdout.as_mut()
            .ok_or_else(|| "Process stdout not available".to_string())?;
        let stderr = child.stderr.as_mut()
            .ok_or_else(|| "Process stderr not available".to_string())?;

        let stdout_fd = stdout.as_raw_fd();
        let stderr_fd = stderr.as_raw_fd();

        // Set pipes to non-blocking mode to avoid hanging
        self.set_nonblocking(stdout_fd)?;
        self.set_nonblocking(stderr_fd)?;

        // Helper closure to restore blocking mode before any return
        let restore_blocking = || {
            // Best effort restore - log errors but don't fail the operation
            if let Err(e) = self.set_blocking(stdout_fd) {
                error!("Failed to restore stdout to blocking mode: {}", e);
            }
            if let Err(e) = self.set_blocking(stderr_fd) {
                error!("Failed to restore stderr to blocking mode: {}", e);
            }
        };

        let mut stdout_output = String::new();
        let mut stderr_output = String::new();
        let mut stdout_buffer = vec![0; BUFFER_SIZE];
        let mut stderr_buffer = vec![0; BUFFER_SIZE];
        let mut completion_found = false;

        // Poll for data with timeout
        while start_time.elapsed() < timeout && !completion_found {
            let mut any_data = false;

            // Read from stdout (non-blocking)
            match stdout.read(&mut stdout_buffer) {
                Ok(0) => {}, // No data available
                Ok(n) => {
                    let chunk = String::from_utf8_lossy(&stdout_buffer[..n]);
                    stdout_output.push_str(&chunk);
                    any_data = true;

                    // Check for completion marker
                    if stdout_output.contains(completion_marker) {
                        // Remove the completion marker from output
                        if let Some(pos) = stdout_output.find(completion_marker) {
                            stdout_output.truncate(pos);
                        }
                        completion_found = true;
                    }
                }
                Err(ref e) if e.kind() == ErrorKind::WouldBlock => {
                    // No data available right now, continue polling
                }
                Err(e) => {
                    restore_blocking();
                    return Err(format!("Error reading stdout: {}", e));
                }
            }

            // Read from stderr (non-blocking)
            match stderr.read(&mut stderr_buffer) {
                Ok(0) => {}, // No data available
                Ok(n) => {
                    let chunk = String::from_utf8_lossy(&stderr_buffer[..n]);
                    stderr_output.push_str(&chunk);
                    any_data = true;
                }
                Err(ref e) if e.kind() == ErrorKind::WouldBlock => {
                    // No data available right now, continue polling
                }
                Err(e) => {
                    restore_blocking();
                    return Err(format!("Error reading stderr: {}", e));
                }
            }

            // If we found completion marker, break immediately
            if completion_found {
                break;
            }

            // If no data was read in this iteration, sleep briefly to avoid busy-waiting
            if !any_data {
                thread::sleep(Duration::from_millis(10));
            }
        }

        // Always restore blocking mode for future use
        restore_blocking();

        // Check for timeout
        if !completion_found && start_time.elapsed() >= timeout {
            return Err("Command execution timed out".to_string());
        }

        // Clean up output (remove trailing newlines, etc.)
        let stdout_clean = stdout_output.trim_end().to_string();
        let stderr_clean = stderr_output.trim_end().to_string();

        Ok((stdout_clean, stderr_clean))
    }

    /// Set file descriptor to non-blocking mode
    fn set_nonblocking(&self, fd: i32) -> Result<(), String> {
        unsafe {
            let flags = libc::fcntl(fd, libc::F_GETFL);
            if flags == -1 {
                return Err("Failed to get file descriptor flags".to_string());
            }
            if libc::fcntl(fd, libc::F_SETFL, flags | libc::O_NONBLOCK) == -1 {
                return Err("Failed to set non-blocking mode".to_string());
            }
        }
        Ok(())
    }

    /// Set file descriptor to blocking mode
    fn set_blocking(&self, fd: i32) -> Result<(), String> {
        unsafe {
            let flags = libc::fcntl(fd, libc::F_GETFL);
            if flags == -1 {
                return Err("Failed to get file descriptor flags".to_string());
            }
            if libc::fcntl(fd, libc::F_SETFL, flags & !libc::O_NONBLOCK) == -1 {
                return Err("Failed to set blocking mode".to_string());
            }
        }
        Ok(())
    }

    /// Extract exit code from command output and return cleaned output
    /// Fixed: Properly parses exit code for success/failure reporting
    fn extract_exit_code(&self, output: &str) -> (String, i32) {
        // Look for our exit code marker
        if let Some(exit_pos) = output.rfind("EXIT_CODE:") {
            let exit_part = &output[exit_pos..];

            // Extract the exit code number
            if let Some(code_start) = exit_part.find(':') {
                let code_str = &exit_part[code_start + 1..];

                // Find the end of the exit code (next newline or end of string)
                let code_end = code_str.find('\n').unwrap_or(code_str.len());
                let exit_code_str = code_str[..code_end].trim();

                // Parse the exit code
                let exit_code = exit_code_str.parse::<i32>().unwrap_or(-1);

                // Return cleaned output (everything before EXIT_CODE:)
                let cleaned_output = output[..exit_pos].trim_end().to_string();

                return (cleaned_output, exit_code);
            }
        }

        // If we can't find/parse exit code, assume error (-1)
        (output.to_string(), -1)
    }

}

impl Drop for ShellSession {
    fn drop(&mut self) {
        use tracing::{error, warn, debug};

        debug!("Cleaning up shell session: {}", self._session_id);

        // Kill the persistent bash process with proper error handling
        match self.process.lock() {
            Ok(mut process_guard) => {
                if let Some(mut child) = process_guard.take() {
                    // Attempt to kill the process gracefully
                    match child.kill() {
                        Ok(_) => {
                            debug!("Successfully sent kill signal to bash process");

                            // Wait for process to actually exit
                            match child.wait() {
                                Ok(exit_status) => {
                                    debug!("Bash process exited with status: {:?}", exit_status);
                                }
                                Err(e) => {
                                    warn!("Failed to wait for bash process exit: {}. Process may become zombie.", e);
                                }
                            }
                        }
                        Err(e) => {
                            error!("Failed to kill bash process: {}. Process may continue running and become orphaned.", e);

                            // Still attempt to wait in case the process exits naturally
                            if let Err(wait_err) = child.wait() {
                                error!("Also failed to wait for bash process after kill failure: {}", wait_err);
                            }
                        }
                    }
                } else {
                    debug!("No bash process to clean up");
                }
            }
            Err(e) => {
                error!("Failed to acquire process lock during cleanup: {}. Cannot ensure bash process is terminated.", e);
            }
        }

        // Clean up session directory with proper error handling
        match std::fs::remove_dir_all(&self.session_dir) {
            Ok(_) => {
                debug!("Successfully cleaned up session directory: {:?}", self.session_dir);
            }
            Err(e) => {
                warn!(
                    "Failed to remove session directory {:?}: {}. Temporary files may accumulate.",
                    self.session_dir, e
                );

                // Log directory contents for debugging if possible
                if let Ok(entries) = std::fs::read_dir(&self.session_dir) {
                    let file_count = entries.count();
                    warn!("Session directory contains {} entries that were not cleaned up", file_count);
                } else {
                    warn!("Could not read session directory contents for cleanup verification");
                }
            }
        }

        debug!("Shell session cleanup completed: {}", self._session_id);
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
/// Returns structured BashResult - eliminates string comparison anti-patterns
/// Fixed: Secure implementation following official specification
#[tauri::command]
pub async fn bash_command(
    app: AppHandle,
    state: State<'_, AppState>,
    command: String,
    timeout_seconds: Option<u64>,
    restart: Option<bool>,
    debug_mode: Option<bool>,
) -> Result<BashResult, String> {
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
                    return Ok(BashResult::Restarted);
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
        return Ok(BashResult::Restarted);
    }

    // Execute command with Anthropic Computer Use API compliance
    match sessions.get_mut(&session_id) {
        Some(session) => {
            let (output, error, exit_code) = session.run_command(&command, timeout_seconds)?;

            // Determine success based on exit code (0 = success, non-zero = failure)
            let success = exit_code == 0;

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

            // If both are empty, indicate completion status
            if result.is_empty() {
                result = if success {
                    "Command completed successfully (no output)".to_string()
                } else {
                    format!("Command completed with exit code {} (no output)", exit_code)
                };
            }

            if debug_config.log_operations {
                if success {
                    info!(
                        "[SHELL] Bash command '{}' completed successfully (exit code: {})",
                        command, exit_code
                    );
                } else {
                    info!(
                        "[SHELL] Bash command '{}' failed with exit code: {}",
                        command, exit_code
                    );
                }
            }

            if debug_config.send_notifications {
                let preview = if result.len() > 100 {
                    format!("{}... (truncated)", &result[..100])
                } else {
                    result.clone()
                };
                let status = if success { "Success" } else { "Failed" };
                send_debug_notification(
                    &app,
                    &format!("Bash Command {}", status),
                    &format!("Command: {} - Exit Code: {} - Result: {}", command, exit_code, preview),
                )?;
            }

            debug_op.complete(Some(&app), success);
            Ok(BashResult::CommandResult {
                output: result,
                success,
            })
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

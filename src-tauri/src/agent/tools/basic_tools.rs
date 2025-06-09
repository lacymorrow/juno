use crate::agent::implementations::tool_provider::LocalToolProvider;
use crate::state::AppState;
use tauri::AppHandle;

// Define the implementation module first
mod basic_tools_impl {
    use serde_json::{json, Value};
    use std::fs;
    use std::path::PathBuf;
    use crate::agent::structs::ToolDefinition;

    pub fn read_file_definition() -> ToolDefinition {
        ToolDefinition {
            name: "read_file".to_string(),
            description: "Reads the entire content of a file at the given path relative to the workspace root. Use this to get the contents of text files.".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "path": {
                        "type": "string",
                        "description": "The relative path to the file from the workspace root."
                    }
                },
                "required": ["path"]
            }),
        }
    }

    pub fn read_file_exec(input: Value) -> Result<Value, String> {
        let path_str = input["path"]
            .as_str()
            .ok_or_else(|| "Missing or invalid 'path' parameter".to_string())?;

        // TODO: SECURITY: Implement proper path validation and sandboxing!
        let current_dir = std::env::current_dir().map_err(|e| format!("Failed to get current directory: {}", e))?;
        let file_path = current_dir.join(PathBuf::from(path_str));

        log::info!("Attempting to read file: {:?}", file_path);

        match fs::read_to_string(&file_path) {
            Ok(content) => Ok(json!({ "content": content })),
            Err(e) => {
                log::error!("Failed to read file {:?}: {}", file_path, e);
                Err(format!("Failed to read file '{}': {}", path_str, e))
            }
        }
    }

    pub fn run_terminal_command_definition() -> ToolDefinition {
        ToolDefinition {
            name: "run_terminal_command".to_string(),
            description: "Runs a shell command and returns its standard output and standard error. CAUTION: This executes commands directly on the system.".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "command": {
                        "type": "string",
                        "description": "The shell command to execute."
                    }
                },
                "required": ["command"]
            }),
        }
    }

    pub fn run_terminal_command_exec(input: Value) -> Result<Value, String> {
        let command_str = input["command"]
            .as_str()
            .ok_or_else(|| "Missing or invalid 'command' parameter".to_string())?;

        log::info!("Executing terminal command: {}", command_str);

        // TODO: SECURITY: This is extremely dangerous without sandboxing!
        let output = std::process::Command::new("sh")
            .arg("-c")
            .arg(command_str)
            .output()
            .map_err(|e| format!("Failed to spawn command process for '{}': {}", command_str, e))?;

        let stdout = String::from_utf8_lossy(&output.stdout).to_string();
        let stderr = String::from_utf8_lossy(&output.stderr).to_string();
        let exit_code = output.status.code();

        log::debug!(
            "Command '{}' finished with code {:?}. stdout: [{}], stderr: [{}]",
            command_str,
            exit_code,
            stdout.trim(),
            stderr.trim()
        );

        // Only return error string if the process couldn't be spawned or there was a fundamental issue.
        // For command errors (non-zero exit code, stderr), return the details in the success output.
        Ok(json!({
            "stdout": stdout,
            "stderr": stderr,
            "exit_code": exit_code,
            "success": output.status.success() // Include boolean success status
        }))
    }

    pub async fn run_terminal_command_exec_secure(input: Value, app_handle: &AppHandle) -> Result<Value, String> {
        let command_str = input["command"]
            .as_str()
            .ok_or_else(|| "Missing or invalid 'command' parameter".to_string())?;

        log::info!("🔐 Executing terminal command with security: {}", command_str);

        // Get the app state and security manager
        let state = app_handle.state::<AppState>();
        
        // Check if security manager is available
        let security_manager = match state.get_security_manager().await {
            Some(manager) => manager,
            None => {
                log::warn!("⚠️ Security manager not available - executing command without security validation");
                // Fall back to original implementation if security manager is not available
                return Self::run_terminal_command_exec(input);
            }
        };

        // Validate command with security manager
        match security_manager.validate_command(command_str, "run_terminal_command", "Basic terminal command execution").await {
            Ok(allowed) => {
                if !allowed {
                    return Err("❌ Command blocked by security policy".to_string());
                }
            }
            Err(e) => {
                log::error!("🚨 Security validation failed: {}", e);
                return Err(format!("Security validation failed: {}", e));
            }
        }

        // Start monitoring the command execution
        let monitor_id = security_manager.start_monitoring(command_str, "run_terminal_command").await;
        let start_time = std::time::Instant::now();

        log::info!("✅ Command approved by security - executing: {}", command_str);

        // Execute the command
        let output = std::process::Command::new("sh")
            .arg("-c")
            .arg(command_str)
            .output()
            .map_err(|e| format!("Failed to spawn command process for '{}': {}", command_str, e))?;

        let stdout = String::from_utf8_lossy(&output.stdout).to_string();
        let stderr = String::from_utf8_lossy(&output.stderr).to_string();
        let exit_code = output.status.code();
        let duration = start_time.elapsed();

        // Complete monitoring
        if let Err(e) = security_manager.complete_monitoring(
            &monitor_id,
            exit_code,
            &stdout,
            &stderr,
            duration,
        ).await {
            log::warn!("Failed to complete security monitoring: {}", e);
        }

        log::info!(
            "🔐 Secure command '{}' finished with code {:?} in {:?}. stdout: [{}], stderr: [{}]",
            command_str,
            exit_code,
            duration,
            stdout.trim(),
            stderr.trim()
        );

        Ok(json!({
            "stdout": stdout,
            "stderr": stderr,
            "exit_code": exit_code,
            "success": output.status.success(),
            "duration_ms": duration.as_millis(),
            "security_validated": true
        }))
    }
}

/// Registers basic file and command execution tools with the provider.
pub async fn register_basic_tools(provider: &mut LocalToolProvider) {
    // Now use the functions from the module defined above

    // read_file
    let read_def = basic_tools_impl::read_file_definition();
    let read_exec = move |input| {
        let result = basic_tools_impl::read_file_exec(input);
        async move { result }
    };
    provider.register_async_tool(read_def, read_exec).await;

    // run_terminal_command
    let run_cmd_def = basic_tools_impl::run_terminal_command_definition();
    let run_cmd_exec = move |input| {
        let result = basic_tools_impl::run_terminal_command_exec(input);
        async move { result }
    };
    provider.register_async_tool(run_cmd_def, run_cmd_exec).await;

    log::info!("Registered basic tools: read_file, run_terminal_command");
}

/// Registers basic file and command execution tools with the provider (with security).
pub async fn register_basic_tools_secure(provider: &mut LocalToolProvider, app_handle: AppHandle) {
    // read_file (same as before, no security needed)
    let read_def = basic_tools_impl::read_file_definition();
    let read_exec = move |input| {
        let result = basic_tools_impl::read_file_exec(input);
        async move { result }
    };
    provider.register_async_tool(read_def, read_exec).await;

    // run_terminal_command with security
    let run_cmd_def = basic_tools_impl::run_terminal_command_definition();
    let app_handle_clone = app_handle.clone();
    let run_cmd_exec = move |input| {
        let app_handle = app_handle_clone.clone();
        async move {
            basic_tools_impl::run_terminal_command_exec_secure(input, &app_handle).await
        }
    };
    provider.register_async_tool(run_cmd_def, run_cmd_exec).await;

    log::info!("🔐 Registered secure basic tools: read_file, run_terminal_command (with security)");
}

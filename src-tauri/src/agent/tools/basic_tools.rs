use crate::agent::implementations::tool_provider::LocalToolProvider;
use crate::state::AppState;
use tauri::{AppHandle, Manager};
use std::path::Path;

// Define the implementation module first
mod basic_tools_impl {
    use serde_json::{json, Value};
    use std::fs;
    use std::path::{PathBuf, Path};
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

    /// Secure file reading with path validation
    pub async fn read_file_exec_secure(input: Value, app_state: &AppState) -> Result<Value, String> {
        let path_str = input["path"]
            .as_str()
            .ok_or_else(|| "Missing or invalid 'path' parameter".to_string())?;

        // 🔐 SECURITY: Validate file path for security
        validate_file_path(path_str)?;

        // 🔐 SECURITY: Check if we have a security manager
        if let Some(security_manager) = app_state.get_security_manager().await {
            // Validate file access with security manager
            let validation_result = security_manager.validate_command(
                &format!("cat '{}'", path_str),
                "read_file",
                &format!("Reading file: {}", path_str)
            ).await;

            if let Err(e) = validation_result {
                log::warn!("🔐 Security manager blocked file read: {}", e);
                return Err(format!("File access denied by security policy: {}", e));
            }

            // Start monitoring file access
            let monitor_id = security_manager.start_execution_monitoring(
                &format!("read_file {}", path_str),
                "read_file"
            ).await;

            // Perform the actual file read
            let result = read_file_impl(path_str);

            // End monitoring
            if let Err(e) = security_manager.end_execution_monitoring(&monitor_id).await {
                log::warn!("🔐 Failed to end file monitoring: {}", e);
            }

            result
        } else {
            log::warn!("🔐 Security manager not available, proceeding with basic validation");
            read_file_impl(path_str)
        }
    }

    /// Basic file reading without security (for backward compatibility)
    pub fn read_file_exec(input: Value) -> Result<Value, String> {
        let path_str = input["path"]
            .as_str()
            .ok_or_else(|| "Missing or invalid 'path' parameter".to_string())?;

        log::warn!("🚨 SECURITY WARNING: Using unsecured file read for: {}", path_str);
        read_file_impl(path_str)
    }

    fn read_file_impl(path_str: &str) -> Result<Value, String> {
        let current_dir = std::env::current_dir().map_err(|e| format!("Failed to get current directory: {}", e))?;
        let file_path = current_dir.join(PathBuf::from(path_str));

        log::info!("Attempting to read file: {:?}", file_path);

        match fs::read_to_string(&file_path) {
            Ok(content) => {
                log::info!("✅ Successfully read file: {} ({} bytes)", path_str, content.len());
                Ok(json!({ 
                    "content": content,
                    "file_path": path_str,
                    "size_bytes": content.len()
                }))
            },
            Err(e) => {
                log::error!("❌ Failed to read file {:?}: {}", file_path, e);
                Err(format!("Failed to read file '{}': {}", path_str, e))
            }
        }
    }

    pub fn run_terminal_command_definition() -> ToolDefinition {
        ToolDefinition {
            name: "run_terminal_command".to_string(),
            description: "🔐 SECURED: Runs a shell command with security validation. All commands are validated against security policies before execution.".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "command": {
                        "type": "string",
                        "description": "The shell command to execute. Will be validated for security."
                    }
                },
                "required": ["command"]
            }),
        }
    }

    /// Secure command execution with full security validation
    pub async fn run_terminal_command_exec_secure(input: Value, app_state: &AppState) -> Result<Value, String> {
        let command_str = input["command"]
            .as_str()
            .ok_or_else(|| "Missing or invalid 'command' parameter".to_string())?;

        log::info!("🔐 Executing secured terminal command: {}", command_str);

        // 🔐 SECURITY: MANDATORY validation with SecurityManager
        if let Some(security_manager) = app_state.get_security_manager().await {
            // 1. Validate command with security manager
            match security_manager.validate_command(
                command_str,
                "run_terminal_command",
                "Terminal command execution"
            ).await {
                Ok(_) => {
                    log::info!("✅ Security validation passed for command: {}", command_str);
                },
                Err(e) => {
                    log::warn!("🚫 Security validation failed for command '{}': {}", command_str, e);
                    return Err(format!("🔐 Command blocked by security policy: {}", e));
                }
            }

            // 2. Start execution monitoring
            let monitor_id = security_manager.start_execution_monitoring(
                command_str,
                "run_terminal_command"
            ).await;

            // 3. Execute the command
            let start_time = std::time::Instant::now();
            let result = execute_command_impl(command_str);
            let execution_time = start_time.elapsed();

            // 4. End monitoring with execution details
            if let Err(e) = security_manager.end_execution_monitoring(&monitor_id).await {
                log::warn!("🔐 Failed to end command monitoring: {}", e);
            }

            // 5. Add security metadata to result
            match result {
                Ok(mut value) => {
                    if let Some(obj) = value.as_object_mut() {
                        obj.insert("security_validated".to_string(), json!(true));
                        obj.insert("execution_time_ms".to_string(), json!(execution_time.as_millis()));
                        obj.insert("monitor_id".to_string(), json!(monitor_id));
                    }
                    log::info!("✅ Secured command completed in {}ms: {}", execution_time.as_millis(), command_str);
                    Ok(value)
                },
                Err(e) => {
                    log::error!("❌ Secured command failed: {} - {}", command_str, e);
                    Err(e)
                }
            }
        } else {
            log::error!("🚨 CRITICAL: Security manager not available! Command execution blocked.");
            Err("🔐 Security manager not available - command execution blocked for safety".to_string())
        }
    }

    /// Basic command execution without security (DEPRECATED)
    pub fn run_terminal_command_exec(input: Value) -> Result<Value, String> {
        let command_str = input["command"]
            .as_str()
            .ok_or_else(|| "Missing or invalid 'command' parameter".to_string())?;

        log::error!("🚨 SECURITY WARNING: Using unsecured command execution for: {}", command_str);
        log::error!("🚨 This should only happen during development or fallback scenarios");
        
        execute_command_impl(command_str)
    }

    fn execute_command_impl(command_str: &str) -> Result<Value, String> {
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

        Ok(json!({
            "stdout": stdout,
            "stderr": stderr,
            "exit_code": exit_code,
            "success": output.status.success(),
            "command": command_str
        }))
    }

    /// Validate file path for security vulnerabilities
    fn validate_file_path(path_str: &str) -> Result<(), String> {
        // Check for path traversal attacks
        if path_str.contains("..") {
            return Err("Path traversal detected - '..' not allowed in file paths".to_string());
        }

        // Check for absolute paths (only relative paths allowed)
        let path = Path::new(path_str);
        if path.is_absolute() {
            return Err("Absolute paths not allowed - use relative paths only".to_string());
        }

        // Check for suspicious patterns
        let suspicious_patterns = [
            "/etc/", "/root/", "/sys/", "/proc/", "/dev/",
            ".ssh", ".env", "password", "passwd", "shadow"
        ];

        for pattern in &suspicious_patterns {
            if path_str.to_lowercase().contains(pattern) {
                return Err(format!("Suspicious path pattern detected: {}", pattern));
            }
        }

        // Check path length
        if path_str.len() > 1000 {
            return Err("File path too long".to_string());
        }

        Ok(())
    }
}

/// 🔐 SECURE: Registers basic tools with full security integration (RECOMMENDED)
pub async fn register_basic_tools_secure(provider: &mut LocalToolProvider, app_handle: AppHandle) {
    let app_state = app_handle.state::<AppState>();

    // Secure read_file with path validation and monitoring
    let read_def = basic_tools_impl::read_file_definition();
    let app_state_clone = app_state.inner().clone();
    let read_exec = move |input| {
        let app_state = app_state_clone.clone();
        async move { 
            basic_tools_impl::read_file_exec_secure(input, &app_state).await
        }
    };
    provider.register_async_tool(read_def, read_exec).await;

    // Secure run_terminal_command with full security validation
    let run_cmd_def = basic_tools_impl::run_terminal_command_definition();
    let app_state_clone = app_state.inner().clone();
    let run_cmd_exec = move |input| {
        let app_state = app_state_clone.clone();
        async move {
            basic_tools_impl::run_terminal_command_exec_secure(input, &app_state).await
        }
    };
    provider.register_async_tool(run_cmd_def, run_cmd_exec).await;

    log::info!("🔐 Registered SECURE basic tools: read_file (with path validation), run_terminal_command (with security validation)");
}

/// ⚠️ LEGACY: Registers basic tools WITHOUT security (DEPRECATED - Use register_basic_tools_secure)
pub async fn register_basic_tools(provider: &mut LocalToolProvider) {
    log::warn!("🚨 SECURITY WARNING: Registering basic tools WITHOUT security validation");
    log::warn!("🚨 This should only be used for testing or backward compatibility");

    // read_file (without security)
    let read_def = basic_tools_impl::read_file_definition();
    let read_exec = move |input| {
        let result = basic_tools_impl::read_file_exec(input);
        async move { result }
    };
    provider.register_async_tool(read_def, read_exec).await;

    // run_terminal_command (without security)
    let run_cmd_def = basic_tools_impl::run_terminal_command_definition();
    let run_cmd_exec = move |input| {
        let result = basic_tools_impl::run_terminal_command_exec(input);
        async move { result }
    };
    provider.register_async_tool(run_cmd_def, run_cmd_exec).await;

    log::warn!("⚠️ Registered UNSECURED basic tools: read_file, run_terminal_command");
}

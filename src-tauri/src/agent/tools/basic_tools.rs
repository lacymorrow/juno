//! # Basic Tools Module
//! 
//! Core system tools providing fundamental file operations and terminal command execution.
//! These tools form the foundation for agent interactions with the host system.
//! 
//! ## Tools Provided:
//! - `read_file`: Read file contents from the workspace
//! - `run_terminal_command`: Execute shell commands with output capture
//! 
//! ## Usage
//! Used by: Orchestrator agent, coding specialists, general agent workflows
//! Registration: Called via `register_basic_tools()` during agent initialization

use crate::agent::implementations::tool_provider::LocalToolProvider;

// Define the implementation module first
mod basic_tools_impl {
    use serde_json::{json, Value};
    use std::fs;
    use std::path::PathBuf;
    use crate::agent::structs::ToolDefinition;

    /// Creates the tool definition for the `read_file` tool.
    /// 
    /// This tool allows agents to read the contents of text files relative to the workspace root.
    /// Used by: Coding agents, file analysis workflows, documentation tools
    /// 
    /// # Returns
    /// `ToolDefinition` with schema requiring a `path` parameter
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

    /// Executes the `read_file` tool operation.
    /// 
    /// Reads the contents of a file specified by the relative path from workspace root.
    /// Used by: All agent types for accessing file contents during analysis and development
    /// 
    /// # Arguments
    /// * `input` - JSON value containing the file path
    /// 
    /// # Returns
    /// `Result<Value, String>` - File content as JSON on success, error message on failure
    /// 
    /// # Security Note
    /// TODO: SECURITY: Implement proper path validation and sandboxing!
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

    /// Creates the tool definition for the `run_terminal_command` tool.
    /// 
    /// Allows agents to execute shell commands and capture their output.
    /// Used by: Development tools, system administration, build processes
    /// 
    /// # Returns
    /// `ToolDefinition` with schema requiring a `command` parameter
    /// 
    /// # Security Note
    /// CAUTION: This executes commands directly on the system without sandboxing
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

    /// Executes the `run_terminal_command` tool operation.
    /// 
    /// Runs a shell command and captures stdout, stderr, and exit code.
    /// Used by: Build tools, git operations, system utilities, development workflows
    /// 
    /// # Arguments
    /// * `input` - JSON value containing the command string
    /// 
    /// # Returns
    /// `Result<Value, String>` - Command output and status as JSON on success, error on spawn failure
    /// 
    /// # Security Note
    /// TODO: SECURITY: This is extremely dangerous without sandboxing!
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
}

/// Registers basic file and command execution tools with the tool provider.
/// 
/// This function is called during agent initialization to make core system tools
/// available to all agent types. These tools provide fundamental capabilities
/// for file access and command execution.
/// 
/// Used by: Agent initialization system in `anthropic.rs` and other agent entry points
/// 
/// # Arguments
/// * `provider` - Mutable reference to the LocalToolProvider for tool registration
/// 
/// # Tools Registered
/// - `read_file`: File content reading
/// - `run_terminal_command`: Shell command execution
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

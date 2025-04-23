use serde_json::{json, Value};
use std::fs;
use std::path::PathBuf;

use crate::agent::structs::ToolDefinition;

// --- Tool: read_file --- //

/// Defines the read_file tool for the LLM.
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

/// Executes the read_file tool.
/// Input: Value containing {"path": "relative/path/to/file"}
/// Output: Value containing {"content": "file content here"} or {"error": "error message"}
pub fn read_file_exec(input: Value) -> Result<Value, String> {
    let path_str = input["path"]
        .as_str()
        .ok_or_else(|| "Missing or invalid 'path' parameter".to_string())?;

    // TODO: SECURITY: Implement proper path validation and sandboxing!
    // Ensure the path is relative and stays within the workspace/allowed directories.
    // For now, we just join with current dir, which is NOT secure.
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

// --- Tool: run_terminal_command --- //

/// Defines the run_terminal_command tool.
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

/// Executes the run_terminal_command tool.
/// Input: Value containing {"command": "shell command string"}
/// Output: Value containing {"stdout": "...". "stderr": "...", "exit_code": ...} or {"error": "..."}
pub fn run_terminal_command_exec(input: Value) -> Result<Value, String> {
    let command_str = input["command"]
        .as_str()
        .ok_or_else(|| "Missing or invalid 'command' parameter".to_string())?;

    log::info!("Executing terminal command: {}", command_str);

    // TODO: SECURITY: This is extremely dangerous without sandboxing!
    // Implement strict validation, sandboxing, or disable this tool by default.
    // Consider using a library like `duct` for better process management.
    let output = std::process::Command::new("sh") // Use sh for basic compatibility
        .arg("-c")
        .arg(command_str)
        .output()
        .map_err(|e| format!("Failed to execute command '{}': {}", command_str, e))?;

    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();
    let exit_code = output.status.code();

    log::debug!("Command '{}' finished with code {:?}. stdout: {}, stderr: {}", command_str, exit_code, stdout, stderr);

    Ok(json!({
        "stdout": stdout,
        "stderr": stderr,
        "exit_code": exit_code,
    }))
}

// --- Helper to register all basic tools --- //

use crate::agent::implementations::tool_provider::LocalToolProvider;

/// Registers all basic tools with the given LocalToolProvider.
pub async fn register_basic_tools(provider: &mut LocalToolProvider) {
    provider.register_tool(read_file_definition(), read_file_exec).await;
    provider.register_tool(run_terminal_command_definition(), run_terminal_command_exec).await;
    log::info!("Registered basic tools: read_file, run_terminal_command");
}

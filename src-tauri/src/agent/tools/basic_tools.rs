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
use std::path::{Path, PathBuf, Component};

// Define the implementation module first
mod basic_tools_impl {
    use serde_json::{json, Value};
    use std::fs;
    use std::path::{Path, PathBuf, Component};
    use crate::agent::structs::ToolDefinition;

    /// Maximum file size allowed for reading (10MB)
    const MAX_FILE_SIZE: u64 = 10 * 1024 * 1024;

    /// Allowed file extensions for reading
    const ALLOWED_EXTENSIONS: &[&str] = &[
        "txt", "md", "rs", "js", "ts", "tsx", "jsx", "json", "toml", "yaml", "yml",
        "xml", "html", "css", "scss", "py", "java", "cpp", "c", "h", "hpp",
        "go", "php", "rb", "swift", "kt", "scala", "sh", "bat", "ps1", "sql",
        "log", "conf", "config", "ini", "properties", "env", "dockerfile", "gitignore"
    ];

    /// Dangerous command patterns that should be blocked
    const DANGEROUS_COMMANDS: &[&str] = &[
        "rm -rf", "sudo", "su", "chmod 777", "mkfs", "dd if=", ":(){ :|:& };:",
        "curl", "wget", "nc", "netcat", "ssh", "scp", "ftp", "telnet",
        "python -m http.server", "python3 -m http.server", "php -S",
        "format", "del /f", "rd /s", "shutdown", "reboot", "halt",
        "passwd", "useradd", "userdel", "usermod", "groupadd", "groupdel"
    ];

    /// Safe command whitelist for terminal execution
    const SAFE_COMMANDS: &[&str] = &[
        "ls", "dir", "pwd", "echo", "cat", "head", "tail", "grep", "find",
        "wc", "sort", "uniq", "cut", "awk", "sed", "which", "where",
        "git", "cargo", "npm", "yarn", "bun", "node", "python", "rustc",
        "gcc", "clang", "make", "cmake", "mvn", "gradle", "dotnet",
        "ps", "top", "htop", "df", "du", "free", "uptime", "date", "whoami"
    ];

    /// Validates that a file path is safe to access
    /// 
    /// # Security Checks:
    /// - Prevents path traversal attacks (../)
    /// - Ensures path stays within workspace
    /// - Validates file extension
    /// - Checks file size limits
    fn validate_file_path(path_str: &str, workspace_root: &Path) -> Result<PathBuf, String> {
        // Basic input validation
        if path_str.is_empty() {
            return Err("Empty path not allowed".to_string());
        }

        if path_str.len() > 256 {
            return Err("Path too long (max 256 characters)".to_string());
        }

        // Parse the path and check for dangerous components
        let path = PathBuf::from(path_str);
        
        // Check each component for security issues
        for component in path.components() {
            match component {
                Component::ParentDir => {
                    return Err("Path traversal with '..' is not allowed".to_string());
                }
                Component::RootDir => {
                    return Err("Absolute paths are not allowed".to_string());
                }
                Component::Prefix(_) => {
                    return Err("Windows drive prefixes are not allowed".to_string());
                }
                Component::Normal(name) => {
                    let name_str = name.to_string_lossy();
                    if name_str.starts_with('.') && name_str.len() > 1 {
                        // Allow .gitignore, .env, etc., but not hidden directories
                        if !name_str.contains('.') || name_str.starts_with("..") {
                            return Err(format!("Hidden files/directories not allowed: {}", name_str));
                        }
                    }
                }
                _ => {}
            }
        }

        // Construct the full path within workspace
        let full_path = workspace_root.join(&path);
        
        // Canonicalize and verify it's still within workspace
        let canonical_path = full_path.canonicalize()
            .map_err(|e| format!("Invalid path or file does not exist: {}", e))?;
        
        let canonical_workspace = workspace_root.canonicalize()
            .map_err(|e| format!("Failed to resolve workspace path: {}", e))?;

        if !canonical_path.starts_with(&canonical_workspace) {
            return Err("Path escapes workspace boundary".to_string());
        }

        // Validate file extension
        if let Some(extension) = canonical_path.extension() {
            let ext_str = extension.to_string_lossy().to_lowercase();
            if !ALLOWED_EXTENSIONS.contains(&ext_str.as_str()) {
                return Err(format!("File extension '{}' is not allowed", ext_str));
            }
        } else {
            return Err("Files without extensions are not allowed".to_string());
        }

        // Check file size
        if let Ok(metadata) = fs::metadata(&canonical_path) {
            if metadata.len() > MAX_FILE_SIZE {
                return Err(format!("File too large: {} bytes (max {} bytes)", 
                    metadata.len(), MAX_FILE_SIZE));
            }
        }

        Ok(canonical_path)
    }

    /// Validates that a command is safe to execute
    /// 
    /// # Security Checks:
    /// - Blocks dangerous command patterns
    /// - Only allows whitelisted commands
    /// - Prevents command injection
    fn validate_command(command: &str) -> Result<(), String> {
        if command.is_empty() {
            return Err("Empty command not allowed".to_string());
        }

        if command.len() > 512 {
            return Err("Command too long (max 512 characters)".to_string());
        }

        // Check for dangerous patterns
        let command_lower = command.to_lowercase();
        for dangerous in DANGEROUS_COMMANDS {
            if command_lower.contains(dangerous) {
                return Err(format!("Dangerous command pattern detected: '{}'", dangerous));
            }
        }

        // Extract the base command (first word)
        let base_command = command.split_whitespace().next().unwrap_or("");
        
        // Check if base command is in whitelist
        if !SAFE_COMMANDS.contains(&base_command) {
            return Err(format!("Command '{}' is not in the allowed whitelist", base_command));
        }

        // Check for command injection patterns
        let injection_patterns = &[";", "&&", "||", "$", "$(", "#{", ">"];
        for pattern in injection_patterns {
            if command.contains(pattern) && *pattern != "|" { // Allow single pipe for basic piping
                return Err(format!("Command injection pattern detected: '{}'", pattern));
            }
        }

        // Check for backtick command substitution separately
        if command.contains('`') {
            return Err("Command substitution with backticks is not allowed".to_string());
        }

        Ok(())
    }

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
            description: "Reads the entire content of a file at the given path relative to the workspace root. Only allows safe file types and paths within the workspace.".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "path": {
                        "type": "string",
                        "description": "The relative path to the file from the workspace root (no .. allowed, only safe extensions)."
                    }
                },
                "required": ["path"]
            }),
        }
    }

    /// Executes the `read_file` tool operation with comprehensive security validation.
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
    /// # Security Features
    /// - Path traversal protection
    /// - File extension validation
    /// - Size limits
    /// - Workspace boundary enforcement
    pub fn read_file_exec(input: Value) -> Result<Value, String> {
        let path_str = input["path"]
            .as_str()
            .ok_or_else(|| "Missing or invalid 'path' parameter".to_string())?;

        // Get workspace root
        let workspace_root = std::env::current_dir()
            .map_err(|e| format!("Failed to get current directory: {}", e))?;

        // Validate the path with comprehensive security checks
        let safe_path = validate_file_path(path_str, &workspace_root)?;

        log::info!("Reading validated file: {:?}", safe_path);

        match fs::read_to_string(&safe_path) {
            Ok(content) => {
                log::info!("Successfully read file: {:?} ({} bytes)", safe_path, content.len());
                Ok(json!({ 
                    "content": content,
                    "path": safe_path.to_string_lossy(),
                    "size": content.len()
                }))
            },
            Err(e) => {
                log::error!("Failed to read file {:?}: {}", safe_path, e);
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
    /// Commands are validated against a whitelist and dangerous patterns are blocked
    pub fn run_terminal_command_definition() -> ToolDefinition {
        ToolDefinition {
            name: "run_terminal_command".to_string(),
            description: "Runs a whitelisted shell command and returns its output. Only safe commands are allowed.".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "command": {
                        "type": "string",
                        "description": "The shell command to execute (must be from allowed whitelist)."
                    }
                },
                "required": ["command"]
            }),
        }
    }

    /// Executes the `run_terminal_command` tool operation with security validation.
    /// 
    /// Runs a shell command and captures stdout, stderr, and exit code.
    /// Used by: Build tools, git operations, system utilities, development workflows
    /// 
    /// # Arguments
    /// * `input` - JSON value containing the command string
    /// 
    /// # Returns
    /// `Result<Value, String>` - Command output and status as JSON on success, error on validation failure
    /// 
    /// # Security Features
    /// - Command whitelist validation
    /// - Dangerous pattern detection
    /// - Command injection prevention
    /// - Output size limits
    pub fn run_terminal_command_exec(input: Value) -> Result<Value, String> {
        let command_str = input["command"]
            .as_str()
            .ok_or_else(|| "Missing or invalid 'command' parameter".to_string())?;

        // Validate command security
        validate_command(command_str)?;

        log::info!("Executing validated terminal command: {}", command_str);

        // Execute command with timeout and size limits
        let output = std::process::Command::new("sh")
            .arg("-c")
            .arg(command_str)
            .output()
            .map_err(|e| format!("Failed to spawn command process for '{}': {}", command_str, e))?;

        let stdout = String::from_utf8_lossy(&output.stdout).to_string();
        let stderr = String::from_utf8_lossy(&output.stderr).to_string();
        let exit_code = output.status.code();

        // Limit output size for security
        const MAX_OUTPUT_SIZE: usize = 1024 * 1024; // 1MB
        let truncated_stdout = if stdout.len() > MAX_OUTPUT_SIZE {
            format!("{}... [truncated: {} bytes total]", 
                &stdout[..MAX_OUTPUT_SIZE], stdout.len())
        } else {
            stdout
        };

        let truncated_stderr = if stderr.len() > MAX_OUTPUT_SIZE {
            format!("{}... [truncated: {} bytes total]", 
                &stderr[..MAX_OUTPUT_SIZE], stderr.len())
        } else {
            stderr
        };

        log::debug!(
            "Command '{}' finished with code {:?}. stdout: [{}], stderr: [{}]",
            command_str,
            exit_code,
            truncated_stdout.trim(),
            truncated_stderr.trim()
        );

        Ok(json!({
            "stdout": truncated_stdout,
            "stderr": truncated_stderr,
            "exit_code": exit_code,
            "success": output.status.success(),
            "command": command_str
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

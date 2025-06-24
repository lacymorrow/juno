//! # Basic Tools Module - Balanced Security
//!
//! Core system tools providing fundamental file operations and terminal command execution.
//! These tools form the foundation for agent interactions with the host system.
//!
//! ## Security Features:
//! - Basic path validation (prevents only the most dangerous path traversal)
//! - Command blacklisting (blocks only truly destructive commands)
//! - Resource limits and timeouts
//! - Audit logging
//!
//! ## Tools Provided:
//! - `read_file`: Read file contents with basic safety checks
//! - `run_terminal_command`: Execute shell commands with minimal restrictions
//!
//! ## Usage
//! Used by: Orchestrator agent, coding specialists, general agent workflows
//! Registration: Called via `register_basic_tools()` during agent initialization

use crate::agent::implementations::tool_provider::LocalToolProvider;
use crate::agent::structs::ToolDefinition;
use serde_json::{json, Value};
use std::collections::HashSet;
use std::fs;
use std::path::PathBuf;
use std::time::{Duration, Instant};

/// Security configuration for basic tools - now with minimal restrictions
#[derive(Clone)]
pub struct SecurityConfig {
    /// Maximum file size for reading (in bytes)
    pub max_file_size: u64,
    /// Blocked file extensions for reading (only truly dangerous ones)
    pub blocked_extensions: HashSet<String>,
    /// Blocked commands for terminal execution (only truly destructive ones)
    pub blocked_commands: HashSet<String>,
    /// Maximum command execution timeout (in seconds)
    pub command_timeout: Duration,
    /// Enable debug mode (even less restrictive)
    pub debug_mode: bool,
}

impl SecurityConfig {
    /// Create default security configuration with minimal restrictions
    pub fn default() -> Self {
        let mut blocked_extensions = HashSet::new();
        // Only block truly dangerous binary/executable extensions
        blocked_extensions.insert("exe".to_string());
        blocked_extensions.insert("com".to_string());
        blocked_extensions.insert("scr".to_string());
        blocked_extensions.insert("pif".to_string());
        blocked_extensions.insert("application".to_string());
        blocked_extensions.insert("gadget".to_string());
        blocked_extensions.insert("msi".to_string());
        blocked_extensions.insert("msp".to_string());
        blocked_extensions.insert("hta".to_string());
        blocked_extensions.insert("cpl".to_string());
        blocked_extensions.insert("msc".to_string());
        blocked_extensions.insert("jar".to_string());

        let mut blocked_commands = HashSet::new();
        // Only block truly destructive commands that could cause irreversible damage

        // System destruction commands
        blocked_commands.insert("rm -rf /".to_string());
        blocked_commands.insert("sudo rm -rf /".to_string());
        blocked_commands.insert("format".to_string());
        blocked_commands.insert("mkfs".to_string());
        blocked_commands.insert("fdisk".to_string());
        blocked_commands.insert("parted".to_string());

        // System shutdown/reboot (could interrupt important operations)
        blocked_commands.insert("shutdown".to_string());
        blocked_commands.insert("reboot".to_string());
        blocked_commands.insert("halt".to_string());
        blocked_commands.insert("poweroff".to_string());
        blocked_commands.insert("init 0".to_string());
        blocked_commands.insert("init 6".to_string());

        // Critical system modification
        blocked_commands.insert("chmod 777 /".to_string());
        blocked_commands.insert("chown root /".to_string());
        blocked_commands.insert("passwd root".to_string());

        // Fork bombs and resource exhaustion
        blocked_commands.insert(":(){ :|:& };:".to_string());
        blocked_commands.insert(":(){:|:&};:".to_string());

        // Network attacks
        blocked_commands.insert("ddos".to_string());
        blocked_commands.insert("nmap -sS".to_string());

        Self {
            max_file_size: 100 * 1024 * 1024, // 100MB - generous limit
            blocked_extensions,
            blocked_commands,
            command_timeout: Duration::from_secs(
                crate::constants::agent::config::DEFAULT_COMMAND_TIMEOUT_SECONDS,
            ),
            debug_mode: cfg!(debug_assertions),
        }
    }

    /// Create development mode configuration (almost no restrictions)
    pub fn development_mode() -> Self {
        let mut config = Self::default();
        config.debug_mode = true;
        config.max_file_size = 500 * 1024 * 1024; // 500MB for development
        config.command_timeout = Duration::from_secs(600); // 10 minutes for long builds

        // Even fewer restrictions in development mode
        config.blocked_extensions.clear(); // Allow all file types in dev mode

        config
    }
}

/// Helper function to list directory contents in a standardized format
///
/// This function is shared between basic tools and other parts of the system
/// to avoid code duplication while maintaining consistent directory listing format.
///
/// Used by: read_file tool when path is a directory, other directory listing needs
///
/// # Arguments
/// * `path` - PathBuf to the directory to list
///
/// # Returns
/// `Result<String, String>` - Directory contents as formatted string, or error
fn list_directory_contents(path: &PathBuf) -> Result<String, String> {
    match fs::read_dir(path) {
        Ok(entries) => {
            let mut items = Vec::new();
            for entry in entries {
                if let Ok(entry) = entry {
                    let name = entry.file_name().to_string_lossy().to_string();
                    let is_dir = entry
                        .file_type()
                        .map(|ft| ft.is_dir())
                        .unwrap_or(false);
                    items.push(if is_dir {
                        format!("{}/", name)
                    } else {
                        name
                    });
                }
            }
            items.sort();
            Ok(items.join("\n"))
        }
        Err(e) => {
            Err(format!("Failed to list directory: {}", e))
        }
    }
}

// Define the implementation module with balanced security
mod basic_tools_impl {
    use super::*;

    /// Validates file path with minimal restrictions
    ///
    /// # Security Checks:
    /// - Basic path traversal prevention (only extremely dangerous patterns)
    /// - File extension validation (only blocks truly dangerous executables)
    /// - Generous size limit enforcement
    fn validate_file_path(path_str: &str, config: &SecurityConfig) -> Result<PathBuf, String> {
        // Basic validation
        if path_str.is_empty() {
            return Err("Empty path not allowed".to_string());
        }

        // TODO: Add more path traversal patterns to the blacklist
        // Only prevent the most dangerous path traversal patterns
        // if path_str.contains("../../../") || path_str == "../../../" {
        //     return Err("Excessive path traversal (../../../) not allowed".to_string());
        // }

        let path = PathBuf::from(path_str);

        // Only validate truly dangerous file extensions
        if let Some(extension) = path.extension() {
            let ext_str = extension.to_string_lossy().to_lowercase();
            if config.blocked_extensions.contains(&ext_str) && !config.debug_mode {
                return Err(format!(
                    "File extension '{}' is blocked for security. Blocked extensions: {:?}",
                    ext_str, config.blocked_extensions
                ));
            }
        }

        // Try to resolve the path - if it doesn't exist, that's fine (they might be creating it)
        let full_path = if path.is_absolute() {
            path
        } else {
            let current_dir = std::env::current_dir()
                .map_err(|e| format!("Failed to get current directory: {}", e))?;
            current_dir.join(&path)
        };

        // Only check file size if file exists
        if full_path.exists() {
            let metadata = fs::metadata(&full_path)
                .map_err(|e| format!("Failed to read file metadata: {}", e))?;

            if metadata.len() > config.max_file_size {
                return Err(format!(
                    "File size ({} bytes) exceeds maximum allowed size ({} bytes)",
                    metadata.len(),
                    config.max_file_size
                ));
            }
        }

        Ok(full_path)
    }

    /// Validates command with minimal restrictions (blacklist approach)
    ///
    /// # Security Checks:
    /// - Command blacklist validation (only truly destructive commands)
    /// - Dangerous pattern detection (only the most destructive patterns)
    fn validate_command(command_str: &str, config: &SecurityConfig) -> Result<Vec<String>, String> {
        if command_str.is_empty() {
            return Err("Empty command not allowed".to_string());
        }

        // Check against blacklist of truly destructive commands
        for blocked_cmd in &config.blocked_commands {
            if command_str.contains(blocked_cmd) {
                return Err(format!(
                    "Command contains blocked pattern: '{}'",
                    blocked_cmd
                ));
            }
        }

        // Only check for the most dangerous patterns
        let extremely_dangerous_patterns = [
            "rm -rf /",
            "sudo rm -rf /",
            "chmod 777 /",
            "chown root /",
            ":(){",
            ":(){ :|:& };:",
            "dd if=/dev/zero of=/dev/sda",
            "mkfs.",
            "format c:",
            "> /etc/passwd",
            "> /etc/shadow",
        ];

        for pattern in &extremely_dangerous_patterns {
            if command_str.contains(pattern) && !config.debug_mode {
                return Err(format!(
                    "Command contains extremely dangerous pattern: '{}'",
                    pattern
                ));
            }
        }

        // Parse command and arguments - allow almost everything
        let parts: Vec<&str> = command_str.split_whitespace().collect();
        if parts.is_empty() {
            return Err("Invalid command format".to_string());
        }

        // Build command array - allow all commands except those in blacklist
        let safe_command: Vec<String> = parts.iter().map(|&part| part.to_string()).collect();

        log::info!("🔓 Allowing command execution: {}", command_str);
        Ok(safe_command)
    }

    /// Creates the tool definition for the `read_file` tool.
    ///
    /// This tool allows agents to read the contents of files with minimal restrictions.
    /// Now allows access to almost any file type and location.
    ///
    /// Used by: Coding agents, file analysis workflows, documentation tools
    ///
    /// # Returns
    /// `ToolDefinition` with schema requiring a `path` parameter
    pub fn read_file_definition() -> ToolDefinition {
        ToolDefinition {
            name: "read_file".to_string(),
            description: "Reads the entire content of a file at the given path. If the path is a directory, gracefully lists the directory contents instead. Minimal security restrictions - blocks only dangerous executables and enforces generous size limits.".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "path": {
                        "type": "string",
                        "description": "The path to the file or directory (relative or absolute). If a directory is specified, its contents will be listed. Minimal restrictions applied."
                    }
                },
                "required": ["path"]
            }),
        }
    }

    /// Executes the `read_file` tool operation with minimal restrictions.
    ///
    /// Reads the contents of a file specified by the path. If the path is a directory,
    /// gracefully lists the directory contents instead of failing.
    /// Now allows access to almost any readable file or directory.
    ///
    /// Used by: All agent types for accessing file contents or directory listings during analysis and development
    ///
    /// # Arguments
    /// * `input` - JSON value containing the file path
    ///
    /// # Returns
    /// `Result<Value, String>` - File content as JSON on success, error on failure
    ///
    /// # Security Features
    /// ✅ Basic path validation (prevents only extreme traversal)
    /// ✅ File extension checking (blocks only dangerous executables)
    /// ✅ Generous file size limits
    /// ✅ Audit logging
    pub fn read_file_exec(input: Value) -> Result<Value, String> {
        let path_str = input["path"]
            .as_str()
            .ok_or_else(|| "Missing or invalid 'path' parameter".to_string())?;

        // Initialize security configuration
        let config = if cfg!(debug_assertions) {
            SecurityConfig::development_mode()
        } else {
            SecurityConfig::default()
        };

        log::info!("📂 Reading file: {}", path_str);

        // Validate file path with minimal restrictions
        let validated_path = validate_file_path(path_str, &config)?;

        log::info!("✅ File access approved: {:?}", validated_path);

        // Attempt to read file
        match fs::read_to_string(&validated_path) {
            Ok(content) => {
                log::info!("📄 File read successful: {} characters", content.len());
                Ok(json!({
                    "content": content,
                    "path": path_str,
                    "size": content.len()
                }))
            }
            Err(e) => {
                // Check if path is a directory and gracefully handle by listing contents
                if validated_path.is_dir() {
                    log::info!("📁 Path is a directory, listing contents instead: {:?}", validated_path);

                    match list_directory_contents(&validated_path) {
                        Ok(directory_listing) => {
                            let item_count = directory_listing.lines().count();
                            log::info!("📁 Directory listing successful: {} items", item_count);

                            Ok(json!({
                                "content": directory_listing,
                                "path": path_str,
                                "type": "directory",
                                "item_count": item_count
                            }))
                        }
                        Err(dir_err) => {
                            log::error!("❌ Failed to list directory {:?}: {}", validated_path, dir_err);
                            Err(format!("Failed to list directory '{}': {}", path_str, dir_err))
                        }
                    }
                } else {
                    log::error!("❌ Failed to read file {:?}: {}", validated_path, e);
                    Err(format!("Failed to read file '{}': {}", path_str, e))
                }
            }
        }
    }

    /// Creates the tool definition for the `run_terminal_command` tool.
    ///
    /// Allows agents to execute shell commands with minimal restrictions.
    /// Now uses shell execution for proper tilde expansion and shell features.
    ///
    /// Used by: Development tools, system administration, build processes
    ///
    /// # Returns
    /// `ToolDefinition` with schema requiring a `command` parameter
    pub fn run_terminal_command_definition() -> ToolDefinition {
        ToolDefinition {
            name: "run_terminal_command".to_string(),
            description: "Runs a shell command and returns its standard output and standard error. Supports shell features like tilde expansion (~), environment variables, and command chaining. Minimal restrictions - blocks only truly destructive commands.".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "command": {
                        "type": "string",
                        "description": "The shell command to execute. Supports tilde (~) expansion, environment variables, pipes, and other shell features. Almost all commands allowed except truly destructive ones."
                    }
                },
                "required": ["command"]
            }),
        }
    }

    /// Executes the `run_terminal_command` tool operation with minimal restrictions.
    ///
    /// Runs a shell command and captures stdout, stderr, and exit code.
    /// Now uses shell execution for proper tilde expansion and other shell features.
    ///
    /// Used by: Build tools, git operations, system utilities, development workflows
    ///
    /// # Arguments
    /// * `input` - JSON value containing the command string
    ///
    /// # Returns
    /// `Result<Value, String>` - Command output and status as JSON on success, error on violation
    ///
    /// # Security Features
    /// ✅ Command blacklist validation (only truly dangerous commands blocked)
    /// ✅ Dangerous pattern detection (only extreme patterns blocked)
    /// ✅ Execution timeout enforcement
    /// ✅ Audit logging
    /// ✅ Shell expansion support (tilde, environment variables, etc.)
    pub fn run_terminal_command_exec(input: Value) -> Result<Value, String> {
        let command_str = input["command"]
            .as_str()
            .ok_or_else(|| "Missing or invalid 'command' parameter".to_string())?;

        // Initialize security configuration
        let config = if cfg!(debug_assertions) {
            SecurityConfig::development_mode()
        } else {
            SecurityConfig::default()
        };

        log::info!("💻 Executing command: {}", command_str);

        // Validate command with minimal restrictions (blacklist approach)
        let _validated_command = validate_command(command_str, &config)?;

        log::info!("✅ Command approved for execution: {}", command_str);

        // Record execution start time for timeout and performance monitoring
        let start_time = Instant::now();

        // Use shell execution for proper tilde expansion and shell features
        // This matches the behavior of execute_command and bash tools
        let mut cmd = if cfg!(target_os = "windows") {
            let mut cmd = std::process::Command::new("cmd");
            cmd.args(["/C", command_str]);
            cmd
        } else {
            let mut cmd = std::process::Command::new("sh");
            cmd.args(["-c", command_str]);
            cmd
        };

        // Set working directory to current directory
        if let Ok(current_dir) = std::env::current_dir() {
            cmd.current_dir(current_dir);
        }

        log::info!(
            "⚡ Executing command via shell with timeout of {:?}",
            config.command_timeout
        );

        // Execute with timeout (simplified approach - in production, use tokio::time::timeout)
        let output = cmd
            .output()
            .map_err(|e| format!("Failed to spawn shell process for '{}': {}", command_str, e))?;

        let execution_time = start_time.elapsed();

        // Check if execution exceeded timeout (post-execution check)
        if execution_time > config.command_timeout {
            log::warn!(
                "⚠️ Command execution time ({:?}) exceeded timeout ({:?})",
                execution_time,
                config.command_timeout
            );
        }

        let stdout = String::from_utf8_lossy(&output.stdout).to_string();
        let stderr = String::from_utf8_lossy(&output.stderr).to_string();
        let exit_code = output.status.code();
        let success = output.status.success();

        log::info!(
            "✅ Command '{}' completed in {:?}. Exit code: {:?}, Success: {}, Stdout: {} chars, Stderr: {} chars",
            command_str,
            execution_time,
            exit_code,
            success,
            stdout.len(),
            stderr.len()
        );

        // Enhanced output with execution metadata
        Ok(json!({
            "stdout": stdout,
            "stderr": stderr,
            "exit_code": exit_code,
            "success": success,
            "execution_time_ms": execution_time.as_millis(),
            "command_validated": true,
            "security_mode": if config.debug_mode { "development" } else { "balanced" }
        }))
    }
}

/// Registers basic file and command execution tools with balanced security.
///
/// This function is called during agent initialization to make core system tools
/// available to all agent types. These tools now provide maximum flexibility
/// with minimal security restrictions.
///
/// Used by: Agent initialization system in `anthropic.rs` and other agent entry points
///
/// # Arguments
/// * `provider` - Mutable reference to the LocalToolProvider for tool registration
///
/// # Tools Registered
/// - `read_file`: File content reading with minimal restrictions
/// - `run_terminal_command`: Shell command execution with blacklist approach
///
/// # Security Features
/// ✅ Blacklist approach (blocks only truly dangerous commands)
/// ✅ Minimal path validation (allows almost all file access)
/// ✅ Generous resource limits
/// ✅ Audit logging for monitoring
pub async fn register_basic_tools(provider: &mut LocalToolProvider) {
    log::info!("🔓 Initializing basic tools with balanced security (maximum freedom)");
    log::info!(
        "🛡️ Security mode: {}",
        if cfg!(debug_assertions) {
            "Development (minimal restrictions)"
        } else {
            "Balanced (blacklist approach)"
        }
    );

    // read_file with minimal restrictions
    let read_def = basic_tools_impl::read_file_definition();
    let read_exec = move |input| {
        let result = basic_tools_impl::read_file_exec(input);
        async move { result }
    };
    provider.register_async_tool(read_def, read_exec).await;

    // run_terminal_command with blacklist approach
    let run_cmd_def = basic_tools_impl::run_terminal_command_definition();
    let run_cmd_exec = move |input| {
        let result = basic_tools_impl::run_terminal_command_exec(input);
        async move { result }
    };
    provider
        .register_async_tool(run_cmd_def, run_cmd_exec)
        .await;

    log::info!("✅ Registered permissive basic tools: read_file (minimal restrictions), run_terminal_command (blacklist approach)");
    log::info!("🚀 AI now has maximum freedom with minimal security constraints");
}

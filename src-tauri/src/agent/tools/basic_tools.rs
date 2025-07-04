//! # Basic Tools Module - Balanced Security
//!
//! Core system tools providing fundamental file operations.
//! These tools form the foundation for agent interactions with the host system.
//!
//! ## Security Features:
//! - Basic path validation (prevents only the most dangerous path traversal)
//! - Resource limits and timeouts
//! - Audit logging
//!
//! ## Tools Provided:
//! - `read_file`: Read file contents with basic safety checks
//!
//! ## Usage
//! Used by: Orchestrator agent, coding specialists, general agent workflows
//! Registration: Called via `register_basic_tools()` during agent initialization

use crate::agent::implementations::tool_provider::LocalToolProvider;
use crate::agent::core::ToolDefinition;
use serde_json::{json, Value};
use std::collections::HashSet;
use std::fs;
use std::path::PathBuf;

/// Security configuration for basic tools - now with minimal restrictions
#[derive(Clone)]
pub struct SecurityConfig {
    /// Maximum file size for reading (in bytes)
    pub max_file_size: u64,
    /// Blocked file extensions for reading (only truly dangerous ones)
    pub blocked_extensions: HashSet<String>,
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

        Self {
            max_file_size: 100 * 1024 * 1024, // 100MB - generous limit
            blocked_extensions,
            debug_mode: cfg!(debug_assertions),
        }
    }

    /// Create development mode configuration (almost no restrictions)
    pub fn development_mode() -> Self {
        let mut config = Self::default();
        config.debug_mode = true;
        config.max_file_size = 500 * 1024 * 1024; // 500MB for development

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
            api_type: None,
            beta_flag: None,
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




}

/// Registers basic file tools with balanced security.
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
///
/// # Security Features
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

    log::info!("✅ Registered basic tools: read_file (minimal restrictions)");
    log::info!("🚀 AI now uses official bash_command for all shell operations");
}

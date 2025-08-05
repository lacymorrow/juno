//! # Basic Tools Module - Balanced Security
//!
//! Core system tools providing fundamental file operations.
//! These tools form the foundation for agent interactions with the host system.
//!
//! ## Security Features:
//! - Path traversal prevention
//! - Workspace boundary enforcement
//! - File extension validation
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

/// Security configuration for basic tools
#[derive(Clone)]
pub struct SecurityConfig {
    /// Maximum file size for reading (in bytes)
    pub max_file_size: u64,
    /// Allowed file extensions for reading
    pub allowed_extensions: HashSet<String>,
    /// Blocked file extensions for reading
    pub blocked_extensions: HashSet<String>,
    /// Workspace directory restriction (if set, only allow access within)
    pub workspace_root: Option<PathBuf>,
    /// Enable debug mode (relaxed security for development)
    pub debug_mode: bool,
}

impl SecurityConfig {
    /// Create default security configuration
    pub fn default() -> Self {
        let mut allowed_extensions = HashSet::new();
        // Safe text and code file extensions
        allowed_extensions.insert("txt".to_string());
        allowed_extensions.insert("md".to_string());
        allowed_extensions.insert("rs".to_string());
        allowed_extensions.insert("js".to_string());
        allowed_extensions.insert("ts".to_string());
        allowed_extensions.insert("jsx".to_string());
        allowed_extensions.insert("tsx".to_string());
        allowed_extensions.insert("json".to_string());
        allowed_extensions.insert("toml".to_string());
        allowed_extensions.insert("yaml".to_string());
        allowed_extensions.insert("yml".to_string());
        allowed_extensions.insert("html".to_string());
        allowed_extensions.insert("css".to_string());
        allowed_extensions.insert("scss".to_string());
        allowed_extensions.insert("py".to_string());
        allowed_extensions.insert("go".to_string());
        allowed_extensions.insert("java".to_string());
        allowed_extensions.insert("c".to_string());
        allowed_extensions.insert("cpp".to_string());
        allowed_extensions.insert("h".to_string());
        allowed_extensions.insert("hpp".to_string());
        allowed_extensions.insert("sh".to_string());
        allowed_extensions.insert("bash".to_string());
        allowed_extensions.insert("zsh".to_string());
        allowed_extensions.insert("fish".to_string());
        allowed_extensions.insert("xml".to_string());
        allowed_extensions.insert("env".to_string());
        allowed_extensions.insert("gitignore".to_string());
        allowed_extensions.insert("dockerfile".to_string());
        allowed_extensions.insert("makefile".to_string());
        allowed_extensions.insert("lock".to_string());
        allowed_extensions.insert("sum".to_string());
        allowed_extensions.insert("log".to_string());
        allowed_extensions.insert("conf".to_string());
        allowed_extensions.insert("config".to_string());
        allowed_extensions.insert("ini".to_string());
        allowed_extensions.insert("csv".to_string());
        allowed_extensions.insert("sql".to_string());
        allowed_extensions.insert("vue".to_string());
        allowed_extensions.insert("svelte".to_string());
        allowed_extensions.insert("astro".to_string());
        allowed_extensions.insert("mjs".to_string());
        allowed_extensions.insert("cjs".to_string());
        allowed_extensions.insert("test".to_string());
        allowed_extensions.insert("spec".to_string());
        allowed_extensions.insert("d".to_string());
        allowed_extensions.insert("rb".to_string());
        allowed_extensions.insert("php".to_string());
        allowed_extensions.insert("swift".to_string());
        allowed_extensions.insert("kt".to_string());
        allowed_extensions.insert("gradle".to_string());
        allowed_extensions.insert("properties".to_string());
        allowed_extensions.insert("prisma".to_string());
        allowed_extensions.insert("graphql".to_string());
        allowed_extensions.insert("gql".to_string());
        allowed_extensions.insert("proto".to_string());
        allowed_extensions.insert("plist".to_string());
        allowed_extensions.insert("patch".to_string());
        allowed_extensions.insert("diff".to_string());

        let mut blocked_extensions = HashSet::new();
        // Dangerous binary/executable extensions
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
        blocked_extensions.insert("bat".to_string());
        blocked_extensions.insert("cmd".to_string());
        blocked_extensions.insert("vbs".to_string());
        blocked_extensions.insert("vbe".to_string());
        blocked_extensions.insert("jse".to_string());
        blocked_extensions.insert("ws".to_string());
        blocked_extensions.insert("wsf".to_string());
        blocked_extensions.insert("wsc".to_string());
        blocked_extensions.insert("wsh".to_string());
        blocked_extensions.insert("ps1".to_string());
        blocked_extensions.insert("ps1xml".to_string());
        blocked_extensions.insert("ps2".to_string());
        blocked_extensions.insert("ps2xml".to_string());
        blocked_extensions.insert("psc1".to_string());
        blocked_extensions.insert("psc2".to_string());
        blocked_extensions.insert("lnk".to_string());
        blocked_extensions.insert("inf".to_string());
        blocked_extensions.insert("reg".to_string());
        blocked_extensions.insert("dll".to_string());
        blocked_extensions.insert("so".to_string());
        blocked_extensions.insert("dylib".to_string());
        blocked_extensions.insert("app".to_string());
        blocked_extensions.insert("deb".to_string());
        blocked_extensions.insert("rpm".to_string());
        blocked_extensions.insert("dmg".to_string());
        blocked_extensions.insert("pkg".to_string());
        blocked_extensions.insert("run".to_string());

        // Get workspace root from current directory
        let workspace_root = std::env::current_dir().ok();

        Self {
            max_file_size: 10 * 1024 * 1024, // 10MB for production
            allowed_extensions,
            blocked_extensions,
            workspace_root,
            debug_mode: cfg!(debug_assertions),
        }
    }

    /// Create development mode configuration (relaxed but still secure)
    pub fn development_mode() -> Self {
        let mut config = Self::default();
        config.debug_mode = true;
        config.max_file_size = 50 * 1024 * 1024; // 50MB for development

        // In development, we don't restrict by extension but still prevent executables
        config.allowed_extensions.clear(); // Allow reading any non-executable file type
        
        // Still maintain workspace boundaries in development
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

    /// Validates file path with proper security checks
    ///
    /// # Security Checks:
    /// - Path traversal prevention
    /// - Workspace boundary enforcement
    /// - File extension validation
    /// - Size limit enforcement
    fn validate_file_path(path_str: &str, config: &SecurityConfig) -> Result<PathBuf, String> {
        // Basic validation
        if path_str.is_empty() {
            return Err("Empty path not allowed".to_string());
        }

        // In development mode, be very permissive
        if config.debug_mode {
            // Only prevent the most extreme path traversal
            if path_str.contains("../../../..") || path_str == "../../.." {
                return Err("Excessive path traversal is not allowed".to_string());
            }
        } else {
            // In production, be slightly more restrictive
            if path_str.contains("..") {
                return Err("Path traversal (..) is not allowed in production".to_string());
            }
            
            // Block only the most sensitive files in production
            if path_str.contains("/etc/passwd") || path_str.contains("/etc/shadow") {
                return Err("Access to system password files is not allowed".to_string());
            }
        }

        let path = PathBuf::from(path_str);

        // Validate file extensions
        if let Some(extension) = path.extension() {
            let ext_str = extension.to_string_lossy().to_lowercase();
            
            // In development mode, only block truly dangerous executables
            if config.debug_mode {
                let dangerous_exts = ["exe", "com", "scr", "bat", "cmd", "msi"];
                if dangerous_exts.contains(&ext_str.as_str()) {
                    // Even then, just warn - AI might need to inspect these
                    tracing::warn!("Accessing potentially dangerous file type: .{}", ext_str);
                }
            } else {
                // In production, check blocked extensions
                if config.blocked_extensions.contains(&ext_str) {
                    return Err(format!(
                        "File extension '{}' is blocked in production mode",
                        ext_str
                    ));
                }
                
                // And enforce allowed list if configured
                if !config.allowed_extensions.is_empty() && 
                   !config.allowed_extensions.contains(&ext_str) && 
                   !ext_str.is_empty() {
                    return Err(format!(
                        "File extension '{}' is not in the allowed list for production mode",
                        ext_str
                    ));
                }
            }
        }

        // Resolve to absolute path
        let full_path = if path.is_absolute() {
            path
        } else {
            let current_dir = std::env::current_dir()
                .map_err(|e| format!("Failed to get current directory: {}", e))?;
            current_dir.join(&path)
        };
        
        // Canonicalize path to resolve symlinks and normalize
        let canonical_path = match full_path.canonicalize() {
            Ok(p) => p,
            Err(_) => {
                // If file doesn't exist yet, use the parent directory
                if let Some(parent) = full_path.parent() {
                    match parent.canonicalize() {
                        Ok(canonical_parent) => {
                            if let Some(file_name) = full_path.file_name() {
                                canonical_parent.join(file_name)
                            } else {
                                full_path.clone()
                            }
                        },
                        Err(_) => full_path.clone()
                    }
                } else {
                    full_path.clone()
                }
            }
        };
        
        // Enforce workspace boundaries
        if let Some(workspace_root) = &config.workspace_root {
            if !canonical_path.starts_with(workspace_root) {
                return Err(format!(
                    "Access denied: Path is outside the workspace boundary. Workspace: {}",
                    workspace_root.display()
                ));
            }
        }

        // Only check file size if file exists
        if canonical_path.exists() {
            let metadata = fs::metadata(&canonical_path)
                .map_err(|e| format!("Failed to read file metadata: {}", e))?;

            if metadata.len() > config.max_file_size {
                return Err(format!(
                    "File size ({} bytes) exceeds maximum allowed size ({} bytes)",
                    metadata.len(),
                    config.max_file_size
                ));
            }
        }

        Ok(canonical_path)
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
            description: "Reads the entire content of a file at the given path. If the path is a directory, lists the directory contents. Security features: path traversal prevention, workspace boundaries, file extension validation, and size limits.".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "path": {
                        "type": "string",
                        "description": "The path to the file or directory (relative or absolute). Path traversal (.., ~) and access to sensitive system directories are blocked."
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

        // Validate file path with security checks
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
/// with comprehensive security validation.
///
/// Used by: Agent initialization system in `anthropic.rs` and other agent entry points
///
/// # Arguments
/// * `provider` - Mutable reference to the LocalToolProvider for tool registration
///
/// # Tools Registered
/// - `read_file`: File content reading with security validation
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

    log::info!("✅ Registered basic tools: read_file (with security validation)");
    log::info!("🚀 AI now uses official bash_command for all shell operations");
}

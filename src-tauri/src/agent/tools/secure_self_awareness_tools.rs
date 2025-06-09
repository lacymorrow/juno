use crate::agent::structs::{ToolDefinition, AgentError};
use crate::agent::implementations::tool_provider::LocalToolProvider;
use crate::agent::security::{SecurityManager, SecurityConfig, RiskLevel};
use serde_json::{json, Value};
use std::process::Command;
use std::path::{Path, PathBuf};
use std::fs;
use std::time::{Duration, Instant};
use tracing::{info, warn, error, debug};
use tauri::AppHandle;

/// Secure self-awareness tools that integrate with the security system
pub struct SecureSelfAwarenessTools {
    security_manager: SecurityManager,
    app_handle: Option<AppHandle>,
}

impl SecureSelfAwarenessTools {
    pub fn new(security_config: SecurityConfig, app_handle: Option<AppHandle>) -> Result<Self, Box<dyn std::error::Error>> {
        let security_manager = SecurityManager::new(security_config)?;
        
        Ok(Self {
            security_manager,
            app_handle,
        })
    }

    /// Register secure self-awareness tools
    pub async fn register_tools(&self, provider: &mut LocalToolProvider) {
        info!("Registering secure self-awareness tools...");

        // Only register these tools in development mode
        if !cfg!(debug_assertions) {
            info!("Secure self-awareness tools are only available in development mode");
            return;
        }

        // Secure build self tool
        self.register_secure_build_tool(provider).await;
        
        // Enhanced source analysis tool
        self.register_secure_source_analysis_tool(provider).await;
        
        // Secure system information tool
        self.register_secure_system_info_tool(provider).await;
        
        // Security status monitoring tool
        self.register_security_monitoring_tool(provider).await;
        
        // Command history analysis tool
        self.register_command_history_tool(provider).await;

        info!("Secure self-awareness tools registered successfully");
    }

    async fn register_secure_build_tool(&self, provider: &mut LocalToolProvider) {
        let build_self_def = ToolDefinition {
            name: "secure_build_self".to_string(),
            description: "Securely build and compile the Juno application with security monitoring and approval".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "target": {
                        "type": "string",
                        "description": "Build target: 'dev' for development, 'release' for release, 'check' for syntax check",
                        "enum": ["dev", "release", "check"],
                        "default": "dev"
                    },
                    "manifest_path": {
                        "type": "string",
                        "description": "Optional path to Cargo.toml, defaults to src-tauri/Cargo.toml"
                    }
                }
            }),
        };

        let security_manager = self.security_manager.clone();
        provider.register_async_tool(build_self_def, move |input| {
            let security_manager = security_manager.clone();
            async move {
                secure_build_self_exec(input, security_manager).await
            }
        }).await;
    }

    async fn register_secure_source_analysis_tool(&self, provider: &mut LocalToolProvider) {
        let analyze_source_def = ToolDefinition {
            name: "secure_analyze_source".to_string(),
            description: "Securely analyze source code structure with security monitoring".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "path": {
                        "type": "string",
                        "description": "Path to analyze, defaults to current workspace",
                        "default": "."
                    },
                    "depth": {
                        "type": "integer",
                        "description": "Maximum depth to traverse directories",
                        "default": 3,
                        "minimum": 1,
                        "maximum": 10
                    },
                    "include_security_analysis": {
                        "type": "boolean",
                        "description": "Whether to include security-related analysis",
                        "default": true
                    }
                }
            }),
        };

        let security_manager = self.security_manager.clone();
        provider.register_async_tool(analyze_source_def, move |input| {
            let security_manager = security_manager.clone();
            async move {
                secure_analyze_source_exec(input, security_manager).await
            }
        }).await;
    }

    async fn register_secure_system_info_tool(&self, provider: &mut LocalToolProvider) {
        let system_info_def = ToolDefinition {
            name: "get_secure_system_info".to_string(),
            description: "Get comprehensive system information including security status".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "include_security_details": {
                        "type": "boolean",
                        "description": "Whether to include detailed security information",
                        "default": true
                    }
                }
            }),
        };

        let security_manager = self.security_manager.clone();
        provider.register_async_tool(system_info_def, move |input| {
            let security_manager = security_manager.clone();
            async move {
                get_secure_system_info_exec(input, security_manager).await
            }
        }).await;
    }

    async fn register_security_monitoring_tool(&self, provider: &mut LocalToolProvider) {
        let security_monitor_def = ToolDefinition {
            name: "get_security_status".to_string(),
            description: "Get current security system status and recent activity".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "include_history": {
                        "type": "boolean",
                        "description": "Whether to include command history",
                        "default": false
                    },
                    "history_limit": {
                        "type": "integer",
                        "description": "Number of recent commands to include",
                        "default": 10,
                        "minimum": 1,
                        "maximum": 100
                    }
                }
            }),
        };

        let security_manager = self.security_manager.clone();
        provider.register_async_tool(security_monitor_def, move |input| {
            let security_manager = security_manager.clone();
            async move {
                get_security_status_exec(input, security_manager).await
            }
        }).await;
    }

    async fn register_command_history_tool(&self, provider: &mut LocalToolProvider) {
        let command_history_def = ToolDefinition {
            name: "analyze_command_history".to_string(),
            description: "Analyze recent command execution history and patterns".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "limit": {
                        "type": "integer",
                        "description": "Number of recent commands to analyze",
                        "default": 50,
                        "minimum": 1,
                        "maximum": 500
                    },
                    "include_file_changes": {
                        "type": "boolean",
                        "description": "Whether to include file change analysis",
                        "default": true
                    }
                }
            }),
        };

        let security_manager = self.security_manager.clone();
        provider.register_async_tool(command_history_def, move |input| {
            let security_manager = security_manager.clone();
            async move {
                analyze_command_history_exec(input, security_manager).await
            }
        }).await;
    }
}

/// Securely execute the build_self command with security monitoring
async fn secure_build_self_exec(input: Value, security_manager: SecurityManager) -> Result<Value, String> {
    let target = input["target"].as_str().unwrap_or("dev");
    let manifest_path = input["manifest_path"].as_str().unwrap_or("src-tauri/Cargo.toml");

    info!("Secure build requested with target: {}, manifest: {}", target, manifest_path);

    // Validate inputs
    if !matches!(target, "dev" | "release" | "check") {
        return Err(format!("Invalid target: {}. Must be 'dev', 'release', or 'check'", target));
    }

    if !Path::new(manifest_path).exists() {
        return Err(format!("Manifest file not found: {}", manifest_path));
    }

    // Construct the command
    let command = match target {
        "dev" => format!("cargo build --manifest-path {}", manifest_path),
        "release" => format!("cargo build --release --manifest-path {}", manifest_path),
        "check" => format!("cargo check --manifest-path {}", manifest_path),
        _ => return Err("Invalid target".to_string()),
    };

    // Validate command with security manager
    let context = format!("Self-build operation: {} target", target);
    if !security_manager.validate_command(&command, "secure_build_self", &context).await? {
        return Err("Build command blocked by security policy".to_string());
    }

    // Start monitoring
    let monitor_id = security_manager.start_monitoring(&command, "secure_build_self").await;
    let start_time = Instant::now();

    info!("Executing secure build command: {}", command);

    // Execute the command
    let output = Command::new("cargo")
        .args(match target {
            "dev" => vec!["build", "--manifest-path", manifest_path],
            "release" => vec!["build", "--release", "--manifest-path", manifest_path],
            "check" => vec!["check", "--manifest-path", manifest_path],
            _ => return Err("Invalid target".to_string()),
        })
        .output()
        .map_err(|e| format!("Failed to execute cargo command: {}", e))?;

    let execution_time = start_time.elapsed();
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    // Complete monitoring
    security_manager.complete_monitoring(
        &monitor_id,
        output.status.code(),
        &stdout,
        &stderr,
        execution_time,
    ).await.map_err(|e| format!("Failed to complete monitoring: {}", e))?;

    if output.status.success() {
        info!("Secure build completed successfully for target: {}", target);
        Ok(json!({
            "success": true,
            "target": target,
            "message": format!("Successfully built Juno with target: {}", target),
            "stdout": stdout.to_string(),
            "stderr": stderr.to_string(),
            "execution_time_ms": execution_time.as_millis(),
            "security_monitor_id": monitor_id
        }))
    } else {
        error!("Secure build failed for target: {}", target);
        Err(format!("Build failed: {}", stderr))
    }
}

/// Securely analyze source structure with security monitoring
async fn secure_analyze_source_exec(input: Value, security_manager: SecurityManager) -> Result<Value, String> {
    let path = input["path"].as_str().unwrap_or(".");
    let depth = input["depth"].as_u64().unwrap_or(3) as usize;
    let include_security = input["include_security_analysis"].as_bool().unwrap_or(true);

    info!("Secure source analysis at path: {}, depth: {}, security: {}", path, depth, include_security);

    let base_path = Path::new(path);
    if !base_path.exists() {
        return Err(format!("Path does not exist: {}", path));
    }

    // Start monitoring file access
    let monitor_id = security_manager.start_monitoring(
        &format!("analyze_source {}", path),
        "secure_analyze_source"
    ).await;

    let start_time = Instant::now();
    let structure = analyze_directory_secure(base_path, depth, 0, &security_manager, &monitor_id).await?;
    let execution_time = start_time.elapsed();

    // Complete monitoring
    security_manager.complete_monitoring(
        &monitor_id,
        Some(0),
        "Source analysis completed",
        "",
        execution_time,
    ).await.map_err(|e| format!("Failed to complete monitoring: {}", e))?;

    let mut response = json!({
        "success": true,
        "path": path,
        "depth": depth,
        "structure": structure,
        "analysis": {
            "total_files": count_files_recursive(&structure),
            "file_types": analyze_file_types(&structure),
            "key_directories": identify_key_directories(&structure)
        },
        "execution_time_ms": execution_time.as_millis(),
        "security_monitor_id": monitor_id
    });

    if include_security {
        let security_analysis = perform_security_analysis(&structure, &security_manager).await;
        response["security_analysis"] = security_analysis;
    }

    Ok(response)
}

/// Get secure system information including security status
async fn get_secure_system_info_exec(input: Value, security_manager: SecurityManager) -> Result<Value, String> {
    let include_security = input["include_security_details"].as_bool().unwrap_or(true);

    info!("Getting secure system information, security details: {}", include_security);

    // Get basic system info
    let cwd = std::env::current_dir()
        .map(|p| p.to_string_lossy().to_string())
        .unwrap_or_else(|_| "Unknown".to_string());

    let cargo_manifest_dir = std::env::var("CARGO_MANIFEST_DIR").unwrap_or_else(|_| "Unknown".to_string());
    let cargo_pkg_name = std::env::var("CARGO_PKG_NAME").unwrap_or_else(|_| "Unknown".to_string());
    let cargo_pkg_version = std::env::var("CARGO_PKG_VERSION").unwrap_or_else(|_| "Unknown".to_string());

    let workspace_root = find_workspace_root(&cwd).unwrap_or_else(|| cwd.clone());

    let mut response = json!({
        "success": true,
        "system": {
            "os": std::env::consts::OS,
            "arch": std::env::consts::ARCH,
            "development_mode": cfg!(debug_assertions),
            "security_enabled": include_security
        },
        "workspace": {
            "current_directory": cwd,
            "workspace_root": workspace_root,
            "manifest_dir": cargo_manifest_dir,
            "source_location": "~/repo/juno"
        },
        "package": {
            "name": cargo_pkg_name,
            "version": cargo_pkg_version
        },
        "creator_info": {
            "creator": "Lacy",
            "mission": "Push the world towards global utopia and unite AI and humanity",
            "vision": "Harmonious collaboration between artificial and human intelligence, reducing suffering and promoting peace and prosperity for all"
        },
        "architecture": {
            "prompt_location": "src-tauri/src/agent/prompts/templates.rs",
            "main_orchestration": "src-tauri/src/anthropic.rs",
            "security_framework": "src-tauri/src/agent/security/",
            "agent_modes": ["single", "multi"],
            "security_features": [
                "command_validation",
                "approval_management", 
                "execution_monitoring",
                "file_change_tracking",
                "rate_limiting"
            ]
        }
    });

    if include_security {
        let security_status = security_manager.get_security_status().await;
        response["security_status"] = json!(security_status);
    }

    Ok(response)
}

/// Get current security status
async fn get_security_status_exec(input: Value, security_manager: SecurityManager) -> Result<Value, String> {
    let include_history = input["include_history"].as_bool().unwrap_or(false);
    let history_limit = input["history_limit"].as_u64().unwrap_or(10) as usize;

    info!("Getting security status, history: {}, limit: {}", include_history, history_limit);

    let security_status = security_manager.get_security_status().await;
    let mut response = json!({
        "success": true,
        "security_status": security_status,
        "timestamp": std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs()
    });

    if include_history {
        let command_history = security_manager.get_command_history(history_limit).await;
        response["recent_commands"] = json!(command_history);
    }

    Ok(response)
}

/// Analyze command execution history
async fn analyze_command_history_exec(input: Value, security_manager: SecurityManager) -> Result<Value, String> {
    let limit = input["limit"].as_u64().unwrap_or(50) as usize;
    let include_file_changes = input["include_file_changes"].as_bool().unwrap_or(true);

    info!("Analyzing command history, limit: {}, file changes: {}", limit, include_file_changes);

    let command_history = security_manager.get_command_history(limit).await;
    
    // Analyze patterns
    let mut tool_usage = std::collections::HashMap::new();
    let mut risk_levels = std::collections::HashMap::new();
    let mut total_execution_time = Duration::new(0, 0);
    let mut failed_commands = 0;
    let mut approved_commands = 0;

    for entry in &command_history {
        *tool_usage.entry(entry.tool_name.clone()).or_insert(0) += 1;
        total_execution_time += entry.execution_time;
        
        if entry.exit_code != Some(0) {
            failed_commands += 1;
        }
        
        if entry.user_approved {
            approved_commands += 1;
        }
    }

    let avg_execution_time = if !command_history.is_empty() {
        total_execution_time / command_history.len() as u32
    } else {
        Duration::new(0, 0)
    };

    let mut response = json!({
        "success": true,
        "analysis": {
            "total_commands": command_history.len(),
            "failed_commands": failed_commands,
            "approved_commands": approved_commands,
            "tool_usage": tool_usage,
            "average_execution_time_ms": avg_execution_time.as_millis(),
            "total_execution_time_ms": total_execution_time.as_millis()
        },
        "recent_commands": command_history
    });

    Ok(response)
}

/// Perform security analysis on source structure
async fn perform_security_analysis(structure: &Value, security_manager: &SecurityManager) -> Value {
    // Analyze for potential security concerns in the codebase
    let mut security_files = Vec::new();
    let mut config_files = Vec::new();
    let mut executable_files = Vec::new();

    collect_security_relevant_files(structure, "", &mut security_files, &mut config_files, &mut executable_files);

    json!({
        "security_relevant_files": security_files,
        "configuration_files": config_files,
        "executable_files": executable_files,
        "security_recommendations": [
            "Review file permissions on executable files",
            "Ensure configuration files don't contain secrets",
            "Validate all external dependencies",
            "Monitor file changes in sensitive directories"
        ]
    })
}

/// Collect security-relevant files from structure
fn collect_security_relevant_files(
    structure: &Value,
    path: &str,
    security_files: &mut Vec<String>,
    config_files: &mut Vec<String>,
    executable_files: &mut Vec<String>,
) {
    if structure["type"] == "file" {
        let name = structure["name"].as_str().unwrap_or("");
        let current_path = if path.is_empty() { name.to_string() } else { format!("{}/{}", path, name) };
        
        // Check for security-relevant files
        if name.contains("security") || name.contains("auth") || name.contains("password") || name.contains("key") {
            security_files.push(current_path.clone());
        }
        
        // Check for config files
        if name.ends_with(".toml") || name.ends_with(".json") || name.ends_with(".yaml") || name.ends_with(".yml") {
            config_files.push(current_path.clone());
        }
        
        // Check for executable files
        if name.ends_with(".sh") || name.ends_with(".exe") || name.ends_with(".bat") {
            executable_files.push(current_path);
        }
    } else if structure["type"] == "directory" {
        let name = structure["name"].as_str().unwrap_or("");
        let current_path = if path.is_empty() { name.to_string() } else { format!("{}/{}", path, name) };
        
        if let Some(children) = structure["children"].as_array() {
            for child in children {
                collect_security_relevant_files(child, &current_path, security_files, config_files, executable_files);
            }
        }
    }
}

/// Analyze directory structure with security monitoring
async fn analyze_directory_secure(
    path: &Path,
    max_depth: usize,
    current_depth: usize,
    security_manager: &SecurityManager,
    monitor_id: &str,
) -> Result<Value, String> {
    if current_depth >= max_depth {
        return Ok(json!({
            "name": path.file_name().unwrap_or_default().to_string_lossy(),
            "type": "directory",
            "truncated": true
        }));
    }

    let entries = fs::read_dir(path)
        .map_err(|e| format!("Failed to read directory {}: {}", path.display(), e))?;

    let mut children = Vec::new();

    for entry in entries {
        let entry = entry.map_err(|e| format!("Failed to read directory entry: {}", e))?;
        let entry_path = entry.path();
        let name = entry_path.file_name().unwrap_or_default().to_string_lossy().to_string();

        // Skip hidden files and common build directories
        if name.starts_with('.') || name == "target" || name == "node_modules" || name == "dist" {
            continue;
        }

        if entry_path.is_dir() {
            children.push(analyze_directory_secure(&entry_path, max_depth, current_depth + 1, security_manager, monitor_id).await?);
        } else {
            children.push(json!({
                "name": name,
                "type": "file",
                "extension": entry_path.extension().unwrap_or_default().to_string_lossy()
            }));
        }
    }

    Ok(json!({
        "name": path.file_name().unwrap_or_default().to_string_lossy(),
        "type": "directory",
        "children": children
    }))
}

/// Count files recursively in structure (reused from original)
fn count_files_recursive(structure: &Value) -> usize {
    if structure["type"] == "file" {
        return 1;
    } else if structure["type"] == "directory" {
        if let Some(children) = structure["children"].as_array() {
            return children.iter().map(count_files_recursive).sum();
        }
    }
    0
}

/// Analyze file types in structure (reused from original)
fn analyze_file_types(structure: &Value) -> Value {
    let mut file_types = std::collections::HashMap::new();
    collect_file_types(structure, &mut file_types);

    let mut types_vec: Vec<_> = file_types.into_iter().collect();
    types_vec.sort_by(|a, b| b.1.cmp(&a.1));

    json!(types_vec.into_iter().take(10).collect::<Vec<_>>())
}

/// Collect file types recursively (reused from original)
fn collect_file_types(structure: &Value, file_types: &mut std::collections::HashMap<String, usize>) {
    if structure["type"] == "file" {
        let extension = structure["extension"].as_str().unwrap_or("no_extension").to_string();
        *file_types.entry(extension).or_insert(0) += 1;
    } else if structure["type"] == "directory" {
        if let Some(children) = structure["children"].as_array() {
            for child in children {
                collect_file_types(child, file_types);
            }
        }
    }
}

/// Identify key directories in the project (reused from original)
fn identify_key_directories(structure: &Value) -> Vec<String> {
    let mut key_dirs = Vec::new();
    collect_key_directories(structure, &mut key_dirs, "");
    key_dirs
}

/// Collect key directories recursively (reused from original)
fn collect_key_directories(structure: &Value, key_dirs: &mut Vec<String>, path: &str) {
    if structure["type"] == "directory" {
        let name = structure["name"].as_str().unwrap_or("");
        let current_path = if path.is_empty() { name.to_string() } else { format!("{}/{}", path, name) };

        // Check if this is a key directory
        if matches!(name, "src" | "src-tauri" | "components" | "agent" | "tools" | "prompts" | "commands" | "lib" | "security") {
            key_dirs.push(current_path.clone());
        }

        if let Some(children) = structure["children"].as_array() {
            for child in children {
                collect_key_directories(child, key_dirs, &current_path);
            }
        }
    }
}

/// Find the workspace root by looking for Cargo.toml files (reused from original)
fn find_workspace_root(start_path: &str) -> Option<String> {
    let mut current = PathBuf::from(start_path);

    while let Some(parent) = current.parent() {
        let cargo_toml = parent.join("Cargo.toml");
        if cargo_toml.exists() {
            // Check if it's a workspace
            if let Ok(content) = fs::read_to_string(&cargo_toml) {
                if content.contains("[workspace]") {
                    return Some(parent.to_string_lossy().to_string());
                }
            }
        }
        current = parent.to_path_buf();
    }

    None
}
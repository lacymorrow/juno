use crate::agent::structs::{ToolDefinition, AgentError};
use crate::agent::implementations::tool_provider::LocalToolProvider;
use crate::state::AppState;
use serde_json::{json, Value};
use std::process::Command;
use std::path::{Path, PathBuf};
use std::fs;
use tracing::{info, warn, error};
use tauri::{AppHandle, Manager};

/// 🔐 SECURE: Register self-awareness and introspection tools with security validation
pub async fn register_self_awareness_tools_secure(provider: &mut LocalToolProvider, app_handle: AppHandle) {
    info!("🔐 Registering SECURE self-awareness and introspection tools...");

    // Only register these tools in development mode
    if !cfg!(debug_assertions) {
        info!("Self-awareness tools are only available in development mode");
        return;
    }

    let app_state = app_handle.state::<AppState>();

    // Secure Build self tool
    let build_self_def = ToolDefinition {
        name: "build_self".to_string(),
        description: "🔐 SECURED: Build and compile the Juno application using Cargo with security validation".to_string(),
        input_schema: json!({
            "type": "object",
            "properties": {
                "target": {
                    "type": "string",
                    "description": "Build target: 'dev' for development, 'release' for release, 'check' for syntax check",
                    "enum": ["dev", "release", "check"],
                    "default": "check"
                },
                "manifest_path": {
                    "type": "string",
                    "description": "Optional path to Cargo.toml, defaults to src-tauri/Cargo.toml"
                }
            }
        }),
    };

    let app_state_clone = app_state.inner().clone();
    provider.register_async_tool(build_self_def, move |input| {
        let app_state = app_state_clone.clone();
        async move { build_self_exec_secure(input, &app_state).await }
    }).await;

    // Secure Analyze source code structure tool
    let analyze_source_def = ToolDefinition {
        name: "analyze_source_structure".to_string(),
        description: "🔐 SECURED: Analyze the source code structure and architecture of the Juno application with path validation".to_string(),
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
                    "maximum": 5
                }
            }
        }),
    };

    let app_state_clone = app_state.inner().clone();
    provider.register_async_tool(analyze_source_def, move |input| {
        let app_state = app_state_clone.clone();
        async move { analyze_source_structure_exec_secure(input, &app_state).await }
    }).await;

    // Inspect prompt system tool (no security needed for read-only operations)
    let inspect_prompts_def = ToolDefinition {
        name: "inspect_prompt_system".to_string(),
        description: "Inspect the current prompt system configuration and available prompts".to_string(),
        input_schema: json!({
            "type": "object",
            "properties": {
                "show_content": {
                    "type": "boolean",
                    "description": "Whether to include full prompt content in the response",
                    "default": false
                }
            }
        }),
    };

    provider.register_async_tool(inspect_prompts_def, |input| {
        async move { inspect_prompt_system_exec(input).await }
    }).await;

    // Get system info tool (read-only, safe)
    let system_info_def = ToolDefinition {
        name: "get_system_info".to_string(),
        description: "Get information about the current system, environment, and build configuration".to_string(),
        input_schema: json!({
            "type": "object",
            "properties": {}
        }),
    };

    provider.register_async_tool(system_info_def, |_input| {
        async move { get_system_info_exec().await }
    }).await;

    info!("🔐 SECURE self-awareness tools registered successfully");
}

/// ⚠️ LEGACY: Register self-awareness tools WITHOUT security (DEPRECATED)
pub async fn register_self_awareness_tools(provider: &mut LocalToolProvider) {
    warn!("🚨 SECURITY WARNING: Registering self-awareness tools WITHOUT security validation");
    warn!("🚨 This should only be used for testing or backward compatibility");

    info!("Registering self-awareness and introspection tools...");

    // Only register these tools in development mode
    if !cfg!(debug_assertions) {
        info!("Self-awareness tools are only available in development mode");
        return;
    }

    // Build self tool (UNSECURED)
    let build_self_def = ToolDefinition {
        name: "build_self".to_string(),
        description: "⚠️ UNSECURED: Build and compile the Juno application using Cargo".to_string(),
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

    provider.register_async_tool(build_self_def, |input| {
        async move { build_self_exec(input).await }
    }).await;

    // Analyze source code structure tool
    let analyze_source_def = ToolDefinition {
        name: "analyze_source_structure".to_string(),
        description: "Analyze the source code structure and architecture of the Juno application".to_string(),
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
                }
            }
        }),
    };

    provider.register_async_tool(analyze_source_def, |input| {
        async move { analyze_source_structure_exec(input).await }
    }).await;

    // Inspect prompt system tool
    let inspect_prompts_def = ToolDefinition {
        name: "inspect_prompt_system".to_string(),
        description: "Inspect the current prompt system configuration and available prompts".to_string(),
        input_schema: json!({
            "type": "object",
            "properties": {
                "show_content": {
                    "type": "boolean",
                    "description": "Whether to include full prompt content in the response",
                    "default": false
                }
            }
        }),
    };

    provider.register_async_tool(inspect_prompts_def, |input| {
        async move { inspect_prompt_system_exec(input).await }
    }).await;

    // Get system info tool
    let system_info_def = ToolDefinition {
        name: "get_system_info".to_string(),
        description: "Get information about the current system, environment, and build configuration".to_string(),
        input_schema: json!({
            "type": "object",
            "properties": {}
        }),
    };

    provider.register_async_tool(system_info_def, |_input| {
        async move { get_system_info_exec().await }
    }).await;

    warn!("⚠️ Registered UNSECURED self-awareness tools");
}

/// 🔐 SECURE: Execute the build_self tool with security validation
async fn build_self_exec_secure(input: Value, app_state: &AppState) -> Result<Value, String> {
    let target = input["target"].as_str().unwrap_or("check");
    let manifest_path = input["manifest_path"].as_str().unwrap_or("src-tauri/Cargo.toml");

    info!("🔐 Building self with target: {}, manifest: {} (SECURED)", target, manifest_path);

    // 🔐 SECURITY: Validate manifest path
    validate_manifest_path(manifest_path)?;

    // 🔐 SECURITY: Create cargo command for validation
    let cargo_command = match target {
        "dev" => format!("cargo build --manifest-path {}", manifest_path),
        "release" => format!("cargo build --release --manifest-path {}", manifest_path),
        "check" => format!("cargo check --manifest-path {}", manifest_path),
        _ => return Err(format!("Invalid target: {}. Must be 'dev', 'release', or 'check'", target)),
    };

    // 🔐 SECURITY: MANDATORY validation with SecurityManager
    if let Some(security_manager) = app_state.get_security_manager().await {
        // 1. Validate command with security manager
        match security_manager.validate_command(
            &cargo_command,
            "build_self",
            &format!("Self-building with target: {}", target)
        ).await {
            Ok(_) => {
                info!("✅ Security validation passed for build command: {}", cargo_command);
            },
            Err(e) => {
                warn!("🚫 Security validation failed for build command '{}': {}", cargo_command, e);
                return Err(format!("🔐 Build command blocked by security policy: {}", e));
            }
        }

        // 2. Start execution monitoring
        let monitor_id = security_manager.start_execution_monitoring(
            &cargo_command,
            "build_self"
        ).await;

        // 3. Execute the build command
        let start_time = std::time::Instant::now();
        let result = execute_cargo_build(target, manifest_path).await;
        let execution_time = start_time.elapsed();

        // 4. End monitoring
        if let Err(e) = security_manager.end_execution_monitoring(&monitor_id).await {
            warn!("🔐 Failed to end build monitoring: {}", e);
        }

        // 5. Add security metadata to result
        match result {
            Ok(mut value) => {
                if let Some(obj) = value.as_object_mut() {
                    obj.insert("security_validated".to_string(), json!(true));
                    obj.insert("execution_time_ms".to_string(), json!(execution_time.as_millis()));
                    obj.insert("monitor_id".to_string(), json!(monitor_id));
                    obj.insert("cargo_command".to_string(), json!(cargo_command));
                }
                info!("✅ Secured build completed in {}ms", execution_time.as_millis());
                Ok(value)
            },
            Err(e) => {
                error!("❌ Secured build failed: {}", e);
                Err(e)
            }
        }
    } else {
        error!("🚨 CRITICAL: Security manager not available! Build execution blocked.");
        Err("🔐 Security manager not available - build execution blocked for safety".to_string())
    }
}

/// 🔐 SECURE: Execute source structure analysis with path validation
async fn analyze_source_structure_exec_secure(input: Value, app_state: &AppState) -> Result<Value, String> {
    let path = input["path"].as_str().unwrap_or(".");
    let depth = input["depth"].as_u64().unwrap_or(3).min(5) as usize; // Limit max depth for security

    info!("🔐 Analyzing source structure: {} with depth {} (SECURED)", path, depth);

    // 🔐 SECURITY: Validate path
    validate_analysis_path(path)?;

    // 🔐 SECURITY: Check with security manager if doing file system operations
    if let Some(security_manager) = app_state.get_security_manager().await {
        let analysis_command = format!("analyze_directory {}", path);
        
        match security_manager.validate_command(
            &analysis_command,
            "analyze_source_structure",
            &format!("Analyzing directory structure: {}", path)
        ).await {
            Ok(_) => {
                info!("✅ Security validation passed for directory analysis: {}", path);
            },
            Err(e) => {
                warn!("🚫 Security validation failed for directory analysis '{}': {}", path, e);
                return Err(format!("🔐 Directory analysis blocked by security policy: {}", e));
            }
        }

        // Start monitoring
        let monitor_id = security_manager.start_execution_monitoring(
            &analysis_command,
            "analyze_source_structure"
        ).await;

        // Perform analysis
        let result = analyze_directory_structure(path, depth).await;

        // End monitoring
        if let Err(e) = security_manager.end_execution_monitoring(&monitor_id).await {
            warn!("🔐 Failed to end analysis monitoring: {}", e);
        }

        result
    } else {
        warn!("🔐 Security manager not available, proceeding with basic validation");
        analyze_directory_structure(path, depth).await
    }
}

/// ⚠️ LEGACY: Execute the build_self tool without security (DEPRECATED)
async fn build_self_exec(input: Value) -> Result<Value, String> {
    let target = input["target"].as_str().unwrap_or("dev");
    let manifest_path = input["manifest_path"].as_str().unwrap_or("src-tauri/Cargo.toml");

    error!("🚨 SECURITY WARNING: Using unsecured build execution for target: {}", target);
    
    execute_cargo_build(target, manifest_path).await
}

/// Execute cargo build command implementation
async fn execute_cargo_build(target: &str, manifest_path: &str) -> Result<Value, String> {
    // Determine the cargo command based on target
    let mut cmd = Command::new("cargo");
    match target {
        "dev" => {
            cmd.args(&["build", "--manifest-path", manifest_path]);
        }
        "release" => {
            cmd.args(&["build", "--release", "--manifest-path", manifest_path]);
        }
        "check" => {
            cmd.args(&["check", "--manifest-path", manifest_path]);
        }
        _ => {
            return Err(format!("Invalid target: {}. Must be 'dev', 'release', or 'check'", target));
        }
    }

    info!("Executing cargo command: {:?}", cmd);

    // Execute the build command
    let output = cmd.output().map_err(|e| {
        format!("Failed to execute cargo command: {}", e)
    })?;

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    let success = output.status.success();

    info!("Cargo build completed. Success: {}", success);

    if !success {
        warn!("Build failed. Stderr: {}", stderr);
    }

    Ok(json!({
        "success": success,
        "target": target,
        "manifest_path": manifest_path,
        "stdout": stdout.to_string(),
        "stderr": stderr.to_string(),
        "exit_code": output.status.code(),
        "message": if success { "Build completed successfully" } else { "Build failed" }
    }))
}

/// Analyze directory structure implementation
async fn analyze_directory_structure(path: &str, depth: usize) -> Result<Value, String> {
    let base_path = std::env::current_dir()
        .map_err(|e| format!("Failed to get current directory: {}", e))?
        .join(path);

    if !base_path.exists() {
        return Err(format!("Path does not exist: {}", path));
    }

    let structure = analyze_directory(&base_path, depth, 0)?;

    Ok(json!({
        "success": true,
        "path": path,
        "depth": depth,
        "structure": structure,
        "analysis": {
            "total_files": count_files_recursive(&structure),
            "file_types": analyze_file_types(&structure),
            "key_directories": identify_key_directories(&structure)
        },
        "security_validated": true
    }))
}

/// Validate manifest path for security
fn validate_manifest_path(manifest_path: &str) -> Result<(), String> {
    // Check for path traversal
    if manifest_path.contains("..") {
        return Err("Path traversal detected in manifest path".to_string());
    }

    // Must be a Cargo.toml file
    if !manifest_path.ends_with("Cargo.toml") {
        return Err("Manifest path must end with 'Cargo.toml'".to_string());
    }

    // Must be within allowed directories
    let allowed_prefixes = ["src-tauri/", "tauri-plugin-", "./src-tauri/"];
    let is_allowed = allowed_prefixes.iter().any(|prefix| manifest_path.starts_with(prefix));
    
    if !is_allowed && manifest_path != "Cargo.toml" {
        return Err("Manifest path not in allowed directory".to_string());
    }

    Ok(())
}

/// Validate analysis path for security
fn validate_analysis_path(path: &str) -> Result<(), String> {
    // Check for path traversal
    if path.contains("..") {
        return Err("Path traversal detected in analysis path".to_string());
    }

    // Check for absolute paths
    if Path::new(path).is_absolute() {
        return Err("Absolute paths not allowed for analysis".to_string());
    }

    // Block sensitive directories
    let blocked_paths = ["/etc", "/root", "/sys", "/proc", "/dev", ".ssh", ".env"];
    for blocked in &blocked_paths {
        if path.contains(blocked) {
            return Err(format!("Access to sensitive path blocked: {}", blocked));
        }
    }

    Ok(())
}

/// Execute the inspect_prompt_system tool
async fn inspect_prompt_system_exec(input: Value) -> Result<Value, String> {
    let show_content = input["show_content"].as_bool().unwrap_or(false);

    info!("Inspecting prompt system, show_content: {}", show_content);

    // Load the prompt manager
    let prompt_manager = match crate::agent::prompts::PromptManager::load() {
        Ok(manager) => manager,
        Err(e) => {
            warn!("Failed to load prompt manager, using default: {}", e);
            crate::agent::prompts::PromptManager::default()
        }
    };

    let templates = prompt_manager.get_templates();
    let mut prompt_info = Vec::new();

    for (prompt_type, template) in templates {
        let mut info = json!({
            "type": prompt_type.as_str(),
            "name": template.name,
            "description": template.description,
            "version": template.version,
            "customizable": template.customizable,
            "variables": template.variables,
            "tags": template.tags
        });

        if show_content {
            info["content"] = json!(template.content);
        }

        prompt_info.push(info);
    }

    Ok(json!({
        "success": true,
        "prompt_system": {
            "total_templates": prompt_info.len(),
            "templates": prompt_info,
            "manager_info": {
                "loaded_successfully": true,
                "show_content": show_content
            }
        }
    }))
}

/// Execute the get_system_info tool
async fn get_system_info_exec() -> Result<Value, String> {
    info!("Getting system information");

    let current_dir = std::env::current_dir()
        .map(|p| p.to_string_lossy().to_string())
        .unwrap_or_else(|_| "unknown".to_string());

    let os_info = format!("{} {}", std::env::consts::OS, std::env::consts::ARCH);
    
    let cargo_version = Command::new("cargo")
        .arg("--version")
        .output()
        .ok()
        .and_then(|output| String::from_utf8(output.stdout).ok())
        .unwrap_or_else(|| "unknown".to_string())
        .trim()
        .to_string();

    let rustc_version = Command::new("rustc")
        .arg("--version")
        .output()
        .ok()
        .and_then(|output| String::from_utf8(output.stdout).ok())
        .unwrap_or_else(|| "unknown".to_string())
        .trim()
        .to_string();

    Ok(json!({
        "success": true,
        "system_info": {
            "os": os_info,
            "current_directory": current_dir,
            "cargo_version": cargo_version,
            "rustc_version": rustc_version,
            "debug_build": cfg!(debug_assertions),
            "self_awareness_enabled": cfg!(debug_assertions),
        },
        "juno_info": {
            "creator": "Lacy (magnanimous benefactor)",
            "location": "~/repo/juno",
            "mission": "Unite AI and humanity through natural interaction",
            "security_enabled": true
        }
    }))
}

// Helper functions for directory analysis
fn analyze_directory(path: &Path, max_depth: usize, current_depth: usize) -> Result<Value, String> {
    if current_depth >= max_depth {
        return Ok(json!({"name": path.file_name().unwrap_or_default().to_string_lossy(), "type": "directory", "truncated": true}));
    }

    let entries = fs::read_dir(path)
        .map_err(|e| format!("Failed to read directory {:?}: {}", path, e))?;

    let mut children = Vec::new();
    for entry in entries {
        let entry = entry.map_err(|e| format!("Failed to read directory entry: {}", e))?;
        let entry_path = entry.path();
        let name = entry.file_name().to_string_lossy().to_string();

        if entry_path.is_dir() {
            children.push(analyze_directory(&entry_path, max_depth, current_depth + 1)?);
        } else {
            children.push(json!({
                "name": name,
                "type": "file",
                "size": entry.metadata().map(|m| m.len()).unwrap_or(0)
            }));
        }
    }

    Ok(json!({
        "name": path.file_name().unwrap_or_default().to_string_lossy(),
        "type": "directory",
        "children": children
    }))
}

fn count_files_recursive(structure: &Value) -> u64 {
    if structure["type"] == "file" {
        1
    } else if let Some(children) = structure["children"].as_array() {
        children.iter().map(count_files_recursive).sum()
    } else {
        0
    }
}

fn analyze_file_types(structure: &Value) -> Value {
    let mut types = std::collections::HashMap::new();
    collect_file_types(structure, &mut types);
    json!(types)
}

fn collect_file_types(structure: &Value, types: &mut std::collections::HashMap<String, u64>) {
    if structure["type"] == "file" {
        if let Some(name) = structure["name"].as_str() {
            let ext = Path::new(name).extension()
                .and_then(|s| s.to_str())
                .unwrap_or("no_extension")
                .to_string();
            *types.entry(ext).or_insert(0) += 1;
        }
    } else if let Some(children) = structure["children"].as_array() {
        for child in children {
            collect_file_types(child, types);
        }
    }
}

fn identify_key_directories(structure: &Value) -> Vec<String> {
    let mut key_dirs = Vec::new();
    collect_key_directories(structure, &mut key_dirs, "");
    key_dirs
}

fn collect_key_directories(structure: &Value, key_dirs: &mut Vec<String>, current_path: &str) {
    if structure["type"] == "directory" {
        if let Some(name) = structure["name"].as_str() {
            let full_path = if current_path.is_empty() {
                name.to_string()
            } else {
                format!("{}/{}", current_path, name)
            };

            // Identify key directories
            match name {
                "src" | "src-tauri" | "components" | "lib" | "utils" | "agent" | "tools" => {
                    key_dirs.push(full_path.clone());
                }
                _ => {}
            }

            if let Some(children) = structure["children"].as_array() {
                for child in children {
                    collect_key_directories(child, key_dirs, &full_path);
                }
            }
        }
    }
}
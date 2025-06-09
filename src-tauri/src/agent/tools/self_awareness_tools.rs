use crate::agent::structs::{ToolDefinition, AgentError};
use crate::agent::implementations::tool_provider::LocalToolProvider;
use serde_json::{json, Value};
use std::process::Command;
use std::path::{Path, PathBuf};
use std::fs;
use tracing::{info, warn, error};
use tauri::AppHandle;

/// Register self-awareness and introspection tools
pub async fn register_self_awareness_tools(provider: &mut LocalToolProvider) {
    info!("Registering self-awareness and introspection tools...");

    // Only register these tools in development mode
    if !cfg!(debug_assertions) {
        info!("Self-awareness tools are only available in development mode");
        return;
    }

    // Build self tool
    let build_self_def = ToolDefinition {
        name: "build_self".to_string(),
        description: "Build and compile the Juno application using Cargo in development mode".to_string(),
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

    info!("Self-awareness tools registered successfully");
}

/// Execute the build_self tool
async fn build_self_exec(input: Value) -> Result<Value, String> {
    let target = input["target"].as_str().unwrap_or("dev");
    let manifest_path = input["manifest_path"].as_str().unwrap_or("src-tauri/Cargo.toml");

    info!("Building self with target: {}, manifest: {}", target, manifest_path);

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

    // Execute the build command
    let output = cmd.output().map_err(|e| {
        format!("Failed to execute cargo command: {}", e)
    })?;

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    if output.status.success() {
        info!("Build successful for target: {}", target);
        Ok(json!({
            "success": true,
            "target": target,
            "message": format!("Successfully built Juno with target: {}", target),
            "stdout": stdout.to_string(),
            "stderr": stderr.to_string()
        }))
    } else {
        error!("Build failed for target: {}", target);
        Err(format!("Build failed: {}", stderr))
    }
}

/// Execute the analyze_source_structure tool
async fn analyze_source_structure_exec(input: Value) -> Result<Value, String> {
    let path = input["path"].as_str().unwrap_or(".");
    let depth = input["depth"].as_u64().unwrap_or(3) as usize;

    info!("Analyzing source structure at path: {}, depth: {}", path, depth);

    let base_path = Path::new(path);
    if !base_path.exists() {
        return Err(format!("Path does not exist: {}", path));
    }

    let structure = analyze_directory(base_path, depth, 0)?;

    Ok(json!({
        "success": true,
        "path": path,
        "depth": depth,
        "structure": structure,
        "analysis": {
            "total_files": count_files_recursive(&structure),
            "file_types": analyze_file_types(&structure),
            "key_directories": identify_key_directories(&structure)
        }
    }))
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
        "development_mode": cfg!(debug_assertions),
        "total_prompts": prompt_info.len(),
        "prompts": prompt_info,
        "global_variables": prompt_manager.get_global_variables()
    }))
}

/// Execute the get_system_info tool
async fn get_system_info_exec() -> Result<Value, String> {
    info!("Getting system information");

    // Get current working directory
    let cwd = std::env::current_dir()
        .map(|p| p.to_string_lossy().to_string())
        .unwrap_or_else(|_| "Unknown".to_string());

    // Get environment variables
    let cargo_manifest_dir = std::env::var("CARGO_MANIFEST_DIR").unwrap_or_else(|_| "Unknown".to_string());
    let cargo_pkg_name = std::env::var("CARGO_PKG_NAME").unwrap_or_else(|_| "Unknown".to_string());
    let cargo_pkg_version = std::env::var("CARGO_PKG_VERSION").unwrap_or_else(|_| "Unknown".to_string());

    // Detect workspace root (look for Cargo.toml with workspace)
    let workspace_root = find_workspace_root(&cwd).unwrap_or_else(|| cwd.clone());

    Ok(json!({
        "success": true,
        "system": {
            "os": std::env::consts::OS,
            "arch": std::env::consts::ARCH,
            "development_mode": cfg!(debug_assertions)
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
            "mission": "A magnanimous benefactor working to push the world towards utopia and unite AI and humanity",
            "vision": "Harmonious collaboration between artificial and human intelligence"
        },
        "architecture": {
            "prompt_location": "src-tauri/src/agent/prompts/templates.rs",
            "main_orchestration": "src-tauri/src/anthropic.rs",
            "agent_modes": ["single", "multi"],
            "current_mode": crate::agent::providers::factory::BrainFactory::get_agent_mode()
        }
    }))
}

/// Recursively analyze directory structure
fn analyze_directory(path: &Path, max_depth: usize, current_depth: usize) -> Result<Value, String> {
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
            children.push(analyze_directory(&entry_path, max_depth, current_depth + 1)?);
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

/// Count files recursively in structure
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

/// Analyze file types in structure
fn analyze_file_types(structure: &Value) -> Value {
    let mut file_types = std::collections::HashMap::new();
    collect_file_types(structure, &mut file_types);
    
    let mut types_vec: Vec<_> = file_types.into_iter().collect();
    types_vec.sort_by(|a, b| b.1.cmp(&a.1));
    
    json!(types_vec.into_iter().take(10).collect::<Vec<_>>())
}

/// Collect file types recursively
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

/// Identify key directories in the project
fn identify_key_directories(structure: &Value) -> Vec<String> {
    let mut key_dirs = Vec::new();
    collect_key_directories(structure, &mut key_dirs, "");
    key_dirs
}

/// Collect key directories recursively
fn collect_key_directories(structure: &Value, key_dirs: &mut Vec<String>, path: &str) {
    if structure["type"] == "directory" {
        let name = structure["name"].as_str().unwrap_or("");
        let current_path = if path.is_empty() { name.to_string() } else { format!("{}/{}", path, name) };
        
        // Check if this is a key directory
        if matches!(name, "src" | "src-tauri" | "components" | "agent" | "tools" | "prompts" | "commands" | "lib") {
            key_dirs.push(current_path.clone());
        }
        
        if let Some(children) = structure["children"].as_array() {
            for child in children {
                collect_key_directories(child, key_dirs, &current_path);
            }
        }
    }
}

/// Find the workspace root by looking for Cargo.toml files
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
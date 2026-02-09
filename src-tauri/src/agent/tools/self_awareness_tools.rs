//! # Self-Awareness Tools Module
//!
//! Introspection and self-building capabilities for the Juno AI Computer Use Agent.
//! Enables the agent to understand its own source code, build system, architecture,
//! and operational environment. These tools are only available in debug mode for security.
//!
//! ## Core Capabilities:
//! - Self-compilation and build management
//! - Source code structure analysis
//! - Prompt system inspection
//! - System and environment awareness
//! - Workspace and project discovery
//!
//! ## Security Model:
//! - Only active in development mode (`cfg!(debug_assertions)`)
//! - No access to sensitive runtime data
//! - Limited to build and analysis operations
//!
//! ## Usage
//! Used by: Self-improvement workflows, debugging, architecture understanding
//! Registration: Called via `register_self_awareness_tools()` in debug mode only

use crate::agent::core::ToolDefinition;
use crate::agent::implementations::tool_provider::LocalToolProvider;
use serde_json::{json, Value};
use std::process::Command;
use std::path::{Path, PathBuf};
use std::fs;
use tracing::{info, error};

/// Registers self-awareness and introspection tools with the tool provider.
///
/// This function enables the agent to understand its own architecture, build system,
/// and operational environment. Tools are only registered in development mode for security.
///
/// Used by: Agent initialization, self-improvement workflows, debugging sessions
///
/// # Arguments
/// * `provider` - Mutable reference to LocalToolProvider for tool registration
///
/// # Security Note
/// Tools are only available when `cfg!(debug_assertions)` is true to prevent
/// production systems from having self-modification capabilities.
///
/// # Tools Registered
/// - `build_self`: Compile the Juno application using Cargo
/// - `analyze_source_structure`: Analyze codebase structure and architecture
/// - `inspect_prompt_system`: Examine prompt configuration and templates
/// - `get_system_info`: Get system, workspace, and build information
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
        api_type: None,
        beta_flag: None,
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
        api_type: None,
        beta_flag: None,
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
        api_type: None,
        beta_flag: None,
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
        api_type: None,
        beta_flag: None,
    };

    provider.register_async_tool(system_info_def, |_input| {
        async move { get_system_info_exec().await }
    }).await;

    info!("Self-awareness tools registered successfully");
}

/// Executes the `build_self` tool to compile the Juno application.
///
/// Provides the agent with the ability to rebuild itself using Cargo.
/// Supports different build targets for development, release, and syntax checking.
///
/// Used by: Self-improvement workflows, build verification, development debugging
///
/// # Arguments
/// * `input` - JSON containing target type and optional manifest path
///
/// # Returns
/// `Result<Value, String>` - Build status and output or error message
///
/// # Build Targets
/// - `dev`: Development build with debug symbols
/// - `release`: Optimized release build
/// - `check`: Syntax and type checking only
async fn build_self_exec(input: Value) -> Result<Value, String> {
    let target = input["target"].as_str().unwrap_or("dev");
    let manifest_path = input["manifest_path"].as_str().unwrap_or("src-tauri/Cargo.toml");

    info!("Building self with target: {}, manifest: {}", target, manifest_path);

    // Determine the cargo command based on target
    let mut cmd = Command::new("cargo");
    match target {
        "dev" => {
            cmd.args(["build", "--manifest-path", manifest_path]);
        }
        "release" => {
            cmd.args(["build", "--release", "--manifest-path", manifest_path]);
        }
        "check" => {
            cmd.args(["check", "--manifest-path", manifest_path]);
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

/// Executes the `analyze_source_structure` tool to examine codebase architecture.
///
/// Provides detailed analysis of the project structure, file organization,
/// and architectural patterns. Helps the agent understand its own composition.
///
/// Used by: Architecture analysis, codebase understanding, documentation generation
///
/// # Arguments
/// * `input` - JSON containing path to analyze and traversal depth
///
/// # Returns
/// `Result<Value, String>` - Detailed structure analysis or error message
///
/// # Analysis Features
/// - Directory tree structure
/// - File type distribution
/// - Key directory identification
/// - Architecture pattern recognition
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

/// Executes the `inspect_prompt_system` tool to examine prompt configuration.
///
/// Allows the agent to understand its own prompt system, templates, and
/// configuration. Provides insight into how the agent's behavior is defined.
///
/// Used by: Prompt system debugging, behavior analysis, template management
///
/// # Arguments
/// * `input` - JSON containing options for content display
///
/// # Returns
/// `Result<Value, String>` - Prompt system details or error message
///
/// # Features
/// - Template inventory and metadata
/// - Variable and configuration analysis
/// - Development mode awareness
/// - Optional full content display
async fn inspect_prompt_system_exec(input: Value) -> Result<Value, String> {
    let show_content = input["show_content"].as_bool().unwrap_or(false);

    info!("Inspecting prompt system, show_content: {}", show_content);

    // Load the prompt manager
    let prompt_manager = crate::agent::prompts::PromptManager::new();

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

/// Executes the `get_system_info` tool to gather environment information.
///
/// Provides comprehensive information about the agent's operational environment,
/// workspace configuration, and build context. Includes creator attribution
/// and mission statement.
///
/// Used by: Environment analysis, workspace discovery, debugging, system reporting
///
/// # Returns
/// `Result<Value, String>` - System information or error message
///
/// # Information Gathered
/// - Operating system and architecture
/// - Workspace and directory structure
/// - Package and version information
/// - Creator attribution and mission
/// - Agent architecture details
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
            "mission": "Push the world towards global utopia and unite AI and humanity",
            "vision": "Harmonious collaboration between artificial and human intelligence, reducing suffering and promoting peace and prosperity for all, remove money and conflicts of interest from politics"
        },
        "architecture": {
            "prompt_location": "src-tauri/src/agent/prompts/templates.rs",
            "main_orchestration": "src-tauri/src/anthropic.rs",
            "agent_modes": ["single", "multi"],
            "current_mode": crate::agent::providers::factory::BrainFactory::get_agent_mode()
        }
    }))
}

/// Recursively analyzes directory structure for source code organization.
///
/// Helper function that traverses the filesystem to build a hierarchical
/// representation of the project structure.
///
/// Used by: `analyze_source_structure_exec` for directory tree analysis
///
/// # Arguments
/// * `path` - Directory path to analyze
/// * `max_depth` - Maximum recursion depth
/// * `current_depth` - Current recursion level
///
/// # Returns
/// `Result<Value, String>` - JSON structure representation or error
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

/// Counts total files recursively in a directory structure.
///
/// Helper function for statistical analysis of codebase size.
/// Used by: Source structure analysis for metrics generation
///
/// # Arguments
/// * `structure` - JSON structure from directory analysis
///
/// # Returns
/// Total count of files in the structure
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

/// Analyzes file type distribution in the codebase.
///
/// Helper function that categorizes files by extension to understand
/// the technology composition of the project.
///
/// Used by: Source structure analysis for technology stack identification
///
/// # Arguments
/// * `structure` - JSON structure from directory analysis
///
/// # Returns
/// JSON object with file type statistics
fn analyze_file_types(structure: &Value) -> Value {
    let mut file_types = std::collections::HashMap::new();
    collect_file_types(structure, &mut file_types);

    let mut types_vec: Vec<_> = file_types.into_iter().collect();
    types_vec.sort_by(|a, b| b.1.cmp(&a.1));

    json!(types_vec.into_iter().take(10).collect::<Vec<_>>())
}

/// Recursively collects file type statistics.
///
/// Helper function for file type analysis that traverses the structure
/// and categorizes files by their extensions.
///
/// Used by: `analyze_file_types` for recursive file type counting
///
/// # Arguments
/// * `structure` - JSON structure to traverse
/// * `file_types` - Mutable map to accumulate file type counts
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

/// Identifies key architectural directories in the project.
///
/// Helper function that recognizes important directories based on naming
/// conventions and architectural patterns commonly used in the project.
///
/// Used by: Source structure analysis for architecture understanding
///
/// # Arguments
/// * `structure` - JSON structure from directory analysis
///
/// # Returns
/// Vector of key directory paths
fn identify_key_directories(structure: &Value) -> Vec<String> {
    let mut key_dirs = Vec::new();
    collect_key_directories(structure, &mut key_dirs, "");
    key_dirs
}

/// Recursively collects key directory paths.
///
/// Helper function that traverses the structure and identifies directories
/// that are architecturally significant to the project organization.
///
/// Used by: `identify_key_directories` for recursive directory identification
///
/// # Arguments
/// * `structure` - JSON structure to traverse
/// * `key_dirs` - Mutable vector to accumulate key directory paths
/// * `path` - Current path context for building full paths
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

/// Finds the workspace root by looking for Cargo.toml with workspace configuration.
///
/// Helper function that traverses up the directory tree to locate the
/// workspace root directory containing the main Cargo.toml file.
///
/// Used by: System information gathering for workspace detection
///
/// # Arguments
/// * `start_path` - Starting path for workspace search
///
/// # Returns
/// Optional workspace root path as string
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

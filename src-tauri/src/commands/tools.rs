use serde_json::{json, Value};
use tauri::{AppHandle, State};
use tracing::info;

use crate::agent::tools::{tool_config::ToolConfigManager, ToolCategory};
use crate::state::AppState;

/// Get all tool configurations organized by category
#[tauri::command]
pub async fn get_tool_configurations(
    _app_handle: AppHandle,
    state: State<'_, AppState>,
) -> Result<Value, String> {
    tracing::debug!("Getting tool configurations");

    let config_manager = state.get_tool_config_manager().await;
    let config_guard = config_manager.lock().await;

    let mut result = serde_json::Map::new();

    // Organize tools by category
    for category in ToolCategory::all_categories() {
        let tools_in_category = config_guard.get_tools_by_category(&category);
        let category_enabled = config_guard
            .category_enabled
            .get(&category)
            .unwrap_or(&true);

        let mut category_tools = Vec::new();
        for tool_config in tools_in_category {
            category_tools.push(json!({
                "name": tool_config.name,
                "category": format!("{:?}", tool_config.category),
                "enabled": tool_config.enabled,
                "description": tool_config.description.as_deref().unwrap_or("No description"),
                "required": tool_config.required,
                "server_id": tool_config.server_id
            }));
        }

        let category_info = json!({
                "name": category.display_name(),
                "description": category.description(),
                "enabled": category_enabled,
                "tools": category_tools
        });

        result.insert(
            ToolConfigManager::format_tool_category(&category),
            category_info,
        );
    }

    Ok(Value::Object(result))
}

/// Get a specific tool configuration
#[tauri::command]
pub async fn get_tool_config(
    tool_name: String,
    state: State<'_, AppState>,
) -> Result<Value, String> {
    info!("Getting tool config for: {}", tool_name);

    let config_manager = state.get_tool_config_manager().await;
    let config_guard = config_manager.lock().await;

    if let Some(tool_config) = config_guard.get_tool_config(&tool_name) {
        Ok(json!({
            "name": tool_config.name,
            "category": format!("{:?}", tool_config.category),
            "enabled": tool_config.enabled,
            "description": tool_config.description.as_deref().unwrap_or("No description"),
            "required": tool_config.required,
            "server_id": tool_config.server_id
        }))
    } else {
        Err(format!("Tool '{}' not found", tool_name))
    }
}

/// Set tool enabled status
#[tauri::command]
pub async fn set_tool_enabled(
    tool_name: String,
    enabled: bool,
    app_handle: AppHandle,
    state: State<'_, AppState>,
) -> Result<(), String> {
    info!("Setting tool {} enabled: {}", tool_name, enabled);

    let config_manager = state.get_tool_config_manager().await;
    {
        let mut config_guard = config_manager.lock().await;
        config_guard.set_tool_enabled(&tool_name, enabled);
    }

    // Save configuration to file
    state.save_tool_config(&app_handle).await?;

    Ok(())
}

/// Set tool category enabled status
#[tauri::command]
pub async fn set_tool_category_enabled(
    category: String,
    enabled: bool,
    app_handle: AppHandle,
    state: State<'_, AppState>,
) -> Result<(), String> {
    info!("Setting category {} enabled: {}", category, enabled);

    // Parse category from string using the proper parsing method
    let tool_category = ToolConfigManager::parse_tool_category(&category)
        .map_err(|e| format!("Failed to parse category '{}': {}", category, e))?;

    let config_manager = state.get_tool_config_manager().await;
    {
        let mut config_guard = config_manager.lock().await;
        config_guard.set_category_enabled(&tool_category, enabled);
    }

    // Save configuration to file
    state.save_tool_config(&app_handle).await?;

    Ok(())
}

/// Get list of enabled tools
#[tauri::command]
pub async fn get_enabled_tools(state: State<'_, AppState>) -> Result<Vec<String>, String> {
    info!("Getting enabled tools");

    let config_manager = state.get_tool_config_manager().await;
    let config_guard = config_manager.lock().await;

    Ok(config_guard.get_enabled_tool_names())
}

/// Check if a specific tool is enabled
#[tauri::command]
pub async fn is_tool_enabled(
    tool_name: String,
    state: State<'_, AppState>,
) -> Result<bool, String> {
    info!("Checking if tool {} is enabled", tool_name);

    let config_manager = state.get_tool_config_manager().await;
    let config_guard = config_manager.lock().await;

    Ok(config_guard.is_tool_enabled(&tool_name))
}

/// Reset tool configuration to defaults
#[tauri::command]
pub async fn reset_tool_configuration(
    app_handle: AppHandle,
    state: State<'_, AppState>,
) -> Result<(), String> {
    info!("Resetting tool configuration to defaults");

    let config_manager = state.get_tool_config_manager().await;
    {
        let mut config_guard = config_manager.lock().await;
        config_guard.reset_to_defaults();
    }

    // Save configuration to file
    state.save_tool_config(&app_handle).await?;

    Ok(())
}

/// Get a summary of tool configuration
#[tauri::command]
pub async fn get_tool_configuration_summary(state: State<'_, AppState>) -> Result<Value, String> {
    info!("Getting tool configuration summary");

    let config_manager = state.get_tool_config_manager().await;
    let config_guard = config_manager.lock().await;

    let enabled_tools = config_guard.get_enabled_tools();
    let total_tools = config_guard.tools.len();
    let enabled_tool_count = enabled_tools.len();
    let total_categories = config_guard.category_enabled.len();
    let enabled_categories = config_guard
        .category_enabled
        .values()
        .filter(|&&enabled| enabled)
        .count();
    let required_tools = config_guard
        .tools
        .values()
        .filter(|tool| tool.required)
        .count();

    Ok(json!({
        "total_tools": total_tools,
        "enabled_tools": enabled_tool_count,
        "total_categories": total_categories,
        "enabled_categories": enabled_categories,
        "required_tools": required_tools,
        "mcp_servers": config_guard.mcp_servers.len(),
        "enabled_mcp_servers": config_guard.mcp_servers.values().filter(|server| server.enabled).count()
    }))
}

/// Simple test command to verify tool config system works
#[tauri::command]
pub async fn test_tool_config(state: State<'_, AppState>) -> Result<String, String> {
    info!("Testing tool configuration system");

    let config_manager = state.get_tool_config_manager().await;
    let config_guard = config_manager.lock().await;

    let tool_count = config_guard.tools.len();
    let category_count = config_guard.category_enabled.len();
    let enabled_count = config_guard.get_enabled_tools().len();

    Ok(format!(
        "Tool config system working! {} tools ({} enabled), {} categories",
        tool_count, enabled_count, category_count
    ))
}

/// Set tool approval required setting
#[tauri::command]
pub async fn set_tool_approval_required(
    required: bool,
    state: State<'_, AppState>,
) -> Result<(), String> {
    info!("Setting tool approval required: {}", required);
    let _ = state.set_tool_approval_required(required);
    Ok(())
}

/// Get tool approval required setting
#[tauri::command]
pub async fn get_tool_approval_required(state: State<'_, AppState>) -> Result<bool, String> {
    let required = state.is_tool_approval_required();
    info!("Current tool approval required setting: {}", required);
    Ok(required)
}

/// Approve a pending tool execution
#[tauri::command]
pub async fn approve_tool_execution(
    tool_id: String,
    state: State<'_, AppState>,
) -> Result<bool, String> {
    info!("Approving tool execution for ID: {}", tool_id);
    let success = state.approve_tool(&tool_id).await;
    if success {
        info!("Tool {} approved successfully", tool_id);
    } else {
        info!("Tool {} not found in pending approvals", tool_id);
    }
    Ok(success)
}

/// Deny a pending tool execution
#[tauri::command]
pub async fn deny_tool_execution(
    tool_id: String,
    state: State<'_, AppState>,
) -> Result<bool, String> {
    info!("Denying tool execution for ID: {}", tool_id);
    let success = state.deny_tool(&tool_id).await;
    if success {
        info!("Tool {} denied successfully", tool_id);
    } else {
        info!("Tool {} not found in pending approvals", tool_id);
    }
    Ok(success)
}

/// Get all pending tool approval requests
#[tauri::command]
pub async fn get_pending_tool_approvals(state: State<'_, AppState>) -> Result<Value, String> {
    info!("Getting pending tool approval requests");
    let pending_approvals = state.get_pending_tool_approvals().await;
    Ok(json!(pending_approvals))
}

/// Clear all pending tool approval requests
#[tauri::command]
pub async fn clear_pending_tool_approvals(state: State<'_, AppState>) -> Result<(), String> {
    info!("Clearing all pending tool approval requests");
    state.clear_pending_tool_approvals().await;
    Ok(())
}

/// Simple test command to verify tool config system works
#[tauri::command]
pub async fn test_tool_config_command() -> Result<String, String> {
    Ok("Tool config system is working".to_string())
}

/// Get all currently registered tools from the tool provider
/// This provides dynamic discovery of tools without relying on static configurations
#[tauri::command]
pub async fn get_registered_tools(state: State<'_, AppState>) -> Result<Value, String> {
    info!("Getting all registered tools dynamically");

    // Get the current tool provider instance
    let tool_provider_registry = state.tool_provider_registry.lock().await;

    if let Some(provider_weak) = tool_provider_registry.first() {
        if let Some(provider_arc) = provider_weak.upgrade() {
            let provider_guard = provider_arc.lock().await;

            // Get all tools directly from the provider (bypasses filtering)
            let all_tools_defs = provider_guard.get_all_registered_tools().await;
            let mut all_tools = Vec::new();

            for tool_def in all_tools_defs {
                all_tools.push(json!({
                    "name": tool_def.name,
                    "description": tool_def.description,
                    "input_schema": tool_def.input_schema,
                    "api_type": tool_def.api_type,
                    "beta_flag": tool_def.beta_flag
                }));
            }

            info!("Found {} registered tools", all_tools.len());
            return Ok(json!({
                "tools": all_tools,
                "total_count": all_tools.len()
            }));
        }
    }

    // Fallback: if no provider found, return empty list
    Ok(json!({
        "tools": [],
        "total_count": 0,
        "note": "No tool provider found - this might indicate a system issue"
    }))
}

/// Test the dynamic tool categorization system
/// This demonstrates how tools are automatically categorized without static mappings
#[tauri::command]
pub async fn test_dynamic_tool_categorization() -> Result<Value, String> {
    use crate::agent::implementations::tool_provider::LocalToolProvider;

    // Test data - examples of tool names and descriptions
    let test_tools = vec![
        ("screenshot", "Take a screenshot of the desktop"),
        ("safari_navigate", "Navigate to a URL in Safari browser"),
        ("set_timer", "Set a timer for a specific duration"),
        ("read_file", "Read contents of a file"),
        ("get_window_list", "Get list of all open windows"),
        ("mcp_server_tool", "External MCP server tool"),
        ("click", "Click at coordinates on screen"),
        ("browser_close_tab", "Close a browser tab"),
        ("file_monitor", "Monitor file changes"),
        ("type_text", "Type text using keyboard"),
    ];

    let mut results = Vec::new();

    for (tool_name, description) in test_tools {
        let category = LocalToolProvider::infer_tool_category(tool_name, description);
        results.push(json!({
            "tool_name": tool_name,
            "description": description,
            "inferred_category": format!("{:?}", category)
        }));
    }

    Ok(json!({
        "message": "Dynamic tool categorization test completed",
        "test_results": results,
        "note": "This shows how tools are automatically categorized by name and description patterns"
    }))
}

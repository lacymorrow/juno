use tauri::{AppHandle, State};
use serde_json::{json, Value};
use tracing::info;

use crate::state::AppState;
use crate::agent::tools::{ToolCategory, ToolConfig};

/// Get all tool configurations organized by category
#[tauri::command]
pub async fn get_tool_configurations(
    app_handle: AppHandle,
    state: State<'_, AppState>,
) -> Result<Value, String> {
    // Reduced logging frequency - only log at debug level
    tracing::debug!("Getting tool configurations");

    // Load tool configuration from file if needed
    if let Err(e) = state.load_tool_config(&app_handle).await {
        tracing::warn!("Failed to load tool config: {}, using defaults", e);
    }

    let config_manager = state.get_tool_config_manager().await;
    let config_guard = config_manager.lock().await;

    let mut result = serde_json::Map::new();

    // Organize tools by category
    for category in ToolCategory::all_categories() {
        let tools_in_category = config_guard.get_tools_by_category(&category);
        let category_enabled = config_guard.category_enabled.get(&category).unwrap_or(&true);

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

        result.insert(category.display_name().to_string(), category_info);
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

    // Parse category from string
    let tool_category = match category.as_str() {
        "AnthropicComputerUse" => ToolCategory::AnthropicComputerUse,
        "Desktop" => ToolCategory::Desktop,
        "Browser" => ToolCategory::Browser,
        "Timer" => ToolCategory::Timer,
        "Basic" => ToolCategory::Basic,
        "MCP" => ToolCategory::MCP,
        _ => return Err(format!("Unknown category: {}", category)),
    };

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
pub async fn get_enabled_tools(
    state: State<'_, AppState>,
) -> Result<Vec<String>, String> {
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
pub async fn get_tool_configuration_summary(
    state: State<'_, AppState>,
) -> Result<Value, String> {
    info!("Getting tool configuration summary");

    let config_manager = state.get_tool_config_manager().await;
    let config_guard = config_manager.lock().await;

    let enabled_tools = config_guard.get_enabled_tools();
    let total_tools = config_guard.tools.len();
    let enabled_tool_count = enabled_tools.len();
    let total_categories = config_guard.category_enabled.len();
    let enabled_categories = config_guard.category_enabled.values().filter(|&&enabled| enabled).count();
    let required_tools = config_guard.tools.values().filter(|tool| tool.required).count();

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
pub async fn test_tool_config(
    state: State<'_, AppState>,
) -> Result<String, String> {
    info!("Testing tool configuration system");

    let config_manager = state.get_tool_config_manager().await;
    let config_guard = config_manager.lock().await;

    let tool_count = config_guard.tools.len();
    let category_count = config_guard.category_enabled.len();
    let enabled_count = config_guard.get_enabled_tools().len();

    Ok(format!("Tool config system working! {} tools ({} enabled), {} categories", tool_count, enabled_count, category_count))
}

/// Set tool approval required setting
#[tauri::command]
pub async fn set_tool_approval_required(
    required: bool,
    state: State<'_, AppState>,
) -> Result<(), String> {
    info!("Setting tool approval required: {}", required);
    state.set_tool_approval_required(required);
    Ok(())
}

/// Get tool approval required setting
#[tauri::command]
pub async fn get_tool_approval_required(
    state: State<'_, AppState>,
) -> Result<bool, String> {
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
pub async fn get_pending_tool_approvals(
    state: State<'_, AppState>,
) -> Result<Value, String> {
    info!("Getting pending tool approval requests");
    let pending_approvals = state.get_pending_tool_approvals().await;
    Ok(json!(pending_approvals))
}

/// Clear all pending tool approval requests
#[tauri::command]
pub async fn clear_pending_tool_approvals(
    state: State<'_, AppState>,
) -> Result<(), String> {
    info!("Clearing all pending tool approval requests");
    state.clear_pending_tool_approvals().await;
    Ok(())
}

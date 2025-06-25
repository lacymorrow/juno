//! Debug commands for tool configuration diagnostics

use crate::state::AppState;
use serde_json::{json, Value};
use tauri::{command, State};
use tracing::{info, warn};

/// Debug command to get comprehensive tool configuration information
#[command]
pub async fn debug_tool_configuration(state: State<'_, AppState>) -> Result<Value, String> {
    info!("=== TOOL CONFIGURATION DEBUG ===");

    let config_manager = state.get_tool_config_manager().await;
    let config_guard = config_manager.lock().await;

    // Check computer tool specifically
    let computer_config = config_guard.get_tool_config("computer");
    let computer_enabled = config_guard.is_tool_enabled("computer");

    // Get category status
    let anthropic_category_enabled = config_guard.category_enabled
        .get(&crate::agent::tools::tool_config::ToolCategory::AnthropicComputerUse)
        .copied()
        .unwrap_or(true);

    // Get all tools in AnthropicComputerUse category
    let anthropic_tools = config_guard.get_tools_by_category(
        &crate::agent::tools::tool_config::ToolCategory::AnthropicComputerUse
    );

    // Get all enabled tools
    let enabled_tools = config_guard.get_enabled_tool_names();

    // Count tools by category
    let mut category_counts = std::collections::HashMap::new();
    for (_, tool_config) in &config_guard.tools {
        let category_name = format!("{:?}", tool_config.category);
        *category_counts.entry(category_name).or_insert(0) += 1;
    }

    let debug_info = json!({
        "computer_tool": {
            "exists": computer_config.is_some(),
            "config": computer_config,
            "enabled": computer_enabled,
        },
        "anthropic_category": {
            "enabled": anthropic_category_enabled,
            "tools_count": anthropic_tools.len(),
            "tools": anthropic_tools.iter().map(|t| json!({
                "name": t.name,
                "enabled": t.enabled,
                "required": t.required
            })).collect::<Vec<_>>()
        },
        "total_tools": config_guard.tools.len(),
        "enabled_tools_count": enabled_tools.len(),
        "enabled_tools": enabled_tools,
        "category_counts": category_counts,
        "all_categories": config_guard.category_enabled.iter().map(|(k, v)| {
            (format!("{:?}", k), v)
        }).collect::<std::collections::HashMap<_, _>>()
    });

    info!("Computer tool exists: {:?}", computer_config.is_some());
    info!("Computer tool enabled: {}", computer_enabled);
    info!("AnthropicComputerUse category enabled: {}", anthropic_category_enabled);
    info!("Total tools: {}", config_guard.tools.len());
    info!("Enabled tools: {}", enabled_tools.len());

    if let Some(ref config) = computer_config {
        info!("Computer tool config: required={}, enabled={}", config.required, config.enabled);
    } else {
        warn!("Computer tool configuration not found!");
    }

    Ok(debug_info)
}

/// Debug command to check what tools are actually registered with the tool provider
#[command]
pub async fn debug_registered_tools(state: State<'_, AppState>) -> Result<Value, String> {
    info!("=== REGISTERED TOOLS DEBUG ===");

    // This would require access to the tool provider, which isn't directly available
    // from the state. We'll need to check this differently.

    Ok(json!({
        "note": "Tool provider inspection would require additional implementation",
        "suggestion": "Check logs during tool registration for more details"
    }))
}

/// Reset tool configuration to defaults and report what changed
#[command]
pub async fn debug_reset_tool_config(
    app_handle: tauri::AppHandle,
    state: State<'_, AppState>
) -> Result<Value, String> {
    info!("=== RESETTING TOOL CONFIGURATION ===");

    let config_manager = state.get_tool_config_manager().await;

    // Get current state before reset
    let before_computer_enabled = {
        let config_guard = config_manager.lock().await;
        config_guard.is_tool_enabled("computer")
    };

    // Reset to defaults
    {
        let mut config_guard = config_manager.lock().await;
        config_guard.reset_to_defaults();
    }

    // Save the reset configuration
    state.save_tool_config(&app_handle).await?;

    // Get state after reset
    let after_computer_enabled = {
        let config_guard = config_manager.lock().await;
        config_guard.is_tool_enabled("computer")
    };

    let reset_info = json!({
        "reset_completed": true,
        "computer_tool": {
            "before_reset": before_computer_enabled,
            "after_reset": after_computer_enabled,
            "changed": before_computer_enabled != after_computer_enabled
        }
    });

    info!("Tool configuration reset completed");
    info!("Computer tool enabled before reset: {}", before_computer_enabled);
    info!("Computer tool enabled after reset: {}", after_computer_enabled);

    Ok(reset_info)
}

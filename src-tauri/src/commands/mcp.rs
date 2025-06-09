use std::collections::HashMap;
use tauri::{AppHandle, State};
use tracing::{error, info};

use crate::state::AppState;
use crate::agent::tools::{MCPServerConfig, MCPServerStatus, MCPToolInfo};

/// Add a new MCP server configuration
#[tauri::command]
pub async fn add_mcp_server(
    app_handle: AppHandle,
    state: State<'_, AppState>,
    config: MCPServerConfig,
) -> Result<(), String> {
    info!("Adding MCP server: {}", config.name);

    // Add to MCP manager
    let mcp_manager = state.get_mcp_manager().await;
    {
        let manager_guard = mcp_manager.lock().await;
        manager_guard.add_server(config.clone()).await?;
    }

    // Add to tool configuration
    {
        let tool_config = state.get_tool_config_manager().await;
        let mut config_guard = tool_config.lock().await;
        config_guard.add_mcp_server(config);
    }

    // Save configuration
    state.save_tool_config(&app_handle).await?;

    // Sync tools
    state.sync_mcp_tools().await?;

    Ok(())
}

/// Remove an MCP server
#[tauri::command]
pub async fn remove_mcp_server(
    app_handle: AppHandle,
    state: State<'_, AppState>,
    server_id: String,
) -> Result<(), String> {
    info!("Removing MCP server: {}", server_id);

    // Remove from MCP manager
    let mcp_manager = state.get_mcp_manager().await;
    {
        let manager_guard = mcp_manager.lock().await;
        manager_guard.remove_server(&server_id).await?;
    }

    // Remove from tool configuration
    {
        let tool_config = state.get_tool_config_manager().await;
        let mut config_guard = tool_config.lock().await;
        config_guard.remove_mcp_server(&server_id);
    }

    // Save configuration
    state.save_tool_config(&app_handle).await?;

    Ok(())
}

/// Start an MCP server
#[tauri::command]
pub async fn start_mcp_server(
    state: State<'_, AppState>,
    server_id: String,
) -> Result<(), String> {
    info!("Starting MCP server: {}", server_id);

    let mcp_manager = state.get_mcp_manager().await;
    let manager_guard = mcp_manager.lock().await;
    manager_guard.start_server(&server_id).await?;

    // Sync tools after starting
    drop(manager_guard);
    state.sync_mcp_tools().await?;

    Ok(())
}

/// Stop an MCP server
#[tauri::command]
pub async fn stop_mcp_server(
    state: State<'_, AppState>,
    server_id: String,
) -> Result<(), String> {
    info!("Stopping MCP server: {}", server_id);

    let mcp_manager = state.get_mcp_manager().await;
    let manager_guard = mcp_manager.lock().await;
    manager_guard.stop_server(&server_id).await?;

    Ok(())
}

/// Get all MCP server configurations
#[tauri::command]
pub async fn get_mcp_servers(
    state: State<'_, AppState>,
) -> Result<Vec<MCPServerConfig>, String> {
    let tool_config = state.get_tool_config_manager().await;
    let config_guard = tool_config.lock().await;
    Ok(config_guard.get_mcp_servers())
}

/// Get MCP server statuses
#[tauri::command]
pub async fn get_mcp_server_statuses(
    state: State<'_, AppState>,
) -> Result<HashMap<String, MCPServerStatus>, String> {
    let mcp_manager = state.get_mcp_manager().await;
    let manager_guard = mcp_manager.lock().await;
    Ok(manager_guard.get_server_statuses().await)
}

/// Get all MCP tools
#[tauri::command]
pub async fn get_mcp_tools(
    state: State<'_, AppState>,
) -> Result<Vec<MCPToolInfo>, String> {
    let mcp_manager = state.get_mcp_manager().await;
    let manager_guard = mcp_manager.lock().await;
    Ok(manager_guard.get_all_tools().await)
}

/// Update MCP server configuration
#[tauri::command]
pub async fn update_mcp_server(
    app_handle: AppHandle,
    state: State<'_, AppState>,
    config: MCPServerConfig,
) -> Result<(), String> {
    info!("Updating MCP server: {}", config.name);

    // Update in tool configuration
    {
        let tool_config = state.get_tool_config_manager().await;
        let mut config_guard = tool_config.lock().await;
        config_guard.update_mcp_server(config.clone());
    }

    // Save configuration
    state.save_tool_config(&app_handle).await?;

    // Note: Server will need to be restarted to apply changes
    info!("MCP server configuration updated. Restart required to apply changes.");

    Ok(())
}

/// Enable or disable an MCP server
#[tauri::command]
pub async fn set_mcp_server_enabled(
    app_handle: AppHandle,
    state: State<'_, AppState>,
    server_id: String,
    enabled: bool,
) -> Result<(), String> {
    info!("Setting MCP server {} enabled: {}", server_id, enabled);

    // Update in tool configuration
    {
        let tool_config = state.get_tool_config_manager().await;
        let mut config_guard = tool_config.lock().await;
        config_guard.set_mcp_server_enabled(&server_id, enabled);
    }

    // Save configuration
    state.save_tool_config(&app_handle).await?;

    // Start or stop the server based on enabled status
    if enabled {
        if let Err(e) = start_mcp_server(state.clone(), server_id).await {
            error!("Failed to start MCP server: {}", e);
        }
    } else {
        if let Err(e) = stop_mcp_server(state, server_id).await {
            error!("Failed to stop MCP server: {}", e);
        }
    }

    Ok(())
}

/// Test MCP server connection (without adding it permanently)
#[tauri::command]
pub async fn test_mcp_server_connection(
    config: MCPServerConfig,
) -> Result<Vec<String>, String> {
    info!("Testing MCP server connection: {}", config.name);

    // Create a temporary connection to test
    let mut connection = crate::agent::tools::mcp_integration::MCPServerConnection::new(config);

    match connection.connect().await {
        Ok(()) => {
            let tool_names: Vec<String> = connection.get_tools()
                .iter()
                .map(|tool| tool.name.clone())
                .collect();

            connection.disconnect().await;

            info!("MCP server test successful. Found {} tools", tool_names.len());
            Ok(tool_names)
        }
        Err(e) => {
            connection.disconnect().await;
            Err(format!("Connection test failed: {}", e))
        }
    }
}

/// Initialize all MCP servers from configuration
#[tauri::command]
pub async fn initialize_mcp_servers(
    state: State<'_, AppState>,
) -> Result<(), String> {
    info!("Initializing MCP servers from configuration");
    state.initialize_mcp_servers().await
}

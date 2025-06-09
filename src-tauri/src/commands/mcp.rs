use std::collections::HashMap;
use tauri::{AppHandle, State};
use tracing::{error, info, warn};

use crate::state::AppState;
use crate::agent::tools::{MCPServerConfig, MCPServerStatus, MCPToolInfo};
use crate::agent::tools::mcp_integration::MCPManager;

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
        let mut tool_config = state.get_tool_config_manager().await;
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
        let mut tool_config = state.get_tool_config_manager().await;
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
        let mut tool_config = state.get_tool_config_manager().await;
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
        let mut tool_config = state.get_tool_config_manager().await;
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
    state: State<'_, AppState>,
    config: MCPServerConfig,
) -> Result<String, String> {
    info!("Testing MCP server connection: {}", config.name);

    // Create a temporary MCP manager for testing
    let test_manager = MCPManager::new();
    
    // Try to add and start the server
    match test_manager.add_server(config.clone()).await {
        Ok(_) => {
            info!("Test server added successfully");
            
            // Wait a moment for the server to start
            tokio::time::sleep(std::time::Duration::from_millis(500)).await;
            
            // Check the status
            let statuses = test_manager.get_server_statuses().await;
            if let Some(status) = statuses.get(&config.id) {
                match status {
                    MCPServerStatus::Connected => {
                        let tools = test_manager.get_all_tools().await;
                        let tool_count = tools.len();
                        test_manager.stop_server(&config.id).await?;
                        Ok(format!("Connection successful! Server '{}' is working and provides {} tools.", config.name, tool_count))
                    },
                    MCPServerStatus::Connecting => {
                        test_manager.stop_server(&config.id).await?;
                        Err(format!("Server '{}' is still connecting - this may indicate a slow startup or communication issue.", config.name))
                    },
                    MCPServerStatus::Error(err) => {
                        test_manager.stop_server(&config.id).await?;
                        Err(format!("Server '{}' failed to start: {}", config.name, err))
                    },
                    MCPServerStatus::Timeout => {
                        test_manager.stop_server(&config.id).await?;
                        Err(format!("Server '{}' timed out during startup.", config.name))
                    },
                    MCPServerStatus::Disconnected => {
                        Err(format!("Server '{}' is disconnected - startup may have failed.", config.name))
                    },
                }
            } else {
                Err(format!("Could not find status for test server '{}'", config.name))
            }
        },
        Err(e) => {
            error!("Failed to add test server: {}", e);
            Err(format!("Failed to add test server '{}': {}", config.name, e))
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

/// Get detailed diagnostic information about MCP servers
#[tauri::command]
pub async fn get_mcp_diagnostics(
    state: State<'_, AppState>,
) -> Result<serde_json::Value, String> {
    info!("Getting MCP diagnostics");

    let mcp_manager = state.get_mcp_manager().await;
    let manager_guard = mcp_manager.lock().await;

    let configs = manager_guard.get_server_configs().await;
    let statuses = manager_guard.get_server_statuses().await;
    let tools = manager_guard.get_all_tools().await;

    drop(manager_guard);

    let mut diagnostics = serde_json::json!({
        "total_servers": configs.len(),
        "connected_servers": 0,
        "total_tools": tools.len(),
        "servers": []
    });

    let mut connected_count = 0;
    let mut server_details = Vec::new();

    for config in configs {
        let status = statuses.get(&config.id);
        let is_connected = matches!(status, Some(MCPServerStatus::Connected));
        
        if is_connected {
            connected_count += 1;
        }

        let server_tools: Vec<_> = tools.iter()
            .filter(|t| t.server_id == config.id)
            .map(|t| serde_json::json!({
                "name": t.tool_definition.name,
                "description": t.tool_definition.description,
                "enabled": t.enabled
            }))
            .collect();

        let status_info = match status {
            Some(MCPServerStatus::Connected) => serde_json::json!({"status": "connected"}),
            Some(MCPServerStatus::Connecting) => serde_json::json!({"status": "connecting"}),
            Some(MCPServerStatus::Error(err)) => serde_json::json!({"status": "error", "error": err}),
            Some(MCPServerStatus::Timeout) => serde_json::json!({"status": "timeout"}),
            Some(MCPServerStatus::Disconnected) => serde_json::json!({"status": "disconnected"}),
            None => serde_json::json!({"status": "unknown"}),
        };

        server_details.push(serde_json::json!({
            "id": config.id,
            "name": config.name,
            "command": config.command,
            "args": config.args,
            "enabled": config.enabled,
            "auto_start": config.auto_start,
            "timeout_seconds": config.timeout_seconds,
            "max_retries": config.max_retries,
            "working_directory": config.working_directory,
            "environment_variables": config.environment_variables,
            "status": status_info,
            "tools": server_tools
        }));
    }

    diagnostics["connected_servers"] = serde_json::json!(connected_count);
    diagnostics["servers"] = serde_json::json!(server_details);

    Ok(diagnostics)
}

/// Restart a specific MCP server with enhanced logging
#[tauri::command]
pub async fn restart_mcp_server_with_diagnostics(
    state: State<'_, AppState>,
    server_id: String,
) -> Result<String, String> {
    info!("Restarting MCP server with diagnostics: {}", server_id);

    let mcp_manager = state.get_mcp_manager().await;
    let manager_guard = mcp_manager.lock().await;

    // First, try to stop the server
    if let Err(e) = manager_guard.stop_server(&server_id).await {
        warn!("Failed to stop server {} (may already be stopped): {}", server_id, e);
    }

    // Wait a moment
    tokio::time::sleep(std::time::Duration::from_millis(500)).await;

    // Try to start the server
    match manager_guard.start_server(&server_id).await {
        Ok(_) => {
            // Wait for startup
            tokio::time::sleep(std::time::Duration::from_millis(1000)).await;
            
            // Check final status
            let statuses = manager_guard.get_server_statuses().await;
            if let Some(status) = statuses.get(&server_id) {
                match status {
                    MCPServerStatus::Connected => {
                        let tools = manager_guard.get_all_tools().await;
                        let server_tools: Vec<_> = tools.iter()
                            .filter(|t| t.server_id == server_id)
                            .collect();
                        Ok(format!("Server restarted successfully! Found {} tools.", server_tools.len()))
                    },
                    MCPServerStatus::Error(err) => {
                        Err(format!("Server restart failed: {}", err))
                    },
                    _ => {
                        Ok(format!("Server restarted but status is: {:?}", status))
                    }
                }
            } else {
                Err("Server not found after restart".to_string())
            }
        },
        Err(e) => {
            error!("Failed to restart MCP server {}: {}", server_id, e);
            Err(format!("Failed to restart server: {}", e))
        }
    }
}

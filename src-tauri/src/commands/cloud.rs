use tauri::{State, AppHandle};
use serde::{Deserialize, Serialize};
use tracing::{info, error, debug, warn};

use crate::state::AppState;
use crate::cloud::{CloudConfig, types::ConnectionState};

/// Cloud configuration response for frontend
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CloudConfigResponse {
    pub enabled: bool,
    pub server_url: String,
    pub device_name: String,
    pub device_id: Option<String>,
    pub security_level: String,
    pub auto_connect: bool,
}

/// Cloud status response for frontend
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CloudStatusResponse {
    pub enabled: bool,
    pub connected: bool,
    pub connection_state: String,
    pub device_id: Option<String>,
    pub last_error: Option<String>,
}

/// Get current cloud configuration
#[tauri::command]
pub async fn get_cloud_config(
    app_handle: AppHandle,
    app_state: State<'_, AppState>,
) -> Result<CloudConfigResponse, String> {
    info!("Getting cloud configuration");
    
    let config = app_state.get_cloud_config().await;
    
    Ok(CloudConfigResponse {
        enabled: config.enabled,
        server_url: config.server_url,
        device_name: config.device_name,
        device_id: config.device_id,
        security_level: format!("{:?}", config.security_level),
        auto_connect: config.auto_connect,
    })
}

/// Update cloud configuration
#[tauri::command]
pub async fn update_cloud_config(
    enabled: bool,
    server_url: Option<String>,
    device_name: Option<String>,
    api_key: Option<String>,
    security_level: Option<String>,
    auto_connect: Option<bool>,
    app_handle: AppHandle,
    app_state: State<'_, AppState>,
) -> Result<(), String> {
    info!("Updating cloud configuration");
    
    let mut config = app_state.get_cloud_config().await;
    
    // Update configuration fields
    config.enabled = enabled;
    
    if let Some(url) = server_url {
        config.server_url = url;
    }
    
    if let Some(name) = device_name {
        config.device_name = name;
    }
    
    if let Some(key) = api_key {
        config.api_key = Some(key);
    }
    
    if let Some(level) = security_level {
        config.security_level = match level.as_str() {
            "low" => crate::cloud::config::SecurityLevel::Low,
            "medium" => crate::cloud::config::SecurityLevel::Medium,
            "high" => crate::cloud::config::SecurityLevel::High,
            _ => config.security_level, // Keep existing if invalid
        };
    }
    
    if let Some(auto) = auto_connect {
        config.auto_connect = auto;
    }
    
    // Apply the configuration
    app_state.update_cloud_config(config, &app_handle).await?;
    
    info!("Cloud configuration updated successfully");
    Ok(())
}

/// Get cloud connection status
#[tauri::command]
pub async fn get_cloud_status(
    app_state: State<'_, AppState>,
) -> Result<CloudStatusResponse, String> {
    let enabled = app_state.is_cloud_enabled();
    let mut connected = false;
    let mut connection_state = "disconnected".to_string();
    let mut device_id = None;
    let last_error = None; // TODO: Track last error
    
    // Get connection state if cloud client exists
    if enabled {
        if let Some(client) = app_state.cloud_client.lock().await.as_ref() {
            let state = client.get_connection_state().await;
            connection_state = match state {
                ConnectionState::Disconnected => "disconnected".to_string(),
                ConnectionState::Connecting => "connecting".to_string(),
                ConnectionState::Connected => "connected".to_string(),
                ConnectionState::Authenticated => {
                    connected = true;
                    "authenticated".to_string()
                },
                ConnectionState::Error(err) => {
                    error!("Cloud connection error: {}", err);
                    "error".to_string()
                },
            };
        }
        
        // Get device ID from config
        let config = app_state.get_cloud_config().await;
        device_id = config.device_id;
    }
    
    Ok(CloudStatusResponse {
        enabled,
        connected,
        connection_state,
        device_id,
        last_error,
    })
}

/// Enable cloud connectivity
#[tauri::command]
pub async fn enable_cloud(
    app_handle: AppHandle,
    app_state: State<'_, AppState>,
) -> Result<(), String> {
    info!("Enabling cloud connectivity");
    
    let mut config = app_state.get_cloud_config().await;
    config.enabled = true;
    
    // Validate configuration before enabling
    config.validate()
        .map_err(|e| format!("Configuration validation failed: {}", e))?;
    
    app_state.update_cloud_config(config, &app_handle).await?;
    
    info!("Cloud connectivity enabled");
    Ok(())
}

/// Disable cloud connectivity
#[tauri::command]
pub async fn disable_cloud(
    app_handle: AppHandle,
    app_state: State<'_, AppState>,
) -> Result<(), String> {
    info!("Disabling cloud connectivity");
    
    let mut config = app_state.get_cloud_config().await;
    config.enabled = false;
    
    app_state.update_cloud_config(config, &app_handle).await?;
    
    info!("Cloud connectivity disabled");
    Ok(())
}

/// Test cloud connection
#[tauri::command]
pub async fn test_cloud_connection(
    app_state: State<'_, AppState>,
) -> Result<bool, String> {
    info!("Testing cloud connection");
    
    if !app_state.is_cloud_enabled() {
        return Err("Cloud connectivity is disabled".to_string());
    }
    
    // Check if we have an active connection
    if let Some(client) = app_state.cloud_client.lock().await.as_ref() {
        let state = client.get_connection_state().await;
        match state {
            ConnectionState::Authenticated => {
                info!("Cloud connection test: Connected and authenticated");
                Ok(true)
            },
            ConnectionState::Connected => {
                info!("Cloud connection test: Connected but not authenticated");
                Ok(false)
            },
            ConnectionState::Connecting => {
                info!("Cloud connection test: Currently connecting");
                Ok(false)
            },
            ConnectionState::Disconnected => {
                info!("Cloud connection test: Disconnected");
                Ok(false)
            },
            ConnectionState::Error(err) => {
                error!("Cloud connection test: Error - {}", err);
                Err(format!("Connection error: {}", err))
            },
        }
    } else {
        info!("Cloud connection test: No client available");
        Ok(false)
    }
}

/// Get cloud device information
#[tauri::command]
pub async fn get_cloud_device_info(
    app_state: State<'_, AppState>,
) -> Result<serde_json::Value, String> {
    info!("Getting cloud device information");
    
    let config = app_state.get_cloud_config().await;
    
    let device_info = serde_json::json!({
        "device_id": config.device_id,
        "device_name": config.device_name,
        "platform": std::env::consts::OS,
        "version": env!("CARGO_PKG_VERSION"),
        "capabilities": [
            "text_processing",
            "voice_transcription",
            "screenshot_capture",
            "system_automation",
            "file_operations",
            "web_browsing"
        ],
        "permissions": {
            "desktop_available": app_state.is_desktop_available(),
            "permissions_checked": app_state.are_permissions_checked()
        }
    });
    
    Ok(device_info)
}

/// Generate new device ID
#[tauri::command]
pub async fn generate_device_id(
    app_handle: AppHandle,
    app_state: State<'_, AppState>,
) -> Result<String, String> {
    info!("Generating new device ID");
    
    let new_device_id = crate::cloud::auth::DeviceAuth::generate_device_id();
    
    // Update configuration with new device ID
    let mut config = app_state.get_cloud_config().await;
    config.device_id = Some(new_device_id.clone());
    
    app_state.update_cloud_config(config, &app_handle).await?;
    
    info!("Generated new device ID: {}", new_device_id);
    Ok(new_device_id)
}

#[tauri::command]
pub async fn handle_cloud_message(
    connection_id: String,
    message: String,
    app_state: tauri::State<'_, AppState>,
) -> Result<(), String> {
    info!("Received cloud message from connection {}: {}", connection_id, message);
    
    // Parse the WebSocket message
    let ws_message: crate::cloud::types::WebSocketMessage = serde_json::from_str(&message)
        .map_err(|e| format!("Failed to parse WebSocket message: {}", e))?;
    
    // Handle different message types
    match ws_message.message_type {
        crate::cloud::types::MessageType::Command => {
            let command: crate::cloud::types::CloudCommand = serde_json::from_value(ws_message.data)
                .map_err(|e| format!("Failed to parse cloud command: {}", e))?;
            
            // Get the production connector from app state
            if let Some(connector) = app_state.get_production_cloud_connector() {
                match connector.execute_remote_command(command).await {
                    Ok(response) => {
                        // Send response back via WebSocket
                        if let Err(e) = send_cloud_response(response).await {
                            error!("Failed to send cloud response: {}", e);
                        }
                    },
                    Err(e) => {
                        error!("Failed to execute remote command: {}", e);
                        return Err(format!("Command execution failed: {}", e));
                    }
                }
            } else {
                return Err("Production cloud connector not available".to_string());
            }
        },
        crate::cloud::types::MessageType::Auth => {
            info!("Received authentication message from cloud");
            // Authentication is handled by the connector itself
        },
        crate::cloud::types::MessageType::Heartbeat => {
            debug!("Received heartbeat from cloud");
            // Heartbeat is handled automatically
        },
        crate::cloud::types::MessageType::Error => {
            let error_msg = ws_message.data.get("message")
                .and_then(|m| m.as_str())
                .unwrap_or("Unknown cloud error");
            error!("Received error from cloud: {}", error_msg);
        },
        _ => {
            warn!("Unhandled cloud message type: {:?}", ws_message.message_type);
        }
    }
    
    Ok(())
}

/// Send response back to cloud via WebSocket
async fn send_cloud_response(response: crate::cloud::types::DeviceResponse) -> Result<(), String> {
    // This would be implemented to send the response back through the WebSocket
    // For now, we'll emit it as an event that the frontend can catch and send
    info!("Cloud response ready: {:?}", response);
    Ok(())
}

#[tauri::command]
pub async fn start_production_cloud_connector(
    app_handle: tauri::AppHandle,
    app_state: tauri::State<'_, AppState>,
) -> Result<(), String> {
    info!("Starting production cloud connector");
    
    match crate::cloud::ProductionCloudConnector::new(app_handle).await {
        Ok(connector) => {
            match connector.start().await {
                Ok(()) => {
                    // Store the connector in app state
                    app_state.set_production_cloud_connector(connector).await;
                    info!("Production cloud connector started successfully");
                    Ok(())
                },
                Err(e) => {
                    error!("Failed to start production cloud connector: {}", e);
                    Err(format!("Failed to start connector: {}", e))
                }
            }
        },
        Err(e) => {
            error!("Failed to create production cloud connector: {}", e);
            Err(format!("Failed to create connector: {}", e))
        }
    }
}

#[tauri::command]
pub async fn stop_production_cloud_connector(
    app_state: tauri::State<'_, AppState>,
) -> Result<(), String> {
    info!("Stopping production cloud connector");
    
    if let Some(connector) = app_state.get_production_cloud_connector() {
        match connector.disconnect().await {
            Ok(()) => {
                app_state.clear_production_cloud_connector().await;
                info!("Production cloud connector stopped successfully");
                Ok(())
            },
            Err(e) => {
                error!("Failed to stop production cloud connector: {}", e);
                Err(format!("Failed to stop connector: {}", e))
            }
        }
    } else {
        warn!("No production cloud connector to stop");
        Ok(())
    }
}

#[tauri::command]
pub async fn get_production_cloud_status(
    app_state: tauri::State<'_, AppState>,
) -> Result<serde_json::Value, String> {
    if let Some(connector) = app_state.get_production_cloud_connector() {
        let state = connector.get_connection_state().await;
        let stats = connector.get_connection_stats().await;
        
        Ok(serde_json::json!({
            "connected": matches!(state, crate::cloud::connector::ConnectorState::Ready),
            "state": format!("{:?}", state),
            "stats": stats
        }))
    } else {
        Ok(serde_json::json!({
            "connected": false,
            "state": "Not initialized",
            "stats": null
        }))
    }
}
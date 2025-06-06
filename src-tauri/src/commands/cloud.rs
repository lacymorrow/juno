use tauri::{State, AppHandle};
use serde::{Deserialize, Serialize};
use tracing::{info, error};

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
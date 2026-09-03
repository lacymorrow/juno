//! Cloud connectivity testing commands

use crate::cloud::config::CloudConfig;
use crate::settings::manager::SettingsManager;
use tauri::State;

/// Test cloud backend connectivity
#[tauri::command]
pub async fn test_cloud_backend_connection(
    settings_manager: State<'_, SettingsManager>,
) -> Result<String, String> {
    // Load cloud configuration
    let config = CloudConfig::load_from_centralized_settings(&settings_manager)
        .await
        .map_err(|e| format!("Failed to load cloud config: {}", e))?;

    if !config.enabled {
        return Ok("Cloud connectivity is disabled".to_string());
    }

    // Test the connection
    match config.test_connection().await {
        Ok(()) => Ok(format!(
            "✅ Successfully connected to cloud backend: {}",
            config.server_url
        )),
        Err(e) => Err(format!("❌ Cloud connection failed: {}", e)),
    }
}

/// Get current cloud configuration status
#[tauri::command]
pub async fn get_cloud_config_status(
    settings_manager: State<'_, SettingsManager>,
) -> Result<serde_json::Value, String> {
    let config = CloudConfig::load_from_centralized_settings(&settings_manager)
        .await
        .map_err(|e| format!("Failed to load cloud config: {}", e))?;

    Ok(serde_json::json!({
        "enabled": config.enabled,
        "server_url": config.server_url,
        "api_url": config.get_api_url(),
        "health_url": config.get_health_url(),
        "device_name": config.device_name,
        "device_id": config.device_id,
        "auto_connect": config.auto_connect,
        "security_level": config.security_level,
        "command_timeout": config.command_timeout,
        "has_api_key": config.api_key.is_some()
    }))
}

/// Enable cloud connectivity
#[tauri::command]
pub async fn enable_cloud_backend(
    settings_manager: State<'_, SettingsManager>,
) -> Result<String, String> {
    let mut config = CloudConfig::load_from_centralized_settings(&settings_manager)
        .await
        .map_err(|e| format!("Failed to load cloud config: {}", e))?;

    config
        .enable(&settings_manager)
        .await
        .map_err(|e| format!("Failed to enable cloud: {}", e))?;

    Ok("✅ Cloud connectivity enabled".to_string())
}

/// Disable cloud connectivity
#[tauri::command]
pub async fn disable_cloud_backend(
    settings_manager: State<'_, SettingsManager>,
) -> Result<String, String> {
    let mut config = CloudConfig::load_from_centralized_settings(&settings_manager)
        .await
        .map_err(|e| format!("Failed to load cloud config: {}", e))?;

    config
        .disable(&settings_manager)
        .await
        .map_err(|e| format!("Failed to disable cloud: {}", e))?;

    Ok("✅ Cloud connectivity disabled".to_string())
}

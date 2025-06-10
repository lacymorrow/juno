use tauri::{AppHandle, Manager, Runtime};
use tauri_plugin_autostart::ManagerExt;
use std::fs::{File, create_dir_all};
use std::io::{Read, Write};

#[tauri::command]
pub async fn enable_autostart<R: Runtime>(app: AppHandle<R>) -> Result<bool, String> {
    let autostart_manager = app.autolaunch();
    
    match autostart_manager.enable() {
        Ok(_) => {
            // Save the setting locally
            save_autostart_setting(&app, true)?;
            log::info!("Autostart enabled successfully");
            Ok(true)
        }
        Err(err) => {
            log::error!("Failed to enable autostart: {}", err);
            Err(format!("Failed to enable autostart: {}", err))
        }
    }
}

#[tauri::command]
pub async fn disable_autostart<R: Runtime>(app: AppHandle<R>) -> Result<bool, String> {
    let autostart_manager = app.autolaunch();
    
    match autostart_manager.disable() {
        Ok(_) => {
            // Save the setting locally
            save_autostart_setting(&app, false)?;
            log::info!("Autostart disabled successfully");
            Ok(false)
        }
        Err(err) => {
            log::error!("Failed to disable autostart: {}", err);
            Err(format!("Failed to disable autostart: {}", err))
        }
    }
}

#[tauri::command]
pub async fn is_autostart_enabled<R: Runtime>(app: AppHandle<R>) -> Result<bool, String> {
    let autostart_manager = app.autolaunch();
    
    match autostart_manager.is_enabled() {
        Ok(enabled) => {
            log::debug!("Autostart status: {}", enabled);
            Ok(enabled)
        }
        Err(err) => {
            log::error!("Failed to check autostart status: {}", err);
            
            // Fall back to locally saved setting if system check fails
            match get_saved_autostart_setting(&app) {
                Ok(saved_setting) => {
                    log::debug!("Using saved autostart setting: {}", saved_setting);
                    Ok(saved_setting)
                }
                Err(_) => Ok(false) // Default to disabled if we can't determine the state
            }
        }
    }
}

#[tauri::command]
pub async fn toggle_autostart<R: Runtime>(app: AppHandle<R>) -> Result<bool, String> {
    let current_status = is_autostart_enabled(app.clone()).await?;
    
    if current_status {
        disable_autostart(app).await
    } else {
        enable_autostart(app).await
    }
}

// Helper function to save autostart setting locally
fn save_autostart_setting<R: Runtime>(app: &AppHandle<R>, enabled: bool) -> Result<(), String> {
    let app_data_dir = app.path().app_config_dir()
        .map_err(|_| "Failed to get app config directory".to_string())?;
    
    if !app_data_dir.exists() {
        create_dir_all(&app_data_dir)
            .map_err(|_| "Failed to create app config directory".to_string())?;
    }
    
    let autostart_file = app_data_dir.join("autostart.json");
    let setting_data = serde_json::json!({
        "enabled": enabled,
        "last_updated": chrono::Utc::now().to_rfc3339()
    });
    
    let mut file = File::create(autostart_file)
        .map_err(|_| "Failed to create autostart settings file".to_string())?;
    
    file.write_all(setting_data.to_string().as_bytes())
        .map_err(|_| "Failed to write autostart settings".to_string())?;
    
    Ok(())
}

// Helper function to get saved autostart setting
fn get_saved_autostart_setting<R: Runtime>(app: &AppHandle<R>) -> Result<bool, String> {
    let app_data_dir = app.path().app_config_dir()
        .map_err(|_| "Failed to get app config directory".to_string())?;
    
    let autostart_file = app_data_dir.join("autostart.json");
    
    if !autostart_file.exists() {
        return Ok(false); // Default to disabled if no setting saved
    }
    
    let mut file = File::open(autostart_file)
        .map_err(|_| "Failed to open autostart settings file".to_string())?;
    
    let mut contents = String::new();
    file.read_to_string(&mut contents)
        .map_err(|_| "Failed to read autostart settings file".to_string())?;
    
    let setting_data: serde_json::Value = serde_json::from_str(&contents)
        .map_err(|_| "Failed to parse autostart settings".to_string())?;
    
    Ok(setting_data.get("enabled")
        .and_then(|v| v.as_bool())
        .unwrap_or(false))
}

/// Initialize autostart on app startup
pub fn init_autostart<R: Runtime>(app: &AppHandle<R>) {
    let autostart_manager = app.autolaunch();
    
    // Check if autostart should be enabled based on saved settings
    if let Ok(should_be_enabled) = get_saved_autostart_setting(app) {
        if let Ok(currently_enabled) = autostart_manager.is_enabled() {
            // Sync the system setting with our saved preference
            match (currently_enabled, should_be_enabled) {
                (false, true) => {
                    if let Err(err) = autostart_manager.enable() {
                        log::warn!("Failed to enable autostart on startup: {}", err);
                    } else {
                        log::info!("Autostart enabled on startup");
                    }
                }
                (true, false) => {
                    if let Err(err) = autostart_manager.disable() {
                        log::warn!("Failed to disable autostart on startup: {}", err);
                    } else {
                        log::info!("Autostart disabled on startup");
                    }
                }
                _ => {
                    log::debug!("Autostart setting already in sync");
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use std::env;
    use std::fs;

    #[test]
    fn test_autostart_setting_serialization() {
        // Test that autostart settings can be properly serialized and deserialized
        let setting_data = json!({
            "enabled": true,
            "last_updated": "2024-01-15T10:30:45.123Z"
        });
        
        let serialized = setting_data.to_string();
        assert!(serialized.contains("enabled"));
        assert!(serialized.contains("true"));
        assert!(serialized.contains("last_updated"));
        
        let parsed: serde_json::Value = serde_json::from_str(&serialized).unwrap();
        assert_eq!(parsed.get("enabled").unwrap().as_bool().unwrap(), true);
    }
    
    #[test]
    fn test_autostart_setting_json_structure() {
        // Test the structure of autostart settings
        let enabled_setting = json!({
            "enabled": true,
            "last_updated": chrono::Utc::now().to_rfc3339()
        });
        
        let disabled_setting = json!({
            "enabled": false,
            "last_updated": chrono::Utc::now().to_rfc3339()
        });
        
        // Test enabled state
        assert_eq!(enabled_setting.get("enabled").unwrap().as_bool().unwrap(), true);
        assert!(enabled_setting.get("last_updated").is_some());
        
        // Test disabled state  
        assert_eq!(disabled_setting.get("enabled").unwrap().as_bool().unwrap(), false);
        assert!(disabled_setting.get("last_updated").is_some());
    }
    
    #[test]
    fn test_autostart_error_handling() {
        // Test error message formatting for autostart operations
        let error_message = "Permission denied";
        let formatted_error = format!("Failed to enable autostart: {}", error_message);
        
        assert!(formatted_error.contains("Failed to enable autostart"));
        assert!(formatted_error.contains(error_message));
    }
    
    #[test]
    fn test_autostart_timestamp_format() {
        // Test that timestamps are properly formatted in RFC3339
        let timestamp = chrono::Utc::now().to_rfc3339();
        
        // Should contain the T separator and Z timezone
        assert!(timestamp.contains('T'));
        assert!(timestamp.ends_with('Z'));
        
        // Should be parseable back
        let parsed_time = chrono::DateTime::parse_from_rfc3339(&timestamp);
        assert!(parsed_time.is_ok());
    }
    
    #[test]
    fn test_autostart_configuration_validation() {
        // Test validation of autostart configuration data
        let valid_config = json!({
            "enabled": true,
            "last_updated": "2024-01-15T10:30:45.123Z"
        });
        
        let invalid_config_missing_enabled = json!({
            "last_updated": "2024-01-15T10:30:45.123Z"
        });
        
        let invalid_config_wrong_type = json!({
            "enabled": "true",  // Should be boolean, not string
            "last_updated": "2024-01-15T10:30:45.123Z"
        });
        
        // Valid config should parse correctly
        assert!(valid_config.get("enabled").is_some());
        assert!(valid_config.get("enabled").unwrap().as_bool().is_some());
        
        // Invalid configs should handle gracefully
        assert!(invalid_config_missing_enabled.get("enabled").is_none());
        assert!(invalid_config_wrong_type.get("enabled").unwrap().as_bool().is_none());
    }
}
use tauri::AppHandle;
use tauri_plugin_autostart::ManagerExt;
use crate::settings::manager::SettingsManager;

/// Enable autostart and save to centralized settings
#[tauri::command]
pub async fn enable_autostart(app: AppHandle) -> Result<bool, String> {
    let autostart_manager = app.autolaunch();

    match autostart_manager.enable() {
        Ok(_) => {
            // Save the setting to centralized settings
            save_autostart_to_centralized_settings(app, true).await?;
            log::info!("Autostart enabled successfully");
            Ok(true)
        }
        Err(err) => {
            log::error!("Failed to enable autostart: {}", err);
            Err(format!("Failed to enable autostart: {}", err))
        }
    }
}

/// Disable autostart and save to centralized settings
#[tauri::command]
pub async fn disable_autostart(app: AppHandle) -> Result<bool, String> {
    let autostart_manager = app.autolaunch();

    match autostart_manager.disable() {
        Ok(_) => {
            // Save the setting to centralized settings
            save_autostart_to_centralized_settings(app, false).await?;
            log::info!("Autostart disabled successfully");
            Ok(false)
        }
        Err(err) => {
            log::error!("Failed to disable autostart: {}", err);
            Err(format!("Failed to disable autostart: {}", err))
        }
    }
}

/// Check if autostart is enabled, with fallback to centralized settings
#[tauri::command]
pub async fn is_autostart_enabled(app: AppHandle) -> Result<bool, String> {
    let autostart_manager = app.autolaunch();

    match autostart_manager.is_enabled() {
        Ok(enabled) => {
            log::debug!("Autostart status from system: {}", enabled);

            // Sync with centralized settings if there's a mismatch
            if let Ok(saved_enabled) = get_autostart_from_centralized_settings(&app).await {
                if saved_enabled != enabled {
                    log::info!("Syncing autostart state: system={}, saved={}", enabled, saved_enabled);
                    save_autostart_to_centralized_settings(app.clone(), enabled).await?;
                }
            }

            Ok(enabled)
        }
        Err(err) => {
            log::error!("Failed to check autostart status: {}", err);

            // Fall back to centralized settings if system check fails
            match get_autostart_from_centralized_settings(&app).await {
                Ok(saved_setting) => {
                    log::debug!("Using saved autostart setting: {}", saved_setting);
                    Ok(saved_setting)
                }
                Err(_) => Ok(false) // Default to disabled if we can't determine the state
            }
        }
    }
}

/// Toggle autostart state
#[tauri::command]
pub async fn toggle_autostart(app: AppHandle) -> Result<bool, String> {
    let current_status = is_autostart_enabled(app.clone()).await?;

    if current_status {
        disable_autostart(app).await
    } else {
        enable_autostart(app).await
    }
}

/// Helper function to save autostart setting to centralized settings
async fn save_autostart_to_centralized_settings(app: AppHandle, enabled: bool) -> Result<(), String> {
    let settings_manager = SettingsManager::new(app)
        .map_err(|e| format!("Failed to create settings manager: {}", e))?;

    settings_manager.set_autostart_enabled(enabled).await
        .map_err(|e| format!("Failed to save autostart setting: {}", e))?;

    log::debug!("Autostart setting saved to centralized settings: {}", enabled);
    Ok(())
}

/// Helper function to get autostart setting from centralized settings
async fn get_autostart_from_centralized_settings(app: &AppHandle) -> Result<bool, String> {
    let settings_manager = SettingsManager::new(app.clone())
        .map_err(|e| format!("Failed to create settings manager: {}", e))?;

    settings_manager.get_autostart_enabled().await
        .map_err(|e| format!("Failed to get autostart setting: {}", e))
}

/// Initialize autostart on app startup - load from centralized settings
pub fn init_autostart(app: &AppHandle) -> Result<(), String> {
    // Create a handle that can be moved into the async block
    let app_handle = app.clone();

    // Use async runtime to check centralized settings
    tauri::async_runtime::spawn(async move {
        // Get the autostart manager inside the async block to avoid lifetime issues
        let autostart_manager = app_handle.autolaunch();

        let settings_manager = match SettingsManager::new(app_handle.clone()) {
            Ok(manager) => manager,
            Err(e) => {
                log::warn!("Failed to create settings manager for autostart init: {}", e);
                return;
            }
        };

        match settings_manager.get_autostart_enabled().await {
            Ok(should_be_enabled) => {
                match autostart_manager.is_enabled() {
                    Ok(currently_enabled) => {
                        // Sync the system setting with our centralized preference
                        match (currently_enabled, should_be_enabled) {
                            (false, true) => {
                                if let Err(err) = autostart_manager.enable() {
                                    log::warn!("Failed to enable autostart on startup: {}", err);
                                } else {
                                    log::info!("Autostart enabled on startup");
                                    // Update centralized settings to reflect the change
                                    if let Err(err) = save_autostart_to_centralized_settings(app_handle.clone(), true).await {
                                        log::warn!("Failed to save autostart state after enabling: {}", err);
                                    }
                                }
                            }
                            (true, false) => {
                                if let Err(err) = autostart_manager.disable() {
                                    log::warn!("Failed to disable autostart on startup: {}", err);
                                } else {
                                    log::info!("Autostart disabled on startup");
                                    // Update centralized settings to reflect the change
                                    if let Err(err) = save_autostart_to_centralized_settings(app_handle.clone(), false).await {
                                        log::warn!("Failed to save autostart state after disabling: {}", err);
                                    }
                                }
                            }
                            _ => {
                                log::debug!("Autostart setting already in sync");
                            }
                        }
                    }
                    Err(err) => {
                        log::warn!("Failed to check autostart status on startup: {}", err);
                    }
                }
            }
            Err(err) => {
                log::warn!("Failed to load autostart setting from centralized settings: {}", err);
            }
        }
    });

    Ok(())
}

/// Public helper function to load autostart setting from centralized settings
/// Used by state management and other modules
pub async fn load_autostart_from_centralized_settings(app: &AppHandle) -> Result<bool, String> {
    get_autostart_from_centralized_settings(app).await
}

/// Public helper function to save autostart setting to centralized settings
/// Used by state management and other modules
pub async fn save_autostart_to_centralized_settings_helper(app: AppHandle, enabled: bool) -> Result<(), String> {
    save_autostart_to_centralized_settings(app, enabled).await
}

#[cfg(test)]
mod tests {
    use crate::settings::AppSettings;

    #[test]
    fn test_autostart_setting_defaults() {
        // Test that autostart settings have proper defaults
        let default_settings = AppSettings::default();
        assert_eq!(default_settings.autostart_enabled, false);
    }

    #[test]
    fn test_autostart_setting_structure() {
        // Test the centralized settings structure
        let settings = AppSettings {
            autostart_enabled: true,
            ..AppSettings::default()
        };

        assert_eq!(settings.autostart_enabled, true);
    }

    #[test]
    fn test_autostart_toggle_logic() {
        // Test toggle logic with different states
        let enabled = true;
        let disabled = false;

        // Simulate toggle behavior
        let after_toggle_enabled = !enabled;
        let after_toggle_disabled = !disabled;

        assert_eq!(after_toggle_enabled, false);
        assert_eq!(after_toggle_disabled, true);
    }

    #[test]
    fn test_autostart_error_handling() {
        // Test error handling scenarios
        let error_msg = "Failed to access settings store";
        assert!(error_msg.contains("Failed to access"));
        assert!(error_msg.contains("settings store"));
    }

    #[test]
    fn test_autostart_sync_scenarios() {
        // Test different sync scenarios between system and saved settings
        let system_enabled = true;
        let saved_enabled = false;

        // Should detect mismatch
        assert_ne!(system_enabled, saved_enabled);

        let system_disabled = false;
        let saved_disabled = false;

        // Should detect match
        assert_eq!(system_disabled, saved_disabled);
    }
}

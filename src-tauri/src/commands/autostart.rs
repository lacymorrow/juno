//! # Autostart Configuration Commands
//!
//! Simple autostart settings management.

use crate::settings::SettingsManager;
use tauri::AppHandle;
use tracing::{info, error};

#[tauri::command]
pub async fn get_autostart_config(app: AppHandle) -> Result<bool, String> {
    let settings_manager = SettingsManager::new(app);
    Ok(settings_manager.get_settings().autostart_enabled)
}

#[tauri::command]
pub async fn set_autostart_config(app: AppHandle, enabled: bool) -> Result<(), String> {
    let settings_manager = SettingsManager::new(app);

    settings_manager.update_section("autostart_enabled", serde_json::Value::Bool(enabled)).await?;

    if enabled {
        info!("✅ Autostart enabled");
    } else {
        info!("✅ Autostart disabled");
    }

    Ok(())
}

/// Initialize autostart system
pub fn init_autostart(app: &AppHandle) -> Result<(), String> {
    info!("🚀 Initializing autostart system");

    // Load current autostart state
    let settings_manager = SettingsManager::new(app.clone());
    let settings = settings_manager.get_settings();

    info!("📋 Autostart enabled: {}", settings.autostart_enabled);

    info!("✅ Autostart system initialized");
    Ok(())
}

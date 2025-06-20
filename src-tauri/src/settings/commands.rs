//! # Settings Commands
//!
//! Tauri commands for reactive settings management. These commands provide
//! the bridge between the frontend and the centralized SettingsManager.

use super::manager::SettingsManager;
use super::schema::AppSettings;
use serde_json::Value;
use tauri::{AppHandle, State};
use tracing::{error, info};

/// Initialize the settings manager
#[tauri::command]
pub async fn settings_initialize(app_handle: AppHandle) -> Result<(), String> {
    info!("🔧 Initializing settings system...");

    let manager = SettingsManager::new(app_handle);

    // Initialize and migrate from legacy stores
    manager.initialize().await
        .map_err(|e| format!("Failed to initialize settings: {}", e))?;

    manager.migrate_from_legacy_stores().await
        .map_err(|e| format!("Failed to migrate legacy settings: {}", e))?;

    info!("✅ Settings system initialized successfully");
    Ok(())
}

/// Get all application settings
#[tauri::command]
pub async fn settings_get_all(app_handle: AppHandle) -> Result<AppSettings, String> {
    let manager = SettingsManager::new(app_handle);
    Ok(manager.get_settings())
}

/// Get specific settings section by path
#[tauri::command]
pub async fn settings_get_section(app_handle: AppHandle, path: String) -> Result<Value, String> {
    let manager = SettingsManager::new(app_handle);
    manager.get_section(&path).await
        .map_err(|e| format!("Failed to get settings section '{}': {}", path, e))
}

/// Update settings section
#[tauri::command]
pub async fn settings_update_section(
    app_handle: AppHandle,
    path: String,
    value: Value
) -> Result<(), String> {
    let manager = SettingsManager::new(app_handle);
    manager.update_section(&path, value).await
        .map_err(|e| format!("Failed to update settings section '{}': {}", path, e))
}

/// Update multiple settings sections atomically
#[tauri::command]
pub async fn settings_update_multiple(
    app_handle: AppHandle,
    updates: Vec<(String, Value)>
) -> Result<(), String> {
    let manager = SettingsManager::new(app_handle);
    manager.update_multiple(updates).await
        .map_err(|e| format!("Failed to update multiple settings: {}", e))
}

/// Reset settings section to default
#[tauri::command]
pub async fn settings_reset_section(app_handle: AppHandle, path: String) -> Result<(), String> {
    let manager = SettingsManager::new(app_handle);
    manager.reset_section(&path).await
        .map_err(|e| format!("Failed to reset settings section '{}': {}", path, e))
}

/// Reset all settings to defaults
#[tauri::command]
pub async fn settings_reset_all(app_handle: AppHandle) -> Result<(), String> {
    let manager = SettingsManager::new(app_handle);
    manager.reset_all().await
        .map_err(|e| format!("Failed to reset all settings: {}", e))
}

/// Migrate settings from legacy stores (manual trigger)
#[tauri::command]
pub async fn settings_migrate_legacy(app_handle: AppHandle) -> Result<(), String> {
    let manager = SettingsManager::new(app_handle);
    manager.migrate_from_legacy_stores().await
        .map_err(|e| format!("Failed to migrate legacy settings: {}", e))
}

// Convenience commands for common settings operations

/// Get keyboard shortcuts
#[tauri::command]
pub async fn settings_get_keyboard_shortcuts(app_handle: AppHandle) -> Result<Value, String> {
    settings_get_section(app_handle, "keyboard_shortcuts".to_string()).await
}

/// Update keyboard shortcuts
#[tauri::command]
pub async fn settings_update_keyboard_shortcuts(
    app_handle: AppHandle,
    shortcuts: Value
) -> Result<(), String> {
    settings_update_section(app_handle, "keyboard_shortcuts".to_string(), shortcuts).await
}

/// Get floating bar config
#[tauri::command]
pub async fn settings_get_floating_bar(app_handle: AppHandle) -> Result<Value, String> {
    settings_get_section(app_handle, "floating_bar".to_string()).await
}

/// Update floating bar config
#[tauri::command]
pub async fn settings_update_floating_bar(
    app_handle: AppHandle,
    config: Value
) -> Result<(), String> {
    settings_update_section(app_handle, "floating_bar".to_string(), config).await
}

/// Get agent settings
#[tauri::command]
pub async fn settings_get_agent(app_handle: AppHandle) -> Result<Value, String> {
    settings_get_section(app_handle, "agent".to_string()).await
}

/// Update agent settings
#[tauri::command]
pub async fn settings_update_agent(
    app_handle: AppHandle,
    agent_settings: Value
) -> Result<(), String> {
    settings_update_section(app_handle, "agent".to_string(), agent_settings).await
}

/// Get provider config
#[tauri::command]
pub async fn settings_get_providers(app_handle: AppHandle) -> Result<Value, String> {
    settings_get_section(app_handle, "providers".to_string()).await
}

/// Update provider config
#[tauri::command]
pub async fn settings_update_providers(
    app_handle: AppHandle,
    providers: Value
) -> Result<(), String> {
    settings_update_section(app_handle, "providers".to_string(), providers).await
}

/// Get cloud config
#[tauri::command]
pub async fn settings_get_cloud(app_handle: AppHandle) -> Result<Value, String> {
    settings_get_section(app_handle, "cloud".to_string()).await
}

/// Update cloud config
#[tauri::command]
pub async fn settings_update_cloud(
    app_handle: AppHandle,
    cloud_config: Value
) -> Result<(), String> {
    settings_update_section(app_handle, "cloud".to_string(), cloud_config).await
}

/// Get audio settings
#[tauri::command]
pub async fn settings_get_audio(app_handle: AppHandle) -> Result<Value, String> {
    settings_get_section(app_handle, "audio".to_string()).await
}

/// Update audio settings
#[tauri::command]
pub async fn settings_update_audio(
    app_handle: AppHandle,
    audio_settings: Value
) -> Result<(), String> {
    settings_update_section(app_handle, "audio".to_string(), audio_settings).await
}

/// Get UI settings
#[tauri::command]
pub async fn settings_get_ui(app_handle: AppHandle) -> Result<Value, String> {
    settings_get_section(app_handle, "ui".to_string()).await
}

/// Update UI settings
#[tauri::command]
pub async fn settings_update_ui(
    app_handle: AppHandle,
    ui_settings: Value
) -> Result<(), String> {
    settings_update_section(app_handle, "ui".to_string(), ui_settings).await
}

/// Get onboarding state
#[tauri::command]
pub async fn settings_get_onboarding(app_handle: AppHandle) -> Result<Value, String> {
    settings_get_section(app_handle, "onboarding".to_string()).await
}

/// Update onboarding state
#[tauri::command]
pub async fn settings_update_onboarding(
    app_handle: AppHandle,
    onboarding: Value
) -> Result<(), String> {
    settings_update_section(app_handle, "onboarding".to_string(), onboarding).await
}

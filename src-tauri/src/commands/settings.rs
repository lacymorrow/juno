//! # Centralized Settings Commands
//!
//! Tauri commands for managing all application settings through a single, reactive interface.
//! Replaces scattered settings commands throughout the codebase.

use tauri::{command, AppHandle};
use crate::settings::{
    manager::SettingsManager, AppSettings, KeyboardShortcuts, FloatingBarSettings, AgentSettings,
    ProviderSettings, CloudSettings, AudioSettings, ToolSettings, OnboardingSettings
};

use crate::constants::errors::templates;
use crate::constants::errors::components;
use crate::constants::errors::actions;

// Helper function for error formatting - properly handles template substitution
fn format_error(template: &str, context: &str, error: impl std::fmt::Display) -> String {
    template.replacen("{}", context, 1).replacen("{}", &error.to_string(), 1)
}

/// Get all application settings
#[command]
pub async fn get_all_settings(
    app_handle: AppHandle,
) -> Result<AppSettings, String> {
    let settings_manager = SettingsManager::new(app_handle)
        .map_err(|e| format_error(templates::FAILED_TO_INITIALIZE, components::SETTINGS_MANAGER, e))?;

    settings_manager.get_all_settings().await
        .map_err(|e| format_error(templates::FAILED_TO_RETRIEVE, actions::SETTINGS, e))
}

/// Save all application settings
#[command]
pub async fn save_all_settings(
    app_handle: AppHandle,
    settings: AppSettings,
) -> Result<(), String> {
    let settings_manager = SettingsManager::new(app_handle)
        .map_err(|e| format_error(templates::FAILED_TO_INITIALIZE, components::SETTINGS_MANAGER, e))?;

    settings_manager.save_all_settings(&settings).await
        .map_err(|e| format_error(templates::FAILED_TO_SAVE, actions::SETTINGS, e))
}

// Individual section getters
#[command]
pub async fn get_centralized_keyboard_shortcuts(
    app_handle: AppHandle,
) -> Result<KeyboardShortcuts, String> {
    let settings_manager = SettingsManager::new(app_handle)
        .map_err(|e| format_error(templates::FAILED_TO_INITIALIZE, components::SETTINGS_MANAGER, e))?;

    settings_manager.get_keyboard_shortcuts().await
        .map_err(|e| format_error(templates::FAILED_TO_RETRIEVE, actions::KEYBOARD_SHORTCUTS, e))
}

#[command]
pub async fn get_floating_bar_settings(
    app_handle: AppHandle,
) -> Result<FloatingBarSettings, String> {
    let settings_manager = SettingsManager::new(app_handle)
        .map_err(|e| format_error(templates::FAILED_TO_INITIALIZE, components::SETTINGS_MANAGER, e))?;

    settings_manager.get_floating_bar_settings().await
        .map_err(|e| format_error(templates::FAILED_TO_RETRIEVE, actions::FLOATING_BAR_SETTINGS, e))
}

#[command]
pub async fn get_agent_settings(
    app_handle: AppHandle,
) -> Result<AgentSettings, String> {
    let settings_manager = SettingsManager::new(app_handle)
        .map_err(|e| format_error(templates::FAILED_TO_INITIALIZE, components::SETTINGS_MANAGER, e))?;

    settings_manager.get_agent_settings().await
        .map_err(|e| format_error(templates::FAILED_TO_RETRIEVE, actions::AGENT_SETTINGS, e))
}

#[command]
pub async fn get_centralized_provider_settings(
    app_handle: AppHandle,
) -> Result<ProviderSettings, String> {
    let settings_manager = SettingsManager::new(app_handle)
        .map_err(|e| format_error(templates::FAILED_TO_INITIALIZE, components::SETTINGS_MANAGER, e))?;

    settings_manager.get_provider_settings().await
        .map_err(|e| format_error(templates::FAILED_TO_RETRIEVE, actions::PROVIDER_SETTINGS, e))
}

#[command]
pub async fn get_cloud_settings(
    app_handle: AppHandle,
) -> Result<CloudSettings, String> {
    let settings_manager = SettingsManager::new(app_handle)
        .map_err(|e| format_error(templates::FAILED_TO_INITIALIZE, components::SETTINGS_MANAGER, e))?;

    settings_manager.get_cloud_settings().await
        .map_err(|e| format_error(templates::FAILED_TO_RETRIEVE, actions::CLOUD_SETTINGS, e))
}

#[command]
pub async fn get_audio_settings(
    app_handle: AppHandle,
) -> Result<AudioSettings, String> {
    let settings_manager = SettingsManager::new(app_handle)
        .map_err(|e| format_error(templates::FAILED_TO_INITIALIZE, components::SETTINGS_MANAGER, e))?;

    settings_manager.get_audio_settings().await
        .map_err(|e| format_error(templates::FAILED_TO_RETRIEVE, actions::AUDIO_SETTINGS, e))
}

#[command]
pub async fn get_tool_settings(
    app_handle: AppHandle,
) -> Result<ToolSettings, String> {
    let settings_manager = SettingsManager::new(app_handle)
        .map_err(|e| format_error(templates::FAILED_TO_INITIALIZE, components::SETTINGS_MANAGER, e))?;

    settings_manager.get_tool_settings().await
        .map_err(|e| format_error(templates::FAILED_TO_RETRIEVE, actions::TOOL_SETTINGS, e))
}

#[command]
pub async fn get_onboarding_settings(
    app_handle: AppHandle,
) -> Result<OnboardingSettings, String> {
    let settings_manager = SettingsManager::new(app_handle)
        .map_err(|e| format_error(templates::FAILED_TO_INITIALIZE, components::SETTINGS_MANAGER, e))?;

    settings_manager.get_onboarding_settings().await
        .map_err(|e| format_error(templates::FAILED_TO_RETRIEVE, actions::ONBOARDING_SETTINGS, e))
}

// Individual section setters
#[command]
pub async fn set_centralized_keyboard_shortcuts(
    app_handle: AppHandle,
    shortcuts: KeyboardShortcuts,
) -> Result<(), String> {
    let settings_manager = SettingsManager::new(app_handle)
        .map_err(|e| format_error(templates::FAILED_TO_INITIALIZE, components::SETTINGS_MANAGER, e))?;

    settings_manager.set_keyboard_shortcuts(&shortcuts).await
        .map_err(|e| format_error(templates::FAILED_TO_SET, actions::KEYBOARD_SHORTCUTS, e))
}

#[command]
pub async fn set_floating_bar_settings(
    app_handle: AppHandle,
    settings: FloatingBarSettings,
) -> Result<(), String> {
    let settings_manager = SettingsManager::new(app_handle)
        .map_err(|e| format_error(templates::FAILED_TO_INITIALIZE, components::SETTINGS_MANAGER, e))?;

    settings_manager.set_floating_bar_settings(&settings).await
        .map_err(|e| format_error(templates::FAILED_TO_SET, actions::FLOATING_BAR_SETTINGS, e))
}

#[command]
pub async fn set_agent_settings(
    app_handle: AppHandle,
    settings: AgentSettings,
) -> Result<(), String> {
    let settings_manager = SettingsManager::new(app_handle)
        .map_err(|e| format_error(templates::FAILED_TO_INITIALIZE, components::SETTINGS_MANAGER, e))?;

    settings_manager.set_agent_settings(&settings).await
        .map_err(|e| format_error(templates::FAILED_TO_SET, actions::AGENT_SETTINGS, e))
}

#[command]
pub async fn set_centralized_provider_settings(
    app_handle: AppHandle,
    settings: ProviderSettings,
) -> Result<(), String> {
    let settings_manager = SettingsManager::new(app_handle)
        .map_err(|e| format_error(templates::FAILED_TO_INITIALIZE, components::SETTINGS_MANAGER, e))?;

    settings_manager.set_provider_settings(&settings).await
        .map_err(|e| format_error(templates::FAILED_TO_SET, actions::PROVIDER_SETTINGS, e))
}

#[command]
pub async fn set_cloud_settings(
    app_handle: AppHandle,
    settings: CloudSettings,
) -> Result<(), String> {
    let settings_manager = SettingsManager::new(app_handle)
        .map_err(|e| format_error(templates::FAILED_TO_INITIALIZE, components::SETTINGS_MANAGER, e))?;

    settings_manager.set_cloud_settings(&settings).await
        .map_err(|e| format_error(templates::FAILED_TO_SET, actions::CLOUD_SETTINGS, e))
}

#[command]
pub async fn set_audio_settings(
    app_handle: AppHandle,
    settings: AudioSettings,
) -> Result<(), String> {
    let settings_manager = SettingsManager::new(app_handle)
        .map_err(|e| format_error(templates::FAILED_TO_INITIALIZE, components::SETTINGS_MANAGER, e))?;

    settings_manager.set_audio_settings(&settings).await
        .map_err(|e| format_error(templates::FAILED_TO_SET, actions::AUDIO_SETTINGS, e))
}

#[command]
pub async fn set_tool_settings(
    app_handle: AppHandle,
    settings: ToolSettings,
) -> Result<(), String> {
    let settings_manager = SettingsManager::new(app_handle)
        .map_err(|e| format_error(templates::FAILED_TO_INITIALIZE, components::SETTINGS_MANAGER, e))?;

    settings_manager.set_tool_settings(&settings).await
        .map_err(|e| format_error(templates::FAILED_TO_SET, actions::TOOL_SETTINGS, e))
}

#[command]
pub async fn set_onboarding_settings(
    app_handle: AppHandle,
    settings: OnboardingSettings,
) -> Result<(), String> {
    let settings_manager = SettingsManager::new(app_handle)
        .map_err(|e| format_error(templates::FAILED_TO_INITIALIZE, components::SETTINGS_MANAGER, e))?;

    settings_manager.set_onboarding_settings(&settings).await
        .map_err(|e| format_error(templates::FAILED_TO_SET, actions::ONBOARDING_SETTINGS, e))
}

#[command]
pub async fn set_autostart_enabled(
    app_handle: AppHandle,
    enabled: bool,
) -> Result<(), String> {
    let settings_manager = SettingsManager::new(app_handle)
        .map_err(|e| format_error(templates::FAILED_TO_INITIALIZE, components::SETTINGS_MANAGER, e))?;

    settings_manager.set_autostart_enabled(enabled).await
        .map_err(|e| format_error(templates::FAILED_TO_SET, actions::AUTOSTART_SETTING, e))
}

/// Reset all settings to defaults
#[command]
pub async fn reset_centralized_settings(
    app_handle: AppHandle,
) -> Result<(), String> {
    let settings_manager = SettingsManager::new(app_handle)
        .map_err(|e| format_error(templates::FAILED_TO_INITIALIZE, components::SETTINGS_MANAGER, e))?;

    // Reset to defaults
    let default_settings = AppSettings::default();
    settings_manager.save_all_settings(&default_settings).await
        .map_err(|e| format_error(templates::FAILED_TO_RESTORE, actions::SETTINGS, e))?;

    Ok(())
}

/// Export all settings as JSON string
#[command]
pub async fn export_settings(
    app_handle: AppHandle,
) -> Result<String, String> {
    let settings_manager = SettingsManager::new(app_handle)
        .map_err(|e| format_error(templates::FAILED_TO_INITIALIZE, components::SETTINGS_MANAGER, e))?;

    let settings = settings_manager.get_all_settings().await
        .map_err(|e| format_error(templates::FAILED_TO_RETRIEVE, actions::SETTINGS, e))?;

    serde_json::to_string_pretty(&settings)
        .map_err(|e| format_error(templates::FAILED_TO_ENCODE, actions::SETTINGS_JSON, e))
}

/// Import settings from JSON string
#[command]
pub async fn import_settings(
    app_handle: AppHandle,
    settings_json: String,
) -> Result<(), String> {
    let settings_manager = SettingsManager::new(app_handle)
        .map_err(|e| format_error(templates::FAILED_TO_INITIALIZE, components::SETTINGS_MANAGER, e))?;

    let settings: AppSettings = serde_json::from_str(&settings_json)
        .map_err(|e| format_error(templates::FAILED_TO_PARSE, actions::SETTINGS_JSON, e))?;

    settings_manager.save_all_settings(&settings).await
        .map_err(|e| format_error(templates::FAILED_TO_SAVE, actions::SETTINGS, e))?;

    Ok(())
}

/// Get command overlay visibility setting
#[command]
pub async fn get_command_overlay_enabled(
    app_handle: AppHandle,
) -> Result<bool, String> {
    let settings_manager = SettingsManager::new(app_handle)
        .map_err(|e| format_error(templates::FAILED_TO_INITIALIZE, components::SETTINGS_MANAGER, e))?;

    // For now, store as a simple boolean value in the settings store
    // This can be extended later to use a more comprehensive visualization settings structure
    let store = app_handle.store("settings.json")
        .map_err(|e| format!("Failed to access settings store: {}", e))?;

    Ok(store.get("command_overlay_enabled")
        .and_then(|v| v.as_bool())
        .unwrap_or(true)) // Default to enabled
}

/// Set command overlay visibility setting
#[command]
pub async fn set_command_overlay_enabled(
    app_handle: AppHandle,
    enabled: bool,
) -> Result<(), String> {
    let settings_manager = SettingsManager::new(app_handle)
        .map_err(|e| format_error(templates::FAILED_TO_INITIALIZE, components::SETTINGS_MANAGER, e))?;

    // Store the setting in the settings store
    let store = app_handle.store("settings.json")
        .map_err(|e| format!("Failed to access settings store: {}", e))?;

    store.set("command_overlay_enabled", serde_json::Value::Bool(enabled));
    store.save().map_err(|e| format!("Failed to save settings: {}", e))?;

    Ok(())
}

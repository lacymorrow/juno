//! # Centralized Settings Commands
//!
//! Tauri commands for managing all application settings through a single, reactive interface.
//! Replaces scattered settings commands throughout the codebase.

use tauri::{command, AppHandle, State};
use crate::settings::{
    manager::SettingsManager, AppSettings, KeyboardShortcuts, FloatingBarSettings, AgentSettings,
    ProviderSettings, CloudSettings, AudioSettings, ToolSettings, OnboardingSettings,
    CLISettings, VoiceTranscriptionSettings
};
use tracing::info;
use crate::constants::errors::templates;
use crate::constants::errors::components;

/// Get all application settings
#[command]
pub async fn get_all_settings(
    app_handle: AppHandle,
) -> Result<AppSettings, String> {
    let settings_manager = SettingsManager::new(&app_handle)
        .map_err(|e| format!(templates::FAILED_TO_INITIALIZE, components::SETTINGS_MANAGER, e))?;

    settings_manager.get_all_settings().await
        .map_err(|e| format!("Failed to get settings: {}", e))
}

/// Save all application settings
#[command]
pub async fn save_all_settings(
    app_handle: AppHandle,
    settings: AppSettings,
) -> Result<(), String> {
    let settings_manager = SettingsManager::new(&app_handle)
        .map_err(|e| format!(templates::FAILED_TO_INITIALIZE, components::SETTINGS_MANAGER, e))?;

    settings_manager.save_all_settings(&settings).await
        .map_err(|e| format!("Failed to save settings: {}", e))
}

// Individual section getters
#[command]
pub async fn get_centralized_keyboard_shortcuts(
    app_handle: AppHandle,
) -> Result<KeyboardShortcuts, String> {
    let settings_manager = SettingsManager::new(&app_handle)
        .map_err(|e| format!(templates::FAILED_TO_INITIALIZE, components::SETTINGS_MANAGER, e))?;

    settings_manager.get_keyboard_shortcuts().await
        .map_err(|e| format!("Failed to get keyboard shortcuts: {}", e))
}

#[command]
pub async fn get_floating_bar_settings(
    app_handle: AppHandle,
) -> Result<FloatingBarSettings, String> {
    let settings_manager = SettingsManager::new(&app_handle)
        .map_err(|e| format!(templates::FAILED_TO_INITIALIZE, components::SETTINGS_MANAGER, e))?;

    settings_manager.get_floating_bar_settings().await
        .map_err(|e| format!("Failed to get floating bar settings: {}", e))
}

#[command]
pub async fn get_agent_settings(
    app_handle: AppHandle,
) -> Result<AgentSettings, String> {
    let settings_manager = SettingsManager::new(&app_handle)
        .map_err(|e| format!(templates::FAILED_TO_INITIALIZE, components::SETTINGS_MANAGER, e))?;

    settings_manager.get_agent_settings().await
        .map_err(|e| format!("Failed to get agent settings: {}", e))
}

#[command]
pub async fn get_centralized_provider_settings(
    app_handle: AppHandle,
) -> Result<ProviderSettings, String> {
    let settings_manager = SettingsManager::new(&app_handle)
        .map_err(|e| format!(templates::FAILED_TO_INITIALIZE, components::SETTINGS_MANAGER, e))?;

    settings_manager.get_provider_settings().await
        .map_err(|e| format!("Failed to get provider settings: {}", e))
}

#[command]
pub async fn get_cloud_settings(
    app_handle: AppHandle,
) -> Result<CloudSettings, String> {
    let settings_manager = SettingsManager::new(&app_handle)
        .map_err(|e| format!(templates::FAILED_TO_INITIALIZE, components::SETTINGS_MANAGER, e))?;

    settings_manager.get_cloud_settings().await
        .map_err(|e| format!("Failed to get cloud settings: {}", e))
}

#[command]
pub async fn get_audio_settings(
    app_handle: AppHandle,
) -> Result<AudioSettings, String> {
    let settings_manager = SettingsManager::new(&app_handle)
        .map_err(|e| format!(templates::FAILED_TO_INITIALIZE, components::SETTINGS_MANAGER, e))?;

    settings_manager.get_audio_settings().await
        .map_err(|e| format!("Failed to get audio settings: {}", e))
}

#[command]
pub async fn get_tool_settings(
    app_handle: AppHandle,
) -> Result<ToolSettings, String> {
    let settings_manager = SettingsManager::new(&app_handle)
        .map_err(|e| format!(templates::FAILED_TO_INITIALIZE, components::SETTINGS_MANAGER, e))?;

    settings_manager.get_tool_settings().await
        .map_err(|e| format!("Failed to get tool settings: {}", e))
}

#[command]
pub async fn get_onboarding_settings(
    app_handle: AppHandle,
) -> Result<OnboardingSettings, String> {
    let settings_manager = SettingsManager::new(&app_handle)
        .map_err(|e| format!(templates::FAILED_TO_INITIALIZE, components::SETTINGS_MANAGER, e))?;

    settings_manager.get_onboarding_settings().await
        .map_err(|e| format!("Failed to get onboarding settings: {}", e))
}

// Individual section setters
#[command]
pub async fn set_centralized_keyboard_shortcuts(
    app_handle: AppHandle,
    shortcuts: KeyboardShortcuts,
) -> Result<(), String> {
    let settings_manager = SettingsManager::new(&app_handle)
        .map_err(|e| format!(templates::FAILED_TO_INITIALIZE, components::SETTINGS_MANAGER, e))?;

    settings_manager.set_keyboard_shortcuts(&shortcuts).await
        .map_err(|e| format!("Failed to set keyboard shortcuts: {}", e))
}

#[command]
pub async fn set_floating_bar_settings(
    app_handle: AppHandle,
    settings: FloatingBarSettings,
) -> Result<(), String> {
    let settings_manager = SettingsManager::new(&app_handle)
        .map_err(|e| format!(templates::FAILED_TO_INITIALIZE, components::SETTINGS_MANAGER, e))?;

    settings_manager.set_floating_bar_settings(&settings).await
        .map_err(|e| format!("Failed to set floating bar settings: {}", e))
}

#[command]
pub async fn set_agent_settings(
    app_handle: AppHandle,
    settings: AgentSettings,
) -> Result<(), String> {
    let settings_manager = SettingsManager::new(&app_handle)
        .map_err(|e| format!(templates::FAILED_TO_INITIALIZE, components::SETTINGS_MANAGER, e))?;

    settings_manager.set_agent_settings(&settings).await
        .map_err(|e| format!("Failed to set agent settings: {}", e))
}

#[command]
pub async fn set_centralized_provider_settings(
    app_handle: AppHandle,
    settings: ProviderSettings,
) -> Result<(), String> {
    let settings_manager = SettingsManager::new(&app_handle)
        .map_err(|e| format!(templates::FAILED_TO_INITIALIZE, components::SETTINGS_MANAGER, e))?;

    settings_manager.set_provider_settings(&settings).await
        .map_err(|e| format!("Failed to set provider settings: {}", e))
}

#[command]
pub async fn set_cloud_settings(
    app_handle: AppHandle,
    settings: CloudSettings,
) -> Result<(), String> {
    let settings_manager = SettingsManager::new(&app_handle)
        .map_err(|e| format!(templates::FAILED_TO_INITIALIZE, components::SETTINGS_MANAGER, e))?;

    settings_manager.set_cloud_settings(&settings).await
        .map_err(|e| format!("Failed to set cloud settings: {}", e))
}

#[command]
pub async fn set_audio_settings(
    app_handle: AppHandle,
    settings: AudioSettings,
) -> Result<(), String> {
    let settings_manager = SettingsManager::new(&app_handle)
        .map_err(|e| format!(templates::FAILED_TO_INITIALIZE, components::SETTINGS_MANAGER, e))?;

    settings_manager.set_audio_settings(&settings).await
        .map_err(|e| format!("Failed to set audio settings: {}", e))
}

#[command]
pub async fn set_tool_settings(
    app_handle: AppHandle,
    settings: ToolSettings,
) -> Result<(), String> {
    let settings_manager = SettingsManager::new(&app_handle)
        .map_err(|e| format!(templates::FAILED_TO_INITIALIZE, components::SETTINGS_MANAGER, e))?;

    settings_manager.set_tool_settings(&settings).await
        .map_err(|e| format!("Failed to set tool settings: {}", e))
}

#[command]
pub async fn set_onboarding_settings(
    app_handle: AppHandle,
    settings: OnboardingSettings,
) -> Result<(), String> {
    let settings_manager = SettingsManager::new(&app_handle)
        .map_err(|e| format!(templates::FAILED_TO_INITIALIZE, components::SETTINGS_MANAGER, e))?;

    settings_manager.set_onboarding_settings(&settings).await
        .map_err(|e| format!("Failed to set onboarding settings: {}", e))
}

#[command]
pub async fn set_autostart_enabled(
    app_handle: AppHandle,
    enabled: bool,
) -> Result<(), String> {
    let settings_manager = SettingsManager::new(&app_handle)
        .map_err(|e| format!(templates::FAILED_TO_INITIALIZE, components::SETTINGS_MANAGER, e))?;

    settings_manager.set_autostart_enabled(enabled).await
        .map_err(|e| format!("Failed to set autostart setting: {}", e))
}

/// Reset all settings to defaults
#[command]
pub async fn reset_centralized_settings(
    app_handle: AppHandle,
) -> Result<(), String> {
    let settings_manager = SettingsManager::new(&app_handle)
        .map_err(|e| format!(templates::FAILED_TO_INITIALIZE, components::SETTINGS_MANAGER, e))?;

    let defaults = AppSettings::default();
    settings_manager.save_all_settings(&defaults).await
        .map_err(|e| format!("Failed to reset settings: {}", e))
}

/// Export all settings as JSON string
#[command]
pub async fn export_settings(
    app_handle: AppHandle,
) -> Result<String, String> {
    let settings_manager = SettingsManager::new(&app_handle)
        .map_err(|e| format!(templates::FAILED_TO_INITIALIZE, components::SETTINGS_MANAGER, e))?;

    let settings = settings_manager.get_all_settings().await
        .map_err(|e| format!("Failed to get settings: {}", e))?;

    serde_json::to_string_pretty(&settings)
        .map_err(|e| format!("Failed to serialize settings: {}", e))
}

/// Import settings from JSON string
#[command]
pub async fn import_settings(
    app_handle: AppHandle,
    settings_json: String,
) -> Result<(), String> {
    let settings: AppSettings = serde_json::from_str(&settings_json)
        .map_err(|e| format!("Failed to parse settings JSON: {}", e))?;

    let settings_manager = SettingsManager::new(&app_handle)
        .map_err(|e| format!(templates::FAILED_TO_INITIALIZE, components::SETTINGS_MANAGER, e))?;

    settings_manager.save_all_settings(&settings).await
        .map_err(|e| format!("Failed to import settings: {}", e))
}

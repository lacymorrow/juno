use tauri::{State, AppHandle};
use crate::state::AppState;
use crate::settings::manager::SettingsManager;
use tracing::info;

// Command to set Dictation Mode clipboard saving
#[tauri::command]
pub async fn set_dictation_clipboard_enabled(
    app_handle: AppHandle,
    enabled: bool,
    state: State<'_, AppState>,
) -> Result<(), String> {
    let settings_manager = SettingsManager::new(app_handle)
        .map_err(|e| format!("Failed to initialize settings manager: {}", e))?;

    let mut audio_settings = settings_manager.get_audio_settings().await
        .map_err(|e| format!("Failed to load audio settings: {}", e))?;

    audio_settings.dictation_clipboard_enabled = enabled;

    settings_manager.set_audio_settings(&audio_settings).await
        .map_err(|e| format!("Failed to save audio settings: {}", e))?;

    // Update state for backward compatibility
    state.set_dictation_clipboard_enabled(enabled)
        .map_err(|e| format!("Failed to set dictation_clipboard_enabled: {}", e))?;

    info!("Dictation Mode clipboard saving set to: {}", enabled);
    Ok(())
}

// Command to get current Dictation Mode clipboard setting
#[tauri::command]
pub async fn get_dictation_clipboard_enabled(
    app_handle: AppHandle,
    state: State<'_, AppState>,
) -> Result<bool, String> {
    let settings_manager = SettingsManager::new(app_handle)
        .map_err(|e| format!("Failed to initialize settings manager: {}", e))?;

    let audio_settings = settings_manager.get_audio_settings().await
        .map_err(|e| format!("Failed to load audio settings: {}", e))?;

    // Sync with state for backward compatibility
    state.set_dictation_clipboard_enabled(audio_settings.dictation_clipboard_enabled)
        .map_err(|e| format!("Failed to set dictation_clipboard_enabled: {}", e))?;

    tracing::debug!("Current Dictation Mode clipboard setting: {}", audio_settings.dictation_clipboard_enabled);
    Ok(audio_settings.dictation_clipboard_enabled)
}

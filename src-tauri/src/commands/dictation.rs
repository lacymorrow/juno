use tauri::State;
use crate::state::AppState;
use tracing::info;

// Command to set spacebar dictation clipboard saving
#[tauri::command]
pub async fn set_spacebar_clipboard_enabled(
    enabled: bool,
    state: State<'_, AppState>,
) -> Result<(), String> {
    let mut clipboard_enabled = state.spacebar_clipboard_enabled.lock()
        .map_err(|e| format!("Failed to lock spacebar_clipboard_enabled: {}", e))?;
    *clipboard_enabled = enabled;
    info!("Spacebar dictation clipboard saving set to: {}", enabled);
    Ok(())
}

// Command to get current spacebar dictation clipboard setting
#[tauri::command]
pub async fn get_spacebar_clipboard_enabled(
    state: State<'_, AppState>,
) -> Result<bool, String> {
    let enabled = state.spacebar_clipboard_enabled.lock()
        .map_err(|e| format!("Failed to lock spacebar_clipboard_enabled: {}", e))?
        .clone();
    info!("Current spacebar dictation clipboard setting: {}", enabled);
    Ok(enabled)
}

use tauri::State;
use crate::state::AppState;
use tracing::info;

// Command to set Dictation Mode clipboard saving
#[tauri::command]
pub async fn set_spacebar_clipboard_enabled(
    enabled: bool,
    state: State<'_, AppState>,
) -> Result<(), String> {
    let mut clipboard_enabled = state.spacebar_clipboard_enabled.lock()
        .map_err(|e| format!("Failed to lock spacebar_clipboard_enabled: {}", e))?;
    *clipboard_enabled = enabled;
    info!("Dictation Mode clipboard saving set to: {}", enabled);
    Ok(())
}

// Command to get current Dictation Mode clipboard setting
#[tauri::command]
pub async fn get_spacebar_clipboard_enabled(
    state: State<'_, AppState>,
) -> Result<bool, String> {
    let enabled = state.spacebar_clipboard_enabled.lock()
        .map_err(|e| format!("Failed to lock spacebar_clipboard_enabled: {}", e))?
        .clone();
    info!("Current Dictation Mode clipboard setting: {}", enabled);
    Ok(enabled)
}

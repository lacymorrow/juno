// Commands related to keyboard actions (typing, pressing keys)

use tauri::State;
use crate::state::AppState;
use tracing::{info, error};

#[tauri::command]
pub(crate) async fn dev_type_text(text: String, state: State<'_, AppState>) -> Result<(), String> {
    info!("Executing dev_type_text for text: '{}'", text);
    let desktop = state.get_desktop()?;
    let result = desktop.type_text(&text);
    match result {
        Ok(_) => {
            info!("Successfully typed text: '{}'", text);
            Ok(())
        }
        Err(e) => {
            let error_msg = format!("Failed to type text '{}': {}", text, e);
            error!("{}", error_msg);
            Err(error_msg)
        }
    }
}

#[tauri::command]
pub(crate) async fn dev_press_key(key: String, modifier: Option<String>, state: State<'_, AppState>) -> Result<(), String> {
    info!("Executing dev_press_key for key: '{}' with modifier: {:?}", key, modifier);
    let desktop = state.get_desktop()?;
    match desktop.press_key(&key, modifier.as_deref()) {
        Ok(_) => {
            info!("Successfully pressed key: '{}' with modifier: {:?}", key, modifier);
            Ok(())
        }
        Err(e) => {
            let error_msg = format!("Failed to press key '{}' with modifier '{:?}': {}", key, modifier, e);
            error!("{}", error_msg);
            Err(error_msg)
        }
    }
}

#[tauri::command]
pub(crate) async fn dev_global_type_text(text: String, state: State<'_, AppState>) -> Result<(), String> {
    info!("Executing dev_global_type_text for text: '{}'", text);
    let desktop = state.get_desktop()?;
    desktop.type_text(&text)
        .map_err(|e| format!("Error during global type text: {}", e))
}

#[tauri::command]
pub(crate) async fn dev_hold_key(key: String, duration_ms: Option<u64>, state: State<'_, AppState>) -> Result<(), String> {
    info!("Executing dev_hold_key for key: '{}', duration: {:?} ms", key, duration_ms);
    let desktop = state.get_desktop()?;
    desktop.hold_key(&key, duration_ms)
        .map_err(|e| format!("Error during hold key: {}", e))
}

#[tauri::command]
pub(crate) async fn dev_release_key(key: String, state: State<'_, AppState>) -> Result<(), String> {
    info!("Executing dev_release_key for key: '{}'", key);
    let desktop = state.get_desktop()?;
    desktop.release_key(&key)
        .map_err(|e| format!("Error during release key: {}", e))
}




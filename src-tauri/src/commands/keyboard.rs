// Commands related to keyboard actions (typing, pressing keys)

use tauri::{State, AppHandle, Emitter};
use crate::state::AppState;
use tracing::{info, error};
use serde_json::json;

#[tauri::command]
pub(crate) async fn type_text(text: String, app_handle: AppHandle, state: State<'_, AppState>) -> Result<(), String> {
    info!("Executing type_text for text: '{}'", text);
    let desktop = state.get_desktop()?;
    let result = desktop.type_text(&text);
    match result {
        Ok(_) => {
            info!("Successfully typed text: '{}'", text);
            // Emit key press visualization for typing (show the text being typed)
            if let Err(e) = app_handle.emit("key-press-visualization", json!({
                "key": format!("Type: {}", text.chars().take(20).collect::<String>()),
                "modifier": null
            })) {
                error!("Failed to emit key press visualization for type_text: {}", e);
            }
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
pub(crate) async fn press_key(key: String, modifier: Option<String>, app_handle: AppHandle, state: State<'_, AppState>) -> Result<(), String> {
    info!("Executing press_key for key: '{}' with modifier: {:?}", key, modifier);
    let desktop = state.get_desktop()?;
    match desktop.press_key(&key, modifier.as_deref()) {
        Ok(_) => {
            info!("Successfully pressed key: '{}' with modifier: {:?}", key, modifier);
            // Emit key press visualization event
            if let Err(e) = app_handle.emit("key-press-visualization", json!({
                "key": key,
                "modifier": modifier
            })) {
                error!("Failed to emit key press visualization: {}", e);
            }
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
pub(crate) async fn global_type_text(text: String, app_handle: AppHandle, state: State<'_, AppState>) -> Result<(), String> {
    info!("Executing global_type_text for text: '{}'", text);
    let desktop = state.get_desktop()?;
    let result = desktop.type_text(&text);
    match result {
        Ok(_) => {
            // Emit key press visualization for global typing
            if let Err(e) = app_handle.emit("key-press-visualization", json!({
                "key": format!("Global: {}", text.chars().take(20).collect::<String>()),
                "modifier": null
            })) {
                error!("Failed to emit key press visualization for global_type_text: {}", e);
            }
            Ok(())
        }
        Err(e) => Err(format!("Error during global type text: {}", e))
    }
}

#[tauri::command]
pub(crate) async fn hold_key(key: String, duration_ms: Option<u64>, app_handle: AppHandle, state: State<'_, AppState>) -> Result<(), String> {
    info!("Executing hold_key for key: '{}', duration: {:?} ms", key, duration_ms);
    let desktop = state.get_desktop()?;
    let result = desktop.hold_key(&key, duration_ms);
    match result {
        Ok(_) => {
            // Emit key press visualization for hold key
            if let Err(e) = app_handle.emit("key-press-visualization", json!({
                "key": format!("Hold: {}", key),
                "modifier": duration_ms.map(|d| format!("{}ms", d))
            })) {
                error!("Failed to emit key press visualization for hold_key: {}", e);
            }
            Ok(())
        }
        Err(e) => Err(format!("Error during hold key: {}", e))
    }
}

#[tauri::command]
pub(crate) async fn release_key(key: String, app_handle: AppHandle, state: State<'_, AppState>) -> Result<(), String> {
    info!("Executing release_key for key: '{}'", key);
    let desktop = state.get_desktop()?;
    let result = desktop.release_key(&key);
    match result {
        Ok(_) => {
            // Emit key press visualization for release key
            if let Err(e) = app_handle.emit("key-press-visualization", json!({
                "key": format!("Release: {}", key),
                "modifier": null
            })) {
                error!("Failed to emit key press visualization for release_key: {}", e);
            }
            Ok(())
        }
        Err(e) => Err(format!("Error during release key: {}", e))
    }
}




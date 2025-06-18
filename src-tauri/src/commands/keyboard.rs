// Commands related to keyboard actions (typing, pressing keys)

use tauri::{State, AppHandle, Emitter};
use crate::state::AppState;
use crate::utils::key_parsing;
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

    // Handle the case where key is a combination like "cmd+shift+a"
    let (final_key, final_modifier) = if key.contains('+') {
        // Parse the key combination using our centralized parser
        match key_parsing::split_key_and_modifier(&key) {
            Ok((k, m)) => (k, m.or(modifier)), // Use parsed modifier, or fall back to passed modifier
            Err(e) => {
                let error_msg = format!("Failed to parse key combination '{}': {}", key, e);
                error!("{}", error_msg);
                return Err(error_msg);
            }
        }
    } else {
        // Normal key without combination
        (key, modifier)
    };

    match desktop.press_key(&final_key, final_modifier.as_deref()) {
        Ok(_) => {
            info!("Successfully pressed key: '{}' with modifier: {:?}", final_key, final_modifier);
            // Emit key press visualization event
            if let Err(e) = app_handle.emit("key-press-visualization", json!({
                "key": final_key,
                "modifier": final_modifier
            })) {
                error!("Failed to emit key press visualization: {}", e);
            }
            Ok(())
        }
        Err(e) => {
            let error_msg = format!("Failed to press key '{}' with modifier '{:?}': {}", final_key, final_modifier, e);
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

    // Parse key combinations for hold_key as well
    let parsed_key = if key.contains('+') {
        // For hold_key, we pass the full combination as the key_name
        key.clone()
    } else {
        key.clone()
    };

    let result = desktop.hold_key(&parsed_key, duration_ms);
    match result {
        Ok(_) => {
            // Emit key press visualization for hold key
            if let Err(e) = app_handle.emit("key-press-visualization", json!({
                "key": format!("Hold: {}", parsed_key),
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

    // Parse key combinations for release_key as well
    let parsed_key = if key.contains('+') {
        // For release_key, we pass the full combination as the key_name
        key.clone()
    } else {
        key.clone()
    };

    let result = desktop.release_key(&parsed_key);
    match result {
        Ok(_) => {
            // Emit key press visualization for release key
            if let Err(e) = app_handle.emit("key-press-visualization", json!({
                "key": format!("Release: {}", parsed_key),
                "modifier": null
            })) {
                error!("Failed to emit key press visualization for release_key: {}", e);
            }
            Ok(())
        }
        Err(e) => Err(format!("Error during release key: {}", e))
    }
}




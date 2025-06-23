// Commands related to keyboard actions (typing, pressing keys)

use tauri::{State, AppHandle, Emitter};
use crate::state::AppState;
use crate::utils::key_parsing;
use tracing::{info, error};
use serde_json::json;

/// Type text with optional debug features
#[tauri::command]
pub(crate) async fn type_text(
    text: String,
    app_handle: AppHandle,
    state: State<'_, AppState>,
    debug_mode: Option<bool>
) -> Result<(), String> {
    use crate::commands::debug_utils::{
        should_enable_debug, log_operation_start, validate_input_with_debug,
        validate_text_input, DebugTimer, send_debug_notification
    };

    let debug = should_enable_debug(debug_mode, &state);

    if debug {
        log_operation_start("type_text", &format!("text: '{}' (length: {})",
            text.chars().take(50).collect::<String>(), text.len()));

        // Validate input in debug mode
        validate_input_with_debug(&text, validate_text_input, "type_text")?;
    }

    let timer = if debug { Some(DebugTimer::start("type_text")) } else { None };

    let desktop = state.get_desktop()?;
    let result = desktop.type_text(&text);

    match result {
        Ok(_) => {
            if debug {
                info!("Successfully typed text: '{}'", text);
                let _ = send_debug_notification(&app_handle, "Keyboard",
                    &format!("Typed text ({} chars)", text.len()));
            }

            // Emit key press visualization for typing (show the text being typed)
            if let Err(e) = app_handle.emit("key-press-visualization", json!({
                "key": format!("Type: {}", text.chars().take(20).collect::<String>()),
                "modifier": null
            })) {
                error!("Failed to emit key press visualization for type_text: {}", e);
            }

            if let Some(timer) = timer {
                timer.finish(true);
            }

            Ok(())
        }
        Err(e) => {
            let error_msg = format!("Failed to type text '{}': {}", text, e);
            if debug {
                error!("{}", error_msg);
            }

            if let Some(timer) = timer {
                timer.finish(false);
            }

            Err(error_msg)
        }
    }
}

/// Press a key with optional debug features
#[tauri::command]
pub(crate) async fn press_key(
    key: String,
    modifier: Option<String>,
    app_handle: AppHandle,
    state: State<'_, AppState>,
    debug_mode: Option<bool>
) -> Result<(), String> {
    use crate::commands::debug_utils::{
        should_enable_debug, log_operation_start, validate_input_with_debug,
        validate_key_input, DebugTimer, send_debug_notification
    };

    let debug = should_enable_debug(debug_mode, &state);

    if debug {
        log_operation_start("press_key", &format!("key: '{}', modifier: {:?}", key, modifier));

        // Validate key input in debug mode
        validate_input_with_debug(&key, validate_key_input, "press_key")?;
    } else {
        info!("Executing press_key for key: '{}' with modifier: {:?}", key, modifier);
    }

    let timer = if debug { Some(DebugTimer::start("press_key")) } else { None };
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

    let result = desktop.press_key(&final_key, final_modifier.as_deref());

    match result {
        Ok(_) => {
            if debug {
                info!("Successfully pressed key: '{}' with modifier: {:?}", final_key, final_modifier);
                let _ = send_debug_notification(&app_handle, "Keyboard",
                    &format!("Pressed key: {} {:?}", final_key, final_modifier));
            }

            // Emit key press visualization event
            if let Err(e) = app_handle.emit("key-press-visualization", json!({
                "key": final_key,
                "modifier": final_modifier
            })) {
                error!("Failed to emit key press visualization: {}", e);
            }

            if let Some(timer) = timer {
                timer.finish(true);
            }

            Ok(())
        }
        Err(e) => {
            let error_msg = format!("Failed to press key '{}' with modifier '{:?}': {}", final_key, final_modifier, e);
            if debug {
                error!("{}", error_msg);
            }

            if let Some(timer) = timer {
                timer.finish(false);
            }

            Err(error_msg)
        }
    }
}

/// Type text globally with optional debug features
#[tauri::command]
pub(crate) async fn global_type_text(
    text: String,
    app_handle: AppHandle,
    state: State<'_, AppState>,
    debug_mode: Option<bool>
) -> Result<(), String> {
    use crate::commands::debug_utils::{
        should_enable_debug, log_operation_start, validate_input_with_debug,
        validate_text_input, DebugTimer, send_debug_notification
    };

    let debug = should_enable_debug(debug_mode, &state);

    if debug {
        log_operation_start("global_type_text", &format!("text: '{}' (length: {})",
            text.chars().take(50).collect::<String>(), text.len()));

        // Validate input in debug mode
        validate_input_with_debug(&text, validate_text_input, "global_type_text")?;
    } else {
        info!("Executing global_type_text for text: '{}'", text);
    }

    let timer = if debug { Some(DebugTimer::start("global_type_text")) } else { None };

    let desktop = state.get_desktop()?;
    let result = desktop.type_text(&text);

    match result {
        Ok(_) => {
            if debug {
                let _ = send_debug_notification(&app_handle, "Keyboard",
                    &format!("Global typed text ({} chars)", text.len()));
            }

            // Emit key press visualization for global typing
            if let Err(e) = app_handle.emit("key-press-visualization", json!({
                "key": format!("Global: {}", text.chars().take(20).collect::<String>()),
                "modifier": null
            })) {
                error!("Failed to emit key press visualization for global_type_text: {}", e);
            }

            if let Some(timer) = timer {
                timer.finish(true);
            }

            Ok(())
        }
        Err(e) => {
            if let Some(timer) = timer {
                timer.finish(false);
            }

            Err(format!("Error during global type text: {}", e))
        }
    }
}

/// Hold a key with optional debug features
#[tauri::command]
pub(crate) async fn hold_key(
    key: String,
    duration_ms: Option<u64>,
    app_handle: AppHandle,
    state: State<'_, AppState>,
    debug_mode: Option<bool>
) -> Result<(), String> {
    use crate::commands::debug_utils::{
        should_enable_debug, log_operation_start, validate_input_with_debug,
        validate_key_input, validate_duration, DebugTimer, send_debug_notification
    };

    let debug = should_enable_debug(debug_mode, &state);

    if debug {
        log_operation_start("hold_key", &format!("key: '{}', duration: {:?} ms", key, duration_ms));

        // Validate inputs in debug mode
        validate_input_with_debug(&key, validate_key_input, "hold_key")?;
        if let Some(error) = validate_duration(duration_ms) {
            tracing::warn!("[DEBUG] Duration warning for hold_key: {}", error);
        }
    } else {
        info!("Executing hold_key for key: '{}', duration: {:?} ms", key, duration_ms);
    }

    let timer = if debug { Some(DebugTimer::start("hold_key")) } else { None };
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
            if debug {
                let _ = send_debug_notification(&app_handle, "Keyboard",
                    &format!("Held key: {} for {:?}ms", parsed_key, duration_ms));
            }

            // Emit key press visualization for hold key
            if let Err(e) = app_handle.emit("key-press-visualization", json!({
                "key": format!("Hold: {}", parsed_key),
                "modifier": duration_ms.map(|d| format!("{}ms", d))
            })) {
                error!("Failed to emit key press visualization for hold_key: {}", e);
            }

            if let Some(timer) = timer {
                timer.finish(true);
            }

            Ok(())
        }
        Err(e) => {
            if let Some(timer) = timer {
                timer.finish(false);
            }

            Err(format!("Error during hold key: {}", e))
        }
    }
}

/// Release a key with optional debug features
#[tauri::command]
pub(crate) async fn release_key(
    key: String,
    app_handle: AppHandle,
    state: State<'_, AppState>,
    debug_mode: Option<bool>
) -> Result<(), String> {
    use crate::commands::debug_utils::{
        should_enable_debug, log_operation_start, validate_input_with_debug,
        validate_key_input, DebugTimer, send_debug_notification
    };

    let debug = should_enable_debug(debug_mode, &state);

    if debug {
        log_operation_start("release_key", &format!("key: '{}'", key));

        // Validate input in debug mode
        validate_input_with_debug(&key, validate_key_input, "release_key")?;
    } else {
        info!("Executing release_key for key: '{}'", key);
    }

    let timer = if debug { Some(DebugTimer::start("release_key")) } else { None };
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
            if debug {
                let _ = send_debug_notification(&app_handle, "Keyboard",
                    &format!("Released key: {}", parsed_key));
            }

            // Emit key press visualization for release key
            if let Err(e) = app_handle.emit("key-press-visualization", json!({
                "key": format!("Release: {}", parsed_key),
                "modifier": null
            })) {
                error!("Failed to emit key press visualization for release_key: {}", e);
            }

            if let Some(timer) = timer {
                timer.finish(true);
            }

            Ok(())
        }
        Err(e) => {
            if let Some(timer) = timer {
                timer.finish(false);
            }

            Err(format!("Error during release key: {}", e))
        }
    }
}

// DEPRECATED: Backward compatibility functions for dev_ commands
// These will be removed in Phase 5 of the refactoring

/// DEPRECATED: Use type_text with debug_mode parameter instead
#[tauri::command]
pub(crate) async fn dev_type_text(text: String, app_handle: AppHandle, state: State<'_, AppState>) -> Result<(), String> {
    type_text(text, app_handle, state, Some(true)).await
}

/// DEPRECATED: Use press_key with debug_mode parameter instead
#[tauri::command]
pub(crate) async fn dev_press_key(key: String, modifier: Option<String>, app_handle: AppHandle, state: State<'_, AppState>) -> Result<(), String> {
    press_key(key, modifier, app_handle, state, Some(true)).await
}

/// DEPRECATED: Use global_type_text with debug_mode parameter instead
#[tauri::command]
pub(crate) async fn dev_global_type_text(text: String, app_handle: AppHandle, state: State<'_, AppState>) -> Result<(), String> {
    global_type_text(text, app_handle, state, Some(true)).await
}

/// DEPRECATED: Use hold_key with debug_mode parameter instead
#[tauri::command]
pub(crate) async fn dev_hold_key(key: String, duration_ms: Option<u64>, app_handle: AppHandle, state: State<'_, AppState>) -> Result<(), String> {
    hold_key(key, duration_ms, app_handle, state, Some(true)).await
}

/// DEPRECATED: Use release_key with debug_mode parameter instead
#[tauri::command]
pub(crate) async fn dev_release_key(key: String, app_handle: AppHandle, state: State<'_, AppState>) -> Result<(), String> {
    release_key(key, app_handle, state, Some(true)).await
}

use crate::state::AppState;
use tauri::{State, AppHandle, Manager, Emitter};
use std::sync::{Arc, Mutex};
use tracing::{info, error, warn};

/// Start always listening mode
#[tauri::command]
pub async fn start_always_listening_mode(
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<String, String> {
    info!("[Command] start_always_listening_mode called");

    // Update app state
    if let Ok(mut always_listening_active) = state.always_listening_active.lock() {
        if *always_listening_active {
            return Ok("Already listening mode is already active".to_string());
        }
        *always_listening_active = true;
    } else {
        return Err("Failed to lock always listening state".to_string());
    }

    // Call the plugin command
    match app.try_state::<Arc<Mutex<tauri_plugin_voice_transcription::always_listening::AlwaysListeningController>>>() {
        Some(controller_state) => {
            match tauri_plugin_voice_transcription::commands::start_always_listening(
                app.clone(),
                controller_state
            ).await {
                Ok(_) => {
                    info!("[Command] Always listening mode started successfully");
                    
                    // Emit event to UI
                    if let Err(e) = app.emit("always-listening-mode-changed", true) {
                        error!("[Command] Failed to emit always-listening-mode-changed event: {}", e);
                    }
                    
                    Ok("Always listening mode started successfully".to_string())
                }
                Err(e) => {
                    // Reset state on failure
                    if let Ok(mut always_listening_active) = state.always_listening_active.lock() {
                        *always_listening_active = false;
                    }
                    
                    let err_msg = format!("Failed to start always listening mode: {}", e);
                    error!("[Command] {}", err_msg);
                    Err(err_msg)
                }
            }
        }
        None => {
            // Reset state on failure
            if let Ok(mut always_listening_active) = state.always_listening_active.lock() {
                *always_listening_active = false;
            }
            
            let err_msg = "Always listening controller not available".to_string();
            warn!("[Command] {}", err_msg);
            Err(err_msg)
        }
    }
}

/// Stop always listening mode
#[tauri::command]
pub async fn stop_always_listening_mode(
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<String, String> {
    info!("[Command] stop_always_listening_mode called");

    // Update app state
    if let Ok(mut always_listening_active) = state.always_listening_active.lock() {
        if !*always_listening_active {
            return Ok("Always listening mode is already inactive".to_string());
        }
        *always_listening_active = false;
    } else {
        return Err("Failed to lock always listening state".to_string());
    }

    // Call the plugin command
    match app.try_state::<Arc<Mutex<tauri_plugin_voice_transcription::always_listening::AlwaysListeningController>>>() {
        Some(controller_state) => {
            match tauri_plugin_voice_transcription::commands::stop_always_listening(
                app.clone(),
                controller_state
            ).await {
                Ok(_) => {
                    info!("[Command] Always listening mode stopped successfully");
                    
                    // Emit event to UI
                    if let Err(e) = app.emit("always-listening-mode-changed", false) {
                        error!("[Command] Failed to emit always-listening-mode-changed event: {}", e);
                    }
                    
                    Ok("Always listening mode stopped successfully".to_string())
                }
                Err(e) => {
                    let err_msg = format!("Failed to stop always listening mode: {}", e);
                    error!("[Command] {}", err_msg);
                    Err(err_msg)
                }
            }
        }
        None => {
            let err_msg = "Always listening controller not available".to_string();
            warn!("[Command] {}", err_msg);
            Err(err_msg)
        }
    }
}

/// Toggle always listening mode
#[tauri::command]
pub async fn toggle_always_listening_mode(
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<bool, String> {
    info!("[Command] toggle_always_listening_mode called");

    let was_active = state.always_listening_active.lock()
        .map(|active| *active)
        .unwrap_or(false);

    if was_active {
        stop_always_listening_mode(app, state).await?;
        Ok(false)
    } else {
        start_always_listening_mode(app, state).await?;
        Ok(true)
    }
}

/// Get always listening mode status
#[tauri::command]
pub async fn get_always_listening_status(
    state: State<'_, AppState>,
) -> Result<bool, String> {
    let status = state.always_listening_active.lock()
        .map(|active| *active)
        .unwrap_or(false);
    
    Ok(status)
}

/// Set always listening sensitivity
#[tauri::command]
pub async fn set_always_listening_sensitivity(
    sensitivity: f32,
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<String, String> {
    info!("[Command] set_always_listening_sensitivity called with sensitivity: {}", sensitivity);

    // Update app state
    if let Ok(mut sensitivity_state) = state.always_listening_sensitivity.lock() {
        *sensitivity_state = sensitivity;
    } else {
        return Err("Failed to lock sensitivity state".to_string());
    }

    // Call the plugin command if controller is available
    match app.try_state::<Arc<Mutex<tauri_plugin_voice_transcription::always_listening::AlwaysListeningController>>>() {
        Some(controller_state) => {
            match tauri_plugin_voice_transcription::commands::set_always_listening_sensitivity(
                sensitivity,
                controller_state
            ).await {
                Ok(_) => {
                    info!("[Command] Always listening sensitivity updated successfully");
                    Ok("Sensitivity updated successfully".to_string())
                }
                Err(e) => {
                    let err_msg = format!("Failed to update sensitivity: {}", e);
                    error!("[Command] {}", err_msg);
                    Err(err_msg)
                }
            }
        }
        None => {
            info!("[Command] Always listening controller not available, sensitivity saved to state only");
            Ok("Sensitivity updated in state (controller not active)".to_string())
        }
    }
}

/// Get always listening sensitivity
#[tauri::command]
pub async fn get_always_listening_sensitivity(
    state: State<'_, AppState>,
) -> Result<f32, String> {
    let sensitivity = state.always_listening_sensitivity.lock()
        .map(|s| *s)
        .unwrap_or(0.5);
    
    Ok(sensitivity)
}

/// Set always listening wake words
#[tauri::command]
pub async fn set_always_listening_wake_words(
    wake_words: Vec<String>,
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<String, String> {
    info!("[Command] set_always_listening_wake_words called with wake words: {:?}", wake_words);

    // Update app state
    if let Ok(mut wake_words_state) = state.always_listening_wake_words.lock() {
        *wake_words_state = wake_words.clone();
    } else {
        return Err("Failed to lock wake words state".to_string());
    }

    // Call the plugin command if controller is available
    match app.try_state::<Arc<Mutex<tauri_plugin_voice_transcription::always_listening::AlwaysListeningController>>>() {
        Some(controller_state) => {
            match tauri_plugin_voice_transcription::commands::set_always_listening_wake_words(
                wake_words,
                controller_state
            ).await {
                Ok(_) => {
                    info!("[Command] Always listening wake words updated successfully");
                    Ok("Wake words updated successfully".to_string())
                }
                Err(e) => {
                    let err_msg = format!("Failed to update wake words: {}", e);
                    error!("[Command] {}", err_msg);
                    Err(err_msg)
                }
            }
        }
        None => {
            info!("[Command] Always listening controller not available, wake words saved to state only");
            Ok("Wake words updated in state (controller not active)".to_string())
        }
    }
}

/// Get always listening wake words
#[tauri::command]
pub async fn get_always_listening_wake_words(
    state: State<'_, AppState>,
) -> Result<Vec<String>, String> {
    let wake_words = state.always_listening_wake_words.lock()
        .map(|w| w.clone())
        .unwrap_or_default();
    
    Ok(wake_words)
}
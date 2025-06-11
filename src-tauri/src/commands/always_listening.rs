#[cfg(feature = "voice-features")]
use crate::state::AppState;
#[cfg(feature = "voice-features")]
use tauri::{State, AppHandle, Manager, Emitter};
#[cfg(feature = "voice-features")]
use std::sync::{Arc, Mutex};
#[cfg(feature = "voice-features")]
use tracing::{info, error, warn};

/// Start always listening mode
#[cfg(feature = "voice-features")]
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

                    // Update floating bar
                    crate::commands::floating_bar::handle_always_listening_change(&app, true).await;

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
#[cfg(feature = "voice-features")]
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

                    // Update floating bar
                    crate::commands::floating_bar::handle_always_listening_change(&app, false).await;

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
#[cfg(feature = "voice-features")]
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
#[cfg(feature = "voice-features")]
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
#[cfg(feature = "voice-features")]
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
#[cfg(feature = "voice-features")]
#[tauri::command]
pub async fn get_always_listening_sensitivity(
    state: State<'_, AppState>,
) -> Result<f32, String> {
    let sensitivity = state.always_listening_sensitivity.lock()
        .map(|s| *s)
        .unwrap_or(0.5); // Default sensitivity

    Ok(sensitivity)
}

/// Set always listening wake words
#[cfg(feature = "voice-features")]
#[tauri::command]
pub async fn set_always_listening_wake_words(
    wake_words: Vec<String>,
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<String, String> {
    info!("[Command] set_always_listening_wake_words called with words: {:?}", wake_words);

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
#[cfg(feature = "voice-features")]
#[tauri::command]
pub async fn get_always_listening_wake_words(
    state: State<'_, AppState>,
) -> Result<Vec<String>, String> {
    let wake_words = state.always_listening_wake_words.lock()
        .map(|words| words.clone())
        .unwrap_or_else(|_| vec!["hey juno".to_string()]); // Default wake words

    Ok(wake_words)
}

// Continue with other voice-features functions...
#[cfg(feature = "voice-features")]
#[tauri::command]
pub async fn debug_always_listening_status(
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<serde_json::Value, String> {
    info!("[Command] debug_always_listening_status called");

    // Get app state info
    let app_active = state.always_listening_active.lock()
        .map(|active| *active)
        .unwrap_or(false);

    let app_sensitivity = state.always_listening_sensitivity.lock()
        .map(|s| *s)
        .unwrap_or(0.5);

    let app_wake_words = state.always_listening_wake_words.lock()
        .map(|words| words.clone())
        .unwrap_or_default();

    // Get plugin state info if available
    let plugin_status = match app.try_state::<Arc<Mutex<tauri_plugin_voice_transcription::always_listening::AlwaysListeningController>>>() {
        Some(controller_state) => {
            match controller_state.lock() {
                Ok(controller) => {
                    serde_json::json!({
                        "controller_available": true,
                        "details": "Controller state available"
                    })
                }
                Err(e) => {
                    serde_json::json!({
                        "controller_available": false,
                        "error": format!("Failed to lock controller: {}", e)
                    })
                }
            }
        }
        None => {
            serde_json::json!({
                "controller_available": false,
                "error": "Controller not initialized"
            })
        }
    };

    let debug_info = serde_json::json!({
        "app_state": {
            "active": app_active,
            "sensitivity": app_sensitivity,
            "wake_words": app_wake_words
        },
        "plugin_state": plugin_status,
        "timestamp": chrono::Utc::now().to_rfc3339()
    });

    info!("[Command] Debug info: {}", debug_info);
    Ok(debug_info)
}

/// Enable/disable transcription debugging
#[cfg(feature = "voice-features")]
#[tauri::command]
pub async fn set_transcription_debugging(
    enabled: bool,
    app: AppHandle,
) -> Result<String, String> {
    info!("[Command] set_transcription_debugging called with enabled: {}", enabled);

    // Call the plugin command
    match app.try_state::<Arc<Mutex<tauri_plugin_voice_transcription::always_listening::AlwaysListeningController>>>() {
        Some(controller_state) => {
            match tauri_plugin_voice_transcription::commands::set_transcription_debugging(
                enabled,
                app.clone(),
                controller_state
            ).await {
                Ok(_) => {
                    info!("[Command] Transcription debugging {} successfully",
                          if enabled { "enabled" } else { "disabled" });
                    Ok(format!("Transcription debugging {}",
                              if enabled { "enabled" } else { "disabled" }))
                }
                Err(e) => {
                    let err_msg = format!("Failed to set transcription debugging: {}", e);
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

/// Enable/disable audio level monitoring
#[cfg(feature = "voice-features")]
#[tauri::command]
pub async fn set_audio_level_monitoring(
    enabled: bool,
    app: AppHandle,
) -> Result<String, String> {
    info!("[Command] set_audio_level_monitoring called with enabled: {}", enabled);

    // Call the plugin command
    match app.try_state::<Arc<Mutex<tauri_plugin_voice_transcription::always_listening::AlwaysListeningController>>>() {
        Some(controller_state) => {
            match tauri_plugin_voice_transcription::commands::set_audio_level_monitoring(
                enabled,
                app.clone(),
                controller_state
            ).await {
                Ok(_) => {
                    info!("[Command] Audio level monitoring {} successfully",
                          if enabled { "enabled" } else { "disabled" });
                    Ok(format!("Audio level monitoring {}",
                              if enabled { "enabled" } else { "disabled" }))
                }
                Err(e) => {
                    let err_msg = format!("Failed to set audio level monitoring: {}", e);
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

/// Test the Whisper model with synthetic audio
#[cfg(feature = "voice-features")]
#[tauri::command]
pub async fn test_whisper_model(
    app: AppHandle,
) -> Result<serde_json::Value, String> {
    info!("[Command] test_whisper_model called");

    // Call the plugin command
    match app.try_state::<Arc<Mutex<tauri_plugin_voice_transcription::always_listening::AlwaysListeningController>>>() {
        Some(controller_state) => {
            match tauri_plugin_voice_transcription::commands::test_whisper_model(
                controller_state
            ).await {
                Ok(result) => {
                    info!("[Command] Whisper model test completed");
                    Ok(result)
                }
                Err(e) => {
                    let err_msg = format!("Whisper model test failed: {}", e);
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

/// Force a transcription test with live audio
#[cfg(feature = "voice-features")]
#[tauri::command]
pub async fn force_transcription_test(
    app: AppHandle,
) -> Result<serde_json::Value, String> {
    info!("[Command] force_transcription_test called");

    // Call the plugin command
    match app.try_state::<Arc<Mutex<tauri_plugin_voice_transcription::always_listening::AlwaysListeningController>>>() {
        Some(controller_state) => {
            match tauri_plugin_voice_transcription::commands::force_transcription_test(
                app.clone(),
                controller_state
            ).await {
                Ok(result) => {
                    info!("[Command] Force transcription test completed");
                    Ok(result)
                }
                Err(e) => {
                    let err_msg = format!("Force transcription test failed: {}", e);
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

// Stub implementations when voice features are disabled
#[cfg(not(feature = "voice-features"))]
#[tauri::command]
pub async fn start_always_listening_mode(
    _app: tauri::AppHandle,
    _state: tauri::State<'_, crate::state::AppState>,
) -> Result<String, String> {
    Err("Voice features are disabled in this build".to_string())
}

#[cfg(not(feature = "voice-features"))]
#[tauri::command]
pub async fn stop_always_listening_mode(
    _app: tauri::AppHandle,
    _state: tauri::State<'_, crate::state::AppState>,
) -> Result<String, String> {
    Err("Voice features are disabled in this build".to_string())
}

#[cfg(not(feature = "voice-features"))]
#[tauri::command]
pub async fn toggle_always_listening_mode(
    _app: tauri::AppHandle,
    _state: tauri::State<'_, crate::state::AppState>,
) -> Result<bool, String> {
    Err("Voice features are disabled in this build".to_string())
}

#[cfg(not(feature = "voice-features"))]
#[tauri::command]
pub async fn get_always_listening_status(
    _state: tauri::State<'_, crate::state::AppState>,
) -> Result<bool, String> {
    Ok(false)
}

#[cfg(not(feature = "voice-features"))]
#[tauri::command]
pub async fn set_always_listening_sensitivity(
    _sensitivity: f32,
    _app: tauri::AppHandle,
    _state: tauri::State<'_, crate::state::AppState>,
) -> Result<String, String> {
    Err("Voice features are disabled in this build".to_string())
}

#[cfg(not(feature = "voice-features"))]
#[tauri::command]
pub async fn get_always_listening_sensitivity(
    _state: tauri::State<'_, crate::state::AppState>,
) -> Result<f32, String> {
    Ok(0.5) // Default value
}

#[cfg(not(feature = "voice-features"))]
#[tauri::command]
pub async fn set_always_listening_wake_words(
    _wake_words: Vec<String>,
    _app: tauri::AppHandle,
    _state: tauri::State<'_, crate::state::AppState>,
) -> Result<String, String> {
    Err("Voice features are disabled in this build".to_string())
}

#[cfg(not(feature = "voice-features"))]
#[tauri::command]
pub async fn get_always_listening_wake_words(
    _state: tauri::State<'_, crate::state::AppState>,
) -> Result<Vec<String>, String> {
    Ok(vec![])
}

#[cfg(not(feature = "voice-features"))]
#[tauri::command]
pub async fn debug_always_listening_status(
    _app: tauri::AppHandle,
    _state: tauri::State<'_, crate::state::AppState>,
) -> Result<serde_json::Value, String> {
    Ok(serde_json::json!({
        "voice_features_disabled": true,
        "active": false,
        "sensitivity": 0.5,
        "wake_words": [],
        "debug_enabled": false
    }))
}

// Add stub implementations for remaining functions
#[cfg(not(feature = "voice-features"))]
#[tauri::command]
pub async fn set_transcription_debugging(
    _enabled: bool,
    _app: tauri::AppHandle,
) -> Result<String, String> {
    Err("Voice features are disabled in this build".to_string())
}

#[cfg(not(feature = "voice-features"))]
#[tauri::command]
pub async fn set_audio_level_monitoring(
    _enabled: bool,
    _app: tauri::AppHandle,
) -> Result<String, String> {
    Err("Voice features are disabled in this build".to_string())
}

#[cfg(not(feature = "voice-features"))]
#[tauri::command]
pub async fn test_whisper_model(
    _app: tauri::AppHandle,
) -> Result<serde_json::Value, String> {
    Err("Voice features are disabled in this build".to_string())
}

#[cfg(not(feature = "voice-features"))]
#[tauri::command]
pub async fn force_transcription_test(
    _app: tauri::AppHandle,
) -> Result<serde_json::Value, String> {
    Err("Voice features are disabled in this build".to_string())
}

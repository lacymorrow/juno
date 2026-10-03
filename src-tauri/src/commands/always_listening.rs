use crate::constants::errors::prefixes::COMMAND;
use crate::constants::errors::templates::FAILED_TO_EMIT;
use crate::constants::events;
use crate::settings::manager::SettingsManager;
use crate::state::AppState;
use std::sync::{Arc, Mutex};
use tauri::{AppHandle, Emitter, Manager, State};
use tracing::{error, info, warn};

// Helper function for error formatting - properly handles template substitution
fn format_error(template: &str, context: &str, error: impl std::fmt::Display) -> String {
    template
        .replacen("{}", context, 1)
        .replacen("{}", &error.to_string(), 1)
}

/// Start always listening mode.
///
/// Refused unless the stored voice triggers, through the launch gate, want a
/// wake phrase listened for. This is the one door to the always-listening
/// microphone, so every caller (a trigger save, launch, the end of a
/// dictation, the bar's resume control, a device change) obeys the same rule
/// and none of them can re-open the microphone after voice was switched off.
#[tauri::command]
pub async fn start_always_listening_mode(
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<String, String> {
    info!("[Command] start_always_listening_mode called");

    if crate::commands::triggers::armed_phrases_now(&app).is_empty() {
        info!("[Command] No voice trigger is armed, the microphone stays off");
        return Err("No voice trigger is on".to_string());
    }

    // Get current settings from centralized system
    let settings_manager = SettingsManager::new(app.clone())
        .map_err(|e| format!("Failed to create settings manager: {}", e))?;

    let mut audio_settings = settings_manager
        .get_audio_settings()
        .await
        .map_err(|e| format!("Failed to get audio settings: {}", e))?;

    // No early return on the stored flag: the controller knows whether it is
    // running and starting it twice is a no-op there. Trusting the flag is
    // what left a stopped engine reporting itself as running.

    // Update centralized settings
    audio_settings.always_listening_active = true;
    settings_manager
        .set_audio_settings(&audio_settings)
        .await
        .map_err(|e| format!("Failed to save audio settings: {}", e))?;

    // Update app state
    if let Err(e) = state.set_always_listening_active(true) {
        let err_msg = format!("Failed to set always_listening_active: {}", e);
        error!("[Command] {}", err_msg);
        return Err(err_msg);
    }
    info!("[Command] Successfully updated app state: always_listening_active = true");

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
                    if let Err(e) = app.emit(events::always_listening::MODE_CHANGED, true) {
                        error!("{} {}", COMMAND, format_error(FAILED_TO_EMIT, "always-listening-mode-changed", e));
                    }
                    crate::commands::triggers::emit_listening_outcome(&app, true);

                    // The bar state is deliberately left alone. Waiting for a
                    // wake phrase is not an activity worth showing: it is the
                    // resting state, and putting the bar into AlwaysListening
                    // here made "armed" and "just heard you" look identical.
                    // The bar grows when the phrase actually lands, driven from
                    // the wake-word listener in `integration.rs`.

                    Ok("Always listening mode started successfully".to_string())
                }
                Err(e) => {
                    // Reset centralized settings on failure
                    audio_settings.always_listening_active = false;
                    if let Err(save_err) = settings_manager.set_audio_settings(&audio_settings).await {
                        warn!("[Command] Failed to reset audio settings after error: {}", save_err);
                    }

                    // Reset state on failure
                    if let Err(e) = state.set_always_listening_active(false) {
                        warn!("[Command] Failed to reset always_listening_active: {}", e);
                    }

                    let err_msg = format!("Failed to start always listening mode: {}", e);
                    error!("[Command] {}", err_msg);
                    Err(err_msg)
                }
            }
        }
        None => {
            // Reset centralized settings on failure
            audio_settings.always_listening_active = false;
            if let Err(save_err) = settings_manager.set_audio_settings(&audio_settings).await {
                warn!("[Command] Failed to reset audio settings after controller error: {}", save_err);
            }

            // Reset state on failure
            if let Err(e) = state.set_always_listening_active(false) {
                warn!("[Command] Failed to reset always_listening_active: {}", e);
            }

            let err_msg = "Always listening controller not available".to_string();
            warn!("[Command] {}", err_msg);
            Err(err_msg)
        }
    }
}

/// Stop always listening mode.
///
/// Always stops the controller. It used to return early when the stored
/// flag already read "off", and the flag is not the microphone: a dictation
/// pause, a failed start or a busy controller could each leave it off over a
/// worker that was still capturing, and from then on every stop, the
/// settings toggle included, believed there was nothing to do. The plugin's
/// stop waits for the controller and joins the worker, so when this returns
/// the stream has been dropped.
#[tauri::command]
pub async fn stop_always_listening_mode(
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<String, String> {
    info!("[Command] stop_always_listening_mode called");

    // Stop the microphone first. The bookkeeping below can fail; this must
    // not wait on it.
    let stopped =
        match app.try_state::<Arc<
            Mutex<tauri_plugin_voice_transcription::always_listening::AlwaysListeningController>,
        >>() {
            Some(controller_state) => {
                tauri_plugin_voice_transcription::commands::stop_always_listening(
                    app.clone(),
                    controller_state,
                )
                .await
                .map_err(|e| {
                    let err_msg = format!("Failed to stop always listening mode: {}", e);
                    error!("[Command] {}", err_msg);
                    err_msg
                })?
            }
            // No controller, no microphone.
            None => false,
        };

    if let Err(e) = state.set_always_listening_active(false) {
        warn!("[Command] Failed to clear always_listening_active: {}", e);
    }
    match SettingsManager::new(app.clone()) {
        Ok(settings_manager) => match settings_manager.get_audio_settings().await {
            Ok(mut audio_settings) if audio_settings.always_listening_active => {
                audio_settings.always_listening_active = false;
                if let Err(e) = settings_manager.set_audio_settings(&audio_settings).await {
                    warn!("[Command] Failed to save audio settings: {}", e);
                }
            }
            Ok(_) => {}
            Err(e) => warn!("[Command] Failed to get audio settings: {}", e),
        },
        Err(e) => warn!("[Command] Failed to create settings manager: {}", e),
    }

    info!(
        "[Command] Always listening stopped (was running: {})",
        stopped
    );

    crate::commands::triggers::emit_listening_outcome(&app, false);
    if stopped {
        if let Err(e) = app.emit(events::always_listening::MODE_CHANGED, false) {
            error!(
                "{} {}",
                COMMAND,
                format_error(FAILED_TO_EMIT, "always-listening-mode-changed", e)
            );
        }
        crate::commands::ui_commands::handle_always_listening_change(&app, false).await;
    }

    Ok("Always listening mode stopped".to_string())
}

/// Toggle always listening mode
#[tauri::command]
pub async fn toggle_always_listening_mode(
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<bool, String> {
    info!("[Command] toggle_always_listening_mode called");

    // Get current settings from centralized system
    let settings_manager = SettingsManager::new(app.clone())
        .map_err(|e| format!("Failed to create settings manager: {}", e))?;

    let audio_settings = settings_manager
        .get_audio_settings()
        .await
        .map_err(|e| format!("Failed to get audio settings: {}", e))?;

    if audio_settings.always_listening_active {
        stop_always_listening_mode(app, state).await?;
        Ok(false)
    } else {
        start_always_listening_mode(app, state).await?;
        Ok(true)
    }
}

/// Get always listening mode status
#[tauri::command]
pub async fn get_always_listening_status(app: AppHandle) -> Result<bool, String> {
    // Get status from centralized settings
    let settings_manager = SettingsManager::new(app.clone())
        .map_err(|e| format!("Failed to create settings manager: {}", e))?;

    let audio_settings = settings_manager
        .get_audio_settings()
        .await
        .map_err(|e| format!("Failed to get audio settings: {}", e))?;

    Ok(audio_settings.always_listening_active)
}

/// Set always listening sensitivity
#[tauri::command]
pub async fn set_always_listening_sensitivity(
    sensitivity: f32,
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<String, String> {
    info!(
        "[Command] set_always_listening_sensitivity called with sensitivity: {}",
        sensitivity
    );

    // Validate sensitivity range
    if !(0.0..=1.0).contains(&sensitivity) {
        return Err("Sensitivity must be between 0.0 and 1.0".to_string());
    }

    // Get current settings from centralized system
    let settings_manager = SettingsManager::new(app.clone())
        .map_err(|e| format!("Failed to create settings manager: {}", e))?;

    let mut audio_settings = settings_manager
        .get_audio_settings()
        .await
        .map_err(|e| format!("Failed to get audio settings: {}", e))?;

    // Update centralized settings
    audio_settings.always_listening_sensitivity = sensitivity;
    settings_manager
        .set_audio_settings(&audio_settings)
        .await
        .map_err(|e| format!("Failed to save audio settings: {}", e))?;

    // Update app state
    if let Err(e) = state.set_always_listening_sensitivity(sensitivity) {
        let err_msg = format!("Failed to set always_listening_sensitivity: {}", e);
        error!("[Command] {}", err_msg);
        return Err(err_msg);
    }
    info!(
        "[Command] Successfully updated app state: always_listening_sensitivity = {}",
        sensitivity
    );

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
            info!("[Command] Always listening controller not available, sensitivity saved to centralized settings");
            Ok("Sensitivity updated in centralized settings (controller not active)".to_string())
        }
    }
}

/// Get always listening sensitivity
#[tauri::command]
pub async fn get_always_listening_sensitivity(app: AppHandle) -> Result<f32, String> {
    // Get sensitivity from centralized settings
    let settings_manager = SettingsManager::new(app.clone())
        .map_err(|e| format!("Failed to create settings manager: {}", e))?;

    let audio_settings = settings_manager
        .get_audio_settings()
        .await
        .map_err(|e| format!("Failed to get audio settings: {}", e))?;

    Ok(audio_settings.always_listening_sensitivity)
}

/// Set always listening wake words
#[tauri::command]
pub async fn set_always_listening_wake_words(
    wake_words: Vec<String>,
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<String, String> {
    info!(
        "[Command] set_always_listening_wake_words called with {} wake words",
        wake_words.len()
    );

    // Validate wake words
    if wake_words.is_empty() {
        return Err("At least one wake word is required".to_string());
    }

    // Get current settings from centralized system
    let settings_manager = SettingsManager::new(app.clone())
        .map_err(|e| format!("Failed to create settings manager: {}", e))?;

    let mut audio_settings = settings_manager
        .get_audio_settings()
        .await
        .map_err(|e| format!("Failed to get audio settings: {}", e))?;

    // Update centralized settings
    audio_settings.always_listening_wake_words = wake_words.clone();
    settings_manager
        .set_audio_settings(&audio_settings)
        .await
        .map_err(|e| format!("Failed to save audio settings: {}", e))?;

    // Update app state
    if let Err(e) = state.set_always_listening_wake_words(wake_words.clone()) {
        let err_msg = format!("Failed to set always_listening_wake_words: {}", e);
        error!("[Command] {}", err_msg);
        return Err(err_msg);
    }
    info!(
        "[Command] Successfully updated app state: always_listening_wake_words = {:?}",
        wake_words
    );

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
            info!("[Command] Always listening controller not available, wake words saved to centralized settings");
            Ok("Wake words updated in centralized settings (controller not active)".to_string())
        }
    }
}

/// Get always listening wake words
#[tauri::command]
pub async fn get_always_listening_wake_words(app: AppHandle) -> Result<Vec<String>, String> {
    // Get wake words from centralized settings
    let settings_manager = SettingsManager::new(app.clone())
        .map_err(|e| format!("Failed to create settings manager: {}", e))?;

    let audio_settings = settings_manager
        .get_audio_settings()
        .await
        .map_err(|e| format!("Failed to get audio settings: {}", e))?;

    Ok(audio_settings.always_listening_wake_words)
}

/// Debug command to get detailed always listening status
#[tauri::command]
pub async fn debug_always_listening_status(
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<serde_json::Value, String> {
    info!("[Command] debug_always_listening_status called");

    // Get app state
    let is_active = state.get_always_listening_active().unwrap_or(false);
    let sensitivity = state.get_always_listening_sensitivity().unwrap_or(0.5);
    let wake_words = state.get_always_listening_wake_words().unwrap_or_default();

    // Try to get plugin status if available
    let plugin_status =
        match app.try_state::<Arc<
            Mutex<tauri_plugin_voice_transcription::always_listening::AlwaysListeningController>,
        >>() {
            Some(controller_state) => match controller_state.try_lock() {
                Ok(controller) => {
                    serde_json::json!({
                        "plugin_active": controller.is_active(),
                        "plugin_sensitivity": controller.get_sensitivity(),
                        "plugin_wake_words": controller.get_wake_words(),
                        "plugin_available": true
                    })
                }
                Err(_) => {
                    serde_json::json!({
                        "plugin_available": true,
                        "plugin_locked": true,
                        "message": "Plugin controller is currently locked"
                    })
                }
            },
            None => {
                serde_json::json!({
                    "plugin_available": false,
                    "message": "Always listening controller not initialized"
                })
            }
        };

    let debug_info = serde_json::json!({
        "app_state": {
            "is_active": is_active,
            "sensitivity": sensitivity,
            "wake_words": wake_words
        },
        "plugin_state": plugin_status,
        "system_info": {
            "timestamp": chrono::Utc::now().to_rfc3339(),
            "version": "1.0.0"
        }
    });

    Ok(debug_info)
}

/// Enhanced Debugging Commands
/// Enable/disable transcription debugging
#[tauri::command]
pub async fn set_transcription_debugging(enabled: bool, app: AppHandle) -> Result<String, String> {
    info!(
        "[Command] set_transcription_debugging called with enabled: {}",
        enabled
    );

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
#[tauri::command]
pub async fn set_audio_level_monitoring(enabled: bool, app: AppHandle) -> Result<String, String> {
    info!(
        "[Command] set_audio_level_monitoring called with enabled: {}",
        enabled
    );

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
#[tauri::command]
pub async fn test_whisper_model(app: AppHandle) -> Result<serde_json::Value, String> {
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
#[tauri::command]
pub async fn force_transcription_test(app: AppHandle) -> Result<serde_json::Value, String> {
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

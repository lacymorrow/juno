use tauri::{State, Manager, AppHandle, Emitter};
use std::sync::{Arc, Mutex};
use crate::controller::VoiceController;
use crate::always_listening::AlwaysListeningController;
use crate::error::Error;
use crate::config::VoiceTranscriptionConfig;
use crate::utils::resolve_model_path;
use crate::engine::SttProvider;
use crate::engine_manager::EngineManager;
use crate::engine_parakeet::ParakeetModelStatus;
use tracing::{info, error};
use serde_json::json;
use crate::constants;

/// Enhanced helper function to check VoiceController status and provide comprehensive error messages
/// Uses try_lock to avoid blocking if the mutex is held by another thread
fn check_voice_controller_availability<R: tauri::Runtime>(
    app: &AppHandle<R>
) -> Result<(), Error> {
    match app.try_state::<Arc<Mutex<VoiceController>>>() {
        Some(controller_state) => {
            // State is managed, try to check if controller is actually initialized
            // Use try_lock to avoid blocking - if lock is held, assume controller is busy but available
            match controller_state.try_lock() {
                Ok(controller) => {
                    if !controller.is_initialized() {
                        let error_msg = if let Some(init_error) = controller.get_initialization_error() {
                            format!("Voice transcription is not available. Initialization failed: {}\n\
                                     This usually happens when:\n\
                                     1. The Whisper model file is missing or corrupted\n\
                                     2. The model path cannot be resolved\n\
                                     3. WhisperContext creation failed\n\
                                     Check the app logs for detailed initialization errors.", init_error)
                        } else {
                            "Voice transcription is not available. VoiceController failed to initialize.\n\
                             Check the app logs for initialization errors.".to_string()
                        };
                        error!("[Plugin] VoiceController not initialized: {}", error_msg);
                        return Err(Error::InitializationError(error_msg));
                    }
                    info!("[Plugin] VoiceController availability check passed");
                    Ok(())
                }
                Err(std::sync::TryLockError::WouldBlock) => {
                    // Lock is held - controller exists and is busy, which is fine for availability check
                    info!("[Plugin] VoiceController lock is busy - assuming controller is available (in use)");
                    Ok(())
                }
                Err(std::sync::TryLockError::Poisoned(e)) => {
                    error!("[Plugin] VoiceController mutex is poisoned: {}", e);
                    Err(Error::LockError(format!("VoiceController mutex is poisoned: {}", e)))
                }
            }
        }
        None => {
            let error_msg = "Voice transcription is not available. The VoiceController state is not managed by Tauri.\n\
                            This indicates a critical plugin initialization failure.\n\
                            Check the app logs for initialization errors.";
            error!("[Plugin] VoiceController state not managed: {}", error_msg);
            Err(Error::InitializationError(error_msg.to_string()))
        }
    }
}

#[tauri::command]
pub async fn get_initialization_status(
    controller: State<'_, Arc<Mutex<VoiceController>>>,
) -> Result<serde_json::Value, Error> {
    info!("[Plugin] get_initialization_status command called");

    let voice_controller = match controller.try_lock() {
        Ok(guard) => guard,
        Err(std::sync::TryLockError::WouldBlock) => {
            return Ok(json!({
                "is_initialized": true,
                "model_path": null,
                "initialization_error": null,
                "is_dictating": null,
                "state_managed": true,
                "lock_busy": true
            }));
        }
        Err(std::sync::TryLockError::Poisoned(e)) => {
            return Err(Error::LockError(format!("VoiceController mutex is poisoned: {}", e)));
        }
    };

    let status = json!({
        "is_initialized": voice_controller.is_initialized(),
        "model_path": voice_controller.model_path,
        "initialization_error": voice_controller.get_initialization_error(),
        "is_dictating": voice_controller.is_dictating(),
        "state_managed": true
    });

    info!("[Plugin] Initialization status: {}", status);
    Ok(status)
}

#[tauri::command]
pub async fn start_dictation<R: tauri::Runtime + 'static>(
    app: AppHandle<R>,
    controller: State<'_, Arc<Mutex<VoiceController>>>,
) -> Result<(), Error> {
    info!("[Plugin] start_dictation command called");

    // Check microphone permissions first
    match crate::mic_permissions::ensure_microphone_ready().await {
        Ok(()) => {
            info!("[Plugin] Microphone permissions verified");
        }
        Err(e) => {
            error!("[Plugin] Microphone permission check failed: {}", e);
            return Err(Error::PermissionError(e));
        }
    }

    // Check initialization status before proceeding
    check_voice_controller_availability(&app)?;

    // Use try_lock to avoid blocking if another operation is in progress
    let mut voice_controller = match controller.try_lock() {
        Ok(guard) => guard,
        Err(std::sync::TryLockError::WouldBlock) => {
            info!("[Plugin] VoiceController is busy - dictation may already be starting or stopping");
            return Err(Error::LockError("VoiceController is busy - please try again".to_string()));
        }
        Err(std::sync::TryLockError::Poisoned(e)) => {
            error!("[Plugin] VoiceController mutex is poisoned: {}", e);
            return Err(Error::LockError(format!("VoiceController mutex is poisoned: {}", e)));
        }
    };

    voice_controller.start_dictation(&app)?;

    // Emit started event through the plugin system - sound will be handled automatically by backend
    app.emit(constants::plugin::VOICE_TRANSCRIPTION_DICTATION_STARTED, ())
        .map_err(|e| Error::EventError(format!("Failed to emit dictation-started event: {}", e)))?;

    info!("[Plugin] Dictation started successfully");
    Ok(())
}

#[tauri::command]
pub async fn stop_dictation<R: tauri::Runtime>(
    app: AppHandle<R>,
    controller: State<'_, Arc<Mutex<VoiceController>>>,
) -> Result<bool, Error> {
    info!("[Plugin] stop_dictation command called");

    // Use try_lock to avoid blocking if another operation is in progress
    let mut voice_controller = match controller.try_lock() {
        Ok(guard) => guard,
        Err(std::sync::TryLockError::WouldBlock) => {
            info!("[Plugin] VoiceController is busy - stop will be handled when lock is available");
            // Return false to indicate dictation wasn't stopped (it might be already stopping)
            return Ok(false);
        }
        Err(std::sync::TryLockError::Poisoned(e)) => {
            error!("[Plugin] VoiceController mutex is poisoned: {}", e);
            return Err(Error::LockError(format!("VoiceController mutex is poisoned: {}", e)));
        }
    };

    let result = voice_controller.stop_dictation()?;

    if result {
        // Emit stopped event through the plugin system - sound will be handled automatically by backend
        app.emit(constants::plugin::VOICE_TRANSCRIPTION_DICTATION_STOPPED, ())
            .map_err(|e| Error::EventError(format!("Failed to emit dictation-stopped event: {}", e)))?;
    }

    info!("[Plugin] Dictation stopped: {}", result);
    Ok(result)
}

#[tauri::command]
pub async fn toggle_dictation<R: tauri::Runtime + 'static>(
    app: AppHandle<R>,
    controller: State<'_, Arc<Mutex<VoiceController>>>,
) -> Result<bool, Error> {
    info!("[Plugin] toggle_dictation command called");

    // Enhanced check that verifies both state management and initialization status
    check_voice_controller_availability(&app)?;

    // Acquire-check-release in a block so the MutexGuard (which is !Send) is
    // dropped before any .await, satisfying the Send bound on Tauri command futures.
    let was_dictating = {
        let voice_controller = match controller.try_lock() {
            Ok(guard) => guard,
            Err(std::sync::TryLockError::WouldBlock) => {
                info!("[Plugin] VoiceController is busy - toggle deferred");
                return Err(Error::LockError("VoiceController is busy - please try again".to_string()));
            }
            Err(std::sync::TryLockError::Poisoned(e)) => {
                error!("[Plugin] VoiceController mutex is poisoned: {}", e);
                return Err(Error::LockError(format!("VoiceController mutex is poisoned: {}", e)));
            }
        };
        voice_controller.is_dictating()
    }; // guard dropped here — before any .await

    if was_dictating {
        let mut voice_controller = match controller.try_lock() {
            Ok(guard) => guard,
            Err(std::sync::TryLockError::WouldBlock) => {
                return Err(Error::LockError("VoiceController is busy - please try again".to_string()));
            }
            Err(std::sync::TryLockError::Poisoned(e)) => {
                return Err(Error::LockError(format!("VoiceController mutex is poisoned: {}", e)));
            }
        };
        voice_controller.stop_dictation()?;

        app.emit(constants::plugin::VOICE_TRANSCRIPTION_DICTATION_STOPPED, ())
            .map_err(|e| Error::EventError(format!("Failed to emit dictation-stopped event: {}", e)))?;
        Ok(false)
    } else {
        crate::mic_permissions::ensure_microphone_ready().await
            .map_err(|e| {
                error!("[Plugin] Microphone permission check failed in toggle: {}", e);
                Error::PermissionError(e)
            })?;
        info!("[Plugin] Microphone permissions verified for toggle");

        let mut voice_controller = match controller.try_lock() {
            Ok(guard) => guard,
            Err(std::sync::TryLockError::WouldBlock) => {
                return Err(Error::LockError("VoiceController is busy after permission check".to_string()));
            }
            Err(std::sync::TryLockError::Poisoned(e)) => {
                return Err(Error::LockError(format!("VoiceController mutex is poisoned: {}", e)));
            }
        };

        match voice_controller.start_dictation(&app) {
            Ok(()) => {
                app.emit(constants::plugin::VOICE_TRANSCRIPTION_DICTATION_STARTED, ())
                    .map_err(|e| Error::EventError(format!("Failed to emit dictation-started event: {}", e)))?;
                Ok(true)
            }
            Err(e) => Err(e),
        }
    }
}

#[tauri::command]
pub async fn get_dictation_status(
    controller: State<'_, Arc<Mutex<VoiceController>>>,
) -> Result<bool, Error> {
    info!("[Plugin] get_dictation_status command called");

    let voice_controller = match controller.try_lock() {
        Ok(guard) => guard,
        Err(std::sync::TryLockError::WouldBlock) => {
            // Lock held means some operation is in progress, assume dictating
            info!("[Plugin] VoiceController lock busy - assuming active");
            return Ok(true);
        }
        Err(std::sync::TryLockError::Poisoned(e)) => {
            return Err(Error::LockError(format!("VoiceController mutex is poisoned: {}", e)));
        }
    };

    Ok(voice_controller.is_dictating())
}

#[tauri::command]
pub async fn transcribe_file<R: tauri::Runtime>(
    path: String,
    app: AppHandle<R>,
    controller: State<'_, Arc<Mutex<VoiceController>>>,
) -> Result<String, Error> {
    info!("[Plugin] transcribe_file command called for path: {}", path);

    // Check initialization status before proceeding
    check_voice_controller_availability(&app)?;

    let voice_controller = match controller.try_lock() {
        Ok(guard) => guard,
        Err(std::sync::TryLockError::WouldBlock) => {
            return Err(Error::LockError("VoiceController is busy - please try again".to_string()));
        }
        Err(std::sync::TryLockError::Poisoned(e)) => {
            return Err(Error::LockError(format!("VoiceController mutex is poisoned: {}", e)));
        }
    };

    voice_controller.transcribe_audio_file(&path)
        .map_err(Error::TranscriptionError)
}

#[tauri::command]
pub async fn set_model_path<R: tauri::Runtime>(
    path: String,
    app: AppHandle<R>,
) -> Result<(), Error> {
    info!("[Plugin] set_model_path command called with path: {}", path);

    // Resolve the model path relative to the app directory
    let resolved_model_path = resolve_model_path(&app, &path);

    // Update config
    let _config = VoiceTranscriptionConfig {
        model_path: resolved_model_path.clone(),
        ..VoiceTranscriptionConfig::default()
    };
    // Note: Configuration is now managed through centralized settings

    // Reinitialize the shared Whisper context with the new model
    use crate::shared_whisper::SharedWhisperManager;

    info!("[Plugin] Reinitializing shared Whisper context with new model path: {}", resolved_model_path);
    let shared_context = match SharedWhisperManager::reinitialize(&resolved_model_path) {
        Ok(context) => {
            info!("[Plugin] Shared Whisper context reinitialized successfully");
            context
        }
        Err(e) => {
            error!("[Plugin] Failed to reinitialize shared Whisper context: {}", e);
            return Err(Error::ModelError(format!("Failed to load model: {}", e)));
        }
    };

    // Update existing VoiceController with new shared context (proper state management)
    if let Some(voice_controller_state) = app.try_state::<Arc<Mutex<VoiceController>>>() {
        match voice_controller_state.try_lock() {
            Ok(mut controller) => {
                match controller.update_shared_context(&resolved_model_path, shared_context.clone()) {
                    Ok(_) => {
                        info!("[Plugin] Voice controller updated with shared context");
                    }
                    Err(e) => {
                        error!("[Plugin] Failed to update voice controller with shared context: {}", e);
                        return Err(Error::ModelError(format!("Failed to update voice controller: {}", e)));
                    }
                }
            }
            Err(std::sync::TryLockError::WouldBlock) => {
                error!("[Plugin] VoiceController is busy - cannot update model path now");
                return Err(Error::LockError("VoiceController is busy - cannot update model while in use".to_string()));
            }
            Err(std::sync::TryLockError::Poisoned(e)) => {
                error!("[Plugin] VoiceController mutex is poisoned: {}", e);
                return Err(Error::LockError(format!("VoiceController mutex is poisoned: {}", e)));
            }
        }
    } else {
        let error_msg = "VoiceController state not found - should be managed during plugin initialization";
        error!("[Plugin] {}", error_msg);
        return Err(Error::InitializationError(error_msg.to_string()));
    }

    // Update existing AlwaysListeningController with new shared context (proper state management)
    if let Some(always_listening_state) = app.try_state::<Arc<Mutex<crate::always_listening::AlwaysListeningController>>>() {
        match always_listening_state.try_lock() {
            Ok(mut controller) => {
                match controller.update_shared_context(&resolved_model_path, shared_context.clone()) {
                    Ok(_) => {
                        info!("[Plugin] AlwaysListeningController updated with shared context");
                    }
                    Err(e) => {
                        error!("[Plugin] Failed to update AlwaysListeningController with shared context: {}", e);
                        // Don't fail the whole operation, voice controller still works
                    }
                }
            }
            Err(std::sync::TryLockError::WouldBlock) => {
                error!("[Plugin] AlwaysListeningController is busy - skipping model update");
                // Don't fail the whole operation
            }
            Err(std::sync::TryLockError::Poisoned(e)) => {
                error!("[Plugin] AlwaysListeningController mutex is poisoned: {}", e);
                // Don't fail the whole operation
            }
        }
    } else {
        error!("[Plugin] AlwaysListeningController state not found - should be managed during plugin initialization");
    }

    Ok(())
}

#[tauri::command]
pub async fn get_model_path() -> Result<String, Error> {
    let config = VoiceTranscriptionConfig::default();
    Ok(config.model_path)
}

// Always Listening Mode Commands

#[tauri::command]
pub async fn start_always_listening<R: tauri::Runtime + 'static>(
    app: AppHandle<R>,
    controller: State<'_, Arc<Mutex<AlwaysListeningController>>>,
) -> Result<(), Error> {
    info!("[Plugin] start_always_listening command called");

    // Check microphone permissions first
    match crate::mic_permissions::ensure_microphone_ready().await {
        Ok(()) => {
            info!("[Plugin] Microphone permissions verified for always listening");
        }
        Err(e) => {
            error!("[Plugin] Microphone permission check failed: {}", e);
            return Err(Error::PermissionError(e));
        }
    }

    let mut always_listening_controller = match controller.try_lock() {
        Ok(guard) => guard,
        Err(std::sync::TryLockError::WouldBlock) => {
            return Err(Error::LockError("AlwaysListeningController is busy - please try again".to_string()));
        }
        Err(std::sync::TryLockError::Poisoned(e)) => {
            return Err(Error::LockError(format!("AlwaysListeningController mutex is poisoned: {}", e)));
        }
    };

    always_listening_controller.start_always_listening(&app)?;

    // Emit started event through the plugin system
    app.emit(constants::plugin::ALWAYS_LISTENING_STARTED, ())
        .map_err(|e| Error::EventError(format!("Failed to emit always-listening-started event: {}", e)))?;

    info!("[Plugin] Always listening started successfully");
    Ok(())
}

#[tauri::command]
pub async fn stop_always_listening<R: tauri::Runtime>(
    app: AppHandle<R>,
    controller: State<'_, Arc<Mutex<AlwaysListeningController>>>,
) -> Result<bool, Error> {
    info!("[Plugin] stop_always_listening command called");

    let mut always_listening_controller = match controller.try_lock() {
        Ok(guard) => guard,
        Err(std::sync::TryLockError::WouldBlock) => {
            info!("[Plugin] AlwaysListeningController is busy - stop deferred");
            return Ok(false);
        }
        Err(std::sync::TryLockError::Poisoned(e)) => {
            return Err(Error::LockError(format!("AlwaysListeningController mutex is poisoned: {}", e)));
        }
    };

    let result = always_listening_controller.stop_always_listening()?;

    if result {
        // Emit stopped event through the plugin system
        app.emit(constants::plugin::ALWAYS_LISTENING_STOPPED, ())
            .map_err(|e| Error::EventError(format!("Failed to emit always-listening-stopped event: {}", e)))?;
    }

    info!("[Plugin] Always listening stopped: {}", result);
    Ok(result)
}

#[tauri::command]
pub async fn toggle_always_listening<R: tauri::Runtime + 'static>(
    app: AppHandle<R>,
    controller: State<'_, Arc<Mutex<AlwaysListeningController>>>,
) -> Result<bool, Error> {
    info!("[Plugin] toggle_always_listening command called");

    let was_active = {
        let always_listening_controller = match controller.try_lock() {
            Ok(guard) => guard,
            Err(std::sync::TryLockError::WouldBlock) => {
                return Err(Error::LockError("AlwaysListeningController is busy - please try again".to_string()));
            }
            Err(std::sync::TryLockError::Poisoned(e)) => {
                return Err(Error::LockError(format!("AlwaysListeningController mutex is poisoned: {}", e)));
            }
        };
        always_listening_controller.is_active()
    }; // guard dropped here — before any .await

    if was_active {
        let mut always_listening_controller = match controller.try_lock() {
            Ok(guard) => guard,
            Err(std::sync::TryLockError::WouldBlock) => {
                return Err(Error::LockError("AlwaysListeningController is busy - please try again".to_string()));
            }
            Err(std::sync::TryLockError::Poisoned(e)) => {
                return Err(Error::LockError(format!("AlwaysListeningController mutex is poisoned: {}", e)));
            }
        };
        always_listening_controller.stop_always_listening()?;
        app.emit(constants::plugin::ALWAYS_LISTENING_STOPPED, ())
            .map_err(|e| Error::EventError(format!("Failed to emit always-listening-stopped event: {}", e)))?;
        Ok(false)
    } else {
        crate::mic_permissions::ensure_microphone_ready().await
            .map_err(|e| {
                error!("[Plugin] Microphone permission check failed in always-listening toggle: {}", e);
                Error::PermissionError(e)
            })?;
        info!("[Plugin] Microphone permissions verified for always-listening toggle");

        let mut always_listening_controller = match controller.try_lock() {
            Ok(guard) => guard,
            Err(std::sync::TryLockError::WouldBlock) => {
                return Err(Error::LockError("AlwaysListeningController is busy after permission check".to_string()));
            }
            Err(std::sync::TryLockError::Poisoned(e)) => {
                return Err(Error::LockError(format!("AlwaysListeningController mutex is poisoned: {}", e)));
            }
        };

        always_listening_controller.start_always_listening(&app)?;
        app.emit(constants::plugin::ALWAYS_LISTENING_STARTED, ())
            .map_err(|e| Error::EventError(format!("Failed to emit always-listening-started event: {}", e)))?;
        Ok(true)
    }
}

#[tauri::command]
pub async fn get_always_listening_status(
    controller: State<'_, Arc<Mutex<AlwaysListeningController>>>,
) -> Result<bool, Error> {
    info!("[Plugin] get_always_listening_status command called");

    let always_listening_controller = match controller.try_lock() {
        Ok(guard) => guard,
        Err(std::sync::TryLockError::WouldBlock) => {
            info!("[Plugin] AlwaysListeningController lock busy - assuming active");
            return Ok(true);
        }
        Err(std::sync::TryLockError::Poisoned(e)) => {
            return Err(Error::LockError(format!("AlwaysListeningController mutex is poisoned: {}", e)));
        }
    };

    Ok(always_listening_controller.is_active())
}

#[tauri::command]
pub async fn set_always_listening_sensitivity(
    sensitivity: f32,
    controller: State<'_, Arc<Mutex<AlwaysListeningController>>>,
) -> Result<(), Error> {
    info!("[Plugin] set_always_listening_sensitivity command called with sensitivity: {}", sensitivity);

    let mut always_listening_controller = match controller.try_lock() {
        Ok(guard) => guard,
        Err(std::sync::TryLockError::WouldBlock) => {
            return Err(Error::LockError("AlwaysListeningController is busy - please try again".to_string()));
        }
        Err(std::sync::TryLockError::Poisoned(e)) => {
            return Err(Error::LockError(format!("AlwaysListeningController mutex is poisoned: {}", e)));
        }
    };

    always_listening_controller.set_sensitivity(sensitivity)?;
    Ok(())
}

#[tauri::command]
pub async fn get_always_listening_sensitivity(
    controller: State<'_, Arc<Mutex<AlwaysListeningController>>>,
) -> Result<f32, Error> {
    let always_listening_controller = match controller.try_lock() {
        Ok(guard) => guard,
        Err(std::sync::TryLockError::WouldBlock) => {
            return Err(Error::LockError("AlwaysListeningController is busy - please try again".to_string()));
        }
        Err(std::sync::TryLockError::Poisoned(e)) => {
            return Err(Error::LockError(format!("AlwaysListeningController mutex is poisoned: {}", e)));
        }
    };

    Ok(always_listening_controller.get_sensitivity())
}

#[tauri::command]
pub async fn set_always_listening_wake_words(
    wake_words: Vec<String>,
    controller: State<'_, Arc<Mutex<AlwaysListeningController>>>,
) -> Result<(), Error> {
    info!("[Plugin] set_always_listening_wake_words command called with wake words: {:?}", wake_words);

    let mut always_listening_controller = match controller.try_lock() {
        Ok(guard) => guard,
        Err(std::sync::TryLockError::WouldBlock) => {
            return Err(Error::LockError("AlwaysListeningController is busy - please try again".to_string()));
        }
        Err(std::sync::TryLockError::Poisoned(e)) => {
            return Err(Error::LockError(format!("AlwaysListeningController mutex is poisoned: {}", e)));
        }
    };

    always_listening_controller.set_wake_words(wake_words)?;
    Ok(())
}

#[tauri::command]
pub async fn get_always_listening_wake_words(
    controller: State<'_, Arc<Mutex<AlwaysListeningController>>>,
) -> Result<Vec<String>, Error> {
    info!("[Plugin] get_always_listening_wake_words command called");

    let always_listening_controller = match controller.try_lock() {
        Ok(guard) => guard,
        Err(std::sync::TryLockError::WouldBlock) => {
            return Err(Error::LockError("AlwaysListeningController is busy - please try again".to_string()));
        }
        Err(std::sync::TryLockError::Poisoned(e)) => {
            return Err(Error::LockError(format!("AlwaysListeningController mutex is poisoned: {}", e)));
        }
    };

    Ok(always_listening_controller.get_wake_words())
}

// Enhanced Debugging Commands

#[tauri::command]
pub async fn set_transcription_debugging<R: tauri::Runtime>(
    enabled: bool,
    app: AppHandle<R>,
    controller: State<'_, Arc<Mutex<AlwaysListeningController>>>,
) -> Result<(), Error> {
    info!("[Plugin] set_transcription_debugging command called with enabled: {}", enabled);

    let mut always_listening_controller = match controller.try_lock() {
        Ok(guard) => guard,
        Err(std::sync::TryLockError::WouldBlock) => {
            return Err(Error::LockError("AlwaysListeningController is busy - please try again".to_string()));
        }
        Err(std::sync::TryLockError::Poisoned(e)) => {
            return Err(Error::LockError(format!("AlwaysListeningController mutex is poisoned: {}", e)));
        }
    };

    always_listening_controller.set_transcription_debugging(enabled, &app)?;

    info!("[Plugin] Transcription debugging set to: {}", enabled);
    Ok(())
}

#[tauri::command]
pub async fn set_audio_level_monitoring<R: tauri::Runtime>(
    enabled: bool,
    app: AppHandle<R>,
    controller: State<'_, Arc<Mutex<AlwaysListeningController>>>,
) -> Result<(), Error> {
    info!("[Plugin] set_audio_level_monitoring command called with enabled: {}", enabled);

    let mut always_listening_controller = match controller.try_lock() {
        Ok(guard) => guard,
        Err(std::sync::TryLockError::WouldBlock) => {
            return Err(Error::LockError("AlwaysListeningController is busy - please try again".to_string()));
        }
        Err(std::sync::TryLockError::Poisoned(e)) => {
            return Err(Error::LockError(format!("AlwaysListeningController mutex is poisoned: {}", e)));
        }
    };

    always_listening_controller.set_audio_level_monitoring(enabled, &app)?;

    info!("[Plugin] Audio level monitoring set to: {}", enabled);
    Ok(())
}

#[tauri::command]
pub async fn test_whisper_model(
    controller: State<'_, Arc<Mutex<AlwaysListeningController>>>,
) -> Result<serde_json::Value, Error> {
    info!("[Plugin] test_whisper_model command called");

    let always_listening_controller = match controller.try_lock() {
        Ok(guard) => guard,
        Err(std::sync::TryLockError::WouldBlock) => {
            return Err(Error::LockError("AlwaysListeningController is busy - please try again".to_string()));
        }
        Err(std::sync::TryLockError::Poisoned(e)) => {
            return Err(Error::LockError(format!("AlwaysListeningController mutex is poisoned: {}", e)));
        }
    };

    let test_result = always_listening_controller.test_whisper_model()?;

    info!("[Plugin] Whisper model test completed");
    Ok(test_result)
}

#[tauri::command]
pub async fn force_transcription_test<R: tauri::Runtime>(
    app: AppHandle<R>,
    controller: State<'_, Arc<Mutex<AlwaysListeningController>>>,
) -> Result<serde_json::Value, Error> {
    info!("[Plugin] force_transcription_test command called");

    let mut always_listening_controller = match controller.try_lock() {
        Ok(guard) => guard,
        Err(std::sync::TryLockError::WouldBlock) => {
            return Err(Error::LockError("AlwaysListeningController is busy - please try again".to_string()));
        }
        Err(std::sync::TryLockError::Poisoned(e)) => {
            return Err(Error::LockError(format!("AlwaysListeningController mutex is poisoned: {}", e)));
        }
    };

    let test_result = always_listening_controller.force_transcription_test(&app)?;

    info!("[Plugin] Force transcription test completed");
    Ok(test_result)
}

// Microphone Permission Commands

#[tauri::command]
pub async fn check_microphone_permission() -> Result<String, Error> {
    info!("[Plugin] check_microphone_permission command called");
    
    let status = crate::mic_permissions::check_microphone_permission();
    let status_str = match status {
        crate::mic_permissions::MicrophonePermissionStatus::Granted => "granted",
        crate::mic_permissions::MicrophonePermissionStatus::Denied => "denied",
        crate::mic_permissions::MicrophonePermissionStatus::Undetermined => "undetermined",
        crate::mic_permissions::MicrophonePermissionStatus::NotApplicable => "not_applicable",
    };
    
    info!("[Plugin] Microphone permission status: {}", status_str);
    Ok(status_str.to_string())
}

#[tauri::command]
pub async fn request_microphone_permission() -> Result<String, Error> {
    info!("[Plugin] request_microphone_permission command called");
    
    match crate::mic_permissions::request_microphone_permission().await {
        Ok(status) => {
            let status_str = match status {
                crate::mic_permissions::MicrophonePermissionStatus::Granted => "granted",
                crate::mic_permissions::MicrophonePermissionStatus::Denied => "denied",
                crate::mic_permissions::MicrophonePermissionStatus::Undetermined => "undetermined",
                crate::mic_permissions::MicrophonePermissionStatus::NotApplicable => "not_applicable",
            };
            info!("[Plugin] Microphone permission request result: {}", status_str);
            Ok(status_str.to_string())
        }
        Err(e) => {
            error!("[Plugin] Failed to request microphone permission: {}", e);
            Err(Error::Other(format!("Failed to request microphone permission: {}", e)))
        }
    }
}

#[tauri::command]
pub async fn ensure_microphone_ready() -> Result<(), Error> {
    info!("[Plugin] ensure_microphone_ready command called");

    match crate::mic_permissions::ensure_microphone_ready().await {
        Ok(()) => {
            info!("[Plugin] Microphone is ready");
            Ok(())
        }
        Err(e) => {
            error!("[Plugin] Failed to ensure microphone ready: {}", e);
            Err(Error::Other(e))
        }
    }
}

#[tauri::command]
pub fn get_stt_provider() -> Result<String, Error> {
    info!("[Plugin] get_stt_provider called");
    Ok(EngineManager::current_provider_name().to_string())
}

#[tauri::command]
pub async fn set_stt_provider<R: tauri::Runtime>(
    provider: String,
    app: AppHandle<R>,
) -> Result<String, Error> {
    info!("[Plugin] set_stt_provider called with provider: {}", provider);

    let stt_provider: SttProvider = match provider.to_lowercase().as_str() {
        "whisper" => SttProvider::Whisper,
        "parakeet" => SttProvider::Parakeet,
        other => return Err(Error::Other(format!("Unknown STT provider: '{}'", other))),
    };

    // Resolve paths needed for engine initialization
    let config = VoiceTranscriptionConfig::default();
    let whisper_path = resolve_model_path(&app, &config.model_path);
    let parakeet_dir = resolve_model_path(&app, &config.parakeet_model_dir);

    let engine = EngineManager::switch(stt_provider, &whisper_path, Some(&parakeet_dir))
        .map_err(|e| Error::ModelError(format!("Failed to switch STT engine: {}", e)))?;

    let provider_name = engine.name().to_string();
    info!("[Plugin] STT engine switched to '{}'", provider_name);

    // Push new engine to VoiceController
    if let Some(vc_state) = app.try_state::<Arc<Mutex<VoiceController>>>() {
        match vc_state.try_lock() {
            Ok(mut vc) => {
                if let Err(e) = vc.update_engine(engine.clone()) {
                    error!("[Plugin] Failed to update VoiceController engine: {}", e);
                }
            }
            Err(std::sync::TryLockError::WouldBlock) => {
                error!("[Plugin] VoiceController busy - engine swap will take effect on next recording");
            }
            Err(std::sync::TryLockError::Poisoned(e)) => {
                error!("[Plugin] VoiceController mutex poisoned: {}", e);
            }
        }
    }

    // Push new engine to AlwaysListeningController
    if let Some(al_state) = app.try_state::<Arc<Mutex<AlwaysListeningController>>>() {
        match al_state.try_lock() {
            Ok(mut al) => {
                if let Err(e) = al.update_engine(engine) {
                    error!("[Plugin] Failed to update AlwaysListeningController engine: {}", e);
                }
            }
            Err(std::sync::TryLockError::WouldBlock) => {
                error!("[Plugin] AlwaysListeningController busy - engine swap will take effect on next session");
            }
            Err(std::sync::TryLockError::Poisoned(e)) => {
                error!("[Plugin] AlwaysListeningController mutex poisoned: {}", e);
            }
        }
    }

    Ok(provider_name)
}

#[tauri::command]
pub fn get_parakeet_model_status<R: tauri::Runtime>(
    app: AppHandle<R>,
) -> Result<ParakeetModelStatus, Error> {
    info!("[Plugin] get_parakeet_model_status called");
    let config = VoiceTranscriptionConfig::default();
    let parakeet_dir = resolve_model_path(&app, &config.parakeet_model_dir);
    Ok(ParakeetModelStatus::check(std::path::Path::new(&parakeet_dir)))
}

use crate::state::AppState;
use tauri::{State, Emitter, AppHandle, Manager};
use tracing::{info, warn, error};
use std::sync::{Arc, Mutex};

/// Force reset dictation transcription state (emergency cleanup)
#[tauri::command]
pub async fn force_reset_dictation_transcription(
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<String, String> {
    warn!("[Command] force_reset_dictation_transcription called - performing emergency cleanup");

    // Force stop the voice controller with timeout only if it exists
    match app.try_state::<Arc<Mutex<tauri_plugin_voice_transcription::controller::VoiceController>>>() {
        Some(controller_state) => {
            let stop_result = tokio::time::timeout(
                std::time::Duration::from_secs(3),
                tauri_plugin_voice_transcription::commands::stop_dictation(
                    app.clone(),
                    controller_state
                )
            ).await;

            match stop_result {
                Ok(Ok(_)) => {
                    info!("[Command] Voice controller stopped successfully during force reset");
                }
                Ok(Err(e)) => {
                    error!("[Command] Voice controller stop failed during force reset: {}", e);
                }
                Err(_) => {
                    error!("[Command] Voice controller stop timed out during force reset - may be deadlocked");
                }
            }
        }
        None => {
            warn!("[Command] Voice controller not available - skipping voice controller stop during force reset");
        }
    }

    // Reset dictation input monitor state
    crate::dictation_monitor::force_reset_dictation_input_state().await;

    // Force clean up app state
    if let Ok(mut dictation_active) = state.dictation_active.lock() {
        *dictation_active = false;
    } else {
        error!("[Command] Failed to lock Dictation Mode active during force reset");
    }

    // Emit state change events
    if let Err(e) = app.emit("dictation-active", false) {
        error!("[Command] Failed to emit dictation-active event during force reset: {}", e);
    }

    if let Err(e) = app.emit("dictation-transcription-force-cleanup", ()) {
        error!("[Command] Failed to emit dictation-transcription-force-cleanup event during force reset: {}", e);
    }

    info!("[Command] Force reset of dictation transcription completed");
    Ok("Dictation transcription state has been force reset successfully".to_string())
}

/// Get current dictation transcription state for debugging
#[tauri::command]
pub async fn get_dictation_transcription_status(
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<serde_json::Value, String> {
    // Get app state
    let dictation_active = state.dictation_active.lock()
        .map(|active| *active)
        .unwrap_or(false);

    // Get voice controller state
    let voice_controller_active = match app.try_state::<Arc<Mutex<tauri_plugin_voice_transcription::controller::VoiceController>>>() {
        Some(controller_state) => {
            controller_state.lock()
                .map(|controller| controller.is_dictating())
                .unwrap_or(false)
        }
        None => false
    };

    let status = serde_json::json!({
        "dictation_active": dictation_active,
        "voice_controller_active": voice_controller_active,
        "state_consistent": dictation_active == voice_controller_active,
        "timestamp": chrono::Utc::now().to_rfc3339()
    });

    info!("[Command] Dictation transcription status: {}", status);
    Ok(status)
}

/// Emergency cleanup for stuck dictation state (callable from frontend)
#[tauri::command]
pub async fn emergency_cleanup_dictation_state(
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<String, String> {
    warn!("[Command] emergency_cleanup_dictation_state called - performing comprehensive cleanup");

    // Use the comprehensive emergency cleanup function from dictation_monitor
    match crate::dictation_monitor::emergency_cleanup_dictation_state(&app).await {
        Ok(()) => {
            info!("[Command] Emergency cleanup completed successfully");
            Ok("Emergency cleanup completed successfully".to_string())
        }
        Err(e) => {
            error!("[Command] Emergency cleanup failed: {}", e);
            Err(format!("Emergency cleanup failed: {}", e))
        }
    }
}

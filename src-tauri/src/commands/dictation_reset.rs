use crate::state::AppState;
use tauri::{State, Emitter, AppHandle, Manager};
use tracing::{info, warn, error};
use std::sync::{Arc, Mutex};

/// Force reset dictation transcription state (emergency cleanup)
#[tauri::command]
pub async fn force_reset_dictation_transcription(
    app: AppHandle,
) -> Result<String, String> {
    warn!("[Command] force_reset_dictation_transcription called - performing emergency cleanup");

    // Get app state for cleanup
    let state = app.state::<crate::state::AppState>();

    // Force stop any active voice transcription first
    let voice_controller = state.voice_controller.clone();
    if let Ok(mut controller) = voice_controller.try_lock() {
        if let Some(ref mut voice_ctrl) = *controller {
            match voice_ctrl.stop_transcription().await {
                Ok(()) => {
                    info!("[Command] Voice controller stopped successfully during force reset");
                }
                Err(e) => {
                    error!("[Command] Failed to stop voice controller during force reset: {}", e);
                }
            }
        }
    } else {
        warn!("[Command] Could not acquire voice controller lock during force reset");
    }

    // Reset dictation monitor state
    crate::dictation_monitor::force_reset_dictation_state().await;

    // Reset app state flags
    if let Ok(mut dictation_active) = state.dictation_active.lock() {
        *dictation_active = false;
    } else {
        error!("[Command] Failed to lock dictation_active during force reset");
    }

    // Emit cleanup events to frontend
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
) -> Result<String, String> {
    let state = app.state::<crate::state::AppState>();

    let dictation_active = state.dictation_active.lock()
        .map(|active| *active)
        .unwrap_or(false);

    let voice_controller_active = {
        let voice_controller = state.voice_controller.clone();
        if let Ok(controller) = voice_controller.try_lock() {
            controller.as_ref().map(|vc| vc.is_recording()).unwrap_or(false)
        } else {
            false
        }
    };

    let status = serde_json::json!({
        "dictation_active": dictation_active,
        "voice_controller_active": voice_controller_active,
        "state_consistent": dictation_active == voice_controller_active,
    });

    let status_str = status.to_string();
    info!("[Command] Dictation transcription status: {}", status);
    Ok(status_str)
}

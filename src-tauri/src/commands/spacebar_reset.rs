use crate::state::AppState;
use tauri::{State, Emitter, AppHandle, Manager};
use tracing::{info, warn, error};
use std::sync::{Arc, Mutex};

/// Force reset spacebar transcription state (emergency cleanup)
#[tauri::command]
pub async fn force_reset_spacebar_transcription(
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<String, String> {
    warn!("[Command] force_reset_spacebar_transcription called - performing emergency cleanup");

    // Force stop the voice controller with timeout
    let stop_result = tokio::time::timeout(
        std::time::Duration::from_secs(3),
        tauri_plugin_voice_transcription::commands::stop_dictation(
            app.clone(),
            app.state::<Arc<Mutex<tauri_plugin_voice_transcription::controller::VoiceController>>>()
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

    // Reset spacebar monitor state
    crate::spacebar_monitor::force_reset_spacebar_state().await;

    // Force clean up app state
    if let Ok(mut spacebar_active) = state.spacebar_dictation_active.lock() {
        *spacebar_active = false;
    } else {
        error!("[Command] Failed to lock spacebar_dictation_active during force reset");
    }

    // Emit state change events
    if let Err(e) = app.emit("spacebar-dictation-active", false) {
        error!("[Command] Failed to emit spacebar-dictation-active event during force reset: {}", e);
    }

    if let Err(e) = app.emit("spacebar-transcription-force-cleanup", ()) {
        error!("[Command] Failed to emit spacebar-transcription-force-cleanup event during force reset: {}", e);
    }

    info!("[Command] Force reset of spacebar transcription completed");
    Ok("Spacebar transcription state has been force reset successfully".to_string())
}

/// Get current spacebar transcription state for debugging
#[tauri::command]
pub async fn get_spacebar_transcription_status(
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<serde_json::Value, String> {
    // Get app state
    let spacebar_active = state.spacebar_dictation_active.lock()
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
        "spacebar_dictation_active": spacebar_active,
        "voice_controller_active": voice_controller_active,
        "state_consistent": spacebar_active == voice_controller_active,
        "timestamp": chrono::Utc::now().to_rfc3339()
    });

    info!("[Command] Spacebar transcription status: {}", status);
    Ok(status)
}

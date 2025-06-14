use tauri::{AppHandle, State, Manager, Emitter};
use tracing::{info, warn};
use std::sync::{Arc, Mutex};
use crate::state::AppState;
use crate::constants;

/// Stop all ongoing operations - agent execution, dictation, TTS, etc.
/// This function replicates the same functionality as the escape key handler.
#[tauri::command]
pub async fn stop_all_operations(app_handle: AppHandle) -> Result<String, String> {
    info!("[StopOperations] Stop all operations requested from frontend");

    // Stop TTS immediately
    info!("[StopOperations] Stopping TTS audio playback");
    crate::tts::stop_speech();
    info!("[StopOperations] TTS stop_speech() called");

    // Also emit TTS stop event for frontend audio cleanup
    if let Err(e) = app_handle.emit("tts-stop-requested", ()) {
        warn!("Failed to emit TTS stop event: {}", e);
    } else {
        info!("[StopOperations] tts-stop-requested event emitted successfully");
    }

    // Check if agent is active and stop it
    let app_state = app_handle.state::<AppState>();

    // Check if there's an ongoing cancellation already
    let cancel_requested = *app_state.cancel_rx.borrow();
    if !cancel_requested {
        info!("[StopOperations] Stopping active agent task");
        // Use the signal_cancel method instead of direct field access
        app_state.signal_cancel();
    }

    // Force reset agent input state for comprehensive cleanup
    info!("[StopOperations] Force resetting agent input state");
    crate::agent_monitor::force_reset_agent_input_state().await;

    // Check if dictation is active and stop it
    if let Some(voice_controller_state) = app_handle.try_state::<Arc<Mutex<tauri_plugin_voice_transcription::controller::VoiceController>>>() {
        // Check if dictation is active and stop it synchronously if possible
        if let Ok(voice_controller) = voice_controller_state.lock() {
            if voice_controller.is_dictating() {
                info!("[StopOperations] Dictation active - will attempt to stop it");
                drop(voice_controller); // Release the lock before the async operation

                // Instead of spawning, try to stop dictation directly using the app handle
                // This is a simpler approach that avoids lifetime issues
                let _ = app_handle.emit("stop_dictation", serde_json::Value::Null);
            }
        }
    }

    // Force reset dictation input state for comprehensive cleanup
    info!("[StopOperations] Force resetting dictation input state");
    crate::dictation_monitor::force_reset_dictation_input_state().await;

    // Clean up app state flags
    if let Ok(mut dictation_active) = app_state.dictation_active.lock() {
        if *dictation_active {
            info!("[StopOperations] Resetting dictation active flag");
            *dictation_active = false;
        }
    }

    // Mark agent execution as finished for clean state
    app_state.mark_agent_execution_finished();
    info!("[StopOperations] Agent execution marked as finished");

    // Emit agent stopping event for any running AI agents
    if let Err(e) = app_handle.emit(constants::events::AGENT_STOPPING, ()) {
        warn!("[StopOperations Error] Failed to emit {} event: {}", constants::events::AGENT_STOPPING, e);
    }

    // Emit comprehensive agent-stop-all event for broader compatibility
    if let Err(e) = app_handle.emit("agent-stop-all", ()) {
        warn!("[StopOperations Error] Failed to emit agent-stop-all event: {}", e);
    } else {
        info!("[StopOperations] agent-stop-all event emitted successfully");
    }

    // Emit state update events for UI consistency
    if let Err(e) = app_handle.emit("agent-active", false) {
        warn!("[StopOperations Error] Failed to emit agent-active event: {}", e);
    }

    if let Err(e) = app_handle.emit("dictation-active", false) {
        warn!("[StopOperations Error] Failed to emit dictation-active event: {}", e);
    }

    // Immediately signal floating bar manager about cancellation for quick UI feedback
    let app_handle_for_bar = app_handle.clone();
    tauri::async_runtime::spawn(async move {
        crate::commands::floating_bar::handle_backend_response(
            &app_handle_for_bar,
            "Cancelled",
            Some("All operations cancelled by stop button.".to_string())
        ).await;
    });

    info!("[StopOperations] Stop all operations completed successfully");
    Ok("All operations stopped successfully".to_string())
}

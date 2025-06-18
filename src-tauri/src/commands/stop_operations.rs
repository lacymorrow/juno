use tauri::{AppHandle, Manager, Emitter};
use tracing::{info, warn};
use crate::state::AppState;
use crate::constants;

/// Stop all ongoing operations - agent execution, dictation, TTS, always listening, etc.
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

    // Stop dictation using the comprehensive state manager
    info!("[StopOperations] Stopping dictation through dictation state manager");
    if let Err(e) = crate::commands::dictation_state_manager::force_reset_dictation_state(
        app_handle.clone(),
        Some("Stop all operations requested".to_string())
    ).await {
        warn!("[StopOperations] Failed to stop dictation through state manager: {}", e);
    }

    // Stop always listening mode if active
    info!("[StopOperations] Stopping always listening mode");
    if let Err(e) = crate::commands::always_listening::stop_always_listening_mode(
        app_handle.clone(),
        app_state.clone()
    ).await {
        warn!("[StopOperations] Failed to stop always listening mode: {}", e);
    } else {
        info!("[StopOperations] Always listening mode stopped successfully");
    }

    // Mark agent execution as finished for clean state
    app_state.mark_agent_execution_finished();
    info!("[StopOperations] Agent execution marked as finished");

    // Perform comprehensive emergency state cleanup
    info!("[StopOperations] Performing emergency state cleanup");
    if let Err(e) = crate::state_management::handle_emergency_state_cleanup(&app_handle).await {
        warn!("[StopOperations] Failed to perform emergency state cleanup: {}", e);
    }

    // Emit agent stopping event for any running AI agents
    if let Err(e) = app_handle.emit(constants::events::agent::STOPPING, ()) {
        warn!("[StopOperations Error] Failed to emit {} event: {}", constants::events::agent::STOPPING, e);
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

    // Emit always listening state update
    if let Err(e) = app_handle.emit("always-listening-mode-changed", false) {
        warn!("[StopOperations Error] Failed to emit always-listening-mode-changed event: {}", e);
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

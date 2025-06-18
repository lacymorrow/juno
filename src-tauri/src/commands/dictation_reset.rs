use crate::state::AppState;
use crate::commands::dictation_state_manager;
use tauri::{State, Emitter, AppHandle, Manager};
use tracing::{info, warn, error};
use std::sync::{Arc, Mutex};

/// Force reset dictation transcription state (DEPRECATED - use force_reset_dictation_state instead)
#[tauri::command]
pub async fn force_reset_dictation_transcription(
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<String, String> {
    warn!("[Command] force_reset_dictation_transcription called - delegating to new state manager");

    // Use the new centralized state manager
    dictation_state_manager::force_reset_dictation_state(
        app,
        Some("Legacy force reset command called".to_string())
    ).await
}

/// Get current dictation transcription state for debugging (DEPRECATED - use get_dictation_comprehensive_status instead)
#[tauri::command]
pub async fn get_dictation_transcription_status(
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<serde_json::Value, String> {
    warn!("[Command] get_dictation_transcription_status called - delegating to new state manager");

    // Use the new comprehensive status from state manager
    dictation_state_manager::get_dictation_comprehensive_status(app).await
}



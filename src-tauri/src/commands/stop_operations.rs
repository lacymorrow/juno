use tauri::AppHandle;
use tracing::{info, warn};


use crate::commands::stop_coordinator::get_stop_coordinator;

/// Stop all ongoing operations - agent execution, dictation, TTS, always listening, etc.
/// This function now delegates to the centralized stop coordinator to prevent race conditions
#[tauri::command]
pub async fn stop_all_operations(app_handle: AppHandle) -> Result<String, String> {
    info!("[StopOperations] Stop all operations requested from frontend - delegating to coordinator");

    let coordinator = get_stop_coordinator();
    coordinator.stop_all_operations(&app_handle, "Frontend stop button pressed").await
}

/// Legacy function for backward compatibility - delegates to coordinator
#[tauri::command]
pub async fn emergency_stop_all_operations(app_handle: AppHandle) -> Result<String, String> {
    warn!("[StopOperations] Emergency stop requested from frontend - delegating to coordinator");

    let coordinator = get_stop_coordinator();
    coordinator.emergency_stop(&app_handle, "Frontend emergency stop requested").await
}

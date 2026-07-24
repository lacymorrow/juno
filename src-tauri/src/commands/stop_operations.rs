use tauri::AppHandle;
use tracing::info;

use crate::commands::stop_coordinator::get_stop_coordinator;

/// Stop all ongoing operations - agent execution, dictation, TTS, always listening, etc.
/// This function delegates to the centralized stop coordinator to prevent race conditions
#[tauri::command]
pub async fn stop_all_operations(app_handle: AppHandle) -> Result<String, String> {
    info!(
        "[StopOperations] Stop all operations requested from frontend - delegating to coordinator"
    );

    let coordinator = get_stop_coordinator();
    coordinator
        .stop_all_operations(&app_handle, "Frontend stop button pressed")
        .await
}

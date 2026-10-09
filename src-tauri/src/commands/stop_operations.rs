use tauri::AppHandle;
use tracing::info;

use crate::commands::stop_coordinator::get_stop_coordinator;

/// Stop all ongoing operations - agent execution, dictation, TTS, always listening, etc.
/// This function delegates to the centralized stop coordinator to prevent race conditions
#[tauri::command]
pub async fn stop_all_operations(app_handle: AppHandle) -> Result<String, String> {
    // The window of the stop that killed a freshly served local reply and the
    // speech of the chip after it. Escape and "stop" in words do not come
    // through this command, so a real stop is never held back by it.
    if crate::agent::local_intents::replied_within(crate::agent::local_intents::STOP_SETTLE) {
        info!("[StopOperations] Ignored a stop that arrived as a local reply was finishing");
        return Ok("Ignored: a reply was just served".to_string());
    }
    info!(
        "[StopOperations] Stop all operations requested from frontend - delegating to coordinator"
    );

    let coordinator = get_stop_coordinator();
    coordinator
        .stop_all_operations(&app_handle, "Frontend stop button pressed")
        .await
}

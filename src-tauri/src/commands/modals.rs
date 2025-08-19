use tauri::{AppHandle, Manager};
use crate::events::payloads::ErrorToastPayload;

#[tauri::command]
pub async fn show_error_toast(app_handle: AppHandle, payload: ErrorToastPayload) -> Result<(), String> {
    app_handle.emit("show-error-toast", payload)
        .map_err(|e| e.to_string())?;
    Ok(())
}

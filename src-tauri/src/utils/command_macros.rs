//! Utility macros for reducing boilerplate in Tauri commands
//!
//! These macros provide consistent patterns for:
//! - Error handling and logging
//! - Dev tool notifications
//! - Command result processing
//! - State management

use crate::constants::events;
use tauri::{AppHandle, Emitter};

/// Sends a notification to the frontend dev tools
pub fn send_dev_notification(app: &AppHandle, action: &str, message: &str) -> Result<(), String> {
    let payload = serde_json::json!({
        "action": action,
        "message": message
    });
    app.emit(events::dev::TOOL_NOTIFICATION, payload)
        .map_err(|e| format!("Failed to emit dev tool notification: {}", e))
}

/// Macro for creating standardized command functions with automatic error handling and notifications
#[macro_export]
macro_rules! dev_command {
    (
        $(#[$attr:meta])*
        $vis:vis async fn $name:ident(
            $app:ident: AppHandle,
            $state:ident: State<'_, AppState>,
            $($param:ident: $param_type:ty),* $(,)?
        ) -> Result<$return_type:ty, String> {
            action: $action:expr,
            operation: $operation:expr,
            $body:block
        }
    ) => {
        $(#[$attr])*
        #[tauri::command]
        $vis async fn $name(
            $app: AppHandle,
            $state: State<'_, AppState>,
            $($param: $param_type),*
        ) -> Result<$return_type, String> {
            info!("[DEV_TOOL] {} with params: {}", $action, format!($operation, $($param),*));

            let result: Result<$return_type, String> = async move $body.await;

            match &result {
                Ok(_) => {
                    let success_msg = format!("{} successful", $action);
                    if let Err(e) = $crate::utils::command_macros::send_dev_notification(&$app, $action, &success_msg) {
                        error!("[DEV_TOOL] Failed to send success notification: {}", e);
                    }
                }
                Err(e) => {
                    error!("[DEV_TOOL] {} failed: {}", $action, e);
                    let error_msg = format!("{} failed: {}", $action, e);
                    if let Err(notify_err) = $crate::utils::command_macros::send_dev_notification(&$app, $action, &error_msg) {
                        error!("[DEV_TOOL] Failed to send error notification: {}", notify_err);
                    }
                }
            }

            result
        }
    };
}

/// Macro for creating simple state accessor commands
#[macro_export]
macro_rules! state_command {
    (
        $(#[$attr:meta])*
        $vis:vis fn $name:ident(
            $state:ident: State<'_, AppState>
        ) -> Result<$return_type:ty, String> {
            $body:block
        }
    ) => {
        $(#[$attr])*
        #[tauri::command]
        $vis fn $name($state: State<'_, AppState>) -> Result<$return_type, String> {
            $body
        }
    };
}

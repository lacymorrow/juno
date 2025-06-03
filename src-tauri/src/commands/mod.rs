// Main module for all Tauri commands, broken down by category.

use tauri_plugin_notification::NotificationExt;

// Declare the submodules
pub mod app_url;
pub mod core;
pub mod element;
pub mod filesystem;
pub mod keyboard;
pub mod mouse;
pub mod providers;
pub mod shell;
pub mod text_editor;
pub mod window;
pub mod voice_control;
pub mod orchestrator;

// Re-export commands for easy access in lib.rs
pub use self::core::*;
pub use self::mouse::*;
pub use self::shell::*;
pub use self::voice_control::*;
pub use self::orchestrator::*;

// Shared helper function for sending notifications from dev tools
// Needs to be pub(crate) so submodules can access it via super::
pub(crate) fn send_dev_tool_notification(app: &tauri::AppHandle, title: &str, body: &str) -> Result<(), String> {
    app.notification()
        .builder()
        .title(title)
        .body(body)
        .show()
        .map_err(|e| format!("Failed to send notification: {}", e))
}

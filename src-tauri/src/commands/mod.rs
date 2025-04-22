// Main module for all Tauri commands, broken down by category.

use tauri_plugin_notification::NotificationExt;

// Declare the submodules
pub mod app_url;
pub mod core;
pub mod element;
pub mod keyboard;
pub mod mouse;
pub mod shell;
pub mod text_editor;
pub mod window;

// Re-export all command functions for easy access in main.rs - REMOVED as they are pub(crate)
// pub use app_url::*;
// pub use core::*;
// pub use element::*;
// pub use keyboard::*;
// pub use mouse::*;
// pub use shell::*;
// pub use text_editor::*;
// pub use window::*;

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

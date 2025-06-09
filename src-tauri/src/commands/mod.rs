// Main module for all Tauri commands, broken down by category.

use crate::utils::{gather_system_context, format_system_context_for_agent};
use crate::state::AppState;
use tauri::{State, Emitter, AppHandle, WebviewUrl, WebviewWindowBuilder, Manager};
use tracing::warn;

// Declare the submodules
pub mod registry;
pub mod app_url;
pub mod core;
pub mod dictation;
pub mod dictation_reset;
pub mod element;
pub mod filesystem;
pub mod floating_bar;
pub mod keyboard;
pub mod mouse;
pub mod permissions;
pub mod providers;
pub mod shell;
pub mod shortcuts;
pub mod text_editor;
pub mod window;
pub mod orchestrator;
pub mod sound;
pub mod tools;
pub mod cloud;
pub mod mcp;
pub mod memory;
pub mod always_listening;

// Re-export commands for easy access in lib.rs
pub use self::core::*;
pub use self::dictation::*;
pub use self::dictation_reset::{force_reset_dictation_transcription, get_dictation_transcription_status};
pub use self::floating_bar::*;
pub use self::mouse::*;
pub use self::permissions::*;
pub use self::shell::*;
pub use self::shortcuts::*;
pub use self::orchestrator::*;
pub use self::sound::*;
pub use self::tools::*;
pub use self::cloud::*;
pub use self::mcp::*;
pub use self::memory::*;
pub use self::always_listening::*;

// Explicitly re-export tool functions to ensure they're available
pub use self::tools::{
    get_tool_configurations,
    get_tool_config,
    set_tool_enabled,
    set_tool_category_enabled,
    get_enabled_tools,
    is_tool_enabled,
    reset_tool_configuration,
    get_tool_configuration_summary,
};

// Shared helper function for sending notifications from dev tools
// Needs to be pub(crate) so submodules can access it via super::
pub(crate) fn send_dev_tool_notification(
    app: &tauri::AppHandle,
    action: &str,
    message: &str,
) -> Result<(), String> {
    let payload = serde_json::json!({
        "action": action,
        "message": message
    });
    app.emit("dev-tool-notification", payload)
        .map_err(|e| format!("Failed to emit dev tool notification: {}", e))
}

/// Test command to verify system context gathering
#[tauri::command]
pub async fn test_system_context(state: State<'_, AppState>) -> Result<String, String> {
    match gather_system_context(Some(&*state)).await {
        Ok(context) => {
            let formatted = format_system_context_for_agent(&context);
            Ok(formatted)
        }
        Err(e) => Err(format!("Failed to gather system context: {}", e))
    }
}

/// Open the native settings window
#[tauri::command]
pub async fn open_settings_window(app: AppHandle) -> Result<(), String> {
    // Check if settings window already exists
    if let Some(settings_window) = app.get_webview_window("settings") {
        // If it exists, just show and focus it
        settings_window.show().map_err(|e| e.to_string())?;
        settings_window.set_focus().map_err(|e| e.to_string())?;
        return Ok(());
    }

    // Create new settings window if it doesn't exist
    let settings_window = WebviewWindowBuilder::new(
        &app,
        "settings",
        WebviewUrl::App("/settings".into()),
    )
    .title("Juno Settings")
    .inner_size(800.0, 600.0)
    .min_inner_size(700.0, 500.0)
    .resizable(true)
    .center()
    .visible(false) // Start hidden, show after setup
    .build()
    .map_err(|e| e.to_string())?;

    // Apply macOS-specific styling
    #[cfg(target_os = "macos")]
    {
        use tauri::TitleBarStyle;

        // Set transparent title bar for native look
        if let Err(e) = settings_window.set_title_bar_style(TitleBarStyle::Transparent) {
            warn!("Failed to set title bar style: {}", e);
        }
    }

    // Show the window
    settings_window.show().map_err(|e| e.to_string())?;
    settings_window.set_focus().map_err(|e| e.to_string())?;

    Ok(())
}

/// Close the native settings window
#[tauri::command]
pub async fn close_settings_window(app: AppHandle) -> Result<(), String> {
    if let Some(settings_window) = app.get_webview_window("settings") {
        settings_window.hide().map_err(|e| e.to_string())?;
    }
    Ok(())
}

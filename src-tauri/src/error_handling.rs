//! # Error Handling Module
//!
//! This module provides comprehensive error handling for the Juno application,
//! including error types, error processing, recovery mechanisms, graceful degradation,
//! and application-wide error management patterns.

use tauri::{AppHandle, Emitter, Manager};
use tracing::{error, warn, info};
use std::fmt;
use crate::constants::{events, errors::templates};

/// Application-wide error types for better error categorization
#[derive(Debug, Clone)]
pub enum JunoError {
    /// Permission-related errors (accessibility, microphone, etc.)
    PermissionError(String),
    /// Voice transcription and dictation errors
    VoiceError(String),
    /// AI agent execution errors
    AgentError(String),
    /// Window management and UI errors
    WindowError(String),
    /// File system and environment errors
    FileSystemError(String),
    /// Network and cloud connectivity errors
    NetworkError(String),
    /// Configuration and settings errors
    ConfigurationError(String),
    /// System integration errors (desktop automation, shortcuts)
    SystemError(String),
    /// Generic application errors
    ApplicationError(String),
}

impl fmt::Display for JunoError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            JunoError::PermissionError(msg) => write!(f, "Permission Error: {}", msg),
            JunoError::VoiceError(msg) => write!(f, "Voice Error: {}", msg),
            JunoError::AgentError(msg) => write!(f, "Agent Error: {}", msg),
            JunoError::WindowError(msg) => write!(f, "Window Error: {}", msg),
            JunoError::FileSystemError(msg) => write!(f, "File System Error: {}", msg),
            JunoError::NetworkError(msg) => write!(f, "Network Error: {}", msg),
            JunoError::ConfigurationError(msg) => write!(f, "Configuration Error: {}", msg),
            JunoError::SystemError(msg) => write!(f, "System Error: {}", msg),
            JunoError::ApplicationError(msg) => write!(f, "Application Error: {}", msg),
        }
    }
}

impl std::error::Error for JunoError {}

/// Handle application startup errors with graceful degradation
/// This function logs the error and provides user guidance but does NOT exit the process
/// Returns a JunoError that the caller can handle appropriately
pub fn handle_application_startup_error(error: tauri::Error) -> JunoError {
    error!("Error while running tauri application: {}", error);

    // Enhanced user-friendly error messages
    eprintln!("🚨 Juno failed to start properly.");
    eprintln!("");
    eprintln!("This is most commonly due to missing system permissions.");
    eprintln!("Please ensure you have granted the following permissions:");
    eprintln!("");
    eprintln!("📋 Required Permissions:");
    eprintln!("  • Accessibility (System Settings > Privacy & Security > Accessibility)");
    eprintln!("  • Screen Recording (System Settings > Privacy & Security > Screen Recording)");
    eprintln!("  • Microphone (System Settings > Privacy & Security > Microphone)");
    eprintln!("");
    eprintln!("🔄 If permissions are already granted:");
    eprintln!("  • Try restarting the application");
    eprintln!("  • Check if another instance is already running");
    eprintln!("  • Restart your computer if the issue persists");
    eprintln!("");
    eprintln!("🛠️  Technical Details:");
    eprintln!("  Error: {}", error);
    eprintln!("");
    eprintln!("💬 Need help? Visit: https://github.com/juno-ai/issues");

    // Return an error instead of exiting
    JunoError::ApplicationError(format!("Application startup failed: {}", error))
}

/// EMERGENCY ONLY: Exit the process immediately with error code
/// This should ONLY be used in truly unrecoverable situations where graceful shutdown is impossible
/// Consider using handle_application_startup_error() and returning the error instead
pub fn emergency_exit_with_error(error: tauri::Error) -> ! {
    error!("EMERGENCY EXIT: Unrecoverable application error: {}", error);

    eprintln!("🚨 CRITICAL ERROR: Juno encountered an unrecoverable error and must exit.");
    eprintln!("Error: {}", error);
    eprintln!("💬 Please report this at: https://github.com/juno-ai/issues");

    // This is the ONLY remaining acceptable use of std::process::exit in the application
    // It's only for truly unrecoverable errors where graceful shutdown is impossible
    std::process::exit(1);
}

/// Utility functions for common error handling patterns
pub mod utils {
    use super::*;

    /// Format error messages consistently across the application
    pub fn format_error_message(component: &str, operation: &str, error: &str) -> String {
        format!("[{}] {}: {}", component, operation, error)
    }

    /// Log error with appropriate context and emit UI event if needed
    pub fn log_and_emit_error(
        app_handle: &AppHandle,
        component: &str,
        operation: &str,
        error: &str,
        emit_to_ui: bool,
    ) {
        let formatted_error = format_error_message(component, operation, error);
        error!("{}", formatted_error);

        if emit_to_ui {
            let error_payload = serde_json::json!({
                "component": component,
                "operation": operation,
                "error": error,
                "timestamp": chrono::Utc::now().to_rfc3339()
            });

            if let Err(e) = app_handle.emit(events::system::ERROR_OCCURRED, error_payload) {
                error!("{}", format!(templates::FAILED_TO_EMIT, "error event", e));
            }
        }
    }

    /// Handle voice transcription errors with recovery
    pub async fn handle_voice_error(app_handle: &AppHandle, error: &str) {
        log_and_emit_error(app_handle, "VoiceSystem", "transcription", error, true);

        // Attempt voice system recovery
        if let Some(controller_state) = app_handle.try_state::<std::sync::Arc<std::sync::Mutex<tauri_plugin_voice_transcription::controller::VoiceController>>>() {
            let _ = tauri_plugin_voice_transcription::commands::stop_dictation(
                app_handle.clone(),
                controller_state
            ).await;
        }

        // Reset dictation state
        let app_state = app_handle.state::<crate::state::AppState>();
        if let Err(e) = app_state.set_dictation_active(false) {
            error!("{}", format!(templates::FAILED_TO_UPDATE, "dictation state during error recovery", e));
        }

        if let Err(e) = app_handle.emit(crate::constants::events::dictation::ACTIVE, false) {
            error!("{}", format!(templates::FAILED_TO_EMIT, "dictation-active event after error recovery", e));
        }
    }

    /// Handle agent execution errors with recovery
    pub async fn handle_agent_error(app_handle: &AppHandle, error: &str) {
        log_and_emit_error(app_handle, "AgentSystem", "execution", error, true);

        // Stop agent execution
        let app_state = app_handle.state::<crate::state::AppState>();
        app_state.mark_agent_execution_finished();

        // Stop TTS if running
        crate::tts::stop_speech();

        if let Err(e) = app_handle.emit(crate::constants::events::agent::ACTIVE, false) {
            error!("{}", format!(templates::FAILED_TO_EMIT, "agent-active event after error recovery", e));
        }
    }

    /// Handle window management errors gracefully
    pub fn handle_window_error(app_handle: &AppHandle, error: &str) {
        log_and_emit_error(app_handle, "WindowSystem", "management", error, false);

        // Attempt to restore main window if needed
        if let Some(main_window) = app_handle.get_webview_window(crate::constants::window_labels::MAIN) {
            let _ = main_window.show();
            let _ = main_window.set_focus();
        }
    }

    /// Handle permission errors with user guidance
    pub fn handle_permission_error(app_handle: &AppHandle, error: &str) {
        log_and_emit_error(app_handle, "PermissionSystem", "check", error, true);

        // Emit specific permission guidance event
        let guidance_payload = serde_json::json!({
            "type": "permission_error",
            "message": "System permissions required for full functionality",
            "actions": [
                "Check System Settings > Privacy & Security > Accessibility",
                "Check System Settings > Privacy & Security > Screen Recording",
                "Check System Settings > Privacy & Security > Microphone",
                "Restart application after granting permissions"
            ]
        });

        if let Err(e) = app_handle.emit(events::permissions::GUIDANCE_NEEDED, guidance_payload) {
            error!("{}", format!(templates::FAILED_TO_EMIT, "permission guidance event", e));
        }
    }

    /// Safe lock wrapper that logs errors instead of panicking
    pub fn safe_lock<'a, T>(mutex: &'a std::sync::Mutex<T>, operation: &str) -> Result<std::sync::MutexGuard<'a, T>, String> {
        mutex.lock().map_err(|e| {
            let error_msg = format!(templates::FAILED_TO_ACCESS, format!("lock for {}", operation), e);
            error!("{}", error_msg);
            error_msg
        })
    }

    /// Safe async lock wrapper that logs errors instead of panicking
    pub async fn safe_async_lock<'a, T>(mutex: &'a tokio::sync::Mutex<T>, operation: &str) -> tokio::sync::MutexGuard<'a, T> {
        // Tokio mutexes don't poison, so we can always get the lock
        // We just include the operation parameter for logging consistency
        if operation.is_empty() {
            warn!("safe_async_lock called with empty operation name");
        }
        mutex.lock().await
    }

    /// Convert standard errors to JunoError
    pub fn to_juno_error(error: Box<dyn std::error::Error>, category: &str) -> JunoError {
        match category {
            "permission" => JunoError::PermissionError(error.to_string()),
            "voice" => JunoError::VoiceError(error.to_string()),
            "agent" => JunoError::AgentError(error.to_string()),
            "window" => JunoError::WindowError(error.to_string()),
            "filesystem" => JunoError::FileSystemError(error.to_string()),
            "network" => JunoError::NetworkError(error.to_string()),
            "config" => JunoError::ConfigurationError(error.to_string()),
            "system" => JunoError::SystemError(error.to_string()),
            _ => JunoError::ApplicationError(error.to_string()),
        }
    }

    /// Helper for parsing system values with proper error logging
    pub fn safe_parse<T: std::str::FromStr>(value: &str, field_name: &str) -> Option<T> {
        match value.parse() {
            Ok(parsed) => Some(parsed),
            Err(_) => {
                warn!("{}", format!(templates::FAILED_TO_PROCESS, format!("parse {} from value: '{}'", field_name, value), "Invalid format"));
                None
            }
        }
    }
}

/// Test utilities for error handling validation
#[cfg(test)]
pub mod test_utils {
    use super::*;

    /// Test that all error types can be created and displayed
    pub fn test_error_types() {
        let errors = vec![
            JunoError::PermissionError("Test permission error".to_string()),
            JunoError::VoiceError("Test voice error".to_string()),
            JunoError::AgentError("Test agent error".to_string()),
            JunoError::WindowError("Test window error".to_string()),
            JunoError::FileSystemError("Test filesystem error".to_string()),
            JunoError::NetworkError("Test network error".to_string()),
            JunoError::ConfigurationError("Test config error".to_string()),
            JunoError::SystemError("Test system error".to_string()),
            JunoError::ApplicationError("Test app error".to_string()),
        ];

        for error in errors {
            assert!(!error.to_string().is_empty());
            println!("✅ Error type displays correctly: {}", error);
        }
    }

    /// Test error message formatting
    pub fn test_error_formatting() {
        let formatted = utils::format_error_message("TestComponent", "test_operation", "test error");
        assert_eq!(formatted, "[TestComponent] test_operation: test error");
        println!("✅ Error formatting works correctly");
    }
}

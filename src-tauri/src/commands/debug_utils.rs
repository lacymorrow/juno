//! Unified debug system for command enhancement
//!
//! This module provides debug capabilities that can be conditionally enabled
//! in production functions, eliminating the need for separate dev_ wrapper functions.

use tauri::{AppHandle, Emitter};
use tracing::{debug, info, warn};
use serde_json::json;
use std::time::Instant;
use crate::constants::events;

/// Configuration for debug behavior
#[derive(Debug, Clone)]
pub struct DebugConfig {
    pub enabled: bool,
    pub log_operations: bool,
    pub send_notifications: bool,
    pub validate_inputs: bool,
    pub time_operations: bool,
    pub emit_visualizations: bool,
}

impl DebugConfig {
    /// Create debug config based on build mode
    pub fn from_build_mode() -> Self {
        Self {
            enabled: cfg!(debug_assertions),
            log_operations: cfg!(debug_assertions),
            send_notifications: cfg!(debug_assertions),
            validate_inputs: cfg!(debug_assertions),
            time_operations: cfg!(debug_assertions),
            emit_visualizations: cfg!(debug_assertions),
        }
    }

    /// Create debug config for production with minimal overhead
    pub fn production_mode() -> Self {
        Self {
            enabled: false,
            log_operations: false,
            send_notifications: false,
            validate_inputs: false,
            time_operations: false,
            emit_visualizations: false,
        }
    }

    /// Create debug config for development with all features
    pub fn development_mode() -> Self {
        Self {
            enabled: true,
            log_operations: true,
            send_notifications: true,
            validate_inputs: true,
            time_operations: true,
            emit_visualizations: true,
        }
    }
}

/// Debug operation context for tracking and logging
pub struct DebugOperation {
    pub name: String,
    pub start_time: Instant,
    pub config: DebugConfig,
}

impl DebugOperation {
    /// Start a new debug operation
    pub fn start(name: &str, config: DebugConfig) -> Self {
        let op = Self {
            name: name.to_string(),
            start_time: Instant::now(),
            config,
        };

        if op.config.log_operations {
            debug!("[DEBUG] Starting operation: {}", name);
        }

        op
    }

    /// Complete the operation and log timing
    pub fn complete(&self, app_handle: Option<&AppHandle>, success: bool) {
        if !self.config.enabled {
            return;
        }

        let duration = self.start_time.elapsed();

        if self.config.time_operations {
            if success {
                info!("[DEBUG] ✅ {} completed in {:?}", self.name, duration);
            } else {
                warn!("[DEBUG] ❌ {} failed after {:?}", self.name, duration);
            }
        }

        if self.config.send_notifications {
            if let Some(app) = app_handle {
                let status = if success { "completed" } else { "failed" };
                let message = format!("{} {} in {:?}", self.name, status, duration);
                let _ = send_debug_notification(app, &self.name, &message);
            }
        }
    }
}

/// Send a debug notification to the frontend
pub fn send_debug_notification(
    app_handle: &AppHandle,
    action: &str,
    message: &str,
) -> Result<(), String> {
    let payload = json!({
        "action": action,
        "message": message,
        "timestamp": chrono::Utc::now().to_rfc3339(),
        "debug": true
    });

    app_handle
        .emit(events::dev::TOOL_NOTIFICATION, payload)
        .map_err(|e| format!("Failed to emit debug notification: {}", e))
}

/// Log debug operation with context
pub fn log_debug_operation(operation: &str, details: &str, config: &DebugConfig) {
    if config.log_operations {
        debug!("[DEBUG] {}: {}", operation, details);
    }
}

/// Time an operation and return the result with timing info
pub async fn time_operation<T, F, Fut>(
    operation_name: &str,
    config: &DebugConfig,
    operation: F,
) -> T
where
    F: FnOnce() -> Fut,
    Fut: std::future::Future<Output = T>,
{
    let start = Instant::now();
    let result = operation().await;

    if config.time_operations {
        let duration = start.elapsed();
        info!("[DEBUG] ⏱️  {} took {:?}", operation_name, duration);
    }

    result
}

/// Enhanced input validation for debug mode
pub fn validate_debug_input<T>(
    input: &T,
    validation_name: &str,
    config: &DebugConfig,
    validator: impl Fn(&T) -> Result<(), String>,
) -> Result<(), String> {
    if !config.validate_inputs {
        return Ok(());
    }

    match validator(input) {
        Ok(()) => {
            debug!("[DEBUG] ✅ {} validation passed", validation_name);
            Ok(())
        }
        Err(e) => {
            warn!("[DEBUG] ❌ {} validation failed: {}", validation_name, e);
            Err(format!("Debug validation failed for {}: {}", validation_name, e))
        }
    }
}

/// Emit visualization events for debug mode
pub fn emit_debug_visualization(
    app_handle: &AppHandle,
    event_name: &str,
    data: serde_json::Value,
    config: &DebugConfig,
) -> Result<(), String> {
    if !config.emit_visualizations {
        return Ok(());
    }

    app_handle
        .emit(event_name, data)
        .map_err(|e| format!("Failed to emit debug visualization: {}", e))
}

/// Macro for easy debug operation wrapping
#[macro_export]
macro_rules! debug_operation {
    ($config:expr, $name:expr, $app:expr, $body:block) => {{
        let debug_op = $crate::commands::debug_utils::DebugOperation::start($name, $config.clone());
        let result = $body;
        let success = result.is_ok();
        debug_op.complete($app, success);
        result
    }};
}

/// Macro for debug operation wrapping with Anthropic Computer Use API awareness
#[macro_export]
macro_rules! debug_operation_anthropic {
    ($config:expr, $name:expr, $app:expr, $body:block) => {{
        let debug_op = $crate::commands::debug_utils::DebugOperation::start($name, $config.clone());
        let result = $body;
        let success = match &result {
            Ok(output) => {
                // Check if this is an Anthropic error response
                !$crate::agent::tools::anthropic_computer_use::is_anthropic_error_response(output)
            }
            Err(_) => false,
        };
        debug_op.complete($app, success);
        result
    }};
}

/// Common debug validators
pub mod validators {
    /// Validate text input is not empty
    pub fn non_empty_text(text: &str) -> Result<(), String> {
        if text.trim().is_empty() {
            Err("Text cannot be empty".to_string())
        } else {
            Ok(())
        }
    }

    /// Validate coordinates are reasonable
    pub fn valid_coordinates(x: f64, y: f64) -> Result<(), String> {
        if x < crate::constants::mouse::testing::MIN_COORDINATE_VALUE || y < crate::constants::mouse::testing::MIN_COORDINATE_VALUE ||
           x > crate::constants::mouse::testing::MAX_COORDINATE_VALUE || y > crate::constants::mouse::testing::MAX_COORDINATE_VALUE {
            Err(format!("Coordinates ({}, {}) seem unreasonable", x, y))
        } else {
            Ok(())
        }
    }

    /// Validate duration is reasonable
    pub fn reasonable_duration(duration_ms: u64) -> Result<(), String> {
        if duration_ms > 30000 {
            Err(format!("Duration {}ms seems very long", duration_ms))
        } else {
            Ok(())
        }
    }

    /// Validate file path is safe
    pub fn safe_file_path(path: &str) -> Result<(), String> {
        if path.contains("..") || path.starts_with('/') && !path.starts_with("/Users") {
            Err("File path appears unsafe".to_string())
        } else {
            Ok(())
        }
    }

    /// Validate duration in seconds is reasonable
    pub fn valid_duration_seconds(duration_sec: f64) -> Result<(), String> {
        if duration_sec < 0.0 {
            Err("Duration cannot be negative".to_string())
        } else if duration_sec > crate::constants::text::validation::MAX_OPERATION_DURATION_SECONDS { // 60 seconds max
            Err(format!("Duration too long (max {} seconds)", crate::constants::text::validation::MAX_OPERATION_DURATION_SECONDS))
        } else {
            Ok(())
        }
    }

    /// Validate file path is valid and safe
    pub fn valid_file_path(path: &str) -> Result<(), String> {
        use std::path::Path;

        if path.trim().is_empty() {
            return Err("File path cannot be empty".to_string());
        }

        // Basic safety checks
        if path.contains("..") {
            return Err("File path cannot contain '..' for security reasons".to_string());
        }

        // Check if it's a valid path format
        let path_obj = Path::new(path);
        if path_obj.to_string_lossy().is_empty() {
            return Err("Invalid file path format".to_string());
        }

        Ok(())
    }
}

/// Helper function to determine if debug mode should be enabled
pub fn should_enable_debug(debug_mode: bool, state: &crate::state::AppState) -> bool {
    debug_mode || state.is_debug_mode() || cfg!(debug_assertions)
}

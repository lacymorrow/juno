//! Shared debug utilities for consolidated commands
//!
//! This module provides common debug functionality that was previously scattered
//! across dev_ command wrappers. By centralizing these utilities, we ensure
//! consistent debug behavior across all commands while maintaining a single
//! production code path.

use tauri::{AppHandle, State};
use tracing::{debug, info, warn};
use crate::state::AppState;

/// Determines if debug mode should be enabled for a command
///
/// Checks multiple sources in priority order:
/// 1. Explicit debug_mode parameter (highest priority)
/// 2. AppState debug mode setting
/// 3. Build-time debug assertions
/// 4. Environment variable JUNO_DEBUG
pub fn should_enable_debug(
    debug_mode: Option<bool>,
    state: &State<'_, AppState>
) -> bool {
    debug_mode.unwrap_or_else(|| {
        state.is_debug_mode() ||
        cfg!(debug_assertions) ||
        std::env::var("JUNO_DEBUG").is_ok()
    })
}

/// Send a debug notification to the development tools window
pub fn send_debug_notification(
    app: &AppHandle,
    title: &str,
    message: &str,
) -> Result<(), String> {
    // This is the same function that was used in dev_ commands
    super::send_dev_tool_notification(app, title, message)
}

/// Log debug information with consistent formatting
pub fn log_debug_info(operation: &str, details: &str) {
    debug!("[DEBUG] {}: {}", operation, details);
}

/// Log debug operation with consistent formatting (alias for compatibility)
pub fn log_debug_operation(operation: &str, details: &str) {
    info!("[DEBUG] {}: {}", operation, details);
}

/// Calculate elapsed time from start_time and return in milliseconds
pub fn time_operation(start_time: std::time::Instant) -> f64 {
    start_time.elapsed().as_secs_f64() * 1000.0
}

/// Calculate elapsed time with operation name (compatibility overload)
pub fn time_operation_with_name(operation: &str, start_time: std::time::Instant) -> f64 {
    let duration = start_time.elapsed().as_secs_f64() * 1000.0;
    debug!("[TIMING] {} completed in {:.2}ms", operation, duration);
    duration
}

/// Log debug operation start with timing
pub fn log_operation_start(operation: &str, params: &str) {
    info!("[DEBUG] Starting {}: {}", operation, params);
}

/// Log debug operation completion with timing
pub fn log_operation_complete(operation: &str, success: bool, duration_ms: Option<f64>) {
    let status = if success { "SUCCESS" } else { "FAILED" };
    match duration_ms {
        Some(ms) => info!("[DEBUG] Completed {} - {} ({:.2}ms)", operation, status, ms),
        None => info!("[DEBUG] Completed {} - {}", operation, status),
    }
}

/// Validate input parameters with debug logging
pub fn validate_input_with_debug<T>(
    value: &T,
    validator: impl Fn(&T) -> Option<String>,
    operation: &str,
) -> Result<(), String> {
    if let Some(error) = validator(value) {
        warn!("[DEBUG] Validation failed for {}: {}", operation, error);
        Err(error)
    } else {
        debug!("[DEBUG] Input validation passed for {}", operation);
        Ok(())
    }
}

/// Common clipboard validation
pub fn validate_clipboard_content(content: &str) -> Option<String> {
    if content.is_empty() {
        Some("Cannot set empty clipboard content".to_string())
    } else if content.len() > 1_000_000 { // 1MB limit
        Some(format!("Clipboard content too large: {} bytes (max 1MB)", content.len()))
    } else {
        None
    }
}

/// Common text input validation
pub fn validate_text_input(text: &str) -> Option<String> {
    if text.is_empty() {
        Some("Cannot type empty text".to_string())
    } else if text.len() > 10_000 {
        Some(format!("Text input very long: {} characters, this may be slow", text.len()))
    } else {
        None
    }
}

/// Common key validation
pub fn validate_key_input(key: &str) -> Option<String> {
    if key.trim().is_empty() {
        Some("Cannot use empty/whitespace key".to_string())
    } else {
        None
    }
}

/// Common coordinate validation
pub fn validate_coordinates(x: f64, y: f64) -> Option<String> {
    if x < 0.0 || y < 0.0 {
        Some(format!("Invalid coordinates: ({}, {}) - coordinates cannot be negative", x, y))
    } else if x > 10000.0 || y > 10000.0 {
        Some(format!("Suspicious coordinates: ({}, {}) - very large values", x, y))
    } else {
        None
    }
}

/// Common duration validation
pub fn validate_duration(duration_ms: Option<u64>) -> Option<String> {
    if let Some(duration) = duration_ms {
        if duration > 300_000 { // 5 minutes
            Some(format!("Very long duration: {}ms ({}s) - this may cause issues",
                duration, duration / 1000))
        } else {
            None
        }
    } else {
        None
    }
}

/// Performance timing helper
pub struct DebugTimer {
    operation: String,
    start_time: std::time::Instant,
}

impl DebugTimer {
    pub fn start(operation: &str) -> Self {
        let timer = Self {
            operation: operation.to_string(),
            start_time: std::time::Instant::now(),
        };
        debug!("[PERF] Starting {}", operation);
        timer
    }

    pub fn elapsed_ms(&self) -> f64 {
        self.start_time.elapsed().as_secs_f64() * 1000.0
    }

    pub fn finish(self, success: bool) {
        let duration_ms = self.elapsed_ms();
        log_operation_complete(&self.operation, success, Some(duration_ms));
    }

    pub fn finish_with_result<T, E>(self, result: &Result<T, E>) {
        self.finish(result.is_ok());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_clipboard_validation() {
        assert!(validate_clipboard_content("").is_some());
        assert!(validate_clipboard_content("valid content").is_none());

        let large_content = "x".repeat(2_000_000);
        assert!(validate_clipboard_content(&large_content).is_some());
    }

    #[test]
    fn test_text_input_validation() {
        assert!(validate_text_input("").is_some());
        assert!(validate_text_input("valid text").is_none());

        let long_text = "x".repeat(15_000);
        assert!(validate_text_input(&long_text).is_some());
    }

    #[test]
    fn test_coordinate_validation() {
        assert!(validate_coordinates(-1.0, 5.0).is_some());
        assert!(validate_coordinates(5.0, -1.0).is_some());
        assert!(validate_coordinates(100.0, 200.0).is_none());
        assert!(validate_coordinates(15000.0, 200.0).is_some());
    }

    #[test]
    fn test_duration_validation() {
        assert!(validate_duration(None).is_none());
        assert!(validate_duration(Some(1000)).is_none());
        assert!(validate_duration(Some(400_000)).is_some());
    }

    #[test]
    fn test_debug_timer() {
        let timer = DebugTimer::start("test_operation");
        std::thread::sleep(std::time::Duration::from_millis(1));
        assert!(timer.elapsed_ms() >= 1.0);
        timer.finish(true);
    }
}

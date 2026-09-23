//! Unified debug system for command enhancement
//!
//! This module provides debug capabilities that can be conditionally enabled
//! in production functions, eliminating the need for separate dev_ wrapper functions.

use crate::constants::events;
use serde_json::json;
use std::time::Instant;
use tauri::{AppHandle, Emitter};
use tracing::{debug, info, warn};

/// Configuration for debug behavior.
///
/// This config controls dev-only telemetry: logging, notifications, timing,
/// and visualization events. Every flag here may be (and in release builds is)
/// turned off wholesale, so NOTHING security- or correctness-critical may ever
/// be gated on it.
///
/// History (LAC-4004 / LAC-4013): this struct used to carry a
/// `validate_inputs` flag that gated input validation in every command
/// handler. Because the factories below disable all flags together in release,
/// that silently disabled load-bearing checks (bash timeout bounds, wait
/// duration caps, file-path checks) in production builds. Input validation now
/// runs unconditionally inside each command (or via
/// `agent::tools::path_security`). Do not re-introduce a validation flag here.
#[derive(Debug, Clone)]
pub struct DebugConfig {
    pub enabled: bool,
    pub log_operations: bool,
    pub send_notifications: bool,
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

/// Input validators.
///
/// Despite living in `debug_utils`, these run UNCONDITIONALLY at their call
/// sites — they are enforcement, not diagnostics (LAC-4013). Do not gate calls
/// to them on `DebugConfig` or build mode.
pub mod validators {
    /// Validate text input is not empty
    pub fn non_empty_text(text: &str) -> Result<(), String> {
        if text.trim().is_empty() {
            Err("Text cannot be empty".to_string())
        } else {
            Ok(())
        }
    }

    /// Validate a `hold_key` duration, in milliseconds.
    ///
    /// The cap is `MAX_HOLD_KEY_MS`, the same 300-second ceiling
    /// `resolve_hold_key_duration_ms` clamps the agent path to, so the two
    /// entry points into `hold_key` agree on how long a key may be held.
    /// They used to disagree: this one rejected anything over 30_000ms while
    /// the agent path happily clamped to 300_000ms, so the same request was
    /// legal or not depending on which door it came through. It also used to
    /// depend on the build; LAC-4013 made this validator unconditional.
    ///
    /// Named for `hold_key` rather than "duration" so the 300-second ceiling
    /// cannot be borrowed for an unrelated value that has no business being
    /// five minutes long.
    pub fn reasonable_hold_key_duration_ms(duration_ms: u64) -> Result<(), String> {
        let max_ms = crate::agent::tools::anthropic_computer_use::MAX_HOLD_KEY_MS;
        if duration_ms > max_ms {
            Err(format!(
                "hold_key duration {}ms exceeds the {}ms maximum",
                duration_ms, max_ms
            ))
        } else {
            Ok(())
        }
    }

    /// Validate duration in seconds is within the allowed cap.
    ///
    /// Load-bearing: `core::wait` is reachable from agent tool calls and this
    /// cap is the only bound on how long a single call can sleep.
    pub fn valid_duration_seconds(duration_sec: f64) -> Result<(), String> {
        if !duration_sec.is_finite() {
            Err("Duration must be a finite number".to_string())
        } else if duration_sec < 0.0 {
            Err("Duration cannot be negative".to_string())
        } else if duration_sec > crate::constants::text::validation::MAX_OPERATION_DURATION_SECONDS
        {
            // 60 seconds max
            Err(format!(
                "Duration too long (max {} seconds)",
                crate::constants::text::validation::MAX_OPERATION_DURATION_SECONDS
            ))
        } else {
            Ok(())
        }
    }

    /// Validate file path is valid and safe.
    ///
    /// Interim enforcement: rejects empty paths and `..` traversal. LAC-4013
    /// Fix B replaces the command-level call sites with
    /// `agent::tools::path_security::resolve_within_default_roots` once the
    /// allowed-roots product decision lands. Until then this is the only path
    /// check on the `commands/filesystem.rs` and `commands/text_editor.rs`
    /// surfaces — keep it unconditional.
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

#[cfg(test)]
mod hold_key_cap_tests {
    use super::validators::reasonable_hold_key_duration_ms;
    use crate::agent::tools::anthropic_computer_use::{
        resolve_hold_key_duration_ms, MAX_HOLD_KEY_MS,
    };
    use serde_json::json;

    #[test]
    fn the_validator_and_the_agent_path_share_one_ceiling() {
        // The whole point: whatever the agent path clamps to must be a value
        // this validator accepts. Two caps meant the same request was legal or
        // not depending on which entry point it arrived through.
        let clamped = resolve_hold_key_duration_ms(&json!({ "duration": 9_999 }));
        assert_eq!(clamped, Ok(MAX_HOLD_KEY_MS));
        assert!(reasonable_hold_key_duration_ms(MAX_HOLD_KEY_MS).is_ok());
    }

    #[test]
    fn a_duration_the_old_cap_rejected_is_now_accepted() {
        // 60 seconds: over the old bare 30_000ms limit, well inside the
        // 300-second ceiling Anthropic's computer tool actually allows.
        assert!(reasonable_hold_key_duration_ms(60_000).is_ok());
    }

    #[test]
    fn a_duration_past_the_shared_ceiling_is_rejected() {
        let error = reasonable_hold_key_duration_ms(MAX_HOLD_KEY_MS + 1)
            .expect_err("a hold longer than the cap must be rejected");
        // The message names the unit and the bound, not just "seems very long".
        assert!(error.contains("300000ms"), "unexpected message: {}", error);
    }
}

#[cfg(test)]
mod tests {
    use super::validators::{non_empty_text, valid_duration_seconds, valid_file_path};

    // Regression tests for LAC-4013: these validators are load-bearing and run
    // unconditionally in release builds. Exercise the boundary values.

    #[test]
    fn duration_accepts_zero_and_cap() {
        assert!(valid_duration_seconds(0.0).is_ok());
        assert!(valid_duration_seconds(1.5).is_ok());
        assert!(valid_duration_seconds(
            crate::constants::text::validation::MAX_OPERATION_DURATION_SECONDS
        )
        .is_ok());
    }

    #[test]
    fn duration_rejects_out_of_bounds() {
        assert!(valid_duration_seconds(-0.001).is_err());
        assert!(valid_duration_seconds(
            crate::constants::text::validation::MAX_OPERATION_DURATION_SECONDS + 0.001
        )
        .is_err());
        // The worker-starvation vector from LAC-4004: enormous durations.
        assert!(valid_duration_seconds(1e12).is_err());
    }

    #[test]
    fn duration_rejects_non_finite() {
        assert!(valid_duration_seconds(f64::NAN).is_err());
        assert!(valid_duration_seconds(f64::INFINITY).is_err());
        assert!(valid_duration_seconds(f64::NEG_INFINITY).is_err());
    }

    #[test]
    fn file_path_rejects_empty_and_traversal() {
        assert!(valid_file_path("").is_err());
        assert!(valid_file_path("   ").is_err());
        assert!(valid_file_path("/tmp/../etc/passwd").is_err());
        assert!(valid_file_path("../secrets").is_err());
    }

    #[test]
    fn file_path_accepts_normal_paths() {
        assert!(valid_file_path("/Users/someone/Documents/notes.txt").is_ok());
        assert!(valid_file_path("relative/dir/file.rs").is_ok());
    }

    #[test]
    fn non_empty_text_boundaries() {
        assert!(non_empty_text("").is_err());
        assert!(non_empty_text("  \t").is_err());
        assert!(non_empty_text("x").is_ok());
    }
}

use chrono::{DateTime, Local};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use tracing::{debug, error, info, warn};

/// Log grouping configuration
#[derive(Debug, Clone)]
pub struct LogGroupingConfig {
    /// Show timestamp if this many minutes have passed since last entry
    pub time_threshold_minutes: i64,
    /// Show timestamp if this many events have occurred since last timestamp
    pub event_threshold: usize,
    /// Always show timestamp on first entry of a session
    pub show_first_timestamp: bool,
    /// Time format for display (12h or 24h)
    pub use_24h_format: bool,
}

impl Default for LogGroupingConfig {
    fn default() -> Self {
        Self {
            time_threshold_minutes: 5,
            event_threshold: 10,
            show_first_timestamp: true,
            use_24h_format: false,
        }
    }
}

/// Log group tracker for managing timestamp display
#[derive(Debug)]
struct LogGroupTracker {
    last_timestamp_shown: Option<u64>,
    events_since_last_timestamp: usize,
    config: LogGroupingConfig,
}

impl LogGroupTracker {
    fn new(config: LogGroupingConfig) -> Self {
        Self {
            last_timestamp_shown: None,
            events_since_last_timestamp: 0,
            config,
        }
    }

    fn should_show_timestamp(&self, current_timestamp: u64) -> bool {
        // Always show first timestamp
        if self.last_timestamp_shown.is_none() && self.config.show_first_timestamp {
            return true;
        }

        // Check event threshold
        if self.events_since_last_timestamp >= self.config.event_threshold {
            return true;
        }

        // Check time threshold
        if let Some(last_ts) = self.last_timestamp_shown {
            let time_diff_minutes = (current_timestamp.saturating_sub(last_ts)) / (1000 * 60);
            if time_diff_minutes >= self.config.time_threshold_minutes as u64 {
                return true;
            }
        }

        false
    }

    fn record_event(&mut self, timestamp: u64, showed_timestamp: bool) {
        if showed_timestamp {
            self.last_timestamp_shown = Some(timestamp);
            self.events_since_last_timestamp = 0;
        } else {
            self.events_since_last_timestamp += 1;
        }
    }
}

// Global log formatter instance
lazy_static::lazy_static! {
    static ref LOG_TRACKERS: Arc<Mutex<HashMap<String, LogGroupTracker>>> =
        Arc::new(Mutex::new(HashMap::new()));
}

/// Format timestamp for display
fn format_timestamp(timestamp_ms: u64, use_24h: bool) -> String {
    let timestamp_secs = timestamp_ms / 1000;
    let dt = match DateTime::from_timestamp(timestamp_secs as i64, 0) {
        Some(utc_dt) => utc_dt.with_timezone(&Local),
        None => return "Invalid time".to_string(),
    };

    if use_24h {
        dt.format("%H:%M").to_string()
    } else {
        dt.format("%l:%M %p").to_string().trim().to_string()
    }
}

/// Enhanced logging function with Slack/Apple Messages style grouping
pub fn log_with_grouping(level: &str, component: &str, message: &str, timestamp_ms: Option<u64>) {
    let timestamp = timestamp_ms.unwrap_or_else(|| {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64
    });

    let config = LogGroupingConfig::default();
    let tracker_key = component.to_string();

    let should_show_timestamp = {
        match LOG_TRACKERS.lock() {
            Ok(mut trackers) => {
                let tracker = trackers
                    .entry(tracker_key.clone())
                    .or_insert_with(|| LogGroupTracker::new(config.clone()));

                let show = tracker.should_show_timestamp(timestamp);
                tracker.record_event(timestamp, show);
                show
            }
            Err(e) => {
                // If the mutex is poisoned, log the error and fallback to showing timestamp
                eprintln!(
                    "Log tracker mutex poisoned: {}. Showing timestamp as fallback.",
                    e
                );
                true // Safe fallback - show timestamp when uncertain
            }
        }
    };

    // Format the log message
    let formatted_message = if should_show_timestamp {
        let time_str = format_timestamp(timestamp, config.use_24h_format);
        format!(
            "\n⏰ {} - {}\n🔧 {}: {}",
            time_str,
            component,
            level.to_uppercase(),
            message
        )
    } else {
        format!("   🔧 {}: {}", level.to_uppercase(), message)
    };

    // Log based on level
    match level.to_lowercase().as_str() {
        "error" => error!("{}", formatted_message),
        "warn" => warn!("{}", formatted_message),
        "info" => info!("{}", formatted_message),
        "debug" => debug!("{}", formatted_message),
        _ => info!("{}", formatted_message),
    }
}

/// Log tool execution start with enhanced formatting
pub fn log_tool_start(tool_name: &str, args: Option<&str>) {
    let message = if let Some(args_str) = args {
        format!("Tool execution started: {} (args: {})", tool_name, args_str)
    } else {
        format!("Tool execution started: {}", tool_name)
    };
    log_with_grouping("info", "ToolExecution", &message, None);
}

/// Log tool execution completion with enhanced formatting
pub fn log_tool_complete(tool_name: &str, success: bool, duration_ms: Option<u64>) {
    let status_icon = if success { "✅" } else { "❌" };
    let duration_str = duration_ms
        .map(|d| format!(" ({}ms)", d))
        .unwrap_or_default();

    let message = format!(
        "{} Tool '{}' {}{}",
        status_icon,
        tool_name,
        if success {
            "completed successfully"
        } else {
            "failed"
        },
        duration_str
    );

    let level = if success { "info" } else { "warn" };
    log_with_grouping(level, "ToolExecution", &message, None);
}

/// Log agent action with enhanced formatting
pub fn log_agent_action(action: &str, details: Option<&str>) {
    let message = if let Some(details_str) = details {
        format!("🤖 {}: {}", action, details_str)
    } else {
        format!("🤖 {}", action)
    };
    log_with_grouping("info", "Agent", &message, None);
}

/// Reset grouping state for a component (useful for new sessions)
pub fn reset_grouping(component: &str) {
    match LOG_TRACKERS.lock() {
        Ok(mut trackers) => {
            trackers.remove(component);
        }
        Err(e) => {
            eprintln!(
                "Failed to reset log grouping for '{}': {}. Continuing without reset.",
                component, e
            );
        }
    }
}

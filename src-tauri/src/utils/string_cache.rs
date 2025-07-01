use std::collections::HashMap;
use std::sync::{Arc, RwLock};
use once_cell::sync::Lazy;

/// String interning cache for reducing format! allocations
static STRING_CACHE: Lazy<Arc<RwLock<HashMap<String, Arc<str>>>>> =
    Lazy::new(|| Arc::new(RwLock::new(HashMap::new())));

/// Error message templates for common patterns
pub struct ErrorTemplates;

impl ErrorTemplates {
    pub const LOCK_FAILED: &'static str = "Failed to lock";
    pub const EMIT_FAILED: &'static str = "Failed to emit event";
    pub const ACCESS_FAILED: &'static str = "Failed to access";
    pub const PARSE_FAILED: &'static str = "Failed to parse";
    pub const EXECUTE_FAILED: &'static str = "Failed to execute";
}

/// High-performance string cache for reducing allocations
pub struct StringCache;

impl StringCache {
    /// Get or create an error message with context
    pub fn get_error(template: &'static str, context: &str) -> String {
        let key = format!("{}:{}", template, context);

        // Fast path: check if already cached
        if let Ok(cache) = STRING_CACHE.read() {
            if let Some(cached) = cache.get(&key) {
                return cached.to_string();
            }
        }

        // Slow path: create and cache
        let formatted = format!("{}: {}", template, context);
        if let Ok(mut cache) = STRING_CACHE.write() {
            // Limit cache size to prevent memory leaks
            if cache.len() < 1000 {
                cache.insert(key, formatted.clone().into());
            }
        }

        formatted
    }

    /// Get formatted message for logging
    pub fn get_log_message(level: &'static str, component: &str, message: &str) -> String {
        format!("[{}] {}: {}", level, component, message)
    }

    /// Pre-warm cache with common error patterns
    pub fn initialize() {
        let common_errors = vec![
            ("voice_controller_lock", ErrorTemplates::LOCK_FAILED),
            ("app_handle_clone", ErrorTemplates::ACCESS_FAILED),
            ("event_emit", ErrorTemplates::EMIT_FAILED),
            ("json_parse", ErrorTemplates::PARSE_FAILED),
            ("tool_execute", ErrorTemplates::EXECUTE_FAILED),
        ];

        if let Ok(mut cache) = STRING_CACHE.write() {
            for (context, template) in common_errors {
                let key = format!("{}:{}", template, context);
                let value = format!("{}: {}", template, context);
                cache.insert(key, value.into());
            }
        }
    }
}

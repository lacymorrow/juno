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
    /// Returns Arc<str> to avoid allocation on retrieval
    ///
    /// Note: Still allocates for cache key construction, but avoids allocation
    /// on cache hits and eliminates String conversion from cached Arc<str>
    pub fn get_error(template: &'static str, context: &str) -> Arc<str> {
        let key = format!("{}|{}", template, context);

        // Fast path: check if already cached
        if let Ok(cache) = STRING_CACHE.read() {
            if let Some(cached) = cache.get(&key) {
                return Arc::clone(cached);
            }
        }

        // Slow path: create and cache
        let formatted = format!("{}: {}", template, context);
        let arc_str: Arc<str> = formatted.into();

        if let Ok(mut cache) = STRING_CACHE.write() {
            // Limit cache size to prevent memory leaks
            if cache.len() < 1000 {
                cache.insert(key, Arc::clone(&arc_str));
            }
        }

        arc_str
    }

    /// Get cached error message for common template patterns
    /// This is the main optimization function - replace format!() calls with this
    pub fn get_template_error(template: &'static str, context: &str, error: impl std::fmt::Display) -> String {
        let key = format!("{}|{}|{}", template, context, error);

        // Fast path: check if already cached
        if let Ok(cache) = STRING_CACHE.read() {
            if let Some(cached) = cache.get(&key) {
                return cached.to_string(); // Arc<str> -> String for compatibility
            }
        }

        // Slow path: create using template and cache
        let formatted = format!("{} {}: {}", template, context, error);
        let arc_str: Arc<str> = formatted.clone().into();

        if let Ok(mut cache) = STRING_CACHE.write() {
            // Limit cache size to prevent memory leaks
            if cache.len() < 2000 { // Increased for template errors
                cache.insert(key, arc_str);
            }
        }

        formatted
    }

    /// Get formatted message for logging
    pub fn get_log_message(level: &'static str, component: &str, message: &str) -> String {
        format!("[{}] {}: {}", level, component, message)
    }

    /// Pre-warm cache with common error patterns from the codebase
    pub fn initialize() {
        let common_errors = vec![
            // Voice/Audio patterns
            ("voice_controller_lock", ErrorTemplates::LOCK_FAILED),
            ("TTS provider", ErrorTemplates::ACCESS_FAILED),
            ("voice start sound", "Failed to play"),
            ("voice transcription", ErrorTemplates::PARSE_FAILED),

            // Agent/Brain patterns
            ("agent brain", ErrorTemplates::ACCESS_FAILED),
            ("Computer Use tools", "Failed to register"),
            ("single agent brain", "Failed to initialize"),
            ("orchestrator brain", "Failed to initialize"),
            ("tool execution", ErrorTemplates::EXECUTE_FAILED),

            // State management patterns
            ("dictation active", "Failed to get"),
            ("dictation active", "Failed to set"),
            ("always listening active", "Failed to get"),
            ("always listening active", "Failed to set"),
            ("TTS provider", "Failed to get"),
            ("TTS provider", "Failed to set"),
            ("sound enabled", "Failed to get"),
            ("sound enabled", "Failed to set"),
            ("debug mode", "Failed to get"),
            ("debug mode", "Failed to set"),

            // MCP/Integration patterns
            ("MCP servers", "Failed to start"),
            ("cloud client", "Failed to create"),
            ("cloud client", "Failed to start"),
            ("settings manager", "Failed to create"),
            ("event emit", ErrorTemplates::EMIT_FAILED),

            // File/IO patterns
            ("temporary file", "Failed to create"),
            ("JSON parsing", ErrorTemplates::PARSE_FAILED),
            ("settings JSON", "Failed to save"),
            ("window list JSON", ErrorTemplates::PARSE_FAILED),
        ];

        if let Ok(mut cache) = STRING_CACHE.write() {
            for (context, template) in common_errors {
                let key = format!("{}|{}", template, context);
                let value = format!("{}: {}", template, context);
                cache.insert(key, value.into());
            }
        }
    }

    /// Get cache statistics for performance monitoring
    pub fn get_stats() -> (usize, usize) {
        if let Ok(cache) = STRING_CACHE.read() {
            (cache.len(), cache.capacity())
        } else {
            (0, 0)
        }
    }

    /// Clear cache (useful for testing or memory management)
    pub fn clear() {
        if let Ok(mut cache) = STRING_CACHE.write() {
            cache.clear();
        }
    }
}

/// Convenience function for formatting error messages using templates
/// This provides the same interface as the existing format_error functions
/// but uses the string cache for performance optimization
pub fn format_error_cached(template: &'static str, context: &str, error: impl std::fmt::Display) -> String {
    StringCache::get_template_error(template, context, error)
}

/// Convenience macro to replace format!() calls with cached versions
/// Usage: cached_format!("Failed to {}: {}", "operation", error)
#[macro_export]
macro_rules! cached_format {
    ($template:expr, $context:expr, $error:expr) => {
        $crate::utils::string_cache::StringCache::get_template_error($template, $context, $error)
    };
}

/// Initialize the string cache - should be called at application startup
pub fn initialize_string_cache() {
    log::info!("🚀 Initializing string cache for performance optimization...");
    StringCache::initialize();
    let (count, capacity) = StringCache::get_stats();
    log::info!("✅ String cache initialized with {} pre-warmed entries (capacity: {})", count, capacity);
}

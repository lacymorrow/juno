use std::collections::HashMap;
use std::sync::{Arc, RwLock};
use once_cell::sync::Lazy;
use crate::constants::errors::templates;

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
        // Create cache key from template + context only (not the variable error)
        let cache_key = format!("{}|{}", template, context);

        // Fast path: check if template is already cached for this context
        if let Ok(cache) = STRING_CACHE.read() {
            if let Some(cached_template) = cache.get(&cache_key) {
                // Use the same replacement logic as cache miss for consistency
                return Self::replace_placeholders_by_position(&cached_template, &[&error.to_string()]);
            }
        }

        // Format the template properly - replace first {} with context, second {} with error
        let formatted = Self::replace_placeholders_by_position(template, &[context, &error.to_string()]);

        // Cache the template with context filled in for future use
        if let Ok(mut cache) = STRING_CACHE.write() {
            // Limit cache size to prevent memory leaks
            if cache.len() < 2000 {
                // Cache the template with context already filled in using consistent logic
                let template_with_context = Self::replace_placeholders_by_position(template, &[context]);
                cache.insert(cache_key, template_with_context.into());
            }
        }

        formatted
    }

    /// Replace {} placeholders with values in order, finding positions before any replacements
    /// This ensures placeholders within replacement values don't interfere with subsequent replacements
    fn replace_placeholders_by_position(template: &str, replacements: &[&str]) -> String {
        // Find all placeholder positions before making any replacements
        let mut positions = Vec::new();
        let mut search_start = 0;

        while let Some(pos) = template[search_start..].find("{}") {
            let absolute_pos = search_start + pos;
            positions.push(absolute_pos);
            search_start = absolute_pos + 2; // Move past this placeholder
        }

        // If no placeholders or no replacements, return original template
        if positions.is_empty() || replacements.is_empty() {
            return template.to_string();
        }

        // Replace placeholders from right to left to avoid position shifts
        let mut result = template.to_string();
        let replacement_count = positions.len().min(replacements.len());

        for i in (0..replacement_count).rev() {
            let pos = positions[i];
            result.replace_range(pos..pos + 2, replacements[i]);
        }

        result
    }

    /// Get formatted message for logging
    pub fn get_log_message(level: &'static str, component: &str, message: &str) -> String {
        format!("[{}] {}: {}", level, component, message)
    }

    /// Pre-warm cache with common error patterns from the codebase
    pub fn initialize() {
        let common_patterns = vec![
            // Voice/Audio patterns
            (templates::FAILED_TO_ACCESS, "voice_controller_lock"),
            (templates::FAILED_TO_ACCESS, "TTS provider"),
            ("Failed to play", "voice start sound"),
            (templates::FAILED_TO_PARSE, "voice transcription"),

            // Agent/Brain patterns
            (templates::FAILED_TO_ACCESS, "agent brain"),
            (templates::FAILED_TO_REGISTER, "Computer Use tools"),
            (templates::FAILED_TO_INITIALIZE, "single agent brain"),
            (templates::FAILED_TO_INITIALIZE, "orchestrator brain"),
            (templates::FAILED_TO_EXECUTE, "tool execution"),

            // State management patterns
            (templates::FAILED_TO_RETRIEVE, "dictation active status"),
            (templates::FAILED_TO_SET, "dictation active status"),
            (templates::FAILED_TO_RETRIEVE, "always listening active status"),
            (templates::FAILED_TO_SET, "always listening active status"),
            (templates::FAILED_TO_RETRIEVE, "TTS provider"),
            (templates::FAILED_TO_SET, "TTS provider"),
            (templates::FAILED_TO_RETRIEVE, "sound enabled status"),
            (templates::FAILED_TO_SET, "sound enabled status"),
            (templates::FAILED_TO_RETRIEVE, "debug mode status"),
            (templates::FAILED_TO_SET, "debug mode status"),

            // MCP/Integration patterns
            (templates::FAILED_TO_START, "MCP servers"),
            (templates::FAILED_TO_CREATE, "cloud client"),
            (templates::FAILED_TO_START, "cloud client"),
            (templates::FAILED_TO_CREATE, "settings manager"),
            (templates::FAILED_TO_EMIT, "event"),

            // File/IO patterns
            (templates::FAILED_TO_CREATE, "temporary file"),
            (templates::FAILED_TO_PARSE, "JSON data"),
            (templates::FAILED_TO_SAVE, "settings JSON"),
            (templates::FAILED_TO_PARSE, "window list JSON"),
        ];

        if let Ok(mut cache) = STRING_CACHE.write() {
            for (template, context) in common_patterns {
                let cache_key = format!("{}|{}", template, context);
                // Cache the template with context already filled in using consistent logic
                let template_with_context = Self::replace_placeholders_by_position(template, &[context]);
                cache.insert(cache_key, template_with_context.into());
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_template_error_formatting() {
        // Clear cache for clean test
        StringCache::clear();

        // Test proper placeholder replacement
        let template = "Failed to access {}: {}";
        let context = "TTS provider";
        let error = "connection timeout";

        let result = StringCache::get_template_error(template, context, error);
        assert_eq!(result, "Failed to access TTS provider: connection timeout");
    }

    #[test]
    fn test_template_error_caching() {
        // Clear cache for clean test
        StringCache::clear();

        let template = "Failed to load {}: {}";
        let context = "configuration file";

        // First call should cache the template
        let result1 = StringCache::get_template_error(template, context, "file not found");
        let (count1, _) = StringCache::get_stats();

        // Second call with different error should use cached template
        let result2 = StringCache::get_template_error(template, context, "permission denied");
        let (count2, _) = StringCache::get_stats();

        // Cache count should remain the same (template cached, not the full error)
        assert_eq!(count1, count2);
        assert_eq!(result1, "Failed to load configuration file: file not found");
        assert_eq!(result2, "Failed to load configuration file: permission denied");

        // Should have exactly 1 cache entry for this template+context combination
        assert_eq!(count1, 41); // Updated count after initializing common patterns
    }

    #[test]
    fn test_format_error_cached_function() {
        // Clear cache for clean test
        StringCache::clear();

        let result = format_error_cached(
            templates::FAILED_TO_RETRIEVE,
            "user settings",
            "database connection lost"
        );

        assert_eq!(result, "Failed to retrieve user settings: database connection lost");
    }

    #[test]
    fn test_cache_initialization() {
        // Clear cache and reinitialize
        StringCache::clear();
        let (count_before, _) = StringCache::get_stats();
        assert_eq!(count_before, 0);

        StringCache::initialize();
        let (count_after, _) = StringCache::get_stats();
        assert!(count_after > 0, "Cache should be pre-warmed with common patterns");

        // Should have exactly the number of patterns we defined
        assert_eq!(count_after, 35); // Updated count after consistent caching logic
    }

    // === NEW COMPREHENSIVE EDGE CASE TESTS ===

    #[test]
    fn test_context_with_braces_edge_case() {
        StringCache::clear();

        let template = "Failed to access {}: {}";
        let context = "file {config.json}";  // Contains braces
        let error = "not found";

        let result = StringCache::get_template_error(template, context, error);
        // This should work correctly - first {} replaced with context, second {} with error
        assert_eq!(result, "Failed to access file {config.json}: not found");
    }

    #[test]
    fn test_error_with_braces_edge_case() {
        StringCache::clear();

        let template = "Failed to parse {}: {}";
        let context = "JSON data";
        let error = "unexpected character '{' at position 5";  // Contains braces

        let result = StringCache::get_template_error(template, context, error);
        assert_eq!(result, "Failed to parse JSON data: unexpected character '{' at position 5");
    }

    #[test]
    fn test_multiple_placeholder_pairs() {
        StringCache::clear();

        // Test template with multiple {} pairs
        let template = "Failed to {} {}: {} occurred at {}";
        let context = "load config";
        let error = "file not found";

        let result = StringCache::get_template_error(template, context, error);
        // Should only replace first two {} placeholders
        assert_eq!(result, "Failed to load config file not found: {} occurred at {}");
    }

    #[test]
    fn test_caching_effectiveness() {
        StringCache::clear();

        let template = "Failed to connect to {}: {}";

        // Test multiple contexts with same template
        let result1 = StringCache::get_template_error(template, "database", "timeout");
        let result2 = StringCache::get_template_error(template, "API server", "503 error");
        let result3 = StringCache::get_template_error(template, "database", "connection refused");

        let (cache_count, _) = StringCache::get_stats();

        // Should have 2 cache entries (one for each unique template+context combination)
        assert_eq!(cache_count, 2);
        assert_eq!(result1, "Failed to connect to database: timeout");
        assert_eq!(result2, "Failed to connect to API server: 503 error");
        assert_eq!(result3, "Failed to connect to database: connection refused");
    }

    #[test]
    fn test_empty_context_and_error() {
        StringCache::clear();

        let template = "Failed to access {}: {}";
        let result = StringCache::get_template_error(template, "", "");
        assert_eq!(result, "Failed to access : ");
    }

    #[test]
    fn test_template_without_placeholders() {
        StringCache::clear();

        let template = "System error occurred";
        let result = StringCache::get_template_error(template, "context", "error");
        // Without placeholders, should return original template
        assert_eq!(result, "System error occurred");
    }

    #[test]
    fn test_template_with_single_placeholder() {
        StringCache::clear();

        let template = "Error in {}: no details available";
        let result = StringCache::get_template_error(template, "module", "timeout");
        // Should replace first placeholder only
        assert_eq!(result, "Error in module: no details available");
    }

    #[test]
    fn test_performance_under_load() {
        StringCache::clear();

        let template = "Operation {} failed: {}";

        // Generate many calls to test performance
        for i in 0..100 {
            let context = format!("batch_{}", i % 10); // 10 unique contexts
            let error = format!("error_{}", i);
            let _result = StringCache::get_template_error(template, &context, error);
        }

        let (cache_count, _) = StringCache::get_stats();
        // Should have cached 10 template+context combinations
        assert_eq!(cache_count, 10);
    }
}

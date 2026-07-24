//! # Memory Management Constants
//!
//! Centralized constants for memory management, token limits, and visual context handling
//! to eliminate magic numbers throughout the memory system.

/// Memory configuration limits
pub mod limits {
    /// Default maximum number of messages before pruning
    pub const DEFAULT_MAX_MESSAGES: usize = 15;

    /// Default maximum estimated tokens before pruning
    pub const DEFAULT_MAX_TOKENS: usize = 100000;

    /// Minimum number of messages to keep during pruning
    pub const DEFAULT_MIN_MESSAGES_TO_KEEP: usize = 3;

    /// Number of messages to summarize in a batch
    pub const DEFAULT_SUMMARIZATION_BATCH_SIZE: usize = 25;

    /// Emergency token threshold (leave buffer before 200K API limit)
    pub const EMERGENCY_TOKEN_THRESHOLD: usize = 180000;

    /// Critical token threshold for single message
    pub const CRITICAL_SINGLE_MESSAGE_TOKENS: usize = 50000;

    /// Large message warning threshold
    pub const LARGE_MESSAGE_WARNING_TOKENS: usize = 50000;

    /// Maximum summaries to keep to prevent unbounded growth
    pub const MAX_SUMMARIES_TO_KEEP: usize = 20;

    /// Hot context size for immediate processing
    pub const HOT_CONTEXT_SIZE: usize = 10;

    /// Emergency keep messages count (minimum for emergency pruning)
    pub const EMERGENCY_MIN_KEEP: usize = 2;
}

/// Token estimation constants
pub mod tokens {
    /// Standard characters per token for text content
    pub const CHARS_PER_TOKEN_TEXT: usize = 4;

    /// Characters per token for base64 images (more aggressive estimate)
    pub const CHARS_PER_TOKEN_BASE64_IMAGE: usize = 15;

    /// Characters per token for tool call inputs with images
    pub const CHARS_PER_TOKEN_TOOL_INPUT_IMAGE: usize = 20;

    /// Base tokens for tool call structure
    pub const BASE_TOOL_CALL_TOKENS: usize = 50;

    /// Emergency threshold multiplier (20% above max_tokens)
    pub const EMERGENCY_THRESHOLD_MULTIPLIER: f64 = 1.2;
}

/// Visual context and screenshot handling constants
pub mod visual {
    /// Default screenshot retention time in seconds (5 minutes)
    pub const DEFAULT_SCREENSHOT_RETENTION_SECONDS: u64 = 300;

    /// Default maximum number of screenshots to keep as base64 - FIXED FOR COMPUTER USE
    /// Computer use agents need to see actual screenshots, not text summaries
    pub const DEFAULT_MAX_BASE64_SCREENSHOTS: usize = 4;

    /// Minimum content length to consider as potential base64 image
    pub const MIN_BASE64_CONTENT_LENGTH: usize = 1000;

    /// Minimum content length to consider for screenshot detection
    pub const MIN_SCREENSHOT_CONTENT_LENGTH: usize = 10000;

    /// Percentage threshold for base64 character detection
    pub const BASE64_CHAR_THRESHOLD_PERCENT: usize = 80;
}

/// Conversation summary constants
pub mod summary {
    /// Maximum content length for short summary
    pub const MAX_SHORT_CONTENT_LENGTH: usize = 100;

    /// Minimum word length for keyword extraction
    pub const MIN_KEYWORD_LENGTH: usize = 3;

    /// Maximum number of keywords to extract
    pub const MAX_KEYWORDS_TO_EXTRACT: usize = 5;

    /// Approximate conversation start time offset in seconds (1 hour)
    pub const CONVERSATION_START_OFFSET_SECONDS: u64 = 3600;

    /// Maximum messages for short conversation handling
    pub const SHORT_CONVERSATION_MAX_MESSAGES: usize = 3;
}

/// Performance and optimization constants
pub mod performance {
    /// Maximum cache size before cleanup
    pub const MAX_CACHE_SIZE: usize = 100;

    /// Default operation timeout for metrics
    pub const DEFAULT_OPERATION_TIMEOUT_MS: u64 = 5000;

    /// Memory optimization interval
    pub const MEMORY_OPTIMIZATION_INTERVAL_MS: u64 = 300000; // 5 minutes

    /// Maximum number of latest summaries to return in API responses
    pub const MAX_LATEST_SUMMARIES_RETURNED: usize = 5;
}

/// Base64 image detection patterns
pub mod patterns {
    /// PNG image data URL prefix
    pub const PNG_DATA_URL_PREFIX: &str = "data:image/png;base64,";

    /// JPEG image data URL prefix
    pub const JPEG_DATA_URL_PREFIX: &str = "data:image/jpeg;base64,";

    /// WebP image data URL prefix
    pub const WEBP_DATA_URL_PREFIX: &str = "data:image/webp;base64,";

    /// Generic image data URL prefix
    pub const GENERIC_IMAGE_DATA_PREFIX: &str = "data:image/";

    /// Base64 content identifier
    pub const BASE64_IDENTIFIER: &str = "base64,";
}

/// Common words to exclude from keyword extraction
pub const COMMON_WORDS: &[&str] = &[
    "the", "and", "or", "but", "in", "on", "at", "to", "for", "of", "with", "by",
];

/// Default memory configuration
pub mod defaults {
    use super::limits::*;
    use super::visual::*;

    /// Get default memory configuration values
    pub fn get_memory_config() -> (usize, usize, usize, bool, bool, usize, bool, bool) {
        (
            DEFAULT_MAX_MESSAGES,
            DEFAULT_MAX_TOKENS,
            DEFAULT_MIN_MESSAGES_TO_KEEP,
            true, // auto_prune
            true, // enable_summarization
            DEFAULT_SUMMARIZATION_BATCH_SIZE,
            true, // enable_metrics
            true, // enable_summary_cache
        )
    }

    /// Get default visual context configuration values - FIXED FOR COMPUTER USE
    /// Computer use agents need to see actual screenshots to understand what's on screen
    pub fn get_visual_config() -> (bool, u64, bool, usize, bool) {
        (
            true, // enable_screenshot_compression (but not immediate)
            DEFAULT_SCREENSHOT_RETENTION_SECONDS,
            false, // immediate_compression - FIXED: Don't compress screenshots immediately!
            DEFAULT_MAX_BASE64_SCREENSHOTS, // Now allows 8 real screenshots
            true,  // fallback_to_generic_description
        )
    }
}

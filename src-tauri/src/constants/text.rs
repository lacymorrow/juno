//! # Text Processing Constants
//!
//! Constants for text processing, validation, and content analysis
//! to eliminate magic numbers throughout the text handling system.

/// Text length limits and validation
pub mod limits {
    /// Maximum text length for keyboard input
    pub const MAX_KEYBOARD_INPUT_LENGTH: usize = 10000;

    /// Maximum length for substantial user communication
    pub const MIN_SUBSTANTIAL_COMMUNICATION_LENGTH: usize = 20;

    /// Maximum length for short status messages
    pub const MAX_SHORT_STATUS_MESSAGE_LENGTH: usize = 100;

    /// Minimum length for detailed content
    pub const MIN_DETAILED_CONTENT_LENGTH: usize = 80;

    /// Maximum word count for simple messages
    pub const MAX_SIMPLE_MESSAGE_WORDS: usize = 10;

    /// Minimum word count for substantial content
    pub const MIN_SUBSTANTIAL_CONTENT_WORDS: usize = 15;

    /// Maximum description length for visual reasoning
    pub const MAX_VISUAL_DESCRIPTION_LENGTH: usize = 1000;

    /// Maximum content length for shell output preview
    pub const MAX_SHELL_OUTPUT_PREVIEW_LENGTH: usize = 100;

    /// Maximum content length for file content preview
    pub const MAX_FILE_CONTENT_PREVIEW_LENGTH: usize = 100;
}

/// Content analysis and detection
pub mod analysis {
    /// Minimum sentence count for multi-sentence content
    pub const MIN_MULTI_SENTENCE_COUNT: usize = 2;

    /// Maximum lines for simple content
    pub const MAX_SIMPLE_CONTENT_LINES: usize = 2;

    /// Complexity factor divisor for visual reasoning
    pub const COMPLEXITY_FACTOR_DIVISOR: f32 = 1000.0;

    /// Secondary complexity divisor for interaction context
    pub const SECONDARY_COMPLEXITY_DIVISOR: f32 = 500.0;

    /// Base processing time for visual reasoning (milliseconds)
    pub const BASE_VISUAL_PROCESSING_TIME_MS: u64 = 1000;

    /// Minimum processing time for visual reasoning (milliseconds)
    pub const MIN_VISUAL_PROCESSING_TIME_MS: u64 = 1000;

    /// Threshold for detailed visual descriptions
    pub const DETAILED_VISUAL_DESCRIPTION_THRESHOLD: usize = 500;
}

/// Percentage and ratio constants
pub mod ratios {
    /// Percentage multiplier (100%)
    pub const PERCENTAGE_MULTIPLIER: f64 = 100.0;

    /// Maximum percentage value
    pub const MAX_PERCENTAGE: f64 = 100.0;

    /// Minimum percentage value
    pub const MIN_PERCENTAGE: f64 = 0.0;

    /// Milliseconds per second conversion
    pub const MILLISECONDS_PER_SECOND: f64 = 1000.0;
}

/// Validation and error thresholds
pub mod validation {
    /// Maximum duration in seconds for operations
    pub const MAX_OPERATION_DURATION_SECONDS: f64 = 60.0;

    /// Minimum text length for keyword extraction
    pub const MIN_KEYWORD_LENGTH: usize = 3;

    /// Maximum number of goals for collaborative AI
    pub const MAX_COLLABORATIVE_AI_GOALS: usize = 10;

    /// Minimum description length for requests
    pub const MIN_REQUEST_DESCRIPTION_LENGTH: usize = 10;
}

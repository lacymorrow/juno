//! # API Constants
//!
//! API endpoints, headers, and content types.

// Core API endpoints
pub mod endpoints {
    pub const ANTHROPIC_API_URL: &str = "https://api.anthropic.com/v1/messages";
    pub const OPENAI_API_URL: &str = "https://api.openai.com/v1/chat/completions";
    pub const GEMINI_API_BASE: &str = "https://generativelanguage.googleapis.com/v1beta/models";

    // Cloud services
    pub const CLOUD_SERVER_URL: &str = "wss://juno-cloud-backend.fly.dev/ws";
    pub const GITHUB_URL: &str = "https://github.com/juno-ai";

    // Local development
    pub const LOCALHOST_BASE: &str = "http://localhost";
    pub const LOCALHOST_CHROME_DEBUG: &str = "http://localhost:9222";
    pub const LOCALHOST_MCP_SERVER: &str = "http://localhost:8080";
    pub const WEBSOCKET_LOCALHOST: &str = "ws://localhost:8080";

    // Third-party services
    pub const ELEVENLABS_TTS_BASE: &str = "https://api.elevenlabs.io/v1/text-to-speech";
    pub const REPLICATE_API_BASE: &str = "https://api.replicate.com";
    pub const JUNO_CLOUD_WEBSOCKET: &str = "wss://juno-cloud-backend.fly.dev/ws";

    // Development server
    pub const DEV_SERVER_BASE: &str = "http://localhost:1420";
    pub const HMR_WEBSOCKET: &str = "ws://localhost:1421";
}

// Anthropic Computer Use API Type Definitions (Official Specification)
pub mod computer_use_api_types {
    /// Computer Use API Version 2024-10-22
    pub const COMPUTER_20241022: &str = "computer_20241022";

    /// Computer Use API Version 2025-01-24 (Latest)
    pub const COMPUTER_20250124: &str = "computer_20250124";

    /// Text Editor API Type
    pub const EDIT_TOOL_20250124: &str = "str_replace_based_edit_tool_20250124";

    /// Enhanced Text Editor API Type with undo support
    pub const EDIT_TOOL_20250429: &str = "str_replace_based_edit_tool_20250429";

    /// Bash Command Execution API Type
    pub const BASH_20250124: &str = "bash_20250124";
}

// Beta Flags for Tool Groups (Official Anthropic Specification)
pub mod beta_flags {
    /// Beta flag for computer use 2024-10-22
    pub const COMPUTER_USE_2024_10_22: &str = "computer-use-2024-10-22";

    /// Beta flag for computer use 2025-01-24
    pub const COMPUTER_USE_2025_01_24: &str = "computer-use-2025-01-24";

    /// Beta flag for prompt caching
    pub const PROMPT_CACHING: &str = "prompt-caching-2024-07-31";
}

// Tool Version Groups (Mapping API versions to tool sets)
pub mod tool_version_groups {
    use super::computer_use_api_types::*;

    /// Tools available in computer use 2024-10-22
    pub const COMPUTER_USE_2024_10_22_TOOLS: &[&str] = &[
        COMPUTER_20241022,
        EDIT_TOOL_20250124,
        BASH_20250124,
    ];

    /// Tools available in computer use 2025-01-24 (Latest)
    pub const COMPUTER_USE_2025_01_24_TOOLS: &[&str] = &[
        COMPUTER_20250124,
        EDIT_TOOL_20250429,
        BASH_20250124,
    ];
}

// HTTP headers
pub mod http_headers {
    pub const CONTENT_TYPE: &str = "Content-Type";
    pub const X_API_KEY: &str = "x-api-key";
    pub const APPLICATION_JSON: &str = "application/json";
    pub const AUTHORIZATION: &str = "Authorization";
    pub const USER_AGENT: &str = "User-Agent";
    pub const ANTHROPIC_BETA: &str = "anthropic-beta";
}

// Anthropic content types
pub mod anthropic_content_types {
    pub const MESSAGE_START: &str = "message_start";
    pub const CONTENT_BLOCK_START: &str = "content_block_start";
    pub const CONTENT_BLOCK_DELTA: &str = "content_block_delta";
    pub const CONTENT_BLOCK_STOP: &str = "content_block_stop";
    pub const TEXT_DELTA: &str = "text_delta";
    pub const INPUT_JSON_DELTA: &str = "input_json_delta";
    pub const TOOL_USE: &str = "tool_use";
    pub const TOOL_RESULT: &str = "tool_result";
    pub const TEXT: &str = "text";
}

// Provider names
pub mod provider_names {
    pub const ANTHROPIC: &str = "anthropic";
    pub const OPENAI: &str = "openai";
    pub const GEMINI: &str = "gemini";
    pub const ELEVENLABS: &str = "elevenlabs";
    pub const REPLICATE: &str = "replicate";
    pub const SYSTEM: &str = "system";
}

// Cloud networking constants
pub mod cloud_networking {
    pub const MAX_CONNECTION_RETRIES: u32 = 10;
    pub const BASE_RETRY_DELAY_MS: u64 = 2000;
    pub const BACKOFF_MULTIPLIER: u32 = 2;
    pub const MAX_BACKOFF_EXPONENT: u32 = 5;
    pub const CONNECTION_CHECK_INTERVAL_MS: u64 = 5000;
    pub const WATCHDOG_TIMEOUT_MS: u64 = 60000;
    pub const MAX_RETRY_INTERVAL_MS: u64 = 300000; // 5 minutes

    // Heartbeat and status configuration
    pub const HEARTBEAT_SEND_INTERVAL_MS: u64 = 30000;
    pub const STATUS_CHECK_INTERVAL_MS: u64 = 30000;
    pub const RECONNECTION_DELAY_MS: u64 = 5000;
}

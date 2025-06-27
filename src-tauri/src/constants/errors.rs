//! # Error Constants
//!
//! Error codes and messages used throughout the application.

// Standard JSON-RPC error codes
pub mod codes {
    pub const PARSE_ERROR: i32 = -32700;
    pub const INVALID_REQUEST: i32 = -32600;
    pub const METHOD_NOT_FOUND: i32 = -32601;
    pub const INVALID_PARAMS: i32 = -32602;
    pub const INTERNAL_ERROR: i32 = -32603;
    pub const SERVER_ERROR_START: i32 = -32099;
    pub const SERVER_ERROR_END: i32 = -32000;

    // Application-specific error codes
    pub const TOOL_EXECUTION_ERROR: i32 = -32000;
    pub const ELEMENT_NOT_FOUND: i32 = -32001;
    pub const CACHE_MISS: i32 = -32002;
    pub const UNSUPPORTED_PLATFORM: i32 = -32003;

    // macOS specific error codes
    pub const MACOS_AX_NO_VALUE: i32 = -25212;
    pub const MACOS_AX_ATTRIBUTE_UNSUPPORTED: i32 = -25205;
    pub const MACOS_AX_GET_ATTRIBUTE_FAILED: i32 = -25204;

    // Enhanced orchestration error codes
    pub const ORCHESTRATION_TASK_FAILED: i32 = -32010;
    pub const ORCHESTRATION_BATCH_FAILED: i32 = -32011;
    pub const ORCHESTRATION_AGENT_UNAVAILABLE: i32 = -32012;
    pub const ORCHESTRATION_RESOURCE_EXHAUSTED: i32 = -32013;
    pub const ORCHESTRATION_CASCADING_FAILURE: i32 = -32014;
    pub const ORCHESTRATION_TIMEOUT: i32 = -32015;
    pub const ORCHESTRATION_INVALID_TASK: i32 = -32016;
    pub const ORCHESTRATION_DEPENDENCY_FAILED: i32 = -32017;
}

// Error messages
pub mod messages {
    pub const INVALID_PARAMS: &str = "invalid params";
    pub const METHOD_NOT_FOUND: &str = "method not found";
    pub const PARSE_ERROR: &str = "parse error";
    pub const ELEMENT_NOT_FOUND: &str = "element not found";
    pub const CACHE_MISS: &str = "cache miss";
    pub const UNSUPPORTED_OPERATION: &str = "unsupported operation";
    pub const UNSUPPORTED_PLATFORM: &str = "unsupported platform";
    pub const TOOL_EXECUTION_ERROR: &str = "tool execution error";

    // Enhanced orchestration error messages
    pub const ORCHESTRATION_TASK_FAILED: &str = "orchestration task failed";
    pub const ORCHESTRATION_BATCH_FAILED: &str = "orchestration batch execution failed";
    pub const ORCHESTRATION_AGENT_UNAVAILABLE: &str = "orchestration agent unavailable";
    pub const ORCHESTRATION_RESOURCE_EXHAUSTED: &str = "orchestration resource exhausted";
    pub const ORCHESTRATION_CASCADING_FAILURE: &str = "orchestration cascading failure detected";
    pub const ORCHESTRATION_TIMEOUT: &str = "orchestration timeout exceeded";
    pub const ORCHESTRATION_INVALID_TASK: &str = "orchestration invalid task configuration";
    pub const ORCHESTRATION_DEPENDENCY_FAILED: &str = "orchestration task dependency failed";
}

// Error recovery constants
pub mod recovery {
    // Recovery attempt delays
    pub const ELEMENT_NOT_FOUND_DELAY_MS: u64 = 1000;
    pub const NETWORK_ERROR_DELAY_MS: u64 = 2000;
    pub const TIMEOUT_RECOVERY_DELAY_MS: u64 = 5000;
    pub const RATE_LIMIT_BACKOFF_MS: u64 = 60000;
    pub const BROWSER_NOT_READY_DELAY_MS: u64 = 3000;

    // Default recovery configuration
    pub const DEFAULT_BASE_RETRY_DELAY_MS: u64 = 500;
    pub const DEFAULT_MAX_RETRY_DELAY_MS: u64 = 10000;
    pub const DEFAULT_TIMEOUT_THRESHOLD_MS: u64 = 30000;

    // Backoff configuration
    pub const BACKOFF_MULTIPLIER: u32 = 2;
    pub const MAX_BACKOFF_EXPONENT: u32 = 5;

    // Enhanced orchestration recovery constants
    pub const ORCHESTRATION_TASK_RETRY_DELAY_MS: u64 = 1500;
    pub const ORCHESTRATION_BATCH_FAILURE_DELAY_MS: u64 = 3000;
    pub const ORCHESTRATION_AGENT_FAILURE_DELAY_MS: u64 = 2000;
    pub const ORCHESTRATION_RESOURCE_EXHAUSTION_DELAY_MS: u64 = 5000;
    pub const ORCHESTRATION_CASCADING_FAILURE_THRESHOLD: u32 = 3;

    // Orchestration performance thresholds
    pub const MIN_PARALLEL_FACTOR: f32 = 1.0;
    pub const TARGET_PARALLEL_FACTOR: f32 = 2.0;
    pub const OPTIMAL_PARALLEL_FACTOR: f32 = 3.0;
    pub const MAX_BATCH_RETRY_ATTEMPTS: u32 = 2;
}

// # Error Message Templates
//
// Centralized error message templates used throughout the application.
// These provide consistent error messaging and reduce code duplication.

/// Common error message templates
pub mod templates {
    pub const FAILED_TO_EMIT: &str = "Failed to emit {} event: {}";
    pub const FAILED_TO_INITIALIZE: &str = "Failed to initialize {}: {}";
    pub const FAILED_TO_REGISTER: &str = "Failed to register {}: {}";
    pub const FAILED_TO_SUBMIT: &str = "Failed to submit {}: {}";
    pub const FAILED_TO_PARSE: &str = "Failed to parse {}: {}";
    pub const FAILED_TO_CREATE: &str = "Failed to create {}: {}";
    pub const FAILED_TO_LOAD: &str = "Failed to load {}: {}";
    pub const FAILED_TO_SAVE: &str = "Failed to save {}: {}";
    pub const FAILED_TO_UPDATE: &str = "Failed to update {}: {}";
    pub const FAILED_TO_DELETE: &str = "Failed to delete {}: {}";
    pub const FAILED_TO_CONNECT: &str = "Failed to connect to {}: {}";
    pub const FAILED_TO_EXECUTE: &str = "Failed to execute {}: {}";
    pub const FAILED_TO_START: &str = "Failed to start {}: {}";
    pub const FAILED_TO_STOP: &str = "Failed to stop {}: {}";
    pub const FAILED_TO_CONFIGURE: &str = "Failed to configure {}: {}";
    pub const FAILED_TO_PROCESS: &str = "Failed to process {}: {}";
    pub const FAILED_TO_VALIDATE: &str = "Failed to validate {}: {}";
    pub const FAILED_TO_ACCESS: &str = "Failed to access {}: {}";
    pub const FAILED_TO_RETRIEVE: &str = "Failed to retrieve {}: {}";
    pub const FAILED_TO_SEND: &str = "Failed to send {}: {}";
    pub const FAILED_TO_RECEIVE: &str = "Failed to receive {}: {}";
    pub const FAILED_TO_CONVERT: &str = "Failed to convert {}: {}";
    pub const FAILED_TO_DECODE: &str = "Failed to decode {}: {}";
    pub const FAILED_TO_ENCODE: &str = "Failed to encode {}: {}";
    pub const FAILED_TO_COMPRESS: &str = "Failed to compress {}: {}";
    pub const FAILED_TO_SET: &str = "Failed to set {}: {}";
    pub const FAILED_TO_RESTORE: &str = "Failed to restore {}: {}";
    pub const FAILED_TO_WRITE: &str = "Failed to write {}: {}";
    pub const FAILED_TO_CLEANUP: &str = "Failed to cleanup {}: {}";
    pub const FAILED_TO_CAPTURE: &str = "Failed to capture {}: {}";
}

/// Error categories for consistent classification
pub mod categories {
    pub const PERMISSION_ERROR: &str = "permission_error";
    pub const NETWORK_ERROR: &str = "network_error";
    pub const VALIDATION_ERROR: &str = "validation_error";
    pub const INITIALIZATION_ERROR: &str = "initialization_error";
    pub const CONFIGURATION_ERROR: &str = "configuration_error";
    pub const AGENT_ERROR: &str = "agent_error";
    pub const TOOL_ERROR: &str = "tool_error";
    pub const VOICE_ERROR: &str = "voice_error";
    pub const SYSTEM_ERROR: &str = "system_error";
    pub const PARSING_ERROR: &str = "parsing_error";
    pub const IO_ERROR: &str = "io_error";
    pub const AUTHENTICATION_ERROR: &str = "authentication_error";
    pub const TIMEOUT_ERROR: &str = "timeout_error";
    pub const RESOURCE_ERROR: &str = "resource_error";
    pub const COMPATIBILITY_ERROR: &str = "compatibility_error";
}

/// Context-specific error components
pub mod components {
    pub const EVENT: &str = "event";
    pub const SETTINGS_MANAGER: &str = "settings manager";
    pub const AGENT_BRAIN: &str = "agent brain";
    pub const ORCHESTRATOR: &str = "orchestrator";
    pub const MCP_MANAGER: &str = "MCP manager";
    pub const BROWSER_CONTROLLER: &str = "browser controller";
    pub const DESKTOP_ENGINE: &str = "desktop engine";
    pub const WHISPER_CONTEXT: &str = "Whisper context";
    pub const VOICE_CONTROLLER: &str = "voice controller";
    pub const ALWAYS_LISTENING_CONTROLLER: &str = "always listening controller";
    pub const ESCAPE_KEY: &str = "escape key";
    pub const GLOBAL_SHORTCUTS: &str = "global shortcuts";
    pub const COMPUTER_USE_TOOLS: &str = "Computer Use tools";
    pub const TOOL_PROVIDER: &str = "tool provider";
    pub const SELF_IMPROVEMENT_SYSTEM: &str = "self-improvement system";
    pub const ACCESSIBILITY: &str = "desktop accessibility";
    pub const CLIPBOARD: &str = "clipboard";
    pub const PLAYWRIGHT_DRIVER: &str = "Playwright driver";
    pub const AUTOMATION: &str = "automation";
    pub const APPLICATION_STATE: &str = "application state";
    pub const CLOUD_CLIENT: &str = "cloud client";
    pub const MCP_SERVERS: &str = "MCP servers";
    pub const ONBOARDING_SYSTEM: &str = "onboarding system";
    pub const AUTOSTART_CONFIGURATION: &str = "autostart configuration";
    pub const AI_PROVIDER_SETTINGS: &str = "AI provider settings";
    pub const VOICE_TRANSCRIPTION_CONFIG: &str = "voice transcription config";
    pub const DICTATION_INPUT_MONITORING: &str = "dictation input monitoring";
}

/// Action-specific error components
pub mod actions {
    pub const QUERY: &str = "query";
    pub const EVENT_EMIT: &str = "event emission";
    pub const TOOL_REGISTRATION: &str = "tool registration";
    pub const SHORTCUT_REGISTRATION: &str = "shortcut registration";
    pub const TOOL_EXECUTION: &str = "tool execution";
    pub const COMMAND_EXECUTION: &str = "command execution";
    pub const API_RESPONSE: &str = "API response";
    pub const TOOL_ARGUMENTS: &str = "tool arguments";
    pub const JSON_PARSING: &str = "JSON parsing";
    pub const WEBSOCKET_MESSAGE: &str = "WebSocket message";
    pub const CLOUD_COMMAND: &str = "cloud command";
    pub const SETTINGS_JSON: &str = "settings JSON";
    pub const PAYLOAD_PARSING: &str = "payload parsing";
    pub const KEY_COMBINATION: &str = "key combination";
    pub const TOOL_DEFINITION: &str = "tool definition";
    pub const BATCH_RESPONSE: &str = "batch response";
    pub const DOM_STRUCTURE: &str = "DOM structure";
    pub const IMAGE_PROCESSING: &str = "image processing";
    pub const SCROLL_INPUT: &str = "scroll input";
    pub const WAIT_INPUT: &str = "wait input";
    pub const RELEASE_KEY_INPUT: &str = "release_key input";
    pub const SET_CLIPBOARD_INPUT: &str = "set_clipboard input";
    // Settings-specific actions
    pub const SETTINGS: &str = "settings";
    pub const KEYBOARD_SHORTCUTS: &str = "keyboard shortcuts";
    pub const FLOATING_BAR_SETTINGS: &str = "floating bar settings";
    pub const AGENT_SETTINGS: &str = "agent settings";
    pub const PROVIDER_SETTINGS: &str = "provider settings";
    pub const CLOUD_SETTINGS: &str = "cloud settings";
    pub const AUDIO_SETTINGS: &str = "audio settings";
    pub const TOOL_SETTINGS: &str = "tool settings";
    pub const ONBOARDING_SETTINGS: &str = "onboarding settings";
    pub const AUTOSTART_SETTING: &str = "autostart setting";
    pub const WINDOW_LIST_JSON: &str = "window list JSON";
    pub const WINDOW_INFO_JSON: &str = "window info JSON";
    pub const EXECUTE_COMMAND_INPUT: &str = "execute_command input";
    pub const OPEN_FILE_AND_TYPE_INPUT: &str = "open_file_and_type input";
    pub const COPY_AND_PASTE_INPUT: &str = "copy_and_paste input";
}

/// User-facing error messages for common scenarios
pub mod user_messages {
    pub const PERMISSION_GUIDANCE_NEEDED: &str = "Permission guidance needed";
    pub const VOICE_UNAVAILABLE: &str = "Voice transcription is not available";
    pub const SHORTCUT_PERMISSIONS_MISSING: &str = "This may be due to missing Input Monitoring permissions";
    pub const ESCAPE_KEY_UNAVAILABLE: &str = "continuing without escape key cancellation";
    pub const SHORTCUTS_UNAVAILABLE: &str = "continuing without shortcuts";
    pub const TTS_ESCAPE_WARNING: &str = "TTS will still work but escape key may not stop it";
    pub const RETRYING: &str = "Retrying...";
    pub const USING_DEFAULTS: &str = "using defaults";
    pub const CONTINUING_ANYWAY: &str = "continuing anyway";
    pub const FEATURE_UNAVAILABLE: &str = "will be unavailable";
    pub const INITIALIZATION_INCOMPLETE: &str = "Creating uninitialized controller";
}

/// Logging prefixes for consistent log formatting
pub mod prefixes {
    pub const AGENT_MODE: &str = "[Agent Mode]";
    pub const DICTATION_MODE: &str = "[Dictation Mode]";
    pub const ALWAYS_LISTENING: &str = "[AlwaysListening]";
    pub const MENU: &str = "[Menu]";
    pub const TRAY_MENU: &str = "[TrayMenu]";
    pub const COMMAND: &str = "[Command]";
    pub const STATE_MANAGER: &str = "[StateManager]";
    pub const DICTATION_STATE_MANAGER: &str = "[DictationStateManager]";
    pub const ESCAPE_KEY_COORDINATOR: &str = "[EscapeKeyCoordinator]";
    pub const STOP_COORDINATOR: &str = "[StopCoordinator]";
    pub const TIMER_EVENT: &str = "[Timer Event]";
    pub const EVENT: &str = "[Event]";
    pub const TTS: &str = "[TTS]";
    pub const AGENT_MODE_SHORTCUT: &str = "[Agent Mode Shortcut]";
    pub const DICTATION_INPUT_SHORTCUT: &str = "[Dictation Input Shortcut]";
    pub const DICTATION_TAP_MODE: &str = "[Dictation Tap Mode]";
}



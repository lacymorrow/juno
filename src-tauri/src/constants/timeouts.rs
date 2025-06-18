//! # Timeout Constants
//!
//! All timeout and delay constants used throughout the application.

// Basic delays
pub const MICRO_DELAY_MS: u64 = 10;
pub const MINIMAL_DELAY_MS: u64 = 20;
pub const SMALL_DELAY_MS: u64 = 50;
pub const SHORT_DELAY_MS: u64 = 100;
pub const MEDIUM_DELAY_MS: u64 = 150;
pub const ANIMATION_DELAY_MS: u64 = 300;
pub const STANDARD_DELAY_MS: u64 = 500;
pub const LONG_DELAY_MS: u64 = 800;
pub const VERY_LONG_DELAY_MS: u64 = 1000;
pub const EXTENDED_DELAY_MS: u64 = 2000;
pub const MAX_DELAY_MS: u64 = 3000;

// Standard timeouts
pub const STANDARD_TIMEOUT_MS: u64 = 10000;
pub const BROWSER_TIMEOUT_MS: u64 = 30000;

// Monitor intervals
pub const DICTATION_MONITOR_INTERVAL_MS: u64 = 50;
pub const AGENT_MONITOR_INTERVAL_MS: u64 = 100;
pub const TREE_SEARCH_INTERVAL_MS: u64 = 250;
pub const HEARTBEAT_INTERVAL_MS: u64 = 30000;

// Mouse action delays
pub const MOUSE_MICRO_DELAY_MS: u64 = 10;
pub const MOUSE_CLICK_DELAY_MS: u64 = 50;
pub const MOUSE_ACTION_DELAY_MS: u64 = 100;
pub const MOUSE_SEQUENCE_DELAY_MS: u64 = 300;
pub const DOUBLE_CLICK_DELAY_MS: u64 = 500;

// UI animation delays
pub const UI_FADE_DELAY_MS: u64 = 300;
pub const UI_SLIDE_DELAY_MS: u64 = 600;
pub const UI_NOTIFICATION_DISPLAY_MS: u64 = 3000;

// Permission and system delays
pub const PERMISSION_CHECK_DELAY_MS: u64 = 1000;
pub const SCREEN_RECORDING_CHECK_DELAY_MS: u64 = 2000;
pub const SYSTEM_SETTINGS_OPERATION_TIMEOUT_MS: u64 = 3000;
pub const SYSTEM_SETTINGS_CHECK_TIMEOUT_MS: u64 = 5000;

// MCP server delays
pub const MCP_SERVER_STARTUP_DELAY_MS: u64 = 500;
pub const MCP_SERVER_RESTART_DELAY_MS: u64 = 1000;

// Cloud connection delays
pub const CLOUD_RETRY_BASE_DELAY_MS: u64 = 2000;
pub const CLOUD_HEARTBEAT_INTERVAL_MS: u64 = 30000;
pub const CLOUD_STATUS_INTERVAL_MS: u64 = 30000;
pub const CLOUD_RECONNECT_DELAY_MS: u64 = 5000;
pub const CLOUD_WATCHDOG_INTERVAL_MS: u64 = 60000;

// Audio and voice processing
pub const TTS_PROCESSING_DELAY_MS: u64 = 1000;
pub const PARTIAL_BUFFER_DURATION_MS: u64 = 1500;
pub const FINAL_BUFFER_DURATION_MS: u64 = 5000;
pub const MIN_AUDIO_LENGTH_MS: u64 = 500;

// Navigation and processing timeouts
pub const DEFAULT_NAVIGATION_TIMEOUT_MS: u64 = 30_000;
pub const REPLICATE_TIMEOUT_SECONDS: u64 = 300;
pub const PERMISSION_CHECK_TIMEOUT_MS: u64 = 3_000;
pub const AUDIO_DEVICE_DETECTION_TIMEOUT_MS: u64 = 3_000;
pub const TOOL_EXECUTION_TIMEOUT_MS: u64 = 10_000;
pub const MCP_INTEGRATION_TIMEOUT_MS: u64 = 30_000;

// Browser page delays
pub const BROWSER_PAGE_LOAD_DELAY_MS: u64 = 1000;

// Shell command delays
pub const SHELL_COMMAND_DELAY_MS: u64 = 10;

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

// Common timeout aliases
pub const DEFAULT_TIMEOUT_MS: u64 = 5000;
pub const QUICK_TIMEOUT_MS: u64 = 2000;
pub const SLOW_TIMEOUT_MS: u64 = 10000;

// Frontend UI timeouts (moved from frontend constants.ts)
pub const SOUND_DEBOUNCE_MS: u64 = 300;
pub const HEARTBEAT_INTERVAL_MS: u64 = 30000;
pub const CLOUD_CONNECTION_TIMEOUT_MS: u64 = 10000;
pub const CLOUD_RECONNECT_DELAY_MS: u64 = 5000;
pub const ANIMATION_FAST_MS: u64 = 150;
pub const ANIMATION_NORMAL_MS: u64 = 300;
pub const ANIMATION_SLOW_MS: u64 = 500;

// Standard timeouts (in seconds)
pub const STANDARD_TIMEOUT_SECONDS: u64 = 10;
pub const BROWSER_TIMEOUT_SECONDS: u64 = 30;
pub const NETWORK_TIMEOUT_SECONDS: u64 = 30;
pub const HEARTBEAT_INTERVAL_SECONDS: u64 = 30;
pub const STATUS_UPDATE_INTERVAL_SECONDS: u64 = 30;

// HTTP client timeouts for API requests
pub const HTTP_CONNECT_TIMEOUT_SECONDS: u64 = 10;  // Connection establishment timeout
pub const HTTP_REQUEST_TIMEOUT_SECONDS: u64 = 120; // Total request timeout for LLM responses

// Error recovery timeouts
pub const ERROR_RECOVERY_MAX_RETRY_DELAY_SECONDS: u64 = 10;
pub const ERROR_RECOVERY_TIMEOUT_THRESHOLD_SECONDS: u64 = 30;
pub const ERROR_RECOVERY_WAIT_SHORT_SECONDS: u64 = 3;
pub const ERROR_RECOVERY_WAIT_LONG_SECONDS: u64 = 10;

// Testing timeouts
pub const TESTING_HUMAN_AVERAGE_SECONDS: u64 = 480; // 8 minutes
pub const TESTING_AGENT_AVERAGE_SECONDS: u64 = 120; // 2 minutes
pub const TESTING_RESPONSE_TIME_LIMIT_SECONDS: u64 = 30;
pub const TESTING_QA_TIMEOUT_SECONDS: u64 = 60;

// Voice and audio timeouts
pub const VOICE_CACHE_VALIDITY_SECONDS: u64 = 30;
pub const VOICE_STATE_CHECK_SECONDS: u64 = 2;
pub const VOICE_WAKE_DETECTION_SECONDS: u64 = 60;

// Cloud and network intervals
pub const CLOUD_HEARTBEAT_INTERVAL_SECONDS: u64 = 30;
pub const CLOUD_STATUS_INTERVAL_SECONDS: u64 = 30;
pub const CLOUD_WATCHDOG_INTERVAL_SECONDS: u64 = 60;
pub const CLOUD_MAX_RETRY_DELAY_SECONDS: u64 = 300; // 5 minutes max

// MCP and tool timeouts
pub const MCP_OPERATION_TIMEOUT_SECONDS: u64 = 5;
pub const MCP_GRACEFUL_SHUTDOWN_SECONDS: u64 = 3;
pub const MCP_MAX_BACKOFF_DELAY_SECONDS: u64 = 30;
pub const MCP_SERVER_STARTUP_TIMEOUT_SECONDS: u64 = 45;

// Orchestrator and agent timeouts
pub const ORCHESTRATOR_PARALLEL_EXECUTION_TIMEOUT_SECONDS: u64 = 5;
pub const ORCHESTRATOR_MIN_TIMEOUT_SECONDS: u64 = 30;
pub const ORCHESTRATOR_MAX_TIMEOUT_SECONDS: u64 = 600; // 10 minutes
pub const AGENT_STEP_DELAY_SECONDS: u64 = 1;

// Browser automation timeouts
pub const BROWSER_CONNECTION_TIMEOUT_SECONDS: u64 = 8;
pub const BROWSER_PAGE_OPERATION_TIMEOUT_SECONDS: u64 = 2;
pub const BROWSER_CLICK_TIMEOUT_SECONDS: u64 = 1;

// Desktop automation timeouts
pub const DESKTOP_TYPING_TIMEOUT_SECONDS: u64 = 30;

// Visual reasoning timeouts
pub const VISUAL_PROCESSING_TIMEOUT_SECONDS: u64 = 10;
pub const VISUAL_TEMPORAL_CONTEXT_SECONDS: u64 = 30;

// Collaborative AI timeouts
pub const COLLABORATIVE_AI_KNOWLEDGE_RETRIEVAL_SECONDS: u64 = 30;
pub const COLLABORATIVE_AI_COORDINATION_SECONDS: u64 = 60;

// Monitor intervals
pub const DICTATION_MONITOR_INTERVAL_MS: u64 = 50;
pub const AGENT_MONITOR_INTERVAL_MS: u64 = 100;
pub const TREE_SEARCH_INTERVAL_MS: u64 = 250;

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

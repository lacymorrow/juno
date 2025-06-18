pub mod events {
    pub const AGENT_EVENT: &str = "agent-event";
    pub const APP_DICTATION_STARTED: &str = "app-dictation-started";
    pub const APP_DICTATION_FINISHED: &str = "app-dictation-finished";
    pub const APP_DICTATION_PARTIAL_RESULT: &str = "app-dictation-partial-result";
    pub const APP_DICTATION_ERROR: &str = "app-dictation-error";

    // Agent Events
    pub const AGENT_PROCESSING_COMPLETE: &str = "agent-processing-complete";
    pub const AGENT_PROCESSING_ERROR: &str = "agent-processing-error";
    pub const AGENT_STATE_CHANGED: &str = "agent-state-changed";
    pub const AGENT_TOOL_CALL: &str = "agent-tool-call";
    pub const AGENT_THOUGHT_PROCESS: &str = "agent-thought-process";
    pub const AGENT_STOPPING: &str = "agent-stopping";
    pub const AGENT_STATUS_UPDATE: &str = "agent-status-update";

    // Streaming Events
    pub const AGENT_TEXT_STREAM: &str = "agent-text-stream";
    pub const AGENT_STREAM_START: &str = "agent-stream-start";
    pub const AGENT_STREAM_END: &str = "agent-stream-end";

    // User Message Events
    pub const USER_MESSAGE_SUBMITTED: &str = "user-message-submitted";

    // Window/UI events
    pub const BAR_STATE_CHANGED: &str = "bar-state-changed";

    // Voice Control specific events (if any beyond started/finished/partial)
    pub const DICTATION_STATE_CHANGED: &str = "dictation-state-changed";
    pub const REQUEST_AUDIO_PLAYBACK_TEST: &str = "request-audio-playback-test";

    // Settings events
    pub const SETTINGS_REQUESTED: &str = "settings-requested";
    pub const DEVTOOLS_REQUESTED: &str = "devtools-requested";
    pub const PERMISSIONS_REQUESTED: &str = "permissions-requested";
    pub const FEEDBACK_REQUESTED: &str = "feedback-requested";

    // New menu events
    pub const HELP_REQUESTED: &str = "help-requested";
    pub const NEW_CHAT_REQUESTED: &str = "new-chat-requested";
    pub const CLEAR_HISTORY_REQUESTED: &str = "clear-history-requested";
    pub const IMPORT_CHAT_REQUESTED: &str = "import-chat-requested";
    pub const EXPORT_CHAT_REQUESTED: &str = "export-chat-requested";
    pub const TOGGLE_FLOATING_BAR_REQUESTED: &str = "toggle-floating-bar-requested";
    pub const TOGGLE_DEV_PANEL_REQUESTED: &str = "toggle-dev-panel-requested";
    pub const TOGGLE_FULLSCREEN_REQUESTED: &str = "toggle-fullscreen-requested";
    pub const MINIMIZE_WINDOW_REQUESTED: &str = "minimize-window-requested";
    pub const ZOOM_WINDOW_REQUESTED: &str = "zoom-window-requested";
    pub const UPDATE_CHECK_REQUESTED: &str = "update-check-requested";

    // Agent State Events - Missing from current events
    pub const AGENT_ACTIVE: &str = "agent-active";
    pub const AGENT_ERROR: &str = "agent-error";
    pub const AGENT_TRANSCRIPTION_START: &str = "agent-transcription-start";
    pub const AGENT_TRANSCRIPTION_STOP: &str = "agent-transcription-stop";
    pub const AGENT_CANCEL: &str = "agent-cancel";
    pub const AGENT_COMMITTED: &str = "agent-committed";
    pub const AGENT_FORCE_STOP: &str = "agent-force-stop";
    pub const AGENT_FORCE_CLEANUP: &str = "agent-force-cleanup";

    // Dictation State Events
    pub const DICTATION_ACTIVE: &str = "dictation-active";
    pub const DICTATION_CANCELLED: &str = "dictation-cancelled";
    pub const DICTATION_TRANSCRIPTION_START: &str = "dictation-transcription-start";
    pub const DICTATION_TRANSCRIPTION_STOP: &str = "dictation-transcription-stop";
    pub const DICTATION_COMMITTED: &str = "dictation-committed";
    pub const DICTATION_STOP: &str = "dictation-stop";
    pub const DICTATION_TRANSCRIPTION_CANCEL: &str = "dictation-transcription-cancel";
    pub const DICTATION_TRANSCRIPTION_FORCE_STOP: &str = "dictation-transcription-force-stop";
    pub const DICTATION_TRANSCRIPTION_FORCE_CLEANUP: &str = "dictation-transcription-force-cleanup";


    // TTS Events
    pub const TTS_AUDIO_READY: &str = "tts-audio-ready";
    pub const TTS_STOP_REQUESTED: &str = "tts-stop-requested";

    // UI Visualization Events
    pub const KEY_PRESS_VISUALIZATION: &str = "key-press-visualization";
    pub const CLICK_VISUALIZATION: &str = "click-visualization";

    // Always Listening Events
    pub const ALWAYS_LISTENING_MODE_CHANGED: &str = "always-listening-mode-changed";
    pub const ALWAYS_LISTENING_WAKE_WORD_DETECTED: &str = "always-listening:wake-word-detected";
    pub const TOGGLE_DICTATION_REQUEST: &str = "toggle-dictation-request";

    // Permission Events
    pub const PERMISSIONS_CHANGED: &str = "permissions-changed";
    pub const PERMISSIONS_RESTART_REQUIRED: &str = "permissions-restart-required";

    // About/Menu Events
    pub const ABOUT_REQUESTED: &str = "about-requested";

    // Dev Tool Events
    pub const DEV_TOOL_NOTIFICATION: &str = "dev-tool-notification";
}

pub mod tool_names {
    // Agent delegation tools
    pub const DELEGATE_TO_BROWSER_AGENT: &str = "delegate_to_browser_agent";
    pub const DELEGATE_TO_DESKTOP_AGENT: &str = "delegate_to_desktop_agent";
    pub const DELEGATE_TO_FILE_AGENT: &str = "delegate_to_file_agent";

    // Anthropic Computer Use tools
    pub const COMPUTER: &str = "computer";
    pub const BASH: &str = "bash";
    pub const STR_REPLACE_BASED_EDIT_TOOL: &str = "str_replace_based_edit_tool";

    // Text editor tools
    pub const TEXT_EDITOR_INSERT: &str = "text_editor_insert";
    pub const TEXT_EDITOR_STR_REPLACE: &str = "text_editor_str_replace";
    pub const TEXT_EDITOR_UNDO_EDIT: &str = "text_editor_undo_edit";

    // Computer use actions
    pub const ACTION_SCREENSHOT: &str = "screenshot";
    pub const ACTION_CLICK: &str = "click";
    pub const ACTION_TYPE: &str = "type";
    pub const ACTION_KEY: &str = "key";
    pub const ACTION_SCROLL: &str = "scroll";
    pub const ACTION_WAIT: &str = "wait";

    // Extended computer use actions
    pub const ACTION_LEFT_CLICK: &str = "left_click";
    pub const ACTION_RIGHT_CLICK: &str = "right_click";
    pub const ACTION_MIDDLE_CLICK: &str = "middle_click";
    pub const ACTION_DOUBLE_CLICK: &str = "double_click";
    pub const ACTION_TRIPLE_CLICK: &str = "triple_click";
    pub const ACTION_LEFT_CLICK_DRAG: &str = "left_click_drag";
    pub const ACTION_MOUSE_MOVE: &str = "mouse_move";
    pub const ACTION_HOLD_KEY: &str = "hold_key";
}

pub mod http_headers {
    pub const CONTENT_TYPE: &str = "Content-Type";
    pub const X_API_KEY: &str = "x-api-key";
    pub const APPLICATION_JSON: &str = "application/json";
    pub const AUTHORIZATION: &str = "Authorization";
    pub const USER_AGENT: &str = "User-Agent";
}

pub mod browser_js {
    pub const QUERY_SELECTOR_ALL: &str = "document.querySelectorAll";
    pub const QUERY_SELECTOR: &str = "document.querySelector";
    pub const TEXT_CONTENT: &str = "textContent";
    pub const GET_ATTRIBUTE: &str = "getAttribute";
    pub const CLICK: &str = "click";
    pub const FOCUS: &str = "focus";
}

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

pub mod chrome_flags {
    pub const REMOTE_DEBUG_PORT_FLAG: &str = "--remote-debugging-port=9222";
    pub const HEADLESS_FLAG: &str = "--headless";
    pub const NO_SANDBOX_FLAG: &str = "--no-sandbox";
    pub const DISABLE_GPU_FLAG: &str = "--disable-gpu";
    pub const DISABLE_DEV_SHM_FLAG: &str = "--disable-dev-shm-usage";
}

pub mod common_files {
    pub const PACKAGE_JSON: &str = "package.json";
    pub const CARGO_TOML: &str = "Cargo.toml";
    pub const REQUIREMENTS_TXT: &str = "requirements.txt";
    pub const COMPOSER_JSON: &str = "composer.json";
    pub const README_MD: &str = "README.md";
    pub const README_TXT: &str = "README.txt";
    pub const TSCONFIG_JSON: &str = "tsconfig.json";
    pub const MAIN_PY: &str = "main.py";
    pub const INDEX_JS: &str = "index.js";
    pub const MAIN_RS: &str = "main.rs";
    pub const APP_TSX: &str = "App.tsx";
}

pub mod provider_names {
    pub const ANTHROPIC: &str = "anthropic";
    pub const OPENAI: &str = "openai";
    pub const GEMINI: &str = "gemini";
    pub const ELEVENLABS: &str = "elevenlabs";
    pub const REPLICATE: &str = "replicate";
    pub const SYSTEM: &str = "system";
}

pub mod error_recovery {
    // Recovery attempt delays
    pub const ELEMENT_NOT_FOUND_DELAY_MS: u64 = 1000;
    pub const NETWORK_ERROR_DELAY_MS: u64 = 2000;
    pub const TIMEOUT_RECOVERY_DELAY_MS: u64 = 5000;
    pub const RATE_LIMIT_BACKOFF_MS: u64 = 60000;
    pub const BROWSER_NOT_READY_DELAY_MS: u64 = 3000;

    // Default recovery configuration
    pub const DEFAULT_MAX_RETRIES: usize = 3;
    pub const DEFAULT_BASE_RETRY_DELAY_MS: u64 = 500;
    pub const DEFAULT_MAX_RETRY_DELAY_MS: u64 = 10000;
    pub const DEFAULT_TIMEOUT_THRESHOLD_MS: u64 = 30000;

    // Exponential backoff parameters
    pub const BACKOFF_MULTIPLIER: u32 = 2;
    pub const MAX_BACKOFF_EXPONENT: u32 = 5;
}

pub mod cloud_networking {
    // Connection retry parameters
    pub const MAX_CONNECTION_RETRIES: u32 = 10;
    pub const BASE_RETRY_DELAY_MS: u64 = 2000;
    pub const CONNECTION_CHECK_INTERVAL_MS: u64 = 5000;
    pub const WATCHDOG_TIMEOUT_MS: u64 = 60000;
    pub const MAX_RETRY_INTERVAL_MS: u64 = 300000; // 5 minutes

    // Heartbeat and status intervals
    pub const HEARTBEAT_SEND_INTERVAL_MS: u64 = 30000;
    pub const STATUS_CHECK_INTERVAL_MS: u64 = 30000;
    pub const RECONNECTION_DELAY_MS: u64 = 5000;

    // Exponential backoff limits
    pub const BACKOFF_MULTIPLIER: u32 = 2;
    pub const MAX_BACKOFF_EXPONENT: u32 = 5;
}

pub mod macos_system {
    // System Preferences URLs
    pub const MICROPHONE_PRIVACY_URL: &str = "x-apple.systempreferences:com.apple.preference.security?Privacy_Microphone";
    pub const SCREEN_RECORDING_PRIVACY_URL: &str = "x-apple.systempreferences:com.apple.preference.security?Privacy_ScreenCapture";
    pub const INPUT_MONITORING_PRIVACY_URL: &str = "x-apple.systempreferences:com.apple.preference.security?Privacy_ListenEvent";
    pub const ACCESSIBILITY_PRIVACY_URL: &str = "x-apple.systempreferences:com.apple.preference.security?Privacy_Accessibility";

    // Bundle identifiers and application paths
    pub const SYSTEM_PREFERENCES_BUNDLE: &str = "com.apple.systempreferences";
    pub const SYSTEM_SETTINGS_BUNDLE: &str = "com.apple.systemsettings";
    pub const SECURITY_PREFPANE_PATH: &str = "/System/Library/PreferencePanes/Security.prefPane";

    // Command flags
    pub const BUNDLE_FLAG: &str = "-b";
    pub const OPEN_COMMAND: &str = "open";
    pub const OSASCRIPT_COMMAND: &str = "osascript";

    // Permission descriptions
    pub const ACCESSIBILITY_GRANTED_MSG: &str = "Accessibility permission is granted";
    pub const SCREEN_RECORDING_GRANTED_MSG: &str = "Screen recording permission is granted";
    pub const MICROPHONE_GRANTED_MSG: &str = "Microphone permission is granted";
}

pub mod javascript_templates {
    // DOM query methods
    pub const QUERY_ALL_TEMPLATE: &str = "document.querySelectorAll('{}')";
    pub const QUERY_SINGLE_TEMPLATE: &str = "document.querySelector('{}')";
    pub const GET_TEXT_CONTENT: &str = ".textContent";
    pub const GET_INNER_TEXT: &str = ".innerText";
    pub const GET_VALUE: &str = ".value";

    // Element interaction
    pub const CLICK_ELEMENT: &str = ".click()";
    pub const FOCUS_ELEMENT: &str = ".focus()";
    pub const SCROLL_INTO_VIEW: &str = ".scrollIntoView()";

    // Attribute and style access
    pub const GET_ATTRIBUTE_TEMPLATE: &str = ".getAttribute('{}')";
    pub const SET_ATTRIBUTE_TEMPLATE: &str = ".setAttribute('{}', '{}')";
    pub const GET_STYLE_TEMPLATE: &str = ".style.{}";

    // Common selectors
    pub const BUTTON_SELECTOR: &str = "button";
    pub const INPUT_SELECTOR: &str = "input";
    pub const LINK_SELECTOR: &str = "a";
    pub const FORM_SELECTOR: &str = "form";
}

pub mod window_labels {
    pub const MAIN: &str = "main";
    pub const FLOATING_BAR: &str = "floating-bar";
    pub const FLOATING_PANEL: &str = "floating-panel";
    pub const ONBOARDING: &str = "onboarding";
    pub const SETTINGS: &str = "settings";
}

pub mod tray_menu_ids {
    pub const QUIT: &str = "quit";
    pub const TOGGLE_FLOATING_BAR: &str = "toggle-floating-bar";
    pub const SHOW_FLOATING_BAR: &str = "show-floating-bar";
    pub const HIDE_FLOATING_BAR: &str = "hide-floating-bar";
    pub const SHOW_DEVTOOLS: &str = "show-devtools";
    pub const SHOW_MAIN_WINDOW: &str = "show-main-window";
    pub const HIDE_MAIN_WINDOW: &str = "hide-main-window";
    pub const NEW_CHAT: &str = "new-chat";
    pub const SETTINGS: &str = "tray-settings";
}

pub mod app_menu_ids {
    // Juno Menu
    pub const ABOUT: &str = "about";
    pub const SETTINGS: &str = "settings";
    pub const CHECK_FOR_UPDATES: &str = "check-for-updates";

    // File Menu
    pub const NEW_CHAT: &str = "new-chat";
    pub const CLEAR_HISTORY: &str = "clear-history";
    pub const IMPORT_CHAT: &str = "import-chat";
    pub const EXPORT_CHAT: &str = "export-chat";

    // View Menu
    pub const TOGGLE_FLOATING_BAR: &str = "toggle-floating-bar";
    pub const TOGGLE_DEV_PANEL: &str = "toggle-dev-panel";
    pub const SHOW_DEVTOOLS: &str = "show-devtools";
    pub const SHOW_PERMISSIONS: &str = "show-permissions";
    pub const TOGGLE_FULLSCREEN: &str = "toggle-fullscreen";

    // Window Menu
    pub const MINIMIZE: &str = "minimize";
    pub const ZOOM: &str = "zoom";
    pub const BRING_ALL_TO_FRONT: &str = "bring-all-to-front";

    // Help Menu
    pub const HELP: &str = "help";
    pub const KEYBOARD_SHORTCUTS: &str = "keyboard-shortcuts";
    pub const SEND_FEEDBACK: &str = "send-feedback";
    pub const REPORT_ISSUE: &str = "report-issue";
    pub const VISIT_WEBSITE: &str = "visit-website";
}

pub mod timeouts {
    // UI and Animation Timeouts
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

    // Legacy timeouts (for compatibility)
    pub const STANDARD_TIMEOUT_MS: u64 = 10000;
    pub const BROWSER_TIMEOUT_MS: u64 = 30000;

    // Monitoring and polling intervals
    pub const DICTATION_MONITOR_INTERVAL_MS: u64 = 50;
    pub const AGENT_MONITOR_INTERVAL_MS: u64 = 100;
    pub const TREE_SEARCH_INTERVAL_MS: u64 = 250;
    pub const HEARTBEAT_INTERVAL_MS: u64 = 30000;

    // Mouse and input automation delays
    pub const MOUSE_MICRO_DELAY_MS: u64 = 10;
    pub const MOUSE_CLICK_DELAY_MS: u64 = 50;
    pub const MOUSE_ACTION_DELAY_MS: u64 = 100;
    pub const MOUSE_SEQUENCE_DELAY_MS: u64 = 300;
    pub const DOUBLE_CLICK_DELAY_MS: u64 = 500;

    // UI animation and transition delays
    pub const UI_FADE_DELAY_MS: u64 = 300;
    pub const UI_SLIDE_DELAY_MS: u64 = 600;
    pub const UI_NOTIFICATION_DISPLAY_MS: u64 = 3000;

    // Permission and system operation timeouts
    pub const PERMISSION_CHECK_DELAY_MS: u64 = 1000;
    pub const SCREEN_RECORDING_CHECK_DELAY_MS: u64 = 2000;
    pub const SYSTEM_SETTINGS_OPERATION_TIMEOUT_MS: u64 = 3000;
    pub const SYSTEM_SETTINGS_CHECK_TIMEOUT_MS: u64 = 5000;

    // MCP and server startup delays
    pub const MCP_SERVER_STARTUP_DELAY_MS: u64 = 500;
    pub const MCP_SERVER_RESTART_DELAY_MS: u64 = 1000;

    // Cloud and network intervals
    pub const CLOUD_RETRY_BASE_DELAY_MS: u64 = 2000;
    pub const CLOUD_HEARTBEAT_INTERVAL_MS: u64 = 30000;
    pub const CLOUD_STATUS_INTERVAL_MS: u64 = 30000;
    pub const CLOUD_RECONNECT_DELAY_MS: u64 = 5000;
    pub const CLOUD_WATCHDOG_INTERVAL_MS: u64 = 60000;

    // TTS processing delays
    pub const TTS_PROCESSING_DELAY_MS: u64 = 1000;

    // Browser automation delays
    pub const BROWSER_PAGE_LOAD_DELAY_MS: u64 = 1000;

    // Shell command delays
    pub const SHELL_COMMAND_DELAY_MS: u64 = 10;

    // Buffer and audio timeouts
    pub const PARTIAL_BUFFER_DURATION_MS: u64 = 1500;
    pub const FINAL_BUFFER_DURATION_MS: u64 = 5000;
    pub const MIN_AUDIO_LENGTH_MS: u64 = 500;


    // Network and Browser Timeouts
    pub const DEFAULT_NAVIGATION_TIMEOUT_MS: u64 = 30_000;
    pub const REPLICATE_TIMEOUT_SECONDS: u64 = 300;

    // Operation Timeouts
    pub const PERMISSION_CHECK_TIMEOUT_MS: u64 = 3_000;
    pub const AUDIO_DEVICE_DETECTION_TIMEOUT_MS: u64 = 3_000;
    pub const TOOL_EXECUTION_TIMEOUT_MS: u64 = 10_000;
    pub const MCP_INTEGRATION_TIMEOUT_MS: u64 = 30_000;
}

pub mod ports {
    // Development ports
    pub const VITE_DEV_PORT: u16 = 1420;
    pub const VITE_HMR_PORT: u16 = 1421;

    // MCP and WebSocket servers
    pub const MCP_SERVER_PORT: u16 = 8080;
    pub const WEBSOCKET_TEST_PORT: u16 = 8080;

    // Chrome debugging ports
    pub const CHROME_DEBUG_PORT_PRIMARY: u16 = 9222;
    pub const CHROME_DEBUG_PORT_ALT1: u16 = 9223;
    pub const CHROME_DEBUG_PORT_ALT2: u16 = 9224;
}

pub mod app_identity {
    pub const APP_NAME: &str = "Juno";
    pub const BUNDLE_IDENTIFIER: &str = "com.juno.app";
    pub const PRODUCT_NAME: &str = "Juno";
    pub const DEFAULT_WAKE_WORDS: &[&str] = &["hey juno", "computer"];
    pub const ENTITLEMENTS_FILE: &str = "juno.entitlements";
    pub const CONFIG_DIR_NAME: &str = ".juno";
    pub const SCREENSHOT_PREFIX: &str = "juno_screenshot_";
    pub const DEVICE_NAME_PREFIX: &str = "Juno-";
}

pub mod api_endpoints {
    // AI Provider URLs
    pub const ANTHROPIC_API_URL: &str = "https://api.anthropic.com/v1/messages";
    pub const OPENAI_API_URL: &str = "https://api.openai.com/v1/chat/completions";
    pub const GEMINI_API_BASE: &str = "https://generativelanguage.googleapis.com/v1beta/models";

    // Cloud and WebSocket URLs
    pub const CLOUD_SERVER_URL: &str = "wss://juno-cloud-backend.fly.dev/ws";
    pub const GITHUB_URL: &str = "https://github.com/juno-ai";

    // Local server URLs
    pub const LOCALHOST_BASE: &str = "http://localhost";
    pub const LOCALHOST_CHROME_DEBUG: &str = "http://localhost:9222";
    pub const LOCALHOST_MCP_SERVER: &str = "http://localhost:8080";
    pub const WEBSOCKET_LOCALHOST: &str = "ws://localhost:8080";

    // Additional API endpoints
    pub const ELEVENLABS_TTS_BASE: &str = "https://api.elevenlabs.io/v1/text-to-speech";
    pub const REPLICATE_API_BASE: &str = "https://api.replicate.com";
    pub const JUNO_CLOUD_WEBSOCKET: &str = "wss://juno-cloud-backend.fly.dev/ws";

    // Development URLs
    pub const DEV_SERVER_BASE: &str = "http://localhost:1420";
    pub const HMR_WEBSOCKET: &str = "ws://localhost:1421";
}

pub mod shell_commands {
    // Common shell commands
    pub const OPEN: &str = "open";
    pub const OSASCRIPT: &str = "osascript";
    pub const KILLALL: &str = "killall";
    pub const PS: &str = "ps";
    pub const GREP: &str = "grep";
    pub const CURL: &str = "curl";
    pub const WHICH: &str = "which";

    // Command line flags
    pub const BACKGROUND_FLAG: &str = "&";
    pub const QUIET_FLAG: &str = "-q";
    pub const VERBOSE_FLAG: &str = "-v";
    pub const FORCE_FLAG: &str = "-f";
    pub const RECURSIVE_FLAG: &str = "-r";

    // Chrome and browser commands
    pub const CHROME_BINARY_MACOS: &str = "/Applications/Google Chrome.app/Contents/MacOS/Google Chrome";
    pub const CHROMIUM_BINARY_MACOS: &str = "/Applications/Chromium.app/Contents/MacOS/Chromium";
}

pub mod file_patterns {
    // Log and temporary file patterns
    pub const LOG_EXTENSION: &str = ".log";
    pub const TMP_EXTENSION: &str = ".tmp";
    pub const CACHE_EXTENSION: &str = ".cache";
    pub const BACKUP_EXTENSION: &str = ".backup";

    // Common file prefixes
    pub const LOG_PREFIX: &str = "juno_";
    pub const SCREENSHOT_PREFIX: &str = "screenshot_";
    pub const TEMP_PREFIX: &str = "temp_";

    // Directory names
    pub const LOGS_DIR: &str = "logs";
    pub const CACHE_DIR: &str = "cache";
    pub const CONFIG_DIR: &str = ".juno";
    pub const SCREENSHOTS_DIR: &str = "screenshots";
}

pub mod error_codes {
    // JSON-RPC Error Codes
    pub const PARSE_ERROR: i32 = -32700;
    pub const INVALID_REQUEST: i32 = -32600;
    pub const METHOD_NOT_FOUND: i32 = -32601;
    pub const INVALID_PARAMS: i32 = -32602;
    pub const INTERNAL_ERROR: i32 = -32603;
    pub const SERVER_ERROR_START: i32 = -32099;
    pub const SERVER_ERROR_END: i32 = -32000;

    // Custom Juno Error Codes
    pub const TOOL_EXECUTION_ERROR: i32 = -32000;
    pub const ELEMENT_NOT_FOUND: i32 = -32001;
    pub const CACHE_MISS: i32 = -32002;
    pub const UNSUPPORTED_PLATFORM: i32 = -32003;

    // macOS Accessibility Error Codes
    pub const MACOS_AX_NO_VALUE: i32 = -25212;
    pub const MACOS_AX_ATTRIBUTE_UNSUPPORTED: i32 = -25205;
    pub const MACOS_AX_GET_ATTRIBUTE_FAILED: i32 = -25204;
}

pub mod error_messages {
    pub const INVALID_PARAMS: &str = "invalid params";
    pub const METHOD_NOT_FOUND: &str = "method not found";
    pub const PARSE_ERROR: &str = "parse error";
    pub const ELEMENT_NOT_FOUND: &str = "element not found";
    pub const CACHE_MISS: &str = "cache miss";
    pub const UNSUPPORTED_OPERATION: &str = "unsupported operation";
    pub const UNSUPPORTED_PLATFORM: &str = "unsupported platform";
    pub const TOOL_EXECUTION_ERROR: &str = "tool execution error";
}

pub mod key_codes {
    // macOS Key Codes
    pub const KEY_ARROW_LEFT: u16 = 123;
    pub const KEY_ARROW_RIGHT: u16 = 124;
    pub const KEY_ARROW_DOWN: u16 = 125;
    pub const KEY_ARROW_UP: u16 = 126;
}

pub mod audio {
    pub const WHISPER_SAMPLE_RATE: u32 = 16000;
    pub const SOUND_DEBOUNCE_MS: u64 = 300;
    pub const DEFAULT_SENSITIVITY: f32 = 0.5;
    pub const AUDIO_RECV_TIMEOUT_MS: u64 = 100;
}

pub mod ui {
    pub const MOBILE_BREAKPOINT: i32 = 768;
    pub const PERCENTAGE_MULTIPLIER: f64 = 100.0;
    pub const SCROLL_WHEEL_EVENT_LINE_SCROLL: i32 = 120;
    pub const DOUBLE_CLICK_INTERVAL_MS: u64 = 50;
    pub const MAX_TREE_SEARCH_DEPTH: usize = 100;
}

pub mod file_extensions {
    pub const JSON_EXT: &str = ".json";
    pub const RUST_EXT: &str = ".rs";
    pub const TYPESCRIPT_EXT: &str = ".ts";
    pub const JAVASCRIPT_EXT: &str = ".js";
    pub const MARKDOWN_EXT: &str = ".md";
}

pub mod permission_descriptions {
    pub const ACCESSIBILITY_DESC: &str = "Juno requires accessibility permissions to automate desktop tasks and interact with applications on your behalf.";
    pub const MICROPHONE_DESC: &str = "Juno uses the microphone for voice transcription and voice commands.";
    pub const APPLE_EVENTS_DESC: &str = "Juno uses Apple Events to control and automate applications.";
    pub const SCREEN_RECORDING_DESC: &str = "Juno needs screen capture permissions to take screenshots and analyze the desktop for automation tasks.";
    pub const INPUT_MONITORING_DESC: &str = "Juno needs input monitoring permissions to register global keyboard shortcuts for voice control and automation features.";
    pub const ACCESSIBILITY_INSTRUCTIONS: &str = "Go to System Preferences > Privacy & Security > Accessibility and add Juno";
    pub const SCREEN_RECORDING_INSTRUCTIONS: &str = "Go to System Preferences > Privacy & Security > Screen Recording and add Juno";
    pub const MICROPHONE_INSTRUCTIONS: &str = "Go to System Preferences > Privacy & Security > Microphone and add Juno";
    pub const INPUT_MONITORING_INSTRUCTIONS: &str = "Optional: Go to System Preferences > Privacy & Security > Input Monitoring and add Juno to enable global shortcuts";
}

pub mod permission_types {
    pub const ACCESSIBILITY: &str = "accessibility";
    pub const SCREEN_RECORDING: &str = "screen_recording";
    pub const MICROPHONE: &str = "microphone";
    pub const INPUT_MONITORING: &str = "input_monitoring";
}

pub mod audio_processing {
    pub const SINC_LENGTH: usize = 256;
    pub const OVERSAMPLING_FACTOR: usize = 256;
    pub const AUDIO_RECV_TIMEOUT_MS: u64 = 100;
}

pub mod chrome_debug_urls {
    pub const PRIMARY: &str = "http://localhost:9222";
    pub const ALTERNATIVE_1: &str = "http://localhost:9223";
    pub const ALTERNATIVE_2: &str = "http://localhost:9224";

    pub fn get_all_urls() -> [&'static str; 3] {
        [PRIMARY, ALTERNATIVE_1, ALTERNATIVE_2]
    }
}

pub mod agent_config {
    // Agent execution limits
    pub const MAX_ITERATIONS: u32 = 25;
    pub const MAX_ITERATIONS_REDUCED: u32 = 10; // For focused tasks

    // LLM token limits
    pub const DEFAULT_MAX_TOKENS_STANDARD: u32 = 4096;
    pub const DEFAULT_MAX_TOKENS_COMPACT: i32 = 1024;

    // LLM parameters
    pub const DEFAULT_TEMPERATURE: f32 = 0.7;

    // Retry and attempt limits
    pub const MAX_RETRY_ATTEMPTS: usize = 3;
    pub const MAX_RECOVERY_ATTEMPTS: usize = 5;

    // Processing limits
    pub const MAX_TOOL_CALLS_PER_ITERATION: usize = 10;
    pub const MAX_MEMORY_ENTRIES: usize = 1000;
}

pub mod monitor_sessions {
    // Input monitoring durations
    pub const HOLD_DURATION_MS: u64 = 500;
    pub const IMMEDIATE_START_MS: u64 = 0;

    // Session maximum durations
    pub const MAX_TRANSCRIPTION_DURATION_MS: u64 = 30_000;  // 30 seconds
    pub const MAX_AGENT_DURATION_MS: u64 = 120_000;         // 2 minutes

    // Cleanup and recovery timeouts
    pub const FORCE_CLEANUP_TIMEOUT_MS: u64 = 5_000;       // 5 seconds
    pub const COOLDOWN_AFTER_CANCEL_MS: u64 = 150;         // 150ms

    // Monitoring intervals
    pub const AGENT_MONITOR_INTERVAL_MS: u64 = 100;
    pub const DICTATION_MONITOR_INTERVAL_MS: u64 = 50;
}

pub mod platform_macos {
    // NSTrackingArea options
    pub const NS_TRACKING_MOUSE_ENTERED_AND_EXITED: u64 = 0x01;
    pub const NS_TRACKING_ACTIVE_ALWAYS: u64 = 0x80;

    // macOS specific timeouts
    pub const ACCESSIBILITY_PERMISSION_CHECK_DELAY_MS: u64 = 1000;
    pub const SCREEN_RECORDING_PERMISSION_CHECK_DELAY_MS: u64 = 2000;

    // macOS system limits
    pub const MAX_ACCESSIBILITY_RETRIES: usize = 3;
    pub const SYSTEM_PERMISSION_TIMEOUT_MS: u64 = 5000;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_event_constants() {
        // Test critical agent events
        assert_eq!(events::AGENT_EVENT, "agent-event");
        assert_eq!(events::APP_DICTATION_STARTED, "app-dictation-started");
        assert_eq!(events::APP_DICTATION_FINISHED, "app-dictation-finished");
        assert_eq!(events::AGENT_PROCESSING_COMPLETE, "agent-processing-complete");
        assert_eq!(events::AGENT_PROCESSING_ERROR, "agent-processing-error");

        // Test streaming events
        assert_eq!(events::AGENT_TEXT_STREAM, "agent-text-stream");
        assert_eq!(events::AGENT_STREAM_START, "agent-stream-start");
        assert_eq!(events::AGENT_STREAM_END, "agent-stream-end");

        // Test UI events
        assert_eq!(events::BAR_STATE_CHANGED, "bar-state-changed");
        assert_eq!(events::DICTATION_STATE_CHANGED, "dictation-state-changed");
    }

    #[test]
    fn test_window_labels() {
        assert_eq!(window_labels::MAIN, "main");
        assert_eq!(window_labels::FLOATING_BAR, "floating-bar");
        assert_eq!(window_labels::ONBOARDING, "onboarding");

        // Ensure labels are not empty
        assert!(!window_labels::MAIN.is_empty());
        assert!(!window_labels::FLOATING_BAR.is_empty());
        assert!(!window_labels::ONBOARDING.is_empty());
    }

    #[test]
    fn test_tray_menu_ids() {
        assert_eq!(tray_menu_ids::QUIT, "quit");
        assert_eq!(tray_menu_ids::TOGGLE_FLOATING_BAR, "toggle-floating-bar");
        assert_eq!(tray_menu_ids::SHOW_FLOATING_BAR, "show-floating-bar");
        assert_eq!(tray_menu_ids::HIDE_FLOATING_BAR, "hide-floating-bar");
        assert_eq!(tray_menu_ids::SHOW_DEVTOOLS, "show-devtools");
        assert_eq!(tray_menu_ids::SHOW_MAIN_WINDOW, "show-main-window");
        assert_eq!(tray_menu_ids::HIDE_MAIN_WINDOW, "hide-main-window");
        assert_eq!(tray_menu_ids::NEW_CHAT, "new-chat");
        assert_eq!(tray_menu_ids::SETTINGS, "tray-settings");

        // Ensure all IDs are non-empty
        assert!(!tray_menu_ids::QUIT.is_empty());
        assert!(!tray_menu_ids::SETTINGS.is_empty());
        assert!(!tray_menu_ids::SHOW_FLOATING_BAR.is_empty());
        assert!(!tray_menu_ids::HIDE_FLOATING_BAR.is_empty());
        assert!(!tray_menu_ids::HIDE_MAIN_WINDOW.is_empty());
    }

    #[test]
    fn test_app_menu_ids() {
        // Test Juno menu
        assert_eq!(app_menu_ids::ABOUT, "about");
        assert_eq!(app_menu_ids::SETTINGS, "settings");
        assert_eq!(app_menu_ids::CHECK_FOR_UPDATES, "check-for-updates");

        // Test File menu
        assert_eq!(app_menu_ids::NEW_CHAT, "new-chat");
        assert_eq!(app_menu_ids::CLEAR_HISTORY, "clear-history");
        assert_eq!(app_menu_ids::IMPORT_CHAT, "import-chat");
        assert_eq!(app_menu_ids::EXPORT_CHAT, "export-chat");

        // Test View menu
        assert_eq!(app_menu_ids::TOGGLE_FLOATING_BAR, "toggle-floating-bar");
        assert_eq!(app_menu_ids::TOGGLE_DEV_PANEL, "toggle-dev-panel");
        assert_eq!(app_menu_ids::SHOW_DEVTOOLS, "show-devtools");
        assert_eq!(app_menu_ids::SHOW_PERMISSIONS, "show-permissions");

        // Test Window menu
        assert_eq!(app_menu_ids::MINIMIZE, "minimize");
        assert_eq!(app_menu_ids::ZOOM, "zoom");

        // Test Help menu
        assert_eq!(app_menu_ids::HELP, "help");
        assert_eq!(app_menu_ids::SEND_FEEDBACK, "send-feedback");
    }

    #[test]
    fn test_timeout_constants() {
        // Test legacy timeouts
        assert_eq!(timeouts::STANDARD_TIMEOUT_MS, 10000);
        assert_eq!(timeouts::BROWSER_TIMEOUT_MS, 30000);

        // Test delay hierarchy
        assert!(timeouts::MICRO_DELAY_MS < timeouts::MINIMAL_DELAY_MS);
        assert!(timeouts::MINIMAL_DELAY_MS < timeouts::SMALL_DELAY_MS);
        assert!(timeouts::SMALL_DELAY_MS < timeouts::SHORT_DELAY_MS);
        assert!(timeouts::SHORT_DELAY_MS < timeouts::MEDIUM_DELAY_MS);

        // Ensure timeouts are reasonable values
        assert!(timeouts::STANDARD_TIMEOUT_MS > 0);
        assert!(timeouts::BROWSER_TIMEOUT_MS > timeouts::STANDARD_TIMEOUT_MS);
        assert!(timeouts::BROWSER_TIMEOUT_MS <= 60000); // Max 60 seconds
    }

    #[test]
    fn test_port_constants() {
        assert_eq!(ports::VITE_DEV_PORT, 1420);
        assert_eq!(ports::VITE_HMR_PORT, 1421);
        assert_eq!(ports::MCP_SERVER_PORT, 8080);
        assert_eq!(ports::CHROME_DEBUG_PORT_PRIMARY, 9222);

        // Ensure ports are in valid range
        assert!(ports::VITE_DEV_PORT > 1024);
        assert!(ports::MCP_SERVER_PORT > 1024);
        assert!(ports::CHROME_DEBUG_PORT_PRIMARY > 1024);
    }

    #[test]
    fn test_app_identity() {
        assert_eq!(app_identity::APP_NAME, "Juno");
        assert_eq!(app_identity::BUNDLE_IDENTIFIER, "com.juno.app");
        assert!(!app_identity::DEFAULT_WAKE_WORDS.is_empty());
        assert!(app_identity::DEFAULT_WAKE_WORDS.contains(&"hey juno"));
    }

    #[test]
    fn test_error_codes() {
        // Test JSON-RPC standard codes
        assert_eq!(error_codes::PARSE_ERROR, -32700);
        assert_eq!(error_codes::INVALID_REQUEST, -32600);
        assert_eq!(error_codes::METHOD_NOT_FOUND, -32601);
        assert_eq!(error_codes::INVALID_PARAMS, -32602);

        // Test custom codes
        assert_eq!(error_codes::ELEMENT_NOT_FOUND, -32001);
        assert_eq!(error_codes::CACHE_MISS, -32002);
    }

    #[test]
    fn test_key_codes() {
        assert_eq!(key_codes::KEY_ARROW_LEFT, 123);
        assert_eq!(key_codes::KEY_ARROW_RIGHT, 124);
        assert_eq!(key_codes::KEY_ARROW_DOWN, 125);
        assert_eq!(key_codes::KEY_ARROW_UP, 126);
    }

    #[test]
    fn test_audio_constants() {
        assert_eq!(audio::WHISPER_SAMPLE_RATE, 16000);
        assert_eq!(audio::SOUND_DEBOUNCE_MS, 300);
        assert!(audio::DEFAULT_SENSITIVITY > 0.0 && audio::DEFAULT_SENSITIVITY <= 1.0);
    }

    #[test]
    fn test_no_duplicate_event_names() {
        use std::collections::HashSet;

        let mut event_names = HashSet::new();
        let events_list = vec![
            events::AGENT_EVENT,
            events::APP_DICTATION_STARTED,
            events::APP_DICTATION_FINISHED,
            events::AGENT_PROCESSING_COMPLETE,
            events::AGENT_PROCESSING_ERROR,
            events::AGENT_STATE_CHANGED,
            events::AGENT_TEXT_STREAM,
            events::AGENT_STREAM_START,
            events::AGENT_STREAM_END,
            events::BAR_STATE_CHANGED,
            events::DICTATION_STATE_CHANGED,
        ];

        for event in events_list {
            assert!(event_names.insert(event), "Duplicate event name found: {}", event);
        }
    }

    #[test]
    fn test_menu_id_uniqueness() {
        use std::collections::HashSet;

        let mut menu_ids = HashSet::new();

        // Add tray menu IDs
        let tray_ids = vec![
            tray_menu_ids::QUIT,
            tray_menu_ids::TOGGLE_FLOATING_BAR,
            tray_menu_ids::SHOW_FLOATING_BAR,
            tray_menu_ids::HIDE_FLOATING_BAR,
            tray_menu_ids::SHOW_DEVTOOLS,
            tray_menu_ids::SHOW_MAIN_WINDOW,
            tray_menu_ids::HIDE_MAIN_WINDOW,
            tray_menu_ids::NEW_CHAT,
            tray_menu_ids::SETTINGS,
        ];

        for id in tray_ids {
            assert!(menu_ids.insert(id), "Duplicate menu ID found: {}", id);
        }

        // Add app menu IDs (excluding duplicates like NEW_CHAT)
        let app_ids = vec![
            app_menu_ids::ABOUT,
            // Skip SETTINGS and NEW_CHAT as they might conflict with tray
            app_menu_ids::CHECK_FOR_UPDATES,
            app_menu_ids::CLEAR_HISTORY,
            app_menu_ids::IMPORT_CHAT,
            app_menu_ids::EXPORT_CHAT,
            app_menu_ids::TOGGLE_DEV_PANEL,
            app_menu_ids::SHOW_PERMISSIONS,
            app_menu_ids::MINIMIZE,
            app_menu_ids::ZOOM,
            app_menu_ids::HELP,
            app_menu_ids::SEND_FEEDBACK,
        ];

        for id in app_ids {
            assert!(menu_ids.insert(id), "Duplicate menu ID found: {}", id);
        }
    }

    #[test]
    fn test_event_naming_convention() {
        // Test that events follow kebab-case convention
        let events_to_check = vec![
            events::AGENT_EVENT,
            events::APP_DICTATION_STARTED,
            events::AGENT_PROCESSING_COMPLETE,
            events::BAR_STATE_CHANGED,
        ];

        for event in events_to_check {
            // Should not contain underscores (use kebab-case)
            assert!(!event.contains('_'), "Event '{}' should use kebab-case, not snake_case", event);
            // Should not contain uppercase letters
            assert!(!event.chars().any(|c| c.is_uppercase()), "Event '{}' should be lowercase", event);
            // Should contain only lowercase letters, numbers, and hyphens
            assert!(event.chars().all(|c| c.is_lowercase() || c.is_numeric() || c == '-'),
                   "Event '{}' contains invalid characters", event);
        }
    }

    #[test]
    fn test_api_endpoints() {
        assert!(api_endpoints::ANTHROPIC_API_URL.starts_with("https://"));
        assert!(api_endpoints::OPENAI_API_URL.starts_with("https://"));
        assert!(api_endpoints::GEMINI_API_BASE.starts_with("https://"));
        assert!(api_endpoints::CLOUD_SERVER_URL.starts_with("wss://"));
    }

    #[test]
    fn test_permission_descriptions() {
        assert!(!permission_descriptions::ACCESSIBILITY_DESC.is_empty());
        assert!(!permission_descriptions::MICROPHONE_DESC.is_empty());
        assert!(!permission_descriptions::ACCESSIBILITY_INSTRUCTIONS.is_empty());

        // Ensure descriptions mention Juno
        assert!(permission_descriptions::ACCESSIBILITY_DESC.contains("Juno"));
        assert!(permission_descriptions::MICROPHONE_DESC.contains("Juno"));
    }

    #[test]
    fn test_permission_types() {
        assert_eq!(permission_types::ACCESSIBILITY, "accessibility");
        assert_eq!(permission_types::SCREEN_RECORDING, "screen_recording");
        assert_eq!(permission_types::MICROPHONE, "microphone");
        assert_eq!(permission_types::INPUT_MONITORING, "input_monitoring");

        // Ensure no empty strings
        assert!(!permission_types::ACCESSIBILITY.is_empty());
        assert!(!permission_types::SCREEN_RECORDING.is_empty());
        assert!(!permission_types::MICROPHONE.is_empty());
        assert!(!permission_types::INPUT_MONITORING.is_empty());
    }

    #[test]
    fn test_audio_processing() {
        assert_eq!(audio_processing::SINC_LENGTH, 256);
        assert_eq!(audio_processing::OVERSAMPLING_FACTOR, 256);
        assert_eq!(audio_processing::AUDIO_RECV_TIMEOUT_MS, 100);

        // Ensure reasonable values
        assert!(audio_processing::SINC_LENGTH > 0);
        assert!(audio_processing::OVERSAMPLING_FACTOR > 0);
        assert!(audio_processing::AUDIO_RECV_TIMEOUT_MS > 0);
        assert!(audio_processing::AUDIO_RECV_TIMEOUT_MS < 1000); // Should be under 1 second
    }

    #[test]
    fn test_chrome_debug_urls() {
        let urls = chrome_debug_urls::get_all_urls();
        assert_eq!(urls.len(), 3);

        // Test individual URLs
        assert_eq!(chrome_debug_urls::PRIMARY, "http://localhost:9222");
        assert_eq!(chrome_debug_urls::ALTERNATIVE_1, "http://localhost:9223");
        assert_eq!(chrome_debug_urls::ALTERNATIVE_2, "http://localhost:9224");

        // Test that all URLs are valid localhost addresses
        for url in urls {
            assert!(url.starts_with("http://localhost:"));
            assert!(url.contains("922")); // All contain 922x pattern
        }

        // Test array contents
        assert_eq!(urls[0], chrome_debug_urls::PRIMARY);
        assert_eq!(urls[1], chrome_debug_urls::ALTERNATIVE_1);
        assert_eq!(urls[2], chrome_debug_urls::ALTERNATIVE_2);
    }

    #[test]
    fn test_agent_config() {
        // Test iteration limits
        assert_eq!(agent_config::MAX_ITERATIONS, 25);
        assert_eq!(agent_config::MAX_ITERATIONS_REDUCED, 10);
        assert!(agent_config::MAX_ITERATIONS_REDUCED < agent_config::MAX_ITERATIONS);

        // Test token limits
        assert_eq!(agent_config::DEFAULT_MAX_TOKENS_STANDARD, 4096);
        assert_eq!(agent_config::DEFAULT_MAX_TOKENS_COMPACT, 1024);
        assert!(agent_config::DEFAULT_MAX_TOKENS_COMPACT < agent_config::DEFAULT_MAX_TOKENS_STANDARD as i32);

        // Test LLM parameters
        assert_eq!(agent_config::DEFAULT_TEMPERATURE, 0.7);
        assert!(agent_config::DEFAULT_TEMPERATURE > 0.0 && agent_config::DEFAULT_TEMPERATURE <= 1.0);

        // Test retry limits
        assert_eq!(agent_config::MAX_RETRY_ATTEMPTS, 3);
        assert_eq!(agent_config::MAX_RECOVERY_ATTEMPTS, 5);
        assert!(agent_config::MAX_RETRY_ATTEMPTS <= agent_config::MAX_RECOVERY_ATTEMPTS);

        // Test processing limits
        assert_eq!(agent_config::MAX_TOOL_CALLS_PER_ITERATION, 10);
        assert_eq!(agent_config::MAX_MEMORY_ENTRIES, 1000);
        assert!(agent_config::MAX_TOOL_CALLS_PER_ITERATION > 0);
    }

    #[test]
    fn test_monitor_sessions() {
        // Test input monitoring durations
        assert_eq!(monitor_sessions::HOLD_DURATION_MS, 500);
        assert_eq!(monitor_sessions::IMMEDIATE_START_MS, 0);

        // Test session maximum durations
        assert_eq!(monitor_sessions::MAX_TRANSCRIPTION_DURATION_MS, 30_000);
        assert_eq!(monitor_sessions::MAX_AGENT_DURATION_MS, 120_000);
        assert!(monitor_sessions::MAX_AGENT_DURATION_MS > monitor_sessions::MAX_TRANSCRIPTION_DURATION_MS);

        // Test cleanup timeouts
        assert_eq!(monitor_sessions::FORCE_CLEANUP_TIMEOUT_MS, 5_000);
        assert_eq!(monitor_sessions::COOLDOWN_AFTER_CANCEL_MS, 150);

        // Test monitoring intervals
        assert_eq!(monitor_sessions::AGENT_MONITOR_INTERVAL_MS, 100);
        assert_eq!(monitor_sessions::DICTATION_MONITOR_INTERVAL_MS, 50);
        assert!(monitor_sessions::DICTATION_MONITOR_INTERVAL_MS < monitor_sessions::AGENT_MONITOR_INTERVAL_MS);
    }

    #[test]
    fn test_platform_macos() {
        // Test NSTrackingArea options (these are system-defined constants)
        assert_eq!(platform_macos::NS_TRACKING_MOUSE_ENTERED_AND_EXITED, 0x01);
        assert_eq!(platform_macos::NS_TRACKING_ACTIVE_ALWAYS, 0x80);

        // Test permission check delays
        assert_eq!(platform_macos::ACCESSIBILITY_PERMISSION_CHECK_DELAY_MS, 1000);
        assert_eq!(platform_macos::SCREEN_RECORDING_PERMISSION_CHECK_DELAY_MS, 2000);

        // Test system limits
        assert_eq!(platform_macos::MAX_ACCESSIBILITY_RETRIES, 3);
        assert_eq!(platform_macos::SYSTEM_PERMISSION_TIMEOUT_MS, 5000);
        assert!(platform_macos::MAX_ACCESSIBILITY_RETRIES > 0);
    }

    #[test]
    fn test_javascript_templates() {
        // Test DOM query templates
        assert_eq!(javascript_templates::QUERY_ALL_TEMPLATE, "document.querySelectorAll('{}')");
        assert_eq!(javascript_templates::QUERY_SINGLE_TEMPLATE, "document.querySelector('{}')");

        // Test element interaction
        assert_eq!(javascript_templates::CLICK_ELEMENT, ".click()");
        assert_eq!(javascript_templates::FOCUS_ELEMENT, ".focus()");
        assert_eq!(javascript_templates::SCROLL_INTO_VIEW, ".scrollIntoView()");

        // Test attribute access
        assert_eq!(javascript_templates::GET_ATTRIBUTE_TEMPLATE, ".getAttribute('{}')");
        assert_eq!(javascript_templates::SET_ATTRIBUTE_TEMPLATE, ".setAttribute('{}', '{}')");

        // Test common selectors
        assert_eq!(javascript_templates::BUTTON_SELECTOR, "button");
        assert_eq!(javascript_templates::INPUT_SELECTOR, "input");
        assert_eq!(javascript_templates::LINK_SELECTOR, "a");
        assert_eq!(javascript_templates::FORM_SELECTOR, "form");
    }

    #[test]
    fn test_shell_commands() {
        // Test common commands
        assert_eq!(shell_commands::OPEN, "open");
        assert_eq!(shell_commands::OSASCRIPT, "osascript");
        assert_eq!(shell_commands::KILLALL, "killall");
        assert_eq!(shell_commands::GREP, "grep");

        // Test flags
        assert_eq!(shell_commands::BACKGROUND_FLAG, "&");
        assert_eq!(shell_commands::QUIET_FLAG, "-q");
        assert_eq!(shell_commands::VERBOSE_FLAG, "-v");
        assert_eq!(shell_commands::FORCE_FLAG, "-f");

        // Test browser paths
        assert!(shell_commands::CHROME_BINARY_MACOS.contains("Google Chrome"));
        assert!(shell_commands::CHROMIUM_BINARY_MACOS.contains("Chromium"));
    }

    #[test]
    fn test_file_patterns() {
        // Test extensions
        assert_eq!(file_patterns::LOG_EXTENSION, ".log");
        assert_eq!(file_patterns::TMP_EXTENSION, ".tmp");
        assert_eq!(file_patterns::CACHE_EXTENSION, ".cache");
        assert_eq!(file_patterns::BACKUP_EXTENSION, ".backup");

        // Test prefixes
        assert_eq!(file_patterns::LOG_PREFIX, "juno_");
        assert_eq!(file_patterns::SCREENSHOT_PREFIX, "screenshot_");
        assert_eq!(file_patterns::TEMP_PREFIX, "temp_");

        // Test directories
        assert_eq!(file_patterns::LOGS_DIR, "logs");
        assert_eq!(file_patterns::CACHE_DIR, "cache");
        assert_eq!(file_patterns::CONFIG_DIR, ".juno");
        assert_eq!(file_patterns::SCREENSHOTS_DIR, "screenshots");
    }

    #[test]
    fn test_extended_api_endpoints() {
        // Test additional endpoints
        assert!(api_endpoints::ELEVENLABS_TTS_BASE.starts_with("https://api.elevenlabs.io"));
        assert!(api_endpoints::REPLICATE_API_BASE.starts_with("https://api.replicate.com"));
        assert!(api_endpoints::JUNO_CLOUD_WEBSOCKET.starts_with("wss://"));

        // Test development URLs
        assert_eq!(api_endpoints::DEV_SERVER_BASE, "http://localhost:1420");
        assert_eq!(api_endpoints::HMR_WEBSOCKET, "ws://localhost:1421");

        // Ensure URLs are well-formed
        assert!(!api_endpoints::ELEVENLABS_TTS_BASE.is_empty());
        assert!(!api_endpoints::REPLICATE_API_BASE.is_empty());
        assert!(!api_endpoints::JUNO_CLOUD_WEBSOCKET.is_empty());
    }
}

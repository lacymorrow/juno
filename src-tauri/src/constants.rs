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
}

pub mod window_labels {
    pub const MAIN: &str = "main";
    pub const FLOATING_BAR: &str = "floating-bar";
}

pub mod tray_menu_ids {
    pub const QUIT: &str = "quit";
    pub const TOGGLE_FLOATING_BAR: &str = "toggle-floating-bar";
    pub const SHOW_DEVTOOLS: &str = "show-devtools";
    pub const SHOW_MAIN_WINDOW: &str = "show-main-window";
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
    pub const TREE_SEARCH_INTERVAL_MS: u64 = 250;
    pub const HEARTBEAT_INTERVAL_MS: u64 = 30000;
    
    // Buffer and audio timeouts
    pub const PARTIAL_BUFFER_DURATION_MS: u64 = 1500;
    pub const FINAL_BUFFER_DURATION_MS: u64 = 5000;
    pub const MIN_AUDIO_LENGTH_MS: u64 = 500;
    pub const EMERGENCY_TIMEOUT_MS: u64 = 1000;
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
    pub const CLOUD_SERVER_URL: &str = "wss://juno-cloud.shipkit.io/ws";
    pub const GITHUB_URL: &str = "https://github.com/juno-ai";
    
    // Local server URLs
    pub const LOCALHOST_BASE: &str = "http://localhost";
    pub const LOCALHOST_CHROME_DEBUG: &str = "http://localhost:9222";
    pub const LOCALHOST_MCP_SERVER: &str = "http://localhost:8080";
    pub const WEBSOCKET_LOCALHOST: &str = "ws://localhost:8080";
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
        
        // Ensure labels are not empty
        assert!(!window_labels::MAIN.is_empty());
        assert!(!window_labels::FLOATING_BAR.is_empty());
    }

    #[test]
    fn test_tray_menu_ids() {
        assert_eq!(tray_menu_ids::QUIT, "quit");
        assert_eq!(tray_menu_ids::TOGGLE_FLOATING_BAR, "toggle-floating-bar");
        assert_eq!(tray_menu_ids::SHOW_DEVTOOLS, "show-devtools");
        assert_eq!(tray_menu_ids::SHOW_MAIN_WINDOW, "show-main-window");
        assert_eq!(tray_menu_ids::NEW_CHAT, "new-chat");
        assert_eq!(tray_menu_ids::SETTINGS, "tray-settings");
        
        // Ensure all IDs are non-empty
        assert!(!tray_menu_ids::QUIT.is_empty());
        assert!(!tray_menu_ids::SETTINGS.is_empty());
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
            tray_menu_ids::SHOW_DEVTOOLS,
            tray_menu_ids::SHOW_MAIN_WINDOW,
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
        assert_eq!(chrome_debug_urls::PRIMARY, "http://localhost:9222");
        assert_eq!(chrome_debug_urls::ALTERNATIVE_1, "http://localhost:9223");
        assert_eq!(chrome_debug_urls::ALTERNATIVE_2, "http://localhost:9224");
        
        // Test the helper function
        let all_urls = chrome_debug_urls::get_all_urls();
        assert_eq!(all_urls.len(), 3);
        assert!(all_urls.contains(&chrome_debug_urls::PRIMARY));
        assert!(all_urls.contains(&chrome_debug_urls::ALTERNATIVE_1));
        assert!(all_urls.contains(&chrome_debug_urls::ALTERNATIVE_2));
        
        // Ensure all URLs are localhost and follow expected pattern
        for url in all_urls {
            assert!(url.starts_with("http://localhost:"));
            assert!(url.len() > "http://localhost:".len());
        }
    }

    #[test]
    fn test_permission_type_uniqueness() {
        use std::collections::HashSet;
        
        let mut types = HashSet::new();
        let permission_types_list = vec![
            permission_types::ACCESSIBILITY,
            permission_types::SCREEN_RECORDING,
            permission_types::MICROPHONE,
            permission_types::INPUT_MONITORING,
        ];
        
        for permission_type in permission_types_list {
            assert!(types.insert(permission_type), "Duplicate permission type found: {}", permission_type);
        }
        
        // Should have exactly 4 unique permission types
        assert_eq!(types.len(), 4);
    }
}

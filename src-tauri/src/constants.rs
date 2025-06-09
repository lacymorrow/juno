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
    pub const STANDARD_TIMEOUT_MS: u64 = 10000;
    pub const BROWSER_TIMEOUT_MS: u64 = 30000;
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
        assert_eq!(timeouts::STANDARD_TIMEOUT_MS, 10000);
        assert_eq!(timeouts::BROWSER_TIMEOUT_MS, 30000);
        
        // Ensure timeouts are reasonable values
        assert!(timeouts::STANDARD_TIMEOUT_MS > 0);
        assert!(timeouts::BROWSER_TIMEOUT_MS > timeouts::STANDARD_TIMEOUT_MS);
        assert!(timeouts::BROWSER_TIMEOUT_MS <= 60000); // Max 60 seconds
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
}

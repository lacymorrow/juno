//! # Event Constants
//!
//! All event names used throughout the application.
//! These are shared between frontend and backend via Tauri events.

/// Agent and AI events
pub mod agent {
    pub const EVENT: &str = "agent-event";
    pub const PROCESSING_COMPLETE: &str = "agent-processing-complete";
    pub const PROCESSING_ERROR: &str = "agent-processing-error";
    pub const STATE_CHANGED: &str = "agent-state-changed";
    pub const TOOL_CALL: &str = "agent-tool-call";
    pub const THOUGHT_PROCESS: &str = "agent-thought-process";
    pub const STOPPING: &str = "agent-stopping";
    pub const STATUS_UPDATE: &str = "agent-status-update";
    pub const STOP_ALL: &str = "agent-stop-all";

    // Agent state events
    pub const ACTIVE: &str = "agent-active";
    pub const ERROR: &str = "agent-error";
    pub const TRANSCRIPTION_START: &str = "agent-transcription-start";
    pub const TRANSCRIPTION_STOP: &str = "agent-transcription-stop";
    pub const CANCEL: &str = "agent-cancel";
    pub const COMMITTED: &str = "agent-committed";
    pub const FORCE_STOP: &str = "agent-force-stop";
    pub const FORCE_CLEANUP: &str = "agent-force-cleanup";
    /// Normalized agent submission event for any source (voice or UI)
    pub const QUERY_READY: &str = "agent-query-ready";
}

/// Streaming events
pub mod streaming {
    pub const TEXT_STREAM: &str = "agent-text-stream";
    pub const STREAM_START: &str = "agent-stream-start";
    pub const STREAM_END: &str = "agent-stream-end";
    // Thinking streaming events
    pub const THINKING_START: &str = "agent-thinking-start";
    pub const THINKING_STREAM: &str = "agent-thinking-stream";
    pub const THINKING_END: &str = "agent-thinking-end";
}

/// Dictation and voice events
pub mod dictation {
    pub const STARTED: &str = "app-dictation-started";
    pub const FINISHED: &str = "app-dictation-finished";
    pub const PARTIAL_RESULT: &str = "app-dictation-partial-result";
    pub const ERROR: &str = "app-dictation-error";
    pub const STATE_CHANGED: &str = "dictation-state-changed";

    // Dictation state events
    pub const ACTIVE: &str = "dictation-active";
    pub const CANCELLED: &str = "dictation-cancelled";
    pub const TRANSCRIPTION_START: &str = "dictation-transcription-start";
    pub const TRANSCRIPTION_STOP: &str = "dictation-transcription-stop";
    pub const COMMITTED: &str = "dictation-committed";
    pub const STOP: &str = "dictation-stop";
    pub const TRANSCRIPTION_CANCEL: &str = "dictation-cancel";
    pub const TRANSCRIPTION_FORCE_STOP: &str = "dictation-transcription-force-stop";
    pub const TRANSCRIPTION_FORCE_CLEANUP: &str = "dictation-transcription-force-cleanup";
}

/// Voice transcription events (from plugin)
pub mod voice_transcription {
    pub const FINAL_RESULT: &str = "voice-transcription:final-result";
    pub const DICTATION_STOPPED: &str = "voice-transcription:dictation-stopped";
    pub const ERROR: &str = "voice-transcription:error";
    // Plugin-specific events
    pub const DICTATION_STARTED: &str = "voice-transcription:dictation-started";
    pub const PARTIAL_RESULT: &str = "voice-transcription:partial-result";
}

/// Timer events
pub mod timer {
    pub const EXPIRED: &str = "timer-expired";
    pub const QUEUED: &str = "timer-queued";
    pub const PROCESSED: &str = "timer-processed";
    pub const STATUS_UPDATE: &str = "timer-status-update";
}

/// Force stop events
pub mod force_stop {
    pub const TRANSCRIPTION: &str = "force-stop-transcription";
}

/// UI and window events
pub mod ui {
    pub const BAR_STATE_CHANGED: &str = "bar-state-changed";
    pub const REQUEST_AUDIO_PLAYBACK_TEST: &str = "request-audio-playback-test";
    pub const KEY_PRESS_VISUALIZATION: &str = "key-press-visualization";
    pub const CLICK_VISUALIZATION: &str = "click-visualization";
    pub const UI_CURSOR_HIGHLIGHT_START: &str = "ui-cursor-highlight-start";
    pub const UI_CURSOR_HIGHLIGHT_MOVE: &str = "ui-cursor-highlight-move";
    pub const UI_CURSOR_HIGHLIGHT_STOP: &str = "ui-cursor-highlight-stop";
    
    // Element management events
    pub const ELEMENT_CREATED: &str = "ui-element-created";
    pub const ELEMENT_UPDATED: &str = "ui-element-updated";
    pub const ELEMENT_DELETED: &str = "ui-element-deleted";
}

/// Menu and navigation events
pub mod menu {
    pub const SETTINGS_REQUESTED: &str = "settings-requested";
    pub const DEVTOOLS_REQUESTED: &str = "devtools-requested";
    pub const PERMISSIONS_REQUESTED: &str = "permissions-requested";
    pub const FEEDBACK_REQUESTED: &str = "feedback-requested";
    pub const HELP_REQUESTED: &str = "help-requested";
    pub const NEW_CHAT_REQUESTED: &str = "new-chat-requested";
    pub const IMPORT_CHAT_REQUESTED: &str = "import-chat-requested";
    pub const EXPORT_CHAT_REQUESTED: &str = "export-chat-requested";
    pub const TOGGLE_FLOATING_BAR_REQUESTED: &str = "toggle-floating-bar-requested";
    pub const TOGGLE_DEV_PANEL_REQUESTED: &str = "toggle-dev-panel-requested";
    pub const TOGGLE_FULLSCREEN_REQUESTED: &str = "toggle-fullscreen-requested";
    pub const MINIMIZE_WINDOW_REQUESTED: &str = "minimize-window-requested";
    pub const ZOOM_WINDOW_REQUESTED: &str = "zoom-window-requested";
    pub const UPDATE_CHECK_REQUESTED: &str = "update-check-requested";
    pub const ABOUT_REQUESTED: &str = "about-requested";
    
    // View menu events
    pub const VIEW_CHAT: &str = "menu-view-chat";
    pub const VIEW_DEVTOOLS: &str = "menu-view-devtools";
    pub const VIEW_PERMISSIONS: &str = "menu-view-permissions";
    
    // Chat menu events
    pub const CLEAR_CHAT: &str = "menu-clear-chat";
    
    // Modal events
    pub const SHOW_HELP: &str = "menu-show-help";
    pub const SHOW_FEEDBACK: &str = "menu-show-feedback";
    pub const EXPORT_CHAT: &str = "menu-export-chat";
    pub const IMPORT_CHAT: &str = "menu-import-chat";
    pub const OPEN_SETTINGS: &str = "menu-open-settings";
    
    // Reload events
    pub const RELOAD_APP: &str = "menu-reload-app";
    pub const FORCE_RELOAD: &str = "menu-force-reload";
    
    // Zoom events
    pub const ZOOM_IN: &str = "menu-zoom-in";
    pub const ZOOM_OUT: &str = "menu-zoom-out";
    pub const RESET_ZOOM: &str = "menu-reset-zoom";

    // Edit menu events
    pub const EDIT_UNDO: &str = "menu-edit-undo";
    pub const EDIT_REDO: &str = "menu-edit-redo";
    pub const EDIT_CUT: &str = "menu-edit-cut";
    pub const EDIT_COPY: &str = "menu-edit-copy";
    pub const EDIT_PASTE: &str = "menu-edit-paste";
    pub const EDIT_SELECT_ALL: &str = "menu-edit-select-all";
}

/// Text-to-speech events
pub mod tts {
    pub const AUDIO_READY: &str = "tts-audio-ready";
    pub const STOP_REQUESTED: &str = "tts-stop-requested";
}

/// Always listening events
pub mod always_listening {
    pub const MODE_CHANGED: &str = "always-listening-mode-changed";
    pub const WAKE_WORD_DETECTED: &str = "always-listening:wake-word-detected";
    pub const TOGGLE_DICTATION_REQUEST: &str = "toggle-dictation-request";
    pub const STARTED: &str = "always-listening:started";
    pub const STOPPED: &str = "always-listening:stopped";
    pub const ACTIVATED: &str = "always-listening:activated";
    pub const DEACTIVATED: &str = "always-listening:deactivated";
    pub const STOP_REQUESTED: &str = "always-listening:stop-requested";
    pub const TRANSCRIPTION: &str = "always-listening:transcription";
    pub const COMMAND_PROCESSED: &str = "always-listening:command-processed";
    pub const EVENT: &str = "always-listening-event";
    pub const STOPPED_BY_COMMAND: &str = "always-listening:stopped-by-command";
    pub const RETURN_TO_WAKE_WORD: &str = "always-listening:return-to-wake-word";
}

/// Permission events
pub mod permissions {
    pub const CHANGED: &str = "permissions-changed";
    pub const RESTART_REQUIRED: &str = "permissions-restart-required";
    pub const GUIDANCE_NEEDED: &str = "permission-guidance-needed";
}

/// Development tool events
pub mod dev {
    pub const TOOL_NOTIFICATION: &str = "dev-tool-notification";
}

/// User message events
pub mod messages {
    pub const USER_MESSAGE_SUBMITTED: &str = "user-message-submitted";
}

/// Cloud and connection events
pub mod cloud {
    pub const WEBSOCKET_CONNECT: &str = "websocket-connect";
    pub const WEBSOCKET_SEND: &str = "websocket-send";
    pub const WEBSOCKET_DISCONNECT: &str = "websocket-disconnect";
    pub const CONNECTOR_STATE: &str = "cloud-connector-state";
    pub const CONNECTION_STATE: &str = "cloud-connection-state";
    pub const COMMAND_RECEIVED: &str = "cloud-command-received";
}

/// System and application events
pub mod system {
    pub const ERROR_OCCURRED: &str = "error-occurred";
    pub const STATUS_UPDATE: &str = "system-status-update";
    pub const MCP_STATE_UPDATED: &str = "mcp_state_updated";
    pub const MOUSE_ENTERED_WINDOW: &str = "mouse-entered-window";
    pub const MOUSE_LEFT_WINDOW: &str = "mouse-left-window";
    pub const BACKEND_RESPONSE: &str = "backend-response";
    pub const PROVIDER_SETTINGS_CHANGED: &str = "provider_settings_changed";
    
    // Application lifecycle events
    pub const APP_READY: &str = "app-ready";
    pub const APP_FOCUS: &str = "app-focus";
    pub const APP_BLUR: &str = "app-blur";
    
    // Window events
    pub const WINDOW_MINIMIZE: &str = "window-minimize";
    pub const WINDOW_MAXIMIZE: &str = "window-maximize";
    pub const WINDOW_CLOSE: &str = "window-close";
}

/// Onboarding events
pub mod onboarding {
    pub const COMPLETE: &str = "onboarding-complete";
    pub const SKIPPED: &str = "onboarding-skipped";
}

/// Notification events
pub mod notifications {
    pub const TOAST: &str = "toast-notification";
}

/// Bar and UI state events
pub mod bar {
    pub const STATE_UPDATE: &str = "bar-state-update";
    pub const COMPLETE_TRANSITION: &str = "floating-bar-complete-transition";
    pub const CLEAR_ERROR: &str = "floating-bar-clear-error";
    pub const CONFIG_CHANGED: &str = "floating-bar-config-changed";
}

/// Tool and command execution events
pub mod tools {
    pub const USAGE: &str = "tool-usage";
    pub const APPROVAL_REQUEST: &str = "tool-approval-request";
    pub const COMMAND_EXECUTION_START: &str = "command-execution-start";
    pub const COMMAND_EXECUTION_END: &str = "command-execution-end";
    /// Emitted for every computer use action with target app, sensitivity, and timing.
    /// Frontend can collect these to display a reviewable action audit trail.
    pub const COMPUTER_USE_AUDIT: &str = "computer-use-audit";
    /// Emitted when AX (accessibility) grounding is attempted on a click action.
    /// Includes element role/label and whether AXPress was used vs coordinate fallback.
    pub const AX_GROUNDING_AUDIT: &str = "ax-grounding-audit";
}

/// Continuation events
pub mod continuation {
    pub const AGENT_REQUEST: &str = "agent-continuation-request";
    pub const AGENT_RESPONSE: &str = "agent-continuation-response";
}

/// Dictation state events
pub mod dictation_state {
    pub const CHANGED: &str = "dictation-state-changed";
    pub const FORCE_RESET: &str = "dictation-state-force-reset";
    pub const INPUT_CHANGED: &str = "dictation-input-state-changed";
}

/// Shortcut events
pub mod shortcuts {
    pub const AGENT_MODE: &str = "shortcut-agent-mode";
    pub const DICTATION_INPUT: &str = "shortcut-dictation-input";
    pub const ESCAPE_KEY: &str = "shortcut-escape-key";
}

/// Tool choice events
pub mod tool_choice {
    pub const CONFIG_CHANGED: &str = "tool-choice-config-changed";
    pub const CONFIG_RESET: &str = "tool-choice-config-reset";
    pub const ENABLED_CHANGED: &str = "tool-choice-enabled-changed";
}

/// Plugin events (namespaced with plugin:)
pub mod plugin {
    pub const VOICE_TRANSCRIPTION_DICTATION_STARTED: &str = "plugin:voice-transcription:dictation-started";
    pub const VOICE_TRANSCRIPTION_DICTATION_STOPPED: &str = "plugin:voice-transcription:dictation-stopped";
    pub const ALWAYS_LISTENING_STARTED: &str = "plugin:always-listening:started";
    pub const ALWAYS_LISTENING_STOPPED: &str = "plugin:always-listening:stopped";
}



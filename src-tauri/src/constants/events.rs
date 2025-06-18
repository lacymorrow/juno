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

    // Agent state events
    pub const ACTIVE: &str = "agent-active";
    pub const ERROR: &str = "agent-error";
    pub const TRANSCRIPTION_START: &str = "agent-transcription-start";
    pub const TRANSCRIPTION_STOP: &str = "agent-transcription-stop";
    pub const CANCEL: &str = "agent-cancel";
    pub const COMMITTED: &str = "agent-committed";
    pub const FORCE_STOP: &str = "agent-force-stop";
    pub const FORCE_CLEANUP: &str = "agent-force-cleanup";
}

/// Streaming events
pub mod streaming {
    pub const TEXT_STREAM: &str = "agent-text-stream";
    pub const STREAM_START: &str = "agent-stream-start";
    pub const STREAM_END: &str = "agent-stream-end";
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
    pub const TRANSCRIPTION_CANCEL: &str = "dictation-transcription-cancel";
    pub const TRANSCRIPTION_FORCE_STOP: &str = "dictation-transcription-force-stop";
    pub const TRANSCRIPTION_FORCE_CLEANUP: &str = "dictation-transcription-force-cleanup";
}

/// UI and window events
pub mod ui {
    pub const BAR_STATE_CHANGED: &str = "bar-state-changed";
    pub const REQUEST_AUDIO_PLAYBACK_TEST: &str = "request-audio-playback-test";
    pub const KEY_PRESS_VISUALIZATION: &str = "key-press-visualization";
    pub const CLICK_VISUALIZATION: &str = "click-visualization";
}

/// Menu and navigation events
pub mod menu {
    pub const SETTINGS_REQUESTED: &str = "settings-requested";
    pub const DEVTOOLS_REQUESTED: &str = "devtools-requested";
    pub const PERMISSIONS_REQUESTED: &str = "permissions-requested";
    pub const FEEDBACK_REQUESTED: &str = "feedback-requested";
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
    pub const ABOUT_REQUESTED: &str = "about-requested";
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
}

/// Permission events
pub mod permissions {
    pub const CHANGED: &str = "permissions-changed";
    pub const RESTART_REQUIRED: &str = "permissions-restart-required";
}

/// Development tool events
pub mod dev {
    pub const TOOL_NOTIFICATION: &str = "dev-tool-notification";
}

/// User message events
pub mod messages {
    pub const USER_MESSAGE_SUBMITTED: &str = "user-message-submitted";
}



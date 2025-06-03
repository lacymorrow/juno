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

    // Window/UI events
    pub const BAR_STATE_CHANGED: &str = "bar-state-changed";

    // Voice Control specific events (if any beyond started/finished/partial)
    pub const DICTATION_STATE_CHANGED: &str = "dictation-state-changed";
    pub const REQUEST_AUDIO_PLAYBACK_TEST: &str = "request-audio-playback-test";

    // Settings events
    pub const SETTINGS_REQUESTED: &str = "settings-requested";
}

pub mod window_labels {
    pub const MAIN: &str = "main";
    pub const FLOATING_BAR: &str = "floating-bar";
}

pub mod tray_menu_ids {
    pub const QUIT: &str = "quit";
    pub const TOGGLE_FLOATING_BAR: &str = "toggle-floating-bar";
}

pub mod app_menu_ids {
    pub const SETTINGS: &str = "settings";
    pub const ABOUT: &str = "about";
}

pub mod timeouts {
    pub const STANDARD_TIMEOUT_MS: u64 = 10000;
    pub const BROWSER_TIMEOUT_MS: u64 = 30000;
}

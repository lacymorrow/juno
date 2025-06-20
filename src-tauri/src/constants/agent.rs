//! # Agent Constants
//!
//! AI agent and processing related constants.

// Tool names for delegation
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

// Agent configuration
pub mod config {
    pub const MAX_ITERATIONS: u32 = 40;
    pub const MAX_ITERATIONS_REDUCED: u32 = 20;

    // Token limits
    pub const DEFAULT_MAX_TOKENS_STANDARD: u32 = 4096;
    pub const DEFAULT_MAX_TOKENS_COMPACT: i32 = 1024;

    // Temperature settings
    pub const DEFAULT_TEMPERATURE: f32 = 0.7;

    // Retry settings
    pub const MAX_RETRY_ATTEMPTS: usize = 3;
    pub const MAX_RECOVERY_ATTEMPTS: usize = 5;

    // Tool call limits
    pub const MAX_TOOL_CALLS_PER_ITERATION: usize = 20;
    pub const MAX_MEMORY_ENTRIES: usize = 1000;
}

// Monitor session settings
pub mod monitor_sessions {
    pub const HOLD_DURATION_MS: u64 = 300;
    pub const IMMEDIATE_START_MS: u64 = 0;

    // Max durations
    pub const MAX_TRANSCRIPTION_DURATION_MS: u64 = 30_000;  // 30 seconds
    pub const MAX_AGENT_DURATION_MS: u64 = 180_000;         // 2 minutes

    // Cleanup timeouts
    pub const FORCE_CLEANUP_TIMEOUT_MS: u64 = 5_000;       // 5 seconds
    pub const COOLDOWN_AFTER_CANCEL_MS: u64 = 150;         // 150ms

    // Monitor intervals
    pub const AGENT_MONITOR_INTERVAL_MS: u64 = 100;
    pub const DICTATION_MONITOR_INTERVAL_MS: u64 = 50;
}



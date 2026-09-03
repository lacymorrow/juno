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
    pub const SCREENSHOT: &str = "screenshot";
    pub const LEFT_CLICK: &str = "left_click";
    pub const TYPE: &str = "type";
    pub const KEY: &str = "key";
    pub const SCROLL: &str = "scroll";
    pub const LEFT_CLICK_DRAG: &str = "left_click_drag";
    pub const MOUSE_MOVE: &str = "mouse_move";
    pub const ACCESSIBILITY_INTERFACE: &str = "accessibility_interface";

    // Text editor tools
    pub const TEXT_EDITOR_INSERT: &str = "text_editor_insert";
    pub const TEXT_EDITOR_STR_REPLACE: &str = "text_editor_str_replace";
    pub const TEXT_EDITOR_UNDO_EDIT: &str = "text_editor_undo_edit";

    // Browser tools
    pub const BROWSER_NAVIGATE: &str = "browser_navigate";
    pub const BROWSER_CLICK: &str = "browser_click";
    pub const BROWSER_TYPE: &str = "browser_type";
    pub const BROWSER_SCROLL: &str = "browser_scroll";
    pub const BROWSER_SCREENSHOT: &str = "browser_screenshot";
    pub const BROWSER_GET_CONTENT: &str = "browser_get_content";
    pub const BROWSER_INTERACT: &str = "browser_interact";
    pub const BROWSER_EXTRACT_CONTENT: &str = "browser_extract_content";
    pub const BROWSER_GET_CURRENT_URL: &str = "browser_get_current_url";
    pub const BROWSER_FORM: &str = "browser_form";

    // Safari tools (specialized browser automation for Safari)
    pub const SAFARI_EXTRACT_DOM: &str = "safari_extract_dom";
    pub const SAFARI_CLICK_ELEMENT: &str = "safari_click_element";
    pub const SAFARI_TYPE_TEXT: &str = "safari_type_text";
    pub const SAFARI_GET_URL: &str = "safari_get_url";
    pub const SAFARI_NAVIGATE: &str = "safari_navigate";
    pub const SAFARI_LIST_CLICKABLE_ELEMENTS: &str = "safari_list_clickable_elements";
    pub const SAFARI_EXECUTE_JAVASCRIPT: &str = "safari_execute_javascript";
    pub const SAFARI_CLEAR_CACHE: &str = "safari_clear_cache";

    // Desktop tools
    pub const OPEN_APPLICATION: &str = "open_application";
    pub const OPEN_URL: &str = "open_url";
    pub const DEV_FOCUS_WINDOW: &str = "dev_focus_window";
    pub const DEV_SCROLL_WINDOW: &str = "dev_scroll_window";
    pub const CAPTURE_SCREENSHOT_COMMAND: &str = "capture_screenshot_command";
    pub const DEV_GET_CLIPBOARD: &str = "dev_get_clipboard";
    pub const DEV_SET_CLIPBOARD: &str = "dev_set_clipboard";
    pub const DEV_GET_WINDOW_LIST: &str = "dev_get_window_list";
    pub const DEV_FIND_ELEMENT_BY_SELECTOR: &str = "dev_find_element_by_selector";
    pub const DESKTOP_OPEN_APP: &str = "desktop_open_app";
    pub const DESKTOP_FOCUS_WINDOW: &str = "desktop_focus_window";
    pub const DESKTOP_SCROLL: &str = "desktop_scroll";
    pub const DESKTOP_SCREENSHOT: &str = "desktop_screenshot";
    pub const LAUNCH_APPLICATION: &str = "launch_application";
    pub const GET_RUNNING_APPLICATIONS: &str = "get_running_applications";
    pub const FOCUS_APPLICATION: &str = "focus_application";
    pub const QUIT_APPLICATION: &str = "quit_application";
    pub const GET_SYSTEM_INFO: &str = "get_system_info";
    pub const MANAGE_AUDIO: &str = "manage_audio";

    // Accessibility tools (native macOS element interaction)
    pub const ACCESSIBILITY_SCAN: &str = "accessibility_scan";
    pub const ACCESSIBILITY_CLICK: &str = "accessibility_click";

    // Basic tools (file operations, commands, etc.)
    pub const BASH_COMMAND: &str = "bash_command";
    pub const LIST_FILES: &str = "list_files";
    pub const GET_FILE_CONTENT: &str = "get_file_content";
    pub const SET_FILE_CONTENT: &str = "set_file_content";
    pub const DEV_TEXT_EDITOR_VIEW: &str = "dev_text_editor_view";
    pub const DEV_TEXT_EDITOR_CREATE: &str = "dev_text_editor_create";
    pub const DEV_TEXT_EDITOR_STR_REPLACE: &str = "dev_text_editor_str_replace";
    pub const SYSTEM_EXEC: &str = "system_exec";
    pub const SYSTEM_LIST_FILES: &str = "system_list_files";
    pub const SYSTEM_READ_FILE: &str = "system_read_file";
    pub const SYSTEM_WRITE_FILE: &str = "system_write_file";
    pub const DEV_LIST_FILES: &str = "dev_list_files";
    pub const DEV_GET_FILE_CONTENT: &str = "dev_get_file_content";
    pub const DEV_SET_FILE_CONTENT: &str = "dev_set_file_content";
    pub const FILE_READ: &str = "file_read";
    pub const FILE_WRITE: &str = "file_write";
    pub const FILE_CREATE: &str = "file_create";
    pub const FILE_DELETE: &str = "file_delete";
    pub const COMMAND_EXECUTE: &str = "command_execute";
    pub const SHELL_EXECUTE: &str = "shell_execute";
    pub const BASH_EXECUTE: &str = "bash_execute";

    // Basic file and directory operations (standardized names)
    pub const READ_FILE: &str = "read_file";
    pub const WRITE_FILE: &str = "write_file";
    pub const LIST_DIRECTORY: &str = "list_directory";
    pub const CREATE_DIRECTORY: &str = "create_directory";
    pub const DELETE_FILE: &str = "delete_file";
    pub const TEXT_EDITOR_EDIT: &str = "text_editor_edit";
    pub const EXECUTE_SHELL_COMMAND: &str = "execute_shell_command";

    // Timer tools standardized names
    pub const LIST_TIMERS: &str = "list_timers";
    pub const CANCEL_TIMER: &str = "cancel_timer";
    pub const TIMER_STATUS: &str = "timer_status";
    pub const SET_TIMER: &str = "set_timer";
    pub const SET_SCREEN_MONITOR: &str = "set_screen_monitor";
    pub const SET_FILE_MONITOR: &str = "set_file_monitor";
    pub const CHECK_EXPIRED_TIMERS: &str = "check_expired_timers";

    // Scheduled automation tools (user-facing cron schedules)
    pub const CREATE_SCHEDULED_AUTOMATION: &str = "create_scheduled_automation";
    pub const LIST_SCHEDULED_AUTOMATIONS: &str = "list_scheduled_automations";
    pub const DELETE_SCHEDULED_AUTOMATION: &str = "delete_scheduled_automation";

    // Timer tools
    pub const TIMER_CREATE: &str = "timer_create";
    pub const TIMER_START: &str = "timer_start";
    pub const TIMER_STOP: &str = "timer_stop";
    pub const TIMER_PAUSE: &str = "timer_pause";
    pub const TIMER_RESUME: &str = "timer_resume";
    pub const TIMER_GET_STATUS: &str = "timer_get_status";
    pub const TIMER_LIST: &str = "timer_list";
    pub const TIMER_DELETE: &str = "timer_delete";
    pub const CREATE_TIMER: &str = "create_timer";

    // Computer use actions
    pub const ACTION_SCREENSHOT: &str = "screenshot";
    pub const ACTION_LEFT_CLICK: &str = "left_click";
    pub const ACTION_TYPE: &str = "type";
    pub const ACTION_KEY: &str = "key";
    pub const ACTION_SCROLL: &str = "scroll";
    pub const ACTION_WAIT: &str = "wait";

    // Extended computer use actions
    pub const ACTION_RIGHT_CLICK: &str = "right_click";
    pub const ACTION_MIDDLE_CLICK: &str = "middle_click";
    pub const ACTION_DOUBLE_CLICK: &str = "double_click";
    pub const ACTION_TRIPLE_CLICK: &str = "triple_click";
    pub const ACTION_HOLD_KEY: &str = "hold_key";
    pub const WAIT: &str = "wait";
}

// Intent keywords for user request analysis
pub mod intent_keywords {
    // Browser expert keywords
    pub const BROWSE: &str = "browse";
    pub const WEBSITE: &str = "website";
    pub const URL: &str = "url";
    pub const NAVIGATE: &str = "navigate";
    pub const WEB: &str = "web";
    pub const PAGE: &str = "page";
    pub const FORM: &str = "form";
    pub const SEARCH_ONLINE: &str = "search online";
    pub const INTERNET: &str = "internet";
    pub const BROWSER: &str = "browser";
    pub const LINK: &str = "link";
    pub const DOMAIN: &str = "domain";
    pub const HTTP: &str = "http";

    // Coding expert keywords
    pub const CODE: &str = "code";
    pub const FILE: &str = "file";
    pub const PROGRAM: &str = "program";
    pub const SCRIPT: &str = "script";
    pub const TERMINAL: &str = "terminal";
    pub const COMMAND: &str = "command";
    pub const DEBUG: &str = "debug";
    pub const COMPILE: &str = "compile";
    pub const GIT: &str = "git";
    pub const REPOSITORY: &str = "repository";
    pub const FUNCTION: &str = "function";
    pub const VARIABLE: &str = "variable";
    pub const EDIT: &str = "edit";
    pub const CREATE_FILE: &str = "create file";
    pub const READ_FILE: &str = "read file";
    pub const WRITE_FILE: &str = "write file";
    pub const BASH: &str = "bash";
    pub const SHELL: &str = "shell";

    // Desktop expert keywords
    pub const OPEN_APP: &str = "open app";
    pub const APPLICATION: &str = "application";
    pub const DESKTOP: &str = "desktop";
    pub const WINDOW: &str = "window";
    pub const SCREENSHOT: &str = "screenshot";
    pub const CLICK_ON: &str = "click on";
    pub const TYPE_IN: &str = "type in";
    pub const SHORTCUT: &str = "shortcut";
    pub const MOUSE: &str = "mouse";
    pub const KEYBOARD: &str = "keyboard";
    pub const CLIPBOARD: &str = "clipboard";
}

// Test string constants for user intent analysis
pub mod test_strings {
    // Browser-related test strings
    pub const NAVIGATE_TO_WEBSITE: &str = "please navigate to website";

    // Coding-related test strings
    pub const EDIT_FILE: &str = "edit this file";

    // Desktop-related test strings
    pub const TAKE_SCREENSHOT: &str = "take a screenshot";

    // General test strings
    pub const WEATHER_QUERY: &str = "what is the weather?";
}

// Tool prefixes for pattern matching in tool categorization
pub mod tool_prefixes {
    // Browser tool prefixes
    pub const BROWSER: &str = "browser_";
    pub const SAFARI: &str = "safari_";

    // Desktop tool prefixes
    pub const DEV: &str = "dev_";
    pub const DESKTOP: &str = "desktop_";

    // System tool prefixes
    pub const SYSTEM: &str = "system_";

    // Timer tool prefixes
    pub const TIMER: &str = "timer_";

    // MCP tool prefixes
    pub const MCP: &str = "mcp_";
}

// Agent configuration
pub mod config {
    pub const MAX_ITERATIONS: u32 = 15;
    pub const MAX_ITERATIONS_REDUCED: u32 = 10;

    // Continuation settings - independent timeout for agent continuation requests
    pub const DEFAULT_CONTINUATION_ADDITIONAL_STEPS: u32 = 20;
    pub const CONTINUATION_REQUEST_TIMEOUT_SECONDS: u64 = 300; // 5 minutes - appropriate for user continuation

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

    // Independent timeout settings optimized for agent operations
    pub const DEFAULT_TASK_TIMEOUT_SECONDS: u64 = 300; // 5 minutes - appropriate for general agent tasks
    pub const DEFAULT_COMMAND_TIMEOUT_SECONDS: u64 = 180; // 3 minutes - appropriate for basic commands like terminal operations
}

// Monitor session settings
pub mod monitor_sessions {
    pub const HOLD_DURATION_MS: u64 = 300;
    pub const IMMEDIATE_START_MS: u64 = 15;

    // Max durations
    pub const MAX_TRANSCRIPTION_DURATION_MS: u64 = 30_000; // 30 seconds
    pub const MAX_AGENT_DURATION_MS: u64 = 180_000; // 2 minutes

    // Cleanup timeouts
    pub const FORCE_CLEANUP_TIMEOUT_MS: u64 = 5_000; // 5 seconds
    pub const COOLDOWN_AFTER_CANCEL_MS: u64 = 150; // 150ms

    // Monitor intervals
    pub const AGENT_MONITOR_INTERVAL_MS: u64 = 100;
    pub const DICTATION_MONITOR_INTERVAL_MS: u64 = 50;
}

// Computer action constants (for documentation and consistency)
pub mod computer_actions {
    // Valid Anthropic Computer Use API actions only
    pub const SCREENSHOT: &str = "screenshot";
    pub const LEFT_CLICK: &str = "left_click";
    pub const RIGHT_CLICK: &str = "right_click";
    pub const MIDDLE_CLICK: &str = "middle_click";
    pub const DOUBLE_CLICK: &str = "double_click";
    pub const TRIPLE_CLICK: &str = "triple_click";
    pub const LEFT_CLICK_DRAG: &str = "left_click_drag";
    pub const MOUSE_MOVE: &str = "mouse_move";
    pub const LEFT_MOUSE_DOWN: &str = "left_mouse_down";
    pub const LEFT_MOUSE_UP: &str = "left_mouse_up";
    pub const KEY: &str = "key";
    pub const HOLD_KEY: &str = "hold_key";
    pub const TYPE: &str = "type";
    pub const SCROLL: &str = "scroll";
    pub const CURSOR_POSITION: &str = "cursor_position";
    pub const WAIT: &str = "wait";
}

// Tool descriptive names for enhanced logging
pub mod tool_descriptive_names {
    pub const COMPUTER_SCREENSHOT: &str = "computer/screenshot";
    pub const COMPUTER_GET_CURSOR_POSITION: &str = "computer/get_cursor_position";
    pub const COMPUTER_MOUSE_MOVE: &str = "computer/mouse_move";
    pub const COMPUTER_LEFT_CLICK: &str = "computer/left_click";
    pub const COMPUTER_RIGHT_CLICK: &str = "computer/right_click";
    pub const COMPUTER_MIDDLE_CLICK: &str = "computer/middle_click";
    pub const COMPUTER_DOUBLE_CLICK: &str = "computer/double_click";
    pub const COMPUTER_TRIPLE_CLICK: &str = "computer/triple_click";
    pub const COMPUTER_LEFT_CLICK_DRAG: &str = "computer/left_click_drag";
    pub const COMPUTER_TYPE: &str = "computer/type";
    pub const COMPUTER_KEY: &str = "computer/key";
    pub const COMPUTER_SCROLL: &str = "computer/scroll";
    pub const COMPUTER_HOLD_KEY: &str = "computer/hold_key";
    pub const COMPUTER_RELEASE_KEY: &str = "computer/release_key";
    pub const COMPUTER_LEFT_MOUSE_DOWN: &str = "computer/left_mouse_down";
    pub const COMPUTER_LEFT_MOUSE_UP: &str = "computer/left_mouse_up";
    pub const COMPUTER_WAIT: &str = "computer/wait";

    // Format strings for parametrized tool names
    pub const COMPUTER_MOVE_TO_FORMAT: &str = "computer/move_to({}, {})";
    pub const COMPUTER_CLICK_FORMAT: &str = "computer/click({}, {})";
    pub const COMPUTER_RIGHT_CLICK_FORMAT: &str = "computer/right_click({}, {})";
    pub const COMPUTER_MIDDLE_CLICK_FORMAT: &str = "computer/middle_click({}, {})";
    pub const COMPUTER_DOUBLE_CLICK_FORMAT: &str = "computer/double_click({}, {})";
    pub const COMPUTER_TRIPLE_CLICK_FORMAT: &str = "computer/triple_click({}, {})";
    pub const COMPUTER_DRAG_FORMAT: &str = "computer/drag({}, {}, {}, {})";
    pub const COMPUTER_TYPE_FORMAT: &str = "computer/type(\"{}\")";
    pub const COMPUTER_KEY_FORMAT: &str = "computer/key(\"{}\")";
    pub const COMPUTER_SCROLL_FORMAT: &str = "computer/scroll({}, {}, {})";
    pub const COMPUTER_HOLD_KEY_FORMAT: &str = "computer/hold_key(\"{}\")";
    pub const COMPUTER_RELEASE_KEY_FORMAT: &str = "computer/release_key(\"{}\")";
    pub const COMPUTER_WAIT_FORMAT: &str = "computer/wait({})";

    pub const COMPUTER_MOUSE_DOWN_FORMAT: &str = "computer/mouse_down({}, {})";
    pub const COMPUTER_MOUSE_UP_FORMAT: &str = "computer/mouse_up({}, {})";

    pub const COMPUTER_SCROLL_DIRECTION_FORMAT: &str = "computer/scroll_{}({},{} × {})";
    pub const COMPUTER_SCROLL_SIMPLE_FORMAT: &str = "computer/scroll_{} × {}";
    pub const COMPUTER_ACTION_FORMAT: &str = "computer/{}";

    // Additional format strings for tool execution
    pub const COMPUTER_USE_EXECUTION_FORMAT: &str = "🖥️ Computer Use: {} → {}";
    pub const COMPUTER_ACTION_EXECUTION_LOG_FORMAT: &str = "Executing computer action: {}";
    pub const GET_CURSOR_POSITION_FAILED_FORMAT: &str = "Get cursor position failed: {}";
}

// Agent confidence score constants
pub mod confidence_scores {
    // High confidence for exact tool matches
    pub const HIGH_CONFIDENCE: f32 = 0.95;

    // Partial relevance confidence scores
    pub const PARTIAL_BROWSER_COMPUTER_USE: f32 = 0.3; // Screenshots, clicks can help browser work
    pub const PARTIAL_DESKTOP_BASIC: f32 = 0.2; // Some file ops relate to desktop
    pub const PARTIAL_CODING_DESKTOP: f32 = 0.1; // Very limited overlap
    pub const NO_CONFIDENCE: f32 = 0.0;
}

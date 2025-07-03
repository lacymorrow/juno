//! # UI Constants
//!
//! User interface related constants including window labels and breakpoints.

pub mod window_labels {
    pub const MAIN: &str = "main";
    pub const FLOATING_BAR: &str = "floating-bar";
    pub const FLOATING_PANEL: &str = "floating-panel";
    pub const ONBOARDING: &str = "onboarding";
    pub const SETTINGS: &str = "settings";
}

/// UI state constants for bar state management
/// These values are used by the backend BarState enum and must match frontend expectations
pub mod bar_states {
    pub const DEFAULT: &str = "default";
    pub const EXPANDING: &str = "expanding";
    pub const INPUT: &str = "input";
    pub const SHRINKING: &str = "shrinking";
    pub const SUBMITTING: &str = "submitting";
    pub const LOADING: &str = "loading";
    pub const SUCCESS: &str = "success";
    pub const ERROR: &str = "error";
    pub const SPEAKING: &str = "speaking";
    pub const LISTENING: &str = "listening";
    pub const TRANSCRIBING: &str = "transcribing";
    pub const DICTATING: &str = "dictating";
    pub const DICTATION_READY: &str = "dictation_ready";
    pub const ALWAYS_LISTENING: &str = "always_listening";
    pub const FINISHING: &str = "finishing";
    pub const AGENT_RESPONDING: &str = "agent_responding";
}

/// Voice mode constants
/// These values are used by the frontend VoiceContext and must match frontend expectations
pub mod voice_modes {
    pub const IDLE: &str = "idle";
    pub const AGENT: &str = "agent";
    pub const DICTATION: &str = "dictation";
}

/// Agent status constants
/// These values are used by the frontend agent state management and must match frontend expectations
pub mod agent_status {
    pub const IDLE: &str = "idle";
    pub const DICTATING: &str = "dictating";
    pub const LISTENING: &str = "listening";
    pub const THINKING: &str = "thinking";
    pub const RESPONDING: &str = "responding";
    pub const ERROR: &str = "error";
    pub const WORKING: &str = "working";
    pub const FINISHED: &str = "finished";
    pub const FAILED: &str = "failed";
    pub const CANCELLED: &str = "cancelled";
    pub const OFFLINE: &str = "offline";
    pub const PROCESSING: &str = "processing";
}

/// UI interaction types
/// These values are used by the frontend UI interaction handlers
pub mod interaction_types {
    pub const CLICK: &str = "click";
    pub const FOCUS: &str = "focus";
    pub const BLUR: &str = "blur";
    pub const HOVER: &str = "hover";
    pub const INPUT: &str = "input";
    pub const SUBMIT: &str = "submit";
}

// UI layout constants (moved from frontend constants.ts)
pub const MOBILE_BREAKPOINT: i32 = 768;
pub const PERCENTAGE_MULTIPLIER: f64 = 100.0;
pub const SCROLL_WHEEL_EVENT_LINE_SCROLL: i32 = 120;
pub const DOUBLE_CLICK_INTERVAL_MS: u64 = 50;
pub const MAX_TREE_SEARCH_DEPTH: usize = 100;

/// UI text display constants
pub mod text_display {
    /// Maximum characters to show in key press visualization text
    pub const MAX_KEYPRESS_VISUALIZATION_TEXT_LENGTH: usize = 30;

    /// Maximum characters to show in UI preview text
    pub const MAX_UI_PREVIEW_TEXT_LENGTH: usize = 50;
}

/// Standard resolution constants for Anthropic Computer Use API compliance
/// Screenshots must be scaled to these standard resolutions per specification
pub mod standard_resolutions {
    /// Extended Graphics Array - 1024x768
    pub const XGA: (u32, u32) = (1024, 768);

    /// Wide Extended Graphics Array - 1280x800
    pub const WXGA: (u32, u32) = (1280, 800);

    /// Full Wide Extended Graphics Array - 1366x768
    pub const FWXGA: (u32, u32) = (1366, 768);

    /// All supported standard resolutions
    pub const ALL_RESOLUTIONS: [(u32, u32); 3] = [XGA, WXGA, FWXGA];

    /// Select the best standard resolution based on display dimensions
    /// Returns the standard resolution that best matches the display aspect ratio
    /// and provides reasonable scaling factors
    pub fn select_best_resolution(display_width: u32, display_height: u32) -> (u32, u32) {
        if display_width == 0 || display_height == 0 {
            return XGA; // Default fallback
        }

        let display_aspect = display_width as f32 / display_height as f32;

        // Calculate aspect ratios for each standard resolution
        let xga_aspect = XGA.0 as f32 / XGA.1 as f32;
        let wxga_aspect = WXGA.0 as f32 / WXGA.1 as f32;
        let fwxga_aspect = FWXGA.0 as f32 / FWXGA.1 as f32;

        // Find the closest aspect ratio match
        let xga_diff = (display_aspect - xga_aspect).abs();
        let wxga_diff = (display_aspect - wxga_aspect).abs();
        let fwxga_diff = (display_aspect - fwxga_aspect).abs();

        if xga_diff <= wxga_diff && xga_diff <= fwxga_diff {
            XGA
        } else if wxga_diff <= fwxga_diff {
            WXGA
        } else {
            FWXGA
        }
    }
}

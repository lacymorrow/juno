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

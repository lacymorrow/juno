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

// UI layout constants
pub const MOBILE_BREAKPOINT: i32 = 768;
pub const PERCENTAGE_MULTIPLIER: f64 = 100.0;
pub const SCROLL_WHEEL_EVENT_LINE_SCROLL: i32 = 120;
pub const DOUBLE_CLICK_INTERVAL_MS: u64 = 50;
pub const MAX_TREE_SEARCH_DEPTH: usize = 100;

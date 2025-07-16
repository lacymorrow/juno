//! # Mouse and UI Interaction Constants
//!
//! Constants for mouse operations, UI interactions, and visual feedback
//! to eliminate magic numbers throughout the mouse and UI system.

// Top-level exports for compatibility
pub const DEFAULT_CLICK_DELAY_MS: u64 = 100;
pub const DEFAULT_SCROLL_AMOUNT: i32 = 5;

/// Mouse movement and animation constants
pub mod movement {
    /// Frames per second for smooth mouse movement
    pub const SMOOTH_MOVEMENT_FPS: u64 = 60;

    /// Frame time in milliseconds for smooth movement (1000ms / 60fps = ~16.67ms)
    pub const SMOOTH_MOVEMENT_FRAME_TIME_MS: u64 = 1000 / SMOOTH_MOVEMENT_FPS;

    /// Default movement duration in milliseconds
    pub const DEFAULT_MOVEMENT_DURATION_MS: u64 = 300;

    /// Minimum distance in pixels to trigger smooth movement
    pub const MIN_MOVEMENT_DISTANCE: f64 = 5.0;

    /// Default cursor movement speed multiplier
    pub const DEFAULT_SPEED_MULTIPLIER: f64 = 1.0;
}

/// Mouse click and interaction constants
pub mod interaction {
    /// Default click duration in milliseconds
    pub const DEFAULT_CLICK_DURATION_MS: u64 = 100;

    /// Double click maximum interval in milliseconds
    pub const DOUBLE_CLICK_MAX_INTERVAL_MS: u64 = 500;

    /// Triple click maximum interval in milliseconds
    pub const TRIPLE_CLICK_MAX_INTERVAL_MS: u64 = 300;

    /// Mouse button press hold duration in milliseconds
    pub const MOUSE_BUTTON_HOLD_DURATION_MS: u64 = 50;
}

/// Visual feedback and testing constants
pub mod visual {
    /// Default click visualization color (red)
    pub const DEFAULT_CLICK_COLOR: &str = "#FF0000";

    /// Click visualization duration in milliseconds
    pub const CLICK_VISUALIZATION_DURATION_MS: u64 = 500;

    /// Cursor highlight circle radius in pixels
    pub const CURSOR_HIGHLIGHT_RADIUS: f64 = 20.0;

    /// Cursor highlight animation duration in milliseconds
    pub const CURSOR_HIGHLIGHT_DURATION_MS: u64 = 200;
}

/// Test and QA constants
pub mod testing {
    /// Test circle center X coordinate
    pub const TEST_CIRCLE_CENTER_X: f64 = 500.0;

    /// Test circle center Y coordinate
    pub const TEST_CIRCLE_CENTER_Y: f64 = 300.0;

    /// Test circle radius in pixels
    pub const TEST_CIRCLE_RADIUS: f64 = 100.0;

    /// Maximum coordinate value for validation
    pub const MAX_COORDINATE_VALUE: f64 = 10000.0;

    /// Minimum coordinate value for validation
    pub const MIN_COORDINATE_VALUE: f64 = 0.0;
}

/// Window focus and operation delays
pub mod delays {
    /// Delay after window focus before operation (milliseconds)
    pub const WINDOW_FOCUS_DELAY_MS: u64 = 100;

    /// Delay between mouse operations (milliseconds)
    pub const MOUSE_OPERATION_DELAY_MS: u64 = 10;

    /// Delay for UI state transitions (milliseconds)
    pub const UI_STATE_TRANSITION_DELAY_MS: u64 = 100;
}

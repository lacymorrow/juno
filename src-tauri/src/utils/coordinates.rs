use crate::constants::ui::standard_resolutions;
use once_cell::sync::Lazy;
use serde::Serialize;
use std::sync::RwLock;
use tracing::info;

/// The model name currently in use. Updated at the start of each agent run so
/// that the screenshot pipeline can pick the right resolution tier.
pub static CURRENT_MODEL: Lazy<RwLock<String>> = Lazy::new(|| RwLock::new(String::new()));

/// Update the current model name (called at agent start).
pub fn set_current_model(model: &str) {
    if let Ok(mut m) = CURRENT_MODEL.write() {
        *m = model.to_string();
    }
}

/// Read the current model name.
pub fn get_current_model() -> String {
    CURRENT_MODEL.read().map(|m| m.clone()).unwrap_or_default()
}

// Global state to store the current screenshot scaling information
pub static SCREENSHOT_SCALE: Lazy<RwLock<ScalingInfo>> = Lazy::new(|| {
    RwLock::new(ScalingInfo {
        display_width: 0,
        display_height: 0,
        standard_width: 0,
        standard_height: 0,
        screenshot_width: 0,
        screenshot_height: 0,
        display_to_standard_scale_x: 1.0,
        display_to_standard_scale_y: 1.0,
        screenshot_to_standard_scale_x: 1.0,
        screenshot_to_standard_scale_y: 1.0,
        // Removed legacy fields
        display_origin_x: 0.0,
        display_origin_y: 0.0,
        display_id: None,
    })
});

/// Represents scaling information for Anthropic Computer Use API compliance
/// All coordinates are relative to standard resolutions (XGA, WXGA, FWXGA)
#[derive(Debug, Clone, Copy, Serialize)]
pub struct ScalingInfo {
    // New standard resolution fields
    pub display_width: u32,
    pub display_height: u32,
    pub standard_width: u32,
    pub standard_height: u32,
    pub screenshot_width: u32,
    pub screenshot_height: u32,
    pub display_to_standard_scale_x: f32,
    pub display_to_standard_scale_y: f32,
    pub screenshot_to_standard_scale_x: f32,
    pub screenshot_to_standard_scale_y: f32,

    // NEW: Multi-monitor support - track display origin offset
    pub display_origin_x: f64,
    pub display_origin_y: f64,
    pub display_id: Option<u32>, // Track which display the screenshot came from
}

impl Default for ScalingInfo {
    fn default() -> Self {
        Self {
            display_width: 0,
            display_height: 0,
            standard_width: 0,
            standard_height: 0,
            screenshot_width: 0,
            screenshot_height: 0,
            display_to_standard_scale_x: 1.0,
            display_to_standard_scale_y: 1.0,
            screenshot_to_standard_scale_x: 1.0,
            screenshot_to_standard_scale_y: 1.0,
            display_origin_x: 0.0,
            display_origin_y: 0.0,
            display_id: None,
        }
    }
}

/// Updates the scaling information with standard resolution scaling
/// This is the new primary function for Anthropic Computer Use API compliance
pub fn update_standard_resolution_scaling(
    display_width: u32,
    display_height: u32,
    screenshot_width: u32,
    screenshot_height: u32,
) {
    // Select the best standard resolution based on display aspect ratio.
    // Model-aware: Opus 4.5+ gets higher resolutions (up to 2576px).
    let model = get_current_model();
    let (standard_width, standard_height) = if model.is_empty() {
        standard_resolutions::select_best_resolution(display_width, display_height)
    } else {
        standard_resolutions::select_best_resolution_for_model(
            display_width,
            display_height,
            &model,
        )
    };

    // Calculate scaling factors from display to standard resolution
    let display_to_standard_scale_x = if display_width > 0 {
        standard_width as f32 / display_width as f32
    } else {
        1.0
    };

    let display_to_standard_scale_y = if display_height > 0 {
        standard_height as f32 / display_height as f32
    } else {
        1.0
    };

    // Calculate scaling factors from screenshot to standard resolution
    let screenshot_to_standard_scale_x = if screenshot_width > 0 {
        standard_width as f32 / screenshot_width as f32
    } else {
        1.0
    };

    let screenshot_to_standard_scale_y = if screenshot_height > 0 {
        standard_height as f32 / screenshot_height as f32
    } else {
        1.0
    };

    // Validate scale factors
    let safe_display_scale_x =
        if display_to_standard_scale_x.is_finite() && display_to_standard_scale_x > 0.0 {
            display_to_standard_scale_x
        } else {
            tracing::warn!(
                "Invalid display to standard X scale factor: {}, using 1.0",
                display_to_standard_scale_x
            );
            1.0
        };

    let safe_display_scale_y =
        if display_to_standard_scale_y.is_finite() && display_to_standard_scale_y > 0.0 {
            display_to_standard_scale_y
        } else {
            tracing::warn!(
                "Invalid display to standard Y scale factor: {}, using 1.0",
                display_to_standard_scale_y
            );
            1.0
        };

    let safe_screenshot_scale_x =
        if screenshot_to_standard_scale_x.is_finite() && screenshot_to_standard_scale_x > 0.0 {
            screenshot_to_standard_scale_x
        } else {
            tracing::warn!(
                "Invalid screenshot to standard X scale factor: {}, using 1.0",
                screenshot_to_standard_scale_x
            );
            1.0
        };

    let safe_screenshot_scale_y =
        if screenshot_to_standard_scale_y.is_finite() && screenshot_to_standard_scale_y > 0.0 {
            screenshot_to_standard_scale_y
        } else {
            tracing::warn!(
                "Invalid screenshot to standard Y scale factor: {}, using 1.0",
                screenshot_to_standard_scale_y
            );
            1.0
        };

    if let Ok(mut scaling) = SCREENSHOT_SCALE.write() {
        *scaling = ScalingInfo {
            display_width,
            display_height,
            standard_width,
            standard_height,
            screenshot_width,
            screenshot_height,
            display_to_standard_scale_x: safe_display_scale_x,
            display_to_standard_scale_y: safe_display_scale_y,
            screenshot_to_standard_scale_x: safe_screenshot_scale_x,
            screenshot_to_standard_scale_y: safe_screenshot_scale_y,
            display_origin_x: 0.0,
            display_origin_y: 0.0,
            display_id: None,
        };

        info!("Updated standard resolution scaling: display {}x{} → standard {}x{} → screenshot {}x{}",
            display_width, display_height, standard_width, standard_height, screenshot_width, screenshot_height);
        info!("Scale factors - display→standard: x={:.3}, y={:.3} | screenshot→standard: x={:.3}, y={:.3}",
            safe_display_scale_x, safe_display_scale_y, safe_screenshot_scale_x, safe_screenshot_scale_y);
    } else {
        tracing::error!("Failed to acquire write lock on SCREENSHOT_SCALE");
    }
}

/// NEW: Updates scaling information with display origin for multi-monitor support
/// This fixes the coordinate transformation issues in multi-monitor setups
pub fn update_standard_resolution_scaling_with_display(
    display_width: u32,
    display_height: u32,
    screenshot_width: u32,
    screenshot_height: u32,
    display_origin_x: f64,
    display_origin_y: f64,
    display_id: Option<u32>,
) {
    // Select the best standard resolution based on display aspect ratio.
    // Model-aware: Opus 4.5+ gets higher resolutions (up to 2576px).
    let model = get_current_model();
    let (standard_width, standard_height) = if model.is_empty() {
        standard_resolutions::select_best_resolution(display_width, display_height)
    } else {
        standard_resolutions::select_best_resolution_for_model(
            display_width,
            display_height,
            &model,
        )
    };

    // Calculate scaling factors from display to standard resolution
    let display_to_standard_scale_x = if display_width > 0 {
        standard_width as f32 / display_width as f32
    } else {
        1.0
    };

    let display_to_standard_scale_y = if display_height > 0 {
        standard_height as f32 / display_height as f32
    } else {
        1.0
    };

    // Calculate scaling factors from screenshot to standard resolution
    let screenshot_to_standard_scale_x = if screenshot_width > 0 {
        standard_width as f32 / screenshot_width as f32
    } else {
        1.0
    };

    let screenshot_to_standard_scale_y = if screenshot_height > 0 {
        standard_height as f32 / screenshot_height as f32
    } else {
        1.0
    };

    // Validate scale factors
    let safe_display_scale_x =
        if display_to_standard_scale_x.is_finite() && display_to_standard_scale_x > 0.0 {
            display_to_standard_scale_x
        } else {
            tracing::warn!(
                "Invalid display to standard X scale factor: {}, using 1.0",
                display_to_standard_scale_x
            );
            1.0
        };

    let safe_display_scale_y =
        if display_to_standard_scale_y.is_finite() && display_to_standard_scale_y > 0.0 {
            display_to_standard_scale_y
        } else {
            tracing::warn!(
                "Invalid display to standard Y scale factor: {}, using 1.0",
                display_to_standard_scale_y
            );
            1.0
        };

    let safe_screenshot_scale_x =
        if screenshot_to_standard_scale_x.is_finite() && screenshot_to_standard_scale_x > 0.0 {
            screenshot_to_standard_scale_x
        } else {
            tracing::warn!(
                "Invalid screenshot to standard X scale factor: {}, using 1.0",
                screenshot_to_standard_scale_x
            );
            1.0
        };

    let safe_screenshot_scale_y =
        if screenshot_to_standard_scale_y.is_finite() && screenshot_to_standard_scale_y > 0.0 {
            screenshot_to_standard_scale_y
        } else {
            tracing::warn!(
                "Invalid screenshot to standard Y scale factor: {}, using 1.0",
                screenshot_to_standard_scale_y
            );
            1.0
        };

    // Update scaling information with ALL values including display origin
    if let Ok(mut scaling) = SCREENSHOT_SCALE.write() {
        *scaling = ScalingInfo {
            display_width,
            display_height,
            standard_width,
            standard_height,
            screenshot_width,
            screenshot_height,
            display_to_standard_scale_x: safe_display_scale_x,
            display_to_standard_scale_y: safe_display_scale_y,
            screenshot_to_standard_scale_x: safe_screenshot_scale_x,
            screenshot_to_standard_scale_y: safe_screenshot_scale_y,
            display_origin_x,
            display_origin_y,
            display_id,
        };

        info!("Updated standard resolution scaling with display origin: display {}x{} at origin ({}, {}) → standard {}x{} → screenshot {}x{}",
            display_width, display_height, display_origin_x, display_origin_y, standard_width, standard_height, screenshot_width, screenshot_height);
        info!("Scale factors - display→standard: x={:.3}, y={:.3} | screenshot→standard: x={:.3}, y={:.3}",
            safe_display_scale_x, safe_display_scale_y, safe_screenshot_scale_x, safe_screenshot_scale_y);
        info!(
            "Display origin preserved: ({}, {}), display ID: {:?}",
            display_origin_x, display_origin_y, display_id
        );
    } else {
        tracing::error!(
            "Failed to acquire write lock on SCREENSHOT_SCALE for display origin update"
        );
    }
}

/// Transforms coordinates from standard resolution space to actual screen coordinates
/// This is the primary coordinate transformation for the Anthropic Computer Use API
/// NEW: Now accounts for display origin in multi-monitor setups
pub fn transform_standard_to_screen_coordinates(standard_x: f64, standard_y: f64) -> (f64, f64) {
    if let Ok(scaling) = SCREENSHOT_SCALE.read() {
        // Skip transformation if no scaling was applied or dimensions are invalid
        if scaling.display_width == 0
            || scaling.display_height == 0
            || scaling.standard_width == 0
            || scaling.standard_height == 0
            || scaling.display_to_standard_scale_x <= 0.0
            || scaling.display_to_standard_scale_y <= 0.0
        {
            return (standard_x, standard_y);
        }

        // Transform from standard resolution coordinates to display-relative coordinates
        let display_relative_x = standard_x / scaling.display_to_standard_scale_x as f64;
        let display_relative_y = standard_y / scaling.display_to_standard_scale_y as f64;

        // Add display origin offset to get global screen coordinates
        let screen_x = display_relative_x + scaling.display_origin_x;
        let screen_y = display_relative_y + scaling.display_origin_y;

        info!("Transformed coordinates: standard ({}, {}) → display-relative ({}, {}) → screen ({}, {}) [origin: ({}, {}), scale: x={:.3}, y={:.3}]",
            standard_x, standard_y, display_relative_x, display_relative_y, screen_x, screen_y,
            scaling.display_origin_x, scaling.display_origin_y, scaling.display_to_standard_scale_x, scaling.display_to_standard_scale_y);

        (screen_x, screen_y)
    } else {
        tracing::error!("Failed to acquire read lock on SCREENSHOT_SCALE");
        (standard_x, standard_y) // Return untransformed coordinates as fallback
    }
}

/// Transforms coordinates from actual screen space to standard resolution coordinates
/// Used for converting screen coordinates to API-compatible standard resolution coordinates
/// NEW: Now accounts for display origin in multi-monitor setups
pub fn transform_screen_to_standard_coordinates(screen_x: f64, screen_y: f64) -> (f64, f64) {
    if let Ok(scaling) = SCREENSHOT_SCALE.read() {
        // Skip transformation if no scaling was applied or dimensions are invalid
        if scaling.display_width == 0
            || scaling.display_height == 0
            || scaling.standard_width == 0
            || scaling.standard_height == 0
            || scaling.display_to_standard_scale_x <= 0.0
            || scaling.display_to_standard_scale_y <= 0.0
        {
            return (screen_x, screen_y);
        }

        // Subtract display origin offset to get display-relative coordinates
        let display_relative_x = screen_x - scaling.display_origin_x;
        let display_relative_y = screen_y - scaling.display_origin_y;

        // Transform from display-relative coordinates to standard resolution coordinates
        let standard_x = display_relative_x * scaling.display_to_standard_scale_x as f64;
        let standard_y = display_relative_y * scaling.display_to_standard_scale_y as f64;

        (standard_x, standard_y)
    } else {
        tracing::error!("Failed to acquire read lock on SCREENSHOT_SCALE");
        (screen_x, screen_y) // Return untransformed coordinates as fallback
    }
}

/// Get the current standard resolution being used
pub fn get_current_standard_resolution() -> Result<(u32, u32), String> {
    SCREENSHOT_SCALE
        .read()
        .map(|scaling| (scaling.standard_width, scaling.standard_height))
        .map_err(|_| "Failed to acquire read lock on SCREENSHOT_SCALE".to_string())
}

/// Transforms coordinates from scaled screenshot space to original screen space (LEGACY)
/// Maintained for backward compatibility - now uses standard resolution scaling
pub fn transform_to_screen_coordinates(scaled_x: f64, scaled_y: f64) -> (f64, f64) {
    // Coordinates refer to the last screenshot the agent took. When that was
    // one window on its own, they are pixels in that window's picture.
    if let CoordinateFrame::Window {
        window_id,
        origin,
        scale,
        ..
    } = current_frame()
    {
        let live_origin = live_window_origin(window_id).unwrap_or(origin);
        let (x, y) = window_image_to_screen(scaled_x, scaled_y, live_origin, scale);
        info!(
            "Transformed coordinates: window {} image ({}, {}) → screen ({:.1}, {:.1}) [origin: ({:.1}, {:.1}), scale: {:.3}]",
            window_id, scaled_x, scaled_y, x, y, live_origin.0, live_origin.1, scale
        );
        return (x, y);
    }
    // For backward compatibility, treat input as standard resolution coordinates
    transform_standard_to_screen_coordinates(scaled_x, scaled_y)
}

/// What the agent's coordinates are measured against: the last screenshot it
/// took. A full screenshot means screen coordinates at the standard
/// resolution; a screenshot of one window means pixels in that window's
/// picture, from its top-left corner.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum CoordinateFrame {
    Screen,
    Window {
        pid: i32,
        window_id: u32,
        /// The window's top-left corner in screen points when it was captured.
        /// Used only if the window can no longer be found; otherwise its live
        /// position is read, so a window moved since still gets the click.
        origin: (f64, f64),
        /// Screen points per image pixel.
        scale: f64,
    },
}

impl CoordinateFrame {
    /// The window this frame is measured against, if it is one window.
    pub fn window(&self) -> Option<(i32, u32)> {
        match *self {
            CoordinateFrame::Window { pid, window_id, .. } => Some((pid, window_id)),
            CoordinateFrame::Screen => None,
        }
    }
}

tokio::task_local! {
    /// Which agent the running computer action belongs to. Parallel sessions
    /// each keep their own frame: one session's window picture must never
    /// change how another session's coordinates are read.
    static AGENT_KEY: String;
}

/// The key for the agent with no session id (the Claude CLI's one agent).
pub const DEFAULT_AGENT_KEY: &str = "default";

/// Run `fut` as `agent`'s computer action, so frames and targets it sets are
/// its own.
pub async fn scoped_to_agent<F: std::future::Future>(agent: Option<&str>, fut: F) -> F::Output {
    AGENT_KEY
        .scope(agent.unwrap_or(DEFAULT_AGENT_KEY).to_string(), fut)
        .await
}

/// The agent the current action belongs to; `None` outside any agent action
/// (a Tauri command from the UI), which always reads screen coordinates.
pub fn agent_key() -> Option<String> {
    AGENT_KEY.try_with(|key| key.clone()).ok()
}

static FRAMES: Lazy<std::sync::Mutex<std::collections::HashMap<String, CoordinateFrame>>> =
    Lazy::new(|| std::sync::Mutex::new(std::collections::HashMap::new()));

/// Set by the screenshot the agent just took.
pub fn set_frame(frame: CoordinateFrame) {
    let Some(key) = agent_key() else {
        return;
    };
    if let Ok(mut frames) = FRAMES.lock() {
        frames.insert(key, frame);
    }
}

/// Put an agent back on screen coordinates (a new turn).
pub fn reset_frame(agent: &str) {
    if let Ok(mut frames) = FRAMES.lock() {
        frames.remove(agent);
    }
}

pub fn current_frame() -> CoordinateFrame {
    let Some(key) = agent_key() else {
        return CoordinateFrame::Screen;
    };
    FRAMES
        .lock()
        .ok()
        .and_then(|frames| frames.get(&key).copied())
        .unwrap_or(CoordinateFrame::Screen)
}

/// A pixel in a window's picture, as a screen point.
pub fn window_image_to_screen(x: f64, y: f64, origin: (f64, f64), scale: f64) -> (f64, f64) {
    (origin.0 + x * scale, origin.1 + y * scale)
}

/// A screen point, as a pixel in a window's picture.
pub fn screen_to_window_image(x: f64, y: f64, origin: (f64, f64), scale: f64) -> (f64, f64) {
    if scale <= 0.0 {
        return (x - origin.0, y - origin.1);
    }
    ((x - origin.0) / scale, (y - origin.1) / scale)
}

/// A screen point in whatever frame the agent is reading: the inverse of
/// [`transform_to_screen_coordinates`].
pub fn screen_to_frame(x: f64, y: f64) -> (f64, f64) {
    match current_frame() {
        CoordinateFrame::Window {
            window_id,
            origin,
            scale,
            ..
        } => {
            let live_origin = live_window_origin(window_id).unwrap_or(origin);
            screen_to_window_image(x, y, live_origin, scale)
        }
        CoordinateFrame::Screen => transform_screen_to_standard_coordinates(x, y),
    }
}

#[cfg(target_os = "macos")]
fn live_window_origin(window_id: u32) -> Option<(f64, f64)> {
    computer_use_ai_sdk::platforms::macos::display::list_window_records()
        .into_iter()
        .find(|w| w.id == window_id)
        .map(|w| (w.bounds.0, w.bounds.1))
}

#[cfg(not(target_os = "macos"))]
fn live_window_origin(_window_id: u32) -> Option<(f64, f64)> {
    None
}

/// Transforms coordinates from original screen space to scaled screenshot space (LEGACY)
/// Maintained for backward compatibility - now uses standard resolution scaling
pub fn transform_to_scaled_coordinates(original_x: f64, original_y: f64) -> (f64, f64) {
    // For backward compatibility, return standard resolution coordinates
    transform_screen_to_standard_coordinates(original_x, original_y)
}

/// Get current scaling information (for debugging/testing)
pub fn get_scaling_info() -> Result<ScalingInfo, String> {
    SCREENSHOT_SCALE
        .read()
        .map(|scaling| *scaling)
        .map_err(|_| "Failed to acquire read lock on SCREENSHOT_SCALE".to_string())
}

/// Reset scaling information to default values
pub fn reset_scaling_info() {
    if let Ok(mut scaling) = SCREENSHOT_SCALE.write() {
        *scaling = ScalingInfo::default();
        info!("Reset screenshot scaling info to default values");
    } else {
        tracing::error!("Failed to acquire write lock on SCREENSHOT_SCALE for reset");
    }
}

#[cfg(test)]
mod frame_tests {
    use super::*;

    #[test]
    fn a_window_picture_pixel_maps_to_the_screen_and_back() {
        // Calculator at (368, 410), pictured at one pixel per point.
        let (x, y) = window_image_to_screen(29.0, 320.0, (368.0, 410.0), 1.0);
        assert_eq!((x, y), (397.0, 730.0));
        assert_eq!(
            screen_to_window_image(x, y, (368.0, 410.0), 1.0),
            (29.0, 320.0)
        );
    }

    #[test]
    fn a_shrunk_picture_scales_back_up() {
        // A 2560-point-wide window sent as 1280 pixels: 2 points per pixel.
        let (x, y) = window_image_to_screen(100.0, 50.0, (0.0, 30.0), 2.0);
        assert_eq!((x, y), (200.0, 130.0));
    }

    #[tokio::test]
    async fn a_window_frame_is_the_agents_own() {
        let frame = CoordinateFrame::Window {
            pid: 70,
            window_id: 11502,
            origin: (368.0, 410.0),
            scale: 1.0,
        };
        scoped_to_agent(Some("frame-test-a"), async move { set_frame(frame) }).await;
        let other = scoped_to_agent(Some("frame-test-b"), async { current_frame() }).await;
        assert_eq!(other, CoordinateFrame::Screen);
        let own = scoped_to_agent(Some("frame-test-a"), async { current_frame() }).await;
        assert_eq!(own, frame);
        reset_frame("frame-test-a");
        let reset = scoped_to_agent(Some("frame-test-a"), async { current_frame() }).await;
        assert_eq!(reset, CoordinateFrame::Screen);
    }
}

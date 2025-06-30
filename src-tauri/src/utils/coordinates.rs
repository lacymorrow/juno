use std::sync::RwLock;
use once_cell::sync::Lazy;
use tracing::info;
use serde::Serialize;
use crate::constants::ui::standard_resolutions;

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
        // Legacy fields for backward compatibility
        original_width: 0,
        original_height: 0,
        scaled_width: 0,
        scaled_height: 0,
        scale_factor_x: 1.0,
        scale_factor_y: 1.0,
        scale_factor: 1.0,
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

    // Legacy fields for backward compatibility
    pub original_width: u32,
    pub original_height: u32,
    pub scaled_width: u32,
    pub scaled_height: u32,
    pub scale_factor_x: f32,
    pub scale_factor_y: f32,
    pub scale_factor: f32,
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
            original_width: 0,
            original_height: 0,
            scaled_width: 0,
            scaled_height: 0,
            scale_factor_x: 1.0,
            scale_factor_y: 1.0,
            scale_factor: 1.0,
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
    // Select the best standard resolution based on display aspect ratio
    let (standard_width, standard_height) = standard_resolutions::select_best_resolution(display_width, display_height);

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
    let safe_display_scale_x = if display_to_standard_scale_x.is_finite() && display_to_standard_scale_x > 0.0 {
        display_to_standard_scale_x
    } else {
        tracing::warn!("Invalid display to standard X scale factor: {}, using 1.0", display_to_standard_scale_x);
        1.0
    };

    let safe_display_scale_y = if display_to_standard_scale_y.is_finite() && display_to_standard_scale_y > 0.0 {
        display_to_standard_scale_y
    } else {
        tracing::warn!("Invalid display to standard Y scale factor: {}, using 1.0", display_to_standard_scale_y);
        1.0
    };

    let safe_screenshot_scale_x = if screenshot_to_standard_scale_x.is_finite() && screenshot_to_standard_scale_x > 0.0 {
        screenshot_to_standard_scale_x
    } else {
        tracing::warn!("Invalid screenshot to standard X scale factor: {}, using 1.0", screenshot_to_standard_scale_x);
        1.0
    };

    let safe_screenshot_scale_y = if screenshot_to_standard_scale_y.is_finite() && screenshot_to_standard_scale_y > 0.0 {
        screenshot_to_standard_scale_y
    } else {
        tracing::warn!("Invalid screenshot to standard Y scale factor: {}, using 1.0", screenshot_to_standard_scale_y);
        1.0
    };

    // Calculate legacy scale factors for backward compatibility
    let legacy_scale_x = if display_width > 0 {
        screenshot_width as f32 / display_width as f32
    } else {
        1.0
    };

    let legacy_scale_y = if display_height > 0 {
        screenshot_height as f32 / display_height as f32
    } else {
        1.0
    };

    let legacy_scale_factor = (legacy_scale_x * legacy_scale_y).sqrt();

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
            // Legacy fields for backward compatibility
            original_width: display_width,
            original_height: display_height,
            scaled_width: standard_width,
            scaled_height: standard_height,
            scale_factor_x: safe_display_scale_x,
            scale_factor_y: safe_display_scale_y,
            scale_factor: legacy_scale_factor,
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

/// Updates the scaling information with separate X and Y scale factors (LEGACY)
/// Maintained for backward compatibility
pub fn update_scaling_info_with_separate_factors(
    original_width: u32,
    original_height: u32,
    scaled_width: u32,
    scaled_height: u32,
    scale_factor_x: f32,
    scale_factor_y: f32,
) {
    // For legacy compatibility, treat this as display->screenshot scaling
    // and derive standard resolution scaling from it
    update_standard_resolution_scaling(
        original_width,
        original_height,
        scaled_width,
        scaled_height,
    );
}

/// Updates the scaling information when a screenshot is processed (LEGACY)
/// Maintained for backward compatibility
pub fn update_scaling_info(
    original_width: u32,
    original_height: u32,
    scaled_width: u32,
    scaled_height: u32,
    scale_factor: f32
) {
    // For legacy compatibility, treat this as display->screenshot scaling
    update_standard_resolution_scaling(
        original_width,
        original_height,
        scaled_width,
        scaled_height,
    );
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
    // First do the normal scaling calculation
    update_standard_resolution_scaling(
        display_width,
        display_height,
        screenshot_width,
        screenshot_height,
    );

    // Then update the display origin information
    if let Ok(mut scaling) = SCREENSHOT_SCALE.write() {
        scaling.display_origin_x = display_origin_x;
        scaling.display_origin_y = display_origin_y;
        scaling.display_id = display_id;

        info!("Updated display origin for multi-monitor: origin ({}, {}), display ID: {:?}",
            display_origin_x, display_origin_y, display_id);
    } else {
        tracing::error!("Failed to acquire write lock on SCREENSHOT_SCALE for display origin update");
    }
}

/// Transforms coordinates from standard resolution space to actual screen coordinates
/// This is the primary coordinate transformation for the Anthropic Computer Use API
/// NEW: Now accounts for display origin in multi-monitor setups
pub fn transform_standard_to_screen_coordinates(standard_x: f64, standard_y: f64) -> (f64, f64) {
    if let Ok(scaling) = SCREENSHOT_SCALE.read() {
        // Skip transformation if no scaling was applied or dimensions are invalid
        if scaling.display_width == 0 || scaling.display_height == 0 ||
           scaling.standard_width == 0 || scaling.standard_height == 0 ||
           scaling.display_to_standard_scale_x <= 0.0 || scaling.display_to_standard_scale_y <= 0.0 {
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
        if scaling.display_width == 0 || scaling.display_height == 0 ||
           scaling.standard_width == 0 || scaling.standard_height == 0 ||
           scaling.display_to_standard_scale_x <= 0.0 || scaling.display_to_standard_scale_y <= 0.0 {
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
    SCREENSHOT_SCALE.read()
        .map(|scaling| (scaling.standard_width, scaling.standard_height))
        .map_err(|_| "Failed to acquire read lock on SCREENSHOT_SCALE".to_string())
}

/// Transforms coordinates from scaled screenshot space to original screen space (LEGACY)
/// Maintained for backward compatibility - now uses standard resolution scaling
pub fn transform_to_screen_coordinates(scaled_x: f64, scaled_y: f64) -> (f64, f64) {
    // For backward compatibility, treat input as standard resolution coordinates
    transform_standard_to_screen_coordinates(scaled_x, scaled_y)
}

/// Transforms coordinates from original screen space to scaled screenshot space (LEGACY)
/// Maintained for backward compatibility - now uses standard resolution scaling
pub fn transform_to_scaled_coordinates(original_x: f64, original_y: f64) -> (f64, f64) {
    // For backward compatibility, return standard resolution coordinates
    transform_screen_to_standard_coordinates(original_x, original_y)
}

/// Get current scaling information (for debugging/testing)
pub fn get_scaling_info() -> Result<ScalingInfo, String> {
    SCREENSHOT_SCALE.read()
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

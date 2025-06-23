use std::sync::RwLock;
use once_cell::sync::Lazy;
use tracing::info;
use serde::Serialize;

// Global state to store the current screenshot scaling information
pub static SCREENSHOT_SCALE: Lazy<RwLock<ScalingInfo>> = Lazy::new(|| {
    RwLock::new(ScalingInfo {
        original_width: 0,
        original_height: 0,
        scaled_width: 0,
        scaled_height: 0,
        scale_factor_x: 1.0,  // Separate X scale factor
        scale_factor_y: 1.0,  // Separate Y scale factor
        scale_factor: 1.0,    // Kept for backward compatibility
    })
});

/// Represents scaling information between original screen and scaled screenshot
#[derive(Debug, Clone, Copy, Serialize)]
pub struct ScalingInfo {
    pub original_width: u32,
    pub original_height: u32,
    pub scaled_width: u32,
    pub scaled_height: u32,
    pub scale_factor_x: f32,  // X-axis scale factor
    pub scale_factor_y: f32,  // Y-axis scale factor
    pub scale_factor: f32,    // Legacy unified scale factor for backward compatibility
}

impl Default for ScalingInfo {
    fn default() -> Self {
        Self {
            original_width: 0,
            original_height: 0,
            scaled_width: 0,
            scaled_height: 0,
            scale_factor_x: 1.0,
            scale_factor_y: 1.0,
            scale_factor: 1.0,
        }
    }
}

/// Updates the scaling information with separate X and Y scale factors
pub fn update_scaling_info_with_separate_factors(
    original_width: u32,
    original_height: u32,
    scaled_width: u32,
    scaled_height: u32,
    scale_factor_x: f32,
    scale_factor_y: f32,
) {
    // Validate scale factors to prevent division by zero and infinite values
    let safe_scale_x = if scale_factor_x.is_finite() && scale_factor_x > 0.0 {
        scale_factor_x
    } else {
        tracing::warn!("Invalid X scale factor: {}, using 1.0", scale_factor_x);
        1.0
    };

    let safe_scale_y = if scale_factor_y.is_finite() && scale_factor_y > 0.0 {
        scale_factor_y
    } else {
        tracing::warn!("Invalid Y scale factor: {}, using 1.0", scale_factor_y);
        1.0
    };

    // Calculate legacy unified scale factor (use geometric mean for better accuracy)
    let legacy_scale_factor = (safe_scale_x * safe_scale_y).sqrt();

    if let Ok(mut scaling) = SCREENSHOT_SCALE.write() {
        *scaling = ScalingInfo {
            original_width,
            original_height,
            scaled_width,
            scaled_height,
            scale_factor_x: safe_scale_x,
            scale_factor_y: safe_scale_y,
            scale_factor: legacy_scale_factor,
        };
        info!("Updated screenshot scaling info: original: {}x{}, scaled: {}x{}, scale_x: {:.3}, scale_y: {:.3}",
            original_width, original_height, scaled_width, scaled_height, safe_scale_x, safe_scale_y);
    } else {
        tracing::error!("Failed to acquire write lock on SCREENSHOT_SCALE");
    }
}

/// Updates the scaling information when a screenshot is processed (legacy function)
pub fn update_scaling_info(
    original_width: u32,
    original_height: u32,
    scaled_width: u32,
    scaled_height: u32,
    scale_factor: f32
) {
    // Calculate separate X and Y scale factors
    let scale_factor_x = if original_width > 0 {
        scaled_width as f32 / original_width as f32
    } else {
        1.0
    };

    let scale_factor_y = if original_height > 0 {
        scaled_height as f32 / original_height as f32
    } else {
        1.0
    };

    // Use the new separate factors function
    update_scaling_info_with_separate_factors(
        original_width,
        original_height,
        scaled_width,
        scaled_height,
        scale_factor_x,
        scale_factor_y,
    );
}

/// Transforms coordinates from scaled screenshot space to original screen space
pub fn transform_to_screen_coordinates(scaled_x: f64, scaled_y: f64) -> (f64, f64) {
    if let Ok(scaling) = SCREENSHOT_SCALE.read() {
        // Skip transformation if no scaling was applied or dimensions are invalid
        if scaling.original_width == 0 || scaling.original_height == 0 ||
           scaling.scaled_width == 0 || scaling.scaled_height == 0 ||
           scaling.scale_factor_x <= 0.0 || scaling.scale_factor_y <= 0.0 {
            return (scaled_x, scaled_y);
        }

        // Transform coordinates using separate X and Y scale factors
        let original_x = scaled_x / scaling.scale_factor_x as f64;
        let original_y = scaled_y / scaling.scale_factor_y as f64;

        info!("Transformed coordinates: scaled ({}, {}) → original ({}, {}) using scale_x: {:.3}, scale_y: {:.3}",
            scaled_x, scaled_y, original_x, original_y, scaling.scale_factor_x, scaling.scale_factor_y);

        (original_x, original_y)
    } else {
        tracing::error!("Failed to acquire read lock on SCREENSHOT_SCALE");
        (scaled_x, scaled_y) // Return untransformed coordinates as fallback
    }
}

/// Transforms coordinates from original screen space to scaled screenshot space
pub fn transform_to_scaled_coordinates(original_x: f64, original_y: f64) -> (f64, f64) {
    if let Ok(scaling) = SCREENSHOT_SCALE.read() {
        // Skip transformation if no scaling was applied or dimensions are invalid
        if scaling.original_width == 0 || scaling.original_height == 0 ||
           scaling.scaled_width == 0 || scaling.scaled_height == 0 ||
           scaling.scale_factor_x <= 0.0 || scaling.scale_factor_y <= 0.0 {
            return (original_x, original_y);
        }

        // Transform coordinates using separate X and Y scale factors
        let scaled_x = original_x * scaling.scale_factor_x as f64;
        let scaled_y = original_y * scaling.scale_factor_y as f64;

        (scaled_x, scaled_y)
    } else {
        tracing::error!("Failed to acquire read lock on SCREENSHOT_SCALE");
        (original_x, original_y) // Return untransformed coordinates as fallback
    }
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
        *scaling = ScalingInfo {
            original_width: 0,
            original_height: 0,
            scaled_width: 0,
            scaled_height: 0,
            scale_factor_x: 1.0,
            scale_factor_y: 1.0,
            scale_factor: 1.0,
        };
        info!("Reset screenshot scaling info to default values");
    } else {
        tracing::error!("Failed to acquire write lock on SCREENSHOT_SCALE for reset");
    }
}

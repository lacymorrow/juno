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
        scale_factor: 1.0,  // Default to no scaling
    })
});

/// Represents scaling information between original screen and scaled screenshot
#[derive(Debug, Clone, Copy, Serialize)]
pub struct ScalingInfo {
    pub original_width: u32,
    pub original_height: u32,
    pub scaled_width: u32,
    pub scaled_height: u32,
    pub scale_factor: f32,
}

/// Updates the scaling information when a screenshot is processed
pub fn update_scaling_info(
    original_width: u32,
    original_height: u32,
    scaled_width: u32,
    scaled_height: u32,
    scale_factor: f32
) {
    if let Ok(mut scaling) = SCREENSHOT_SCALE.write() {
        *scaling = ScalingInfo {
            original_width,
            original_height,
            scaled_width,
            scaled_height,
            scale_factor,
        };
        info!("Updated screenshot scaling info: original: {}x{}, scaled: {}x{}, factor: {}",
            original_width, original_height, scaled_width, scaled_height, scale_factor);
    } else {
        tracing::error!("Failed to acquire write lock on SCREENSHOT_SCALE");
    }
}

/// Transforms coordinates from scaled screenshot space to original screen space
pub fn transform_to_screen_coordinates(scaled_x: f64, scaled_y: f64) -> (f64, f64) {
    if let Ok(scaling) = SCREENSHOT_SCALE.read() {
        // Skip transformation if no scaling was applied
        if scaling.original_width == 0 || scaling.original_height == 0 ||
           scaling.scaled_width == 0 || scaling.scaled_height == 0 {
            return (scaled_x, scaled_y);
        }

        // Transform coordinates using the inverse of the scale factor
        let original_x = scaled_x / scaling.scale_factor as f64;
        let original_y = scaled_y / scaling.scale_factor as f64;

        info!("Transformed coordinates: scaled ({}, {}) → original ({}, {})",
            scaled_x, scaled_y, original_x, original_y);

        (original_x, original_y)
    } else {
        tracing::error!("Failed to acquire read lock on SCREENSHOT_SCALE");
        (scaled_x, scaled_y) // Return untransformed coordinates as fallback
    }
}

/// Transforms coordinates from original screen space to scaled screenshot space
pub fn transform_to_scaled_coordinates(original_x: f64, original_y: f64) -> (f64, f64) {
    if let Ok(scaling) = SCREENSHOT_SCALE.read() {
        // Skip transformation if no scaling was applied
        if scaling.original_width == 0 || scaling.original_height == 0 ||
           scaling.scaled_width == 0 || scaling.scaled_height == 0 {
            return (original_x, original_y);
        }

        // Transform coordinates using the scale factor
        let scaled_x = original_x * scaling.scale_factor as f64;
        let scaled_y = original_y * scaling.scale_factor as f64;

        (scaled_x, scaled_y)
    } else {
        tracing::error!("Failed to acquire read lock on SCREENSHOT_SCALE");
        (original_x, original_y) // Return untransformed coordinates as fallback
    }
}

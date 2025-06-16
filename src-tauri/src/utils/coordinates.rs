use std::sync::RwLock;
use once_cell::sync::Lazy;
use tracing::{info, warn, debug};
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

/// Detects scaling from a base64 screenshot and updates scaling info
/// This should be called whenever a screenshot is taken for the AI agent
pub fn detect_and_update_scaling_from_screenshot(screenshot_base64: &str) -> Result<(), String> {
    use base64::Engine;

    // Decode the base64 image
    let engine = base64::engine::general_purpose::STANDARD;
    let image_data = engine.decode(screenshot_base64)
        .map_err(|e| format!("Failed to decode screenshot base64: {}", e))?;

    // Parse the image to get dimensions
    let img = image::load_from_memory(&image_data)
        .map_err(|e| format!("Failed to parse screenshot image: {}", e))?;

    let screenshot_width = img.width();
    let screenshot_height = img.height();

    // Get the actual screen resolution using system APIs
    let (screen_width, screen_height) = get_actual_screen_resolution()
        .ok_or_else(|| "Failed to get actual screen resolution".to_string())?;

    // Calculate scale factor
    let scale_factor_x = screenshot_width as f32 / screen_width as f32;
    let scale_factor_y = screenshot_height as f32 / screen_height as f32;

    // Use the average scale factor (they should be the same for uniform scaling)
    let scale_factor = (scale_factor_x + scale_factor_y) / 2.0;

    debug!("Screenshot scaling detection: screen={}x{}, screenshot={}x{}, scale_factor={}",
        screen_width, screen_height, screenshot_width, screenshot_height, scale_factor);

    // Update the global scaling info
    update_scaling_info(
        screen_width,
        screen_height,
        screenshot_width,
        screenshot_height,
        scale_factor
    );

    Ok(())
}

/// Gets the actual screen resolution using macOS APIs
fn get_actual_screen_resolution() -> Option<(u32, u32)> {
    #[cfg(target_os = "macos")]
    {
        use core_graphics::display::{CGMainDisplayID, CGDisplayBounds};

        unsafe {
            let main_display = CGMainDisplayID();
            let bounds = CGDisplayBounds(main_display);
            let width = bounds.size.width as u32;
            let height = bounds.size.height as u32;
            debug!("Actual screen resolution from CGDisplay: {}x{}", width, height);
            Some((width, height))
        }
    }

    #[cfg(not(target_os = "macos"))]
    {
        warn!("get_actual_screen_resolution: Not implemented for non-macOS platforms");
        None
    }
}

/// Transforms coordinates from scaled screenshot space to original screen space
pub fn transform_to_screen_coordinates(scaled_x: f64, scaled_y: f64) -> (f64, f64) {
    if let Ok(scaling) = SCREENSHOT_SCALE.read() {
        // Skip transformation if no scaling was applied or scaling info not set
        if scaling.original_width == 0 || scaling.original_height == 0 ||
           scaling.scaled_width == 0 || scaling.scaled_height == 0 {
            warn!("No scaling info available, returning coordinates unchanged: ({}, {})", scaled_x, scaled_y);
            return (scaled_x, scaled_y);
        }

        // Transform coordinates using the inverse of the scale factor
        let original_x = scaled_x / scaling.scale_factor as f64;
        let original_y = scaled_y / scaling.scale_factor as f64;

        debug!("Transformed coordinates: scaled ({}, {}) → original ({}, {}) [scale_factor={}]",
            scaled_x, scaled_y, original_x, original_y, scaling.scale_factor);

        (original_x, original_y)
    } else {
        tracing::error!("Failed to acquire read lock on SCREENSHOT_SCALE");
        (scaled_x, scaled_y) // Return untransformed coordinates as fallback
    }
}

/// Transforms coordinates from original screen space to scaled screenshot space
pub fn transform_to_scaled_coordinates(original_x: f64, original_y: f64) -> (f64, f64) {
    if let Ok(scaling) = SCREENSHOT_SCALE.read() {
        // Skip transformation if no scaling was applied or scaling info not set
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

/// Gets current scaling information for debugging
pub fn get_current_scaling_info() -> Result<ScalingInfo, String> {
    SCREENSHOT_SCALE.read()
        .map(|scaling| *scaling)
        .map_err(|_| "Failed to acquire read lock on SCREENSHOT_SCALE".to_string())
}

/// Force update scaling info to native resolution (no scaling)
pub fn set_native_resolution_scaling() -> Result<(), String> {
    let (screen_width, screen_height) = get_actual_screen_resolution()
        .ok_or_else(|| "Failed to get actual screen resolution".to_string())?;

    update_scaling_info(
        screen_width,
        screen_height,
        screen_width,
        screen_height,
        1.0
    );

    info!("Set native resolution scaling: {}x{}", screen_width, screen_height);
    Ok(())
}

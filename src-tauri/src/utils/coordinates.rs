use std::sync::RwLock;
use once_cell::sync::Lazy;
use tracing::info;
use serde::Serialize;

// Global state to store the current screenshot scaling information
pub static SCREENSHOT_SCALE: Lazy<RwLock<DisplayContext>> = Lazy::new(|| {
    RwLock::new(DisplayContext {
        original_width: 0,
        original_height: 0,
        scaled_width: 0,
        scaled_height: 0,
        scale_factor: 1.0,
        display_origin_x: 0.0,
        display_origin_y: 0.0,
        display_id: 0,
        is_primary_display: true,
    })
});

/// Represents display context including scaling and position information
#[derive(Debug, Clone, Copy, Serialize)]
pub struct DisplayContext {
    pub original_width: u32,
    pub original_height: u32,
    pub scaled_width: u32,
    pub scaled_height: u32,
    pub scale_factor: f32,
    pub display_origin_x: f64,
    pub display_origin_y: f64,
    pub display_id: u32,
    pub is_primary_display: bool,
}

/// Legacy ScalingInfo for backward compatibility
#[derive(Debug, Clone, Copy, Serialize)]
pub struct ScalingInfo {
    pub original_width: u32,
    pub original_height: u32,
    pub scaled_width: u32,
    pub scaled_height: u32,
    pub scale_factor: f32,
}

/// Updates the display context when a screenshot is processed
pub fn update_display_context(
    original_width: u32,
    original_height: u32,
    scaled_width: u32,
    scaled_height: u32,
    scale_factor: f32,
    display_origin_x: f64,
    display_origin_y: f64,
    display_id: u32,
    is_primary_display: bool,
) {
    if let Ok(mut context) = SCREENSHOT_SCALE.write() {
        *context = DisplayContext {
            original_width,
            original_height,
            scaled_width,
            scaled_height,
            scale_factor,
            display_origin_x,
            display_origin_y,
            display_id,
            is_primary_display,
        };
        info!("Updated display context: original: {}x{}, scaled: {}x{}, factor: {}, origin: ({}, {}), display_id: {}, primary: {}",
            original_width, original_height, scaled_width, scaled_height, scale_factor,
            display_origin_x, display_origin_y, display_id, is_primary_display);
    } else {
        tracing::error!("Failed to acquire write lock on SCREENSHOT_SCALE");
    }
}

/// Legacy function for backward compatibility
pub fn update_scaling_info(
    original_width: u32,
    original_height: u32,
    scaled_width: u32,
    scaled_height: u32,
    scale_factor: f32
) {
    update_display_context(
        original_width, 
        original_height, 
        scaled_width, 
        scaled_height, 
        scale_factor,
        0.0, // Assume primary display origin
        0.0,
        0,
        true
    );
}

/// Transforms coordinates from scaled screenshot space to global screen coordinates
/// This accounts for both scaling and display offset
pub fn transform_to_screen_coordinates(scaled_x: f64, scaled_y: f64) -> (f64, f64) {
    if let Ok(context) = SCREENSHOT_SCALE.read() {
        // Skip transformation if no context was set
        if context.original_width == 0 || context.original_height == 0 ||
           context.scaled_width == 0 || context.scaled_height == 0 {
            info!("No display context available, returning coordinates unchanged: ({}, {})", scaled_x, scaled_y);
            return (scaled_x, scaled_y);
        }

        // First, transform from scaled screenshot space to display-relative coordinates
        let display_relative_x = scaled_x / context.scale_factor as f64;
        let display_relative_y = scaled_y / context.scale_factor as f64;

        // Then, add the display offset to get global screen coordinates
        let global_x = display_relative_x + context.display_origin_x;
        let global_y = display_relative_y + context.display_origin_y;

        info!("Transformed coordinates: scaled ({}, {}) → display-relative ({}, {}) → global ({}, {})",
            scaled_x, scaled_y, display_relative_x, display_relative_y, global_x, global_y);

        (global_x, global_y)
    } else {
        tracing::error!("Failed to acquire read lock on SCREENSHOT_SCALE");
        (scaled_x, scaled_y) // Return untransformed coordinates as fallback
    }
}

/// Transforms coordinates from global screen space to scaled screenshot space
pub fn transform_to_scaled_coordinates(global_x: f64, global_y: f64) -> (f64, f64) {
    if let Ok(context) = SCREENSHOT_SCALE.read() {
        // Skip transformation if no context was set
        if context.original_width == 0 || context.original_height == 0 ||
           context.scaled_width == 0 || context.scaled_height == 0 {
            return (global_x, global_y);
        }

        // First, convert from global coordinates to display-relative coordinates
        let display_relative_x = global_x - context.display_origin_x;
        let display_relative_y = global_y - context.display_origin_y;

        // Then, scale to screenshot coordinates
        let scaled_x = display_relative_x * context.scale_factor as f64;
        let scaled_y = display_relative_y * context.scale_factor as f64;

        (scaled_x, scaled_y)
    } else {
        tracing::error!("Failed to acquire read lock on SCREENSHOT_SCALE");
        (global_x, global_y) // Return untransformed coordinates as fallback
    }
}

/// Get the current display context
pub fn get_display_context() -> Option<DisplayContext> {
    SCREENSHOT_SCALE.read().ok().map(|context| *context)
}

use std::sync::RwLock;
use once_cell::sync::Lazy;
use tracing::info;
use serde::Serialize;

#[cfg(target_os = "macos")]
use std::process::Command;

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
/// IMPORTANT: This function uses the provided scale_factor parameter as-is for both X and Y axes.
/// It does NOT calculate scale factors from the provided dimensions.
/// If you need scale factors calculated from dimensions, use update_scaling_info_with_separate_factors directly.
pub fn update_scaling_info(
    original_width: u32,
    original_height: u32,
    scaled_width: u32,
    scaled_height: u32,
    scale_factor: f32
) {
    // Validate scale factor to prevent division by zero and infinite values
    let safe_scale_factor = if scale_factor.is_finite() && scale_factor > 0.0 {
        scale_factor
    } else {
        tracing::warn!("Invalid scale factor: {}, using 1.0", scale_factor);
        1.0
    };

    // Use the provided scale_factor for both X and Y axes (uniform scaling)
    // The dimensions are stored for reference but scale factors come from the parameter
    update_scaling_info_with_separate_factors(
        original_width,
        original_height,
        scaled_width,
        scaled_height,
        safe_scale_factor,
        safe_scale_factor,
    );

    info!("Legacy scaling update: using provided scale_factor {:.3} for both axes (dimensions: {}x{} → {}x{})",
        safe_scale_factor, original_width, original_height, scaled_width, scaled_height);
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
        let mut original_x = scaled_x / scaling.scale_factor_x as f64;
        let mut original_y = scaled_y / scaling.scale_factor_y as f64;

        // Apply macOS system UI offsets to account for menu bar and dock
        let (menu_bar_height, dock_offset) = get_macos_system_ui_offsets();

        // Screenshots typically exclude the menu bar, so we need to add it back for global coordinates
        original_y += menu_bar_height;

        // Note: dock_offset is typically only relevant for bottom coordinates near the dock
        // We don't automatically add it since most clicks aren't near the dock area

        info!("Transformed coordinates: scaled ({}, {}) → screen ({}, {}) using scale_x: {:.3}, scale_y: {:.3}, menu_bar_offset: {:.1}",
            scaled_x, scaled_y, original_x, original_y, scaling.scale_factor_x, scaling.scale_factor_y, menu_bar_height);

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

        // Apply reverse macOS system UI offsets
        let (menu_bar_height, _dock_offset) = get_macos_system_ui_offsets();

        // Remove menu bar offset since screenshots exclude it
        let adjusted_x = original_x;
        let adjusted_y = original_y - menu_bar_height;

        // Transform coordinates using separate X and Y scale factors
        let scaled_x = adjusted_x * scaling.scale_factor_x as f64;
        let scaled_y = adjusted_y * scaling.scale_factor_y as f64;

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

#[cfg(target_os = "macos")]
/// Get macOS system UI offsets (menu bar height, dock size)
/// Returns (menu_bar_height, dock_height) in pixels
fn get_macos_system_ui_offsets() -> (f64, f64) {
    // Get menu bar height - standard on macOS is 24-28 pixels
    // This is the most common source of coordinate offset issues
    let menu_bar_height = 24.0; // Conservative estimate for macOS menu bar

    // Get dock offset - this is trickier as it can be hidden/auto-hide
    // For now, we'll detect if dock is visible and affecting coordinates
    let dock_offset = if is_dock_affecting_coordinates() {
        get_dock_size()
    } else {
        0.0
    };

    tracing::debug!("macOS UI offsets: menu_bar_height={}, dock_offset={}", menu_bar_height, dock_offset);
    (menu_bar_height, dock_offset)
}

#[cfg(target_os = "macos")]
/// Check if the dock is currently affecting coordinate calculations
fn is_dock_affecting_coordinates() -> bool {
    // Use defaults read to check dock autohide setting
    let output = Command::new("defaults")
        .args(&["read", "com.apple.dock", "autohide"])
        .output();

    match output {
        Ok(result) => {
            let autohide_enabled = String::from_utf8_lossy(&result.stdout).trim() == "1";

            if autohide_enabled {
                // If autohide is enabled, dock might still be visible
                // We'd need to check if it's currently shown
                false // For now, assume hidden dock doesn't affect coordinates
            } else {
                true // Dock is always visible, affects coordinates
            }
        }
        Err(_) => false // Assume no dock interference if we can't determine
    }
}

#[cfg(target_os = "macos")]
/// Get dock size when it's affecting coordinates
fn get_dock_size() -> f64 {
    // Use defaults to get dock tile size and position
    let tile_size_output = Command::new("defaults")
        .args(&["read", "com.apple.dock", "tilesize"])
        .output();

    let orientation_output = Command::new("defaults")
        .args(&["read", "com.apple.dock", "orientation"])
        .output();

    let tile_size = match tile_size_output {
        Ok(result) => {
            String::from_utf8_lossy(&result.stdout)
                .trim()
                .parse::<f64>()
                .unwrap_or(64.0) // Default macOS dock tile size
        }
        Err(_) => 64.0
    };

    let orientation = match orientation_output {
        Ok(result) => String::from_utf8_lossy(&result.stdout).trim().to_string(),
        Err(_) => "bottom".to_string()
    };

    // Dock affects coordinates differently based on position
    match orientation.as_str() {
        "left" | "right" => 0.0, // Side docks don't affect Y coordinates
        "bottom" | _ => tile_size + 16.0, // Bottom dock + some margin
    }
}

#[cfg(not(target_os = "macos"))]
fn get_macos_system_ui_offsets() -> (f64, f64) {
    (0.0, 0.0)
}

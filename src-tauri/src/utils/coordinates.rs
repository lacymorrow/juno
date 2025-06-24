use std::sync::RwLock;
use once_cell::sync::Lazy;
use tracing::info;
use serde::Serialize;

#[cfg(target_os = "macos")]
use {
    std::process::Command,
};

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
        menu_bar_height: 0.0, // Dynamic menu bar height
        dock_height: 0.0,     // Dynamic dock height
        dock_position: DockPosition::Bottom, // Dynamic dock position
    })
});

/// Dock position on screen
#[derive(Debug, Clone, Copy, Serialize)]
pub enum DockPosition {
    Bottom,
    Left,
    Right,
    Hidden,
}

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
    pub menu_bar_height: f64, // Dynamic menu bar height
    pub dock_height: f64,     // Dynamic dock height
    pub dock_position: DockPosition, // Dynamic dock position
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
            menu_bar_height: 0.0,
            dock_height: 0.0,
            dock_position: DockPosition::Bottom,
        }
    }
}

/// Detect current dock configuration on macOS
#[cfg(target_os = "macos")]
fn detect_dock_info() -> (f64, DockPosition) {

    // First check if dock is hidden
    let auto_hide_output = Command::new("defaults")
        .args(&["read", "com.apple.dock", "autohide"])
        .output();

    let dock_is_hidden = if let Ok(output) = auto_hide_output {
        let output_str = String::from_utf8_lossy(&output.stdout);
        output_str.trim() == "1"
    } else {
        false
    };

    if dock_is_hidden {
        return (0.0, DockPosition::Hidden);
    }

    // Get dock orientation
    let orientation_output = Command::new("defaults")
        .args(&["read", "com.apple.dock", "orientation"])
        .output();

    let dock_position = if let Ok(output) = orientation_output {
        let output_str = String::from_utf8_lossy(&output.stdout);
        match output_str.trim() {
            "left" => DockPosition::Left,
            "right" => DockPosition::Right,
            _ => DockPosition::Bottom,
        }
    } else {
        DockPosition::Bottom
    };

    // Get dock size (tilesize)
    let tilesize_output = Command::new("defaults")
        .args(&["read", "com.apple.dock", "tilesize"])
        .output();

    let dock_tile_size = if let Ok(output) = tilesize_output {
        let output_str = String::from_utf8_lossy(&output.stdout);
        output_str.trim().parse::<f64>().unwrap_or(64.0)
    } else {
        64.0 // Default dock tile size
    };

    // Calculate dock height/width based on tile size and padding
    // Dock height is typically tilesize + some padding for the dock background
    let dock_dimension = dock_tile_size + 16.0; // 16px padding for dock background

    (dock_dimension, dock_position)
}

#[cfg(not(target_os = "macos"))]
fn detect_dock_info() -> (f64, DockPosition) {
    (0.0, DockPosition::Hidden)
}

/// Detect menu bar height on macOS
#[cfg(target_os = "macos")]
fn detect_menu_bar_height() -> f64 {
    // Use system_profiler to get display information
    let output = Command::new("system_profiler")
        .args(&["SPDisplaysDataType", "-json"])
        .output();

    if let Ok(output) = output {
        // For now, use a reasonable default since parsing the JSON would require serde_json
        // In most cases, the menu bar is 24px on standard displays, 28px on notched displays

        // Check if this is a MacBook with notch (approximate detection)
        let output_str = String::from_utf8_lossy(&output.stdout);
        if output_str.contains("MacBook") && output_str.contains("Liquid Retina") {
            return 28.0; // Notched MacBooks have slightly taller menu bars
        }
    }

    // Standard menu bar height for most Macs
    24.0
}

#[cfg(not(target_os = "macos"))]
fn detect_menu_bar_height() -> f64 {
    0.0
}

/// Updates the scaling information with separate X and Y scale factors and dynamic system UI detection
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

    // Detect dynamic system UI
    let menu_bar_height = detect_menu_bar_height();
    let (dock_height, dock_position) = detect_dock_info();

    if let Ok(mut scaling) = SCREENSHOT_SCALE.write() {
        *scaling = ScalingInfo {
            original_width,
            original_height,
            scaled_width,
            scaled_height,
            scale_factor_x: safe_scale_x,
            scale_factor_y: safe_scale_y,
            scale_factor: legacy_scale_factor,
            menu_bar_height,
            dock_height,
            dock_position,
        };
        info!("Updated screenshot scaling info: original: {}x{}, scaled: {}x{}, scale_x: {:.3}, scale_y: {:.3}, menu_bar: {:.1}px, dock: {:.1}px {:?}",
            original_width, original_height, scaled_width, scaled_height, safe_scale_x, safe_scale_y, menu_bar_height, dock_height, dock_position);
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
/// Now includes dynamic dock and menu bar offset compensation
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

        // CRITICAL FIX: Add menu bar height offset
        // Screenshots exclude the menu bar area, but global coordinates include it
        original_y += scaling.menu_bar_height;

        // CRITICAL FIX: Add dock offset based on position
        match scaling.dock_position {
            DockPosition::Bottom => {
                // For bottom dock, no additional offset needed as it's at the bottom
                // The coordinate system already accounts for it
            }
            DockPosition::Left => {
                // For left dock, add width offset to X coordinate
                original_x += scaling.dock_height;
            }
            DockPosition::Right => {
                // For right dock, no offset needed as it's on the right edge
            }
            DockPosition::Hidden => {
                // No dock offset needed
            }
        }

        info!("Transformed coordinates: scaled ({}, {}) → screen ({}, {}) using scale_x: {:.3}, scale_y: {:.3}, menu_bar_offset: {:.1}px, dock_offset: {:.1}px {:?}",
            scaled_x, scaled_y, original_x, original_y, scaling.scale_factor_x, scaling.scale_factor_y, scaling.menu_bar_height, scaling.dock_height, scaling.dock_position);

        (original_x, original_y)
    } else {
        tracing::error!("Failed to acquire read lock on SCREENSHOT_SCALE");
        (scaled_x, scaled_y) // Return untransformed coordinates as fallback
    }
}

/// Transforms coordinates from original screen space to scaled screenshot space
/// Now includes dynamic dock and menu bar offset compensation
pub fn transform_to_scaled_coordinates(original_x: f64, original_y: f64) -> (f64, f64) {
    if let Ok(scaling) = SCREENSHOT_SCALE.read() {
        // Skip transformation if no scaling was applied or dimensions are invalid
        if scaling.original_width == 0 || scaling.original_height == 0 ||
           scaling.scaled_width == 0 || scaling.scaled_height == 0 ||
           scaling.scale_factor_x <= 0.0 || scaling.scale_factor_y <= 0.0 {
            return (original_x, original_y);
        }

        // CRITICAL FIX: Remove menu bar height offset before scaling
        let mut screen_x = original_x;
        let mut screen_y = original_y - scaling.menu_bar_height;

        // CRITICAL FIX: Remove dock offset based on position
        match scaling.dock_position {
            DockPosition::Bottom => {
                // For bottom dock, no additional offset needed
            }
            DockPosition::Left => {
                // For left dock, subtract width offset from X coordinate
                screen_x -= scaling.dock_height;
            }
            DockPosition::Right => {
                // For right dock, no offset needed
            }
            DockPosition::Hidden => {
                // No dock offset needed
            }
        }

        // Transform coordinates using separate X and Y scale factors
        let scaled_x = screen_x * scaling.scale_factor_x as f64;
        let scaled_y = screen_y * scaling.scale_factor_y as f64;

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
            menu_bar_height: 0.0,
            dock_height: 0.0,
            dock_position: DockPosition::Bottom,
        };
        info!("Reset screenshot scaling info to default values");
    } else {
        tracing::error!("Failed to acquire write lock on SCREENSHOT_SCALE for reset");
    }
}

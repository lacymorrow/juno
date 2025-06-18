use core_graphics::display::{
    CGDirectDisplayID, CGDisplayBounds, CGGetActiveDisplayList, CGMainDisplayID, CGRect,
};
use core_graphics::geometry::{CGPoint, CGSize};
use tracing::{debug, trace};
use crate::AutomationError;
use std::collections::HashMap;

/// Information about a display
#[derive(Debug, Clone)]
pub struct DisplayInfo {
    pub id: CGDirectDisplayID,
    pub bounds: CGRect,
    pub is_main: bool,
}

/// Get information about all active displays
pub fn get_active_displays() -> Result<Vec<DisplayInfo>, AutomationError> {
    unsafe {
        let max_displays = 32;
        let mut display_ids: Vec<CGDirectDisplayID> = vec![0; max_displays];
        let mut display_count: u32 = 0;

        let result = CGGetActiveDisplayList(
            max_displays as u32,
            display_ids.as_mut_ptr(),
            &mut display_count,
        );

        if result != 0 {
            return Err(AutomationError::PlatformError(
                format!("Failed to get active display list: {}", result)
            ));
        }

        display_ids.truncate(display_count as usize);
        let main_display_id = CGMainDisplayID();

        let mut displays = Vec::new();
        for &display_id in &display_ids {
            let bounds = CGDisplayBounds(display_id);
            displays.push(DisplayInfo {
                id: display_id,
                bounds,
                is_main: display_id == main_display_id,
            });
        }

        trace!("Found {} active displays", displays.len());
        Ok(displays)
    }
}

/// Get the main display information
pub fn get_main_display() -> Result<DisplayInfo, AutomationError> {
    unsafe {
        let main_display_id = CGMainDisplayID();
        let bounds = CGDisplayBounds(main_display_id);

        Ok(DisplayInfo {
            id: main_display_id,
            bounds,
            is_main: true,
        })
    }
}

/// Check if a point is inside a rectangle
fn point_in_rect(point: CGPoint, rect: CGRect) -> bool {
    point.x >= rect.origin.x
        && point.x < rect.origin.x + rect.size.width
        && point.y >= rect.origin.y
        && point.y < rect.origin.y + rect.size.height
}

/// Calculate distance from point to rectangle center
fn distance_to_rect_center(point: CGPoint, rect: CGRect) -> f64 {
    let center_x = rect.origin.x + rect.size.width / 2.0;
    let center_y = rect.origin.y + rect.size.height / 2.0;
    let dx = point.x - center_x;
    let dy = point.y - center_y;
    (dx * dx + dy * dy).sqrt()
}

/// Find the display that contains the given point
pub fn find_display_containing_point(point: CGPoint) -> Result<DisplayInfo, AutomationError> {
    let displays = get_active_displays()?;

    // First, check if point is exactly within any display bounds
    for display_info in &displays {
        if point_in_rect(point, display_info.bounds) {
            debug!("Point ({}, {}) found in display {}", point.x, point.y, display_info.id);
            return Ok(display_info.clone());
        }
    }

        // If not found in any display, find the closest display
    let mut closest_display = None;
    let mut min_distance = f64::MAX;

    for display_info in &displays {
        let distance = distance_to_rect_center(point, display_info.bounds);
        if distance < min_distance {
            min_distance = distance;
            closest_display = Some(display_info.clone());
        }
    }

    match closest_display {
        Some(display_info) => {
            debug!("Point ({}, {}) not in any display, using closest display {}",
                   point.x, point.y, display_info.id);
            Ok(display_info)
        }
        None => {
            // Fallback to main display
            debug!("No displays found, falling back to main display");
            get_main_display()
        }
    }
}

/// Adjust coordinates for multi-monitor setup
/// Returns the adjusted coordinates that should be used for the event
pub fn adjust_coordinates_for_display(
    x: f64,
    y: f64,
    target_display: Option<DisplayInfo>
) -> Result<(f64, f64), AutomationError> {
    let point = CGPoint::new(x, y);

    // If target display is provided, use it; otherwise find the appropriate display
    let display_info = match target_display {
        Some(d) => d,
        None => find_display_containing_point(point)?,
    };

    // Clamp coordinates to display bounds to ensure they're valid
    let bounds = display_info.bounds;
    let adjusted_x = x.max(bounds.origin.x).min(bounds.origin.x + bounds.size.width - 1.0);
    let adjusted_y = y.max(bounds.origin.y).min(bounds.origin.y + bounds.size.height - 1.0);

    // Log if coordinates were adjusted
    if x != adjusted_x || y != adjusted_y {
        debug!("Coordinates clamped to display {}: ({}, {}) → ({}, {})",
               display_info.id, x, y, adjusted_x, adjusted_y);
    }

    Ok((adjusted_x, adjusted_y))
}

/// Get debug information about all displays
pub fn get_displays_debug_info() -> String {
    match get_active_displays() {
        Ok(displays) => {
            let mut info = format!("Active displays ({}): ", displays.len());
            for (i, display_info) in displays.iter().enumerate() {
                if i > 0 { info.push_str(", "); }
                info.push_str(&format!(
                    "{}[{}x{}+{}+{}{}]",
                    display_info.id,
                    display_info.bounds.size.width as i32,
                    display_info.bounds.size.height as i32,
                    display_info.bounds.origin.x as i32,
                    display_info.bounds.origin.y as i32,
                    if display_info.is_main { "*" } else { "" }
                ));
            }
            info
        }
        Err(e) => format!("Error getting display info: {}", e),
    }
}

/// Get display bounds as a map for easier lookup
pub fn get_display_bounds_map() -> Result<HashMap<CGDirectDisplayID, CGRect>, AutomationError> {
    let displays = get_active_displays()?;
    let mut bounds_map = HashMap::new();

    for display_info in displays {
        bounds_map.insert(display_info.id, display_info.bounds);
    }

    Ok(bounds_map)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_point_in_rect() {
        let rect = CGRect::new(
            &CGPoint::new(0.0, 0.0),
            &CGSize::new(100.0, 100.0)
        );

        assert!(point_in_rect(CGPoint::new(50.0, 50.0), rect));
        assert!(point_in_rect(CGPoint::new(0.0, 0.0), rect));
        assert!(point_in_rect(CGPoint::new(99.0, 99.0), rect));
        assert!(!point_in_rect(CGPoint::new(100.0, 100.0), rect));
        assert!(!point_in_rect(CGPoint::new(-1.0, 50.0), rect));
    }

    #[test]
    fn test_distance_to_rect_center() {
        let rect = CGRect::new(
            &CGPoint::new(0.0, 0.0),
            &CGSize::new(100.0, 100.0)
        );

        // Center point should have 0 distance
        let distance = distance_to_rect_center(CGPoint::new(50.0, 50.0), rect);
        assert!((distance - 0.0).abs() < 0.001);

        // Corner point should have sqrt(50^2 + 50^2) distance
        let distance = distance_to_rect_center(CGPoint::new(0.0, 0.0), rect);
        let expected = (50.0 * 50.0 + 50.0 * 50.0).sqrt();
        assert!((distance - expected).abs() < 0.001);
    }
}

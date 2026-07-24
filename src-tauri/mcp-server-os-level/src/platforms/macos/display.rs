use core_graphics::display::{
    CGDirectDisplayID, CGDisplayBounds, CGGetActiveDisplayList, CGMainDisplayID, CGRect,
};
use core_graphics::geometry::CGPoint;
#[cfg(test)]
use core_graphics::geometry::CGSize;
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

// ── Visible window listing via CGWindowListCopyWindowInfo ──────────────────

use serde::Serialize;

/// Window info returned by `list_visible_windows`.
#[derive(Debug, Clone, Serialize)]
pub struct VisibleWindowInfo {
    pub app_name: String,
    pub window_title: Option<String>,
    /// (x, y) in screen coordinates (top-left origin, points)
    pub position: (f64, f64),
    /// (width, height) in points
    pub size: (f64, f64),
    pub is_frontmost: bool,
    pub layer: i32,
}

// CGWindowListCopyWindowInfo is not wrapped by core-graphics crate at v0.24
extern "C" {
    fn CGWindowListCopyWindowInfo(
        option: u32,
        relative_to_window: u32,
    ) -> core_foundation_sys::array::CFArrayRef;
}

const CG_WINDOW_LIST_OPTION_ON_SCREEN_ONLY: u32 = 1;
const CG_WINDOW_LIST_EXCLUDE_DESKTOP_ELEMENTS: u32 = 1 << 4;

// Convert a CFStringRef to a Rust String.
// Caller must ensure `s` remains valid for the duration of the call.
unsafe fn cfstring_to_rust(s: core_foundation_sys::string::CFStringRef) -> Option<String> {
    use core_foundation_sys::base::CFIndex;
    use core_foundation_sys::string::{CFStringGetCString, kCFStringEncodingUTF8};
    use std::ffi::CStr;

    if s.is_null() {
        return None;
    }
    let mut buf = [0i8; 1024];
    let ok = CFStringGetCString(s, buf.as_mut_ptr(), buf.len() as CFIndex, kCFStringEncodingUTF8);
    if ok as u8 != 0 {
        let cstr = CStr::from_ptr(buf.as_ptr());
        Some(cstr.to_string_lossy().into_owned())
    } else {
        None
    }
}

// Read an f64 from a CFNumberRef stored under `key` in a CFDictionaryRef.
unsafe fn dict_get_f64(
    dict: core_foundation_sys::dictionary::CFDictionaryRef,
    key: &str,
) -> Option<f64> {
    use core_foundation::base::TCFType;
    use core_foundation::string::CFString;
    use core_foundation_sys::dictionary::CFDictionaryGetValue;
    use core_foundation_sys::number::{kCFNumberFloat64Type, CFNumberGetValue, CFNumberRef};
    use std::os::raw::c_void;

    let key_cf = CFString::new(key);
    let val = CFDictionaryGetValue(dict, key_cf.as_concrete_TypeRef() as *const c_void);
    if val.is_null() {
        return None;
    }
    let mut out: f64 = 0.0;
    let ok = CFNumberGetValue(val as CFNumberRef, kCFNumberFloat64Type, &mut out as *mut f64 as *mut c_void);
    if ok as u8 != 0 { Some(out) } else { None }
}

// Read an i32 from a CFNumberRef stored under `key` in a CFDictionaryRef.
unsafe fn dict_get_i32(
    dict: core_foundation_sys::dictionary::CFDictionaryRef,
    key: &str,
) -> Option<i32> {
    use core_foundation::base::TCFType;
    use core_foundation::string::CFString;
    use core_foundation_sys::dictionary::CFDictionaryGetValue;
    use core_foundation_sys::number::{kCFNumberSInt32Type, CFNumberGetValue, CFNumberRef};
    use std::os::raw::c_void;

    let key_cf = CFString::new(key);
    let val = CFDictionaryGetValue(dict, key_cf.as_concrete_TypeRef() as *const c_void);
    if val.is_null() {
        return None;
    }
    let mut out: i32 = 0;
    let ok = CFNumberGetValue(val as CFNumberRef, kCFNumberSInt32Type, &mut out as *mut i32 as *mut c_void);
    if ok as u8 != 0 { Some(out) } else { None }
}

/// Returns all visible user application windows sorted front-to-back.
///
/// Requires Screen Recording permission on macOS 10.15+.  Without it the
/// call succeeds but window titles will be missing.
///
/// Filters out system UI at layer ≥ 20 (menu bar, Dock) and windows owned
/// by "Window Server" or "Notification Center".
pub fn list_visible_windows() -> Result<Vec<VisibleWindowInfo>, AutomationError> {
    use core_foundation::base::TCFType;
    use core_foundation::string::CFString;
    use core_foundation_sys::array::{CFArrayGetCount, CFArrayGetValueAtIndex};
    use core_foundation_sys::base::{CFIndex, CFRelease};
    use core_foundation_sys::dictionary::{CFDictionaryGetValue, CFDictionaryRef};
    use core_foundation_sys::string::CFStringRef;
    use std::os::raw::c_void;

    let option = CG_WINDOW_LIST_OPTION_ON_SCREEN_ONLY | CG_WINDOW_LIST_EXCLUDE_DESKTOP_ELEMENTS;
    let array_ref = unsafe { CGWindowListCopyWindowInfo(option, 0) };

    if array_ref.is_null() {
        return Err(AutomationError::PlatformError(
            "CGWindowListCopyWindowInfo returned null \
             — screen recording permission may be required"
                .to_string(),
        ));
    }

    let count = unsafe { CFArrayGetCount(array_ref) };
    let mut windows = Vec::with_capacity(count as usize);
    let mut first_user_window = true;

    for i in 0..count {
        unsafe {
            let item = CFArrayGetValueAtIndex(array_ref, i as CFIndex);
            if item.is_null() {
                continue;
            }
            let dict_ref = item as CFDictionaryRef;

            // Layer — skip system UI (≥20) and sub-desktop (<-1)
            let layer = match dict_get_i32(dict_ref, "kCGWindowLayer") {
                Some(l) => l,
                None => continue,
            };
            if !(-1..20).contains(&layer) {
                continue;
            }

            // App name — skip headless system processes
            let owner_key = CFString::new("kCGWindowOwnerName");
            let owner_ptr = CFDictionaryGetValue(
                dict_ref,
                owner_key.as_concrete_TypeRef() as *const c_void,
            );
            let app_name = match cfstring_to_rust(owner_ptr as CFStringRef) {
                Some(n) => n,
                None => continue,
            };
            if app_name == "Window Server" || app_name == "Notification Center" {
                continue;
            }

            // Window title (optional — absent when screen recording not granted)
            let name_key = CFString::new("kCGWindowName");
            let name_ptr = CFDictionaryGetValue(
                dict_ref,
                name_key.as_concrete_TypeRef() as *const c_void,
            );
            let window_title = cfstring_to_rust(name_ptr as CFStringRef);

            // Bounds — nested CFDictionary with keys "X", "Y", "Width", "Height"
            let bounds_key = CFString::new("kCGWindowBounds");
            let bounds_ptr = CFDictionaryGetValue(
                dict_ref,
                bounds_key.as_concrete_TypeRef() as *const c_void,
            );
            let (x, y, w, h) = if !bounds_ptr.is_null() {
                let bd = bounds_ptr as CFDictionaryRef;
                (
                    dict_get_f64(bd, "X").unwrap_or(0.0),
                    dict_get_f64(bd, "Y").unwrap_or(0.0),
                    dict_get_f64(bd, "Width").unwrap_or(0.0),
                    dict_get_f64(bd, "Height").unwrap_or(0.0),
                )
            } else {
                (0.0, 0.0, 0.0, 0.0)
            };

            // CGWindowListCopyWindowInfo returns front-to-back order; first layer-0
            // window is the frontmost user app window. Modals/alerts sit at layer 1+
            // so they won't receive this flag, but their position in the array still
            // conveys that they are in front of normal windows.
            let is_frontmost = layer == 0 && first_user_window;
            if is_frontmost {
                first_user_window = false;
            }

            windows.push(VisibleWindowInfo {
                app_name,
                window_title,
                position: (x, y),
                size: (w, h),
                is_frontmost,
                layer,
            });
        }
    }

    // CGWindowListCopyWindowInfo uses Create Rule — caller must release
    unsafe { CFRelease(array_ref as *const c_void) };

    Ok(windows)
}

/// Find the PID of the process owning the frontmost visible window at the given
/// screen coordinates. Uses `CGWindowListCopyWindowInfo` (front-to-back order).
///
/// Returns `None` if no user-space window covers the point, or if the call fails
/// (e.g., screen recording permission not granted).
pub(crate) fn get_pid_at_screen_point(x: f64, y: f64) -> Option<i32> {
    use core_foundation::base::TCFType;
    use core_foundation::string::CFString;
    use core_foundation_sys::array::{CFArrayGetCount, CFArrayGetValueAtIndex};
    use core_foundation_sys::base::{CFIndex, CFRelease};
    use core_foundation_sys::dictionary::{CFDictionaryGetValue, CFDictionaryRef};
    use std::os::raw::c_void;

    let option = CG_WINDOW_LIST_OPTION_ON_SCREEN_ONLY | CG_WINDOW_LIST_EXCLUDE_DESKTOP_ELEMENTS;
    let array_ref = unsafe { CGWindowListCopyWindowInfo(option, 0) };
    if array_ref.is_null() {
        return None;
    }

    let count = unsafe { CFArrayGetCount(array_ref) };
    let mut found_pid: Option<i32> = None;

    'search: for i in 0..count {
        unsafe {
            let item = CFArrayGetValueAtIndex(array_ref, i as CFIndex);
            if item.is_null() {
                continue;
            }
            let dict = item as CFDictionaryRef;

            // Skip system layers (menu bar, Dock, desktop)
            let layer = match dict_get_i32(dict, "kCGWindowLayer") {
                Some(l) => l,
                None => continue,
            };
            if !(-1..20).contains(&layer) {
                continue;
            }

            // Read window bounds
            let bounds_key = CFString::new("kCGWindowBounds");
            let bounds_ptr = CFDictionaryGetValue(
                dict,
                bounds_key.as_concrete_TypeRef() as *const c_void,
            );
            if bounds_ptr.is_null() {
                continue;
            }
            let bd = bounds_ptr as CFDictionaryRef;
            let wx = dict_get_f64(bd, "X").unwrap_or(0.0);
            let wy = dict_get_f64(bd, "Y").unwrap_or(0.0);
            let ww = dict_get_f64(bd, "Width").unwrap_or(0.0);
            let wh = dict_get_f64(bd, "Height").unwrap_or(0.0);

            if x >= wx && x < wx + ww && y >= wy && y < wy + wh {
                if let Some(pid) = dict_get_i32(dict, "kCGWindowOwnerPID") {
                    found_pid = Some(pid);
                    break 'search;
                }
            }
        }
    }

    unsafe { CFRelease(array_ref as *const c_void) };
    found_pid
}

// ── end visible window listing ──────────────────────────────────────────────

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
        let expected = (50.0_f64 * 50.0 + 50.0 * 50.0).sqrt();
        assert!((distance - expected).abs() < 0.001);
    }
}

//! # Global desktop points
//!
//! macOS lays every display out in one coordinate space measured in points:
//! top-left origin at the primary display (after the y flip), the unit
//! NSScreen frames, CSS pixels and `LogicalPosition` use. It is the only space
//! in which a 1x display beside a 2x Retina sits without gaps or overlaps.
//!
//! Tauri (tao 0.35 on macOS) reports "physical" numbers in three different
//! scales, and they only agree when every display has the same density:
//!
//! - a monitor's position and size: its points times ITS OWN scale factor;
//! - the cursor: its points times the PRIMARY display's scale factor;
//! - a window's outer position: its points times the scale factor of the
//!   display the window is on.
//!
//! Comparing them directly is what made the cursor-display follow and the
//! bar's click-through misfire on a second display of another density. The
//! helpers here undo exactly the factor tao applied, so every comparison is
//! made in points. The frontend's mirror is `src/lib/desktopPoints.ts`.

use tauri::{AppHandle, WebviewWindow};

/// A rectangle in global desktop points.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PointRect {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}

impl PointRect {
    /// Half-open: the left and top edges are inside, the right and bottom are not.
    pub fn contains(&self, p: (f64, f64)) -> bool {
        p.0 >= self.x && p.0 < self.x + self.width && p.1 >= self.y && p.1 < self.y + self.height
    }
}

fn factor(scale: f64) -> f64 {
    if scale > 0.0 {
        scale
    } else {
        1.0
    }
}

/// A monitor's frame in points, from tao's position and size (each already
/// multiplied by the monitor's own scale factor).
pub fn monitor_rect_points(position: (i32, i32), size: (u32, u32), scale: f64) -> PointRect {
    let s = factor(scale);
    PointRect {
        x: position.0 as f64 / s,
        y: position.1 as f64 / s,
        width: size.0 as f64 / s,
        height: size.1 as f64 / s,
    }
}

/// The cursor in points, from tao's cursor (multiplied by the primary display's factor).
pub fn cursor_to_points(cursor: (f64, f64), primary_scale: f64) -> (f64, f64) {
    let s = factor(primary_scale);
    (cursor.0 / s, cursor.1 / s)
}

/// A window's top-left in points, from tao's outer position (multiplied by
/// the factor of the display the window is on).
pub fn window_origin_to_points(position: (i32, i32), window_scale: f64) -> (f64, f64) {
    let s = factor(window_scale);
    (position.0 as f64 / s, position.1 as f64 / s)
}

/// Which display contains a point, if any.
pub fn monitor_index_at(rects: &[PointRect], p: (f64, f64)) -> Option<usize> {
    rects.iter().position(|r| r.contains(p))
}

/// Every display's frame in points, in Tauri's order.
pub fn monitors_in_points(window: &WebviewWindow) -> Vec<PointRect> {
    window
        .available_monitors()
        .map(|mons| {
            mons.iter()
                .map(|m| {
                    let p = m.position();
                    let s = m.size();
                    monitor_rect_points((p.x, p.y), (s.width, s.height), m.scale_factor())
                })
                .collect()
        })
        .unwrap_or_default()
}

/// The primary display's scale factor, which tao multiplies the cursor by.
pub fn primary_scale(app: &AppHandle) -> f64 {
    app.primary_monitor()
        .ok()
        .flatten()
        .map(|m| m.scale_factor())
        .unwrap_or(1.0)
}

/// The cursor in global points.
pub fn cursor_points(app: &AppHandle) -> Option<(f64, f64)> {
    let c = app.cursor_position().ok()?;
    Some(cursor_to_points((c.x, c.y), primary_scale(app)))
}

/// A window's top-left in global points.
pub fn window_origin_points(window: &WebviewWindow) -> Option<(f64, f64)> {
    let p = window.outer_position().ok()?;
    let s = window.scale_factor().ok()?;
    Some(window_origin_to_points((p.x, p.y), s))
}

#[cfg(test)]
mod tests {
    use super::*;

    // A 2x Retina laptop as primary (1512x982 points) with a 1x 1920x1080
    // external to its right, as tao reports them.
    fn laptop_and_external() -> Vec<PointRect> {
        vec![
            monitor_rect_points((0, 0), (3024, 1964), 2.0),
            monitor_rect_points((1512, 0), (1920, 1080), 1.0),
        ]
    }

    #[test]
    fn mixed_density_monitors_tile_in_points() {
        let r = laptop_and_external();
        assert_eq!(r[0].width, 1512.0);
        // The external starts exactly where the laptop ends: no overlap, no gap.
        assert_eq!(r[1].x, r[0].x + r[0].width);
    }

    #[test]
    fn a_cursor_on_the_external_is_found_on_the_external() {
        let r = laptop_and_external();
        // 2000 points across, which tao reports as 4000 (primary factor 2).
        let c = cursor_to_points((4000.0, 200.0), 2.0);
        assert_eq!(monitor_index_at(&r, c), Some(1));
        // A cursor on the laptop is not also claimed by the external.
        let c = cursor_to_points((2000.0, 200.0), 2.0);
        assert_eq!(monitor_index_at(&r, c), Some(0));
    }

    #[test]
    fn displays_left_of_and_above_the_primary_have_negative_origins() {
        let r = vec![
            monitor_rect_points((0, 0), (2880, 1800), 2.0),
            monitor_rect_points((-2560, -400), (2560, 1440), 1.0),
            monitor_rect_points((0, -2160), (3840, 2160), 1.0),
        ];
        assert_eq!(monitor_index_at(&r, (-10.0, 0.0)), Some(1));
        assert_eq!(monitor_index_at(&r, (100.0, -1.0)), Some(2));
        assert_eq!(monitor_index_at(&r, (100.0, 1.0)), Some(0));
    }

    #[test]
    fn a_window_origin_is_divided_by_its_own_display_factor() {
        // A window at (1600, 100) points on the 1x external reports (1600, 100).
        assert_eq!(window_origin_to_points((1600, 100), 1.0), (1600.0, 100.0));
        // The same point on a 2x display reports double.
        assert_eq!(window_origin_to_points((3200, 200), 2.0), (1600.0, 100.0));
    }

    #[test]
    fn a_zero_scale_is_treated_as_one() {
        assert_eq!(cursor_to_points((10.0, 20.0), 0.0), (10.0, 20.0));
    }
}

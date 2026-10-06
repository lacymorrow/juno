//! # Click-through for a steady bar window
//!
//! A bar look that uses the steady frame (`src/lib/steadyFrame.ts`) keeps one
//! window per dock well, sized for its largest state, and grows its visible
//! shape in CSS inside it. Most of that window is transparent, and a
//! transparent window still catches every click on it. This module is what
//! lets those clicks through.
//!
//! The page reports the rectangles it is drawing (`set_bar_hit_regions`), in
//! logical pixels relative to the window's top-left. A single poll task reads
//! the cursor, and whenever the cursor crosses into or out of those rectangles
//! it flips the window's `ignoresMouseEvents` and tells the page the mouse
//! entered or left. The native tracking area in `platform/macos.rs` reports
//! enter and leave for the whole window, which for a steady window is mostly
//! empty space, so while regions are set it stays quiet for the bar and this
//! module speaks for it instead.
//!
//! No regions means the look is not a steady one: the window takes every
//! mouse event, as it always did, and the tracking area owns hover again.
//!
//! The poll is the same shape as `cursor_follow`: Tauri's own cursor read, no
//! unsafe Cocoa, no accessibility permission. It idles on an atomic check
//! whenever no look has registered regions.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;
use std::time::Duration;

use serde::Deserialize;
use tauri::{AppHandle, Emitter, Manager};

use crate::constants;

/// One display frame. Hover and click-through follow the cursor this closely.
const POLL_INTERVAL_MS: u64 = 16;

/// A rectangle the page is drawing, in logical pixels from the window's top-left.
#[derive(Debug, Clone, Copy, PartialEq, Deserialize)]
pub struct HitRect {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}

/// The rectangles the steady look is drawing right now, or `None` when the
/// mounted look is not a steady one.
static REGIONS: Mutex<Option<Vec<HitRect>>> = Mutex::new(None);
/// Mirrors `REGIONS.is_some()` so the tracking area can ask without a lock.
static ACTIVE: AtomicBool = AtomicBool::new(false);
/// Guards against spawning more than one poll task.
static TASK_STARTED: AtomicBool = AtomicBool::new(false);

/// Whether a physical cursor position is over any of the regions of a window
/// whose top-left is `origin` (physical) at `scale` physical pixels per point.
///
/// Edges follow the usual half-open rule: the left and top edges are inside,
/// the right and bottom edges are not, so two touching rectangles never both
/// claim the pixel between them.
pub fn cursor_in_regions(
    cursor: (f64, f64),
    origin: (f64, f64),
    scale: f64,
    regions: &[HitRect],
) -> bool {
    let scale = if scale > 0.0 { scale } else { 1.0 };
    let lx = (cursor.0 - origin.0) / scale;
    let ly = (cursor.1 - origin.1) / scale;
    regions
        .iter()
        .any(|r| lx >= r.x && lx < r.x + r.width && ly >= r.y && ly < r.y + r.height)
}

/// What to tell the page when the cursor's inside-ness changes.
///
/// `Some(true)` is an enter and `Some(false)` a leave. The first observation
/// after regions are set announces an enter when the cursor is already over
/// the content (the page starts un-hovered) and says nothing otherwise.
pub fn hover_transition(prev: Option<bool>, now: bool) -> Option<bool> {
    match prev {
        None => now.then_some(true),
        Some(was) if was != now => Some(now),
        Some(_) => None,
    }
}

/// Whether the steady hit test speaks for this window's hover right now.
pub fn owns_hover(window_label: &str) -> bool {
    window_label == constants::ui::window_labels::FLOATING_BAR && ACTIVE.load(Ordering::Relaxed)
}

/// Record the regions the page is drawing. `None` turns click-through off.
pub fn set_regions(regions: Option<Vec<HitRect>>) {
    let active = regions.is_some();
    match REGIONS.lock() {
        Ok(mut guard) => *guard = regions,
        Err(poisoned) => *poisoned.into_inner() = regions,
    }
    ACTIVE.store(active, Ordering::Relaxed);
}

fn current_regions() -> Option<Vec<HitRect>> {
    match REGIONS.lock() {
        Ok(guard) => guard.clone(),
        Err(poisoned) => poisoned.into_inner().clone(),
    }
}

/// Start the single poll task. Safe to call more than once.
pub fn start(app: AppHandle) {
    if TASK_STARTED.swap(true, Ordering::SeqCst) {
        return;
    }
    tauri::async_runtime::spawn(async move {
        // What the cursor was last seen as, so only a change touches the window.
        let mut last_inside: Option<bool> = None;
        loop {
            tokio::time::sleep(Duration::from_millis(POLL_INTERVAL_MS)).await;

            let Some(regions) = current_regions() else {
                last_inside = None;
                continue;
            };
            let Some(window) = app.get_webview_window(constants::ui::window_labels::FLOATING_BAR)
            else {
                continue;
            };
            if !window.is_visible().unwrap_or(false) {
                continue;
            }
            let (Ok(cursor), Ok(origin), Ok(scale)) = (
                app.cursor_position(),
                window.outer_position(),
                window.scale_factor(),
            ) else {
                continue;
            };

            let inside = cursor_in_regions(
                (cursor.x, cursor.y),
                (origin.x as f64, origin.y as f64),
                scale,
                &regions,
            );
            if last_inside == Some(inside) {
                continue;
            }
            if let Err(e) = window.set_ignore_cursor_events(!inside) {
                log::warn!("[BarHitTest] could not set click-through: {e}");
                continue;
            }
            if let Some(entered) = hover_transition(last_inside, inside) {
                let event = if entered {
                    constants::events::system::MOUSE_ENTERED_WINDOW
                } else {
                    constants::events::system::MOUSE_LEFT_WINDOW
                };
                let _ = window.emit(event, ());
            }
            last_inside = Some(inside);
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    const PILL: HitRect = HitRect {
        x: 364.0,
        y: 0.0,
        width: 88.0,
        height: 76.0,
    };

    #[test]
    fn a_cursor_over_the_content_is_inside() {
        // Window at (1000, 40) on a 1x display; the pill footprint starts 364 in.
        assert!(cursor_in_regions(
            (1400.0, 60.0),
            (1000.0, 40.0),
            1.0,
            &[PILL]
        ));
    }

    #[test]
    fn a_cursor_over_the_transparent_rest_of_the_window_is_outside() {
        assert!(!cursor_in_regions(
            (1100.0, 60.0),
            (1000.0, 40.0),
            1.0,
            &[PILL]
        ));
        assert!(!cursor_in_regions(
            (1400.0, 200.0),
            (1000.0, 40.0),
            1.0,
            &[PILL]
        ));
    }

    #[test]
    fn regions_are_logical_and_the_cursor_physical() {
        // Retina: 2 physical pixels per point. The pill's left edge is 728
        // physical pixels in from the window's left.
        assert!(cursor_in_regions(
            (2000.0 + 728.0, 100.0),
            (2000.0, 80.0),
            2.0,
            &[PILL]
        ));
        assert!(!cursor_in_regions(
            (2000.0 + 726.0, 100.0),
            (2000.0, 80.0),
            2.0,
            &[PILL]
        ));
    }

    #[test]
    fn left_and_top_edges_are_inside_right_and_bottom_are_not() {
        let o = (0.0, 0.0);
        assert!(cursor_in_regions((364.0, 0.0), o, 1.0, &[PILL]));
        assert!(!cursor_in_regions((452.0, 10.0), o, 1.0, &[PILL]));
        assert!(!cursor_in_regions((400.0, 76.0), o, 1.0, &[PILL]));
    }

    #[test]
    fn works_on_a_display_left_of_or_above_the_main_one() {
        // Negative global coordinates are ordinary on a multi-display desk.
        let origin = (-1440.0, -900.0);
        assert!(cursor_in_regions(
            (-1440.0 + 400.0, -900.0 + 10.0),
            origin,
            1.0,
            &[PILL]
        ));
    }

    #[test]
    fn any_region_counts() {
        let pane = HitRect {
            x: 0.0,
            y: 100.0,
            width: 452.0,
            height: 360.0,
        };
        assert!(cursor_in_regions(
            (10.0, 200.0),
            (0.0, 0.0),
            1.0,
            &[PILL, pane]
        ));
        assert!(!cursor_in_regions(
            (10.0, 50.0),
            (0.0, 0.0),
            1.0,
            &[PILL, pane]
        ));
    }

    #[test]
    fn no_regions_means_nothing_is_inside() {
        assert!(!cursor_in_regions((1.0, 1.0), (0.0, 0.0), 1.0, &[]));
    }

    #[test]
    fn a_bad_scale_is_treated_as_one() {
        assert!(cursor_in_regions((400.0, 10.0), (0.0, 0.0), 0.0, &[PILL]));
    }

    #[test]
    fn hover_is_announced_only_on_a_change() {
        assert_eq!(hover_transition(None, true), Some(true));
        assert_eq!(hover_transition(None, false), None);
        assert_eq!(hover_transition(Some(false), true), Some(true));
        assert_eq!(hover_transition(Some(true), false), Some(false));
        assert_eq!(hover_transition(Some(true), true), None);
        assert_eq!(hover_transition(Some(false), false), None);
    }
}

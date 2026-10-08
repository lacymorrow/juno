//! # The bar drag, driven from Rust
//!
//! A steady bar look (`src/lib/steadyFrame.ts`) lives in one window far larger
//! than the shape inside it. Handing that window to the OS drag
//! (`startDragging` -> `performWindowDragWithEvent:`) has one fatal property:
//! during a user drag macOS keeps the window's top edge below the menu bar.
//! A pill docked low sits at the bottom of a 574 point tall window, so the
//! visible pill stopped dead about halfway up the screen and the top row of
//! wells could not be reached.
//!
//! So the bar drives the drag itself. Once the page decides a press has become
//! a drag it calls `bar_drag_follow` with where the press landed inside the
//! window, and a task here moves the window under the cursor every 8 ms until
//! the left button comes up. A programmatic move of a borderless window is not
//! constrained to stay below the menu bar (AppKit only constrains titled
//! windows), so the shape follows the cursor anywhere on any display.
//!
//! Every tick runs on the main thread and works entirely in Cocoa's own global
//! points (`NSEvent.mouseLocation`, `setFrameTopLeftPoint:`), so there is no
//! scale factor anywhere to get wrong on a second display.
//!
//! Calling `bar_drag_follow` again during a drag only changes the grab offset:
//! the page does that once, when it re-lays the drawing out for the drag.
//! `bar_drag_stop` ends it from the page's side; release ends it from ours and
//! emits `bar-drag-ended` so the page settles even if its own mouseup was lost.

use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Mutex;

use tauri::AppHandle;

/// One display frame at 120 Hz; the window never trails the cursor visibly.
#[cfg(target_os = "macos")]
const TICK_MS: u64 = 8;

/// A drag is being driven right now.
static DRAGGING: AtomicBool = AtomicBool::new(false);
/// Bumped by every start and stop, so a loop from an older drag exits.
static GENERATION: AtomicU64 = AtomicU64::new(0);
/// Where the press sits inside the window, in points from its top-left.
static GRAB: Mutex<(f64, f64)> = Mutex::new((0.0, 0.0));

/// Whether a bar drag is being driven. The hit test keeps the window
/// interactive for its length.
pub fn is_dragging() -> bool {
    DRAGGING.load(Ordering::Relaxed)
}

fn set_grab(grab: (f64, f64)) {
    match GRAB.lock() {
        Ok(mut g) => *g = grab,
        Err(poisoned) => *poisoned.into_inner() = grab,
    }
}

fn grab() -> (f64, f64) {
    match GRAB.lock() {
        Ok(g) => *g,
        Err(poisoned) => *poisoned.into_inner(),
    }
}

/// The window's top-left in Cocoa coordinates (y up) that keeps the grab
/// point under a cursor at `mouse` (also Cocoa, y up). The grab offset is
/// measured down from the window's top, so the top sits above the cursor.
pub fn cocoa_top_left_for(mouse: (f64, f64), grab: (f64, f64)) -> (f64, f64) {
    (mouse.0 - grab.0, mouse.1 + grab.1)
}

/// Whether the left mouse button is down, from `NSEvent.pressedMouseButtons`.
pub fn left_button_down(pressed_buttons: usize) -> bool {
    pressed_buttons & 1 == 1
}

/// Start following the cursor, or re-grab an in-flight drag.
pub fn follow(app: AppHandle, grab_offset: (f64, f64)) -> Result<(), String> {
    set_grab(grab_offset);
    if DRAGGING.swap(true, Ordering::SeqCst) {
        return Ok(());
    }
    let generation = GENERATION.fetch_add(1, Ordering::SeqCst) + 1;
    run_loop(app, generation)
}

/// End the drag from the page's side. Waits for the main thread so no tick
/// queued before the stop can move the window after it.
pub async fn stop(app: AppHandle) -> Result<(), String> {
    GENERATION.fetch_add(1, Ordering::SeqCst);
    DRAGGING.store(false, Ordering::SeqCst);
    let (tx, rx) = tokio::sync::oneshot::channel();
    app.run_on_main_thread(move || {
        let _ = tx.send(());
    })
    .map_err(|e| format!("Could not stop the bar drag: {e}"))?;
    let _ = rx.await;
    Ok(())
}

#[cfg(not(target_os = "macos"))]
fn run_loop(_app: AppHandle, _generation: u64) -> Result<(), String> {
    // The page falls back to the OS window drag.
    DRAGGING.store(false, Ordering::SeqCst);
    Err("The driven bar drag is macOS only".to_string())
}

#[cfg(target_os = "macos")]
fn run_loop(app: AppHandle, generation: u64) -> Result<(), String> {
    use std::time::Duration;
    use tauri::{Emitter, Manager};

    use crate::constants;

    let window = app
        .get_webview_window(constants::ui::window_labels::FLOATING_BAR)
        .ok_or_else(|| {
            DRAGGING.store(false, Ordering::SeqCst);
            "floating-bar window not found".to_string()
        })?;
    // The cursor is over the shape it pressed, so the window must take the
    // mouse for the whole drag whatever the hit test last decided.
    let _ = window.set_ignore_cursor_events(false);
    let ns_window = match window.ns_window() {
        Ok(ptr) => ptr as usize,
        Err(e) => {
            DRAGGING.store(false, Ordering::SeqCst);
            return Err(format!("No native window for the bar drag: {e}"));
        }
    };

    tauri::async_runtime::spawn(async move {
        loop {
            tokio::time::sleep(Duration::from_millis(TICK_MS)).await;
            if GENERATION.load(Ordering::SeqCst) != generation {
                return;
            }
            let (tx, rx) = tokio::sync::oneshot::channel();
            let scheduled = app.run_on_main_thread(move || {
                // Re-checked on the main thread: a stop that landed while this
                // tick was queued must win.
                if GENERATION.load(Ordering::SeqCst) != generation {
                    let _ = tx.send(None);
                    return;
                }
                let _ = tx.send(Some(tick(ns_window)));
            });
            if scheduled.is_err() {
                break;
            }
            match rx.await {
                Ok(Some(true)) => continue,
                Ok(None) => return,
                // Released, or the main thread dropped the tick.
                Ok(Some(false)) | Err(_) => break,
            }
        }
        // Only the drag this loop belongs to may be ended by it.
        if GENERATION
            .compare_exchange(
                generation,
                generation + 1,
                Ordering::SeqCst,
                Ordering::SeqCst,
            )
            .is_ok()
        {
            DRAGGING.store(false, Ordering::SeqCst);
            if let Err(e) = app.emit_to(
                constants::ui::window_labels::FLOATING_BAR,
                constants::events::bar::DRAG_ENDED,
                (),
            ) {
                log::warn!("[BarDrag] could not report the end of a drag: {e}");
            }
        }
    });
    Ok(())
}

/// One frame of the drag, on the main thread. Returns whether the button is
/// still down (and the window was moved).
#[cfg(target_os = "macos")]
fn tick(ns_window_addr: usize) -> bool {
    use cocoa::base::id as cocoa_id;
    use cocoa::foundation::NSPoint;
    use objc::{class, msg_send, sel, sel_impl};

    // SAFETY: class methods of NSEvent and a live NSWindow pointer handed out
    // by Tauri for the bar window, which lives for the app's lifetime. Main
    // thread only, which the caller guarantees.
    unsafe {
        let pressed: usize = msg_send![class!(NSEvent), pressedMouseButtons];
        if !left_button_down(pressed) {
            return false;
        }
        let mouse: NSPoint = msg_send![class!(NSEvent), mouseLocation];
        let (x, y) = cocoa_top_left_for((mouse.x, mouse.y), grab());
        let ns_window = ns_window_addr as cocoa_id;
        let _: () = msg_send![ns_window, setFrameTopLeftPoint: NSPoint::new(x, y)];
    }
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_grab_point_stays_under_the_cursor() {
        // Cursor at (500, 300) in Cocoa (y up), pressed 40 in and 520 down.
        let (x, y) = cocoa_top_left_for((500.0, 300.0), (40.0, 520.0));
        assert_eq!(x, 460.0);
        // The window's top is 520 points above the cursor.
        assert_eq!(y, 820.0);
    }

    #[test]
    fn a_window_may_be_carried_above_the_top_of_the_screen() {
        // A tall window grabbed near its bottom, with the cursor at the very
        // top of a 982 point screen: the top-left lands above the screen and
        // nothing here clamps it, which is what lets the top wells be reached.
        let (_, y) = cocoa_top_left_for((700.0, 975.0), (226.0, 520.0));
        assert!(y > 982.0);
    }

    #[test]
    fn only_the_left_button_counts() {
        assert!(left_button_down(1));
        assert!(left_button_down(0b11));
        assert!(!left_button_down(0b10));
        assert!(!left_button_down(0));
    }

    #[test]
    fn regrabbing_changes_only_the_offset() {
        set_grab((1.0, 2.0));
        assert_eq!(grab(), (1.0, 2.0));
        set_grab((3.0, 4.0));
        assert_eq!(grab(), (3.0, 4.0));
    }
}

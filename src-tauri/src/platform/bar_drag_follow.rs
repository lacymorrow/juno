//! # The bar's drag: Juno moves the window, on every mouse event
//!
//! Once a press on the bar becomes a drag, the bar window is moved by Juno
//! itself, on the main thread, from `NSEvent` monitors: every
//! `leftMouseDragged` puts the window's top-left at the cursor minus the spot
//! pressed (`setFrameTopLeftPoint:`), and `leftMouseUp` ends it. The window is
//! the pill's own footprint by then (`dragLayout` in `src/lib/steadyFrame.ts`),
//! so the menu bar constraint every `setFrame` goes through holds the pill's
//! top just under the menu bar, which is where the top wells are.
//!
//! ## Why not the OS drag
//!
//! #766 handed the window to `performWindowDragWithEvent:` with a mouse-down
//! made at the cursor. That drag belongs to the WindowServer: it runs on its
//! own, Juno cannot see it, and AppKit only learns the window's frame after
//! the fact. The v0.8.143 `[Drag]` log showed placement exact at the start and
//! then the spot pressed 4 to 64 points off the cursor at release, with the
//! sign varying, and once the window left behind at (1301, 1020), below the
//! bottom of a 900 point display, while the cursor was at (1072, 571). The
//! synthesized mouse-down is not the press the WindowServer saw (other event
//! number, timestamp and location, and in a different window frame), so how
//! it anchors and how long it tracks is up to it. Nothing Juno can pass in
//! that call fixes that.
//!
//! ## Why this is not #727 again
//!
//! #727 followed the cursor with an 8 ms timer on a tokio worker, four
//! main-thread round trips per tick, in a 574 point tall window that the menu
//! bar constraint stopped halfway up the screen. Here there is no timer and no
//! round trip: the move happens inside the event's own dispatch, on the main
//! thread, from `NSEvent.mouseLocation` as it is right then, and the position
//! is absolute (cursor minus grab), so an event that is late or missed cannot
//! leave an error behind for the next one. The window is the footprint, so
//! the constraint is the right one. No class or window level is touched.
//!
//! ## Ending
//!
//! A `leftMouseUp` ends it, and so does any `mouseMoved` (the button is up and
//! we missed the release). The page's settle also stops it
//! (`bar_drag_released`), which covers a release no monitor saw: the release
//! watch (`bar_pointer_held`) notices the button is up and settles. The window
//! is then always glided into a well by the existing settle.

/// `NSEventType` values the follow reacts to.
pub const NS_LEFT_MOUSE_UP: usize = 2;
pub const NS_MOUSE_MOVED: usize = 5;
pub const NS_LEFT_MOUSE_DRAGGED: usize = 6;

/// `NSEventMask` for the three: `1 << type`.
pub const FOLLOW_EVENT_MASK: usize =
    (1 << NS_LEFT_MOUSE_UP) | (1 << NS_MOUSE_MOVED) | (1 << NS_LEFT_MOUSE_DRAGGED);

/// Why the follow stopped.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EndReason {
    /// The left button came up.
    Release,
    /// A move with no button held: the release happened and was not seen.
    ButtonUp,
    /// The page settled the drag (its mouseup, or the release watch).
    Page,
    /// A new drag started while this one was still on.
    Replaced,
}

impl EndReason {
    pub fn describe(self) -> &'static str {
        match self {
            EndReason::Release => "the button came up",
            EndReason::ButtonUp => "a move with the button up (release not seen)",
            EndReason::Page => "the page settled it",
            EndReason::Replaced => "a new drag started",
        }
    }
}

/// What one mouse event means to a drag in progress.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FollowStep {
    /// Put the window under the cursor.
    Move,
    /// Stop following.
    End(EndReason),
    /// Not ours.
    Ignore,
}

/// Classify an `NSEvent` type. Pure.
pub fn classify(event_type: usize) -> FollowStep {
    match event_type {
        NS_LEFT_MOUSE_DRAGGED => FollowStep::Move,
        NS_LEFT_MOUSE_UP => FollowStep::End(EndReason::Release),
        NS_MOUSE_MOVED => FollowStep::End(EndReason::ButtonUp),
        _ => FollowStep::Ignore,
    }
}

/// The Cocoa top-left (`setFrameTopLeftPoint:`: points, y up from the primary
/// display's bottom edge) that puts `grab` (from the window's top-left, y
/// down) under the cursor at `mouse` (`NSEvent.mouseLocation`, Cocoa). One
/// subtraction and one addition: no display, scale or height enters, so it is
/// the same on every display of a mixed-density desk. Pure.
pub fn cocoa_top_left_under_cursor(mouse: (f64, f64), grab: (f64, f64)) -> (f64, f64) {
    (mouse.0 - grab.0, mouse.1 + grab.1)
}

/// How far the window's top-left ended up from where it was asked to go.
/// Zero except where AppKit held it (the menu bar). Pure.
pub fn gap(asked: (f64, f64), got: (f64, f64)) -> (f64, f64) {
    (got.0 - asked.0, got.1 - asked.1)
}

/// What one drag's follow did, for the `[Drag]` log.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct FollowStats {
    /// Window moves made.
    pub moves: u32,
    /// Moves AppKit did not apply exactly (held below the menu bar).
    pub held: u32,
    /// The largest gap seen, by length.
    pub max_gap: (f64, f64),
    /// The gap after the last move: what was left when the drag ended.
    pub last_gap: (f64, f64),
}

impl FollowStats {
    /// Record one move. Half a point is rounding, not a hold.
    pub fn record(&mut self, asked: (f64, f64), got: (f64, f64)) {
        let g = gap(asked, got);
        self.moves += 1;
        self.last_gap = g;
        if g.0.abs() > 0.5 || g.1.abs() > 0.5 {
            self.held += 1;
        }
        if g.0.hypot(g.1) > self.max_gap.0.hypot(self.max_gap.1) {
            self.max_gap = g;
        }
    }
}

#[cfg(target_os = "macos")]
mod imp {
    use super::FOLLOW_EVENT_MASK;
    use super::{classify, cocoa_top_left_under_cursor, EndReason, FollowStats, FollowStep};
    use block::ConcreteBlock;
    use cocoa::base::{id, nil};
    use cocoa::foundation::{NSPoint, NSRect};
    use objc::{class, msg_send, sel, sel_impl};
    use std::ffi::c_void;
    use std::sync::Mutex;
    use std::time::Instant;
    use tauri::AppHandle;
    use tracing::{debug, info, warn};

    /// The drag in progress. Only touched on the main thread; `Send` so it
    /// can live in a static.
    struct Follow {
        local: id,
        global: id,
        window: id,
        grab: (f64, f64),
        started: Instant,
        stats: FollowStats,
    }
    unsafe impl Send for Follow {}

    static FOLLOW: Mutex<Option<Follow>> = Mutex::new(None);

    fn lock() -> std::sync::MutexGuard<'static, Option<Follow>> {
        match FOLLOW.lock() {
            Ok(g) => g,
            Err(poisoned) => poisoned.into_inner(),
        }
    }

    /// Start following with `grab` (points from the window's top-left) under
    /// the cursor. Main thread only: called from `set_bar_frame`'s main-thread
    /// closure right after the window was placed, so no event can slip in
    /// between the placement and the first move.
    pub fn start(app: &AppHandle, window: id, grab: (f64, f64)) -> Result<(), String> {
        if lock().is_some() {
            end(app, EndReason::Replaced);
        }
        let app_local = app.clone();
        let local_block = ConcreteBlock::new(move |event: id| -> id {
            on_event(&app_local, event);
            // Handed back untouched: the page still sees the drag and the
            // release (its window mouseup settles the drag).
            event
        })
        .copy();
        let app_global = app.clone();
        let global_block = ConcreteBlock::new(move |event: id| {
            on_event(&app_global, event);
        })
        .copy();
        // SAFETY: main thread; the blocks are heap copies AppKit retains for
        // the lifetime of each monitor. Mouse monitors need no permission.
        let (local, global): (id, id) = unsafe {
            let local: id = msg_send![
                class!(NSEvent),
                addLocalMonitorForEventsMatchingMask: FOLLOW_EVENT_MASK
                handler: &*local_block as *const _ as *const c_void
            ];
            let global: id = msg_send![
                class!(NSEvent),
                addGlobalMonitorForEventsMatchingMask: FOLLOW_EVENT_MASK
                handler: &*global_block as *const _ as *const c_void
            ];
            (local, global)
        };
        if local == nil && global == nil {
            return Err("AppKit refused both mouse monitors".to_string());
        }
        *lock() = Some(Follow {
            local,
            global,
            window,
            grab,
            started: Instant::now(),
            stats: FollowStats::default(),
        });
        Ok(())
    }

    fn on_event(app: &AppHandle, event: id) {
        if event == nil {
            return;
        }
        // SAFETY: a live NSEvent for the duration of the handler.
        let event_type: usize = unsafe { msg_send![event, type] };
        match classify(event_type) {
            FollowStep::Move => follow(),
            FollowStep::End(reason) => {
                let _ = end(app, reason);
            }
            FollowStep::Ignore => {}
        }
    }

    /// Put the window's top-left at the cursor minus the grab. Main thread.
    fn follow() {
        let (window, grab) = match lock().as_ref() {
            Some(f) => (f.window, f.grab),
            None => return,
        };
        // SAFETY: main thread (monitor handlers run there); `window` is the
        // bar's NSWindow, which lives as long as the app.
        let (asked, got, mouse) = unsafe {
            let mouse: NSPoint = msg_send![class!(NSEvent), mouseLocation];
            let asked = cocoa_top_left_under_cursor((mouse.x, mouse.y), grab);
            let _: () = msg_send![window, setFrameTopLeftPoint: NSPoint::new(asked.0, asked.1)];
            let frame: NSRect = msg_send![window, frame];
            let got = (frame.origin.x, frame.origin.y + frame.size.height);
            (asked, got, (mouse.x, mouse.y))
        };
        // Cocoa is y up. Flipped, so the gap reads y down like every other
        // `[Drag]` line (positive: the window was held lower than asked).
        let (asked, got) = ((asked.0, -asked.1), (got.0, -got.1));
        let (gx, gy) = super::gap(asked, got);
        debug!(
            "[Drag] follow: cursor ({:.1}, {:.1}) cocoa, window held ({:.1}, {:.1}) from the cursor minus the grab",
            mouse.0, mouse.1, gx, gy
        );
        if let Some(f) = lock().as_mut() {
            f.stats.record(asked, got);
        }
    }

    /// Stop following and log what the drag did. Main thread. Idempotent.
    pub fn end(app: &AppHandle, reason: EndReason) -> Option<FollowStats> {
        let f = lock().take()?;
        let (local, global) = (f.local as usize, f.global as usize);
        // Removed on the next turn of the run loop, not from inside one of
        // their own handlers. Nothing reaches them before that: the state is
        // already gone, so a late event does nothing.
        let removed = app.run_on_main_thread(move || {
            // SAFETY: main thread; tokens came from the add calls in `start`.
            unsafe {
                for token in [local, global] {
                    if token != 0 {
                        let _: () = msg_send![class!(NSEvent), removeMonitor: token as id];
                    }
                }
            }
        });
        if let Err(e) = removed {
            warn!("[Drag] could not remove the drag monitors: {}", e);
        }
        let s = f.stats;
        info!(
            "[Drag] follow ended ({}): {} moves over {}ms, largest gap ({:.1}, {:.1}), last ({:.1}, {:.1}), held by the menu bar {} times",
            reason.describe(),
            s.moves,
            f.started.elapsed().as_millis(),
            s.max_gap.0,
            s.max_gap.1,
            s.last_gap.0,
            s.last_gap.1,
            s.held,
        );
        Some(s)
    }
}

#[cfg(target_os = "macos")]
pub use imp::{end, start};

#[cfg(test)]
mod tests {
    use super::*;
    use crate::platform::desktop_points::{cocoa_point_to_points, origin_under_cursor};

    /// The window's top-left in global points (y down) from a Cocoa top-left.
    fn to_points(cocoa_top_left: (f64, f64), primary_h: f64) -> (f64, f64) {
        cocoa_point_to_points(cocoa_top_left, primary_h)
    }

    #[test]
    fn a_drag_event_moves_and_a_release_ends() {
        assert_eq!(classify(NS_LEFT_MOUSE_DRAGGED), FollowStep::Move);
        assert_eq!(
            classify(NS_LEFT_MOUSE_UP),
            FollowStep::End(EndReason::Release)
        );
        // A plain move means the button is up: the release was missed.
        assert_eq!(
            classify(NS_MOUSE_MOVED),
            FollowStep::End(EndReason::ButtonUp)
        );
        // A right click, a scroll, a key: not the drag's business.
        for other in [1usize, 3, 4, 10, 22] {
            assert_eq!(classify(other), FollowStep::Ignore);
        }
    }

    #[test]
    fn the_mask_covers_exactly_the_three_events() {
        assert_eq!(FOLLOW_EVENT_MASK, (1 << 2) | (1 << 5) | (1 << 6));
    }

    #[test]
    fn every_move_puts_the_spot_pressed_under_the_cursor_on_every_display() {
        // A 982 pt primary; cursors on it, on an external to its right, and on
        // one above-left of it (negative x, Cocoa y above the primary's top).
        let primary_h = 982.0;
        let grab = (44.0, 30.0);
        for mouse in [
            (700.0, 500.0),
            (2000.0, 900.0),
            (-1200.0, 1300.0),
            (5.0, -400.0),
        ] {
            let top_left = to_points(cocoa_top_left_under_cursor(mouse, grab), primary_h);
            let cursor = cocoa_point_to_points(mouse, primary_h);
            assert_eq!(top_left, origin_under_cursor(cursor, grab));
            assert_eq!((top_left.0 + grab.0, top_left.1 + grab.1), cursor);
        }
    }

    #[test]
    fn a_fast_flick_over_many_events_leaves_no_drift() {
        // 2000 events, each up to ~400 pt from the last, sweeping across a
        // laptop and two externals and back. Each move is absolute, so the
        // error after the last one is the error after the first: zero.
        let primary_h = 982.0;
        let grab = (37.0, 21.0);
        let mut stats = FollowStats::default();
        for i in 0..2000u32 {
            let t = f64::from(i);
            let mouse = (
                700.0 + 2400.0 * (t * 0.37).sin(),
                500.0 + 900.0 * (t * 0.53).cos(),
            );
            let asked = cocoa_top_left_under_cursor(mouse, grab);
            // AppKit applied it as asked (nowhere near a menu bar here).
            stats.record(asked, asked);
            let top_left = to_points(asked, primary_h);
            let cursor = cocoa_point_to_points(mouse, primary_h);
            assert_eq!(
                (
                    cursor.0 - top_left.0 - grab.0,
                    cursor.1 - top_left.1 - grab.1
                ),
                (0.0, 0.0),
                "drift at event {i}"
            );
        }
        assert_eq!(stats.moves, 2000);
        assert_eq!(stats.held, 0);
        assert_eq!(stats.max_gap, (0.0, 0.0));
    }

    #[test]
    fn a_missed_event_cannot_leave_an_error_behind() {
        // Skip every other event: the window jumps further, but each landing
        // is still exactly under the cursor.
        let grab = (10.0, 10.0);
        for step in (0..200).step_by(2) {
            let mouse = (f64::from(step) * 13.0, 400.0 - f64::from(step) * 7.0);
            let asked = cocoa_top_left_under_cursor(mouse, grab);
            assert_eq!((asked.0 + grab.0, asked.1 - grab.1), mouse);
        }
    }

    #[test]
    fn the_menu_bar_hold_is_counted_and_not_carried() {
        let mut stats = FollowStats::default();
        // Held 24 pt lower by the menu bar (y down), then free again.
        stats.record((100.0, 0.0), (100.0, 24.0));
        stats.record((200.0, 300.0), (200.0, 300.0));
        assert_eq!(stats.moves, 2);
        assert_eq!(stats.held, 1);
        assert_eq!(stats.max_gap, (0.0, 24.0));
        // Once the cursor leaves the menu bar the window is exact again.
        assert_eq!(stats.last_gap, (0.0, 0.0));
    }

    #[test]
    fn rounding_is_not_a_hold() {
        let mut stats = FollowStats::default();
        stats.record((100.4, 50.0), (100.0, 50.0));
        assert_eq!(stats.held, 0);
    }

    #[test]
    fn every_end_reason_reads_as_words() {
        for r in [
            EndReason::Release,
            EndReason::ButtonUp,
            EndReason::Page,
            EndReason::Replaced,
        ] {
            assert!(!r.describe().is_empty());
        }
    }
}

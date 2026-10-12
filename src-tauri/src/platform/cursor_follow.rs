//! # Cursor-display follow
//!
//! Keeps the floating bar on whichever display the cursor is on, so it is always
//! where the user is looking instead of stranded on another monitor (the
//! behavior Wispr Flow has). Spaces on one display are already covered by the
//! bar's `CanJoinAllSpaces` collection behavior; this handles physical displays.
//!
//! A single background task polls Tauri's own cursor position (no unsafe Cocoa,
//! no accessibility permission) and, only when the containing display changes,
//! emits `cursor-display-changed` with the cursor's position in global points
//! (see `platform::desktop_points`; tao's raw cursor and monitor numbers are in
//! different scales on a mixed-density desk, which made this miss the second
//! display or pick the wrong one). The
//! frontend owns the well math, so it re-homes the bar to the same drag-well
//! slot on the new display. The task runs for the app's life and is gated by an
//! atomic flag mirroring the user's setting, so toggling never spawns a second
//! task.

use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use tauri::{AppHandle, Emitter, Manager};

use crate::constants;
use crate::platform::desktop_points;
use crate::state::AppState;

/// Poll cadence. Fast enough that the bar is already there when the user looks,
/// cheap enough to ignore (a cursor read plus a monitor-bounds check).
const POLL_INTERVAL_MS: u64 = 120;

/// Mirrors the user's "follow cursor display" setting.
static FOLLOW_ENABLED: AtomicBool = AtomicBool::new(false);
/// Guards against spawning more than one poll task.
static TASK_STARTED: AtomicBool = AtomicBool::new(false);

/// Turn cursor-follow on or off at runtime (from the setting).
pub fn set_enabled(enabled: bool) {
    FOLLOW_ENABLED.store(enabled, Ordering::Relaxed);
}

fn is_enabled() -> bool {
    FOLLOW_ENABLED.load(Ordering::Relaxed)
}

/// A monitor identity stable enough to detect a change: its top-left in
/// points. Distinct displays never share an origin in the global layout.
type MonitorKey = (i32, i32);

/// Start the single poll task. Safe to call once at setup; later calls no-op.
pub fn start(app: AppHandle) {
    if TASK_STARTED.swap(true, Ordering::SeqCst) {
        return;
    }
    tauri::async_runtime::spawn(async move {
        let mut last_key: Option<MonitorKey> = None;
        loop {
            tokio::time::sleep(Duration::from_millis(POLL_INTERVAL_MS)).await;

            if !is_enabled() {
                // Forget the last display so re-enabling doesn't fire a stale jump.
                last_key = None;
                continue;
            }

            // Never yank the bar around during onboarding.
            if let Some(state) = app.try_state::<AppState>() {
                if state.is_onboarding_active() {
                    continue;
                }
            }

            let Some(pos) = desktop_points::cursor_points(&app) else {
                continue;
            };
            let Some(window) = app.get_webview_window(constants::ui::window_labels::FLOATING_BAR)
            else {
                continue;
            };
            let monitors = desktop_points::monitors_in_points(&window);

            // The display whose bounds contain the cursor.
            let Some(monitor) =
                desktop_points::monitor_index_at(&monitors, pos).and_then(|i| monitors.get(i))
            else {
                continue;
            };

            let key = (monitor.x.round() as i32, monitor.y.round() as i32);
            if last_key == Some(key) {
                continue;
            }
            // The first observation seeds the baseline without moving the bar, so
            // enabling the feature (or launch) never triggers a spurious jump.
            let seeding = last_key.is_none();
            last_key = Some(key);
            if seeding {
                continue;
            }

            if let Err(e) = app.emit(
                constants::events::bar::CURSOR_DISPLAY_CHANGED,
                serde_json::json!({ "x": pos.0, "y": pos.1 }),
            ) {
                log::warn!("[CursorFollow] failed to emit display change: {e}");
            }
        }
    });
}

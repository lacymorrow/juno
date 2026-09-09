//! # Cursor-display follow
//!
//! Keeps the floating bar on whichever display the cursor is on, so it is always
//! where the user is looking instead of stranded on another monitor (the
//! behavior Wispr Flow has). Spaces on one display are already covered by the
//! bar's `CanJoinAllSpaces` collection behavior; this handles physical displays.
//!
//! A single background task polls Tauri's own cursor position (no unsafe Cocoa,
//! no accessibility permission) and, only when the containing display changes,
//! emits `cursor-display-changed` with the cursor's physical position. The
//! frontend owns the well math, so it re-homes the bar to the same drag-well
//! slot on the new display. The task runs for the app's life and is gated by an
//! atomic flag mirroring the user's setting, so toggling never spawns a second
//! task.

use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use tauri::{AppHandle, Emitter, Manager};

use crate::constants;
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

/// A monitor identity stable enough to detect a change: its top-left in physical
/// pixels. Distinct displays never share an origin in the global layout.
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

            let Ok(pos) = app.cursor_position() else {
                continue;
            };
            let Some(window) = app.get_webview_window(constants::ui::window_labels::FLOATING_BAR)
            else {
                continue;
            };
            let Ok(monitors) = window.available_monitors() else {
                continue;
            };

            // The display whose bounds contain the cursor.
            let containing = monitors.iter().find(|m| {
                let mp = m.position();
                let ms = m.size();
                let x = pos.x as i32;
                let y = pos.y as i32;
                x >= mp.x && x < mp.x + ms.width as i32 && y >= mp.y && y < mp.y + ms.height as i32
            });
            let Some(monitor) = containing else {
                continue;
            };

            let key = (monitor.position().x, monitor.position().y);
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
                serde_json::json!({ "x": pos.x, "y": pos.y }),
            ) {
                log::warn!("[CursorFollow] failed to emit display change: {e}");
            }
        }
    });
}

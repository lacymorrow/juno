//! # Floating-bar position persistence
//!
//! A tiny pair of commands that remember where the floating bar last settled
//! (the snapped well), so it reopens there on the next launch instead of the
//! default spot. Physical pixels, stored in a small dedicated store file so it
//! never interferes with the centralized settings serialization.

use log::warn;
use serde::{Deserialize, Serialize};
use tauri::{command, AppHandle, Manager, PhysicalPosition};
use tauri_plugin_store::StoreExt;

const BAR_POSITION_STORE_FILE: &str = "bar_position.json";
const BAR_POSITION_KEY: &str = "last_well";

/// The bar's last settled top-left, in physical pixels.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BarPosition {
    pub x: i32,
    pub y: i32,
}

/// The last well the bar snapped into, or `None` if nothing is stored yet.
#[command]
pub async fn get_bar_position(app_handle: AppHandle) -> Result<Option<BarPosition>, String> {
    let store = app_handle
        .store(BAR_POSITION_STORE_FILE)
        .map_err(|e| format!("Failed to open bar-position store: {}", e))?;

    match store.get(BAR_POSITION_KEY) {
        Some(value) => serde_json::from_value(value)
            .map(Some)
            .map_err(|e| format!("Failed to parse bar position: {}", e)),
        None => Ok(None),
    }
}

/// The stored position, read without going through a command.
///
/// `get_bar_position` is the webview's way of asking the same question, and it
/// cannot be the only way: startup has to know where the bar belongs before any
/// webview exists to ask on its behalf. A store that is missing, empty or
/// unreadable means the same thing here as it does there, which is that this
/// install has never put the bar anywhere.
pub fn stored_bar_position(app_handle: &AppHandle) -> Option<BarPosition> {
    let store = app_handle.store(BAR_POSITION_STORE_FILE).ok()?;
    let value = store.get(BAR_POSITION_KEY)?;
    serde_json::from_value(value).ok()
}

/// Put the bar back in the well it was left in, before anything can see it.
///
/// The window is created at whatever spot macOS cascades a new window to, which
/// is roughly the middle of the screen, and the real spot is a well the webview
/// works out once it has loaded. On a healthy release build the webview gets
/// there first and nobody sees the placeholder. On a debug build it does not:
/// the startup fallback puts the bar up after a couple of seconds, at the
/// cascade spot, and the bar then visibly jumps to its well when the webview
/// finally reports in. That jump is what people saw on launch.
///
/// Rust already knows the answer, because the bar persists its well on every
/// settle. Applying it during setup means the bar is in the right place before
/// the first show, whichever path shows it, and the webview's own placement
/// lands on the same coordinates rather than moving anything.
///
/// Only the position is restored, never a size: the placeholder frame in
/// `tauri.conf.json` is already the idle bar's size, and the size the bar wants
/// depends on state only the webview has.
///
/// Nothing to restore, or a position that is no longer on any display (a
/// monitor unplugged since the last run), leaves the window where it is. The
/// webview re-snaps a remembered position to the nearest current well anyway,
/// so a bar off the edge of a vanished display fixes itself; putting it
/// somewhere invented here would only be a second place for it to jump from.
pub fn restore_bar_position(app_handle: &AppHandle) {
    let Some(saved) = stored_bar_position(app_handle) else {
        return;
    };
    let Some(window) =
        app_handle.get_webview_window(crate::constants::ui::window_labels::FLOATING_BAR)
    else {
        return;
    };
    let Ok(monitors) = window.available_monitors() else {
        return;
    };
    let on_screen = monitors.iter().any(|m| {
        let p = m.position();
        let s = m.size();
        saved.x >= p.x
            && saved.x < p.x + s.width as i32
            && saved.y >= p.y
            && saved.y < p.y + s.height as i32
    });
    if !on_screen {
        return;
    }
    if let Err(e) = window.set_position(PhysicalPosition::new(saved.x, saved.y)) {
        warn!("Could not restore the floating bar's last position: {}", e);
    }
}

/// Remember the bar's landing position (physical px) for the next launch.
#[command]
pub async fn set_bar_position(app_handle: AppHandle, x: i32, y: i32) -> Result<(), String> {
    let store = app_handle
        .store(BAR_POSITION_STORE_FILE)
        .map_err(|e| format!("Failed to open bar-position store: {}", e))?;

    let value = serde_json::to_value(BarPosition { x, y })
        .map_err(|e| format!("Failed to serialize bar position: {}", e))?;

    store.set(BAR_POSITION_KEY, value);
    store
        .save()
        .map_err(|e| format!("Failed to save bar position: {}", e))?;

    Ok(())
}

/// Put the floating bar on screen, now that it knows where it belongs.
///
/// The window is created hidden at the frame in `tauri.conf.json`, which is a
/// placeholder: the bar only learns its real spot (the well it was left in last
/// launch, or the default one for this display) once the webview has mounted
/// and asked. Showing it before that meant it appeared at the placeholder frame
/// and then moved, which is the jump people saw while the app finished loading.
/// The bar calls this itself once its first frame is set, so the first thing on
/// screen is already the right one. `restore_bar_position` covers the case
/// where the webview is slow, by applying the stored well to the hidden window
/// during setup; this command stays the normal path because only the webview
/// knows the size the well has to be computed for.
///
/// Safe to call more than once: showing a visible window does nothing, and the
/// onboarding hold is re-checked on every call. The setup assistant is always
/// on top of the bar in the stacking order but has nothing to say to it, so a
/// bar that would land on top of onboarding is withheld and put up when setup
/// closes instead.
#[command]
pub async fn show_bar_when_ready(app_handle: AppHandle) -> Result<(), String> {
    if crate::window_management::onboarding_is_open(&app_handle) {
        crate::window_management::mark_bar_withheld_for_onboarding();
        return Ok(());
    }

    let window = app_handle
        .get_webview_window(crate::constants::ui::window_labels::FLOATING_BAR)
        .ok_or("floating-bar window not found")?;
    window
        .show()
        .map_err(|e| format!("Failed to show the floating bar: {}", e))
}

/// Move + resize the floating bar atomically so a compact<->hover transition
/// cannot show an intermediate frame (which made the centered dot jump). On
/// macOS this is a single `NSWindow setFrame:`, dispatched to the main thread;
/// elsewhere it falls back to separate position/size calls. `x`/`y` are the
/// target top-left in physical pixels; `width`/`height` are logical points.
#[command]
pub async fn set_bar_frame(
    app_handle: AppHandle,
    x: f64,
    y: f64,
    width: f64,
    height: f64,
) -> Result<(), String> {
    #[cfg(target_os = "macos")]
    {
        let app = app_handle.clone();
        let (tx, rx) = std::sync::mpsc::channel();
        app_handle
            .run_on_main_thread(move || {
                let _ = tx.send(crate::platform::macos::set_bar_frame_atomic(
                    &app, x, y, width, height,
                ));
            })
            .map_err(|e| e.to_string())?;
        rx.recv()
            .map_err(|e| format!("set_bar_frame main-thread call dropped: {}", e))?
    }
    #[cfg(not(target_os = "macos"))]
    {
        use tauri::{LogicalSize, Manager, PhysicalPosition};
        let window = app_handle
            .get_webview_window(crate::constants::ui::window_labels::FLOATING_BAR)
            .ok_or("floating-bar window not found")?;
        window
            .set_position(PhysicalPosition::new(x, y))
            .map_err(|e| e.to_string())?;
        window
            .set_size(LogicalSize::new(width, height))
            .map_err(|e| e.to_string())?;
        Ok(())
    }
}

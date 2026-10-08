//! # Floating-bar position persistence
//!
//! A tiny pair of commands that remember where the floating bar last settled
//! (the snapped well), so it reopens there on the next launch instead of the
//! default spot. Global desktop points (see `platform::desktop_points`),
//! stored in a small dedicated store file so it never interferes with the
//! centralized settings serialization.

use log::warn;
use serde::{Deserialize, Serialize};
use tauri::{command, AppHandle, LogicalPosition, Manager};
use tauri_plugin_store::StoreExt;

const BAR_POSITION_STORE_FILE: &str = crate::constants::settings::store_files::BAR_POSITION;
/// Points, not the physical pixels the old `last_well` key held. Physical
/// pixels meant a different thing on each display of a mixed-density desk, so
/// the old value is ignored rather than converted: it may not be on the
/// display it names, and the bar re-defaults to the top-right well once.
const BAR_POSITION_KEY: &str = "last_well_points";

/// The bar's last settled top-left, in global desktop points.
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
    let monitors = crate::platform::desktop_points::monitors_in_points(&window);
    let on_screen = crate::platform::desktop_points::monitor_index_at(
        &monitors,
        (saved.x as f64, saved.y as f64),
    )
    .is_some();
    if !on_screen {
        return;
    }
    if let Err(e) = window.set_position(LogicalPosition::new(saved.x, saved.y)) {
        warn!("Could not restore the floating bar's last position: {}", e);
    }
}

/// Remember the bar's landing position (global points) for the next launch.
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
    crate::startup_timing::mark_once("bar first paint reported");
    if crate::window_management::onboarding_is_open(&app_handle) {
        crate::window_management::mark_bar_withheld_for_onboarding();
        return Ok(());
    }

    app_handle
        .get_webview_window(crate::constants::ui::window_labels::FLOATING_BAR)
        .ok_or("floating-bar window not found")?;
    // Revealed, not shown: the bar arrives out of smoke every launch (see
    // `intro.rs`). A visible bar is left alone, so the repeat calls stay
    // harmless.
    crate::intro::show_bar_with_reveal(&app_handle);
    Ok(())
}

/// Where `set_bar_frame` left the bar's top-left, in global desktop points.
#[derive(Debug, Clone, Copy, Serialize)]
pub struct BarFrameOrigin {
    pub x: f64,
    pub y: f64,
}

/// Move + resize the floating bar atomically so a frame change cannot show an
/// intermediate frame. On macOS this is a single `NSWindow setFrame:`,
/// dispatched to the main thread; elsewhere it falls back to separate
/// position/size calls.
///
/// Everything is in global desktop points: `x`/`y` the target top-left,
/// `width`/`height` the size. Points are the one space that is the same on
/// every display, so a frame aimed at a well on a second display of another
/// density lands there (the old physical-pixel delta was scaled by the
/// display the window was leaving).
///
/// `grab_x`/`grab_y`, when both are given, place the window by the cursor
/// instead: that point inside the window (from its top-left) goes under the
/// cursor as it is at the moment the frame is set, and `x`/`y` are only the
/// fallback when the cursor cannot be read. The bar's drag uses it so the spot
/// the user pressed is still under the cursor when the OS drag takes over.
/// Returns the top-left the window ended up with.
#[command]
pub async fn set_bar_frame(
    app_handle: AppHandle,
    x: f64,
    y: f64,
    width: f64,
    height: f64,
    grab_x: Option<f64>,
    grab_y: Option<f64>,
) -> Result<BarFrameOrigin, String> {
    let grab = grab_x.zip(grab_y);
    #[cfg(target_os = "macos")]
    {
        let app = app_handle.clone();
        let (tx, rx) = tokio::sync::oneshot::channel();
        app_handle
            .run_on_main_thread(move || {
                let _ = tx.send(crate::platform::macos::set_bar_frame_atomic(
                    &app, x, y, width, height, grab,
                ));
            })
            .map_err(|e| e.to_string())?;
        let (x, y) = rx
            .await
            .map_err(|e| format!("set_bar_frame main-thread call dropped: {}", e))??;
        Ok(BarFrameOrigin { x, y })
    }
    #[cfg(not(target_os = "macos"))]
    {
        use crate::platform::desktop_points::{cursor_points, origin_under_cursor};
        use tauri::LogicalSize;
        let window = app_handle
            .get_webview_window(crate::constants::ui::window_labels::FLOATING_BAR)
            .ok_or("floating-bar window not found")?;
        let (x, y) = grab
            .and_then(|g| cursor_points(&app_handle).map(|c| origin_under_cursor(c, g)))
            .unwrap_or((x, y));
        window
            .set_position(LogicalPosition::new(x, y))
            .map_err(|e| e.to_string())?;
        window
            .set_size(LogicalSize::new(width, height))
            .map_err(|e| e.to_string())?;
        Ok(BarFrameOrigin { x, y })
    }
}

/// Whether the left mouse button is held right now.
///
/// During the OS window drag the page sees no mouse events, so it cannot tell
/// a long drag from a drag whose release it missed. The drop overlay keeps its
/// wells up for as long as this is true, however long the drag; the bar
/// settles a drag once it turns false, even if its own mouseup never arrived.
/// Errors off macOS, where both fall back to their timers.
#[command]
pub async fn bar_pointer_held() -> Result<bool, String> {
    #[cfg(target_os = "macos")]
    {
        Ok(crate::platform::bar_hit_test::left_button_down())
    }
    #[cfg(not(target_os = "macos"))]
    {
        Err("The pointer state is only read on macOS".to_string())
    }
}

/// A drop overlay has just been shown: keep the bar above it.
///
/// The overlay and the bar share the floating level, and showing the overlay
/// orders it in front of the bar, so its dim could cover the pill being
/// dragged. Only an overlay may ask, and only for itself; the decision is
/// `bar_stacking`'s.
#[command]
pub async fn bar_order_above_snap_wells(
    app_handle: AppHandle,
    window: tauri::WebviewWindow,
) -> Result<(), String> {
    let label = window.label();
    if !label.starts_with(crate::constants::ui::window_labels::SNAP_WELLS_OVERLAY) {
        return Err(format!("'{label}' is not a drop overlay"));
    }
    crate::bar_stacking::raise_over_overlay(&app_handle, label);
    Ok(())
}

/// Make sure every display has its own snap-well overlay window.
///
/// With "Displays have separate Spaces" (the macOS default) a window is drawn
/// on one display only, so the single overlay that used to span every monitor
/// showed the wells on one display and none on the others. The first overlay
/// is declared in `tauri.conf.json` and built after launch (`startup_windows`),
/// or here if a drag gets there first; display `i > 0` gets a copy of that
/// declaration labelled `snap-wells-overlay-i`, built the first time a drag
/// needs it. Returns how many displays there are. A copy that cannot be built
/// is logged and skipped: the other displays still get their wells.
#[command]
pub async fn ensure_snap_wells_overlays(app_handle: AppHandle) -> Result<usize, String> {
    let base_label = crate::constants::ui::window_labels::SNAP_WELLS_OVERLAY;
    let displays = app_handle
        .available_monitors()
        .map_err(|e| format!("Could not list displays: {e}"))?
        .len();
    let base = crate::window_management::declared_window_config(&app_handle, base_label)
        .ok_or("The snap-wells overlay is not declared in tauri.conf.json")?;
    for index in 0..displays {
        let label = overlay_label(index);
        if app_handle.get_webview_window(&label).is_some() {
            continue;
        }
        let mut config = base.clone();
        config.label = label.clone();
        config.visible = false;
        let built = tauri::WebviewWindowBuilder::from_config(&app_handle, &config)
            .and_then(|builder| builder.build());
        if let Err(e) = built {
            warn!("Could not build the snap-wells overlay '{}': {}", label, e);
        }
    }
    Ok(displays.max(1))
}

/// The overlay window for display `index`: the declared label for the first,
/// the label plus `-index` for the rest. The page reads its display back from it.
pub fn overlay_label(index: usize) -> String {
    let base = crate::constants::ui::window_labels::SNAP_WELLS_OVERLAY;
    if index == 0 {
        base.to_string()
    } else {
        format!("{base}-{index}")
    }
}

/// Tell the backend what a steady bar look is drawing, so the transparent rest
/// of its window lets clicks through.
///
/// `regions` are rectangles in logical pixels from the window's top-left.
/// `None` means the mounted look is not a steady one: click-through is turned
/// off and the window takes every mouse event again, as it always did. See
/// `platform::bar_hit_test`.
#[command]
pub async fn set_bar_hit_regions(
    app_handle: AppHandle,
    regions: Option<Vec<crate::platform::bar_hit_test::HitRect>>,
) -> Result<(), String> {
    let active = regions.is_some();
    crate::platform::bar_hit_test::set_regions(regions);
    if active {
        crate::platform::bar_hit_test::start(app_handle);
        return Ok(());
    }
    if let Some(window) =
        app_handle.get_webview_window(crate::constants::ui::window_labels::FLOATING_BAR)
    {
        window
            .set_ignore_cursor_events(false)
            .map_err(|e| format!("Failed to restore the bar's mouse events: {}", e))?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_first_display_keeps_the_declared_overlay() {
        assert_eq!(overlay_label(0), "snap-wells-overlay");
        assert_eq!(overlay_label(2), "snap-wells-overlay-2");
    }
}

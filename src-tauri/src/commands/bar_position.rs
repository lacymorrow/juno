//! # Floating-bar position persistence
//!
//! A tiny pair of commands that remember where the floating bar last settled
//! (the snapped well), so it reopens there on the next launch instead of the
//! default spot. Physical pixels, stored in a small dedicated store file so it
//! never interferes with the centralized settings serialization.

use serde::{Deserialize, Serialize};
use tauri::{command, AppHandle};
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

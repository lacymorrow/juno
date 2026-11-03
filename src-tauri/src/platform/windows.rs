//! # Windows Platform Module
//!
//! Windows-specific window behaviour for the Juno application.

use tauri::AppHandle;
use tracing::{info, warn};

/// Apply Windows-specific setup for all application windows
pub fn apply_windows_setup(_app_handle: &AppHandle) {
    info!("Windows-specific setup not implemented yet");
}

/// Make the floating bar a window that a click does not activate, the way the
/// Mac bar takes a click without taking the person away from their app.
///
/// The shape:
/// - At rest the bar is not focusable, which tao implements as
///   `WS_EX_NOACTIVATE`: clicking or dragging it leaves the foreground window
///   (and its caret) where they were, and showing it never activates it.
/// - When the person means to type (the Pill's "T" button, a click in its
///   input, the Island's input), the bar's page asks for focus with
///   `setFocus()`, as it already does on the Mac. That activates the window on
///   purpose (SetForegroundWindow), whatever its style.
/// - Once it is focused the bar is made focusable, so typing and clicking
///   inside it behave like any window's. The moment it loses focus (the person
///   clicks back into their app) it goes back to not focusable, and the next
///   plain click is non-activating again.
pub fn make_bar_non_activating(bar: &tauri::WebviewWindow) {
    if let Err(e) = bar.set_focusable(false) {
        warn!("[Bar] Could not make the bar non-activating: {}", e);
        return;
    }
    let window = bar.clone();
    bar.on_window_event(move |event| {
        if let tauri::WindowEvent::Focused(focused) = event {
            // Off the event handler, so the style change reaches the window as
            // its own event-loop message rather than inside the focus change
            // that is still being delivered.
            let window = window.clone();
            let focused = *focused;
            tauri::async_runtime::spawn(async move {
                if let Err(e) = window.set_focusable(focused) {
                    warn!("[Bar] Could not set the bar focusable={}: {}", focused, e);
                }
            });
        }
    });
}

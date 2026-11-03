//! # Windows Platform Module
//!
//! Windows-specific window behaviour for the Juno application.

use std::time::Duration;
use tauri::AppHandle;
use tracing::{info, warn};

/// Apply Windows-specific setup for all application windows
pub fn apply_windows_setup(_app_handle: &AppHandle) {
    info!("Windows-specific setup not implemented yet");
}

/// How long after the bar reports losing focus to look at who has the
/// foreground. Focus moving from the bar's window into its WebView2 (a click
/// in the input) reports a loss too, while the bar is still in front.
const FOREGROUND_SETTLE: Duration = Duration::from_millis(150);

/// Make the floating bar a window that a click does not activate, the way the
/// Mac bar takes a click without taking the person away from their app.
///
/// The shape:
/// - At rest the bar is not focusable, which tao implements as
///   `WS_EX_NOACTIVATE`: clicking or dragging it leaves the foreground window
///   (and its caret) where they were, and showing it never activates it.
/// - When the person means to type (the Pill's "T" button, a click in its
///   input, the Island's input), the page calls `focus_bar_for_typing`, which
///   makes the bar focusable and activates it ([`activate_bar_for_typing`]).
/// - Once some other window has the foreground (the person clicked back into
///   their app), the bar goes back to not focusable. Only then: making the
///   foreground window non-activating deactivates it, which is what closed the
///   bar chat when it was done on every reported focus loss.
pub fn make_bar_non_activating(bar: &tauri::WebviewWindow) {
    if let Err(e) = bar.set_focusable(false) {
        warn!("[Bar] Could not make the bar non-activating: {}", e);
        return;
    }
    let window = bar.clone();
    bar.on_window_event(move |event| {
        let tauri::WindowEvent::Focused(focused) = event else {
            return;
        };
        // Off the event handler, so the style change reaches the window as
        // its own event-loop message rather than inside the focus change
        // that is still being delivered.
        let window = window.clone();
        let focused = *focused;
        tauri::async_runtime::spawn(async move {
            if !focused {
                tokio::time::sleep(FOREGROUND_SETTLE).await;
                if is_foreground(&window) {
                    return;
                }
            }
            if let Err(e) = window.set_focusable(focused) {
                warn!("[Bar] Could not set the bar focusable={}: {}", focused, e);
            }
        });
    });
}

/// Whether the bar's window is the foreground window right now.
fn is_foreground(bar: &tauri::WebviewWindow) -> bool {
    let Ok(hwnd) = bar.hwnd() else {
        return false;
    };
    // SAFETY: GetForegroundWindow takes nothing and returns a handle value,
    // which is only compared.
    let foreground = unsafe { windows_sys::Win32::UI::WindowsAndMessaging::GetForegroundWindow() };
    foreground as isize == hwnd.0 as isize
}

/// Make the bar focusable and give it the foreground and the keyboard.
///
/// The click that asked for this landed in WebView2's process, not Juno's, so
/// Windows does not let Juno take the foreground with a plain
/// `SetForegroundWindow`. tao's fallback presses Alt with `SendInput`, and that
/// Alt leaves the window in menu mode, where the next key typed is eaten.
/// Attaching to the foreground thread's input for the call is the other way
/// to be allowed, and it sends no keys. Focusing the bar's window hands the
/// keyboard to the page (wry moves focus into WebView2 on WM_SETFOCUS).
pub fn activate_bar_for_typing(bar: &tauri::WebviewWindow) {
    if let Err(e) = bar.set_focusable(true) {
        warn!("[Bar] Could not make the bar focusable: {}", e);
    }
    let Ok(hwnd) = bar.hwnd() else {
        warn!("[Bar] No window handle for the bar; cannot give it the keyboard");
        return;
    };
    let hwnd = hwnd.0 as isize;
    // After the style change above, which is queued on the same thread.
    if let Err(e) = bar.run_on_main_thread(move || {
        // SAFETY: plain Win32 calls on a window handle this process owns, on
        // the thread that owns it. The input attachment is undone before
        // returning.
        unsafe { take_foreground(hwnd as _) }
    }) {
        warn!(
            "[Bar] Could not reach the main thread to focus the bar: {}",
            e
        );
    }
}

/// Foreground and keyboard focus for `hwnd`, borrowing the foreground
/// thread's input state for the duration of the call.
unsafe fn take_foreground(hwnd: windows_sys::Win32::Foundation::HWND) {
    use windows_sys::Win32::System::Threading::{AttachThreadInput, GetCurrentThreadId};
    use windows_sys::Win32::UI::Input::KeyboardAndMouse::SetFocus;
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        BringWindowToTop, GetForegroundWindow, GetWindowThreadProcessId, SetForegroundWindow,
    };

    let foreground = GetForegroundWindow();
    // Already in front: the keyboard is in the page (a click in the input of
    // an open chat lands here). Focusing the window again would pull focus out
    // of WebView2 and back, which the page reads as a blur.
    if foreground == hwnd {
        return;
    }
    let ours = GetCurrentThreadId();
    let theirs = if foreground.is_null() {
        0
    } else {
        GetWindowThreadProcessId(foreground, std::ptr::null_mut())
    };
    let attached = theirs != 0 && theirs != ours && AttachThreadInput(ours, theirs, 1) != 0;
    BringWindowToTop(hwnd);
    if SetForegroundWindow(hwnd) == 0 {
        warn!("[Bar] Windows refused the bar the foreground");
    }
    SetFocus(hwnd);
    if attached {
        AttachThreadInput(ours, theirs, 0);
    }
}

//! # Windows Platform Module
//!
//! Windows-specific window behaviour for the Juno application.

use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;
use tauri::{AppHandle, Manager};
use tracing::{info, warn};

/// Apply Windows-specific setup for all application windows
pub fn apply_windows_setup(_app_handle: &AppHandle) {
    info!("Windows-specific setup not implemented yet");
}

/// Whether the bar is showing its chat (the Pill's pane, the Island's chat
/// face), as the page reports it through `set_bar_chat_open`.
static BAR_CHAT_OPEN: AtomicBool = AtomicBool::new(false);

/// How long after the bar reports losing focus to look at who has the
/// foreground. Focus moving from the bar's window into its WebView2 reports a
/// loss too, while the bar is still in front.
const FOREGROUND_SETTLE: Duration = Duration::from_millis(150);

/// The floating bar's focus on Windows, the way the Mac bar behaves.
///
/// One rule: the bar is an ordinary activatable window while it is open for
/// typing (its chat is open, or its text input was asked for), and
/// non-activating (`WS_EX_NOACTIVATE`, tao's `focusable: false`) only while it
/// is collapsed. Collapsed, a click or a drag on the Pill leaves the
/// foreground app and its caret alone. Open, a click anywhere in it activates
/// it like any window, so clicking into the chat's input just works.
///
/// - Opening for typing (the T button, the expand button, a click in the
///   input) calls `focus_bar_for_typing`: focusable, then activated
///   ([`activate_bar_for_typing`]).
/// - The chat opening or closing arrives through [`bar_chat_open_changed`].
/// - Collapsing back to non-activating waits until another window has the
///   foreground. Making the foreground window non-activating deactivates it
///   and hands the foreground to the desktop, so it is never done to the bar
///   while the bar is in front.
pub fn make_bar_non_activating(bar: &tauri::WebviewWindow) {
    if let Err(e) = bar.set_focusable(false) {
        warn!("[Bar] Could not make the bar non-activating: {}", e);
        return;
    }
    install_keyboard_menu_guard(bar);
    let window = bar.clone();
    bar.on_window_event(move |event| {
        let tauri::WindowEvent::Focused(false) = event else {
            return;
        };
        let window = window.clone();
        tauri::async_runtime::spawn(async move {
            tokio::time::sleep(FOREGROUND_SETTLE).await;
            collapse_if_put_away(&window);
        });
    });
}

/// The page says the bar's chat opened or closed.
pub fn bar_chat_open_changed(app: &AppHandle, open: bool) {
    BAR_CHAT_OPEN.store(open, Ordering::SeqCst);
    let Some(bar) = app.get_webview_window(crate::constants::ui::window_labels::FLOATING_BAR)
    else {
        return;
    };
    if open {
        set_focusable(&bar, true);
    } else {
        collapse_if_put_away(&bar);
    }
}

/// Non-activating again, once the bar is collapsed and not in front.
fn collapse_if_put_away(bar: &tauri::WebviewWindow) {
    if BAR_CHAT_OPEN.load(Ordering::SeqCst) || is_foreground(bar) {
        return;
    }
    set_focusable(bar, false);
}

fn set_focusable(bar: &tauri::WebviewWindow, focusable: bool) {
    if let Err(e) = bar.set_focusable(focusable) {
        warn!("[Bar] Could not set the bar focusable={}: {}", focusable, e);
    }
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

/// Make the bar activatable and bring it to the front with the keyboard.
///
/// The click that asks for this landed in WebView2's process, so Windows does
/// not let Juno take the foreground with a plain `SetForegroundWindow`. tao's
/// `set_focus` then presses Alt, which unlocks it; [`install_keyboard_menu_guard`]
/// keeps that Alt from opening keyboard menu mode, which would eat the next
/// key typed.
pub fn activate_bar_for_typing(bar: &tauri::WebviewWindow) {
    set_focusable(bar, true);
    if let Err(e) = bar.set_focus() {
        warn!("[Bar] Could not bring the bar to the front: {}", e);
    }
}

/// Swallow `SC_KEYMENU` on the bar. A lone Alt (tao's foreground unlock, or
/// the person's own) puts a window in keyboard menu mode, where the next key
/// goes to a menu instead of the page. The bar has no menu, so there is
/// nothing to lose.
fn install_keyboard_menu_guard(bar: &tauri::WebviewWindow) {
    let Ok(hwnd) = bar.hwnd() else {
        warn!("[Bar] No window handle for the bar; Alt may still eat a key");
        return;
    };
    let hwnd = hwnd.0 as isize;
    // A subclass is installed from the thread that owns the window.
    let installed = bar.run_on_main_thread(move || {
        // SAFETY: `hwnd` is the bar's live window, owned by this thread, and
        // `no_keyboard_menu` matches SUBCLASSPROC.
        let ok = unsafe {
            windows_sys::Win32::UI::Shell::SetWindowSubclass(
                hwnd as _,
                Some(no_keyboard_menu),
                KEYBOARD_MENU_GUARD_ID,
                0,
            )
        };
        if ok == 0 {
            warn!("[Bar] Could not install the keyboard menu guard");
        }
    });
    if let Err(e) = installed {
        warn!(
            "[Bar] Could not reach the main thread for the keyboard menu guard: {}",
            e
        );
    }
}

/// Identifies Juno's subclass on the bar window among tao's and wry's.
const KEYBOARD_MENU_GUARD_ID: usize = 0x4A55_4E4F; // "JUNO"

unsafe extern "system" fn no_keyboard_menu(
    hwnd: windows_sys::Win32::Foundation::HWND,
    msg: u32,
    wparam: usize,
    lparam: isize,
    _id: usize,
    _data: usize,
) -> isize {
    use windows_sys::Win32::UI::WindowsAndMessaging::{SC_KEYMENU, WM_SYSCOMMAND};
    if msg == WM_SYSCOMMAND && (wparam & 0xFFF0) == SC_KEYMENU as usize {
        return 0;
    }
    windows_sys::Win32::UI::Shell::DefSubclassProc(hwnd, msg, wparam, lparam)
}

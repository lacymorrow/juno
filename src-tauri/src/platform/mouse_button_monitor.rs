//! # Passive mouse-button monitor
//!
//! Lets a trigger fire from a mouse button (typically a spare side button, but
//! any button works) the same way `stop_key_monitor` observes the stop key:
//! with non-consuming `NSEvent` global + local monitors. The button events are
//! never swallowed, so normal clicking is unaffected in every other app.
//!
//! The tauri global-shortcut plugin is keyboard-only, so mouse bindings cannot
//! go through it; this observer is how [`crate::triggers::Binding::Mouse`]
//! bindings reach [`crate::events::shortcuts::fire_trigger_edge`].
//!
//! Global monitors only see events in other apps when the process is trusted
//! for Accessibility (or Input Monitoring) — the same permission Juno already
//! needs for computer use.

use crate::triggers::{TriggerMethod, TriggerTarget};
use tauri::AppHandle;

/// AppKit `buttonNumber` mapped to the trigger it activates.
pub type MouseBinding = (u16, TriggerMethod, TriggerTarget);

/// Compute whether an AppKit mouse event type is a press (down) edge.
/// Down types: Left=1, Right=3, Other=25. Up types: Left=2, Right=4, Other=26.
pub fn is_mouse_down(event_type: usize) -> Option<bool> {
    match event_type {
        1 | 3 | 25 => Some(true),
        2 | 4 | 26 => Some(false),
        _ => None,
    }
}

#[cfg(target_os = "macos")]
mod imp {
    use super::{is_mouse_down, MouseBinding};
    use block::ConcreteBlock;
    use cocoa::base::{id, nil};
    use objc::{class, msg_send, sel, sel_impl};
    use std::ffi::c_void;
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::Mutex;
    use tauri::AppHandle;
    use tracing::{debug, error, info, warn};

    // NSEventMask bits: 1 << NSEventType. Left/Right/Other mouse down+up.
    const LEFT_DOWN: usize = 1 << 1;
    const LEFT_UP: usize = 1 << 2;
    const RIGHT_DOWN: usize = 1 << 3;
    const RIGHT_UP: usize = 1 << 4;
    const OTHER_DOWN: usize = 1 << 25;
    const OTHER_UP: usize = 1 << 26;

    struct Monitors {
        global: id,
        local: id,
    }
    unsafe impl Send for Monitors {}

    static MONITORS: Mutex<Option<Monitors>> = Mutex::new(None);
    static INSTALLED: AtomicBool = AtomicBool::new(false);
    static BINDINGS: Mutex<Vec<MouseBinding>> = Mutex::new(Vec::new());

    /// The NSEvent mask covering only the button kinds actually bound, so we
    /// don't observe left-clicks unless a left-click trigger exists.
    fn mask_for(bindings: &[MouseBinding]) -> usize {
        let mut mask = 0usize;
        for (button, _, _) in bindings {
            mask |= match button {
                0 => LEFT_DOWN | LEFT_UP,
                1 => RIGHT_DOWN | RIGHT_UP,
                _ => OTHER_DOWN | OTHER_UP,
            };
        }
        mask
    }

    fn on_event(app: &AppHandle, event: id) {
        if event == nil {
            return;
        }
        // SAFETY: `event` is a live NSEvent handed to us by AppKit for the
        // duration of the handler; these selectors are plain getters.
        let (event_type, button_number): (usize, i64) = unsafe {
            let event_type: usize = msg_send![event, type];
            let button_number: i64 = msg_send![event, buttonNumber];
            (event_type, button_number)
        };
        let Some(pressed) = is_mouse_down(event_type) else {
            return;
        };
        if button_number < 0 {
            return;
        }
        let button = button_number as u16;

        // Copy out the matching triggers, then release the lock before routing.
        let matches: Vec<(super::TriggerMethod, super::TriggerTarget)> = {
            let guard = match BINDINGS.lock() {
                Ok(g) => g,
                Err(poisoned) => poisoned.into_inner(),
            };
            guard
                .iter()
                .filter(|(b, _, _)| *b == button)
                .map(|(_, m, t)| (*m, *t))
                .collect()
        };
        for (method, target) in matches {
            debug!(
                "[MouseButtonMonitor] Button {} {} -> {:?}/{:?}",
                button,
                if pressed { "down" } else { "up" },
                method,
                target
            );
            crate::events::shortcuts::fire_trigger_edge(app, method, target, pressed);
        }
    }

    /// Install (or re-install) the monitors for the given bindings. Removing any
    /// prior monitors first keeps the observed mask in sync with the bindings.
    pub fn sync(app: &AppHandle, bindings: Vec<MouseBinding>) -> Result<(), String> {
        remove(app)?;

        if let Ok(mut guard) = BINDINGS.lock() {
            *guard = bindings.clone();
        }
        if bindings.is_empty() {
            return Ok(());
        }

        match crate::commands::native_permissions::NativePermissionChecker::check_accessibility_permission() {
            Ok(true) => {}
            Ok(false) => warn!(
                "[MouseButtonMonitor] Accessibility permission is NOT granted — mouse-button triggers will only fire while a Juno window is focused. Grant Accessibility in System Settings > Privacy & Security."
            ),
            Err(e) => warn!("[MouseButtonMonitor] Could not verify Accessibility permission: {}", e),
        }

        let mask = mask_for(&bindings);
        let app_for_main = app.clone();
        app.run_on_main_thread(move || {
            let mut guard = match MONITORS.lock() {
                Ok(g) => g,
                Err(poisoned) => poisoned.into_inner(),
            };
            if guard.is_some() {
                INSTALLED.store(true, Ordering::SeqCst);
                return;
            }

            let app_global = app_for_main.clone();
            let global_block = ConcreteBlock::new(move |event: id| {
                on_event(&app_global, event);
            })
            .copy();

            let app_local = app_for_main.clone();
            let local_block = ConcreteBlock::new(move |event: id| -> id {
                on_event(&app_local, event);
                event // hand back untouched: never consume the click
            })
            .copy();

            // SAFETY: called on the main thread; the blocks are heap copies
            // AppKit retains for the lifetime of the monitor.
            let (global, local): (id, id) = unsafe {
                let global: id = msg_send![
                    class!(NSEvent),
                    addGlobalMonitorForEventsMatchingMask: mask
                    handler: &*global_block as *const _ as *const c_void
                ];
                let local: id = msg_send![
                    class!(NSEvent),
                    addLocalMonitorForEventsMatchingMask: mask
                    handler: &*local_block as *const _ as *const c_void
                ];
                (global, local)
            };

            if global == nil && local == nil {
                error!("[MouseButtonMonitor] AppKit refused both NSEvent monitors — mouse triggers will not fire");
                return;
            }
            if global == nil {
                warn!("[MouseButtonMonitor] Global NSEvent monitor unavailable — mouse triggers only fire while Juno is focused");
            }

            *guard = Some(Monitors { global, local });
            INSTALLED.store(true, Ordering::SeqCst);
            info!(
                "[MouseButtonMonitor] Installed (mask={:#x}, global={}, local={})",
                mask,
                global != nil,
                local != nil
            );
        })
        .map_err(|e| format!("Failed to dispatch mouse-button monitor install to main thread: {}", e))
    }

    pub fn remove(app: &AppHandle) -> Result<(), String> {
        if !INSTALLED.load(Ordering::SeqCst) {
            return Ok(());
        }
        INSTALLED.store(false, Ordering::SeqCst);

        app.run_on_main_thread(move || {
            let taken = match MONITORS.lock() {
                Ok(mut g) => g.take(),
                Err(poisoned) => poisoned.into_inner().take(),
            };
            if let Some(monitors) = taken {
                // SAFETY: main thread; tokens came from addGlobal/LocalMonitor above.
                unsafe {
                    if monitors.global != nil {
                        let _: () = msg_send![class!(NSEvent), removeMonitor: monitors.global];
                    }
                    if monitors.local != nil {
                        let _: () = msg_send![class!(NSEvent), removeMonitor: monitors.local];
                    }
                }
                info!("[MouseButtonMonitor] Removed");
            }
        })
        .map_err(|e| {
            format!(
                "Failed to dispatch mouse-button monitor removal to main thread: {}",
                e
            )
        })
    }
}

#[cfg(not(target_os = "macos"))]
mod imp {
    use super::MouseBinding;
    use tauri::AppHandle;

    pub fn sync(_app: &AppHandle, _bindings: Vec<MouseBinding>) -> Result<(), String> {
        Ok(())
    }

    pub fn remove(_app: &AppHandle) -> Result<(), String> {
        Ok(())
    }
}

/// Install/refresh the passive mouse-button monitor for the given bindings.
/// An empty list tears the monitor down.
pub fn sync(app: &AppHandle, bindings: Vec<MouseBinding>) -> Result<(), String> {
    imp::sync(app, bindings)
}

/// Remove the monitor if present.
pub fn remove(app: &AppHandle) -> Result<(), String> {
    imp::remove(app)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn down_and_up_types_classified() {
        assert_eq!(is_mouse_down(25), Some(true)); // OtherMouseDown
        assert_eq!(is_mouse_down(26), Some(false)); // OtherMouseUp
        assert_eq!(is_mouse_down(1), Some(true)); // LeftMouseDown
        assert_eq!(is_mouse_down(2), Some(false)); // LeftMouseUp
        assert_eq!(is_mouse_down(10), None); // KeyDown is not a mouse event
    }
}

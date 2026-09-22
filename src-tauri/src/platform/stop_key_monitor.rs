//! # Passive stop-key monitor
//!
//! Observes the user's stop key (Escape by default) the way an ordinary macOS
//! app does: with `NSEvent` monitors that *watch* key events instead of an
//! exclusive Carbon hot key that swallows them.
//!
//! * `addGlobalMonitorForEventsMatchingMask:` sees presses delivered to other
//!   applications. AppKit only reports key events to a global monitor when the
//!   process is trusted for Accessibility (or Input Monitoring) — the same
//!   permission Juno already needs for computer use. Adding a global *key*
//!   monitor from an untrusted process also makes macOS raise its own
//!   "Accessibility Access" alert, so the global monitor is only added once
//!   Accessibility is granted (it would deliver nothing before that anyway).
//!   Onboarding installs this monitor before its window opens; without the
//!   gate, the system alert beat the onboarding window to the screen.
//! * `addLocalMonitorForEventsMatchingMask:` sees presses delivered to Juno's
//!   own windows. The handler returns the event untouched, so the web view and
//!   any HTML dropdown still receive it.
//!
//! Neither monitor can consume the event, so every other app that cares about
//! Escape keeps receiving it exactly as before. The monitor is installed only
//! while Juno has something to stop (see `EscapeKeyCoordinator`) and removed
//! when idle. When Accessibility lands while the monitor is up (the 1Hz
//! permissions poller notices), `ensure_global` adds the global half.
//!
//! Everything that touches AppKit runs on the main thread via
//! `AppHandle::run_on_main_thread`; the callbacks hand the press off to the
//! shortcut handler, which spawns the stop work on the async runtime.

use tauri::AppHandle;

/// NSEventModifierFlagShift | Control | Option | Command. Caps Lock (1 << 16)
/// and Fn (1 << 23) are deliberately ignored — "Escape with Caps Lock on" is
/// still Escape.
const CHORD_MODIFIER_MASK: usize = (1 << 17) | (1 << 18) | (1 << 19) | (1 << 20);

/// Decide whether a raw key event is a press of the bare stop key.
///
/// Pure so the matching rules are unit-testable without AppKit:
/// * the virtual key code must match,
/// * no chord modifier (Shift/Control/Option/Command) may be held,
/// * auto-repeat events are ignored so holding the key fires once.
pub fn is_stop_key_press(
    key_code: u16,
    modifier_flags: usize,
    is_repeat: bool,
    target: u16,
) -> bool {
    key_code == target && modifier_flags & CHORD_MODIFIER_MASK == 0 && !is_repeat
}

#[cfg(target_os = "macos")]
mod imp {
    use super::is_stop_key_press;
    use block::ConcreteBlock;
    use cocoa::base::{id, nil, BOOL, NO};
    use objc::{class, msg_send, sel, sel_impl};
    use std::ffi::c_void;
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::Mutex;
    use tauri::AppHandle;
    use tracing::{debug, error, info, warn};

    /// NSEventMaskKeyDown | NSEventMaskKeyUp (1 << NSEventTypeKeyDown, 1 << NSEventTypeKeyUp).
    const KEY_EVENT_MASK: usize = (1 << 10) | (1 << 11);
    const NS_EVENT_TYPE_KEY_DOWN: usize = 10;

    /// Monitor tokens handed back by AppKit. Only ever touched on the main
    /// thread; the `Send` impl exists so they can live in a `static` Mutex.
    /// `global` stays `nil` until Accessibility is granted (see module docs).
    struct Monitors {
        global: id,
        local: id,
        key_code: u16,
    }
    unsafe impl Send for Monitors {}

    static MONITORS: Mutex<Option<Monitors>> = Mutex::new(None);
    static INSTALLED: AtomicBool = AtomicBool::new(false);

    /// Whether adding the global key monitor is safe right now: only when the
    /// process is already trusted for Accessibility. Untrusted, the call is
    /// useless (AppKit delivers nothing) and it makes macOS raise the native
    /// Accessibility alert on Juno's behalf.
    fn accessibility_trusted() -> bool {
        match crate::commands::native_permissions::NativePermissionChecker::check_accessibility_permission() {
            Ok(true) => true,
            Ok(false) => false,
            Err(e) => {
                warn!("[StopKeyMonitor] Could not verify Accessibility permission: {}", e);
                false
            }
        }
    }

    /// Add the global monitor for `key_code`. Main thread only.
    ///
    /// SAFETY: must be called on the main thread; the block is a heap copy
    /// that AppKit retains for the lifetime of the monitor.
    unsafe fn add_global_monitor(app: &AppHandle, key_code: u16) -> id {
        let app_global = app.clone();
        let global_block = ConcreteBlock::new(move |event: id| {
            on_event(&app_global, event, key_code);
        })
        .copy();
        msg_send![
            class!(NSEvent),
            addGlobalMonitorForEventsMatchingMask: KEY_EVENT_MASK
            handler: &*global_block as *const _ as *const c_void
        ]
    }

    fn on_event(app: &AppHandle, event: id, target: u16) {
        if event == nil {
            return;
        }
        // A keystroke Juno itself synthesized (dictation insertion, agent
        // typing) must never read as the user pressing the stop key.
        // SAFETY: `event` is a live NSEvent for the duration of the handler.
        if unsafe { crate::platform::synthetic_events::is_juno_synthesized_event(event) } {
            return;
        }
        // SAFETY: `event` is a live NSEvent handed to us by AppKit for the
        // duration of the handler; these selectors are plain getters.
        let (key_code, flags, is_repeat, event_type): (u16, usize, bool, usize) = unsafe {
            let key_code: u16 = msg_send![event, keyCode];
            let flags: usize = msg_send![event, modifierFlags];
            let repeat: BOOL = msg_send![event, isARepeat];
            let event_type: usize = msg_send![event, type];
            (key_code, flags, repeat != NO, event_type)
        };
        if !is_stop_key_press(key_code, flags, is_repeat, target) {
            return;
        }
        let pressed = event_type == NS_EVENT_TYPE_KEY_DOWN;
        debug!(
            "[StopKeyMonitor] Observed stop key (code {}) {}",
            key_code,
            if pressed { "down" } else { "up" }
        );
        crate::events::shortcuts::handle_stop_key_event(app, pressed);
    }

    /// Install the global + local monitors for `key_code`. Idempotent.
    ///
    /// Returns once the install has been *dispatched* to the main thread; the
    /// monitors themselves come up a tick later. Failures on the main thread
    /// are logged and leave `is_installed()` false.
    pub fn install(app: &AppHandle, key_code: u16) -> Result<(), String> {
        if INSTALLED.load(Ordering::SeqCst) {
            debug!("[StopKeyMonitor] Already installed; checking the global half");
            // A monitor installed before Accessibility was granted is local
            // only; a later install (an agent run) is the moment to complete it.
            return ensure_global(app);
        }

        let trusted = accessibility_trusted();
        if !trusted {
            info!(
                "[StopKeyMonitor] Accessibility not granted yet — installing the local monitor only; the global one is added when Accessibility lands"
            );
        }

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

            let app_local = app_for_main.clone();
            let local_block = ConcreteBlock::new(move |event: id| -> id {
                on_event(&app_local, event, key_code);
                // Hand the event back untouched so Juno's own web views still see it.
                event
            })
            .copy();

            // SAFETY: called on the main thread; the blocks are heap copies
            // that AppKit retains for the lifetime of the monitor.
            let (global, local): (id, id) = unsafe {
                let global: id = if trusted {
                    add_global_monitor(&app_for_main, key_code)
                } else {
                    nil
                };
                let local: id = msg_send![
                    class!(NSEvent),
                    addLocalMonitorForEventsMatchingMask: KEY_EVENT_MASK
                    handler: &*local_block as *const _ as *const c_void
                ];
                (global, local)
            };

            if global == nil && local == nil {
                error!("[StopKeyMonitor] AppKit refused both NSEvent monitors — stop key will not be observed");
                return;
            }
            if global == nil && trusted {
                warn!("[StopKeyMonitor] Global NSEvent monitor unavailable — stop key only observed while Juno is focused");
            }

            *guard = Some(Monitors {
                global,
                local,
                key_code,
            });
            INSTALLED.store(true, Ordering::SeqCst);
            info!(
                "[StopKeyMonitor] Passive stop-key monitor installed (key code {}, global={}, local={})",
                key_code,
                global != nil,
                local != nil
            );
        })
        .map_err(|e| format!("Failed to dispatch stop-key monitor install to main thread: {}", e))
    }

    /// Add the global half of an installed monitor once Accessibility is
    /// granted. No-op when nothing is installed, the global monitor already
    /// exists, or the process is still untrusted.
    pub fn ensure_global(app: &AppHandle) -> Result<(), String> {
        if !INSTALLED.load(Ordering::SeqCst) {
            return Ok(());
        }
        let needs_global = match MONITORS.lock() {
            Ok(g) => g.as_ref().map(|m| m.global == nil).unwrap_or(false),
            Err(poisoned) => poisoned
                .into_inner()
                .as_ref()
                .map(|m| m.global == nil)
                .unwrap_or(false),
        };
        if !needs_global || !accessibility_trusted() {
            return Ok(());
        }

        let app_for_main = app.clone();
        app.run_on_main_thread(move || {
            let mut guard = match MONITORS.lock() {
                Ok(g) => g,
                Err(poisoned) => poisoned.into_inner(),
            };
            let Some(monitors) = guard.as_mut() else {
                return;
            };
            if monitors.global != nil {
                return;
            }
            // SAFETY: main thread (run_on_main_thread), see add_global_monitor.
            let global = unsafe { add_global_monitor(&app_for_main, monitors.key_code) };
            if global == nil {
                warn!("[StopKeyMonitor] Global NSEvent monitor unavailable — stop key only observed while Juno is focused");
                return;
            }
            monitors.global = global;
            info!(
                "[StopKeyMonitor] Global stop-key monitor added now that Accessibility is granted (key code {})",
                monitors.key_code
            );
        })
        .map_err(|e| format!("Failed to dispatch stop-key monitor upgrade to main thread: {}", e))
    }

    /// Remove the monitors. Idempotent; safe to call when nothing is installed.
    pub fn remove(app: &AppHandle) -> Result<(), String> {
        if !INSTALLED.load(Ordering::SeqCst) {
            debug!("[StopKeyMonitor] Not installed, nothing to remove");
            return Ok(());
        }
        // Flip the flag first so a concurrent install() after this point re-installs.
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
                info!("[StopKeyMonitor] Passive stop-key monitor removed");
            }
        })
        .map_err(|e| {
            format!(
                "Failed to dispatch stop-key monitor removal to main thread: {}",
                e
            )
        })
    }

    pub fn is_installed() -> bool {
        INSTALLED.load(Ordering::SeqCst)
    }
}

#[cfg(not(target_os = "macos"))]
mod imp {
    use tauri::AppHandle;

    pub fn install(_app: &AppHandle, _key_code: u16) -> Result<(), String> {
        Err("Passive stop-key monitor is only available on macOS".to_string())
    }

    pub fn remove(_app: &AppHandle) -> Result<(), String> {
        Ok(())
    }

    pub fn ensure_global(_app: &AppHandle) -> Result<(), String> {
        Ok(())
    }

    pub fn is_installed() -> bool {
        false
    }
}

/// Install the passive monitor for `key_code` (macOS virtual key code, 53 = Escape).
pub fn install(app: &AppHandle, key_code: u16) -> Result<(), String> {
    imp::install(app, key_code)
}

/// Complete an installed monitor with its global half once Accessibility is
/// granted. Called when the permissions poller sees the grant land; safe to
/// call any time.
pub fn ensure_global(app: &AppHandle) -> Result<(), String> {
    imp::ensure_global(app)
}

/// Remove the passive monitor if present.
pub fn remove(app: &AppHandle) -> Result<(), String> {
    imp::remove(app)
}

/// Whether the passive monitor is currently installed.
pub fn is_installed() -> bool {
    imp::is_installed()
}

#[cfg(test)]
mod tests {
    use super::*;

    const ESCAPE: u16 = 53;

    #[test]
    fn bare_escape_matches() {
        assert!(is_stop_key_press(ESCAPE, 0, false, ESCAPE));
    }

    #[test]
    fn caps_lock_and_fn_do_not_turn_escape_into_a_chord() {
        assert!(is_stop_key_press(ESCAPE, 1 << 16, false, ESCAPE));
        assert!(is_stop_key_press(ESCAPE, 1 << 23, false, ESCAPE));
        // AppKit also sets device-dependent low bits; those are not chords either.
        assert!(is_stop_key_press(ESCAPE, 0x100, false, ESCAPE));
    }

    #[test]
    fn chord_modifiers_do_not_match() {
        for flag in [1usize << 17, 1 << 18, 1 << 19, 1 << 20] {
            assert!(
                !is_stop_key_press(ESCAPE, flag, false, ESCAPE),
                "modifier flag {:#x} should not count as a bare press",
                flag
            );
        }
    }

    #[test]
    fn other_keys_and_repeats_do_not_match() {
        assert!(!is_stop_key_press(49, 0, false, ESCAPE)); // Space
        assert!(!is_stop_key_press(ESCAPE, 0, true, ESCAPE)); // auto-repeat
    }
}

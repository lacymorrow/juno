//! # Passive bare-modifier monitor (the Fn/globe key)
//!
//! Fn produces no ordinary key event. It arrives as `NSEventTypeFlagsChanged`,
//! which the tauri global-shortcut plugin cannot register and a webview never
//! sees, so it needs its own observer, in the same non-consuming shape as
//! [`crate::platform::mouse_button_monitor`].
//!
//! Verified on hardware before this was written: a global monitor does receive
//! the Fn key, reporting `keyCode` 63 with bit `1 << 23` set on press and clear
//! on release. Clean down and up edges, which is what push-to-talk needs.
//!
//! Caps Lock is deliberately not offered. The same check showed it emits one
//! event per press and none on release, because it is a hardware toggle: the
//! "up" edge only arrives when you press it again. Push-to-talk on a key with
//! no release edge would hold the microphone open until the next press, which
//! is not push-to-talk. No amount of code here changes that; remapping it with
//! `hidutil` to a spare function key is the honest route, and that already
//! works through the ordinary keyboard binding.
//!
//! Global monitors only see events in other apps once the process is trusted
//! for Accessibility. Until then this installs the local monitor only: adding a
//! global *key* monitor while untrusted delivers nothing and makes macOS raise
//! its own Accessibility alert on Juno's behalf, which is exactly the surprise
//! the onboarding work went to some trouble to remove.

use crate::triggers::{ModifierKey, TriggerMethod, TriggerTarget};
use tauri::AppHandle;

/// A bare modifier key mapped to the trigger it activates.
pub type ModifierBinding = (ModifierKey, TriggerMethod, TriggerTarget);

/// Which edge a `flagsChanged` event represents for `target`, if any.
///
/// `Some(true)` is press, `Some(false)` is release, `None` means the event was
/// about some other key.
///
/// The key code has to be checked, not just the flag bit: AppKit sets the
/// Function bit on arrow keys, F1 through F20, Home, End, Page Up, Page Down
/// and Delete as well, so testing the bit alone would fire the microphone on
/// every arrow press.
pub fn modifier_edge(key_code: u16, modifier_flags: usize, target: ModifierKey) -> Option<bool> {
    if key_code != target.key_code() {
        return None;
    }
    Some(modifier_flags & target.flag_bit() != 0)
}

#[cfg(target_os = "macos")]
mod imp {
    use super::{modifier_edge, ModifierBinding};
    use block::ConcreteBlock;
    use cocoa::base::{id, nil};
    use objc::{class, msg_send, sel, sel_impl};
    use std::ffi::c_void;
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::Mutex;
    use tauri::AppHandle;
    use tracing::{debug, error, info, warn};

    /// `NSEventMaskFlagsChanged`: 1 << NSEventTypeFlagsChanged(12).
    const FLAGS_CHANGED_MASK: usize = 1 << 12;

    /// `global` stays `nil` until Accessibility is granted (see module docs).
    struct Monitors {
        global: id,
        local: id,
    }
    unsafe impl Send for Monitors {}

    static MONITORS: Mutex<Option<Monitors>> = Mutex::new(None);
    static INSTALLED: AtomicBool = AtomicBool::new(false);
    static BINDINGS: Mutex<Vec<ModifierBinding>> = Mutex::new(Vec::new());
    /// While setup is asking someone to press their key, report what arrives
    /// instead of acting on it.
    static CAPTURING: AtomicBool = AtomicBool::new(false);

    /// Whether adding the global monitor is safe right now.
    fn accessibility_trusted() -> bool {
        match crate::commands::native_permissions::NativePermissionChecker::check_accessibility_permission() {
            Ok(trusted) => trusted,
            Err(e) => {
                warn!("[ModifierKeyMonitor] Could not verify Accessibility permission: {}", e);
                false
            }
        }
    }

    /// SAFETY: must be called on the main thread; the block is a heap copy that
    /// AppKit retains for the lifetime of the monitor.
    unsafe fn add_global_monitor(app: &AppHandle) -> id {
        let app_global = app.clone();
        let global_block = ConcreteBlock::new(move |event: id| {
            on_event(&app_global, event);
        })
        .copy();
        msg_send![
            class!(NSEvent),
            addGlobalMonitorForEventsMatchingMask: FLAGS_CHANGED_MASK
            handler: &*global_block as *const _ as *const c_void
        ]
    }

    fn on_event(app: &AppHandle, event: id) {
        if event == nil {
            return;
        }
        // SAFETY: `event` is a live NSEvent handed to us by AppKit for the
        // duration of the handler; these selectors are plain getters.
        let (key_code, flags): (u16, usize) = unsafe {
            let key_code: u16 = msg_send![event, keyCode];
            let flags: usize = msg_send![event, modifierFlags];
            (key_code, flags)
        };

        // Setup is asking which key to use. Report the press and act on
        // nothing: this is someone choosing a binding, not using one.
        if CAPTURING.load(Ordering::SeqCst) {
            for key in super::ModifierKey::ALL {
                if modifier_edge(key_code, flags, key) == Some(true) {
                    debug!("[ModifierKeyMonitor] Captured {} for binding", key.label());
                    if let Err(e) = tauri::Emitter::emit(
                        app,
                        crate::constants::events::triggers::KEY_CAPTURED,
                        serde_json::json!({ "key": key }),
                    ) {
                        warn!(
                            "[ModifierKeyMonitor] Could not report the captured key: {}",
                            e
                        );
                    }
                }
            }
            return;
        }

        // Copy out the matching triggers, then release the lock before routing.
        let matches: Vec<(bool, super::TriggerMethod, super::TriggerTarget)> = {
            let guard = match BINDINGS.lock() {
                Ok(g) => g,
                Err(poisoned) => poisoned.into_inner(),
            };
            guard
                .iter()
                .filter_map(|(key, method, target)| {
                    modifier_edge(key_code, flags, *key).map(|pressed| (pressed, *method, *target))
                })
                .collect()
        };

        for (pressed, method, target) in matches {
            debug!(
                "[ModifierKeyMonitor] key {} {} -> {:?}/{:?}",
                key_code,
                if pressed { "down" } else { "up" },
                method,
                target
            );
            crate::events::shortcuts::fire_trigger_edge(app, method, target, pressed);
        }
    }

    /// Install (or re-install) the monitors for the given bindings.
    pub fn sync(app: &AppHandle, bindings: Vec<ModifierBinding>) -> Result<(), String> {
        remove(app)?;

        if let Ok(mut guard) = BINDINGS.lock() {
            *guard = bindings.clone();
        }
        if bindings.is_empty() {
            return Ok(());
        }

        let trusted = accessibility_trusted();
        if !trusted {
            info!(
                "[ModifierKeyMonitor] Accessibility not granted yet; installing the local monitor only, so the key works while Juno is focused. ensure_global adds the rest when it lands."
            );
        }
        install(app, trusted)
    }

    /// Put the monitors up. Idempotent; `global` is skipped when untrusted.
    fn install(app: &AppHandle, trusted: bool) -> Result<(), String> {
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
                on_event(&app_local, event);
                event // hand back untouched: never consume the key
            })
            .copy();

            // SAFETY: called on the main thread; the blocks are heap copies
            // AppKit retains for the lifetime of the monitor.
            let (global, local): (id, id) = unsafe {
                let global: id = if trusted {
                    add_global_monitor(&app_for_main)
                } else {
                    nil
                };
                let local: id = msg_send![
                    class!(NSEvent),
                    addLocalMonitorForEventsMatchingMask: FLAGS_CHANGED_MASK
                    handler: &*local_block as *const _ as *const c_void
                ];
                (global, local)
            };

            if global == nil && local == nil {
                error!("[ModifierKeyMonitor] AppKit refused both NSEvent monitors; the key will not fire");
                return;
            }

            *guard = Some(Monitors { global, local });
            INSTALLED.store(true, Ordering::SeqCst);
            info!(
                "[ModifierKeyMonitor] Installed (global={}, local={})",
                global != nil,
                local != nil
            );
        })
        .map_err(|e| {
            format!(
                "Failed to dispatch modifier-key monitor install to main thread: {}",
                e
            )
        })
    }

    /// Listen for a bare modifier so setup can ask someone to press theirs.
    ///
    /// The monitors go up even with no binding configured, because the whole
    /// point is to find out whether this keyboard has the key at all. The
    /// local half is enough: setup's own window is focused while it asks, and
    /// Accessibility may well not be granted yet.
    pub fn set_capture(app: &AppHandle, active: bool) -> Result<(), String> {
        CAPTURING.store(active, Ordering::SeqCst);
        if active {
            return install(app, accessibility_trusted());
        }
        // Keep the monitors only if a real binding still needs them.
        let still_bound = match BINDINGS.lock() {
            Ok(g) => !g.is_empty(),
            Err(poisoned) => !poisoned.into_inner().is_empty(),
        };
        if still_bound {
            Ok(())
        } else {
            remove(app)
        }
    }

    /// Add the global half once Accessibility lands, without disturbing the
    /// local one. Called by the permissions poller on the grant.
    pub fn ensure_global(app: &AppHandle) -> Result<(), String> {
        if !INSTALLED.load(Ordering::SeqCst) || !accessibility_trusted() {
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
            // SAFETY: main thread, and the block is a heap copy AppKit retains.
            let global: id = unsafe { add_global_monitor(&app_for_main) };
            if global == nil {
                warn!("[ModifierKeyMonitor] AppKit still refused the global monitor");
                return;
            }
            monitors.global = global;
            info!("[ModifierKeyMonitor] Global monitor added after Accessibility was granted");
        })
        .map_err(|e| {
            format!(
                "Failed to dispatch modifier-key global monitor to main thread: {}",
                e
            )
        })
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
                // SAFETY: main thread; tokens came from addGlobal/LocalMonitor.
                unsafe {
                    if monitors.global != nil {
                        let _: () = msg_send![class!(NSEvent), removeMonitor: monitors.global];
                    }
                    if monitors.local != nil {
                        let _: () = msg_send![class!(NSEvent), removeMonitor: monitors.local];
                    }
                }
                info!("[ModifierKeyMonitor] Removed");
            }
        })
        .map_err(|e| {
            format!(
                "Failed to dispatch modifier-key monitor removal to main thread: {}",
                e
            )
        })
    }
}

#[cfg(not(target_os = "macos"))]
mod imp {
    use super::ModifierBinding;
    use tauri::AppHandle;

    pub fn sync(_app: &AppHandle, _bindings: Vec<ModifierBinding>) -> Result<(), String> {
        Ok(())
    }

    pub fn ensure_global(_app: &AppHandle) -> Result<(), String> {
        Ok(())
    }

    pub fn set_capture(_app: &AppHandle, _active: bool) -> Result<(), String> {
        Ok(())
    }

    pub fn remove(_app: &AppHandle) -> Result<(), String> {
        Ok(())
    }
}

/// Install or refresh the monitor for the given bindings. An empty list tears
/// it down.
pub fn sync(app: &AppHandle, bindings: Vec<ModifierBinding>) -> Result<(), String> {
    imp::sync(app, bindings)
}

/// Add the global half once Accessibility is granted.
pub fn ensure_global(app: &AppHandle) -> Result<(), String> {
    imp::ensure_global(app)
}

/// Listen for a bare modifier key and report it instead of acting on it, so
/// setup can ask someone to press theirs rather than guess at their hardware.
pub fn set_capture(app: &AppHandle, active: bool) -> Result<(), String> {
    imp::set_capture(app, active)
}

/// Tear the monitor down.
#[allow(dead_code)]
pub fn remove(app: &AppHandle) -> Result<(), String> {
    imp::remove(app)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The bits and key codes below were read off real hardware, not docs.
    const FN_HELD: usize = 0x00800100;
    const FN_RELEASED: usize = 0x00000100;

    #[test]
    fn the_fn_key_reports_press_and_release() {
        assert_eq!(modifier_edge(63, FN_HELD, ModifierKey::Fn), Some(true));
        assert_eq!(modifier_edge(63, FN_RELEASED, ModifierKey::Fn), Some(false));
    }

    #[test]
    fn an_arrow_key_carrying_the_function_bit_is_not_the_fn_key() {
        // AppKit sets the Function bit for arrows, F1-F20, Home, End, Page
        // Up/Down and Delete. Testing the bit alone would start dictation on
        // every arrow press.
        assert_eq!(modifier_edge(126, FN_HELD, ModifierKey::Fn), None);
        assert_eq!(modifier_edge(123, FN_HELD, ModifierKey::Fn), None);
    }

    #[test]
    fn other_modifiers_are_ignored() {
        // Shift is key code 56 and has its own bit; it must not reach a
        // trigger bound to Fn.
        assert_eq!(modifier_edge(56, 0x00020102, ModifierKey::Fn), None);
    }
}

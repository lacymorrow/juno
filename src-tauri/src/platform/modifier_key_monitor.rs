//! # Passive bare-modifier monitor (Fn, Fn with Control, Control, Right Option)
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
//! Fn and Control held together is read out of the same flag word: Control is
//! key code 59 (left) or 62 (right) with bit `1 << 18`, and AppKit keeps the
//! Function bit set in the flags of Control's own event while Fn is down. The
//! chord is down while both bits are set and up the moment either clears, which
//! is what a hold needs. See [`ModifierState`] for how the chord and the plain
//! globe key share a finger without both firing.
//!
//! Control and Right Option (key code 61, told apart from the left one) are
//! the same kind of hold for a keyboard with no Fn key. Both are ingredients of
//! ordinary shortcuts, so they only count as a hold after
//! [`MODIFIER_HOLD_DELAY_MS`], and any other key going down cancels them: that
//! is what stops Control+C from opening the microphone. See [`ModifierState`].
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

use crate::triggers::ModifierKey;
use tauri::AppHandle;

/// The Function key's virtual key code on `NSEventTypeFlagsChanged`.
const FN_KEY_CODE: u16 = 63;
/// `NSEventModifierFlagFunction`.
const FN_FLAG: usize = 1 << 23;
/// Left and right Control's virtual key codes.
const CONTROL_KEY_CODES: [u16; 2] = [59, 62];
/// `NSEventModifierFlagControl`.
const CONTROL_FLAG: usize = 1 << 18;

/// Which edge a `flagsChanged` event represents for the plain Fn key, if any.
///
/// `Some(true)` is press, `Some(false)` is release, `None` means the event was
/// about some other key.
///
/// The key code has to be checked, not just the flag bit: AppKit sets the
/// Function bit on arrow keys, F1 through F20, Home, End, Page Up, Page Down
/// and Delete as well, so testing the bit alone would fire the microphone on
/// every arrow press.
pub fn modifier_edge(key_code: u16, modifier_flags: usize) -> Option<bool> {
    if key_code != FN_KEY_CODE {
        return None;
    }
    Some(modifier_flags & FN_FLAG != 0)
}

/// The bare modifier this event is the *press* edge of, if any.
///
/// Capture mode reports presses and ignores releases: someone choosing a key
/// has chosen it the moment they push it, and reporting the release too would
/// hand setup the same answer twice. Only the plain globe key is ever
/// captured; a chord is picked afterwards.
pub fn captured_key(key_code: u16, modifier_flags: usize) -> Option<ModifierKey> {
    (modifier_edge(key_code, modifier_flags) == Some(true)).then_some(ModifierKey::Fn)
}

/// How long a bare modifier must stay down before it counts as a hold.
///
/// A chord is two keys that never land in the same instant, so Fn then Control
/// is Fn alone for a few milliseconds. Starting the agent hold on that first
/// edge flashed the agent for a chord that was never meant for it. A short
/// wait fixes that: if Control joins inside the window only the chord starts,
/// and if Fn comes up inside it the press was a fumble and nothing starts.
///
/// 100 ms is the longest a finger takes to land the second key of a deliberate
/// chord, and about the shortest a person can hold a key on purpose, so a
/// chord never flashes and a hold never feels late. The same window is what
/// keeps a bare Control or Right Option from firing on the front edge of an
/// ordinary Control+C. Only a key that can be the front half of something else
/// pays it: Fn when the chord is bound, and Control and Right Option always.
/// Every other trigger starts on its first edge, as before.
pub const MODIFIER_HOLD_DELAY_MS: u64 = 100;

/// Right Option's virtual key code on `NSEventTypeFlagsChanged`.
const RIGHT_OPTION_KEY_CODE: u16 = 61;
/// `NX_DEVICERALTKEYMASK`: set while the right Option key itself is down. The
/// shared Option bit stays set while the left one is held, so on its own it
/// cannot tell a right press from a right release.
const RIGHT_OPTION_DEVICE_FLAG: usize = 0x40;
/// Right Command, left Command, left Shift, Caps Lock, left Option and right
/// Shift: a press of any of these while a bare hold is still waiting means the
/// key was the front of a shortcut.
const OTHER_MODIFIER_KEY_CODES: [u16; 6] = [54, 55, 56, 57, 58, 60];

/// A bare modifier a trigger is bound to, and what else its key carries.
///
/// Which gestures it carries, and what each one does, is read from the trigger
/// list when the edge arrives. This only records what the state machine needs
/// to know about timing: a key that also has a Tap row must fire on its first
/// edge, because a short press is the whole gesture.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ModifierBinding {
    pub key: ModifierKey,
    /// The same key also carries a Tap row.
    pub tap: bool,
}

impl ModifierBinding {
    #[cfg(test)]
    pub fn hold(key: ModifierKey) -> Self {
        Self { key, tap: false }
    }
}

/// An edge for a bound key: `(key, pressed)`.
pub type Edge = (ModifierKey, bool);

/// What is currently held, so a chord and the plain keys it contains agree on
/// who owns the finger, and so a hold only starts once it has lasted.
///
/// The clock is passed in as `now_ms`, so the whole machine is pure and tests
/// run it on a fake one. Nothing here reads time or sleeps; the caller asks
/// [`ModifierState::poll`] when a deadline passes.
///
/// - A delayed key goes down as *pending* with a deadline. Letting go before
///   the deadline drops it without a trace. [`ModifierState::poll`], or the
///   next event, promotes it to a real hold once the deadline has passed.
/// - With both Hold Fn and Hold Fn+Control bound, pressing Fn then Control is
///   one gesture, the chord. Control joining inside the window means Fn never
///   starts. Control joining after Fn has started releases Fn and starts the
///   chord.
/// - Either way the plain key stays quiet until it is itself released, so
///   letting go of Control while still on Fn does not bring the plain hold
///   back from the dead.
/// - A bare Control or Right Option is cancelled by any other key going down
///   ([`ModifierState::on_key_down`]): pending it never starts, started it is
///   released, and either way it stays quiet until it comes up.
#[derive(Debug, Default)]
pub struct ModifierState {
    down: std::collections::HashSet<ModifierKey>,
    /// Waiting out the delay: key and the time it becomes a hold.
    pending: std::collections::HashMap<ModifierKey, u64>,
    /// Held, but spoken for (by a chord, or cancelled) until it is released.
    yielded: std::collections::HashSet<ModifierKey>,
}

impl ModifierState {
    fn tracked(&self, key: ModifierKey) -> bool {
        self.down.contains(&key) || self.pending.contains_key(&key) || self.yielded.contains(&key)
    }

    /// Whether any key is still waiting out its delay, so the caller knows to
    /// come back and [`poll`](Self::poll).
    pub fn has_pending(&self) -> bool {
        !self.pending.is_empty()
    }

    /// Promote every pending key whose deadline has passed to a hold.
    pub fn poll(&mut self, now_ms: u64) -> Vec<Edge> {
        let mut due: Vec<(u64, ModifierKey)> = self
            .pending
            .iter()
            .filter(|(_, deadline)| **deadline <= now_ms)
            .map(|(key, deadline)| (*deadline, *key))
            .collect();
        due.sort_by_key(|(deadline, key)| (*deadline, key.shortcut()));
        let mut out = Vec::new();
        for (_, key) in due {
            self.pending.remove(&key);
            self.down.insert(key);
            out.push((key, true));
        }
        out
    }

    /// Start a hold now, or after the delay.
    fn begin(&mut self, key: ModifierKey, delayed: bool, now_ms: u64, out: &mut Vec<Edge>) {
        if self.tracked(key) {
            return;
        }
        if delayed {
            self.pending
                .insert(key, now_ms.saturating_add(MODIFIER_HOLD_DELAY_MS));
        } else {
            self.down.insert(key);
            out.push((key, true));
        }
    }

    /// The key came up: forget it, and release it if it had started.
    fn end(&mut self, key: ModifierKey, out: &mut Vec<Edge>) {
        self.yielded.remove(&key);
        self.pending.remove(&key);
        if self.down.remove(&key) {
            out.push((key, false));
        }
    }

    /// Take the finger from `key` without waiting for it to come up.
    fn yield_key(&mut self, key: ModifierKey, out: &mut Vec<Edge>) {
        self.pending.remove(&key);
        if self.down.remove(&key) {
            out.push((key, false));
        }
        self.yielded.insert(key);
    }

    /// Any ordinary key going down. A bare Control or Right Option that is
    /// still down was the front of a shortcut, not a hold.
    pub fn on_key_down(&mut self, now_ms: u64) -> Vec<Edge> {
        let mut out = self.poll(now_ms);
        for key in [ModifierKey::Control, ModifierKey::RightOption] {
            if self.pending.contains_key(&key) || self.down.contains(&key) {
                self.yield_key(key, &mut out);
            }
        }
        out
    }

    /// Fold one `flagsChanged` event in. Returns the edges it produced, in the
    /// order they must be delivered.
    pub fn on_flags_changed(
        &mut self,
        key_code: u16,
        flags: usize,
        bound: &[ModifierBinding],
        now_ms: u64,
    ) -> Vec<Edge> {
        // A deadline that passed before this event arrived has already
        // happened: the hold started, and this event may be what ends it.
        let mut out = self.poll(now_ms);

        let is_bound = |key: ModifierKey| bound.iter().any(|b| b.key == key);
        let has_tap = |key: ModifierKey| bound.iter().any(|b| b.key == key && b.tap);
        let fn_held = flags & FN_FLAG != 0;
        let control_held = flags & CONTROL_FLAG != 0;
        let is_control_key = CONTROL_KEY_CODES.contains(&key_code);

        // Another modifier going down while a bare Control or Right Option is
        // still waiting means it was the front of a shortcut.
        let own_codes_of = |key: ModifierKey| match key {
            ModifierKey::Control => CONTROL_KEY_CODES.to_vec(),
            ModifierKey::RightOption => vec![RIGHT_OPTION_KEY_CODE],
            _ => Vec::new(),
        };
        for key in [ModifierKey::Control, ModifierKey::RightOption] {
            let other = OTHER_MODIFIER_KEY_CODES.contains(&key_code)
                || (key == ModifierKey::Control && key_code == RIGHT_OPTION_KEY_CODE)
                || (key == ModifierKey::RightOption && is_control_key);
            if other && !own_codes_of(key).contains(&key_code) && self.pending.contains_key(&key) {
                self.yield_key(key, &mut out);
            }
        }

        // The chord first, so that it can take the finger from the plain keys
        // within the same event.
        if is_bound(ModifierKey::FnControl) && (key_code == FN_KEY_CODE || is_control_key) {
            let active = fn_held && control_held;
            let was = self.down.contains(&ModifierKey::FnControl);
            if active && !was {
                self.yield_key(ModifierKey::Fn, &mut out);
                self.yield_key(ModifierKey::Control, &mut out);
                self.down.insert(ModifierKey::FnControl);
                out.push((ModifierKey::FnControl, true));
            } else if !active && was {
                self.down.remove(&ModifierKey::FnControl);
                out.push((ModifierKey::FnControl, false));
            }
        }

        if key_code == FN_KEY_CODE {
            if !fn_held {
                // Fn up. Whatever it was doing, a short press that never made
                // it past the window ends here without having started.
                self.end(ModifierKey::Fn, &mut out);
            } else if is_bound(ModifierKey::Fn) {
                let delayed = is_bound(ModifierKey::FnControl) && !has_tap(ModifierKey::Fn);
                self.begin(ModifierKey::Fn, delayed, now_ms, &mut out);
            }
        }

        if is_control_key {
            if !control_held {
                self.end(ModifierKey::Control, &mut out);
            } else if is_bound(ModifierKey::Control) && !fn_held {
                // With Fn down this Control belongs to Fn+Control, bound or
                // not. A bare Control hold is always delayed, so a shortcut
                // that starts with Control has the window to say so.
                self.begin(ModifierKey::Control, true, now_ms, &mut out);
            }
        }

        if key_code == RIGHT_OPTION_KEY_CODE {
            if flags & RIGHT_OPTION_DEVICE_FLAG == 0 {
                self.end(ModifierKey::RightOption, &mut out);
            } else if is_bound(ModifierKey::RightOption) {
                self.begin(ModifierKey::RightOption, true, now_ms, &mut out);
            }
        }

        out
    }
}

/// How long a capture request stands before it lapses on its own.
///
/// Capture swallows the key, so a request that is never withdrawn disables the
/// key for the rest of the run. That is exactly what happened: setup asks for
/// capture on its last screen and the window is destroyed on Done, so the
/// matching `set_capture(false)` never arrived and Fn stopped working
/// everywhere until Juno was quit. The frontend now withdraws it properly, but
/// a swallowed key must not depend on a webview living long enough to say so,
/// hence the lease. Generous enough that nobody reading the screen loses it.
#[cfg(target_os = "macos")]
const CAPTURE_LEASE: std::time::Duration = std::time::Duration::from_secs(120);

#[cfg(target_os = "macos")]
mod imp {
    use super::{
        captured_key, ModifierBinding, ModifierState, CAPTURE_LEASE, MODIFIER_HOLD_DELAY_MS,
    };
    use block::ConcreteBlock;
    use cocoa::base::{id, nil};
    use objc::{class, msg_send, sel, sel_impl};
    use std::ffi::c_void;
    use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
    use std::sync::Mutex;
    use tauri::AppHandle;
    use tracing::{debug, error, info, warn};

    /// `NSEventTypeKeyDown` and `NSEventTypeFlagsChanged`.
    const KEY_DOWN_TYPE: usize = 10;
    const FLAGS_CHANGED_TYPE: usize = 12;
    /// `NSEventMask` for the two: 1 << type. Key downs are watched so a bare
    /// Control or Right Option can tell it was the front of a shortcut.
    const FLAGS_CHANGED_MASK: usize = (1 << FLAGS_CHANGED_TYPE) | (1 << KEY_DOWN_TYPE);

    /// Milliseconds since the first call, the clock the state machine runs on.
    fn now_ms() -> u64 {
        static START: std::sync::OnceLock<std::time::Instant> = std::sync::OnceLock::new();
        START
            .get_or_init(std::time::Instant::now)
            .elapsed()
            .as_millis() as u64
    }

    /// `global` stays `nil` until Accessibility is granted (see module docs).
    struct Monitors {
        global: id,
        local: id,
    }
    unsafe impl Send for Monitors {}

    static MONITORS: Mutex<Option<Monitors>> = Mutex::new(None);
    static INSTALLED: AtomicBool = AtomicBool::new(false);
    static BINDINGS: Mutex<Vec<ModifierBinding>> = Mutex::new(Vec::new());
    /// What is held right now. Reset whenever the bindings change.
    static HELD: Mutex<Option<ModifierState>> = Mutex::new(None);
    /// While setup is asking someone to press their key, report what arrives
    /// instead of acting on it.
    static CAPTURING: AtomicBool = AtomicBool::new(false);
    /// Bumped by every change to `CAPTURING`, so a lapsing lease can tell its
    /// own request apart from a later one and leave that later one alone.
    static CAPTURE_GENERATION: AtomicU64 = AtomicU64::new(0);

    /// Stop reporting presses and go back to firing triggers.
    ///
    /// Called from every route out of capture: the explicit withdrawal, the
    /// lease lapsing, and a binding being saved. Saving counts because the
    /// person has just chosen their key, which is the whole job capture was
    /// asked to do.
    fn end_capture() {
        CAPTURE_GENERATION.fetch_add(1, Ordering::SeqCst);
        if CAPTURING.swap(false, Ordering::SeqCst) {
            debug!("[ModifierKeyMonitor] Capture ended; presses fire their trigger again");
        }
    }

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
        // Keystrokes Juno itself synthesizes (dictation insertion, agent
        // typing, Cmd+V paste) must never fire a modifier trigger.
        // SAFETY: `event` is a live NSEvent for the duration of the handler.
        if unsafe { crate::platform::synthetic_events::is_juno_synthesized_event(event) } {
            return;
        }
        // SAFETY: `event` is a live NSEvent handed to us by AppKit for the
        // duration of the handler; these selectors are plain getters.
        let (event_type, key_code, flags): (usize, u16, usize) = unsafe {
            let event_type: usize = msg_send![event, type];
            let key_code: u16 = msg_send![event, keyCode];
            let flags: usize = msg_send![event, modifierFlags];
            (event_type, key_code, flags)
        };

        // An ordinary key going down. Setup capture has no use for it, and it
        // only matters to a bare Control or Right Option that is waiting or
        // held: that key was the front of a shortcut, not a hold.
        if event_type == KEY_DOWN_TYPE {
            if CAPTURING.load(Ordering::SeqCst) {
                return;
            }
            let edges = {
                let mut held = match HELD.lock() {
                    Ok(g) => g,
                    Err(poisoned) => poisoned.into_inner(),
                };
                match held.as_mut() {
                    Some(state) => state.on_key_down(now_ms()),
                    None => return,
                }
            };
            deliver(app, edges);
            return;
        }
        if event_type != FLAGS_CHANGED_TYPE {
            return;
        }

        // Setup is asking which key to use. Report the press and act on
        // nothing: this is someone choosing a binding, not using one.
        if CAPTURING.load(Ordering::SeqCst) {
            if let Some(key) = captured_key(key_code, flags) {
                debug!("[ModifierKeyMonitor] Captured {} for binding", key.label());
                // `shortcut` is what a binding is written down as, so the
                // settings window can record this press the same way it
                // records any other key it was asked to listen for. `key` is
                // kept beside it for screens that only need to know which key
                // arrived.
                if let Err(e) = tauri::Emitter::emit(
                    app,
                    crate::constants::events::triggers::KEY_CAPTURED,
                    serde_json::json!({ "key": key, "shortcut": key.shortcut() }),
                ) {
                    warn!(
                        "[ModifierKeyMonitor] Could not report the captured key: {}",
                        e
                    );
                }
            }
            return;
        }

        // Which bound keys this event is an edge of. Read and release the locks
        // before routing, because routing reads the trigger list.
        let bound: Vec<ModifierBinding> = match BINDINGS.lock() {
            Ok(g) => g.clone(),
            Err(poisoned) => poisoned.into_inner().clone(),
        };
        let (edges, waiting) = {
            let mut held = match HELD.lock() {
                Ok(g) => g,
                Err(poisoned) => poisoned.into_inner(),
            };
            let state = held.get_or_insert_with(ModifierState::default);
            let edges = state.on_flags_changed(key_code, flags, &bound, now_ms());
            (edges, state.has_pending())
        };
        deliver(app, edges);
        if waiting {
            schedule_poll(app);
        }
    }

    /// Hand each edge to the gesture recognizer.
    fn deliver(app: &AppHandle, edges: Vec<(crate::triggers::ModifierKey, bool)>) {
        for (key, pressed) in edges {
            debug!(
                "[ModifierKeyMonitor] {} {}",
                key.label(),
                if pressed { "down" } else { "up" }
            );
            crate::events::shortcuts::fire_key_edge(
                app,
                &crate::triggers::Binding::Keyboard {
                    shortcut: key.shortcut().to_string(),
                },
                pressed,
            );
        }
    }

    /// Come back when the hold delay is up and start whatever outlasted it.
    ///
    /// Nothing else would: a key held still sends no further events, so the
    /// hold would never start. A key that was let go in the window is already
    /// gone from the state, so a late wake finds nothing to do.
    fn schedule_poll(app: &AppHandle) {
        let app = app.clone();
        tauri::async_runtime::spawn(async move {
            tokio::time::sleep(std::time::Duration::from_millis(MODIFIER_HOLD_DELAY_MS)).await;
            let edges = {
                let mut held = match HELD.lock() {
                    Ok(g) => g,
                    Err(poisoned) => poisoned.into_inner(),
                };
                match held.as_mut() {
                    Some(state) => state.poll(now_ms()),
                    None => return,
                }
            };
            deliver(&app, edges);
        });
    }

    /// Install (or re-install) the monitors for the given bindings.
    pub fn sync(app: &AppHandle, bindings: Vec<ModifierBinding>) -> Result<(), String> {
        // A binding has just been written, so whoever was choosing a key has
        // finished choosing. Ending capture here means a lost "stop capturing"
        // cannot leave the key swallowed all the way to the next launch.
        end_capture();

        remove(app)?;

        if let Ok(mut guard) = BINDINGS.lock() {
            *guard = bindings.clone();
        }
        // Whatever was held belonged to the old bindings.
        if let Ok(mut held) = HELD.lock() {
            *held = None;
        }
        if bindings.is_empty() {
            return Ok(());
        }

        let trusted = accessibility_trusted();
        if !trusted {
            // Worth a warning, not a note: this is the state a "the Fn key does
            // nothing" report is usually in. The local monitor only sees events
            // delivered to Juno, and the bar does not take keyboard focus, so
            // in practice the key does nothing until Accessibility is granted.
            warn!(
                "[ModifierKeyMonitor] Accessibility is NOT granted, so only the local monitor is installed and the key fires solely while a Juno window is focused. Grant Accessibility in System Settings > Privacy & Security; ensure_global completes the monitor the moment it lands."
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
        if !active {
            return withdraw_capture(app);
        }

        let generation = CAPTURE_GENERATION.fetch_add(1, Ordering::SeqCst) + 1;
        CAPTURING.store(true, Ordering::SeqCst);
        // The lease. See CAPTURE_LEASE: the key stays swallowed until this
        // request is withdrawn, and the window that asked can be destroyed
        // before it manages to withdraw anything.
        let app_for_lease = app.clone();
        tauri::async_runtime::spawn(async move {
            tokio::time::sleep(CAPTURE_LEASE).await;
            // A newer request, or any other end to this one, moved the
            // generation on. Leave that one alone.
            if CAPTURE_GENERATION.load(Ordering::SeqCst) != generation {
                return;
            }
            warn!(
                "[ModifierKeyMonitor] Capture was never withdrawn; letting it lapse so the key fires its trigger again"
            );
            if let Err(e) = withdraw_capture(&app_for_lease) {
                warn!("[ModifierKeyMonitor] Could not lapse the capture: {}", e);
            }
        });
        install(app, accessibility_trusted())
    }

    /// End capture and take the monitors back down if nothing is bound to them.
    fn withdraw_capture(app: &AppHandle) -> Result<(), String> {
        end_capture();
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
    /// Control's device-independent bit (1 << 18) on top of the base flags.
    const CONTROL_HELD: usize = 0x00040100;
    const FN_AND_CONTROL_HELD: usize = FN_HELD | 0x00040000;

    const FN: u16 = 63;
    const LEFT_CONTROL: u16 = 59;
    const RIGHT_CONTROL: u16 = 62;

    #[test]
    fn the_fn_key_reports_press_and_release() {
        assert_eq!(modifier_edge(63, FN_HELD), Some(true));
        assert_eq!(modifier_edge(63, FN_RELEASED), Some(false));
    }

    #[test]
    fn an_arrow_key_carrying_the_function_bit_is_not_the_fn_key() {
        // AppKit sets the Function bit for arrows, F1-F20, Home, End, Page
        // Up/Down and Delete. Testing the bit alone would start dictation on
        // every arrow press.
        assert_eq!(modifier_edge(126, FN_HELD), None);
        assert_eq!(modifier_edge(123, FN_HELD), None);
    }

    #[test]
    fn first_fn_down_on_a_fresh_state_is_a_press_edge() {
        // Nothing is armed lazily: the very first Fn down after launch must
        // produce its press edge, not the second one.
        let mut state = ModifierState::default();
        let bound = [ModifierBinding::hold(ModifierKey::Fn)];
        assert_eq!(
            state.on_flags_changed(FN_KEY_CODE, FN_HELD, &bound, 0),
            vec![(ModifierKey::Fn, true)]
        );
    }

    #[test]
    fn first_fn_down_is_captured_without_any_prior_event() {
        assert_eq!(captured_key(FN_KEY_CODE, FN_HELD), Some(ModifierKey::Fn));
    }

    #[test]
    fn other_modifiers_are_ignored() {
        // Shift is key code 56 and has its own bit; it must not reach a
        // trigger bound to Fn.
        assert_eq!(modifier_edge(56, 0x00020102), None);
    }

    #[test]
    fn capture_reports_the_fn_press_only() {
        assert_eq!(captured_key(63, FN_HELD), Some(ModifierKey::Fn));
        // The release edge is the same key arriving again; reporting it would
        // answer setup's question twice for one press.
        assert_eq!(captured_key(63, FN_RELEASED), None);
    }

    #[test]
    fn capture_ignores_keys_that_merely_carry_the_function_bit() {
        // Left arrow and F5, both of which set 1 << 23 on macOS.
        assert_eq!(captured_key(123, FN_HELD), None);
        assert_eq!(captured_key(96, FN_HELD), None);
        // And an ordinary modifier, which has its own bit entirely.
        assert_eq!(captured_key(56, 0x00020102), None);
        // Control alone is not the globe key either.
        assert_eq!(captured_key(LEFT_CONTROL, CONTROL_HELD), None);
    }

    /* ------------------------------------------------------------ */
    /* The state machine, on a fake clock                            */
    /* ------------------------------------------------------------ */

    const D: u64 = MODIFIER_HOLD_DELAY_MS;
    const LEFT_OPTION: u16 = 58;
    const RIGHT_OPTION: u16 = 61;
    const SHIFT: u16 = 56;
    /// Option's shared bit (1 << 19) with the device bit for the right key.
    const RIGHT_OPTION_HELD: usize = 0x00080140;
    /// The same bit with the left key's device bit.
    const LEFT_OPTION_HELD: usize = 0x00080120;
    /// Both Option keys down: shared bit and both device bits.
    const BOTH_OPTIONS_HELD: usize = 0x00080160;
    const NOTHING_HELD: usize = 0x00000100;

    fn hold(keys: &[ModifierKey]) -> Vec<ModifierBinding> {
        keys.iter().map(|k| ModifierBinding::hold(*k)).collect()
    }

    fn both() -> Vec<ModifierBinding> {
        hold(&[ModifierKey::Fn, ModifierKey::FnControl])
    }

    #[test]
    fn the_window_is_a_tenth_of_a_second() {
        // Named, so the number is argued about in one place. Long enough for a
        // chord's second key to land, short enough that a hold feels instant.
        assert_eq!(MODIFIER_HOLD_DELAY_MS, 100);
    }

    #[test]
    fn the_chord_is_down_only_while_both_keys_are_held() {
        let mut st = ModifierState::default();
        let bound = hold(&[ModifierKey::FnControl]);

        // Fn alone is not the chord.
        assert!(st.on_flags_changed(FN, FN_HELD, &bound, 0).is_empty());
        // Control joins: the chord goes down.
        assert_eq!(
            st.on_flags_changed(LEFT_CONTROL, FN_AND_CONTROL_HELD, &bound, 10),
            vec![(ModifierKey::FnControl, true)]
        );
        // Control up while Fn stays: the chord is over.
        assert_eq!(
            st.on_flags_changed(LEFT_CONTROL, FN_HELD, &bound, 500),
            vec![(ModifierKey::FnControl, false)]
        );
        // Fn up afterwards says nothing more.
        assert!(st.on_flags_changed(FN, FN_RELEASED, &bound, 600).is_empty());
    }

    #[test]
    fn the_chord_works_pressed_in_either_order_and_released_in_either_order() {
        for (first, second) in [(FN, LEFT_CONTROL), (LEFT_CONTROL, FN)] {
            let mut st = ModifierState::default();
            let bound = hold(&[ModifierKey::FnControl]);
            let after_first = if first == FN { FN_HELD } else { CONTROL_HELD };
            assert!(st
                .on_flags_changed(first, after_first, &bound, 0)
                .is_empty());
            assert_eq!(
                st.on_flags_changed(second, FN_AND_CONTROL_HELD, &bound, 10),
                vec![(ModifierKey::FnControl, true)],
                "pressed {first} then {second}"
            );
            // Release whichever came first.
            let remaining = if first == FN { CONTROL_HELD } else { FN_HELD };
            assert_eq!(
                st.on_flags_changed(first, remaining, &bound, 400),
                vec![(ModifierKey::FnControl, false)],
                "released {first} first"
            );
            assert!(st
                .on_flags_changed(second, FN_RELEASED, &bound, 410)
                .is_empty());
        }
    }

    #[test]
    fn the_right_control_key_makes_the_chord_too() {
        let mut st = ModifierState::default();
        let bound = hold(&[ModifierKey::FnControl]);
        st.on_flags_changed(FN, FN_HELD, &bound, 0);
        assert_eq!(
            st.on_flags_changed(RIGHT_CONTROL, FN_AND_CONTROL_HELD, &bound, 10),
            vec![(ModifierKey::FnControl, true)]
        );
    }

    #[test]
    fn control_alone_and_an_arrow_with_the_function_bit_never_fire_the_chord() {
        let mut st = ModifierState::default();
        let bound = hold(&[ModifierKey::FnControl]);
        assert!(st
            .on_flags_changed(LEFT_CONTROL, CONTROL_HELD, &bound, 0)
            .is_empty());
        assert!(st
            .on_flags_changed(LEFT_CONTROL, FN_RELEASED, &bound, 10)
            .is_empty());
        // An arrow arrives with the Function bit and, here, Control held.
        assert!(st
            .on_flags_changed(126, FN_AND_CONTROL_HELD, &bound, 20)
            .is_empty());
    }

    #[test]
    fn no_release_edge_arrives_for_a_chord_that_never_went_down() {
        // A stray Control release must not end a hold that never started.
        let mut st = ModifierState::default();
        let bound = hold(&[ModifierKey::FnControl]);
        assert!(st
            .on_flags_changed(LEFT_CONTROL, FN_RELEASED, &bound, 0)
            .is_empty());
        assert!(st.on_flags_changed(FN, FN_RELEASED, &bound, 10).is_empty());
    }

    /* -- Fn waits out the window when a chord is bound ----------- */

    #[test]
    fn fn_then_control_inside_the_window_starts_the_chord_and_never_the_plain_hold() {
        let mut st = ModifierState::default();
        assert!(st.on_flags_changed(FN, FN_HELD, &both(), 0).is_empty());
        assert!(st.has_pending(), "Fn is waiting, not started");
        assert_eq!(
            st.on_flags_changed(LEFT_CONTROL, FN_AND_CONTROL_HELD, &both(), D - 1),
            vec![(ModifierKey::FnControl, true)],
            "no flash of the plain hold"
        );
        assert!(!st.has_pending(), "the chord took Fn's pending start");
        assert!(
            st.poll(10 * D).is_empty(),
            "the deadline passes and Fn still does not start"
        );
        assert_eq!(
            st.on_flags_changed(LEFT_CONTROL, FN_HELD, &both(), 500),
            vec![(ModifierKey::FnControl, false)],
            "Control up ends the chord and does not resurrect the plain hold"
        );
        assert!(st
            .on_flags_changed(FN, FN_RELEASED, &both(), 510)
            .is_empty());
    }

    #[test]
    fn control_then_fn_is_the_chord_and_the_plain_key_never_starts() {
        let mut st = ModifierState::default();
        assert!(st
            .on_flags_changed(LEFT_CONTROL, CONTROL_HELD, &both(), 0)
            .is_empty());
        assert_eq!(
            st.on_flags_changed(FN, FN_AND_CONTROL_HELD, &both(), 30),
            vec![(ModifierKey::FnControl, true)],
            "no flash of the plain hold"
        );
        assert!(st.poll(10 * D).is_empty());
        assert_eq!(
            st.on_flags_changed(FN, CONTROL_HELD, &both(), 400),
            vec![(ModifierKey::FnControl, false)]
        );
        assert!(st
            .on_flags_changed(LEFT_CONTROL, FN_RELEASED, &both(), 410)
            .is_empty());
    }

    #[test]
    fn fn_alone_past_the_window_starts_the_plain_hold() {
        let mut st = ModifierState::default();
        assert!(st.on_flags_changed(FN, FN_HELD, &both(), 0).is_empty());
        assert!(st.poll(D - 1).is_empty(), "not before the window is up");
        assert_eq!(st.poll(D), vec![(ModifierKey::Fn, true)]);
        assert!(!st.has_pending());
        assert_eq!(
            st.on_flags_changed(FN, FN_RELEASED, &both(), 900),
            vec![(ModifierKey::Fn, false)]
        );
    }

    #[test]
    fn fn_let_go_inside_the_window_is_a_fumble_and_starts_nothing() {
        let mut st = ModifierState::default();
        assert!(st.on_flags_changed(FN, FN_HELD, &both(), 0).is_empty());
        assert!(
            st.on_flags_changed(FN, FN_RELEASED, &both(), D - 1)
                .is_empty(),
            "neither a down nor an up edge"
        );
        assert!(st.poll(10 * D).is_empty());
        // And the next press starts the wait afresh.
        assert!(st.on_flags_changed(FN, FN_HELD, &both(), 1000).is_empty());
        assert_eq!(st.poll(1000 + D), vec![(ModifierKey::Fn, true)]);
    }

    #[test]
    fn a_release_that_beats_the_timer_still_sees_the_hold_start_first() {
        // The wake-up can run late. An event that arrives after the deadline
        // has to find the hold already started, or the release would be
        // dropped as a fumble and the microphone would never close.
        let mut st = ModifierState::default();
        st.on_flags_changed(FN, FN_HELD, &both(), 0);
        assert_eq!(
            st.on_flags_changed(FN, FN_RELEASED, &both(), D + 20),
            vec![(ModifierKey::Fn, true), (ModifierKey::Fn, false)]
        );
    }

    #[test]
    fn a_quick_press_leaves_nothing_held_for_the_next_hold() {
        // The reported sequence with the default bindings (Hold Fn, Hold
        // Fn+Control): a ~150ms press, then an ordinary hold of each.
        let mut st = ModifierState::default();
        st.on_flags_changed(FN, FN_HELD, &both(), 0);
        assert_eq!(
            st.on_flags_changed(FN, FN_RELEASED, &both(), 150),
            vec![(ModifierKey::Fn, true), (ModifierKey::Fn, false)],
            "the down edge is always followed by its up edge, in that order"
        );
        assert!(!st.has_pending());
        assert!(st.poll(10 * D).is_empty(), "nothing is left waiting");

        // A real hold of Fn starts and ends normally.
        st.on_flags_changed(FN, FN_HELD, &both(), 1000);
        assert_eq!(st.poll(1000 + D), vec![(ModifierKey::Fn, true)]);
        assert_eq!(
            st.on_flags_changed(FN, FN_RELEASED, &both(), 1800),
            vec![(ModifierKey::Fn, false)]
        );

        // And so does a hold of the chord.
        st.on_flags_changed(FN, FN_HELD, &both(), 2000);
        assert_eq!(
            st.on_flags_changed(LEFT_CONTROL, FN_AND_CONTROL_HELD, &both(), 2020),
            vec![(ModifierKey::FnControl, true)]
        );
        assert_eq!(
            st.on_flags_changed(LEFT_CONTROL, FN_HELD, &both(), 2600),
            vec![(ModifierKey::FnControl, false)]
        );
        assert!(st
            .on_flags_changed(FN, FN_RELEASED, &both(), 2610)
            .is_empty());
    }

    #[test]
    fn control_joining_after_the_window_hands_the_finger_from_the_plain_key_to_the_chord() {
        let mut st = ModifierState::default();
        st.on_flags_changed(FN, FN_HELD, &both(), 0);
        assert_eq!(st.poll(D), vec![(ModifierKey::Fn, true)]);
        assert_eq!(
            st.on_flags_changed(LEFT_CONTROL, FN_AND_CONTROL_HELD, &both(), 300),
            vec![(ModifierKey::Fn, false), (ModifierKey::FnControl, true)],
            "Control joins late: plain Fn is let go, the chord starts"
        );
        assert_eq!(
            st.on_flags_changed(LEFT_CONTROL, FN_HELD, &both(), 600),
            vec![(ModifierKey::FnControl, false)]
        );
        assert!(
            st.poll(5000).is_empty(),
            "Fn is still down but belongs to the chord until it is released"
        );
        assert!(st
            .on_flags_changed(FN, FN_RELEASED, &both(), 700)
            .is_empty());
        // And the plain key works again on the next press.
        assert!(st.on_flags_changed(FN, FN_HELD, &both(), 800).is_empty());
        assert_eq!(st.poll(800 + D), vec![(ModifierKey::Fn, true)]);
    }

    #[test]
    fn repeated_flag_events_do_not_repeat_an_edge_or_move_the_deadline() {
        let mut st = ModifierState::default();
        assert!(st.on_flags_changed(FN, FN_HELD, &both(), 0).is_empty());
        assert!(st.on_flags_changed(FN, FN_HELD, &both(), 60).is_empty());
        assert_eq!(
            st.poll(D),
            vec![(ModifierKey::Fn, true)],
            "the first press's deadline stands"
        );
        assert!(st.on_flags_changed(FN, FN_HELD, &both(), 200).is_empty());
    }

    #[test]
    fn plain_fn_starts_at_once_when_no_chord_is_bound() {
        // The window is for a chord to land in. With no chord bound there is
        // nothing to wait for, and Fn's latency is untouched.
        let mut st = ModifierState::default();
        let bound = hold(&[ModifierKey::Fn]);
        assert_eq!(
            st.on_flags_changed(FN, FN_HELD, &bound, 0),
            vec![(ModifierKey::Fn, true)]
        );
        assert!(!st.has_pending());
        // Control while Fn is held means nothing to a plain-Fn binding.
        assert!(st
            .on_flags_changed(LEFT_CONTROL, FN_AND_CONTROL_HELD, &bound, 10)
            .is_empty());
        assert_eq!(
            st.on_flags_changed(FN, CONTROL_HELD, &bound, 20),
            vec![(ModifierKey::Fn, false)]
        );
    }

    #[test]
    fn fn_that_also_carries_a_tap_is_not_made_to_wait() {
        // A short press is the whole of a tap, so the fumble rule would eat it.
        let mut st = ModifierState::default();
        let bound = vec![
            ModifierBinding {
                key: ModifierKey::Fn,
                tap: true,
            },
            ModifierBinding::hold(ModifierKey::FnControl),
        ];
        assert_eq!(
            st.on_flags_changed(FN, FN_HELD, &bound, 0),
            vec![(ModifierKey::Fn, true)]
        );
        assert_eq!(
            st.on_flags_changed(FN, FN_RELEASED, &bound, 30),
            vec![(ModifierKey::Fn, false)]
        );
    }

    #[test]
    fn the_chord_itself_starts_without_waiting() {
        // Only the front half of a chord waits. The chord is complete the
        // moment its second key lands.
        let mut st = ModifierState::default();
        let bound = hold(&[ModifierKey::FnControl]);
        st.on_flags_changed(LEFT_CONTROL, CONTROL_HELD, &bound, 0);
        assert_eq!(
            st.on_flags_changed(FN, FN_AND_CONTROL_HELD, &bound, 1),
            vec![(ModifierKey::FnControl, true)]
        );
    }

    /* -- Bare Control ------------------------------------------- */

    #[test]
    fn a_bare_control_hold_starts_after_the_window_and_ends_on_release() {
        let mut st = ModifierState::default();
        let bound = hold(&[ModifierKey::Control]);
        assert!(st
            .on_flags_changed(LEFT_CONTROL, CONTROL_HELD, &bound, 0)
            .is_empty());
        assert_eq!(st.poll(D), vec![(ModifierKey::Control, true)]);
        assert_eq!(
            st.on_flags_changed(LEFT_CONTROL, NOTHING_HELD, &bound, 700),
            vec![(ModifierKey::Control, false)]
        );
    }

    #[test]
    fn the_right_control_key_is_a_bare_control_hold_too() {
        let mut st = ModifierState::default();
        let bound = hold(&[ModifierKey::Control]);
        st.on_flags_changed(RIGHT_CONTROL, CONTROL_HELD, &bound, 0);
        assert_eq!(st.poll(D), vec![(ModifierKey::Control, true)]);
    }

    #[test]
    fn a_key_pressed_while_control_is_waiting_cancels_it() {
        // Control+C: the C lands inside the window, so Control was the front
        // of a shortcut and the microphone never opens.
        let mut st = ModifierState::default();
        let bound = hold(&[ModifierKey::Control]);
        st.on_flags_changed(LEFT_CONTROL, CONTROL_HELD, &bound, 0);
        assert!(st.on_key_down(D / 2).is_empty());
        assert!(st.poll(10 * D).is_empty(), "never starts, even held on");
        assert!(st
            .on_flags_changed(LEFT_CONTROL, NOTHING_HELD, &bound, 800)
            .is_empty());
        // The next clean press is a hold again.
        st.on_flags_changed(LEFT_CONTROL, CONTROL_HELD, &bound, 900);
        assert_eq!(st.poll(900 + D), vec![(ModifierKey::Control, true)]);
    }

    #[test]
    fn a_key_pressed_while_control_is_held_ends_the_hold_and_it_stays_ended() {
        // Control held past the window, then Control+Tab: the hold is let go
        // rather than left holding the microphone through a shortcut.
        let mut st = ModifierState::default();
        let bound = hold(&[ModifierKey::Control]);
        st.on_flags_changed(LEFT_CONTROL, CONTROL_HELD, &bound, 0);
        assert_eq!(st.poll(D), vec![(ModifierKey::Control, true)]);
        assert_eq!(st.on_key_down(400), vec![(ModifierKey::Control, false)]);
        assert!(st.poll(5000).is_empty());
        assert!(
            st.on_flags_changed(LEFT_CONTROL, NOTHING_HELD, &bound, 900)
                .is_empty(),
            "the release has nothing left to end"
        );
    }

    #[test]
    fn a_whole_control_shortcut_inside_the_window_fires_nothing() {
        let mut st = ModifierState::default();
        let bound = hold(&[ModifierKey::Control]);
        assert!(st
            .on_flags_changed(LEFT_CONTROL, CONTROL_HELD, &bound, 0)
            .is_empty());
        assert!(st.on_key_down(20).is_empty());
        assert!(st
            .on_flags_changed(LEFT_CONTROL, NOTHING_HELD, &bound, 60)
            .is_empty());
        assert!(st.poll(10 * D).is_empty());
    }

    #[test]
    fn another_modifier_pressed_while_control_is_waiting_cancels_it() {
        // Control+Shift+...: the shortcut is on its way.
        let mut st = ModifierState::default();
        let bound = hold(&[ModifierKey::Control]);
        st.on_flags_changed(LEFT_CONTROL, CONTROL_HELD, &bound, 0);
        assert!(st
            .on_flags_changed(SHIFT, CONTROL_HELD | 0x20000, &bound, 30)
            .is_empty());
        assert!(st.poll(10 * D).is_empty());
    }

    #[test]
    fn a_key_down_with_nothing_held_or_waiting_does_nothing() {
        let mut st = ModifierState::default();
        assert!(st.on_key_down(0).is_empty());
    }

    #[test]
    fn a_key_down_does_not_end_a_plain_fn_hold() {
        // Fn already carries arrows, Home and End on a laptop, and the owner's
        // rule is about Control and Right Option, so Fn is left alone.
        let mut st = ModifierState::default();
        let bound = hold(&[ModifierKey::Fn]);
        st.on_flags_changed(FN, FN_HELD, &bound, 0);
        assert!(st.on_key_down(300).is_empty());
        assert_eq!(
            st.on_flags_changed(FN, FN_RELEASED, &bound, 400),
            vec![(ModifierKey::Fn, false)]
        );
    }

    /* -- Control and the chord share a finger ------------------- */

    #[test]
    fn hold_control_and_hold_fn_control_coexist_and_the_chord_wins_either_way() {
        let bound = hold(&[
            ModifierKey::Fn,
            ModifierKey::FnControl,
            ModifierKey::Control,
        ]);

        // Control first, then Fn inside the window: only the chord.
        let mut st = ModifierState::default();
        assert!(st
            .on_flags_changed(LEFT_CONTROL, CONTROL_HELD, &bound, 0)
            .is_empty());
        assert_eq!(
            st.on_flags_changed(FN, FN_AND_CONTROL_HELD, &bound, 40),
            vec![(ModifierKey::FnControl, true)]
        );
        assert!(st.poll(10 * D).is_empty(), "neither half starts alone");

        // Fn first, then Control inside the window: only the chord.
        let mut st = ModifierState::default();
        assert!(st.on_flags_changed(FN, FN_HELD, &bound, 0).is_empty());
        assert_eq!(
            st.on_flags_changed(LEFT_CONTROL, FN_AND_CONTROL_HELD, &bound, 40),
            vec![(ModifierKey::FnControl, true)]
        );
        assert!(st.poll(10 * D).is_empty(), "neither half starts alone");
        // Control released first: the chord ends and Control stays quiet.
        assert_eq!(
            st.on_flags_changed(LEFT_CONTROL, FN_HELD, &bound, 300),
            vec![(ModifierKey::FnControl, false)]
        );
        assert!(st.poll(5000).is_empty());
    }

    #[test]
    fn control_alone_is_still_control_when_the_chord_is_bound_too() {
        let bound = hold(&[ModifierKey::FnControl, ModifierKey::Control]);
        let mut st = ModifierState::default();
        st.on_flags_changed(LEFT_CONTROL, CONTROL_HELD, &bound, 0);
        assert_eq!(st.poll(D), vec![(ModifierKey::Control, true)]);
        // Fn joining late takes the finger for the chord.
        assert_eq!(
            st.on_flags_changed(FN, FN_AND_CONTROL_HELD, &bound, 400),
            vec![
                (ModifierKey::Control, false),
                (ModifierKey::FnControl, true)
            ]
        );
    }

    #[test]
    fn a_bare_control_does_not_start_while_fn_is_down_even_with_no_chord_bound() {
        // Fn+Control is a combination of its own, bound or not.
        let bound = hold(&[ModifierKey::Control]);
        let mut st = ModifierState::default();
        st.on_flags_changed(FN, FN_HELD, &bound, 0);
        st.on_flags_changed(LEFT_CONTROL, FN_AND_CONTROL_HELD, &bound, 10);
        assert!(st.poll(10 * D).is_empty());
    }

    /* -- Right Option ------------------------------------------- */

    #[test]
    fn a_right_option_hold_starts_after_the_window_and_ends_on_release() {
        let mut st = ModifierState::default();
        let bound = hold(&[ModifierKey::RightOption]);
        assert!(st
            .on_flags_changed(RIGHT_OPTION, RIGHT_OPTION_HELD, &bound, 0)
            .is_empty());
        assert_eq!(st.poll(D), vec![(ModifierKey::RightOption, true)]);
        assert_eq!(
            st.on_flags_changed(RIGHT_OPTION, NOTHING_HELD, &bound, 600),
            vec![(ModifierKey::RightOption, false)]
        );
    }

    #[test]
    fn the_left_option_key_is_not_right_option() {
        let mut st = ModifierState::default();
        let bound = hold(&[ModifierKey::RightOption]);
        assert!(st
            .on_flags_changed(LEFT_OPTION, LEFT_OPTION_HELD, &bound, 0)
            .is_empty());
        assert!(!st.has_pending());
        assert!(st.poll(10 * D).is_empty());
    }

    #[test]
    fn letting_go_of_right_option_while_left_option_is_down_is_a_release() {
        // The shared Option bit is still set after the right key comes up, so
        // the device bit is what says which key moved.
        let mut st = ModifierState::default();
        let bound = hold(&[ModifierKey::RightOption]);
        st.on_flags_changed(LEFT_OPTION, LEFT_OPTION_HELD, &bound, 0);
        st.on_flags_changed(RIGHT_OPTION, BOTH_OPTIONS_HELD, &bound, 10);
        assert_eq!(st.poll(10 + D), vec![(ModifierKey::RightOption, true)]);
        assert_eq!(
            st.on_flags_changed(RIGHT_OPTION, LEFT_OPTION_HELD, &bound, 500),
            vec![(ModifierKey::RightOption, false)]
        );
    }

    #[test]
    fn a_key_pressed_while_right_option_is_waiting_cancels_it() {
        // Option+letter types a symbol; it must not open the microphone.
        let mut st = ModifierState::default();
        let bound = hold(&[ModifierKey::RightOption]);
        st.on_flags_changed(RIGHT_OPTION, RIGHT_OPTION_HELD, &bound, 0);
        assert!(st.on_key_down(30).is_empty());
        assert!(st.poll(10 * D).is_empty());
        assert!(st
            .on_flags_changed(RIGHT_OPTION, NOTHING_HELD, &bound, 500)
            .is_empty());
    }

    #[test]
    fn a_key_pressed_while_right_option_is_held_ends_the_hold() {
        let mut st = ModifierState::default();
        let bound = hold(&[ModifierKey::RightOption]);
        st.on_flags_changed(RIGHT_OPTION, RIGHT_OPTION_HELD, &bound, 0);
        assert_eq!(st.poll(D), vec![(ModifierKey::RightOption, true)]);
        assert_eq!(st.on_key_down(300), vec![(ModifierKey::RightOption, false)]);
        assert!(st
            .on_flags_changed(RIGHT_OPTION, NOTHING_HELD, &bound, 900)
            .is_empty());
    }

    #[test]
    fn control_and_right_option_hold_together_as_the_no_fn_pair() {
        // The defaults for a keyboard with no Fn key: agent on Right Option,
        // dictation on Control. They never share an edge.
        let bound = hold(&[ModifierKey::RightOption, ModifierKey::Control]);
        let mut st = ModifierState::default();
        st.on_flags_changed(RIGHT_OPTION, RIGHT_OPTION_HELD, &bound, 0);
        assert_eq!(st.poll(D), vec![(ModifierKey::RightOption, true)]);
        assert_eq!(
            st.on_flags_changed(RIGHT_OPTION, NOTHING_HELD, &bound, 300),
            vec![(ModifierKey::RightOption, false)]
        );
        st.on_flags_changed(LEFT_CONTROL, CONTROL_HELD, &bound, 400);
        assert_eq!(st.poll(400 + D), vec![(ModifierKey::Control, true)]);
        assert_eq!(
            st.on_flags_changed(LEFT_CONTROL, NOTHING_HELD, &bound, 900),
            vec![(ModifierKey::Control, false)]
        );
    }

    #[test]
    fn an_unbound_key_is_ignored() {
        let mut st = ModifierState::default();
        let bound = hold(&[ModifierKey::Fn]);
        assert!(st
            .on_flags_changed(RIGHT_OPTION, RIGHT_OPTION_HELD, &bound, 0)
            .is_empty());
        assert!(st
            .on_flags_changed(LEFT_CONTROL, CONTROL_HELD, &bound, 10)
            .is_empty());
        assert!(!st.has_pending());
    }
}

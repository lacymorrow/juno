//! # Global Shortcut Handler
//!
//! Every way a key or a mouse button can summon Juno arrives here. The two
//! utility shortcuts (Escape to stop, Cmd+Comma to open settings) are fixed
//! constants; everything else is a [`crate::triggers::Trigger`], and a key is
//! handed to [`recognizer`], which decides which of that key's independent
//! gestures the edge belongs to.

use std::sync::Mutex;
use std::time::Instant;

use tauri::{AppHandle, Emitter, Manager};
use tauri_plugin_global_shortcut::{Shortcut, ShortcutEvent, ShortcutState};
use tracing::{debug, error, info};

use crate::constants::{errors::templates, events, monitor_sessions};
use crate::state;
use crate::triggers::{Binding, KeyGestures, TriggerTarget};

/// Parse a shortcut string into a Shortcut object
pub fn parse_shortcut_string(shortcut_str: &str) -> Option<Shortcut> {
    crate::parse_shortcut_string(shortcut_str)
}

/// Handle global shortcut events
pub fn handle_global_shortcut(app: &AppHandle, shortcut: &Shortcut, event: &ShortcutEvent) {
    debug!(
        "[GlobalShortcut Triggered] Shortcut: {:?}, State: {:?}",
        shortcut,
        event.state()
    );

    // The two utility shortcuts are fixed constants, not settings: Escape
    // cancels and Cmd+Comma opens settings, the way every Mac app does it, so
    // there is nothing to look up and nothing a stored value could override.
    // Activation is driven by the triggers below, and Say is a gesture of its
    // own, so there is no voice-activation shortcut at all.
    let stop_shortcut: Option<Shortcut> =
        parse_shortcut_string(crate::constants::settings::defaults::STOP_CURRENT_TASK);
    let settings_shortcut: Option<Shortcut> =
        parse_shortcut_string(crate::constants::settings::defaults::OPEN_SETTINGS);

    // Handle each shortcut type (use separate conditions to check all shortcuts)
    if let Some(stop_shortcut_obj) = stop_shortcut {
        if *shortcut == stop_shortcut_obj {
            handle_escape_key_shortcut(app, event);
        }
    }

    // Check settings shortcut
    if let Some(settings_shortcut_obj) = settings_shortcut {
        if *shortcut == settings_shortcut_obj {
            handle_settings_shortcut(app, event);
        }
    }

    dispatch_activation_triggers(app, shortcut, event);
}

/// Hand a matching combo to its key's recognizer.
///
/// One key, one call, however many rows describe it. The old loop dispatched
/// per matching row and had to carry two "already fired" flags to stop a single
/// press from activating the same target twice; a key is now resolved once and
/// the recognizer says which gesture it was.
fn dispatch_activation_triggers(app: &AppHandle, shortcut: &Shortcut, event: &ShortcutEvent) {
    let triggers = match app.state::<state::AppState>().get_triggers() {
        Ok(t) => t,
        Err(e) => {
            error!("[GlobalShortcut] Failed to read triggers: {}", e);
            return;
        }
    };

    for binding in crate::triggers::bound_keys(&triggers) {
        let Binding::Keyboard { shortcut: combo } = &binding else {
            continue; // mouse buttons have their own monitor
        };
        // A bare modifier such as Fn is a keyboard binding, but the plugin
        // cannot register it and the flags-changed monitor fires it directly.
        // Skipping it here keeps that one edge from arriving twice if the combo
        // parser ever learns to spell it.
        if crate::triggers::bare_modifier(combo).is_some() {
            continue;
        }
        let Some(parsed) = parse_shortcut_string(combo) else {
            continue;
        };
        if *shortcut != parsed {
            continue;
        }
        // Logged at info, because "the shortcut does nothing" is the report that
        // keeps coming back and this is the line that settles it: the
        // registration log says the combo was claimed, and this one says a press
        // of it arrived and was routed.
        info!("[GlobalShortcut] {} fired ({:?})", combo, event.state());
        fire_key_edge(app, &binding, event.state() == ShortcutState::Pressed);
        return;
    }
}

/// Handle settings shortcut (Cmd+, by default)
fn handle_settings_shortcut(app: &AppHandle, event: &ShortcutEvent) {
    if event.state() == ShortcutState::Pressed {
        info!("[Settings Shortcut] Pressed - opening settings window");
        let app_handle_clone = app.clone();
        tauri::async_runtime::spawn(async move {
            if let Err(e) = crate::window_management::open_settings_window(app_handle_clone).await {
                error!("[Settings Shortcut] Failed to open settings window: {}", e);
            }
        });
    }
}

/// Handle the stop shortcut when it arrives through the global-shortcut
/// plugin (only used when `stop_current_task` is a modified chord).
fn handle_escape_key_shortcut(app: &AppHandle, event: &ShortcutEvent) {
    handle_stop_key_event(app, event.state() == ShortcutState::Pressed);
}

/// Universal "cancel anything" handler for the stop key.
///
/// Reached from two observers that must behave identically:
/// * the passive NSEvent monitor (`platform::stop_key_monitor`) for a bare
///   Escape/function key — the key is never consumed, other apps still see it;
/// * the global-shortcut plugin for a modified chord.
///
/// Always emits the visual-feedback event; only triggers the coordinated stop
/// on a press outside onboarding.
pub fn handle_stop_key_event(app: &AppHandle, pressed: bool) {
    let shortcut_state = if pressed { "pressed" } else { "released" };

    // Always emit visual feedback event (for onboarding UI)
    if let Err(e) = app.emit(
        events::shortcuts::ESCAPE_KEY,
        serde_json::json!({
            "state": shortcut_state,
            "shortcut": "escape_key"
        }),
    ) {
        error!(
            "[Escape Key] Failed to emit shortcut detection event: {}",
            e
        );
    }

    if !pressed {
        return;
    }

    // During onboarding, only provide visual feedback — don't trigger stop.
    let app_state = app.state::<state::AppState>();
    if app_state.is_onboarding_active() {
        info!("[Escape Key] Pressed during onboarding - visual feedback only");
        return;
    }

    let app_handle_clone = app.clone();
    tauri::async_runtime::spawn(async move {
        // When nothing is actually running, Escape has no work to cancel — treat
        // it as "close the chat pane" instead, so the global monitor lets Escape
        // dismiss the pane even when the bar is not focused. The pane arms this
        // monitor only while it is open (see `set_bar_pane_open`).
        if !crate::commands::escape_key_coordinator::something_to_stop(&app_handle_clone).await {
            info!("[Escape Key] Pressed while idle - dismissing chat pane");
            if let Err(e) = app_handle_clone.emit(events::bar::DISMISS_PANE, ()) {
                error!("[Escape Key] Failed to emit dismiss-pane: {}", e);
            }
            return;
        }

        info!("[Escape Key] Pressed - initiating coordinated stop");
        // Immediate visual feedback — set bar to Stopping state before cleanup begins
        crate::commands::ui_commands::set_stopping_state().await;

        // Play a subtle system sound for audio confirmation
        tokio::task::spawn_blocking(|| {
            let _ = std::process::Command::new("afplay")
                .arg("/System/Library/Sounds/Tink.aiff")
                .output();
        });

        let coordinator = crate::commands::stop_coordinator::get_stop_coordinator();
        if let Err(e) = coordinator
            .stop_all_operations(&app_handle_clone, "Escape key pressed")
            .await
        {
            error!(
                "[Escape Key] Failed to stop operations via coordinator: {}",
                e
            );
        }
    });
}

/// # The gesture recognizer
///
/// One recognizer per bound key, sitting in front of the agent and dictation
/// monitors. Every input source reaches it: global shortcuts,
/// `platform::modifier_key_monitor` for Fn, and
/// `platform::mouse_button_monitor`.
///
/// Its whole job is to say **which independent gesture** an edge belongs to. It
/// does not own a session, it has no hands-free mode, and it never sends one
/// gesture's edge to another gesture's target. What it remembers per key is
/// only gesture bookkeeping: when a short release opened the double-tap window,
/// which second-press branch is still undecided, whether a release has already
/// been claimed, and whether a double-tap-and-hold is currently down.
///
/// What it replaced was a derived double tap: a recognizer that turned a second
/// press of a *hold* key into the hold trigger's own tap path, so one row
/// silently answered two gestures. A double tap is a row of its own now, with
/// its own key and its own target.
///
/// The edges take `now` rather than reading the clock, so the state machine is
/// a pure function of its inputs and the tests drive it with synthetic timings
/// instead of real keys. [`promote`] is the exception and takes no clock: the
/// caller's timer *is* the clock, and it and the release race each other for
/// the same `pending` under one lock, so whichever arrives first wins outright.
pub mod recognizer {
    use std::collections::HashMap;
    use std::sync::LazyLock;
    use std::time::Duration;

    use super::*;

    /// What one edge resolves to. The caller performs it.
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub enum Decision {
        /// This edge belongs to no gesture on this key.
        Ignore,
        /// Start the hold path for this target. The microphone opens at
        /// `IMMEDIATE_START_MS` and `HOLD_DURATION_MS` decides whether the
        /// release commits.
        HoldPress(TriggerTarget),
        /// End the hold path for this target.
        HoldRelease(TriggerTarget),
        /// Run the Tap code path for this target now. That path starts a
        /// session when none is running and stops the one that is.
        Tap(TriggerTarget),
        /// A second press two gestures could claim. This is the one place that
        /// waits: still held at `SECOND_PRESS_HOLD_MS` means `hold` starts, and
        /// a release before then means `quick`.
        Resolve { quick: Quick, hold: TriggerTarget },
    }

    /// What a release before `SECOND_PRESS_HOLD_MS` turns out to mean.
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub enum Quick {
        /// A double tap: run the Tap code path for this target.
        Tap(TriggerTarget),
        /// An ordinary stop of the tap session already running on this key.
        Stop(TriggerTarget),
    }

    /// The outcome of a second press that was held long enough.
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct Promoted {
        /// The double-tap-and-hold target whose hold starts now.
        pub hold: TriggerTarget,
        /// A tap session this press is taking the key away from, which is
        /// cancelled rather than finished: the person is starting something
        /// else, not ending what they said.
        pub cancel: Option<TriggerTarget>,
    }

    #[derive(Debug, Clone, Copy)]
    struct Pending {
        quick: Quick,
        hold: TriggerTarget,
    }

    #[derive(Debug, Default)]
    struct KeyState {
        /// When the short release that opened the double-tap window happened.
        window_opened_at: Option<Instant>,
        /// A second press no gesture has claimed yet.
        pending: Option<Pending>,
        /// The next release of this key has already been accounted for.
        swallow_release: bool,
        /// A double-tap-and-hold is down; its release ends that target.
        holding: Option<TriggerTarget>,
    }

    static KEYS: LazyLock<Mutex<HashMap<String, KeyState>>> =
        LazyLock::new(|| Mutex::new(HashMap::new()));

    fn with<R>(key: &str, f: impl FnOnce(&mut KeyState) -> R) -> R {
        let mut guard = match KEYS.lock() {
            Ok(g) => g,
            Err(poisoned) => poisoned.into_inner(),
        };
        f(guard.entry(key.to_string()).or_default())
    }

    fn window_ms() -> Duration {
        Duration::from_millis(monitor_sessions::DOUBLE_TAP_WINDOW_MS)
    }

    /// Resolve a press-down edge.
    ///
    /// `session` is the tap or double-tap session this key is currently
    /// running, read from the session registry rather than from a flag of the
    /// recognizer's own. That is what lets it hold no session state: "stop what
    /// a tap started" is a question something else already answers.
    pub fn on_press(
        key: &str,
        gestures: KeyGestures,
        session: Option<TriggerTarget>,
        now: Instant,
    ) -> Decision {
        with(key, |s| {
            let in_window = s
                .window_opened_at
                .take()
                .is_some_and(|at| now.duration_since(at) <= window_ms());

            if in_window {
                if let Some(hold) = gestures.double_tap_hold {
                    let quick = match (session, gestures.double_tap) {
                        // A tap session is running and this key also
                        // double-taps-and-holds. A quick release is the
                        // ordinary stop; holding on takes the key for the hold.
                        (Some(target), _) => Quick::Stop(target),
                        (None, Some(target)) => Quick::Tap(target),
                        (None, None) => {
                            // Nothing else wants this press, so there is
                            // nothing to wait for: the hold starts on the down
                            // edge like any other hold.
                            s.holding = Some(hold);
                            s.swallow_release = false;
                            return Decision::HoldPress(hold);
                        }
                    };
                    s.pending = Some(Pending { quick, hold });
                    return Decision::Resolve { quick, hold };
                }
                if let Some(target) = gestures.double_tap {
                    // Double tap alone: the Tap code path on the down edge, and
                    // the release that follows belongs to this gesture.
                    s.swallow_release = true;
                    return Decision::Tap(target);
                }
            }

            // A tap or double-tap session is running: the next press of that key
            // stops it on the down edge, and its release is swallowed.
            if let Some(target) = session {
                s.swallow_release = true;
                return Decision::Tap(target);
            }

            if let Some(target) = gestures.hold {
                return Decision::HoldPress(target);
            }

            // Tap acts on the release. A key carrying only double gestures does
            // nothing at all on a first press, which is the rule: no keyboard
            // trigger fires on a single tap of a hold key.
            Decision::Ignore
        })
    }

    /// Resolve a release edge.
    pub fn on_release(key: &str, gestures: KeyGestures, _now: Instant) -> Decision {
        with(key, |s| {
            if s.swallow_release {
                s.swallow_release = false;
                s.window_opened_at = None;
                return Decision::Ignore;
            }

            // An undecided second press. Reaching here at all means the release
            // beat the `SECOND_PRESS_HOLD_MS` timer to the mutex, so this is the
            // short reading: `promote` takes `pending` under the same lock, and
            // whichever arrives first wins outright.
            if let Some(p) = s.pending.take() {
                // Either reading runs the Tap code path: that path starts a
                // session when none is open and stops the one that is, which is
                // exactly the difference between a double tap and an ordinary
                // stop.
                let target = match p.quick {
                    Quick::Tap(target) | Quick::Stop(target) => target,
                };
                return Decision::Tap(target);
            }

            if let Some(target) = s.holding.take() {
                return Decision::HoldRelease(target);
            }

            if let Some(target) = gestures.hold {
                return Decision::HoldRelease(target);
            }

            if let Some(target) = gestures.tap {
                return Decision::Tap(target);
            }

            Decision::Ignore
        })
    }

    /// Record what the hold monitor made of a release.
    ///
    /// Only a short release opens the double-tap window, and only on a key that
    /// has a gesture waiting for a second press. A committed hold and an idle
    /// release both close it, so a key with one gesture never carries a hidden
    /// second meaning.
    pub fn note_hold_release(key: &str, gestures: KeyGestures, was_short: bool, now: Instant) {
        with(key, |s| {
            s.window_opened_at = if was_short && gestures.has_double() {
                Some(now)
            } else {
                None
            };
        });
    }

    /// Record that a Tap gesture's release just *started* a session.
    ///
    /// The window opens only from the tap that started something, so stopping
    /// after a real sentence can never be read as the first half of a double
    /// tap. And only when the key has a double-tap-and-hold to offer, since
    /// Tap cannot share a key with Hold or Double tap.
    pub fn note_tap_started(key: &str, gestures: KeyGestures, now: Instant) {
        with(key, |s| {
            s.window_opened_at = if gestures.double_tap_hold.is_some() {
                Some(now)
            } else {
                None
            };
        });
    }

    /// `SECOND_PRESS_HOLD_MS` after an undecided second press: is it a hold?
    ///
    /// `Some` means the key is still down and the double-tap-and-hold has
    /// earned it. `None` means the release got here first and was already
    /// resolved as the short reading.
    pub fn promote(key: &str) -> Option<Promoted> {
        with(key, |s| {
            let p = s.pending.take()?;
            s.holding = Some(p.hold);
            Some(Promoted {
                hold: p.hold,
                cancel: match p.quick {
                    Quick::Stop(target) => Some(target),
                    Quick::Tap(_) => None,
                },
            })
        })
    }

    /// Forget every key's bookkeeping.
    ///
    /// Called whenever the triggers are re-registered. A half-finished gesture
    /// is about a binding that may not exist any more, and an open window left
    /// over from the old set would make the first press after a rebind mean
    /// something nobody asked for.
    pub fn forget_all() {
        let mut guard = match KEYS.lock() {
            Ok(g) => g,
            Err(poisoned) => poisoned.into_inner(),
        };
        guard.clear();
    }

    /// Forget one key. Tests keep to their own keys so they can run in
    /// parallel against the process-global map.
    #[cfg(test)]
    fn forget(key: &str) {
        let mut guard = match KEYS.lock() {
            Ok(g) => g,
            Err(poisoned) => poisoned.into_inner(),
        };
        guard.remove(key);
    }

    #[cfg(test)]
    mod tests {
        use super::*;
        use crate::triggers::TriggerTarget::{Agent, Dictation};

        const KEY: &str = "fn (globe)";

        fn fresh(key: &str) {
            forget(key);
        }

        fn hold_only(target: TriggerTarget) -> KeyGestures {
            KeyGestures {
                hold: Some(target),
                ..Default::default()
            }
        }

        fn hold_and_double_tap(hold: TriggerTarget, double: TriggerTarget) -> KeyGestures {
            KeyGestures {
                hold: Some(hold),
                double_tap: Some(double),
                ..Default::default()
            }
        }

        fn hold_and_double_tap_hold(hold: TriggerTarget, dth: TriggerTarget) -> KeyGestures {
            KeyGestures {
                hold: Some(hold),
                double_tap_hold: Some(dth),
                ..Default::default()
            }
        }

        fn tap_and_double_tap_hold(tap: TriggerTarget, dth: TriggerTarget) -> KeyGestures {
            KeyGestures {
                tap: Some(tap),
                double_tap_hold: Some(dth),
                ..Default::default()
            }
        }

        /// A moment `ms` after `base`.
        fn at(base: Instant, ms: u64) -> Instant {
            base + Duration::from_millis(ms)
        }

        /* ------------------------------------------------------------ */
        /* A hold key is only a hold key                                */
        /* ------------------------------------------------------------ */

        #[test]
        fn a_hold_key_holds_and_nothing_else() {
            let key = "hold-only";
            fresh(key);
            let g = hold_only(Dictation);
            let t0 = Instant::now();

            assert_eq!(
                on_press(key, g, None, t0),
                Decision::HoldPress(Dictation),
                "the down edge starts the hold"
            );
            assert_eq!(
                on_release(key, g, at(t0, 500)),
                Decision::HoldRelease(Dictation)
            );
            note_hold_release(key, g, false, at(t0, 500)); // committed

            // A second press is just another hold. Nothing on this key taps.
            assert_eq!(
                on_press(key, g, None, at(t0, 550)),
                Decision::HoldPress(Dictation)
            );
        }

        #[test]
        fn a_short_tap_on_a_hold_only_key_opens_no_window() {
            // The deleted behaviour: a short cancel followed by a second press
            // used to fire the trigger's own tap path and keep listening. With
            // no double gesture bound there is nothing for a second press to
            // mean, so it is a hold again.
            let key = "hold-only-short";
            fresh(key);
            let g = hold_only(Agent);
            let t0 = Instant::now();

            assert_eq!(on_press(key, g, None, t0), Decision::HoldPress(Agent));
            assert_eq!(on_release(key, g, at(t0, 90)), Decision::HoldRelease(Agent));
            note_hold_release(key, g, true, at(t0, 90)); // short = cancelled

            assert_eq!(
                on_press(key, g, None, at(t0, 150)),
                Decision::HoldPress(Agent),
                "a hold key has no derived double tap any more"
            );
        }

        /* ------------------------------------------------------------ */
        /* Hold + Double tap on one key                                 */
        /* ------------------------------------------------------------ */

        #[test]
        fn a_double_tap_fires_its_own_target_not_the_holds() {
            // The point of the model. Holding talks to Juno; double-tapping the
            // same key dictates. Two rows, two targets, one key.
            let key = "hold-plus-double";
            fresh(key);
            let g = hold_and_double_tap(Agent, Dictation);
            let t0 = Instant::now();

            assert_eq!(on_press(key, g, None, t0), Decision::HoldPress(Agent));
            assert_eq!(on_release(key, g, at(t0, 80)), Decision::HoldRelease(Agent));
            note_hold_release(key, g, true, at(t0, 80));

            assert_eq!(
                on_press(key, g, None, at(t0, 200)),
                Decision::Tap(Dictation),
                "the second press is the double tap's own trigger"
            );
            assert_eq!(
                on_release(key, g, at(t0, 260)),
                Decision::Ignore,
                "its release belongs to that press, not to a hold"
            );
        }

        #[test]
        fn a_late_second_press_is_a_fresh_hold() {
            let key = "late-second";
            fresh(key);
            let g = hold_and_double_tap(Agent, Dictation);
            let t0 = Instant::now();

            note_hold_release(key, g, true, t0);
            let late = at(t0, monitor_sessions::DOUBLE_TAP_WINDOW_MS + 50);
            assert_eq!(on_press(key, g, None, late), Decision::HoldPress(Agent));
        }

        #[test]
        fn a_second_press_exactly_on_the_window_edge_still_counts() {
            let key = "window-edge";
            fresh(key);
            let g = hold_and_double_tap(Agent, Dictation);
            let t0 = Instant::now();
            note_hold_release(key, g, true, t0);
            let edge = at(t0, monitor_sessions::DOUBLE_TAP_WINDOW_MS);
            assert_eq!(on_press(key, g, None, edge), Decision::Tap(Dictation));
        }

        #[test]
        fn a_committed_hold_closes_the_window() {
            let key = "committed";
            fresh(key);
            let g = hold_and_double_tap(Agent, Dictation);
            let t0 = Instant::now();
            note_hold_release(key, g, false, t0); // a real hold, not a tap
            assert_eq!(
                on_press(key, g, None, at(t0, 50)),
                Decision::HoldPress(Agent)
            );
        }

        #[test]
        fn a_running_double_tap_session_stops_on_the_next_press() {
            let key = "double-stop";
            fresh(key);
            let g = hold_and_double_tap(Agent, Dictation);
            let t0 = Instant::now();

            // The double tap has started a dictation session.
            assert_eq!(
                on_press(key, g, Some(Dictation), t0),
                Decision::Tap(Dictation),
                "the press stops it on the down edge"
            );
            assert_eq!(on_release(key, g, at(t0, 60)), Decision::Ignore);
        }

        /* ------------------------------------------------------------ */
        /* Hold + Double tap and hold: the headline case                */
        /* ------------------------------------------------------------ */

        #[test]
        fn holding_the_second_press_starts_the_other_trigger() {
            let key = "hold-plus-dth";
            fresh(key);
            let g = hold_and_double_tap_hold(Agent, Dictation);
            let t0 = Instant::now();

            // First press, short: a cancelled hold, which opens the window.
            assert_eq!(on_press(key, g, None, t0), Decision::HoldPress(Agent));
            assert_eq!(on_release(key, g, at(t0, 70)), Decision::HoldRelease(Agent));
            note_hold_release(key, g, true, at(t0, 70));

            // Second press. Only one gesture wants it, so it starts at once:
            // nothing to disambiguate means nothing to wait for.
            assert_eq!(
                on_press(key, g, None, at(t0, 180)),
                Decision::HoldPress(Dictation)
            );
            assert_eq!(
                on_release(key, g, at(t0, 900)),
                Decision::HoldRelease(Dictation),
                "and its own release ends it, not the hold trigger's"
            );
        }

        #[test]
        fn the_hold_trigger_still_works_after_a_double_tap_and_hold() {
            let key = "both-still-work";
            fresh(key);
            let g = hold_and_double_tap_hold(Agent, Dictation);
            let t0 = Instant::now();

            note_hold_release(key, g, true, t0);
            assert_eq!(
                on_press(key, g, None, at(t0, 100)),
                Decision::HoldPress(Dictation)
            );
            assert_eq!(
                on_release(key, g, at(t0, 800)),
                Decision::HoldRelease(Dictation)
            );
            note_hold_release(key, g, false, at(t0, 800));

            // Back to the plain hold.
            assert_eq!(
                on_press(key, g, None, at(t0, 2000)),
                Decision::HoldPress(Agent)
            );
            assert_eq!(
                on_release(key, g, at(t0, 2600)),
                Decision::HoldRelease(Agent)
            );
        }

        /* ------------------------------------------------------------ */
        /* Double tap + Double tap and hold: the one place that waits    */
        /* ------------------------------------------------------------ */

        #[test]
        fn a_second_press_two_gestures_want_is_resolved_by_how_long_it_is_held() {
            let key = "ambiguous";
            fresh(key);
            let g = KeyGestures {
                double_tap: Some(Agent),
                double_tap_hold: Some(Dictation),
                ..Default::default()
            };
            let t0 = Instant::now();

            // A key with only double gestures does nothing on a first press, and
            // its short release opens the window.
            assert_eq!(on_press(key, g, None, t0), Decision::Ignore);
            assert_eq!(on_release(key, g, at(t0, 60)), Decision::Ignore);
            note_hold_release(key, g, true, at(t0, 60));

            assert_eq!(
                on_press(key, g, None, at(t0, 160)),
                Decision::Resolve {
                    quick: Quick::Tap(Agent),
                    hold: Dictation
                },
                "undecided until the key has been held long enough"
            );

            // Released early: the double tap wins, on its own target.
            assert_eq!(on_release(key, g, at(t0, 260)), Decision::Tap(Agent));
            assert_eq!(promote(key), None, "the timer finds nothing left to claim");
        }

        #[test]
        fn still_held_at_the_threshold_is_the_double_tap_and_hold() {
            let key = "ambiguous-held";
            fresh(key);
            let g = KeyGestures {
                double_tap: Some(Agent),
                double_tap_hold: Some(Dictation),
                ..Default::default()
            };
            let t0 = Instant::now();
            note_hold_release(key, g, true, t0);

            assert!(matches!(
                on_press(key, g, None, at(t0, 100)),
                Decision::Resolve { .. }
            ));
            assert_eq!(
                promote(key),
                Some(Promoted {
                    hold: Dictation,
                    cancel: None
                })
            );
            assert_eq!(
                on_release(key, g, at(t0, 1200)),
                Decision::HoldRelease(Dictation),
                "the release ends the hold the timer started"
            );
        }

        #[test]
        fn promote_and_release_cannot_both_claim_the_same_press() {
            // The race the recognizer has to settle, because the timer and the
            // key race each other. Both take `pending` under one lock, so
            // whichever arrives first wins and the other finds nothing.
            let key = "race";
            fresh(key);
            let g = KeyGestures {
                double_tap: Some(Agent),
                double_tap_hold: Some(Dictation),
                ..Default::default()
            };
            let t0 = Instant::now();
            note_hold_release(key, g, true, t0);
            let _ = on_press(key, g, None, at(t0, 50));

            assert!(promote(key).is_some(), "the timer got there first");
            assert_eq!(
                on_release(key, g, at(t0, 60)),
                Decision::HoldRelease(Dictation),
                "so the release ends that hold rather than firing a double tap"
            );
            assert_eq!(promote(key), None, "and nothing is claimed twice");
        }

        /* ------------------------------------------------------------ */
        /* Tap + Double tap and hold                                    */
        /* ------------------------------------------------------------ */

        #[test]
        fn a_tap_fires_on_the_release_of_the_first_press() {
            let key = "tap";
            fresh(key);
            let g = KeyGestures {
                tap: Some(Dictation),
                ..Default::default()
            };
            let t0 = Instant::now();

            assert_eq!(
                on_press(key, g, None, t0),
                Decision::Ignore,
                "the press must not start hold tracking"
            );
            assert_eq!(on_release(key, g, at(t0, 100)), Decision::Tap(Dictation));
        }

        #[test]
        fn a_tap_session_stops_on_the_next_press_of_its_key() {
            let key = "tap-stop";
            fresh(key);
            let g = KeyGestures {
                tap: Some(Dictation),
                ..Default::default()
            };
            let t0 = Instant::now();

            assert_eq!(
                on_press(key, g, Some(Dictation), t0),
                Decision::Tap(Dictation)
            );
            assert_eq!(
                on_release(key, g, at(t0, 80)),
                Decision::Ignore,
                "the stop happened on the down edge"
            );
        }

        #[test]
        fn holding_the_press_after_a_tap_cancels_the_tap_and_starts_the_hold() {
            let key = "tap-plus-dth";
            fresh(key);
            let g = tap_and_double_tap_hold(Dictation, Agent);
            let t0 = Instant::now();

            // A tap starts a dictation session, and that is the release the
            // window opens from.
            assert_eq!(on_press(key, g, None, t0), Decision::Ignore);
            assert_eq!(on_release(key, g, at(t0, 90)), Decision::Tap(Dictation));
            note_tap_started(key, g, at(t0, 90));

            assert_eq!(
                on_press(key, g, Some(Dictation), at(t0, 200)),
                Decision::Resolve {
                    quick: Quick::Stop(Dictation),
                    hold: Agent
                }
            );
            assert_eq!(
                promote(key),
                Some(Promoted {
                    hold: Agent,
                    cancel: Some(Dictation)
                }),
                "the tap session is cancelled, not finished"
            );
        }

        #[test]
        fn a_quick_press_after_a_tap_is_an_ordinary_stop() {
            let key = "tap-plus-dth-stop";
            fresh(key);
            let g = tap_and_double_tap_hold(Dictation, Agent);
            let t0 = Instant::now();
            note_tap_started(key, g, t0);

            assert!(matches!(
                on_press(key, g, Some(Dictation), at(t0, 120)),
                Decision::Resolve {
                    quick: Quick::Stop(Dictation),
                    ..
                }
            ));
            assert_eq!(
                on_release(key, g, at(t0, 200)),
                Decision::Tap(Dictation),
                "a quick release finishes the sentence the ordinary way"
            );
        }

        #[test]
        fn stopping_after_a_real_sentence_opens_no_window() {
            // The window opens only from the tap that *started* a session, so a
            // stop long after cannot be mistaken for the first half of a
            // double tap.
            let key = "tap-stop-no-window";
            fresh(key);
            let g = tap_and_double_tap_hold(Dictation, Agent);
            let t0 = Instant::now();

            // Stop the session: press down (stop), release swallowed.
            assert_eq!(
                on_press(key, g, Some(Dictation), t0),
                Decision::Tap(Dictation)
            );
            assert_eq!(on_release(key, g, at(t0, 70)), Decision::Ignore);

            // Nothing opened a window, so the next press is an ordinary first
            // press and the tap fires on its release.
            assert_eq!(on_press(key, g, None, at(t0, 150)), Decision::Ignore);
            assert_eq!(on_release(key, g, at(t0, 240)), Decision::Tap(Dictation));
        }

        #[test]
        fn a_tap_on_a_key_without_a_double_gesture_opens_no_window() {
            let key = "tap-alone";
            fresh(key);
            let g = KeyGestures {
                tap: Some(Dictation),
                ..Default::default()
            };
            let t0 = Instant::now();
            note_tap_started(key, g, t0);
            assert_eq!(
                on_press(key, g, None, at(t0, 50)),
                Decision::Ignore,
                "a first press on a tap-only key does nothing"
            );
        }

        /* ------------------------------------------------------------ */
        /* Keys do not bleed into each other                            */
        /* ------------------------------------------------------------ */

        #[test]
        fn each_key_has_its_own_recognizer() {
            fresh("key-a");
            fresh("key-b");
            let g = hold_and_double_tap(Agent, Dictation);
            let t0 = Instant::now();

            note_hold_release("key-a", g, true, t0);
            // A press on another key inside key-a's window is a first press.
            assert_eq!(
                on_press("key-b", g, None, at(t0, 50)),
                Decision::HoldPress(Agent)
            );
            // key-a's window is untouched.
            assert_eq!(
                on_press("key-a", g, None, at(t0, 100)),
                Decision::Tap(Dictation)
            );
        }

        #[test]
        fn forgetting_a_key_clears_its_window() {
            fresh(KEY);
            let g = hold_and_double_tap(Agent, Dictation);
            let t0 = Instant::now();
            note_hold_release(KEY, g, true, t0);
            forget(KEY);
            assert_eq!(
                on_press(KEY, g, None, at(t0, 50)),
                Decision::HoldPress(Agent)
            );
        }

        #[test]
        fn an_unbound_key_decides_nothing() {
            let key = "unbound";
            fresh(key);
            let g = KeyGestures::default();
            let t0 = Instant::now();
            assert_eq!(on_press(key, g, None, t0), Decision::Ignore);
            assert_eq!(on_release(key, g, at(t0, 50)), Decision::Ignore);
        }
    }
}

/// Route one edge of one physical key.
///
/// The single entry point for every input source: global shortcuts,
/// `platform::modifier_key_monitor` for Fn, and
/// `platform::mouse_button_monitor`. It owns the onboarding and bar-voice
/// guards so all three behave identically, asks [`recognizer`] which of the
/// key's gestures this edge belongs to, and performs the answer.
pub(crate) fn fire_key_edge(app: &AppHandle, binding: &Binding, pressed: bool) {
    let signature = binding.signature();
    let app_state = app.state::<state::AppState>();

    let triggers = app_state.get_triggers().unwrap_or_default();
    let gestures = crate::triggers::gestures_on_key(&triggers, &signature);
    if gestures.is_empty() {
        return;
    }

    // Visual feedback first, from every source. Onboarding lights its key caps
    // from these events, so a trigger on the globe key has to emit them too:
    // the modifier monitor used to call straight into activation, which is why
    // the final onboarding screen could only ever light up for a combo.
    emit_key_feedback(app, gestures, pressed);

    // During onboarding, activation is suppressed. Feedback above still fires.
    if app_state.is_onboarding_active() {
        return;
    }

    // A bar-initiated spoken query is open: any activation input ends it on
    // release and consumes both edges, so nothing new starts on the busy
    // controller (see the historical race note).
    if crate::agent_monitor::bar_voice_active() {
        if !pressed {
            let _ = app.emit(events::agent::TRANSCRIPTION_STOP, ());
        }
        return;
    }

    let now = Instant::now();
    let decision = if pressed {
        let session = running_tap_session(&app_state, gestures);
        recognizer::on_press(&signature, gestures, session, now)
    } else {
        recognizer::on_release(&signature, gestures, now)
    };

    debug!(
        "[GestureRecognizer] {} {} -> {:?}",
        signature,
        if pressed { "down" } else { "up" },
        decision
    );
    perform(app, &signature, gestures, decision);
}

/// Which gestures this key serves, as visual-feedback events.
fn emit_key_feedback(app: &AppHandle, gestures: KeyGestures, pressed: bool) {
    let state = if pressed { "pressed" } else { "released" };
    for target in gestures.targets() {
        let (event, name) = match target {
            TriggerTarget::Agent => (events::shortcuts::AGENT_MODE, "agent_mode"),
            TriggerTarget::Dictation => (events::shortcuts::DICTATION_INPUT, "dictation_input"),
        };
        if let Err(e) = app.emit(
            event,
            serde_json::json!({ "state": state, "shortcut": name }),
        ) {
            error!("[GestureRecognizer] Failed to emit {}: {}", event, e);
        }
    }
}

/// The tap or double-tap session this key is running, if any.
///
/// Read from the voice-session registry, which records how each session began,
/// rather than from a flag the recognizer keeps. A session started by holding
/// ends when the key comes up, so it is never something a later press stops;
/// one started by a tap is exactly what the next press of that key stops.
fn running_tap_session(
    app_state: &tauri::State<'_, state::AppState>,
    gestures: KeyGestures,
) -> Option<TriggerTarget> {
    let session = app_state.current_voice_session()?;
    // A session that has already been stopped and is waiting on its transcript
    // is not something a press can stop again.
    if !session.is_live() {
        return None;
    }
    if session.method != state::VoiceStartMethod::Toggle {
        return None;
    }
    let target = match session.target {
        state::VoiceTarget::Agent => TriggerTarget::Agent,
        state::VoiceTarget::Dictation => TriggerTarget::Dictation,
    };
    // Only a key that actually taps that target stops it. Another key's hold
    // has no business ending somebody else's sentence.
    let owns = gestures.tap == Some(target) || gestures.double_tap == Some(target);
    if owns {
        Some(target)
    } else {
        None
    }
}

/// Carry out one recognizer decision.
fn perform(
    app: &AppHandle,
    signature: &str,
    gestures: KeyGestures,
    decision: recognizer::Decision,
) {
    use recognizer::Decision;

    match decision {
        Decision::Ignore => {}
        Decision::HoldPress(target) => start_hold(app, target),
        Decision::HoldRelease(target) => end_hold(app, signature, gestures, target),
        Decision::Tap(target) => run_tap_path(app, signature, gestures, target),
        Decision::Resolve { .. } => {
            // The one place that waits. The start cue plays when the hold
            // actually begins, which is what tells the person when to talk.
            let app_clone = app.clone();
            let key = signature.to_string();
            tauri::async_runtime::spawn(async move {
                tokio::time::sleep(std::time::Duration::from_millis(
                    monitor_sessions::SECOND_PRESS_HOLD_MS,
                ))
                .await;
                let Some(promoted) = recognizer::promote(&key) else {
                    return; // the release got there first
                };
                if let Some(cancel) = promoted.cancel {
                    info!(
                        "[GestureRecognizer] Cancelling the {:?} tap session: the key is being held",
                        cancel
                    );
                    cancel_session(&app_clone, cancel);
                }
                info!(
                    "[GestureRecognizer] Second press held past {} ms -> hold {:?}",
                    monitor_sessions::SECOND_PRESS_HOLD_MS,
                    promoted.hold
                );
                start_hold(&app_clone, promoted.hold);
            });
        }
    }
}

fn start_hold(app: &AppHandle, target: TriggerTarget) {
    match target {
        TriggerTarget::Agent => {
            tauri::async_runtime::spawn(async move {
                crate::agent_monitor::on_agent_input_pressed().await;
            });
        }
        TriggerTarget::Dictation => {
            let app_clone = app.clone();
            tauri::async_runtime::spawn(async move {
                crate::dictation_monitor::on_dictation_input_pressed(&app_clone).await;
            });
        }
    }
}

fn end_hold(app: &AppHandle, signature: &str, gestures: KeyGestures, target: TriggerTarget) {
    let app_clone = app.clone();
    let key = signature.to_string();
    match target {
        TriggerTarget::Agent => {
            tauri::async_runtime::spawn(async move {
                let outcome = crate::agent_monitor::on_agent_input_released_with_mode(
                    &app_clone,
                    state::AgentTriggerMode::Hold,
                )
                .await;
                // Only a short-tap cancel opens the double-tap window. A
                // committed hold and an idle release both close it.
                recognizer::note_hold_release(
                    &key,
                    gestures,
                    matches!(outcome, crate::agent_monitor::AgentRelease::Cancelled),
                    Instant::now(),
                );
            });
        }
        TriggerTarget::Dictation => {
            tauri::async_runtime::spawn(async move {
                let outcome =
                    crate::dictation_monitor::on_dictation_input_released(&app_clone).await;
                recognizer::note_hold_release(
                    &key,
                    gestures,
                    matches!(outcome, crate::dictation_monitor::HoldRelease::Cancelled),
                    Instant::now(),
                );
            });
        }
    }
}

/// The Tap code path: exactly what a Tap trigger calls.
fn run_tap_path(app: &AppHandle, signature: &str, gestures: KeyGestures, target: TriggerTarget) {
    let starting = !session_live_for(app, target);
    match target {
        TriggerTarget::Agent => {
            let app_clone = app.clone();
            tauri::async_runtime::spawn(async move {
                crate::agent_monitor::on_agent_input_released_with_mode(
                    &app_clone,
                    state::AgentTriggerMode::Tap,
                )
                .await;
            });
        }
        TriggerTarget::Dictation => handle_dictation_tap_mode(app),
    }
    if starting {
        recognizer::note_tap_started(signature, gestures, Instant::now());
    }
}

/// Is a session already open for this target?
///
/// Decides whether the Tap path above is about to start something or stop
/// something, which is the difference between opening the double-tap window
/// and leaving it shut.
fn session_live_for(app: &AppHandle, target: TriggerTarget) -> bool {
    let app_state = app.state::<state::AppState>();
    match target {
        TriggerTarget::Agent => crate::agent_monitor::bar_voice_active(),
        // The same rule the tap path itself uses, so the double-tap window
        // and the tap agree about whether this press opened something. See
        // [`state::dictation_tap_means_stop`].
        TriggerTarget::Dictation => state::dictation_tap_means_stop(
            app_state.current_voice_session(),
            app_state.is_dictation_active(),
        ),
    }
}

/// Discard whatever this target has captured so far.
///
/// Used when a second press takes the key away from a tap session: the person
/// is starting something else, not finishing what they said.
fn cancel_session(app: &AppHandle, target: TriggerTarget) {
    let event = match target {
        TriggerTarget::Agent => events::agent::CANCEL,
        TriggerTarget::Dictation => events::dictation::TRANSCRIPTION_CANCEL,
    };
    if let Err(e) = app.emit(event, ()) {
        error!("[GestureRecognizer] Failed to emit {}: {}", event, e);
    }
}

/// Handle dictation tap mode: start a session, or stop the one that is running.
fn handle_dictation_tap_mode(app: &AppHandle) {
    info!("[Dictation Tap Mode] Entered handle_dictation_tap_mode");

    // Asked of the session registry, not of the liveness flag alone. The flag
    // is a mirror that drifts in both directions, and each direction had its
    // own stuck state; the registry is the record that owns the microphone.
    // The rule, and the two bugs it closes, are in
    // [`state::dictation_tap_means_stop`]. Neither reads the VoiceController
    // mutex, which can be held through a final decode.
    let app_state = app.state::<state::AppState>();
    let stopping = state::dictation_tap_means_stop(
        app_state.current_voice_session(),
        app_state.is_dictation_active(),
    );

    info!(
        "[Dictation Tap Mode] session: {:?}, dictation flag: {}, so this tap {}",
        app_state.current_voice_session().map(|s| s.describe()),
        app_state.is_dictation_active(),
        if stopping { "stops" } else { "starts" }
    );

    if stopping {
        info!("[Dictation Input Shortcut] Tap mode - stopping active dictation");

        // Immediate stop cue on the tap edge — before the async stop that waits
        // on speech-to-text finalization. Keeps tap-mode stop as responsive as
        // hold-mode release.
        crate::commands::sound::play_cue(
            app,
            crate::commands::sound::SoundType::NotificationDecorative01,
        );

        // Route through the same stop event as hold mode. Calling stop_dictation()
        // directly here skipped handle_dictation_stop, so no bar-state update was
        // emitted and the bar stayed stuck on "listening" until the slow final
        // result arrived. Emitting dictation::STOP flips the bar to the processing
        // state immediately and then finalizes, exactly like a hold release.
        if let Err(e) = app.emit(events::dictation::STOP, ()) {
            error!("[Dictation Tap Mode] Failed to emit dictation-stop: {}", e);
        }
    } else {
        info!("[Dictation Input Shortcut] Tap mode - starting dictation mode transcription");

        // Immediate start cue on the tap edge — before the async start that
        // initializes audio capture.
        crate::commands::sound::play_cue(
            app,
            crate::commands::sound::SoundType::NotificationAmbient,
        );

        let app_handle = app.clone();
        tauri::async_runtime::spawn(async move {
            // Emit dictation transcription start event instead of active event
            // This will be handled by the event listener in events/handlers.rs
            // Say how this was triggered, so the session records its method at
            // birth rather than a stop path inferring it later. "toggle" is the
            // registry's word for a session a tap opened, which is what the
            // recognizer reads back when deciding whether a later press stops it.
            if let Err(e) = app_handle.emit(
                events::dictation::TRANSCRIPTION_START,
                serde_json::json!({ "method": "toggle" }),
            ) {
                error!(
                    "[Dictation Tap Mode] Failed to emit dictation-transcription-start event: {}",
                    e
                );
            }

            info!("[Dictation Tap Mode] Emitted dictation start event for handler processing");
        });
    }
}

/// Add a new command to trigger shortcut testing events during onboarding
#[tauri::command]
pub async fn trigger_shortcut_test_event(
    app: AppHandle,
    shortcut_name: String,
    state: String,
) -> Result<(), String> {
    let event_name = match shortcut_name.as_str() {
        "agent_mode" => events::shortcuts::AGENT_MODE,
        "dictation_input" => events::shortcuts::DICTATION_INPUT,
        _ => return Err("Unknown shortcut name".to_string()),
    };

    if let Err(e) = app.emit(
        event_name,
        serde_json::json!({
            "state": state,
            "shortcut": shortcut_name,
            "test_mode": true
        }),
    ) {
        return Err(crate::format_error(
            templates::FAILED_TO_EMIT,
            "test event",
            e,
        ));
    }

    Ok(())
}

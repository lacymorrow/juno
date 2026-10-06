//! # Global Shortcut Handler
//!
//! Every way a key or a mouse button can summon Juno arrives here. The two
//! utility shortcuts (Escape to stop, Cmd+Comma to open settings) are fixed
//! constants; everything else is a [`crate::triggers::Trigger`], and a key is
//! handed to [`recognizer`], which decides which of that key's independent
//! gestures the edge belongs to.

use std::sync::Mutex;

use tauri::{AppHandle, Emitter, Manager};
use tauri_plugin_global_shortcut::{Shortcut, ShortcutEvent, ShortcutState};
use tracing::{debug, error, info};

use crate::constants::{errors::templates, events};
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
        // A dictation open on top of a running agent is the layer Escape
        // takes off first; the run keeps going (see `escape_scope`).
        if crate::commands::dictation::escape_spent_on_dictation(&app_handle_clone).await {
            return;
        }

        // When nothing is actually running, Escape has no work to cancel — treat
        // it as "close the chat pane" instead, so the global monitor lets Escape
        // dismiss the pane even when the bar is not focused. The pane arms this
        // monitor only while it is open (see `set_bar_pane_open`).
        //
        // The bar's own state is not reset here: the look answers with the
        // `escape` interaction, which lands in `bar_escape_to_idle`, and only
        // the look knows whether a popup inside it owns this press (the skill
        // autocomplete). The local monitor sees presses on Juno's own windows
        // too, so resetting here would collapse the bar under that popup.
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
        // Whatever the stop left behind (a `Stopping` the response never
        // cleared, a composer), the bar ends at rest.
        if let Some(manager) = crate::commands::ui_commands::get_ui_manager().await {
            manager.lock().await.escape_to_idle().await;
        }
    });
}

/// # The gesture recognizer
///
/// One recognizer per bound key, sitting in front of the agent and dictation
/// monitors. Every input source reaches it: global shortcuts,
/// `platform::modifier_key_monitor` for Fn and Fn+Control, and
/// `platform::mouse_button_monitor`.
///
/// Its whole job is to say **which gesture** an edge belongs to. There are two
/// a key can carry, Hold and Tap, and one key carries only one of them, so
/// there is almost nothing to decide: a Hold key starts on the down edge and
/// ends on the up edge, a Tap key acts on the release, and the next press of a
/// key whose tap session is running stops it. What it remembers per key is a
/// single bit, that a press has already been spent stopping a session and its
/// release must be swallowed.
///
/// The edges take `now` for the callers that already hold a clock; nothing here
/// reads one.
pub mod recognizer {
    use std::collections::HashMap;
    use std::sync::LazyLock;

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
    }

    #[derive(Debug, Default)]
    struct KeyState {
        /// The next release of this key has already been accounted for.
        swallow_release: bool,
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

    /// Resolve a press-down edge.
    ///
    /// `session` is the tap session this key is currently running, read from
    /// the session registry rather than from a flag of the recognizer's own.
    /// That is what lets it hold no session state: "stop what a tap started" is
    /// a question something else already answers.
    pub fn on_press(key: &str, gestures: KeyGestures, session: Option<TriggerTarget>) -> Decision {
        with(key, |s| {
            // A tap session is running: the next press of that key stops it on
            // the down edge, and its release is swallowed.
            if let Some(target) = session {
                s.swallow_release = true;
                return Decision::Tap(target);
            }

            if let Some(target) = gestures.hold {
                return Decision::HoldPress(target);
            }

            // Tap acts on the release.
            Decision::Ignore
        })
    }

    /// Resolve a release edge.
    pub fn on_release(key: &str, gestures: KeyGestures) -> Decision {
        with(key, |s| {
            if s.swallow_release {
                s.swallow_release = false;
                return Decision::Ignore;
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

    /// Forget every key's bookkeeping.
    ///
    /// Called whenever the triggers are re-registered. A half-finished gesture
    /// is about a binding that may not exist any more.
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

        fn hold_only(target: TriggerTarget) -> KeyGestures {
            KeyGestures {
                hold: Some(target),
                ..Default::default()
            }
        }

        fn tap_only(target: TriggerTarget) -> KeyGestures {
            KeyGestures {
                tap: Some(target),
                ..Default::default()
            }
        }

        #[test]
        fn a_hold_key_holds_and_nothing_else() {
            let key = "hold-only";
            forget(key);
            let g = hold_only(Dictation);

            assert_eq!(
                on_press(key, g, None),
                Decision::HoldPress(Dictation),
                "the down edge starts the hold"
            );
            assert_eq!(on_release(key, g), Decision::HoldRelease(Dictation));

            // A second press is just another hold. There is no double tap.
            assert_eq!(on_press(key, g, None), Decision::HoldPress(Dictation));
        }

        #[test]
        fn a_short_tap_on_a_hold_key_is_followed_by_an_ordinary_hold() {
            // The deleted behaviour: a short cancel followed quickly by a
            // second press used to open a window and mean something else.
            // Now every press of a hold key is a hold.
            let key = "hold-quick-twice";
            forget(key);
            let g = hold_only(Agent);
            for _ in 0..3 {
                assert_eq!(on_press(key, g, None), Decision::HoldPress(Agent));
                assert_eq!(on_release(key, g), Decision::HoldRelease(Agent));
            }
        }

        #[test]
        fn a_tap_fires_on_the_release_of_the_first_press() {
            let key = "tap";
            forget(key);
            let g = tap_only(Dictation);

            assert_eq!(
                on_press(key, g, None),
                Decision::Ignore,
                "the press must not start hold tracking"
            );
            assert_eq!(on_release(key, g), Decision::Tap(Dictation));
        }

        #[test]
        fn a_tap_session_stops_on_the_next_press_of_its_key() {
            let key = "tap-stop";
            forget(key);
            let g = tap_only(Dictation);

            assert_eq!(on_press(key, g, Some(Dictation)), Decision::Tap(Dictation));
            assert_eq!(
                on_release(key, g),
                Decision::Ignore,
                "the stop happened on the down edge"
            );
            // And the swallow was spent: the next release is an ordinary tap.
            assert_eq!(on_release(key, g), Decision::Tap(Dictation));
        }

        #[test]
        fn each_key_has_its_own_recognizer() {
            forget("key-a");
            forget("key-b");
            let g = tap_only(Agent);

            // key-a spends a press stopping a session.
            assert_eq!(on_press("key-a", g, Some(Agent)), Decision::Tap(Agent));
            // key-b is untouched by it.
            assert_eq!(on_release("key-b", g), Decision::Tap(Agent));
            assert_eq!(on_release("key-a", g), Decision::Ignore);
        }

        #[test]
        fn the_chord_and_the_plain_key_are_independent_holds() {
            // Fn holds to talk to Juno, Fn+Control holds to dictate. They are
            // two keys with two recognizers and never see each other's edges.
            let fn_key = "fn (globe)";
            let chord = "fn + control";
            forget(fn_key);
            forget(chord);

            assert_eq!(
                on_press(fn_key, hold_only(Agent), None),
                Decision::HoldPress(Agent)
            );
            assert_eq!(
                on_release(fn_key, hold_only(Agent)),
                Decision::HoldRelease(Agent)
            );
            assert_eq!(
                on_press(chord, hold_only(Dictation), None),
                Decision::HoldPress(Dictation)
            );
            assert_eq!(
                on_release(chord, hold_only(Dictation)),
                Decision::HoldRelease(Dictation)
            );
        }

        #[test]
        fn forgetting_a_key_clears_a_pending_swallow() {
            let key = "forgotten";
            forget(key);
            let g = tap_only(Dictation);
            assert_eq!(on_press(key, g, Some(Dictation)), Decision::Tap(Dictation));
            forget(key);
            assert_eq!(on_release(key, g), Decision::Tap(Dictation));
        }

        #[test]
        fn an_unbound_key_decides_nothing() {
            let key = "unbound";
            forget(key);
            let g = KeyGestures::default();
            assert_eq!(on_press(key, g, None), Decision::Ignore);
            assert_eq!(on_release(key, g), Decision::Ignore);
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
            // The key is up, so whatever hold its monitor was tracking is
            // over too. Without this a hold that began before the bar query
            // opened would never see its release, and the monitor would
            // refuse every later press of that key as "already active".
            if let Some(target) = gestures.hold {
                hold_edges::enqueue(app, hold_edges::HoldEdge::Forget(target));
            }
        }
        return;
    }

    let decision = if pressed {
        let session = running_tap_session(&app_state, gestures);
        recognizer::on_press(&signature, gestures, session)
    } else {
        recognizer::on_release(&signature, gestures)
    };

    debug!(
        "[GestureRecognizer] {} {} -> {:?}",
        signature,
        if pressed { "down" } else { "up" },
        decision
    );
    perform(app, decision);
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

/// The tap session this key is running, if any.
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
    (gestures.tap == Some(target)).then_some(target)
}

/// Carry out one recognizer decision.
fn perform(app: &AppHandle, decision: recognizer::Decision) {
    use recognizer::Decision;

    match decision {
        Decision::Ignore => {}
        Decision::HoldPress(target) => start_hold(app, target),
        Decision::HoldRelease(target) => end_hold(app, target),
        Decision::Tap(target) => run_tap_path(app, target),
    }
}

fn start_hold(app: &AppHandle, target: TriggerTarget) {
    hold_edges::enqueue(app, hold_edges::HoldEdge::Press(target));
}

fn end_hold(app: &AppHandle, target: TriggerTarget) {
    hold_edges::enqueue(app, hold_edges::HoldEdge::Release(target));
}

/// # Hold edges run in the order the keys moved
///
/// Every press and release used to be its own spawned task. Two spawns from
/// the event thread are not ordered: on the multi-threaded runtime the
/// release's task can reach the input monitor's lock before the press's does.
/// That happens exactly when the two edges arrive together, which is what a
/// delayed modifier does when it is let go just past its window (the
/// flags-changed event delivers the overdue down edge and the up edge in one
/// batch). The monitor then saw the release first (nothing to end), then the
/// press (a hold starts), and no release ever came: the session started
/// listening on a key that was already up, and stayed there.
///
/// One consumer, fed in arrival order, removes the race by construction. The
/// work it does per edge is a lock and an emit, so nothing queues behind it
/// for long.
mod hold_edges {
    use super::*;
    use tokio::sync::mpsc::{unbounded_channel, UnboundedSender};

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub enum HoldEdge {
        Press(TriggerTarget),
        Release(TriggerTarget),
        /// The Tap path for the agent, which ends in the same monitor.
        AgentTap,
        /// The key is up but its release was spent elsewhere: put the
        /// monitor back to rest without acting on anything.
        Forget(TriggerTarget),
    }

    type Queue = Option<UnboundedSender<(AppHandle, HoldEdge)>>;
    static QUEUE: Mutex<Queue> = Mutex::new(None);

    pub fn enqueue(app: &AppHandle, edge: HoldEdge) {
        let mut guard = match QUEUE.lock() {
            Ok(g) => g,
            Err(poisoned) => poisoned.into_inner(),
        };
        let alive = guard.as_ref().is_some_and(|tx| !tx.is_closed());
        if !alive {
            let (tx, mut rx) = unbounded_channel::<(AppHandle, HoldEdge)>();
            tauri::async_runtime::spawn(async move {
                while let Some((app, edge)) = rx.recv().await {
                    run(&app, edge).await;
                }
            });
            *guard = Some(tx);
        }
        let sent = guard
            .as_ref()
            .is_some_and(|tx| tx.send((app.clone(), edge)).is_ok());
        drop(guard);
        if !sent {
            // Never drop an edge: a lost release is the stuck state itself.
            // Run it on its own rather than not at all.
            error!(
                "[GestureRecognizer] Hold edge queue unavailable; running {:?} directly",
                edge
            );
            let app = app.clone();
            tauri::async_runtime::spawn(async move { run(&app, edge).await });
        }
    }

    async fn run(app: &AppHandle, edge: HoldEdge) {
        match edge {
            HoldEdge::Press(TriggerTarget::Agent) => {
                crate::agent_monitor::on_agent_input_pressed().await;
            }
            HoldEdge::Press(TriggerTarget::Dictation) => {
                crate::dictation_monitor::on_dictation_input_pressed(app).await;
            }
            HoldEdge::Release(TriggerTarget::Agent) => {
                crate::agent_monitor::on_agent_input_released_with_mode(
                    app,
                    state::AgentTriggerMode::Hold,
                )
                .await;
            }
            HoldEdge::Release(TriggerTarget::Dictation) => {
                crate::dictation_monitor::on_dictation_input_released(app).await;
            }
            HoldEdge::Forget(TriggerTarget::Agent) => {
                crate::agent_monitor::force_reset_agent_input_state().await;
            }
            HoldEdge::Forget(TriggerTarget::Dictation) => {
                crate::dictation_monitor::force_reset_dictation_input_state().await;
            }
            HoldEdge::AgentTap => {
                crate::agent_monitor::on_agent_input_released_with_mode(
                    app,
                    state::AgentTriggerMode::Tap,
                )
                .await;
            }
        }
    }
}

/// The Tap code path: exactly what a Tap trigger calls.
fn run_tap_path(app: &AppHandle, target: TriggerTarget) {
    match target {
        TriggerTarget::Agent => hold_edges::enqueue(app, hold_edges::HoldEdge::AgentTap),
        TriggerTarget::Dictation => handle_dictation_tap_mode(app),
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

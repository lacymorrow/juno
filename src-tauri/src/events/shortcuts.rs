//! # Global Shortcut Handler
//!
//! This module handles all global keyboard shortcuts for the Juno application,
//! including escape key handling, agent mode toggle, and dictation input shortcuts.

use tauri::{AppHandle, Emitter, Manager};
use tauri_plugin_global_shortcut::{Shortcut, ShortcutEvent, ShortcutState};
use tracing::{debug, error, info};

use crate::constants::{errors::templates, events};
use crate::state;

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

    let app_state = app.state::<state::AppState>();

    // The two utility shortcuts are fixed constants, not settings: Escape
    // cancels and Cmd+Comma opens settings, the way every Mac app does it, so
    // there is nothing to look up and nothing a stored value could override.
    // Activation (agent + dictation) is driven by the triggers matrix below,
    // and voice is a trigger method of its own, so there is no
    // voice-activation shortcut at all.
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

    // Route the incoming shortcut through the activation triggers: any enabled
    // key-bound trigger whose combo matches fires by its own method/target, so
    // the bounded matrix (push-to-talk / toggle x agent / dictation) works even
    // when several combos are bound.
    dispatch_activation_triggers(app, &app_state, shortcut, event);
}

/// Fire every enabled keyboard trigger whose binding matches `shortcut`.
fn dispatch_activation_triggers(
    app: &AppHandle,
    app_state: &tauri::State<'_, state::AppState>,
    shortcut: &Shortcut,
    event: &ShortcutEvent,
) {
    use crate::triggers::{Binding, TriggerTarget};

    let triggers = match app_state.get_triggers() {
        Ok(t) => t,
        Err(e) => {
            error!("[GlobalShortcut] Failed to read triggers: {}", e);
            return;
        }
    };

    // One key press is one activation per target, however many rows describe it.
    // Two enabled triggers can legitimately share a combo while pointing at
    // different targets, but nothing good comes of dispatching the same target
    // twice for a single edge: the second call re-enters a session the first
    // one just started, and the UI is told "agent mode activated" once per
    // matching row. That is where the stack of identical toasts came from.
    let mut fired_agent = false;
    let mut fired_dictation = false;

    for trigger in triggers.iter().filter(|t| t.enabled) {
        let Some(Binding::Keyboard { shortcut: combo }) = &trigger.binding else {
            continue; // voice + mouse handled elsewhere
        };
        // A bare modifier such as Fn is a keyboard binding, but the plugin
        // cannot register it and the flags-changed monitor fires it directly.
        // Skipping it here keeps that one edge from arriving twice if the
        // combo parser ever learns to spell it.
        if crate::triggers::bare_modifier(combo).is_some() {
            continue;
        }
        let Some(parsed) = parse_shortcut_string(combo) else {
            continue;
        };
        if *shortcut != parsed {
            continue;
        }
        // Logged at info, because "the shortcut does nothing" is the report
        // that keeps coming back and this is the line that settles it: the
        // registration log above says the combo was claimed, and this one says
        // a press of it arrived and was routed.
        info!(
            "[GlobalShortcut] {} fired {} ({:?})",
            combo,
            trigger.label(),
            event.state()
        );

        match trigger.target {
            TriggerTarget::Agent => {
                if fired_agent {
                    continue;
                }
                fired_agent = true;
                handle_agent_mode_shortcut(app, event, trigger.method);
            }
            TriggerTarget::Dictation => {
                if fired_dictation {
                    continue;
                }
                fired_dictation = true;
                handle_dictation_input_shortcut(app, event, trigger.method);
            }
        }

        if fired_agent && fired_dictation {
            break;
        }
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

/// Handle agent mode shortcut (Option+D by default)
/// Shared activation routing for a key OR mouse trigger edge. Owns the
/// onboarding and bar-voice guards so both input sources behave identically,
/// then drives the agent/dictation monitors by the trigger's own method.
pub(crate) fn fire_trigger_edge(
    app: &AppHandle,
    method: crate::triggers::TriggerMethod,
    target: crate::triggers::TriggerTarget,
    pressed: bool,
) {
    use crate::triggers::{TriggerMethod, TriggerTarget};

    let app_state = app.state::<state::AppState>();

    // During onboarding, activation is suppressed (callers still emit their own
    // visual-feedback events before calling this).
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

    match target {
        TriggerTarget::Agent => {
            match method {
                TriggerMethod::PushToTalk => {
                    let app_clone = app.clone();
                    tauri::async_runtime::spawn(async move {
                        if pressed {
                            crate::agent_monitor::on_agent_input_pressed().await;
                        } else {
                            crate::agent_monitor::on_agent_input_released_with_mode(
                                &app_clone,
                                state::AgentTriggerMode::Hold,
                            )
                            .await;
                        }
                    });
                }
                TriggerMethod::Toggle => {
                    // Tap: act on the release edge only (press+release = one tap),
                    // mirroring the dictation arm below. The press must NOT start
                    // hold-tracking: the background monitor polls every 100ms and
                    // `check_and_start_agent` fires at IMMEDIATE_START_MS (0ms), so
                    // any tick landing inside the tap flipped `agent_started` true
                    // and the release then took the cancel path instead of starting
                    // the toggle. A natural tap is ~100ms, so the toggle was a coin
                    // flip. Skipping the press keeps `hold_start_time` unset, the
                    // poll never engages, and the release reliably toggles.
                    if !pressed {
                        let app_clone = app.clone();
                        tauri::async_runtime::spawn(async move {
                            crate::agent_monitor::on_agent_input_released_with_mode(
                                &app_clone,
                                state::AgentTriggerMode::Tap,
                            )
                            .await;
                        });
                    }
                }
                TriggerMethod::Voice => {} // voice never routes through key/mouse dispatch
            }
        }
        TriggerTarget::Dictation => match method {
            TriggerMethod::PushToTalk => {
                let app_clone = app.clone();
                tauri::async_runtime::spawn(async move {
                    if pressed {
                        crate::dictation_monitor::on_dictation_input_pressed(&app_clone).await;
                    } else {
                        crate::dictation_monitor::on_dictation_input_released(&app_clone).await;
                    }
                });
            }
            TriggerMethod::Toggle => {
                // Tap: act on the release edge only (press+release = one tap).
                if !pressed {
                    handle_dictation_tap_mode(app);
                }
            }
            TriggerMethod::Voice => {} // voice never routes through key/mouse dispatch
        },
    }
}

fn handle_agent_mode_shortcut(
    app: &AppHandle,
    event: &ShortcutEvent,
    method: crate::triggers::TriggerMethod,
) {
    // Emit shortcut detection events for visual feedback in onboarding
    let shortcut_state = match event.state() {
        ShortcutState::Pressed => "pressed",
        ShortcutState::Released => "released",
    };

    if let Err(e) = app.emit(
        events::shortcuts::AGENT_MODE,
        serde_json::json!({
            "state": shortcut_state,
            "shortcut": "agent_mode"
        }),
    ) {
        error!(
            "[Agent Mode Shortcut] Failed to emit shortcut detection event: {}",
            e
        );
    }

    // Route the actual activation (onboarding + bar-voice guards live in the
    // shared core, so keyboard and mouse triggers behave identically).
    fire_trigger_edge(
        app,
        method,
        crate::triggers::TriggerTarget::Agent,
        event.state() == ShortcutState::Pressed,
    );
}

// Removed handle_agent_tap_mode - unified through AgentMonitor

/// Handle dictation input shortcut (Option+Space by default)
fn handle_dictation_input_shortcut(
    app: &AppHandle,
    event: &ShortcutEvent,
    method: crate::triggers::TriggerMethod,
) {
    // Emit shortcut detection events for visual feedback in onboarding
    let shortcut_state = match event.state() {
        ShortcutState::Pressed => "pressed",
        ShortcutState::Released => "released",
    };

    if let Err(e) = app.emit(
        events::shortcuts::DICTATION_INPUT,
        serde_json::json!({
            "state": shortcut_state,
            "shortcut": "dictation_input"
        }),
    ) {
        error!(
            "[Dictation Input Shortcut] Failed to emit shortcut detection event: {}",
            e
        );
    }

    // Route the actual activation through the shared core (onboarding +
    // bar-voice guards, tap/hold from this trigger's method).
    fire_trigger_edge(
        app,
        method,
        crate::triggers::TriggerTarget::Dictation,
        event.state() == ShortcutState::Pressed,
    );
}

/// Handle dictation tap mode (new functionality)
fn handle_dictation_tap_mode(app: &AppHandle) {
    info!("[Dictation Tap Mode] Entered handle_dictation_tap_mode");

    // Check if dictation is currently active using AppState (no locking required)
    // This avoids the VoiceController mutex which can be held during audio processing
    let app_state = app.state::<state::AppState>();
    let is_dictation_active = app_state.is_dictation_active();

    info!(
        "[Dictation Tap Mode] is_dictation_active (from AppState): {}",
        is_dictation_active
    );

    if is_dictation_active {
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
            // birth rather than a stop path inferring it later.
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

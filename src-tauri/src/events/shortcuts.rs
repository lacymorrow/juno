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

    // Get current keyboard shortcuts from state
    let current_shortcuts = match app_state.get_keyboard_shortcuts() {
        Ok(shortcuts) => shortcuts,
        Err(e) => {
            error!(
                "{}",
                crate::format_error(templates::FAILED_TO_RETRIEVE, "keyboard shortcuts", e)
            );
            return; // Exit early if we can't get shortcuts
        }
    };

    // Parse the utility shortcuts (stop / open-settings / voice-activation).
    // Activation (agent + dictation) is driven by the triggers matrix below.
    let stop_shortcut: Option<Shortcut> =
        parse_shortcut_string(&current_shortcuts.stop_current_task);
    let settings_shortcut: Option<Shortcut> =
        parse_shortcut_string(&current_shortcuts.open_settings);
    let voice_activation_shortcut: Option<Shortcut> =
        parse_shortcut_string(&current_shortcuts.voice_activation);

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

    // Check voice activation shortcut
    if let Some(voice_activation_obj) = voice_activation_shortcut {
        if *shortcut == voice_activation_obj {
            handle_voice_activation_shortcut(app, event);
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

    for trigger in triggers.iter().filter(|t| t.enabled) {
        let Some(Binding::Keyboard { shortcut: combo }) = &trigger.binding else {
            continue; // voice + mouse handled elsewhere
        };
        let Some(parsed) = parse_shortcut_string(combo) else {
            continue;
        };
        if *shortcut != parsed {
            continue;
        }
        match trigger.target {
            TriggerTarget::Agent => handle_agent_mode_shortcut(app, event, trigger.method),
            TriggerTarget::Dictation => handle_dictation_input_shortcut(app, event, trigger.method),
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
            let agent_mode = if method == TriggerMethod::PushToTalk {
                state::AgentTriggerMode::Hold
            } else {
                state::AgentTriggerMode::Tap
            };
            let app_clone = app.clone();
            tauri::async_runtime::spawn(async move {
                if pressed {
                    crate::agent_monitor::on_agent_input_pressed().await;
                } else {
                    crate::agent_monitor::on_agent_input_released_with_mode(&app_clone, agent_mode)
                        .await;
                }
            });
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
            if let Err(e) = app_handle.emit(events::dictation::TRANSCRIPTION_START, ()) {
                error!(
                    "[Dictation Tap Mode] Failed to emit dictation-transcription-start event: {}",
                    e
                );
            }

            info!("[Dictation Tap Mode] Emitted dictation start event for handler processing");
        });
    }
}

/// Always-on tap-to-toggle voice recording from anywhere on macOS (Option+Shift+V default).
fn handle_voice_activation_shortcut(app: &AppHandle, event: &ShortcutEvent) {
    // Only fire on key press — this is a stateless toggle, no hold semantics
    if event.state() != ShortcutState::Pressed {
        return;
    }

    // Emit visual feedback event (for onboarding UI and status indicators)
    if let Err(e) = app.emit(
        events::shortcuts::VOICE_ACTIVATION,
        serde_json::json!({
            "state": "pressed",
            "shortcut": "voice_activation"
        }),
    ) {
        error!("[Voice Activation] Failed to emit shortcut event: {}", e);
    }

    // During onboarding, only provide visual feedback
    let app_state = app.state::<state::AppState>();
    if app_state.is_onboarding_active() {
        info!("[Voice Activation] Pressed during onboarding - visual feedback only");
        return;
    }

    // Delegate to the dictation tap handler — same behaviour: toggle recording on each press
    info!("[Voice Activation] Triggering voice activation (tap-mode dictation toggle)");
    handle_dictation_tap_mode(app);
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
        "voice_activation" => events::shortcuts::VOICE_ACTIVATION,
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

//! # Global Shortcut Handler
//!
//! This module handles all global keyboard shortcuts for the Juno application,
//! including escape key handling, agent mode toggle, and dictation input shortcuts.

use std::sync::{Arc, Mutex};
use tauri::{AppHandle, Emitter, Manager};
use tauri_plugin_global_shortcut::{Shortcut, ShortcutEvent, ShortcutState};
use tauri_plugin_voice_transcription::controller::VoiceController;
use tracing::{debug, error, info};

use crate::state;
use crate::constants::{events, errors::templates};

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
            error!("{}", crate::format_error(templates::FAILED_TO_RETRIEVE, "keyboard shortcuts", e));
            return; // Exit early if we can't get shortcuts
        }
    };

    // Parse all shortcuts from configuration (including stop_current_task)
    let stop_shortcut: Option<Shortcut> =
        parse_shortcut_string(&current_shortcuts.stop_current_task);
    let agent_shortcut: Option<Shortcut> =
        parse_shortcut_string(&current_shortcuts.agent_mode);
    let dictation_shortcut: Option<Shortcut> =
        parse_shortcut_string(&current_shortcuts.dictation_input);
    let settings_shortcut: Option<Shortcut> =
        parse_shortcut_string(&current_shortcuts.open_settings);
    let voice_activation_shortcut: Option<Shortcut> =
        parse_shortcut_string(&current_shortcuts.voice_activation);

    debug!("Current shortcuts — stop:{} agent:{} dictation:{} settings:{} voice:{} | incoming:{:?}",
        current_shortcuts.stop_current_task,
        current_shortcuts.agent_mode,
        current_shortcuts.dictation_input,
        current_shortcuts.open_settings,
        current_shortcuts.voice_activation,
        shortcut
    );

    // Handle each shortcut type (use separate conditions to check all shortcuts)
    if let Some(stop_shortcut_obj) = stop_shortcut {
        if *shortcut == stop_shortcut_obj {
            handle_escape_key_shortcut(app, event);
        }
    }

    if let Some(agent_shortcut_obj) = agent_shortcut {
        if *shortcut == agent_shortcut_obj {
            handle_agent_mode_shortcut(app, event);
        }
    }

    // Check settings shortcut
    if let Some(settings_shortcut_obj) = settings_shortcut {
        if *shortcut == settings_shortcut_obj {
            handle_settings_shortcut(app, event);
        }
    }

    // Check dictation shortcut separately (not as else-if to avoid exclusion)
    if let Some(dictation_shortcut_obj) = dictation_shortcut {
        if *shortcut == dictation_shortcut_obj {
            handle_dictation_input_shortcut(app, event);
        }
    }

    // Check voice activation shortcut
    if let Some(voice_activation_obj) = voice_activation_shortcut {
        if *shortcut == voice_activation_obj {
            handle_voice_activation_shortcut(app, event);
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
                error!(
                    "[Settings Shortcut] Failed to open settings window: {}",
                    e
                );
            }
        });
    }
}

/// Handle escape key shortcut - universal "cancel anything" button
/// Uses the new escape key coordinator to prevent race conditions.
/// Always emits a visual feedback event; only triggers stop when not in onboarding.
fn handle_escape_key_shortcut(app: &AppHandle, event: &ShortcutEvent) {
    let shortcut_state = match event.state() {
        ShortcutState::Pressed => "pressed",
        ShortcutState::Released => "released",
    };

    // Always emit visual feedback event (for onboarding UI)
    if let Err(e) = app.emit(
        events::shortcuts::ESCAPE_KEY,
        serde_json::json!({
            "state": shortcut_state,
            "shortcut": "escape_key"
        }),
    ) {
        error!("[Escape Key] Failed to emit shortcut detection event: {}", e);
    }

    if event.state() == ShortcutState::Pressed {
        // During onboarding, only provide visual feedback — don't trigger stop.
        // The visual feedback event was already emitted above (line 121).
        let app_state = app.state::<state::AppState>();
        if app_state.is_onboarding_active() {
            info!("[Escape Key] Pressed during onboarding - visual feedback only");
            return;
        }

        info!("[Escape Key] Pressed - initiating coordinated stop");
        let app_handle_clone = app.clone();
        tauri::async_runtime::spawn(async move {
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
}

/// Handle agent mode shortcut (Option+D by default)
fn handle_agent_mode_shortcut(app: &AppHandle, event: &ShortcutEvent) {
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

    // During onboarding, only provide visual feedback — don't trigger agent mode
    let app_state = app.state::<state::AppState>();
    if app_state.is_onboarding_active() {
        info!("[Agent Mode Shortcut] Pressed during onboarding - visual feedback only");
        return;
    }

    // Unified behavior: forward both press and release to agent_monitor.
    // AgentMonitor will branch based on AgentTriggerMode (tap vs hold).
    let app_clone = app.clone();
    let event_state = event.state();
    tauri::async_runtime::spawn(async move {
        if event_state == ShortcutState::Pressed {
            crate::agent_monitor::on_agent_input_pressed().await;
        } else if event_state == ShortcutState::Released {
            crate::agent_monitor::on_agent_input_released(&app_clone).await;
        }
    });
}

// Removed handle_agent_tap_mode - unified through AgentMonitor

/// Handle dictation input shortcut (Option+Space by default)
fn handle_dictation_input_shortcut(app: &AppHandle, event: &ShortcutEvent) {
    let app_clone = app.clone();
    let event_state = event.state();

    // Emit shortcut detection events for visual feedback in onboarding
    let shortcut_state = match event_state {
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

    // During onboarding, only provide visual feedback — don't trigger dictation
    let app_state = app.state::<state::AppState>();
    if app_state.is_onboarding_active() {
        info!("[Dictation Input Shortcut] Pressed during onboarding - visual feedback only");
        return;
    }

    // FIXED: Check dictation trigger mode to determine behavior
    let trigger_mode = app_state
        .get_dictation_trigger_mode()
        .unwrap_or(state::DictationTriggerMode::Hold);

    info!("[Dictation Input Shortcut] Trigger mode: {:?}, event state: {:?}", trigger_mode, event.state());
    match trigger_mode {
        state::DictationTriggerMode::Tap => {
            // Tap mode: Only handle key release (press+release = tap)
            if event.state() == ShortcutState::Released {
                info!("[Dictation Input Shortcut] Tap mode - calling handle_dictation_tap_mode");
                handle_dictation_tap_mode(app);
            }
        }
        state::DictationTriggerMode::Hold => {
            // Hold mode: Handle both press and release, route to dictation_monitor
            info!("[Dictation Shortcut] Hold mode - spawning async handler for {:?}", event_state);
            tauri::async_runtime::spawn(async move {
                info!("[Dictation Shortcut] Async task started for {:?}", event_state);
                if event_state == ShortcutState::Pressed {
                    crate::dictation_monitor::on_dictation_input_pressed().await;
                } else if event_state == ShortcutState::Released {
                    crate::dictation_monitor::on_dictation_input_released(&app_clone).await;
                }
                info!("[Dictation Shortcut] Async task completed for {:?}", event_state);
            });
        }
    }
}

/// Handle dictation tap mode (new functionality)
fn handle_dictation_tap_mode(app: &AppHandle) {
    info!("[Dictation Tap Mode] Entered handle_dictation_tap_mode");

    // Check if dictation is currently active using AppState (no locking required)
    // This avoids the VoiceController mutex which can be held during audio processing
    let app_state = app.state::<state::AppState>();
    let is_dictation_active = app_state.is_dictation_active();

    info!("[Dictation Tap Mode] is_dictation_active (from AppState): {}", is_dictation_active);

    if is_dictation_active {
        info!("[Dictation Input Shortcut] Tap mode - stopping active dictation");

        let app_handle = app.clone();
        tauri::async_runtime::spawn(async move {
            // Stop dictation
            if let Some(voice_controller_state) =
                app_handle.try_state::<Arc<Mutex<VoiceController>>>()
            {
                match tauri_plugin_voice_transcription::commands::stop_dictation(
                    app_handle.clone(),
                    voice_controller_state,
                )
                .await
                {
                    Ok(_) => {
                        info!("[Dictation Tap Mode] Stopped dictation successfully");
                    }
                    Err(e) => {
                        error!("[Dictation Tap Mode] Failed to stop dictation: {}", e);
                    }
                }
            }
        });
    } else {
        info!("[Dictation Input Shortcut] Tap mode - starting dictation mode transcription");

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
        return Err(crate::format_error(templates::FAILED_TO_EMIT, "test event", e));
    }

    Ok(())
}

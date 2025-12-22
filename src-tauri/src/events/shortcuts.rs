//! # Global Shortcut Handler
//!
//! This module handles all global keyboard shortcuts for the Juno application,
//! including escape key handling, agent mode toggle, and dictation input shortcuts.

use std::sync::{Arc, Mutex};
use tauri::{AppHandle, Emitter, Manager};
use tauri_plugin_global_shortcut::{Code, Shortcut, ShortcutEvent, ShortcutState};
use tauri_plugin_voice_transcription::controller::VoiceController;
use tracing::{error, info};

use crate::state;
use crate::constants::{events, errors::templates};

// Helper function for error formatting - properly handles template substitution
fn format_error(template: &str, context: &str, error: impl std::fmt::Display) -> String {
    template.replacen("{}", context, 1).replacen("{}", &error.to_string(), 1)
}

/// Parse a shortcut string into a Shortcut object
pub fn parse_shortcut_string(shortcut_str: &str) -> Option<Shortcut> {
    crate::parse_shortcut_string(shortcut_str)
}

/// Handle global shortcut events
pub fn handle_global_shortcut(app: &AppHandle, shortcut: &Shortcut, event: &ShortcutEvent) {
    println!(
        "[GlobalShortcut Triggered] Shortcut: {:?}, State: {:?}",
        shortcut,
        event.state()
    );

    let app_state = app.state::<state::AppState>();

    // Get current keyboard shortcuts from state
    let current_shortcuts = match app_state.get_keyboard_shortcuts() {
        Ok(shortcuts) => shortcuts,
        Err(e) => {
            error!("{}", format_error(templates::FAILED_TO_RETRIEVE, "keyboard shortcuts", e));
            return; // Exit early if we can't get shortcuts
        }
    };

    // Create shortcut objects from current configuration
    let escape_shortcut = Shortcut::new(None, Code::Escape);

    // Parse shortcuts from configuration
    let agent_shortcut: Option<Shortcut> =
        parse_shortcut_string(&current_shortcuts.agent_mode_toggle);
    let dictation_shortcut: Option<Shortcut> =
        parse_shortcut_string(&current_shortcuts.dictation_input);
    let settings_shortcut: Option<Shortcut> =
        parse_shortcut_string(&current_shortcuts.open_settings);

    // Handle each shortcut type (use separate conditions to check all shortcuts)
    if *shortcut == escape_shortcut {
        handle_escape_key_shortcut(app, event);
    } else if let Some(agent_shortcut_obj) = agent_shortcut {
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
/// Uses the new escape key coordinator to prevent race conditions
fn handle_escape_key_shortcut(app: &AppHandle, event: &ShortcutEvent) {
    if event.state() == ShortcutState::Pressed {
        info!("[Escape Key] Pressed - initiating coordinated stop");

        let app_handle_clone = app.clone();
        tauri::async_runtime::spawn(async move {
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
            "shortcut": "agent_mode_toggle"
        }),
    ) {
        error!(
            "[Agent Mode Shortcut] Failed to emit shortcut detection event: {}",
            e
        );
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

    // FIXED: Check dictation trigger mode to determine behavior
    let app_state = app.state::<state::AppState>();
    let trigger_mode = app_state
        .get_dictation_trigger_mode()
        .unwrap_or(state::DictationTriggerMode::Hold);

    match trigger_mode {
        state::DictationTriggerMode::Tap => {
            // Tap mode: Only handle key release (press+release = tap)
            if event.state() == ShortcutState::Released {
                handle_dictation_tap_mode(app);
            }
        }
        state::DictationTriggerMode::Hold => {
            // Hold mode: Handle both press and release, route to dictation_monitor
            tauri::async_runtime::spawn(async move {
                if event_state == ShortcutState::Pressed {
                    crate::dictation_monitor::on_dictation_input_pressed().await;
                } else if event_state == ShortcutState::Released {
                    crate::dictation_monitor::on_dictation_input_released(&app_clone).await;
                }
            });
        }
    }
}

/// Handle dictation tap mode (new functionality)
fn handle_dictation_tap_mode(app: &AppHandle) {
    // Check if dictation is currently active by checking the VoiceController directly
    let is_dictation_active =
        if let Some(voice_controller_state) = app.try_state::<Arc<Mutex<VoiceController>>>() {
            voice_controller_state
                .lock()
                .map(|controller| controller.is_dictating())
                .unwrap_or(false)
        } else {
            false
        };

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

/// Add a new command to trigger shortcut testing events during onboarding
#[tauri::command]
pub async fn trigger_shortcut_test_event(
    app: AppHandle,
    shortcut_name: String,
    state: String,
) -> Result<(), String> {
    let event_name = match shortcut_name.as_str() {
        "agent_mode_toggle" => events::shortcuts::AGENT_MODE,
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
        return Err(format_error(templates::FAILED_TO_EMIT, "test event", e));
    }

    Ok(())
}

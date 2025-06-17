//! # Global Shortcut Handler
//!
//! This module handles all global keyboard shortcuts for the Juno application,
//! including escape key handling, agent mode toggle, and dictation input shortcuts.

use std::sync::{Arc, Mutex};
use tauri::{AppHandle, Emitter, Manager};
use tauri_plugin_global_shortcut::{Shortcut, Code, ShortcutState, ShortcutEvent};
use tauri_plugin_voice_transcription::controller::VoiceController;
use tracing::{info, error};

use crate::{constants, state};

/// Parse a shortcut string into a Shortcut object
pub fn parse_shortcut_string(shortcut_str: &str) -> Option<Shortcut> {
    crate::parse_shortcut_string(shortcut_str)
}

/// Handle global shortcut events
pub fn handle_global_shortcut(app: &AppHandle, shortcut: &Shortcut, event: &ShortcutEvent) {
    println!("[GlobalShortcut Triggered] Shortcut: {:?}, State: {:?}", shortcut, event.state());

    let app_state = app.state::<state::AppState>();

    // Get current keyboard shortcuts from state
    let current_shortcuts = match app_state.keyboard_shortcuts.lock() {
        Ok(shortcuts) => shortcuts.clone(),
        Err(e) => {
            error!("Failed to get keyboard shortcuts: {}", e);
            return; // Exit early if we can't get shortcuts
        }
    };

    // Create shortcut objects from current configuration
    let escape_shortcut = Shortcut::new(None, Code::Escape);

    // Parse shortcuts from configuration
    let agent_shortcut: Option<Shortcut> = parse_shortcut_string(&current_shortcuts.agent_mode_toggle);
    let dictation_shortcut: Option<Shortcut> = parse_shortcut_string(&current_shortcuts.dictation_input);

    // Handle each shortcut type
    if *shortcut == escape_shortcut {
        handle_escape_key_shortcut(app, event);
    } else if let Some(agent_shortcut_obj) = agent_shortcut {
        if *shortcut == agent_shortcut_obj {
            handle_agent_mode_shortcut(app, event);
        }
    } else if let Some(dictation_shortcut_obj) = dictation_shortcut {
        if *shortcut == dictation_shortcut_obj {
            handle_dictation_input_shortcut(app, event);
        }
    }
}

/// Handle escape key shortcut (primarily for cancelling dictation)
fn handle_escape_key_shortcut(app: &AppHandle, event: &ShortcutEvent) {
    if event.state() == ShortcutState::Pressed {
        info!("[Escape Key] Pressed - checking if dictation is active");

        let app_state = app.state::<state::AppState>();
        let is_dictation_active = app_state.dictation_active.lock()
            .map(|active| *active)
            .unwrap_or(false);

        if is_dictation_active {
            info!("[Escape Key] Cancelling active dictation");

            // Stop dictation using the voice controller
            let app_handle_clone = app.clone();
            tauri::async_runtime::spawn(async move {
                if let Some(voice_controller_state) = app_handle_clone.try_state::<Arc<Mutex<VoiceController>>>() {
                    match tauri_plugin_voice_transcription::commands::stop_dictation(
                        app_handle_clone.clone(),
                        voice_controller_state
                    ).await {
                        Ok(_) => {
                            info!("[Escape Key] Successfully stopped dictation");
                        }
                        Err(e) => {
                            error!("[Escape Key] Failed to stop dictation: {}", e);
                        }
                    }
                }
            });

            // Reset dictation state
            if let Ok(mut dictation_active) = app_state.dictation_active.lock() {
                *dictation_active = false;
            }

            // Emit state change event
            if let Err(e) = app.emit(constants::events::DICTATION_ACTIVE, false) {
                error!("[Escape Key] Failed to emit dictation-active event: {}", e);
            }

            // Update floating bar
            let app_handle_clone = app.clone();
            tauri::async_runtime::spawn(async move {
                crate::commands::floating_bar::handle_dictation_mode_change(&app_handle_clone, false).await;
            });
        } else {
            info!("[Escape Key] Pressed but dictation is not active - ignoring");
        }
    }
}

/// Handle agent mode shortcut (Option+D by default)
fn handle_agent_mode_shortcut(app: &AppHandle, event: &ShortcutEvent) {
    if event.state() == ShortcutState::Pressed {
        info!("[Agent Mode Shortcut] Pressed - starting agent mode transcription");

        let app_handle = app.clone();
        tauri::async_runtime::spawn(async move {
            // Emit agent mode start event
            if let Err(e) = app_handle.emit("app-dictation-started", ()) {
                error!("[Agent Mode] Failed to emit app-dictation-started event: {}", e);
            }

            // Start voice transcription for agent mode
            if let Some(voice_controller_state) = app_handle.try_state::<Arc<Mutex<VoiceController>>>() {
                match tauri_plugin_voice_transcription::commands::start_dictation(
                    app_handle.clone(),
                    voice_controller_state
                ).await {
                    Ok(_) => {
                        info!("[Agent Mode] Started transcription successfully");
                    }
                    Err(e) => {
                        error!("[Agent Mode] Failed to start transcription: {}", e);
                    }
                }
            }
        });
    }
}

/// Handle dictation input shortcut (Space bar by default)
fn handle_dictation_input_shortcut(app: &AppHandle, event: &ShortcutEvent) {
    let app_clone = app.clone();
    let event_state = event.state();

    tauri::async_runtime::spawn(async move {
        if event_state == ShortcutState::Pressed {
            crate::dictation_monitor::on_dictation_input_pressed().await;
        } else if event_state == ShortcutState::Released {
            crate::dictation_monitor::on_dictation_input_released(&app_clone).await;
        }

        // Update app state - Note: dictation_pressed field doesn't exist, but this is placeholder code
        // The real field would need to be added to AppState if needed

        // Emit event for frontend
        if let Err(e) = app_clone.emit("dictation-input-state-changed", event_state == ShortcutState::Pressed) {
            error!("Failed to emit dictation-input-state-changed event: {}", e);
        }
    });
}

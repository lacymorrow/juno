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

/// Handle escape key shortcut - universal "cancel anything" button
/// Only handles the key if we have registered it (meaning we have something to cancel)
fn handle_escape_key_shortcut(app: &AppHandle, event: &ShortcutEvent) {
    if event.state() == ShortcutState::Pressed {
        // First check if we should even be handling this escape key press
        // If the escape key isn't registered or has no users, let it pass through to other apps
        use std::sync::atomic::Ordering;
        let is_registered = crate::commands::shortcuts::ESCAPE_KEY_REGISTERED.load(Ordering::SeqCst);
        let user_count = crate::commands::shortcuts::ESCAPE_KEY_USERS.load(Ordering::SeqCst);

        if !is_registered || user_count == 0 {
            info!("[Escape Key] Pressed but not registered for cancellation (users: {}) - ignoring to let other apps handle it", user_count);
            return; // Let the escape key pass through to other applications
        }

        info!("[Escape Key] Pressed - checking for active operations to cancel (registered users: {})", user_count);

        let app_state = app.state::<state::AppState>();

        // Check what operations are currently active
        let is_dictation_active = app_state.dictation_active.lock()
            .map(|active| *active)
            .unwrap_or(false);

        let is_agent_active = !*app_state.cancel_rx.borrow();

        // Always listening status
        let is_always_listening_active = app_state.always_listening_active.lock()
            .map(|active| *active)
            .unwrap_or(false);

        // Always call comprehensive stop_all_operations for consistency
        // This ensures all operations are stopped regardless of state detection
        info!("[Escape Key] Cancelling all operations (detected states - dictation: {}, agent: {}, always_listening: {})",
              is_dictation_active, is_agent_active, is_always_listening_active);

        let app_handle_clone = app.clone();
        tauri::async_runtime::spawn(async move {
            match crate::commands::stop_operations::stop_all_operations(app_handle_clone).await {
                Ok(message) => {
                    info!("[Escape Key] Successfully stopped all operations: {}", message);
                }
                Err(e) => {
                    error!("[Escape Key] Failed to stop all operations: {}", e);
                }
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

    if let Err(e) = app.emit("shortcut-agent-mode", serde_json::json!({
        "state": shortcut_state,
        "shortcut": "agent_mode_toggle"
    })) {
        error!("[Agent Mode Shortcut] Failed to emit shortcut detection event: {}", e);
    }

    if event.state() == ShortcutState::Pressed {
        // Check if dictation is currently active by checking the VoiceController directly
        let is_dictation_active = if let Some(voice_controller_state) = app.try_state::<Arc<Mutex<VoiceController>>>() {
            voice_controller_state.lock()
                .map(|controller| controller.is_dictating())
                .unwrap_or(false)
        } else {
            false
        };

        if is_dictation_active {
            info!("[Agent Mode Shortcut] Pressed - stopping active dictation");

            let app_handle = app.clone();
            tauri::async_runtime::spawn(async move {
                // Stop dictation
                if let Some(voice_controller_state) = app_handle.try_state::<Arc<Mutex<VoiceController>>>() {
                    match tauri_plugin_voice_transcription::commands::stop_dictation(
                        app_handle.clone(),
                        voice_controller_state
                    ).await {
                        Ok(_) => {
                            info!("[Agent Mode] Stopped dictation successfully");
                        }
                        Err(e) => {
                            error!("[Agent Mode] Failed to stop dictation: {}", e);
                        }
                    }
                }
            });
        } else {
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
}

/// Handle dictation input shortcut (Space bar by default)
fn handle_dictation_input_shortcut(app: &AppHandle, event: &ShortcutEvent) {
    let app_clone = app.clone();
    let event_state = event.state();

    // Emit shortcut detection events for visual feedback in onboarding
    let shortcut_state = match event_state {
        ShortcutState::Pressed => "pressed",
        ShortcutState::Released => "released",
    };

    if let Err(e) = app.emit("shortcut-dictation-input", serde_json::json!({
        "state": shortcut_state,
        "shortcut": "dictation_input"
    })) {
        error!("[Dictation Input Shortcut] Failed to emit shortcut detection event: {}", e);
    }

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

/// Add a new command to trigger shortcut testing events during onboarding
#[tauri::command]
pub async fn trigger_shortcut_test_event(
    app: AppHandle,
    shortcut_name: String,
    state: String
) -> Result<(), String> {
    let event_name = match shortcut_name.as_str() {
        "agent_mode_toggle" => "shortcut-agent-mode",
        "dictation_input" => "shortcut-dictation-input",
        _ => return Err("Unknown shortcut name".to_string()),
    };

    if let Err(e) = app.emit(event_name, serde_json::json!({
        "state": state,
        "shortcut": shortcut_name,
        "test_mode": true
    })) {
        return Err(format!("Failed to emit test event: {}", e));
    }

    Ok(())
}

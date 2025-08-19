//! # Global Shortcut Handler
//!
//! This module handles all global keyboard shortcuts for the Juno application,
//! including escape key handling, agent mode toggle, and dictation input shortcuts.

use std::sync::{Arc, Mutex};
use tauri::{AppHandle, Emitter, Manager};
use tauri_plugin_global_shortcut::{Code, Shortcut, ShortcutEvent, ShortcutState};
use tauri_plugin_voice_transcription::controller::VoiceController;
use tracing::{error, info, warn};

use crate::state;
use crate::constants::{events, errors::templates};
use crate::commands::native_permissions::NativePermissionChecker;

// Helper function for error formatting - properly handles template substitution
fn format_error(template: &str, context: &str, error: impl std::fmt::Display) -> String {
    template.replacen("{}", context, 1).replacen("{}", &error.to_string(), 1)
}

/// Parse a shortcut string into a Shortcut object
pub fn parse_shortcut_string(shortcut_str: &str) -> Option<Shortcut> {
    crate::parse_shortcut_string(shortcut_str)
}

/// Check if we have necessary permissions for voice transcription
fn has_required_permissions() -> bool {
    // Check accessibility permission (required for voice transcription to work properly)
    match NativePermissionChecker::check_accessibility_permission() {
        Ok(granted) => {
            if !granted {
                warn!("[Permissions] Accessibility permission not granted - voice transcription will not work");
            }
            granted
        }
        Err(e) => {
            error!("[Permissions] Failed to check accessibility permission: {}", e);
            false // Assume not granted on error
        }
    }
}

/// Show notification when permissions are missing with action to open settings
fn show_permission_error_notification(app: &AppHandle) {
    let app_clone = app.clone();
    let app_clone2 = app.clone();
    
    // Emit toast notification for immediate visual feedback
    if let Err(e) = app_clone2.emit("show-toast", serde_json::json!({
        "title": "Permissions Required",
        "message": "Accessibility permission needed. Click to open Settings.",
        "type": "error",
        "duration": 5000,
        "action": "open_accessibility_settings"
    })) {
        info!("[Permissions] Could not emit toast event: {}", e);
    }
    
    tauri::async_runtime::spawn(async move {
        use tauri_plugin_notification::NotificationExt;
        
        // Also show system notification for redundancy
        // Note: macOS notifications don't support click actions directly,
        // but we can emit an event for the UI to show a dialog with a button
        if let Err(e) = app_clone.notification()
            .builder()
            .title("Permissions Required")
            .body("Accessibility permission needed. Please grant access in System Settings > Privacy & Security > Accessibility.")
            .show() 
        {
            info!("[Permissions] Could not show system notification: {}", e);
        }
        
        // Emit an event to the frontend to show a dialog with an "Open Settings" button
        // This gives the user control over when to open settings
        if let Err(e) = app_clone.emit("permissions-required", serde_json::json!({
            "type": "accessibility",
            "message": "Voice features require Accessibility permission",
            "action": "open_settings"
        })) {
            error!("[Permissions] Failed to emit permissions-required event: {}", e);
        }
        
        info!("[Permissions] Notified user about missing permissions - awaiting user action");
    });
}

/// Show immediate agent mode notification for better user feedback
fn show_agent_mode_notification(app: &AppHandle) {
    // Use both system notification and toast for immediate feedback
    // This provides instant visual feedback that the shortcut was recognized
    let app_clone = app.clone();
    let app_clone2 = app.clone();
    
    // Emit toast notification event for frontend
    if let Err(e) = app_clone2.emit("show-toast", serde_json::json!({
        "title": "Agent Mode",
        "message": "Listening...",
        "type": "info",
        "duration": 2000
    })) {
        info!("[Agent Mode] Could not emit toast event: {}", e);
    }
    
    tauri::async_runtime::spawn(async move {
        use tauri_plugin_notification::NotificationExt;
        
        // Also show system notification for users who prefer that
        // This notification appears immediately, before voice controller initialization
        if let Err(e) = app_clone.notification()
            .builder()
            .title("Agent Mode")
            .body("Listening...")
            .show() 
        {
            info!("[Agent Mode] Could not show system notification: {}", e);
            // Don't treat this as a critical error - the mode still works without the notification
        }
    });
}

/// Show immediate dictation mode notification for better user feedback
fn show_dictation_mode_notification(app: &AppHandle) {
    // Use both system notification and toast for immediate feedback
    // This provides instant visual feedback that the shortcut was recognized
    let app_clone = app.clone();
    let app_clone2 = app.clone();
    
    // Emit toast notification event for frontend
    if let Err(e) = app_clone2.emit("show-toast", serde_json::json!({
        "title": "Dictation Mode",
        "message": "Listening...",
        "type": "info",
        "duration": 2000
    })) {
        info!("[Dictation Mode] Could not emit toast event: {}", e);
    }
    
    tauri::async_runtime::spawn(async move {
        use tauri_plugin_notification::NotificationExt;
        
        // Also show system notification for users who prefer that
        // This notification appears immediately, before voice controller initialization
        if let Err(e) = app_clone.notification()
            .builder()
            .title("Dictation Mode")
            .body("Listening...")
            .show() 
        {
            info!("[Dictation Mode] Could not show system notification: {}", e);
            // Don't treat this as a critical error - the mode still works without the notification
        }
    });
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

    // Handle each shortcut type (use separate conditions to check all shortcuts)
    if *shortcut == escape_shortcut {
        handle_escape_key_shortcut(app, event);
    } else if let Some(agent_shortcut_obj) = agent_shortcut {
        if *shortcut == agent_shortcut_obj {
            handle_agent_mode_shortcut(app, event);
        }
    }

    // Check dictation shortcut separately (not as else-if to avoid exclusion)
    if let Some(dictation_shortcut_obj) = dictation_shortcut {
        if *shortcut == dictation_shortcut_obj {
            handle_dictation_input_shortcut(app, event);
        }
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

    // FIXED: Check agent trigger mode to determine behavior
    let app_state = app.state::<state::AppState>();
    let trigger_mode = app_state
        .get_agent_trigger_mode()
        .unwrap_or(state::AgentTriggerMode::Tap);

    match trigger_mode {
        state::AgentTriggerMode::Tap => {
            // Tap mode: Only handle key release (press+release = tap)
            if event.state() == ShortcutState::Released {
                handle_agent_tap_mode(app);
            }
        }
        state::AgentTriggerMode::Hold => {
            // Hold mode: Handle both press and release, route to agent_monitor
            let app_clone = app.clone();
            let event_state = event.state();
            
            // Check permissions and show appropriate notification on press
            if event_state == ShortcutState::Pressed {
                if !has_required_permissions() {
                    warn!("[Agent Mode] Cannot start - missing required permissions");
                    show_permission_error_notification(app);
                    return; // Don't try to initialize voice controller without permissions
                }
                show_agent_mode_notification(app);
            }
            
            tauri::async_runtime::spawn(async move {
                if event_state == ShortcutState::Pressed {
                    crate::agent_monitor::on_agent_input_pressed().await;
                } else if event_state == ShortcutState::Released {
                    crate::agent_monitor::on_agent_input_released(&app_clone).await;
                }
            });
        }
    }
}

/// Handle agent tap mode (original behavior)
fn handle_agent_tap_mode(app: &AppHandle) {
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
        info!("[Agent Mode Shortcut] Tap mode - stopping active dictation");

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
                        info!("[Agent Mode] Stopped dictation successfully");
                    }
                    Err(e) => {
                        error!("[Agent Mode] Failed to stop dictation: {}", e);
                    }
                }
            }
        });
    } else {
        info!("[Agent Mode Shortcut] Tap mode - starting agent mode transcription");

        // Check permissions first before trying to initialize voice controller
        if !has_required_permissions() {
            warn!("[Agent Mode] Cannot start - missing required permissions");
            show_permission_error_notification(app);
            return; // Don't try to initialize voice controller without permissions
        }

        // Immediately show notification for better user feedback
        show_agent_mode_notification(app);

        let app_handle = app.clone();
        tauri::async_runtime::spawn(async move {
            // Emit agent transcription start event
            // This will be handled by the event listener in integration.rs
            if let Err(e) = app_handle.emit(events::agent::TRANSCRIPTION_START, ()) {
                error!(
                    "[Agent Mode] Failed to emit agent transcription start event: {}",
                    e
                );
            }
            
            info!("[Agent Mode] Emitted agent start event for handler processing");
        });
    }
}

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
            
            // Check permissions and show appropriate notification on press
            if event_state == ShortcutState::Pressed {
                if !has_required_permissions() {
                    warn!("[Dictation Mode] Cannot start - missing required permissions");
                    show_permission_error_notification(&app_clone);
                    return; // Don't try to initialize voice controller without permissions
                }
                show_dictation_mode_notification(&app_clone);
            }
            
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

        // Check permissions first before trying to initialize voice controller
        if !has_required_permissions() {
            warn!("[Dictation Mode] Cannot start - missing required permissions");
            show_permission_error_notification(app);
            return; // Don't try to initialize voice controller without permissions
        }

        // Immediately show notification for better user feedback
        show_dictation_mode_notification(app);

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

/// Command for frontend to open accessibility settings when user clicks the button
#[tauri::command]
pub async fn open_accessibility_settings_on_request() -> Result<(), String> {
    info!("[Permissions] User requested to open accessibility settings");
    
    // Use the existing command to open the settings
    match crate::commands::permissions::open_system_preferences("accessibility".to_string()).await {
        Ok(_) => {
            info!("[Permissions] Successfully opened Accessibility settings per user request");
            Ok(())
        }
        Err(e) => {
            error!("[Permissions] Failed to open Accessibility settings: {}", e);
            Err(format!("Failed to open System Settings: {}", e))
        }
    }
}

//! # Mode Keyboard Shortcuts
//! 
//! Simplified keyboard shortcut handlers for mode transitions

use tauri::{AppHandle, Manager};
use tauri_plugin_global_shortcut::{Shortcut, ShortcutEvent, ShortcutState};
use tracing::{error, info};

use crate::mode_manager::{AppMode, get_mode_manager};
use crate::state;

/// Handle global shortcut events for modes
pub fn handle_mode_shortcut(app: &AppHandle, shortcut: &Shortcut, event: &ShortcutEvent) {
    let app_state = app.state::<state::AppState>();

    // Get current keyboard shortcuts from state
    let shortcuts = match app_state.get_keyboard_shortcuts() {
        Ok(s) => s,
        Err(e) => {
            error!("Failed to get keyboard shortcuts: {}", e);
            return;
        }
    };

    // Parse all shortcuts from configuration (including stop_current_task)
    let stop_shortcut = crate::parse_shortcut_string(&shortcuts.stop_current_task);
    let agent_shortcut = crate::parse_shortcut_string(&shortcuts.agent_mode);
    let dictation_shortcut = crate::parse_shortcut_string(&shortcuts.dictation_input);

    // Handle stop shortcut - always transitions to idle
    if let Some(stop) = stop_shortcut {
        if *shortcut == stop && event.state() == ShortcutState::Pressed {
            handle_escape_key(app);
            return;
        }
    }

    // Handle agent mode shortcut
    if let Some(agent) = agent_shortcut {
        if *shortcut == agent {
            handle_agent_shortcut(app, event);
            return;
        }
    }

    // Handle dictation mode shortcut
    if let Some(dictation) = dictation_shortcut {
        if *shortcut == dictation {
            handle_dictation_shortcut(app, event);
        }
    }
}

/// Handle escape key - universal cancel
fn handle_escape_key(app: &AppHandle) {
    info!("[Escape] Cancelling current mode");
    
    let app_handle = app.clone();
    tauri::async_runtime::spawn(async move {
        if let Err(e) = get_mode_manager()
            .transition_to(AppMode::Idle, "Escape key pressed".to_string(), &app_handle)
            .await
        {
            error!("Failed to cancel mode: {}", e);
        }
    });
}

/// Handle agent mode shortcut
fn handle_agent_shortcut(app: &AppHandle, event: &ShortcutEvent) {
    let app_state = app.state::<state::AppState>();
    let trigger_mode = app_state
        .get_agent_trigger_mode()
        .unwrap_or(state::AgentTriggerMode::Tap);

    match trigger_mode {
        state::AgentTriggerMode::Tap => {
            if event.state() == ShortcutState::Released {
                toggle_agent_mode(app);
            }
        }
        state::AgentTriggerMode::Hold => {
            match event.state() {
                ShortcutState::Pressed => start_agent_mode(app),
                ShortcutState::Released => stop_agent_mode(app),
            }
        }
    }
}

/// Handle dictation mode shortcut
fn handle_dictation_shortcut(app: &AppHandle, event: &ShortcutEvent) {
    let app_state = app.state::<state::AppState>();
    let trigger_mode = app_state
        .get_dictation_trigger_mode()
        .unwrap_or(state::DictationTriggerMode::Hold);

    match trigger_mode {
        state::DictationTriggerMode::Tap => {
            if event.state() == ShortcutState::Released {
                toggle_dictation_mode(app);
            }
        }
        state::DictationTriggerMode::Hold => {
            match event.state() {
                ShortcutState::Pressed => start_dictation_mode(app),
                ShortcutState::Released => stop_dictation_mode(app),
            }
        }
    }
}

/// Toggle agent mode on/off
fn toggle_agent_mode(app: &AppHandle) {
    let app_handle = app.clone();
    tauri::async_runtime::spawn(async move {
        let mode_manager = get_mode_manager();
        let current = mode_manager.get_mode().await;
        
        let target_mode = if current == AppMode::Agent {
            AppMode::Idle
        } else {
            AppMode::Agent
        };
        
        if let Err(e) = mode_manager
            .transition_to(target_mode, "Keyboard shortcut (tap)".to_string(), &app_handle)
            .await
        {
            error!("Failed to toggle agent mode: {}", e);
        }
    });
}

/// Start agent mode
fn start_agent_mode(app: &AppHandle) {
    let app_handle = app.clone();
    tauri::async_runtime::spawn(async move {
        if let Err(e) = get_mode_manager()
            .transition_to(AppMode::Agent, "Keyboard shortcut (hold)".to_string(), &app_handle)
            .await
        {
            error!("Failed to start agent mode: {}", e);
        }
    });
}

/// Stop agent mode
fn stop_agent_mode(app: &AppHandle) {
    let app_handle = app.clone();
    tauri::async_runtime::spawn(async move {
        let current = get_mode_manager().get_mode().await;
        if current == AppMode::Agent {
            if let Err(e) = get_mode_manager()
                .transition_to(AppMode::Idle, "Keyboard shortcut released".to_string(), &app_handle)
                .await
            {
                error!("Failed to stop agent mode: {}", e);
            }
        }
    });
}

/// Toggle dictation mode on/off
fn toggle_dictation_mode(app: &AppHandle) {
    let app_handle = app.clone();
    tauri::async_runtime::spawn(async move {
        let mode_manager = get_mode_manager();
        let current = mode_manager.get_mode().await;
        
        let target_mode = if current == AppMode::Dictation {
            AppMode::Idle
        } else {
            AppMode::Dictation
        };
        
        if let Err(e) = mode_manager
            .transition_to(target_mode, "Keyboard shortcut (tap)".to_string(), &app_handle)
            .await
        {
            error!("Failed to toggle dictation mode: {}", e);
        }
    });
}

/// Start dictation mode
fn start_dictation_mode(app: &AppHandle) {
    let app_handle = app.clone();
    tauri::async_runtime::spawn(async move {
        if let Err(e) = get_mode_manager()
            .transition_to(AppMode::Dictation, "Keyboard shortcut (hold)".to_string(), &app_handle)
            .await
        {
            error!("Failed to start dictation mode: {}", e);
        }
    });
}

/// Stop dictation mode
fn stop_dictation_mode(app: &AppHandle) {
    let app_handle = app.clone();
    tauri::async_runtime::spawn(async move {
        let current = get_mode_manager().get_mode().await;
        if current == AppMode::Dictation {
            if let Err(e) = get_mode_manager()
                .transition_to(AppMode::Idle, "Keyboard shortcut released".to_string(), &app_handle)
                .await
            {
                error!("Failed to stop dictation mode: {}", e);
            }
        }
    });
}
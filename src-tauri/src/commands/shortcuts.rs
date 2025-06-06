// Commands for managing keyboard shortcuts configuration

use crate::state::{AppState, KeyboardShortcuts};
use tauri::{State, AppHandle};
use tauri_plugin_global_shortcut::GlobalShortcutExt;
use tauri_plugin_store::StoreExt;
use tracing::{info, error, warn};
use serde_json;

/// Get the current keyboard shortcuts configuration
#[tauri::command]
pub async fn get_keyboard_shortcuts(
    state: State<'_, AppState>,
) -> Result<KeyboardShortcuts, String> {
    let shortcuts = state.keyboard_shortcuts.lock()
        .map_err(|e| format!("Failed to lock keyboard shortcuts: {}", e))?
        .clone();

    Ok(shortcuts)
}

/// Update a specific keyboard shortcut
#[tauri::command]
pub async fn set_keyboard_shortcut(
    app: AppHandle,
    state: State<'_, AppState>,
    shortcut_name: String,
    shortcut_value: String,
) -> Result<(), String> {
    // Validate the shortcut format
    validate_shortcut_format(&shortcut_value)?;

    // Update the shortcut in state
    {
        let mut shortcuts = state.keyboard_shortcuts.lock()
            .map_err(|e| format!("Failed to lock keyboard shortcuts: {}", e))?;

        match shortcut_name.as_str() {
            "agent_mode_toggle" => shortcuts.agent_mode_toggle = shortcut_value.clone(),
            "dictation_input" => shortcuts.dictation_input = shortcut_value.clone(),
            "stop_current_task" => shortcuts.stop_current_task = shortcut_value.clone(),
            "open_settings" => return Err("The settings shortcut cannot be changed".to_string()),
            _ => return Err(format!("Unknown shortcut name: {}", shortcut_name)),
        }
    }

    // Save to persistent storage
    save_shortcuts_to_store(&app, &state).await?;

    // Re-register global shortcuts with new values
    update_global_shortcuts(&app, &state).await?;

    info!("Updated keyboard shortcut '{}' to '{}'", shortcut_name, shortcut_value);
    Ok(())
}

/// Update multiple keyboard shortcuts at once
#[tauri::command]
pub async fn set_keyboard_shortcuts(
    app: AppHandle,
    state: State<'_, AppState>,
    shortcuts: KeyboardShortcuts,
) -> Result<(), String> {
    // Validate all shortcuts
    validate_shortcut_format(&shortcuts.agent_mode_toggle)?;
    validate_shortcut_format(&shortcuts.dictation_input)?;
    validate_shortcut_format(&shortcuts.stop_current_task)?;
    validate_shortcut_format(&shortcuts.open_settings)?;

    // Update state
    {
        let mut current_shortcuts = state.keyboard_shortcuts.lock()
            .map_err(|e| format!("Failed to lock keyboard shortcuts: {}", e))?;
        *current_shortcuts = shortcuts.clone();
    }

    // Save to persistent storage
    save_shortcuts_to_store(&app, &state).await?;

    // Re-register global shortcuts
    update_global_shortcuts(&app, &state).await?;

    info!("Updated all keyboard shortcuts");
    Ok(())
}

/// Reset keyboard shortcuts to defaults
#[tauri::command]
pub async fn reset_keyboard_shortcuts(
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<(), String> {
    let default_shortcuts = KeyboardShortcuts::default();

    // Update state
    {
        let mut shortcuts = state.keyboard_shortcuts.lock()
            .map_err(|e| format!("Failed to lock keyboard shortcuts: {}", e))?;
        *shortcuts = default_shortcuts.clone();
    }

    // Save to persistent storage
    save_shortcuts_to_store(&app, &state).await?;

    // Re-register global shortcuts
    update_global_shortcuts(&app, &state).await?;

    info!("Reset keyboard shortcuts to defaults");
    Ok(())
}

/// Load keyboard shortcuts from persistent storage
pub async fn load_shortcuts_from_store(app: &AppHandle, state: &AppState) -> Result<(), String> {
    let store = app.store("keyboard_shortcuts.json")
        .map_err(|e| format!("Failed to access shortcuts store: {}", e))?;

        // Try to load each shortcut from the store
    let agent_mode_toggle = store.get("agent_mode_toggle")
        .and_then(|v| v.as_str().map(|s| s.to_string()));

    let dictation_input = store.get("dictation_input")
        .and_then(|v| v.as_str().map(|s| s.to_string()));

    let stop_current_task = store.get("stop_current_task")
        .and_then(|v| v.as_str().map(|s| s.to_string()));

    let open_settings = store.get("open_settings")
        .and_then(|v| v.as_str().map(|s| s.to_string()));

    // If any shortcuts exist in store, use them, otherwise use defaults
    if agent_mode_toggle.is_some() || dictation_input.is_some() ||
       stop_current_task.is_some() || open_settings.is_some() {

        let defaults = KeyboardShortcuts::default();
        let loaded_shortcuts = KeyboardShortcuts {
            agent_mode_toggle: agent_mode_toggle.unwrap_or(defaults.agent_mode_toggle),
            dictation_input: dictation_input.unwrap_or(defaults.dictation_input),
            stop_current_task: stop_current_task.unwrap_or(defaults.stop_current_task),
            open_settings: open_settings.unwrap_or(defaults.open_settings),
        };

        let mut shortcuts = state.keyboard_shortcuts.lock()
            .map_err(|e| format!("Failed to lock keyboard shortcuts: {}", e))?;
        *shortcuts = loaded_shortcuts;
        info!("Loaded keyboard shortcuts from store");
    } else {
        info!("No shortcuts found in store, using defaults");
    }

    Ok(())
}

/// Save keyboard shortcuts to persistent storage
async fn save_shortcuts_to_store(app: &AppHandle, state: &AppState) -> Result<(), String> {
    let store = app.store("keyboard_shortcuts.json")
        .map_err(|e| format!("Failed to access shortcuts store: {}", e))?;

    let shortcuts = state.keyboard_shortcuts.lock()
        .map_err(|e| format!("Failed to lock keyboard shortcuts: {}", e))?
        .clone();

        // Store each shortcut individually
    store.set("agent_mode_toggle", serde_json::Value::String(shortcuts.agent_mode_toggle));
    store.set("dictation_input", serde_json::Value::String(shortcuts.dictation_input));
    store.set("stop_current_task", serde_json::Value::String(shortcuts.stop_current_task));
    store.set("open_settings", serde_json::Value::String(shortcuts.open_settings));

    // Save the store to disk
    store.save()
        .map_err(|e| format!("Failed to save shortcuts store: {}", e))?;

    info!("Saved keyboard shortcuts to store");
    Ok(())
}



/// Validate shortcut format (basic validation)
fn validate_shortcut_format(shortcut: &str) -> Result<(), String> {
    if shortcut.trim().is_empty() {
        return Err("Shortcut cannot be empty".to_string());
    }

    // Basic validation for common modifier keys and key combinations
    let valid_modifiers = ["Cmd", "Ctrl", "Alt", "Option", "Shift"];
    let parts: Vec<&str> = shortcut.split('+').collect();

    if parts.is_empty() {
        return Err("Invalid shortcut format".to_string());
    }

    // If there are multiple parts, all but the last should be valid modifiers
    for part in &parts[..parts.len().saturating_sub(1)] {
        if !valid_modifiers.contains(&part.trim()) {
            return Err(format!("Invalid modifier key: {}", part.trim()));
        }
    }

    Ok(())
}

/// Re-register global shortcuts with current values
pub async fn update_global_shortcuts(app: &AppHandle, state: &AppState) -> Result<(), String> {
    // Unregister existing shortcuts
    if let Err(e) = app.global_shortcut().unregister_all() {
        warn!("Failed to unregister existing shortcuts: {}", e);
    }

    let shortcuts = state.keyboard_shortcuts.lock()
        .map_err(|e| format!("Failed to lock keyboard shortcuts: {}", e))?
        .clone();

    // Import parse_shortcut_string from lib.rs
    use crate::parse_shortcut_string;

    // Register the agent mode toggle shortcut
    if let Some(shortcut) = parse_shortcut_string(&shortcuts.agent_mode_toggle) {
        if let Err(e) = app.global_shortcut().register(shortcut) {
            error!("Failed to register agent mode toggle shortcut: {}", e);
        } else {
            info!("Registered agent mode toggle shortcut: {}", shortcuts.agent_mode_toggle);
        }
    } else {
        warn!("Failed to parse agent mode toggle shortcut: {}", shortcuts.agent_mode_toggle);
    }

    // Register the dictation input shortcut
    if let Some(shortcut) = parse_shortcut_string(&shortcuts.dictation_input) {
        if let Err(e) = app.global_shortcut().register(shortcut) {
            error!("Failed to register dictation input shortcut: {}", e);
        } else {
            info!("Registered dictation input shortcut: {}", shortcuts.dictation_input);
        }
    } else {
        warn!("Failed to parse dictation input shortcut: {}", shortcuts.dictation_input);
    }

    // Note: Escape key is registered dynamically during agent execution
    // Settings shortcut is handled by the menu system

    info!("Updated global shortcut registrations");
    Ok(())
}

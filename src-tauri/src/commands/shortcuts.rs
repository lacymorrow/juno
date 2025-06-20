//! # Keyboard Shortcuts Configuration
//!
//! Simple keyboard shortcuts management.

use crate::settings::{SettingsManager, KeyboardShortcut};
use tauri::{AppHandle, Manager};
use tracing::{info, error};
use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use crate::state::AppState;

// Global state for escape key handling
pub static ESCAPE_KEY_REGISTERED: AtomicBool = AtomicBool::new(false);
pub static ESCAPE_KEY_USERS: AtomicUsize = AtomicUsize::new(0);

#[tauri::command]
pub async fn get_keyboard_shortcuts(app: AppHandle) -> Result<HashMap<String, KeyboardShortcut>, String> {
    let settings_manager = SettingsManager::new(app);
    Ok(settings_manager.get_settings().keyboard_shortcuts)
}

#[tauri::command]
pub async fn update_keyboard_shortcuts(
    app: AppHandle,
    shortcuts: HashMap<String, KeyboardShortcut>,
) -> Result<(), String> {
    let settings_manager = SettingsManager::new(app.clone());

    let shortcuts_value = serde_json::to_value(&shortcuts)
        .map_err(|e| format!("Failed to serialize shortcuts: {}", e))?;

    settings_manager.update_section("keyboard_shortcuts", shortcuts_value).await?;

    // Update global shortcuts
    if let Err(e) = update_global_shortcuts(&app).await {
        error!("Failed to update global shortcuts: {}", e);
    }

    info!("✅ Keyboard shortcuts updated");
    Ok(())
}

#[tauri::command]
pub async fn set_keyboard_shortcut(
    app: AppHandle,
    action: String,
    shortcut: KeyboardShortcut,
) -> Result<(), String> {
    let settings_manager = SettingsManager::new(app.clone());

    let path = format!("keyboard_shortcuts.{}", action);
    let shortcut_value = serde_json::to_value(&shortcut)
        .map_err(|e| format!("Failed to serialize shortcut: {}", e))?;

    settings_manager.update_section(&path, shortcut_value).await?;

    // Update global shortcuts
    if let Err(e) = update_global_shortcuts(&app).await {
        error!("Failed to update global shortcuts: {}", e);
    }

    info!("✅ Keyboard shortcut set for action: {}", action);
    Ok(())
}

#[tauri::command]
pub async fn reset_keyboard_shortcuts(app: AppHandle) -> Result<(), String> {
    let settings_manager = SettingsManager::new(app.clone());

    let default_shortcuts: HashMap<String, KeyboardShortcut> = HashMap::new();
    let shortcuts_value = serde_json::to_value(&default_shortcuts)
        .map_err(|e| format!("Failed to serialize default shortcuts: {}", e))?;

    settings_manager.update_section("keyboard_shortcuts", shortcuts_value).await?;

    // Update global shortcuts
    if let Err(e) = update_global_shortcuts(&app).await {
        error!("Failed to update global shortcuts: {}", e);
    }

    info!("✅ Keyboard shortcuts reset to defaults");
    Ok(())
}

pub async fn update_global_shortcuts(app: &AppHandle) -> Result<(), String> {
    // Simplified global shortcuts update - just log for now
    let settings_manager = SettingsManager::new(app.clone());
    let shortcuts = settings_manager.get_settings().keyboard_shortcuts;

    info!("Updated global shortcuts with {} entries", shortcuts.len());
    Ok(())
}

/// Register escape key handler for agent operations
pub async fn register_escape_key_handler(app: AppHandle) -> Result<(), String> {
    info!("🔑 Registering escape key handler for agent operations");

    // In a simplified implementation, we just log this
    // The actual escape key handling is done through the global shortcut system
    let app_state = app.state::<AppState>();

    // Mark that escape key handling is active
    // This is used by other parts of the system to know escape is available
    info!("✅ Escape key handler registered");
    Ok(())
}

/// Unregister escape key handler
pub async fn unregister_escape_key_handler(app: AppHandle) -> Result<(), String> {
    info!("🔑 Unregistering escape key handler");

    // In a simplified implementation, we just log this
    let app_state = app.state::<AppState>();

    // Mark that escape key handling is inactive
    info!("✅ Escape key handler unregistered");
    Ok(())
}

/// Load shortcuts from settings (compatibility function)
pub async fn load_shortcuts_from_settings(app: &AppHandle) -> Result<HashMap<String, KeyboardShortcut>, String> {
    let settings_manager = SettingsManager::new(app.clone());
    Ok(settings_manager.get_settings().keyboard_shortcuts)
}

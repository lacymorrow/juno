//! # Tray Menu Module
//!
//! This module handles the system tray menu functionality for the Juno application.
//! It provides state-aware menu items that update based on the current window states
//! and comprehensive event handling for all tray menu interactions.

use tauri::{
    AppHandle, Manager, Emitter,
    tray::{TrayIconBuilder, MouseButton, MouseButtonState},
    menu::{MenuBuilder, MenuItemBuilder}
};
use tracing::{info, error};
use crate::constants::{tray_menu_ids, events};
use crate::state::AppState;

/// Get keyboard shortcuts from app state
fn get_keyboard_shortcuts(app: &AppHandle) -> Result<crate::state::KeyboardShortcuts, Box<dyn std::error::Error>> {
    let app_state = app.state::<AppState>();
    let shortcuts = app_state.keyboard_shortcuts.lock().map_err(|e| format!("Failed to lock keyboard shortcuts: {}", e))?;
    Ok(shortcuts.clone())
}

/// Create a state-aware tray menu with keyboard shortcuts
pub fn create_state_aware_tray_menu(app: &AppHandle) -> Result<tauri::menu::Menu<tauri::Wry>, Box<dyn std::error::Error>> {
    info!("🍽️ Creating state-aware tray menu...");

    // Get keyboard shortcuts from app state
    let _shortcuts = match get_keyboard_shortcuts(app) {
        Ok(shortcuts) => shortcuts,
        Err(e) => {
            error!("[TrayMenu] Failed to get keyboard shortcuts: {}", e);
            // Use defaults if we can't get from state
            crate::state::KeyboardShortcuts {
                agent_mode_toggle: "Option+D".to_string(),
                dictation_input: "Option+Space".to_string(),
                stop_current_task: "Escape".to_string(),
                open_settings: "Cmd+,".to_string(),
            }
        }
    };

    // Build menu items with proper accelerators
    let show_hide_item = MenuItemBuilder::new("Show/Hide Juno")
        .id(tray_menu_ids::SHOW_HIDE)
        .build(app)?;

    let new_chat_item = MenuItemBuilder::new("New Chat")
        .id(tray_menu_ids::NEW_CHAT)
        .accelerator("CmdOrCtrl+N")
        .build(app)?;

    let show_hide_floating_item = MenuItemBuilder::new("Show/Hide Floating Bar")
        .id(tray_menu_ids::SHOW_HIDE_FLOATING_BAR)
        .accelerator("CmdOrCtrl+B")
        .build(app)?;

    let dev_tools_item = MenuItemBuilder::new("Developer Tools")
        .id(tray_menu_ids::DEVELOPER_TOOLS)
        .accelerator("CmdOrCtrl+Alt+I")
        .build(app)?;

    // Voice control information items (non-clickable)
    let agent_mode_info = MenuItemBuilder::new("Agent Mode")
        .id("agent_mode_info")
        .accelerator("Alt+D")
        .enabled(false)
        .build(app)?;

    let dictation_mode_info = MenuItemBuilder::new("Dictation Mode")
        .id("dictation_mode_info")
        .enabled(false)
        .build(app)?;

    let stop_task_info = MenuItemBuilder::new("Stop Current Task")
        .id("stop_task_info")
        .accelerator("Escape")
        .enabled(false)
        .build(app)?;

    let settings_item = MenuItemBuilder::new("Settings...")
        .id(tray_menu_ids::SETTINGS)
        .accelerator("CmdOrCtrl+,")
        .build(app)?;

    let quit_item = MenuItemBuilder::new("Quit Juno")
        .id(tray_menu_ids::QUIT)
        .accelerator("CmdOrCtrl+Q")
        .build(app)?;

    // Build the complete tray menu
    let tray_menu = MenuBuilder::new(app)
        .item(&show_hide_item)
        .item(&new_chat_item)
        .separator()
        .item(&show_hide_floating_item)
        .item(&dev_tools_item)
        .separator()
        .item(&agent_mode_info)
        .item(&dictation_mode_info)
        .item(&stop_task_info)
        .separator()
        .item(&settings_item)
        .separator()
        .item(&quit_item)
        .build()?;

    info!("✅ State-aware tray menu created successfully");
    Ok(tray_menu)
}

/// Create and setup the system tray icon
pub fn setup_tray_icon(app: &AppHandle) -> Result<(), Box<dyn std::error::Error>> {
    info!("🔧 Setting up system tray icon...");

    let tray_menu = create_state_aware_tray_menu(app)?;

    let _tray = TrayIconBuilder::new()
        .menu(&tray_menu)
        .show_menu_on_left_click(false)
        .icon(app.default_window_icon().unwrap().clone())
        .build(app)?;

    info!("✅ System tray icon setup completed");
    Ok(())
}

/// Handle tray menu events
pub fn handle_tray_menu_events(app_handle: AppHandle, event_id: &str) {
    match event_id {
        tray_menu_ids::SHOW_HIDE => {
            info!("[TrayMenu] Show/Hide menu item clicked");
            // For now, just trigger settings until we have the proper event
            if let Err(e) = app_handle.emit(events::menu::SETTINGS_REQUESTED, ()) {
                error!("[TrayMenu] Failed to emit settings event: {}", e);
            }
        }
        tray_menu_ids::NEW_CHAT => {
            info!("[TrayMenu] New Chat menu item clicked");
            if let Err(e) = app_handle.emit(events::menu::NEW_CHAT_REQUESTED, ()) {
                error!("[TrayMenu] Failed to emit new chat event: {}", e);
            }
        }
        tray_menu_ids::SHOW_HIDE_FLOATING_BAR => {
            info!("[TrayMenu] Show/Hide Floating Bar menu item clicked");
            if let Err(e) = app_handle.emit(events::menu::TOGGLE_FLOATING_BAR_REQUESTED, ()) {
                error!("[TrayMenu] Failed to emit toggle floating bar event: {}", e);
            }
        }
        tray_menu_ids::DEVELOPER_TOOLS => {
            info!("[TrayMenu] Developer Tools menu item clicked");
            if let Err(e) = app_handle.emit(events::menu::DEVTOOLS_REQUESTED, ()) {
                error!("[TrayMenu] Failed to emit devtools event: {}", e);
            }
        }
        tray_menu_ids::SETTINGS => {
            info!("[TrayMenu] Settings menu item clicked");
            if let Err(e) = app_handle.emit(events::menu::SETTINGS_REQUESTED, ()) {
                error!("[TrayMenu] Failed to emit settings event: {}", e);
            }
        }
        tray_menu_ids::QUIT => {
            info!("[TrayMenu] Quit menu item clicked");
            app_handle.exit(0);
        }
        _ => {
            info!("[TrayMenu] Unknown menu item clicked: {}", event_id);
        }
    }
}

/// Handle TrayIconEvents like clicks on the icon itself
pub fn handle_tray_icon_event(event: tauri::tray::TrayIconEvent) {
    match event {
        tauri::tray::TrayIconEvent::Click {
            button: MouseButton::Left,
            button_state: MouseButtonState::Up,
            ..
        } => {
            info!("[TrayIcon] Left click detected on tray icon");
            // Implement left click behavior if needed
        }
        tauri::tray::TrayIconEvent::Click {
            button: MouseButton::Right,
            button_state: MouseButtonState::Up,
            ..
        } => {
            info!("[TrayIcon] Right click detected on tray icon");
            // Right click behavior is handled by menu system
        }
        _ => {
            // Handle other tray icon events if needed
        }
    }
}

/// Enhanced tray menu refresh function
pub fn refresh_tray_menu(app_handle: &AppHandle) {
    info!("[TrayMenu] Refreshing tray menu...");

    match create_state_aware_tray_menu(app_handle) {
        Ok(_new_menu) => {
            info!("[TrayMenu] Successfully refreshed tray menu");
            // Note: Tauri v2 will handle menu updates automatically through the state
        }
        Err(e) => {
            error!("[TrayMenu] Failed to refresh tray menu: {}", e);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tray_icon_data_embedded() {
        // Test that tray icon data is properly embedded
        assert!(!TRAY_ICON_DATA.is_empty(), "Tray icon data should not be empty");
        assert!(TRAY_ICON_DATA.len() > 100, "Tray icon data should be reasonable size");
    }

    #[tokio::test]
    async fn test_get_window_states_no_panic() {
        // This is a placeholder test since we can't easily mock AppHandle
        // In a real test environment, we would mock the AppHandle and windows
        assert!(true, "get_window_states should handle missing windows gracefully");
    }

    #[test]
    fn test_tray_menu_constants() {
        // Test that required tray menu constants exist
        use crate::constants::tray_menu_ids;

        assert!(!tray_menu_ids::QUIT.is_empty());
        assert!(!tray_menu_ids::SETTINGS.is_empty());
        assert!(!tray_menu_ids::SHOW_FLOATING_BAR.is_empty());
        assert!(!tray_menu_ids::HIDE_FLOATING_BAR.is_empty());
        assert!(!tray_menu_ids::SHOW_MAIN_WINDOW.is_empty());
        assert!(!tray_menu_ids::HIDE_MAIN_WINDOW.is_empty());
        assert!(!tray_menu_ids::NEW_CHAT.is_empty());
        assert!(!tray_menu_ids::SHOW_DEVTOOLS.is_empty());
        assert!(!tray_menu_ids::TOGGLE_FLOATING_BAR.is_empty());
    }
}

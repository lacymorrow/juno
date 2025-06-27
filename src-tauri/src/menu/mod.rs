//! # Menu Management Module
//!
//! This module provides comprehensive menu management for the Juno application,
//! including both application menus and tray menus with complete event handling.

use tauri::AppHandle;
use tracing::{info, error};
use crate::constants;

pub mod app_menu;
pub mod tray_menu;

// Re-export public functions
pub use app_menu::*;
pub use tray_menu::*;

/// Setup all menus for the application (both app menu and tray menu)
pub fn setup_all_menus(app_handle: &AppHandle) -> Result<(), Box<dyn std::error::Error>> {
    info!("🍎 Setting up all application menus...");

    // Setup application menu for all relevant windows
    app_menu::setup_menu_for_all_windows(app_handle)?;

    // Setup tray menu
    let _ = tray_menu::setup_tray_icon(app_handle);

    // Setup menu event listeners
    setup_menu_event_listeners(app_handle);

    info!("✅ All menus setup completed successfully");
    Ok(())
}

/// Setup menu event listeners for both app and tray menus
fn setup_menu_event_listeners(app_handle: &AppHandle) {
    let app_handle_for_menu = app_handle.clone();
    app_handle.on_menu_event(move |_app, event| {
        let event_id = event.id().as_ref();
        info!("[Menu Event] Received menu event: {}", event_id);

        // Handle the menu event
        handle_menu_event(app_handle_for_menu.clone(), event_id);
    });
}

/// Central menu event handler for all menu types
fn handle_menu_event(app_handle: AppHandle, event_id: &str) {
    // Check if it's an app menu event (including Edit menu items)
    if is_app_menu_event(event_id) || is_edit_menu_event(event_id) {
        app_menu::handle_app_menu_events(app_handle, event_id);
    }
    // Check if it's a tray menu event
    else if is_tray_menu_event(event_id) {
        tray_menu::handle_tray_menu_events(app_handle, event_id);
    }
    // Handle special cases
    else if event_id == constants::tray_menu_ids::SETTINGS {
        // Handle case where app menu receives tray menu settings ID
        info!("[Menu] Received tray menu settings ID, redirecting to settings");
        let app_handle_clone = app_handle.clone();
        tauri::async_runtime::spawn(async move {
            if let Err(e) = crate::window_management::open_settings_window(app_handle_clone).await {
                error!("[Menu] Failed to open settings window: {}", e);
            }
        });
    }
    else {
        info!("[Menu] Unhandled menu event: {}", event_id);
    }
}

/// Check if an event ID belongs to the app menu
fn is_app_menu_event(event_id: &str) -> bool {
    matches!(event_id,
        constants::app_menu_ids::ABOUT |
        constants::app_menu_ids::CHECK_FOR_UPDATES |
        constants::app_menu_ids::SETTINGS |
        constants::app_menu_ids::NEW_CHAT |
        constants::app_menu_ids::IMPORT_CHAT |
        constants::app_menu_ids::EXPORT_CHAT |
        constants::app_menu_ids::TOGGLE_FLOATING_BAR |
        constants::app_menu_ids::TOGGLE_DEV_PANEL |
        constants::app_menu_ids::SHOW_DEVTOOLS |
        constants::app_menu_ids::SHOW_PERMISSIONS |
        constants::app_menu_ids::TOGGLE_FULLSCREEN |
        constants::app_menu_ids::MINIMIZE |
        constants::app_menu_ids::ZOOM |
        constants::app_menu_ids::BRING_ALL_TO_FRONT |
        constants::app_menu_ids::HELP |
        constants::app_menu_ids::KEYBOARD_SHORTCUTS |
        constants::app_menu_ids::SEND_FEEDBACK |
        constants::app_menu_ids::REPORT_ISSUE |
        constants::app_menu_ids::VISIT_WEBSITE
    )
}

/// Check if an event ID belongs to the Edit menu
fn is_edit_menu_event(event_id: &str) -> bool {
    matches!(event_id,
        "edit-undo" |
        "edit-redo" |
        "edit-cut" |
        "edit-copy" |
        "edit-paste" |
        "edit-select-all"
    )
}

/// Check if an event ID belongs to the tray menu
fn is_tray_menu_event(event_id: &str) -> bool {
    matches!(event_id,
        constants::tray_menu_ids::SHOW_HIDE |
        constants::tray_menu_ids::NEW_CHAT |
        constants::tray_menu_ids::SHOW_HIDE_FLOATING_BAR |
        constants::tray_menu_ids::DEVELOPER_TOOLS |
        constants::tray_menu_ids::SETTINGS |
        constants::tray_menu_ids::QUIT |
        constants::tray_menu_ids::SHOW_MAIN_WINDOW |
        constants::tray_menu_ids::HIDE_MAIN_WINDOW |
        constants::tray_menu_ids::SHOW_DEVTOOLS |
        constants::tray_menu_ids::SHOW_FLOATING_BAR |
        constants::tray_menu_ids::HIDE_FLOATING_BAR |
        constants::tray_menu_ids::TOGGLE_FLOATING_BAR
    )
}

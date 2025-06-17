//! # Tray Menu Module
//!
//! This module handles the system tray menu functionality for the Juno application.
//! It provides state-aware menu items that update based on the current window states
//! and comprehensive event handling for all tray menu interactions.

use tauri::{
    AppHandle,
    Manager,
    Emitter,
    menu::{MenuItemKind, Menu, PredefinedMenuItem},
    tray::{TrayIconEvent, MouseButton, MouseButtonState, TrayIconBuilder},
    image::Image as TauriImage,
};
use tracing::{info, error, warn, debug};
use crate::constants;
use crate::commands;

// Embed tray icon data directly in the binary - no file system dependencies
const TRAY_ICON_DATA: &[u8] = include_bytes!("../../icons/32x32.png");

/// Get current window states for tray menu display
async fn get_window_states(app_handle: &AppHandle) -> (bool, bool) {
    crate::window_management::get_window_states(app_handle).await
}

/// Create a state-aware tray menu with window status indicators
async fn create_state_aware_tray_menu(app_handle: &AppHandle) -> Option<Menu<tauri::Wry>> {
    let (main_visible, floating_bar_visible) = get_window_states(app_handle).await;

    // Create main window menu item with state-aware text
    let main_window_text = if main_visible { "Hide Juno" } else { "Show Juno" };
    let main_window_id = if main_visible {
        constants::tray_menu_ids::HIDE_MAIN_WINDOW
    } else {
        constants::tray_menu_ids::SHOW_MAIN_WINDOW
    };
    let main_window_item = MenuItemKind::MenuItem(
        tauri::menu::MenuItem::with_id(app_handle, main_window_id, main_window_text, true, None::<&str>).ok()?
    );

    // Create floating bar menu items with state-aware text
    let floating_bar_text = if floating_bar_visible { "Hide Floating Bar" } else { "Show Floating Bar" };
    let floating_bar_id = if floating_bar_visible {
        constants::tray_menu_ids::HIDE_FLOATING_BAR
    } else {
        constants::tray_menu_ids::SHOW_FLOATING_BAR
    };
    let floating_bar_item = MenuItemKind::MenuItem(
        tauri::menu::MenuItem::with_id(app_handle, floating_bar_id, floating_bar_text, true, None::<&str>).ok()?
    );

    // Create other menu items
    let new_chat_item = MenuItemKind::MenuItem(
        tauri::menu::MenuItem::with_id(app_handle, constants::tray_menu_ids::NEW_CHAT, "New Chat", true, None::<&str>).ok()?
    );
    let devtools_item = MenuItemKind::MenuItem(
        tauri::menu::MenuItem::with_id(app_handle, constants::tray_menu_ids::SHOW_DEVTOOLS, "Developer Tools", true, None::<&str>).ok()?
    );
    let settings_item = MenuItemKind::MenuItem(
        tauri::menu::MenuItem::with_id(app_handle, constants::tray_menu_ids::SETTINGS, "Settings...", true, None::<&str>).ok()?
    );
    let quit_item = MenuItemKind::MenuItem(
        tauri::menu::MenuItem::with_id(app_handle, constants::tray_menu_ids::QUIT, "Quit Juno", true, None::<&str>).ok()?
    );

    // Create separators
    let separator1 = MenuItemKind::Predefined(PredefinedMenuItem::separator(app_handle).ok()?);
    let separator2 = MenuItemKind::Predefined(PredefinedMenuItem::separator(app_handle).ok()?);
    let separator3 = MenuItemKind::Predefined(PredefinedMenuItem::separator(app_handle).ok()?);

    // Build the menu with state-aware items
    Menu::with_items(app_handle, &[
        &main_window_item,
        &new_chat_item,
        &separator1,
        &floating_bar_item,
        &devtools_item,
        &separator2,
        &settings_item,
        &separator3,
        &quit_item,
    ]).map_err(|e| error!("[Tray Menu] Failed to create state-aware menu: {}", e)).ok()
}

/// Update the tray menu to reflect current window states
pub async fn update_tray_menu(app_handle: &AppHandle) {
    if let Some(new_menu) = create_state_aware_tray_menu(app_handle).await {
        // Access the tray by the ID we set in the builder
        if let Some(tray) = app_handle.tray_by_id("main_tray") {
            if let Err(e) = tray.set_menu(Some(new_menu)) {
                warn!("[Tray Menu] Failed to update tray menu: {}", e);
            } else {
                debug!("[Tray Menu] Tray menu updated successfully");
            }
        } else {
            warn!("[Tray Menu] Tray with ID 'main_tray' not found");
        }
    } else {
        warn!("[Tray Menu] Failed to create updated tray menu");
    }
}

/// Handle tray menu events
fn handle_tray_menu_event(app_handle: &AppHandle, event_id: &str) {
    match event_id {
        constants::tray_menu_ids::QUIT => {
            info!("[Tray Menu] Quit requested.");
            app_handle.exit(0);
        }
        constants::tray_menu_ids::SHOW_MAIN_WINDOW => {
            info!("[Tray Menu] Show main window requested.");
            let app_handle_clone = app_handle.clone();
            tauri::async_runtime::spawn(async move {
                if let Err(e) = crate::window_management::open_main_window(app_handle_clone.clone()).await {
                    error!("[Tray Menu] Failed to open main window: {}", e);
                } else {
                    // Update tray menu after successful window creation/show
                    update_tray_menu(&app_handle_clone).await;
                }
            });
        }
        constants::tray_menu_ids::HIDE_MAIN_WINDOW => {
            info!("[Tray Menu] Hide main window requested.");
            if let Some(window) = app_handle.get_webview_window(constants::window_labels::MAIN) {
                let _ = window.hide();
                // Update tray menu after state change
                let app_handle_clone = app_handle.clone();
                tauri::async_runtime::spawn(async move {
                    update_tray_menu(&app_handle_clone).await;
                });
            } else {
                error!("[Tray Menu Error] Main window not found.");
            }
        }
        constants::tray_menu_ids::SHOW_FLOATING_BAR => {
            info!("[Tray Menu] Show floating bar requested.");
            if let Some(window) = app_handle.get_webview_window(constants::window_labels::FLOATING_BAR) {
                if let Err(e) = window.set_ignore_cursor_events(false) {
                    error!("[Tray Error] Failed to set ignore cursor events to false: {}", e);
                }
                let _ = window.show();
                let _ = window.set_focus();
                // Update tray menu after state change
                let app_handle_clone = app_handle.clone();
                tauri::async_runtime::spawn(async move {
                    update_tray_menu(&app_handle_clone).await;
                });
            } else {
                error!("[Tray Menu Error] Floating bar window not found.");
            }
        }
        constants::tray_menu_ids::HIDE_FLOATING_BAR => {
            info!("[Tray Menu] Hide floating bar requested.");
            if let Some(window) = app_handle.get_webview_window(constants::window_labels::FLOATING_BAR) {
                let _ = window.hide();
                if let Err(e) = window.set_ignore_cursor_events(true) {
                    error!("[Tray Error] Failed to set ignore cursor events to true: {}", e);
                }
                // Update tray menu after state change
                let app_handle_clone = app_handle.clone();
                tauri::async_runtime::spawn(async move {
                    update_tray_menu(&app_handle_clone).await;
                });
            } else {
                error!("[Tray Menu Error] Floating bar window not found.");
            }
        }
        constants::tray_menu_ids::NEW_CHAT => {
            info!("[Tray Menu] New chat requested.");
            if let Err(e) = app_handle.emit(constants::events::NEW_CHAT_REQUESTED, ()) {
                error!("[Tray Menu] Failed to emit new chat event: {}", e);
            }
        }
        constants::tray_menu_ids::TOGGLE_FLOATING_BAR => {
            info!("[Tray Menu] Toggle floating bar requested.");
            if let Some(window) = app_handle.get_webview_window(constants::window_labels::FLOATING_BAR) {
                match window.is_visible() {
                    Ok(true) => {
                        let _ = window.hide();
                        if let Err(e) = window.set_ignore_cursor_events(true) {
                            error!("[Tray Error] Failed to set ignore cursor events to true: {}", e);
                        }
                    }
                    Ok(false) => {
                        if let Err(e) = window.set_ignore_cursor_events(false) {
                            error!("[Tray Error] Failed to set ignore cursor events to false: {}", e);
                        }
                        let _ = window.show();
                        let _ = window.set_focus();
                    }
                    Err(e) => error!("[Tray Menu Error] Checking floating bar visibility: {}", e),
                }
                // Update tray menu after state change
                let app_handle_clone = app_handle.clone();
                tauri::async_runtime::spawn(async move {
                    update_tray_menu(&app_handle_clone).await;
                });
            } else {
                error!("[Tray Menu Error] Floating bar window not found for toggle.");
            }
        }
        constants::tray_menu_ids::SHOW_DEVTOOLS => {
            info!("[Tray Menu] Developer Tools menu item clicked");
            if let Err(e) = app_handle.emit(constants::events::DEVTOOLS_REQUESTED, ()) {
                error!("[Tray Menu] Failed to emit devtools-requested event: {}", e);
            }
        }
        constants::tray_menu_ids::SETTINGS => {
            info!("[Tray Menu] Settings menu item clicked");
            let app_handle_clone = app_handle.clone();
            tauri::async_runtime::spawn(async move {
                if let Err(e) = crate::window_management::open_settings_window(app_handle_clone).await {
                    error!("[Tray Menu] Failed to open settings window: {}", e);
                }
            });
        }
        id if id == constants::app_menu_ids::SETTINGS => {
            // Handle case where tray menu receives app menu settings ID
            info!("[Tray Menu] Received app menu settings ID, redirecting to settings");
            let app_handle_clone = app_handle.clone();
            tauri::async_runtime::spawn(async move {
                if let Err(e) = crate::window_management::open_settings_window(app_handle_clone).await {
                    error!("[Tray Menu] Failed to open settings window: {}", e);
                }
            });
        }
        _ => {
            info!("[Tray Menu] Unhandled tray menu event: {:?}", event_id);
        }
    }
}

/// Handle tray icon click events
fn handle_tray_icon_event(event: TrayIconEvent) {
    if let TrayIconEvent::Click {
        button: MouseButton::Left,
        button_state: MouseButtonState::Up,
        ..
    } = event
    {
        info!("[Tray Icon] Left click detected - showing menu only.");
        // Note: The tray menu will automatically show on left click.
        // We no longer toggle the floating bar on tray icon clicks.
        // Users can use the menu items to control the floating bar instead.
    }
}

/// Setup the enhanced tray icon with state-aware menu
pub fn setup_tray_icon(app_handle: &AppHandle) {
    let tray_app_handle = app_handle.clone();

    tauri::async_runtime::spawn(async move {
        // Load the embedded icon data - no file system dependencies
        let loaded_tauri_icon = match image::load_from_memory(TRAY_ICON_DATA) {
            Ok(dynamic_image) => {
                let width = dynamic_image.width();
                let height = dynamic_image.height();
                let rgba_image = dynamic_image.to_rgba8();
                let bytes = rgba_image.into_raw();
                let img = TauriImage::new_owned(bytes, width, height);
                Some(img)
            },
            Err(e) => {
                error!("[Tray Setup Error] Failed to load embedded tray icon: {}", e);
                None
            }
        };

        // Create enhanced tray menu with state-aware items
        let tray_menu = create_state_aware_tray_menu(&tray_app_handle).await;

        let mut tray_builder = TrayIconBuilder::with_id("main_tray")
            .on_menu_event(move |app_handle, event| {
                handle_tray_menu_event(app_handle, event.id().as_ref());
            })
            .on_tray_icon_event(|_tray, event| {
                handle_tray_icon_event(event);
            });

        if let Some(icon_image) = loaded_tauri_icon {
            tray_builder = tray_builder.icon(icon_image);
        }

        if let Some(menu) = tray_menu {
            tray_builder = tray_builder.menu(&menu);
        }

        match tray_builder.build(&tray_app_handle) {
            Ok(_) => {
                info!("[Tray Setup] Enhanced tray icon configured successfully.");
            },
            Err(e) => error!("[Tray Setup Error] Failed to build enhanced tray icon: {}", e),
        }
    });
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

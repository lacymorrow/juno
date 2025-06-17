use tauri::{
    AppHandle,
    Emitter,
    menu::{Menu, PredefinedMenuItem, SubmenuBuilder, MenuItemBuilder}
};
use tracing::{info, error};
use crate::constants;
use crate::commands;

/// Setup the application menu for the main window
pub fn setup_app_menu(app: &AppHandle) -> Result<Menu<tauri::Wry>, Box<dyn std::error::Error>> {
    info!("🍎 Setting up application menu...");

    // Juno Application Menu
    let about_menu_item = MenuItemBuilder::new("About Juno")
        .id(constants::app_menu_ids::ABOUT)
        .build(app)?;

    let check_updates_menu_item = MenuItemBuilder::new("Check for Updates...")
        .id(constants::app_menu_ids::CHECK_FOR_UPDATES)
        .build(app)?;

    let settings_menu_item = MenuItemBuilder::new("Settings...")
        .id(constants::app_menu_ids::SETTINGS)
        .accelerator("CmdOrCtrl+,")
        .build(app)?;

    let app_submenu = SubmenuBuilder::new(app, "Juno")
        .item(&about_menu_item)
        .separator()
        .item(&check_updates_menu_item)
        .separator()
        .item(&settings_menu_item)
        .separator()
        .services()
        .separator()
        .hide()
        .hide_others()
        .quit()
        .build()?;

    // File Menu
    let new_chat_menu_item = MenuItemBuilder::new("New Chat")
        .id(constants::app_menu_ids::NEW_CHAT)
        .accelerator("CmdOrCtrl+N")
        .build(app)?;

    let clear_history_menu_item = MenuItemBuilder::new("Clear History")
        .id(constants::app_menu_ids::CLEAR_HISTORY)
        .accelerator("CmdOrCtrl+Shift+Delete")
        .build(app)?;

    let import_chat_menu_item = MenuItemBuilder::new("Import Chat...")
        .id(constants::app_menu_ids::IMPORT_CHAT)
        .accelerator("CmdOrCtrl+O")
        .build(app)?;

    let export_chat_menu_item = MenuItemBuilder::new("Export Chat...")
        .id(constants::app_menu_ids::EXPORT_CHAT)
        .accelerator("CmdOrCtrl+S")
        .build(app)?;

    let file_submenu = SubmenuBuilder::new(app, "File")
        .item(&new_chat_menu_item)
        .separator()
        .item(&clear_history_menu_item)
        .separator()
        .item(&import_chat_menu_item)
        .item(&export_chat_menu_item)
        .build()?;

    // Edit Menu with standard keyboard shortcuts
    let edit_submenu = SubmenuBuilder::new(app, "Edit")
        .item(&PredefinedMenuItem::undo(app, None)?)
        .item(&PredefinedMenuItem::redo(app, None)?)
        .separator()
        .item(&PredefinedMenuItem::cut(app, None)?)
        .item(&PredefinedMenuItem::copy(app, None)?)
        .item(&PredefinedMenuItem::paste(app, None)?)
        .separator()
        .item(&PredefinedMenuItem::select_all(app, None)?)
        .build()?;

    // View Menu
    let toggle_floating_bar_menu_item = MenuItemBuilder::new("Toggle Floating Bar")
        .id(constants::app_menu_ids::TOGGLE_FLOATING_BAR)
        .accelerator("CmdOrCtrl+B")
        .build(app)?;

    let toggle_dev_panel_menu_item = MenuItemBuilder::new("Toggle Developer Panel")
        .id(constants::app_menu_ids::TOGGLE_DEV_PANEL)
        .accelerator("CmdOrCtrl+Shift+D")
        .build(app)?;

    let show_devtools_menu_item = MenuItemBuilder::new("Developer Tools")
        .id(constants::app_menu_ids::SHOW_DEVTOOLS)
        .accelerator("CmdOrCtrl+Alt+I")
        .build(app)?;

    let show_permissions_menu_item = MenuItemBuilder::new("Permissions...")
        .id(constants::app_menu_ids::SHOW_PERMISSIONS)
        .build(app)?;

    let toggle_fullscreen_menu_item = MenuItemBuilder::new("Toggle Full Screen")
        .id(constants::app_menu_ids::TOGGLE_FULLSCREEN)
        .accelerator("CmdOrCtrl+Ctrl+F")
        .build(app)?;

    let view_submenu = SubmenuBuilder::new(app, "View")
        .item(&toggle_floating_bar_menu_item)
        .item(&toggle_dev_panel_menu_item)
        .separator()
        .item(&show_devtools_menu_item)
        .item(&show_permissions_menu_item)
        .separator()
        .item(&toggle_fullscreen_menu_item)
        .build()?;

    // Window Menu
    let minimize_menu_item = MenuItemBuilder::new("Minimize")
        .id(constants::app_menu_ids::MINIMIZE)
        .accelerator("CmdOrCtrl+M")
        .build(app)?;

    let zoom_menu_item = MenuItemBuilder::new("Zoom")
        .id(constants::app_menu_ids::ZOOM)
        .build(app)?;

    let bring_all_to_front_menu_item = MenuItemBuilder::new("Bring All to Front")
        .id(constants::app_menu_ids::BRING_ALL_TO_FRONT)
        .build(app)?;

    let window_submenu = SubmenuBuilder::new(app, "Window")
        .item(&minimize_menu_item)
        .item(&zoom_menu_item)
        .separator()
        .item(&bring_all_to_front_menu_item)
        .build()?;

    // Help Menu
    let help_menu_item = MenuItemBuilder::new("Juno Help")
        .id(constants::app_menu_ids::HELP)
        .accelerator("CmdOrCtrl+?")
        .build(app)?;

    let keyboard_shortcuts_menu_item = MenuItemBuilder::new("Keyboard Shortcuts")
        .id(constants::app_menu_ids::KEYBOARD_SHORTCUTS)
        .accelerator("CmdOrCtrl+/")
        .build(app)?;

    let send_feedback_menu_item = MenuItemBuilder::new("Send Feedback...")
        .id(constants::app_menu_ids::SEND_FEEDBACK)
        .build(app)?;

    let report_issue_menu_item = MenuItemBuilder::new("Report Issue...")
        .id(constants::app_menu_ids::REPORT_ISSUE)
        .build(app)?;

    let visit_website_menu_item = MenuItemBuilder::new("Visit Website")
        .id(constants::app_menu_ids::VISIT_WEBSITE)
        .build(app)?;

    let help_submenu = SubmenuBuilder::new(app, "Help")
        .item(&help_menu_item)
        .item(&keyboard_shortcuts_menu_item)
        .separator()
        .item(&send_feedback_menu_item)
        .item(&report_issue_menu_item)
        .separator()
        .item(&visit_website_menu_item)
        .build()?;

    // Build the complete menu
    let app_menu = tauri::menu::MenuBuilder::new(app)
        .items(&[&app_submenu, &file_submenu, &edit_submenu, &view_submenu, &window_submenu, &help_submenu])
        .build()?;

    info!("✅ Application menu setup completed");
    Ok(app_menu)
}

/// Handle menu events for the application
pub fn handle_app_menu_events(app_handle: AppHandle, event_id: &str) {
    match event_id {
        // Juno Menu
        constants::app_menu_ids::ABOUT => {
            info!("[Menu] About menu item clicked");
            if let Err(e) = app_handle.emit("about-requested", ()) {
                error!("[Menu] Failed to emit about event: {}", e);
            }
        }
        constants::app_menu_ids::CHECK_FOR_UPDATES => {
            info!("[Menu] Check for Updates menu item clicked");
            if let Err(e) = app_handle.emit(constants::events::UPDATE_CHECK_REQUESTED, ()) {
                error!("[Menu] Failed to emit update check event: {}", e);
            }
        }
        constants::app_menu_ids::SETTINGS => {
            info!("[Menu] Settings menu item clicked");
            let app_handle_clone = app_handle.clone();
            tauri::async_runtime::spawn(async move {
                if let Err(e) = commands::open_settings_window(app_handle_clone).await {
                    error!("[Menu] Failed to open settings window: {}", e);
                }
            });
        }

        // File Menu
        constants::app_menu_ids::NEW_CHAT => {
            info!("[Menu] New Chat menu item clicked");
            if let Err(e) = app_handle.emit(constants::events::NEW_CHAT_REQUESTED, ()) {
                error!("[Menu] Failed to emit new chat event: {}", e);
            }
        }
        constants::app_menu_ids::CLEAR_HISTORY => {
            info!("[Menu] Clear History menu item clicked");
            if let Err(e) = app_handle.emit(constants::events::CLEAR_HISTORY_REQUESTED, ()) {
                error!("[Menu] Failed to emit clear history event: {}", e);
            }
        }
        constants::app_menu_ids::IMPORT_CHAT => {
            info!("[Menu] Import Chat menu item clicked");
            if let Err(e) = app_handle.emit(constants::events::IMPORT_CHAT_REQUESTED, ()) {
                error!("[Menu] Failed to emit import chat event: {}", e);
            }
        }
        constants::app_menu_ids::EXPORT_CHAT => {
            info!("[Menu] Export Chat menu item clicked");
            if let Err(e) = app_handle.emit(constants::events::EXPORT_CHAT_REQUESTED, ()) {
                error!("[Menu] Failed to emit export chat event: {}", e);
            }
        }

        // View Menu
        constants::app_menu_ids::TOGGLE_FLOATING_BAR => {
            info!("[Menu] Toggle Floating Bar menu item clicked");
            if let Err(e) = app_handle.emit(constants::events::TOGGLE_FLOATING_BAR_REQUESTED, ()) {
                error!("[Menu] Failed to emit toggle floating bar event: {}", e);
            }
        }
        constants::app_menu_ids::TOGGLE_DEV_PANEL => {
            info!("[Menu] Toggle Dev Panel menu item clicked");
            if let Err(e) = app_handle.emit(constants::events::TOGGLE_DEV_PANEL_REQUESTED, ()) {
                error!("[Menu] Failed to emit toggle dev panel event: {}", e);
            }
        }
        constants::app_menu_ids::SHOW_DEVTOOLS => {
            info!("[Menu] Developer Tools menu item clicked");
            if let Err(e) = app_handle.emit(constants::events::DEVTOOLS_REQUESTED, ()) {
                error!("[Menu] Failed to emit devtools event: {}", e);
            }
        }
        constants::app_menu_ids::SHOW_PERMISSIONS => {
            info!("[Menu] Permissions menu item clicked");
            if let Err(e) = app_handle.emit(constants::events::PERMISSIONS_REQUESTED, ()) {
                error!("[Menu] Failed to emit permissions event: {}", e);
            }
        }
        constants::app_menu_ids::TOGGLE_FULLSCREEN => {
            info!("[Menu] Toggle Fullscreen menu item clicked");
            if let Err(e) = app_handle.emit(constants::events::TOGGLE_FULLSCREEN_REQUESTED, ()) {
                error!("[Menu] Failed to emit toggle fullscreen event: {}", e);
            }
        }

        // Window Menu
        constants::app_menu_ids::MINIMIZE => {
            info!("[Menu] Minimize menu item clicked");
            if let Err(e) = app_handle.emit(constants::events::MINIMIZE_WINDOW_REQUESTED, ()) {
                error!("[Menu] Failed to emit minimize event: {}", e);
            }
        }
        constants::app_menu_ids::ZOOM => {
            info!("[Menu] Zoom menu item clicked");
            if let Err(e) = app_handle.emit(constants::events::ZOOM_WINDOW_REQUESTED, ()) {
                error!("[Menu] Failed to emit zoom event: {}", e);
            }
        }
        constants::app_menu_ids::BRING_ALL_TO_FRONT => {
            info!("[Menu] Bring All to Front menu item clicked");
            // This is handled automatically by macOS for most cases
            info!("[Menu] Bring All to Front executed");
        }

        // Help Menu
        constants::app_menu_ids::HELP => {
            info!("[Menu] Help menu item clicked");
            if let Err(e) = app_handle.emit(constants::events::HELP_REQUESTED, "general") {
                error!("[Menu] Failed to emit help event: {}", e);
            }
        }
        constants::app_menu_ids::KEYBOARD_SHORTCUTS => {
            info!("[Menu] Keyboard Shortcuts menu item clicked");
            if let Err(e) = app_handle.emit(constants::events::HELP_REQUESTED, "shortcuts") {
                error!("[Menu] Failed to emit keyboard shortcuts event: {}", e);
            }
        }
        constants::app_menu_ids::SEND_FEEDBACK => {
            info!("[Menu] Send Feedback menu item clicked");
            if let Err(e) = app_handle.emit(constants::events::FEEDBACK_REQUESTED, "feedback") {
                error!("[Menu] Failed to emit feedback event: {}", e);
            }
        }
        constants::app_menu_ids::REPORT_ISSUE => {
            info!("[Menu] Report Issue menu item clicked");
            if let Err(e) = app_handle.emit(constants::events::FEEDBACK_REQUESTED, "issue") {
                error!("[Menu] Failed to emit report issue event: {}", e);
            }
        }
        constants::app_menu_ids::VISIT_WEBSITE => {
            info!("[Menu] Visit Website menu item clicked");
            // Open website in default browser
            if let Err(e) = open::that("https://github.com/juno-ai") {
                error!("[Menu] Failed to open website: {}", e);
            }
        }

        // Handle tray menu settings redirected to app menu
        id if id == constants::tray_menu_ids::SETTINGS => {
            info!("[Menu] Received tray menu settings ID, redirecting to settings");
            let app_handle_clone = app_handle.clone();
            tauri::async_runtime::spawn(async move {
                if let Err(e) = commands::open_settings_window(app_handle_clone).await {
                    error!("[Menu] Failed to open settings window: {}", e);
                }
            });
        }

        _ => {
            info!("[Menu] Unhandled menu event: {:?}", event_id);
        }
    }
}

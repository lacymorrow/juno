use tauri::{
    AppHandle,
    Emitter,
    Manager,
    menu::{Menu, PredefinedMenuItem, SubmenuBuilder, MenuItemBuilder}
};
use tracing::{info, error};
use crate::constants;
use crate::commands;
use crate::constants::events;
use crate::constants::errors::{templates, prefixes};

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
        .item(&import_chat_menu_item)
        .item(&export_chat_menu_item)
        .build()?;

    // Edit Menu with standard keyboard shortcuts
    // Create custom menu items with proper labels and IDs for web content compatibility
    let undo_item = MenuItemBuilder::new("Undo")
        .id("edit-undo")
        .accelerator("CmdOrCtrl+Z")
        .build(app)?;

    let redo_item = MenuItemBuilder::new("Redo")
        .id("edit-redo")
        .accelerator("CmdOrCtrl+Shift+Z")
        .build(app)?;

    let cut_item = MenuItemBuilder::new("Cut")
        .id("edit-cut")
        .accelerator("CmdOrCtrl+X")
        .build(app)?;

    let copy_item = MenuItemBuilder::new("Copy")
        .id("edit-copy")
        .accelerator("CmdOrCtrl+C")
        .build(app)?;

    let paste_item = MenuItemBuilder::new("Paste")
        .id("edit-paste")
        .accelerator("CmdOrCtrl+V")
        .build(app)?;

    let select_all_item = MenuItemBuilder::new("Select All")
        .id("edit-select-all")
        .accelerator("CmdOrCtrl+A")
        .build(app)?;

    let edit_submenu = SubmenuBuilder::new(app, "Edit")
        .item(&undo_item)
        .item(&redo_item)
        .separator()
        .item(&cut_item)
        .item(&copy_item)
        .item(&paste_item)
        .separator()
        .item(&select_all_item)
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
            if let Err(e) = app_handle.emit(events::menu::ABOUT_REQUESTED, ()) {
                error!("{} Failed to emit about: {}", prefixes::MENU, e);
            }
        }
        constants::app_menu_ids::CHECK_FOR_UPDATES => {
            info!("[Menu] Check for Updates menu item clicked");
            if let Err(e) = app_handle.emit(constants::events::menu::UPDATE_CHECK_REQUESTED, ()) {
                error!("{} Failed to emit update check: {}", prefixes::MENU, e);
            }
        }
        constants::app_menu_ids::SETTINGS => {
            info!("[Menu] Settings menu item clicked");
            let app_handle_clone = app_handle.clone();
            tauri::async_runtime::spawn(async move {
                if let Err(e) = crate::window_management::open_settings_window(app_handle_clone).await {
                    error!("{} Failed to process settings window open: {}", prefixes::MENU, e);
                }
            });
        }

        // Edit Menu - Handle predefined menu items explicitly for web content
        "edit-undo" => {
            info!("[Menu] Undo menu item clicked");
            if let Err(e) = app_handle.emit(events::menu::EDIT_UNDO, ()) {
                error!("{} Failed to emit undo: {}", prefixes::MENU, e);
            }
        }
        "edit-redo" => {
            info!("[Menu] Redo menu item clicked");
            if let Err(e) = app_handle.emit(events::menu::EDIT_REDO, ()) {
                error!("{} Failed to emit redo: {}", prefixes::MENU, e);
            }
        }
        "edit-cut" => {
            info!("[Menu] Cut menu item clicked");
            if let Err(e) = app_handle.emit(events::menu::EDIT_CUT, ()) {
                error!("{} Failed to emit cut: {}", prefixes::MENU, e);
            }
        }
        "edit-copy" => {
            info!("[Menu] Copy menu item clicked");
            if let Err(e) = app_handle.emit(events::menu::EDIT_COPY, ()) {
                error!("{} Failed to emit copy: {}", prefixes::MENU, e);
            }
        }
        "edit-paste" => {
            info!("[Menu] Paste menu item clicked");
            if let Err(e) = app_handle.emit(events::menu::EDIT_PASTE, ()) {
                error!("{} Failed to emit paste: {}", prefixes::MENU, e);
            }
        }
        "edit-select-all" => {
            info!("[Menu] Select All menu item clicked");
            if let Err(e) = app_handle.emit(events::menu::EDIT_SELECT_ALL, ()) {
                error!("{} Failed to emit select all: {}", prefixes::MENU, e);
            }
        }

        // File Menu
        constants::app_menu_ids::NEW_CHAT => {
            info!("[Menu] New Chat menu item clicked");
            if let Err(e) = app_handle.emit(constants::events::menu::NEW_CHAT_REQUESTED, ()) {
                error!("{} Failed to emit new chat: {}", prefixes::MENU, e);
            }
        }
        constants::app_menu_ids::IMPORT_CHAT => {
            info!("[Menu] Import Chat menu item clicked");
            if let Err(e) = app_handle.emit(constants::events::menu::IMPORT_CHAT_REQUESTED, ()) {
                error!("{} Failed to emit import chat: {}", prefixes::MENU, e);
            }
        }
        constants::app_menu_ids::EXPORT_CHAT => {
            info!("[Menu] Export Chat menu item clicked");
            if let Err(e) = app_handle.emit(constants::events::menu::EXPORT_CHAT_REQUESTED, ()) {
                error!("{} Failed to emit export chat: {}", prefixes::MENU, e);
            }
        }

        // View Menu
        constants::app_menu_ids::TOGGLE_FLOATING_BAR => {
            info!("[Menu] Toggle Floating Bar menu item clicked");
            if let Err(e) = app_handle.emit(constants::events::menu::TOGGLE_FLOATING_BAR_REQUESTED, ()) {
                error!("{} Failed to emit toggle floating bar: {}", prefixes::MENU, e);
            }
        }
        constants::app_menu_ids::TOGGLE_DEV_PANEL => {
            info!("[Menu] Toggle Dev Panel menu item clicked");
            if let Err(e) = app_handle.emit(constants::events::menu::TOGGLE_DEV_PANEL_REQUESTED, ()) {
                error!("{} Failed to emit toggle dev panel: {}", prefixes::MENU, e);
            }
        }
        constants::app_menu_ids::SHOW_DEVTOOLS => {
            info!("[Menu] Developer Tools menu item clicked");
            if let Err(e) = app_handle.emit(constants::events::menu::DEVTOOLS_REQUESTED, ()) {
                error!("{} Failed to emit devtools: {}", prefixes::MENU, e);
            }
        }
        constants::app_menu_ids::SHOW_PERMISSIONS => {
            info!("[Menu] Permissions menu item clicked");
            if let Err(e) = app_handle.emit(constants::events::menu::PERMISSIONS_REQUESTED, ()) {
                error!("{} Failed to emit permissions: {}", prefixes::MENU, e);
            }
        }


        // Window Menu - handled by window management module
        constants::app_menu_ids::MINIMIZE |
        constants::app_menu_ids::ZOOM |
        constants::app_menu_ids::BRING_ALL_TO_FRONT |
        constants::app_menu_ids::TOGGLE_FULLSCREEN => {
            let app_handle_clone = app_handle.clone();
            let event_id_owned = event_id.to_string(); // Convert to owned String
            tauri::async_runtime::spawn(async move {
                crate::window_management::handle_window_menu_event(&app_handle_clone, &event_id_owned).await;
            });
        }

        // Help Menu
        constants::app_menu_ids::HELP => {
            info!("[Menu] Help menu item clicked");
            if let Err(e) = app_handle.emit(constants::events::menu::HELP_REQUESTED, "general") {
                error!("{} Failed to emit help: {}", prefixes::MENU, e);
            }
        }
        constants::app_menu_ids::KEYBOARD_SHORTCUTS => {
            info!("[Menu] Keyboard Shortcuts menu item clicked");
            if let Err(e) = app_handle.emit(constants::events::menu::HELP_REQUESTED, "shortcuts") {
                error!("{} Failed to emit keyboard shortcuts: {}", prefixes::MENU, e);
            }
        }
        constants::app_menu_ids::SEND_FEEDBACK => {
            info!("[Menu] Send Feedback menu item clicked");
            if let Err(e) = app_handle.emit(constants::events::menu::FEEDBACK_REQUESTED, "feedback") {
                error!("{} Failed to emit feedback: {}", prefixes::MENU, e);
            }
        }
        constants::app_menu_ids::REPORT_ISSUE => {
            info!("[Menu] Report Issue menu item clicked");
            if let Err(e) = app_handle.emit(constants::events::menu::FEEDBACK_REQUESTED, "issue") {
                error!("{} Failed to emit report issue: {}", prefixes::MENU, e);
            }
        }
        constants::app_menu_ids::VISIT_WEBSITE => {
            info!("[Menu] Visit Website menu item clicked");
            // Open website in default browser
            if let Err(e) = open::that("https://github.com/juno-ai") {
                error!("{} Failed to process website open: {}", prefixes::MENU, e);
            }
        }

        // Handle tray menu settings redirected to app menu
        id if id == constants::tray_menu_ids::SETTINGS => {
            info!("[Menu] Received tray menu settings ID, redirecting to settings");
            let app_handle_clone = app_handle.clone();
            tauri::async_runtime::spawn(async move {
                if let Err(e) = crate::window_management::open_settings_window(app_handle_clone).await {
                    error!("{} Failed to process settings window open: {}", prefixes::MENU, e);
                }
            });
        }

        _ => {
            info!("[Menu] Unhandled menu event: {:?}", event_id);
        }
    }
}

/// Setup menu for all windows that should support Edit menu functionality
pub fn setup_menu_for_all_windows(app_handle: &AppHandle) -> Result<(), Box<dyn std::error::Error>> {
    info!("🔗 Setting up menu for all windows...");

    // Get the app menu
    let app_menu = setup_app_menu(app_handle)?;

    // Set menu on main app (this covers the main window)
    app_handle.set_menu(app_menu.clone())?;

    // List of window labels that should have the menu
    let window_labels = ["main", "settings", "onboarding"];

    for label in window_labels {
        if let Some(window) = app_handle.get_window(label) {
            if let Err(e) = window.set_menu(app_menu.clone()) {
                error!("{} Failed to configure menu for window '{}': {}", prefixes::MENU, label, e);
            } else {
                info!("[Menu] ✅ Menu set for window '{}'", label);
            }
        }
    }

    info!("✅ Menu setup completed for all windows");
    Ok(())
}

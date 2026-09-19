use crate::constants;
use crate::constants::errors::{prefixes, templates};
use crate::constants::events;
use tauri::{
    menu::{AboutMetadata, Menu, MenuItemBuilder, SubmenuBuilder},
    AppHandle, Emitter, Manager,
};
use tracing::{error, info};

/// Helper function to format error messages with proper template substitution
fn format_error(template: &str, context: &str, error: impl std::fmt::Display) -> String {
    template
        .replacen("{}", context, 1)
        .replacen("{}", &error.to_string(), 1)
}

/// Setup the application menu for the main window
pub fn setup_app_menu(app: &AppHandle) -> Result<Menu<tauri::Wry>, Box<dyn std::error::Error>> {
    info!("🍎 Setting up application menu...");

    // Juno Application Menu — About uses native macOS about panel
    let about_metadata = AboutMetadata {
        name: Some("Juno".to_string()),
        version: Some(env!("CARGO_PKG_VERSION").to_string()),
        copyright: Some("Copyright \u{00a9} 2026 Lacy Morrow".to_string()),
        comments: Some("AI-powered desktop automation for macOS".to_string()),
        ..Default::default()
    };

    let check_updates_menu_item = MenuItemBuilder::new("Check for Updates...")
        .id(constants::app_menu_ids::CHECK_FOR_UPDATES)
        .build(app)?;

    let settings_menu_item = MenuItemBuilder::new("Settings...")
        .id(constants::app_menu_ids::SETTINGS)
        .accelerator("CmdOrCtrl+,")
        .build(app)?;

    let mut app_submenu = SubmenuBuilder::new(app, "Juno")
        .about(Some(about_metadata))
        .separator();

    // A demo build must never update itself. The public build carries no demo
    // key, so an update would drop the person back at "Connect Your AI" with
    // nothing to connect. Take the affordance away rather than let it fail.
    if !crate::demo::is_demo_build() {
        app_submenu = app_submenu.item(&check_updates_menu_item).separator();
    }

    let app_submenu = app_submenu
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

    // Cmd+W is the one shortcut every macOS user already has in their hands, and
    // without a Close item in the menu bar macOS has nothing to fire, so the key
    // did nothing at all in the settings and onboarding windows.
    //
    // This is a custom item rather than the predefined `close_window`, because
    // that one destroys whatever window has focus and Juno's windows do not all
    // want the same thing. The chat window is put away rather than destroyed,
    // because it holds the conversation's React state and scroll position and
    // because closing it hands the conversation back to the bar. Settings and
    // onboarding really are destroyed. So the handler routes to the same close
    // paths the rest of the UI already uses, and each window keeps its own one
    // right answer.
    let close_window_menu_item = MenuItemBuilder::new("Close Window")
        .id(constants::app_menu_ids::CLOSE_WINDOW)
        .accelerator("CmdOrCtrl+W")
        .build(app)?;

    let file_submenu = SubmenuBuilder::new(app, "File")
        .item(&new_chat_menu_item)
        .separator()
        .item(&import_chat_menu_item)
        .item(&export_chat_menu_item)
        .separator()
        .item(&close_window_menu_item)
        .build()?;

    // Edit Menu with native Tauri predefined items for proper keyboard shortcut handling
    let edit_submenu = SubmenuBuilder::new(app, "Edit")
        .undo()
        .redo()
        .separator()
        .cut()
        .copy()
        .paste()
        .separator()
        .select_all()
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

    let zoom_in_menu_item = MenuItemBuilder::new("Zoom In")
        .id(constants::app_menu_ids::ZOOM_IN)
        .accelerator("CmdOrCtrl+=")
        .build(app)?;

    let zoom_out_menu_item = MenuItemBuilder::new("Zoom Out")
        .id(constants::app_menu_ids::ZOOM_OUT)
        .accelerator("CmdOrCtrl+-")
        .build(app)?;

    let actual_size_menu_item = MenuItemBuilder::new("Actual Size")
        .id(constants::app_menu_ids::ACTUAL_SIZE)
        .accelerator("CmdOrCtrl+0")
        .build(app)?;

    let view_submenu = SubmenuBuilder::new(app, "View")
        .item(&toggle_floating_bar_menu_item)
        .item(&toggle_dev_panel_menu_item)
        .separator()
        .item(&zoom_in_menu_item)
        .item(&zoom_out_menu_item)
        .item(&actual_size_menu_item)
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
        .items(&[
            &app_submenu,
            &file_submenu,
            &edit_submenu,
            &view_submenu,
            &window_submenu,
            &help_submenu,
        ])
        .build()?;

    info!("✅ Application menu setup completed");
    Ok(app_menu)
}

/// Handle menu events for the application
pub fn handle_app_menu_events(app_handle: AppHandle, event_id: &str) {
    match event_id {
        // Juno Menu — About is handled natively via PredefinedMenuItem::about()
        constants::app_menu_ids::CHECK_FOR_UPDATES => {
            info!("[Menu] Check for Updates menu item clicked");
            // The item is not built in a demo build, but a keyboard accelerator
            // or a restored menu could still reach here. Updating away from the
            // demo key is never what the person wanted.
            if crate::demo::is_demo_build() {
                info!("[Menu] Ignoring update check: this is a demo build");
                return;
            }
            if let Err(e) = app_handle.emit(constants::events::menu::UPDATE_CHECK_REQUESTED, ()) {
                error!(
                    "{} {}",
                    prefixes::MENU,
                    format_error(templates::FAILED_TO_EMIT, "update check", e)
                );
            }
        }
        constants::app_menu_ids::SETTINGS => {
            info!("[Menu] Settings menu item clicked");
            let app_handle_clone = app_handle.clone();
            tauri::async_runtime::spawn(async move {
                if let Err(e) =
                    crate::window_management::open_settings_window(app_handle_clone).await
                {
                    error!(
                        "{} {}",
                        prefixes::MENU,
                        format_error(templates::FAILED_TO_PROCESS, "settings window open", e)
                    );
                }
            });
        }

        // Edit Menu operations are now handled natively by PredefinedMenuItem
        // No custom event handling needed for edit operations

        // File Menu
        constants::app_menu_ids::NEW_CHAT => {
            info!("[Menu] New Chat menu item clicked");
            if let Err(e) = app_handle.emit(constants::events::menu::NEW_CHAT_REQUESTED, ()) {
                error!(
                    "{} {}",
                    prefixes::MENU,
                    format_error(templates::FAILED_TO_EMIT, "new chat", e)
                );
            }
        }
        constants::app_menu_ids::IMPORT_CHAT => {
            info!("[Menu] Import Chat menu item clicked");
            if let Err(e) = app_handle.emit(constants::events::menu::IMPORT_CHAT_REQUESTED, ()) {
                error!(
                    "{} {}",
                    prefixes::MENU,
                    format_error(templates::FAILED_TO_EMIT, "import chat", e)
                );
            }
        }
        constants::app_menu_ids::EXPORT_CHAT => {
            info!("[Menu] Export Chat menu item clicked");
            if let Err(e) = app_handle.emit(constants::events::menu::EXPORT_CHAT_REQUESTED, ()) {
                error!(
                    "{} {}",
                    prefixes::MENU,
                    format_error(templates::FAILED_TO_EMIT, "export chat", e)
                );
            }
        }

        constants::app_menu_ids::CLOSE_WINDOW => {
            info!("[Menu] Close Window menu item clicked");
            let app_handle_clone = app_handle.clone();
            tauri::async_runtime::spawn(async move {
                close_focused_window(app_handle_clone).await;
            });
        }

        // View Menu
        constants::app_menu_ids::TOGGLE_FLOATING_BAR => {
            info!("[Menu] Toggle Floating Bar menu item clicked");
            if let Err(e) =
                app_handle.emit(constants::events::menu::TOGGLE_FLOATING_BAR_REQUESTED, ())
            {
                error!(
                    "{} {}",
                    prefixes::MENU,
                    format_error(templates::FAILED_TO_EMIT, "toggle floating bar", e)
                );
            }
        }
        constants::app_menu_ids::TOGGLE_DEV_PANEL => {
            info!("[Menu] Toggle Dev Panel menu item clicked");
            if let Err(e) = app_handle.emit(constants::events::menu::TOGGLE_DEV_PANEL_REQUESTED, ())
            {
                error!(
                    "{} {}",
                    prefixes::MENU,
                    format_error(templates::FAILED_TO_EMIT, "toggle dev panel", e)
                );
            }
        }
        constants::app_menu_ids::SHOW_DEVTOOLS => {
            info!("[Menu] Developer Tools menu item clicked");
            if let Err(e) = app_handle.emit(constants::events::menu::DEVTOOLS_REQUESTED, ()) {
                error!(
                    "{} {}",
                    prefixes::MENU,
                    format_error(templates::FAILED_TO_EMIT, "devtools", e)
                );
            }
        }
        constants::app_menu_ids::SHOW_PERMISSIONS => {
            info!("[Menu] Permissions menu item clicked");
            if let Err(e) = app_handle.emit(constants::events::menu::PERMISSIONS_REQUESTED, ()) {
                error!(
                    "{} {}",
                    prefixes::MENU,
                    format_error(templates::FAILED_TO_EMIT, "permissions", e)
                );
            }
        }

        // View Menu - Zoom controls
        constants::app_menu_ids::ZOOM_IN => {
            info!("[Menu] Zoom In menu item clicked");
            if let Err(e) = app_handle.emit(events::menu::ZOOM_IN, ()) {
                error!(
                    "{} {}",
                    prefixes::MENU,
                    format_error(templates::FAILED_TO_EMIT, "zoom in", e)
                );
            }
            // Also apply zoom directly to the focused webview
            apply_zoom_to_focused_window(&app_handle, ZoomAction::In);
        }
        constants::app_menu_ids::ZOOM_OUT => {
            info!("[Menu] Zoom Out menu item clicked");
            if let Err(e) = app_handle.emit(events::menu::ZOOM_OUT, ()) {
                error!(
                    "{} {}",
                    prefixes::MENU,
                    format_error(templates::FAILED_TO_EMIT, "zoom out", e)
                );
            }
            apply_zoom_to_focused_window(&app_handle, ZoomAction::Out);
        }
        constants::app_menu_ids::ACTUAL_SIZE => {
            info!("[Menu] Actual Size menu item clicked");
            if let Err(e) = app_handle.emit(events::menu::RESET_ZOOM, ()) {
                error!(
                    "{} {}",
                    prefixes::MENU,
                    format_error(templates::FAILED_TO_EMIT, "reset zoom", e)
                );
            }
            apply_zoom_to_focused_window(&app_handle, ZoomAction::Reset);
        }

        // Window Menu - handled by window management module
        constants::app_menu_ids::MINIMIZE
        | constants::app_menu_ids::ZOOM
        | constants::app_menu_ids::BRING_ALL_TO_FRONT
        | constants::app_menu_ids::TOGGLE_FULLSCREEN => {
            let app_handle_clone = app_handle.clone();
            let event_id_owned = event_id.to_string(); // Convert to owned String
            tauri::async_runtime::spawn(async move {
                crate::window_management::handle_window_menu_event(
                    &app_handle_clone,
                    &event_id_owned,
                )
                .await;
            });
        }

        // Help Menu
        constants::app_menu_ids::HELP => {
            info!("[Menu] Help menu item clicked");
            if let Err(e) = app_handle.emit(constants::events::menu::SHOW_HELP, "general") {
                error!(
                    "{} {}",
                    prefixes::MENU,
                    format_error(templates::FAILED_TO_EMIT, "help", e)
                );
            }
        }
        constants::app_menu_ids::KEYBOARD_SHORTCUTS => {
            info!("[Menu] Keyboard Shortcuts menu item clicked");
            if let Err(e) = app_handle.emit(constants::events::menu::SHOW_HELP, "shortcuts") {
                error!(
                    "{} {}",
                    prefixes::MENU,
                    format_error(templates::FAILED_TO_EMIT, "keyboard shortcuts", e)
                );
            }
        }
        constants::app_menu_ids::SEND_FEEDBACK => {
            info!("[Menu] Send Feedback menu item clicked");
            if let Err(e) = app_handle.emit(constants::events::menu::SHOW_FEEDBACK, "feedback") {
                error!(
                    "{} {}",
                    prefixes::MENU,
                    format_error(templates::FAILED_TO_EMIT, "feedback", e)
                );
            }
        }
        constants::app_menu_ids::REPORT_ISSUE => {
            info!("[Menu] Report Issue menu item clicked");
            if let Err(e) = app_handle.emit(constants::events::menu::SHOW_FEEDBACK, "issue") {
                error!(
                    "{} {}",
                    prefixes::MENU,
                    format_error(templates::FAILED_TO_EMIT, "report issue", e)
                );
            }
        }
        constants::app_menu_ids::VISIT_WEBSITE => {
            info!("[Menu] Visit Website menu item clicked");
            // Open website in default browser
            if let Err(e) = open::that("https://github.com/juno-ai") {
                error!(
                    "{} {}",
                    prefixes::MENU,
                    format_error(templates::FAILED_TO_PROCESS, "website open", e)
                );
            }
        }

        // Handle tray menu settings redirected to app menu
        id if id == constants::tray_menu_ids::SETTINGS => {
            info!("[Menu] Received tray menu settings ID, redirecting to settings");
            let app_handle_clone = app_handle.clone();
            tauri::async_runtime::spawn(async move {
                if let Err(e) =
                    crate::window_management::open_settings_window(app_handle_clone).await
                {
                    error!(
                        "{} {}",
                        prefixes::MENU,
                        format_error(templates::FAILED_TO_PROCESS, "settings window open", e)
                    );
                }
            });
        }

        _ => {
            info!("[Menu] Unhandled menu event: {:?}", event_id);
        }
    }
}

/// The label of the window the person is actually looking at, if any.
///
/// Cmd+W has to act on the front window, and only the window itself knows
/// whether it is the front one. Exactly one window answers yes, so the
/// non-deterministic map order does not matter.
fn focused_window_label(app_handle: &AppHandle) -> Option<String> {
    app_handle
        .webview_windows()
        .into_iter()
        .find(|(_, window)| window.is_focused().unwrap_or(false))
        .map(|(label, _)| label)
}

/// Put the focused window away, in whatever way that particular window is meant
/// to be put away.
///
/// Each window already has one right answer for "close me", and Cmd+W is just
/// another way of asking. The chat window hides, keeping its webview state.
/// Settings and onboarding are destroyed, the same as their red X does.
///
/// Settings used to hide here too, because a rebuilt settings window came back
/// without the transparency its vibrant sidebar needs. It is now rebuilt from
/// its declared config in tauri.conf.json, so that reason is gone, and settings
/// gets the behaviour it actually wants: it is thrown away and rebuilt, so it
/// cannot reopen showing a permission state it read some time yesterday.
///
/// The floating bar, the floating panel and the overlays are deliberately not in
/// this list. They are panels and chrome, not documents: closing the bar with
/// Cmd+W would take away the hotkey surface and the mic with no window left to
/// bring them back. Hiding the bar stays where it already lives, on Cmd+B and
/// in the tray.
async fn close_focused_window(app_handle: AppHandle) {
    let Some(label) = focused_window_label(&app_handle) else {
        info!("[Menu] Close Window: no Juno window is focused, nothing to close");
        return;
    };

    let result = match label.as_str() {
        constants::window_labels::MAIN => {
            crate::window_management::close_main_window(app_handle).await
        }
        constants::window_labels::SETTINGS => {
            crate::window_management::close_settings_window(app_handle).await
        }
        constants::window_labels::ONBOARDING => {
            crate::window_management::close_onboarding_window(app_handle).await
        }
        other => {
            info!(
                "[Menu] Close Window ignored for '{}': it is not a closable window",
                other
            );
            return;
        }
    };

    if let Err(e) = result {
        error!(
            "{} {}",
            prefixes::MENU,
            format_error(
                templates::FAILED_TO_PROCESS,
                &format!("close of window '{}'", label),
                e
            )
        );
    }
}

/// Zoom action for webview zoom control
enum ZoomAction {
    In,
    Out,
    Reset,
}

/// Apply zoom to the currently focused webview window using JavaScript
fn apply_zoom_to_focused_window(app_handle: &AppHandle, action: ZoomAction) {
    // Try the main window first, then settings — these are the zoomable windows
    let window_labels = ["main", "settings"];
    for label in window_labels {
        if let Some(window) = app_handle.get_webview_window(label) {
            if window.is_focused().unwrap_or(false) {
                let js = match action {
                    ZoomAction::In => {
                        "document.documentElement.style.zoom = String(Math.min(parseFloat(document.documentElement.style.zoom || '1') + 0.1, 3.0))"
                    }
                    ZoomAction::Out => {
                        "document.documentElement.style.zoom = String(Math.max(parseFloat(document.documentElement.style.zoom || '1') - 0.1, 0.3))"
                    }
                    ZoomAction::Reset => {
                        "document.documentElement.style.zoom = '1'"
                    }
                };
                if let Err(e) = window.eval(js) {
                    error!("[Menu] Failed to apply zoom to window '{}': {}", label, e);
                }
                return;
            }
        }
    }
    // If no focused window found, apply to main as a fallback
    if let Some(window) = app_handle.get_webview_window("main") {
        let js = match action {
            ZoomAction::In => {
                "document.documentElement.style.zoom = String(Math.min(parseFloat(document.documentElement.style.zoom || '1') + 0.1, 3.0))"
            }
            ZoomAction::Out => {
                "document.documentElement.style.zoom = String(Math.max(parseFloat(document.documentElement.style.zoom || '1') - 0.1, 0.3))"
            }
            ZoomAction::Reset => {
                "document.documentElement.style.zoom = '1'"
            }
        };
        if let Err(e) = window.eval(js) {
            error!("[Menu] Failed to apply zoom to main window: {}", e);
        }
    }
}

/// Setup menu for all windows that should support Edit menu functionality
pub fn setup_menu_for_all_windows(
    app_handle: &AppHandle,
) -> Result<(), Box<dyn std::error::Error>> {
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
                error!(
                    "{} {}",
                    prefixes::MENU,
                    format_error(
                        templates::FAILED_TO_CONFIGURE,
                        &format!("menu for window '{}'", label),
                        e
                    )
                );
            } else {
                info!("[Menu] ✅ Menu set for window '{}'", label);
            }
        }
    }

    info!("✅ Menu setup completed for all windows");
    Ok(())
}

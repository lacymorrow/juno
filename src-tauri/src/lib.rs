#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

// Import necessary external crates and standard library items
use clap::Parser;
use computer_use_ai_sdk::Desktop;
use dotenvy::dotenv;
use std::env;
use std::sync::{Arc, Mutex};
use tauri::{ // Add Manager and missing items here
    Manager, WindowEvent,
    menu::{Menu, MenuItemBuilder, MenuItemKind, PredefinedMenuItem},
    tray::{TrayIconEvent, MouseButton, MouseButtonState},
    image::Image,
    Runtime, // Required for window.ns_window()
    AppHandle, // Keep AppHandle
    Emitter, // Import Emitter trait for .emit()
    WebviewWindow, // Keep WebviewWindow
    Wry, // Keep Wry if needed elsewhere, remove if not
};
use tracing_subscriber::{fmt, EnvFilter}; // Add fmt and EnvFilter
use tracing::info; // Import the info macro
use std::sync::atomic::{AtomicBool, Ordering};

// macOS specific imports
#[cfg(target_os = "macos")]
use {
    cocoa::{
        appkit::{self, NSEvent, NSWindow, NSView},
        base::{id as cocoa_id, nil, YES, NO, BOOL},
        foundation::{self, NSAutoreleasePool, NSPoint, NSRect, NSSize},
    },
    objc::{class, msg_send, runtime::Object, sel, sel_impl},
    std::thread,
    // std::sync::Arc, // No longer needed for monitor handle
    // std::sync::Mutex, // No longer needed for monitor handle
    objc::runtime::{Class, Object, Sel}, // Added for NSTrackingArea
    core_graphics::geometry::CGRect, // Added for NSTrackingArea options
};

// Declare modules
pub mod tts;
pub mod state;
pub mod anthropic;
pub mod tools;
pub mod commands;
pub mod cli;
pub mod utils;

// Re-export key items for discoverability by main.rs and tauri::generate_handler
pub use commands::*;
pub use anthropic::submit_query; // Re-export the submit_query command

// Added for selector parsing

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    // Explicitly initialize tracing with INFO level by default
    fmt()
        .with_env_filter(EnvFilter::from_default_env().add_directive("info".parse().unwrap()))
        .init();
    dotenv().ok();
    let cli = cli::Cli::parse();

    // --- Initialize Desktop Automation Engine --- (Moved before CLI handling)
    let desktop_instance_result = Desktop::new(false, true);
    let desktop_instance = match desktop_instance_result {
        Ok(instance) => instance,
        Err(e) => {
            eprintln!("FATAL: Failed to initialize Desktop Automation Engine: {}", e);
            tracing::error!("Failed to initialize Desktop Automation Engine: {}", e);
            std::process::exit(1);
        }
    };

    // --- Handle CLI Commands ---
    // If handle_cli_commands returns true, it means a command was executed
    // and the application should exit.
    if cli::runner::handle_cli_commands(&cli, &desktop_instance) {
        return; // Exit early if a CLI command was handled
    }

    // --- Proceed with Tauri Application Launch if no CLI command was run ---
    println!("No CLI commands detected or tests requiring exit, launching Tauri application...");
    let desktop_arc = Arc::new(desktop_instance);

    // Create the AppState
    let app_state = state::AppState {
        desktop: desktop_arc.clone(),
        last_edited_file: Mutex::new(None), // Initialize undo state
        previous_content: Mutex::new(None), // Initialize undo state
    };

    // --- Tauri Application Builder ---
    let builder = tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_notification::init())
        .manage(app_state) // Manage the AppState
        .invoke_handler(tauri::generate_handler![
            // Use re-exported commands
            greet,
            list_apps,
            check_server_status,
            submit_query,
            tts::invoke_tts, // Use the main invoke_tts command for Tauri
            capture_screenshot_command,
            dev_get_focused_element_info,
            capture_element_screenshot_command,
            dev_click_focused_element,
            dev_type_text,
            dev_press_key,
            dev_open_application,
            dev_open_url,
            dev_scroll_window,
            dev_global_type_text,
            dev_get_clipboard,
            dev_set_clipboard,
            dev_hold_key,
            dev_release_key,
            dev_wait,
            dev_find_element_by_selector,
            dev_click_element_by_selector,
        ])
        .on_menu_event(|app, event| { // Attach menu event handler directly
            let window = app.get_webview_window("main").unwrap();
            match event.id.as_ref() {
                "quit" => {
                    println!("[Menu] Quit requested.");
                    app.exit(0);
                }
                "toggle" => { // Keep toggle for floating bar if needed elsewhere, or remove if only tray controls it
                    println!("[Menu] Toggle floating bar requested.");
                    if let Some(window) = app.get_webview_window("floating-bar") {
                        match window.is_visible() {
                            Ok(true) => window.hide().unwrap(),
                            Ok(false) => {
                                window.show().unwrap();
                                window.set_focus().unwrap();
                            },
                            Err(e) => eprintln!("[Menu Error] checking floating bar visibility: {}", e),
                        }
                    } else {
                         eprintln!("[Menu Error] Floating bar window not found for toggle.");
                    }
                }
                "toggle_panel" => {
                    println!("[Menu] Toggle panel requested.");
                    let main_window_visible = window.is_visible().unwrap_or(false);
                    if main_window_visible {
                        window.hide().unwrap();
                        if let Some(MenuItemKind::MenuItem(item)) = app.menu().unwrap().get("toggle_panel") {
                            item.set_text("Show Panel").unwrap();
                        }
                    } else {
                        window.show().unwrap();
                        window.set_focus().unwrap();
                         if let Some(MenuItemKind::MenuItem(item)) = app.menu().unwrap().get("toggle_panel") {
                            item.set_text("Hide Panel").unwrap();
                        }
                    }
                }
                _ => {
                     println!("[Menu] Unhandled event: {:?}", event.id);
                }
            }
        })
        .on_tray_icon_event(|tray, event| { // Attach tray event handler directly
            if let TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                ..
            } = event
            {
                println!("[Tray] Left click detected.");
                let app = tray.app_handle();
                if let Some(window) = app.get_webview_window("floating-bar") {
                    match window.is_visible() {
                        Ok(true) => {
                            window.hide().unwrap();
                            // Make the window ignore mouse events when hidden
                            if let Err(e) = window.set_ignore_cursor_events(true) {
                                eprintln!("[Tray Error] Failed to set ignore cursor events to true: {}", e);
                            }
                            println!("[Tray] Floating bar hidden and ignoring clicks.");
                        },
                        Ok(false) => {
                            // Make the window accept mouse events again when shown
                            if let Err(e) = window.set_ignore_cursor_events(false) {
                                eprintln!("[Tray Error] Failed to set ignore cursor events to false: {}", e);
                            }
                            window.show().unwrap();
                            window.set_focus().unwrap();
                            println!("[Tray] Floating bar shown, focused, and accepting clicks.");
                        },
                        Err(e) => eprintln!("[Tray Error] checking floating bar visibility: {}", e),
                    }
                } else {
                     eprintln!("[Tray Error] Floating bar window not found on left click.");
                }
            }
        })
        .setup(|app| {
            let app_handle = app.handle().clone();

            let toggle_panel_item = MenuItemBuilder::new("Show Panel")
                .id("toggle_panel")
                .build(&app_handle)
                .expect("Failed to build toggle_panel item");
            let quit_item = PredefinedMenuItem::quit(&app_handle, Some("Quit Juno"))
                .expect("Failed to build quit item");

            let menu = Menu::with_items(&app_handle, &[
                &toggle_panel_item,
                &quit_item,
            ]).expect("Failed to create menu");

            let icon_path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../src/assets/tray-Template.png");
            let icon_bytes = std::fs::read(&icon_path).expect("Failed to read icon file");
            let icon = Image::new_owned(
                icon_bytes, // Pass owned Vec<u8>
                32, // Provide explicit width (adjust if needed)
                32  // Provide explicit height (adjust if needed)
            );

            let _tray = tauri::tray::TrayIconBuilder::new()
                .menu(&menu)
                .icon(icon)
                .icon_as_template(true)
                .tooltip("Juno")
                .show_menu_on_left_click(false)
                .build(&app_handle)
                .expect("Failed to build tray icon");

            let main_window = app.get_webview_window("main")
               .ok_or_else(|| "Fatal: Main window not found during setup".to_string())?;

            let window_event_handle = app.handle().clone();
            main_window.on_window_event(move |event| {
                match event {
                    WindowEvent::CloseRequested { api, .. } => {
                        api.prevent_close();
                        let window = window_event_handle.get_webview_window("main").unwrap();
                        window.hide().unwrap();
                        info!("[INFO] Main window hidden via close request."); // Keep info usage
                    }
                    _ => {}
                }
            });

            if let Some(_floating_bar) = app.get_webview_window("floating-bar") {
                println!("Floating bar window found.");
            } else {
                eprintln!("Warning: Floating bar window not found during setup.");
            }

            // --- macOS Specific Setup for Floating Bar --- ///
            #[cfg(target_os = "macos")]
            {
                info!("Applying macOS specific setup...");
                if let Some(window) = app_handle.get_webview_window("floating-bar") {
                    info!("Found floating-bar for macOS setup.");
                    match window.ns_window() {
                        Ok(ns_window_ptr) => {
                            let ns_window = ns_window_ptr as cocoa_id;
                            unsafe {
                                // Allow the window to receive mouse move events even when not key
                                ns_window.setAcceptsMouseMovedEvents_(YES);

                                // Add tracking area for hover events
                                let view = ns_window.contentView();
                                if !view.is_null() {
                                    let options: i64 = appkit::NSTrackingAreaOptions::MouseEnteredAndExited.bits()
                                                    | appkit::NSTrackingAreaOptions::ActiveAlways.bits(); // Track even when window isn't key

                                    let bounds: NSRect = msg_send![view, bounds];

                                    let tracking_area: cocoa_id = msg_send![
                                        class!(NSTrackingArea),
                                        alloc
                                    ];
                                    let tracking_area_init: cocoa_id = msg_send![
                                        tracking_area,
                                        initWithRect:bounds
                                        options:options
                                        owner:view // Use view as owner for simplicity, might need a delegate later
                                        userInfo:nil
                                    ];

                                    // Ensure tracking area covers the view
                                    let _: () = msg_send![tracking_area_init, release]; // Release initial alloc
                                    let _: () = msg_send![view, addTrackingArea: tracking_area_init];

                                    // We need a delegate or observe notifications to actually *get* the events.
                                    // For now, enabling acceptsMouseMovedEvents and adding the area is the setup.
                                    // Actual event handling requires more setup (delegate or notification center).
                                    info!("[macOS] Set acceptsMouseMovedEvents=YES and added NSTrackingArea to floating-bar view.");

                                    // --- Emit events (Placeholder - Requires Delegate/Observer) ---
                                    // This part needs a proper delegate or NotificationCenter observer.
                                    // For demonstration, we'll just log where the emit *should* happen.
                                    // In a real implementation, mouseEntered:/mouseExited: methods of a delegate
                                    // attached to the view (or notifications) would trigger these.
                                    // let emitter = window.app_handle().emit_to("floating-bar", ...);
                                    info!("[macOS] Hover event emission logic needs delegate/observer implementation.");

                                } else {
                                    eprintln!("[macOS Error] Could not get contentView for floating-bar.");
                                }
                            }
                        }
                        Err(e) => {
                            eprintln!("Error getting NSWindow for floating-bar: {}", e);
                        }
                    }
                } else {
                    eprintln!("Warning: floating-bar window not found during macOS specific setup.");
                }
            }
            // --- End macOS Specific Setup ---

            Ok(())
        });

    builder
        .run(tauri::generate_context!()) // Use context relative to lib.rs now
        .expect("error while running tauri application");
}

// Unit tests module
#[cfg(test)]
mod tests {
    // You might need to import items from your modules here
    // use super::*;

    fn add(a: i32, b: i32) -> i32 {
        a + b
    }

    #[test]
    fn test_simple_addition() {
        assert_eq!(add(2, 2), 4, "Check basic addition");
    }

    #[test]
    fn test_focused_element_info_placeholder() {
        // This test might need refactoring if it relied on functions moved to utils or commands
        assert!(true, "Placeholder test for focused element concept");
    }
}

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

// Import necessary external crates and standard library items
use clap::Parser;
use computer_use_ai_sdk::Desktop;
use dotenvy::dotenv;
use std::env;
use std::sync::{Arc, Mutex};
use tauri::{ // Add Manager and missing items here
    Manager, AppHandle, State, WindowEvent,
    menu::{Menu, MenuItemBuilder, MenuItemKind, PredefinedMenuItem},
    tray::{TrayIconBuilder, TrayIconEvent, MouseButton, MouseButtonState},
    image::Image
};
use tracing_subscriber::{fmt, EnvFilter}; // Add fmt and EnvFilter

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
use computer_use_ai_sdk::Selector;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    // Explicitly initialize tracing with INFO level by default
    fmt()
        .with_env_filter(EnvFilter::from_default_env().add_directive("info".parse().unwrap()))
        .init();
    dotenv().ok();
    let cli = cli::Cli::parse();

    let desktop_instance_result = Desktop::new(false, true);
    let desktop_instance = match desktop_instance_result {
        Ok(instance) => instance,
        Err(e) => {
            eprintln!("FATAL: Failed to initialize Desktop Automation Engine: {}", e);
            tracing::error!("Failed to initialize Desktop Automation Engine: {}", e);
            std::process::exit(1);
        }
    };

    // Handle test flags
    let mut ran_test = false;
    let mut test_result: Result<(), String> = Ok(());
    if cli.test_focused_element_ns {
        #[cfg(target_os = "macos")] { test_result = utils::run_test_focused_element_ns(); ran_test = true; }
        #[cfg(not(target_os = "macos"))] { eprintln!("Error: --test-focused-element-ns is only supported on macOS."); test_result = Err("Unsupported platform".to_string()); ran_test = true; }
    }
    if cli.check_accessibility {
        #[cfg(target_os = "macos")] { test_result = utils::run_check_accessibility(); ran_test = true; }
        #[cfg(not(target_os = "macos"))] { println!("Warning: --check-accessibility is macOS-specific. Skipping check."); ran_test = true; }
    }
    if ran_test {
        match test_result { Ok(_) => std::process::exit(0), Err(_) => std::process::exit(1), }
    }

    println!("No test flags detected, launching Tauri application...");
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
            get_logs,
            tts::replicate::invoke_replicate_tts,
            tts::elevenlabs::invoke_elevenlabs_tts,
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
                        Ok(true) => window.hide().unwrap(),
                        Ok(false) => {
                            window.show().unwrap();
                            window.set_focus().unwrap();
                            println!("[Tray] Floating bar shown and focused.");
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
            let quit_item = PredefinedMenuItem::quit(&app_handle, Some("Quit dotdot"))
                .expect("Failed to build quit item");

            let menu = Menu::with_items(&app_handle, &[
                &toggle_panel_item,
                &quit_item,
            ]).expect("Failed to build tray menu");

            let icon_path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../src/assets/tray-Template.png");
            let icon_bytes = std::fs::read(&icon_path).expect("Failed to read icon file");
            let icon = Image::from_bytes(&icon_bytes).expect("Failed to create image from bytes");

            let _tray = tauri::tray::TrayIconBuilder::new()
                .menu(&menu)
                .icon(icon)
                .icon_as_template(true)
                .tooltip("dotdot")
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
                        tracing::info!("[INFO] Main window hidden via close request.");
                    }
                    _ => {}
                }
            });

            if let Some(_floating_bar) = app.get_webview_window("floating-bar") {
                println!("Floating bar window found.");
            } else {
                eprintln!("Warning: Floating bar window not found during setup.");
            }

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

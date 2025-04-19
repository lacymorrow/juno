#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use clap::Parser;
use computer_use_ai_sdk::Desktop;
use dotenvy::dotenv;
use std::env;
use std::sync::Arc;

// Import our modules
mod api;
mod commands;
mod models;
mod tts;
mod utils;

// Re-export the commands for Tauri
pub use commands::*;

// Define a struct to hold the application state
pub(crate) struct AppState {
    desktop: Arc<Desktop>,
}

#[cfg(target_os = "macos")]
fn run_macos_specific_tests(cli: &models::Cli) -> Result<(), String> {
    use computer_use_ai_sdk::platforms::macos::element::get_focused_element_ns_workspace;
    
    if cli.test_focused_element_ns {
        println!("Running NSWorkspace-based focused element test...");
        match get_focused_element_ns_workspace(true, true) {
            Ok(element) => {
                println!("Success! Found focused element:");
                println!("{:#?}", element.attributes());
            }
            Err(e) => {
                println!("Error: {}", e);
                return Err(format!("NSWorkspace focused element test failed: {}", e));
            }
        }
    }

    if cli.check_accessibility {
        use computer_use_ai_sdk::platforms::macos::utils::check_accessibility_permissions;
        println!("Checking accessibility permissions...");
        match check_accessibility_permissions() {
            Ok(has_permissions) => {
                if has_permissions {
                    println!("Accessibility permissions are granted.");
                } else {
                    println!("Accessibility permissions are NOT granted.");
                    return Err("Accessibility permissions are not granted.".to_string());
                }
            }
            Err(e) => {
                println!("Error checking accessibility permissions: {}", e);
                return Err(format!("Failed to check accessibility permissions: {}", e));
            }
        }
    }

    Ok(())
}

#[cfg(not(target_os = "macos"))]
fn run_macos_specific_tests(_cli: &models::Cli) -> Result<(), String> {
    println!("macOS-specific tests are not available on this platform.");
    Ok(())
}

#[tauri::command]
fn initialize_app_state() -> Result<(), String> {
    // Load environment variables from .env file if it exists
    let _ = dotenv();

    // Parse command-line arguments
    let cli = models::Cli::parse();

    // Run platform-specific tests if requested
    if let Err(e) = run_macos_specific_tests(&cli) {
        return Err(e);
    }

    Ok(())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    // Load environment variables from .env file if it exists
    let _ = dotenv();

    // Initialize the Desktop instance
    let desktop = match Desktop::new() {
        Ok(desktop) => Arc::new(desktop),
        Err(e) => {
            eprintln!("Failed to initialize Desktop: {}", e);
            std::process::exit(1);
        }
    };

    tauri::Builder::default()
        .manage(AppState { desktop })
        .invoke_handler(tauri::generate_handler![
            // Basic commands
            greet,
            check_server_status,
            list_apps,
            get_logs,
            
            // Screenshot commands
            capture_screenshot_command,
            dev_get_focused_element_info,
            capture_element_screenshot_command,
            
            // Query commands
            submit_query,
            
            // TTS commands
            tts::elevenlabs::invoke_elevenlabs_tts,
            tts::replicate::invoke_replicate_tts,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
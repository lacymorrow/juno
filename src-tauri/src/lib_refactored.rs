#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

//! Refactored main library file demonstrating reduced complexity and better organization
//! 
//! This file shows how the codebase can be simplified by:
//! - Using the command registry system for organized command registration
//! - Moving application setup logic to separate modules
//! - Reducing the size of the main entry point
//! - Using macros to eliminate repetitive patterns

// External crates and standard library
use clap::Parser;
use computer_use_ai_sdk::Desktop;
use dotenvy::dotenv;
use std::sync::Arc;
use tauri::Manager;
use tracing_subscriber::{fmt, EnvFilter};
use tracing::{info, warn};

// Re-export macros for command definitions
pub use crate::{dev_command, qa_test_command, state_command, generate_invoke_handler};

// Declare modules - much cleaner organization
pub mod tts;
pub mod state;
pub mod anthropic;
pub mod tools;
pub mod commands;
pub mod cli;
pub mod utils;
pub mod agent;
pub mod agents;
pub mod constants;
pub mod dictation_monitor;

// Application setup modules (new - extracted from main lib.rs)
mod app_setup;
mod menu_setup;
mod tray_setup;
mod shortcut_setup;

// Embed tray icon data directly in the binary
const TRAY_ICON_DATA: &[u8] = include_bytes!("../icons/32x32.png");

/// Initialize logging with consistent formatting
fn init_logging() {
    fmt()
        .with_env_filter(EnvFilter::from_default_env().add_directive("info".parse().unwrap()))
        .with_target(false)
        .with_thread_ids(false)
        .with_ansi(true)
        .compact()
        .init();
}

/// Initialize desktop automation engine with proper error handling
fn init_desktop() -> Option<Arc<Desktop>> {
    match Desktop::new_with_auto_redirect(false, true, true) {
        Ok(instance) => {
            info!("Desktop Automation Engine initialized successfully");
            Some(Arc::new(instance))
        },
        Err(e) => {
            warn!("Failed to initialize Desktop Automation Engine: {}", e);
            info!("App will start with limited functionality");
            None
        }
    }
}

/// Initialize AI provider settings
fn init_providers() {
    if let Err(e) = agent::providers::factory::BrainFactory::init() {
        warn!("Failed to initialize AI provider settings: {}", e);
        info!("Continuing with environment variables or fallback defaults");
    } else {
        info!("Provider settings initialized from configuration");
    }
}

/// Main entry point - significantly simplified
#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    // Initialize core systems
    init_logging();
    dotenv().ok();
    let _cli = cli::Cli::parse();

    // Initialize subsystems
    let desktop_arc = init_desktop();
    init_providers();

    // Create application state
    let app_state = state::AppState::new(desktop_arc);
    commands::shell::init_shell_state(&app_state);

    // Build and run Tauri application
    let builder = tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_notification::init())
        .plugin(tauri_plugin_voice_transcription::init())
        .plugin(tauri_plugin_process::init())
        .plugin(shortcut_setup::create_shortcut_plugin()) // Extracted to module
        .manage(app_state)
        .invoke_handler(generate_invoke_handler!()) // Using the registry macro
        .setup(app_setup::setup_application); // Extracted to module

    // Run the application
    builder
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

/// Export re-used items for backwards compatibility
pub use anthropic::submit_query;

// Re-export command functions for the registry
pub use commands::{
    app_url::*, core::*, dictation::*, element::*, filesystem::*, floating_bar::*,
    keyboard::*, mouse::*, permissions::*, providers::*, shell::*, text_editor::*,
    window::*, orchestrator::*, sound::*, tools::*,
    dictation_reset::{force_reset_dictation_transcription, get_dictation_transcription_status},
};
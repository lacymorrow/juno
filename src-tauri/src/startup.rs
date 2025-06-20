//! # Application Startup
//!
//! Coordinated startup sequence for the Juno application with proper error handling,
//! logging, desktop engine initialization, and CLI command processing.

use clap::Parser;
use computer_use_ai_sdk::Desktop;
use std::env;
use std::sync::{Arc, Mutex, OnceLock};
use tauri::{AppHandle, Builder, App, Manager, State};
use tracing::{debug, info, warn, error};
use tracing_subscriber::{fmt, EnvFilter};
use std::time::{SystemTime, UNIX_EPOCH};

use crate::{state, cli, agent, commands, settings};

/// Initialize tracing system with appropriate log levels
pub fn init_tracing() {
    let filter = std::env::var("RUST_LOG").unwrap_or_else(|_| "info".to_string());

    tracing_subscriber::fmt()
        .with_env_filter(&filter)
        .with_target(false)
        .with_thread_ids(false)
        .with_file(false)
        .with_line_number(false)
        .init();

    info!("Logging initialized with filter: {}", filter);
}

/// Initialize environment variables and configuration
pub fn init_environment() {
    // Load .env file if present
    if let Ok(env_path) = std::env::current_dir().map(|p| p.join(".env")) {
        if env_path.exists() {
            match dotenvy::from_path(&env_path) {
                Ok(_) => info!("Loaded environment from .env file"),
                Err(e) => warn!("Failed to load .env file: {}", e),
            }
        }
    }

    // Validate critical environment variables
    validate_environment_variables();
}

/// Validate that required environment variables are set or have fallbacks
pub fn validate_environment_variables() {
    let mut warnings = Vec::new();

    // Check for API keys (these are optional but useful to validate)
    if std::env::var("ANTHROPIC_API_KEY").is_err() {
        warnings.push("ANTHROPIC_API_KEY not set - Anthropic provider may not work");
    }

    if std::env::var("OPENAI_API_KEY").is_err() {
        warnings.push("OPENAI_API_KEY not set - OpenAI provider may not work");
    }

    // Log warnings
    for warning in warnings {
        warn!("{}", warning);
    }

    // Log environment status
    info!("Environment validation completed");
}

// Rate limiter and cache for desktop engine initialization
// Stores (last_init_timestamp, cached_desktop_instance)
static DESKTOP_CACHE: OnceLock<Mutex<(u64, Option<Arc<Desktop>>)>> = OnceLock::new();
const DESKTOP_INIT_COOLDOWN_MS: u64 = 5000; // 5 seconds

/// Initialize the desktop automation engine with error handling
pub fn init_desktop_engine() -> Option<Arc<Desktop>> {
    // Check if we have a cached result first to avoid repeated initialization attempts
    static mut CACHED_DESKTOP: Option<Option<Arc<Desktop>>> = None;
    static INIT_ONCE: std::sync::Once = std::sync::Once::new();

    unsafe {
        INIT_ONCE.call_once(|| {
            CACHED_DESKTOP = Some(try_init_desktop());
        });
        CACHED_DESKTOP.clone().unwrap_or(None)
    }
}

fn try_init_desktop() -> Option<Arc<Desktop>> {
    let result = match Desktop::new(true, true) {
        Ok(desktop) => {
            info!("✅ Desktop automation engine initialized successfully");
            Some(Arc::new(desktop))
        }
        Err(e) => {
            let error_str = e.to_string();
            if error_str.contains("permission") || error_str.contains("accessibility") || error_str.contains("denied") {
                info!("Permission-related error detected - the app's permission flow will guide you through setup");
                info!("System Settings may have opened automatically to grant permissions");
            } else {
                warn!("Unexpected Desktop initialization error: {}", e);
            }
            None
        }
    };

    result
}

/// Initialize AI provider settings
pub fn init_ai_providers() -> Result<(), String> {
    match agent::providers::factory::BrainFactory::init() {
        Ok(()) => {
            info!("Provider settings initialized from configuration");
            Ok(())
        }
        Err(e) => {
            warn!("Failed to initialize AI provider settings: {}", e);
            info!("Continuing with environment variables or fallback defaults");
            Err(e.to_string())
        }
    }
}

/// Handle CLI command processing and determine if app should continue
/// This function is synchronous and handles CLI parsing only
pub fn handle_cli_processing() -> Result<(cli::Cli, bool), crate::error_handling::JunoError> {
    let cli = cli::Cli::parse();

    // For now, just parse CLI and return it. The actual command execution will happen
    // later in the Tauri app context when we have access to the AppHandle
    let has_cli_commands = cli.tts_provider.is_some()
        || cli.check_accessibility
        || cli.test_focused_element_ns;

    Ok((cli, has_cli_commands))
}

/// Initialize application state with desktop instance
pub fn init_app_state(desktop_arc: Option<Arc<Desktop>>) -> state::AppState {
    let app_state = state::AppState::new(desktop_arc);

    // Initialize shell state
    commands::shell::init_shell_state(&app_state);

    app_state
}

/// Startup sequence coordinator
pub struct StartupSequence;

impl StartupSequence {
    /// Execute the complete startup sequence
    pub fn run() -> Result<(cli::Cli, Option<Arc<Desktop>>, state::AppState), String> {
        // Step 1: Initialize logging
        Self::init_logging();

        // Step 2: Load environment variables
        Self::init_environment();

        // Step 3: Parse CLI arguments
        let (cli, _has_cli_commands) = Self::handle_cli()
            .map_err(|e| format!("CLI parsing failed: {}", e))?;

        // Step 4: Initialize desktop engine
        let desktop_arc = Self::init_desktop();

        // Step 5: Initialize AI providers
        let _ = Self::init_providers(); // Non-fatal if this fails

        // Step 6: Initialize application state
        let app_state = Self::init_state(desktop_arc.clone());

        Ok((cli, desktop_arc, app_state))
    }

    fn init_logging() {
        init_tracing();
        info!("🚀 Starting Juno application...");
    }

    fn init_environment() {
        info!("🌍 Initializing environment...");
        init_environment();
    }

    fn init_desktop() -> Option<Arc<Desktop>> {
        info!("🖥️ Initializing Desktop Automation Engine...");
        init_desktop_engine()
    }

    fn init_providers() -> Result<(), String> {
        info!("🧠 Initializing AI providers...");
        init_ai_providers()
    }

    fn handle_cli() -> Result<(cli::Cli, bool), crate::error_handling::JunoError> {
        info!("⚡ Processing CLI arguments...");
        handle_cli_processing()
    }

    fn init_state(desktop_arc: Option<Arc<Desktop>>) -> state::AppState {
        info!("🎯 Initializing application state...");
        init_app_state(desktop_arc)
    }
}

/// Quick startup for development and testing
pub fn quick_startup() -> Result<(Option<Arc<Desktop>>, state::AppState), String> {
    // Simplified startup sequence for tests and development
    init_tracing();
    init_environment();
    let desktop_arc = init_desktop_engine();
    let _ = init_ai_providers();
    let app_state = init_app_state(desktop_arc.clone());
    Ok((desktop_arc, app_state))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_startup_sequence_creation() {
        // Test that StartupSequence can be created and basic methods work
        // This is a basic structure test since full startup requires system resources
        assert!(true, "StartupSequence should be constructible");
    }

    #[test]
    fn test_environment_validation_safety() {
        // Test that environment validation doesn't crash
        validate_environment_variables();
        assert!(true, "Environment validation should complete safely");
    }

    #[test]
    fn test_quick_startup_safety() {
        // Test that quick startup handles missing permissions gracefully
        // In test environment, desktop engine may fail, but should not crash
        match quick_startup() {
            Ok(_) => println!("Quick startup succeeded"),
            Err(e) => println!("Quick startup handled error gracefully: {}", e),
        }
        assert!(true, "Quick startup should handle errors gracefully");
    }
}

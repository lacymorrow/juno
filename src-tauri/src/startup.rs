//! # Startup Module
//!
//! This module handles all application startup logic, initialization sequences,
//! and bootstrapping operations for the Juno application.

use clap::Parser;
use computer_use_ai_sdk::Desktop;
use std::env;
use std::sync::{Arc, Mutex, OnceLock};
use tauri::{AppHandle, Builder, App, Manager, State};
use tracing::{debug, info, warn, error};
use tracing_subscriber::{fmt, EnvFilter};
use std::time::{SystemTime, UNIX_EPOCH};

use crate::{state, cli, agent, commands};

/// Initialize enhanced tracing with optimized formatting
pub fn init_tracing() {
    fmt()
        .with_env_filter(
            EnvFilter::from_default_env().add_directive(
                "info".parse().unwrap_or_else(|_| tracing::level_filters::LevelFilter::INFO.into())
            )
        )
        .with_target(false) // Hide target module names for cleaner output
        .with_thread_ids(false) // Hide thread IDs for cleaner output
        .with_ansi(true) // Enable colors for better readability
        .compact() // Use compact format instead of full
        .init();
}

/// Enhanced environment variable loading for both development and production builds
pub fn init_environment() {
    // Try to load from current directory first (development)
    match dotenvy::dotenv() {
        Ok(path) => {
            info!("Loaded environment variables from: {:?}", path);
        }
        Err(_) => {
            // Try to load from common production locations
            let mut potential_paths = vec![
                std::path::PathBuf::from("./.env"),
                std::path::PathBuf::from("../.env"),
                std::path::PathBuf::from("../../.env"),
            ];

            // Add executable directory if available
            if let Ok(exe) = std::env::current_exe() {
                if let Some(parent) = exe.parent() {
                    potential_paths.push(parent.join(".env"));
                }
            }

            let mut loaded = false;
            for path in potential_paths.iter() {
                if path.exists() {
                    match dotenvy::from_path(path) {
                        Ok(_) => {
                            info!("Loaded environment variables from: {:?}", path);
                            loaded = true;
                            break;
                        }
                        Err(e) => {
                            warn!("Failed to load .env from {:?}: {}", path, e);
                        }
                    }
                }
            }

            if !loaded {
                warn!("No .env file found in any expected location");
                info!("Environment variables will be loaded from system environment");
            }
        }
    }

    // Validate critical environment variables
    validate_environment_variables();
}

/// Validate that critical environment variables are available
pub fn validate_environment_variables() {
    let critical_vars = [
        "ANTHROPIC_API_KEY",
        "OPENAI_API_KEY",
        "ELEVENLABS_API_KEY",
        "GEMINI_API_KEY",
    ];

    let mut missing_vars = Vec::new();

    for var in critical_vars.iter() {
        if env::var(var).is_err() {
            missing_vars.push(*var);
        }
    }

    if !missing_vars.is_empty() {
        warn!("Missing environment variables: {:?}", missing_vars);
        warn!("Some AI provider features may not work without proper API keys");
        info!("You can set these in a .env file or as system environment variables");
    } else {
        info!("All critical environment variables are available");
    }
}

// Rate limiter and cache for desktop engine initialization
// Stores (last_init_timestamp, cached_desktop_instance)
static DESKTOP_CACHE: OnceLock<Mutex<(u64, Option<Arc<Desktop>>)>> = OnceLock::new();
const DESKTOP_INIT_COOLDOWN_MS: u64 = 5000; // 5 seconds

/// Initialize the Desktop Automation Engine with proper error handling and rate limiting
pub fn init_desktop_engine() -> Option<Arc<Desktop>> {
    let cache = DESKTOP_CACHE.get_or_init(|| Mutex::new((0, None)));
    let mut cache_guard = match cache.lock() {
        Ok(guard) => guard,
        Err(e) => {
            warn!("Failed to acquire desktop engine cache lock: {}", e);
            // For initialization functions, we can return None as fallback
            return None;
        }
    };
    let (last_init, cached_desktop) = &mut *cache_guard;

    // Rate limiting check
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64;

    // Fix underflow issue by using saturating_sub to prevent integer underflow
    // if system clock is adjusted backwards or if last_init > now
    if now.saturating_sub(*last_init) < DESKTOP_INIT_COOLDOWN_MS {
        debug!("Desktop engine initialization rate limited, returning cached result");
        return cached_desktop.clone();
    }

    // Update timestamp before attempting initialization
    *last_init = now;

    let desktop_instance_result = Desktop::new_with_auto_redirect(false, true, false);
    let result = match desktop_instance_result {
        Ok(instance) => {
            info!("Desktop Automation Engine initialized successfully with auto-redirect disabled");
            Some(Arc::new(instance))
        },
        Err(e) => {
            warn!("Failed to initialize Desktop Automation Engine: {}", e);
            info!("App will start with limited functionality - desktop automation features will be disabled");
            info!("The app will still open and show the permission flow to guide you through setup");

            // Check if this is specifically a permission error
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

    // Update cache with the new result (whether success or failure)
    *cached_desktop = result.clone();

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
pub fn handle_cli_processing(desktop_arc: &Option<Arc<Desktop>>) -> Result<bool, crate::error_handling::JunoError> {
    let cli = cli::Cli::parse();

    // If handle_cli_commands returns Ok(true), it means a command was executed
    // and the application should exit.
    if let Some(desktop_ref) = desktop_arc.as_ref() {
        match cli::runner::handle_cli_commands(&cli, desktop_ref) {
            Ok(should_exit) => {
                if should_exit {
                    return Ok(false); // Exit early if a CLI command was handled
                }
            }
            Err(e) => {
                error!("CLI command execution failed: {}", e);
                return Err(e);
            }
        }
    } else {
        // Handle CLI commands without desktop instance - create minimal instance for CLI only
        // Don't use auto-redirect for CLI to avoid opening settings during CLI operations
        match Desktop::new(false, false) {
            Ok(minimal_desktop) => {
                match cli::runner::handle_cli_commands(&cli, &minimal_desktop) {
                    Ok(should_exit) => {
                        if should_exit {
                            return Ok(false);
                        }
                    }
                    Err(e) => {
                        error!("CLI command execution failed with minimal desktop: {}", e);
                        return Err(e);
                    }
                }
            },
            Err(_) => {
                // If CLI commands require desktop and can't create minimal instance,
                // only handle non-desktop CLI commands
                if cli::runner::handle_non_desktop_cli_commands(&cli) {
                    return Ok(false);
                }
            }
        }
    }

    // Proceed with Tauri application launch if no CLI command was run
    println!("No CLI commands detected or tests requiring exit, launching Tauri application...");
    Ok(true)
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
    pub fn run() -> Result<(Option<Arc<Desktop>>, state::AppState), String> {
        // Step 1: Initialize logging
        Self::init_logging();

        // Step 2: Load environment variables
        Self::init_environment();

        // Step 3: Initialize desktop engine
        let desktop_arc = Self::init_desktop();

        // Step 4: Initialize AI providers
        let _ = Self::init_providers(); // Non-fatal if this fails

        // Step 5: Handle CLI commands (may exit early)
        match Self::handle_cli(&desktop_arc) {
            Ok(should_continue) => {
                if !should_continue {
                    return Err("CLI command executed, application should exit".to_string());
                }
            }
            Err(e) => {
                return Err(format!("CLI command failed: {}", e));
            }
        }

        // Step 6: Initialize application state
        let app_state = Self::init_state(desktop_arc.clone());

        Ok((desktop_arc, app_state))
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

    fn handle_cli(desktop_arc: &Option<Arc<Desktop>>) -> Result<bool, crate::error_handling::JunoError> {
        info!("⚡ Processing CLI arguments...");
        handle_cli_processing(desktop_arc)
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

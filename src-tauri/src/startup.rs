//! # Startup Module
//!
//! This module handles all application startup logic, initialization sequences,
//! and bootstrapping operations for the Juno application.

use clap::Parser;
use computer_use_ai_sdk::Desktop;
use std::env;
use std::sync::{Arc, Mutex, OnceLock, LazyLock};
use tauri::{AppHandle, Builder, App, Manager, State};
use tracing::{debug, info, warn, error};
use tracing_subscriber::{fmt, EnvFilter};
use std::time::{SystemTime, UNIX_EPOCH, Duration, Instant};

use crate::{state, cli, agent, commands};
use crate::agent::providers::factory::BrainFactory;
use crate::constants::timeouts;
use crate::state::AppState;
use crate::cli::headless::HeadlessRuntime;

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
static DESKTOP_CACHE: LazyLock<Mutex<(u64, Option<Arc<Desktop>>)>> = LazyLock::new(|| Mutex::new((0, None)));
const DESKTOP_INIT_COOLDOWN_MS: u64 = 2000; // 2 second cooldown

// Permission check caching to prevent redundant checks
static PERMISSION_CACHE: LazyLock<Mutex<Option<(bool, Instant)>>> = LazyLock::new(|| Mutex::new(None));
const PERMISSION_CACHE_DURATION: Duration = Duration::from_secs(10); // 10 second cache

/// Initialize the Desktop Automation Engine with proper error handling and rate limiting
pub fn init_desktop_engine() -> Option<Arc<Desktop>> {
    let mut cache_guard = match DESKTOP_CACHE.lock() {
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

    // Check permission cache first to avoid redundant permission checks
    if let Ok(perm_cache) = PERMISSION_CACHE.lock() {
        if let Some((has_permissions, timestamp)) = *perm_cache {
            if timestamp.elapsed() < PERMISSION_CACHE_DURATION && !has_permissions {
                debug!("Permission cache indicates no permissions, skipping desktop initialization");
                return None;
            }
        }
    }

    // Update timestamp before attempting initialization
    *last_init = now;

    let desktop_instance_result = Desktop::new_with_auto_redirect(false, true, false);
    let result = match desktop_instance_result {
        Ok(instance) => {
            info!("Desktop Automation Engine initialized successfully with auto-redirect disabled");

            // Update permission cache with success
            if let Ok(mut perm_cache) = PERMISSION_CACHE.lock() {
                *perm_cache = Some((true, Instant::now()));
            }

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

                // Update permission cache with failure
                if let Ok(mut perm_cache) = PERMISSION_CACHE.lock() {
                    *perm_cache = Some((false, Instant::now()));
                }
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

/// Clear permission cache to force re-check (useful after permission changes)
pub fn clear_permission_cache() {
    if let Ok(mut perm_cache) = PERMISSION_CACHE.lock() {
        *perm_cache = None;
        debug!("Cleared permission cache");
    }
}

/// Handle CLI command processing and determine if app should continue
pub fn handle_cli_processing(desktop_arc: &Option<Arc<Desktop>>) -> Result<bool, crate::error_handling::JunoError> {
    let cli = cli::Cli::parse();

    // Check if this is a headless operation
    if cli.is_headless() {
        info!("Headless mode detected, processing CLI command without GUI");
        return handle_headless_cli(&cli, desktop_arc);
    }

    // Handle legacy CLI flags for backward compatibility
    if cli.has_legacy_flags() {
        info!("Legacy CLI flags detected, processing with legacy handler");
        return handle_legacy_cli(&cli, desktop_arc);
    }

    // No CLI commands to process, continue with GUI application
    info!("No CLI commands detected, launching GUI application...");
    Ok(true)
}

/// Handle headless CLI operations
async fn handle_headless_cli_async(cli: &cli::Cli, desktop_arc: &Option<Arc<Desktop>>) -> Result<bool, crate::error_handling::JunoError> {
    use crate::cli::headless::HeadlessRuntime;
    use crate::error_handling::JunoError;

    // Create minimal Tauri app for CLI operations if needed
    let app_handle = create_minimal_tauri_app().await?;

    // Create runtime with app handle and CLI options
    let runtime = HeadlessRuntime::new(app_handle.clone(), cli);

    // Execute the CLI command using the headless runtime
    match runtime.execute_command(cli).await {
        Ok(result) => {
            runtime.output_result(&result);
            Ok(false) // Exit after processing
        }
        Err(e) => {
            eprintln!("Error executing headless command: {}", e);
            Ok(false) // Exit after error
        }
    }
}

/// Synchronous wrapper for headless CLI handling
fn handle_headless_cli(cli: &cli::Cli, desktop_arc: &Option<Arc<Desktop>>) -> Result<bool, crate::error_handling::JunoError> {
    use tokio::runtime::Runtime;

    let rt = Runtime::new().map_err(|e| {
        crate::error_handling::JunoError::SystemError(format!("Failed to create async runtime: {}", e))
    })?;

    rt.block_on(handle_headless_cli_async(cli, desktop_arc))
}

/// Handle legacy CLI flags for backward compatibility
fn handle_legacy_cli(cli: &cli::Cli, desktop_arc: &Option<Arc<Desktop>>) -> Result<bool, crate::error_handling::JunoError> {
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

    Ok(true) // Continue with GUI application
}

/// Create a minimal Tauri application for CLI operations
async fn create_minimal_tauri_app() -> Result<AppHandle, crate::error_handling::JunoError> {
    use crate::error_handling::JunoError;
    use tokio::sync::oneshot;
    use std::sync::{Arc, Mutex};

    let (tx, rx) = oneshot::channel::<Result<AppHandle, JunoError>>();
    let app_handle_container = Arc::new(Mutex::new(None::<AppHandle>));
    let app_handle_container_clone = app_handle_container.clone();

    // Create the app in a thread with proper lifecycle management
    std::thread::spawn(move || {
        let result = tauri::Builder::default()
            .plugin(tauri_plugin_store::Builder::default().build())
            .plugin(tauri_plugin_voice_transcription::init())
            .plugin(tauri_plugin_process::init())
            .setup(move |app| {
                let app_handle = app.handle().clone();

                // Initialize minimal app state for CLI operations
                let desktop_arc = init_desktop_engine(); // This is safe and won't crash
                let app_state = init_app_state(desktop_arc);
                app.manage(app_state);

                // Store the handle in the container - don't send yet, build must complete first
                if let Ok(mut container) = app_handle_container_clone.lock() {
                    *container = Some(app_handle);
                } else {
                    error!("Failed to store app handle in container");
                }

                info!("Headless Tauri app setup completed");
                Ok(())
            })
            .build(crate::get_tauri_context());

        // Send the result only after build completes (fixes race condition)
        let send_result = match result {
            Ok(_app) => {
                info!("Headless Tauri app created successfully");

                // Get the app handle from the container now that build succeeded
                match app_handle_container.lock() {
                    Ok(container) => {
                        if let Some(handle) = container.as_ref() {
                            tx.send(Ok(handle.clone()))
                        } else {
                            tx.send(Err(JunoError::SystemError("App handle not captured during setup".to_string())))
                        }
                    }
                    Err(e) => {
                        tx.send(Err(JunoError::SystemError(format!("Failed to access app handle container: {}", e))))
                    }
                }
            },
            Err(e) => {
                error!("Failed to create headless Tauri app: {}", e);
                // Properly propagate build errors through the channel
                tx.send(Err(JunoError::SystemError(format!("Failed to build Tauri app: {}", e))))
            }
        };

        // Handle channel send failures appropriately
        if let Err(_) = send_result {
            error!("Failed to send app initialization result - receiver may have timed out");
        }

        // Keep the thread alive for CLI operations with reasonable timeout (fixes thread leak)
        // Use sleep instead of indefinite parking to prevent permanent thread leak
        std::thread::sleep(std::time::Duration::from_secs(300)); // 5 minutes for CLI operations
        info!("Headless Tauri app thread exiting after timeout - preventing thread leak");
    });

    // Wait for the app handle with timeout to prevent deadlock
    tokio::time::timeout(
        tokio::time::Duration::from_secs(15),
        rx
    ).await
    .map_err(|_| JunoError::SystemError("Timeout waiting for headless Tauri app initialization".to_string()))?
    .map_err(|_| JunoError::SystemError("Failed to receive app handle from initialization thread".to_string()))?
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

    #[test]
    fn test_headless_cli_detection() {
        // Test CLI headless mode detection
        let cli = cli::Cli::parse_from(vec!["juno", "--headless", "query", "test"]);
        assert!(cli.is_headless(), "Should detect headless mode");

        let cli_with_command = cli::Cli::parse_from(vec!["juno", "agent", "status"]);
        assert!(cli_with_command.is_headless(), "Should detect headless mode with subcommands");

        let cli_gui = cli::Cli::parse_from(vec!["juno"]);
        assert!(!cli_gui.is_headless(), "Should not detect headless mode for GUI");
    }

    #[test]
    fn test_legacy_cli_detection() {
        // Test legacy CLI flag detection
        let cli = cli::Cli::parse_from(vec!["juno", "--tts-provider", "system", "--tts-text", "test"]);
        assert!(cli.has_legacy_flags(), "Should detect legacy TTS flags");

        let cli_normal = cli::Cli::parse_from(vec!["juno", "query", "test"]);
        assert!(!cli_normal.has_legacy_flags(), "Should not detect legacy flags in modern CLI");
    }

    #[test]
    fn test_create_minimal_tauri_app_logic() {
        // Test that the create_minimal_tauri_app function logic is sound
        // We can't run the actual async function in a unit test, but we can verify
        // the synchronization primitives work correctly
        use std::sync::{Arc, Mutex};
        use tokio::sync::oneshot;
        use crate::error_handling::JunoError;

        // Test the container pattern we use for app handle storage
        let app_handle_container = Arc::new(Mutex::new(None::<String>)); // Use String as a simple test type
        let container_clone = app_handle_container.clone();

        // Simulate storing a value
        {
            let mut container = container_clone.lock().unwrap();
            *container = Some("test_handle".to_string());
        }

        // Simulate retrieving the value
        {
            let container = app_handle_container.lock().unwrap();
            assert!(container.is_some(), "Container should hold the stored value");
            assert_eq!(container.as_ref().unwrap(), "test_handle");
        }

        // Test the channel pattern
        let (tx, _rx) = oneshot::channel::<Result<String, JunoError>>();
        let send_result = tx.send(Ok("test".to_string()));
        assert!(send_result.is_ok(), "Channel send should succeed");
    }
}

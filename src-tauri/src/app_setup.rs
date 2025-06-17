use std::sync::Arc;
use tauri::{AppHandle, Manager, Emitter, Listener};
use tracing::{info, warn, error};
use crate::constants;
use crate::commands;
use crate::state::AppState;

/// Initialize all application components after Tauri setup
pub async fn initialize_application(app_handle: AppHandle) -> Result<(), String> {
    info!("🚀 Starting application initialization...");

    // Load environment variables first
    initialize_environment(&app_handle).await?;

    // Initialize keyboard shortcuts
    initialize_shortcuts(&app_handle).await?;

    // Initialize MCP servers
    initialize_mcp_servers(&app_handle).await?;

    // Initialize cloud services
    initialize_cloud_services(&app_handle).await?;

    // Initialize monitoring systems
    initialize_monitoring_systems(&app_handle).await?;

    // Initialize floating bar manager
    initialize_floating_bar_manager(&app_handle).await?;

    // Initialize orchestrator
    initialize_orchestrator(&app_handle).await?;

    // Initialize autostart configuration
    initialize_autostart(&app_handle).await?;

    // Play boot sound
    play_boot_sound(&app_handle).await?;

    info!("✅ Application initialization completed successfully");
    Ok(())
}

/// Load and validate environment variables
async fn initialize_environment(app_handle: &AppHandle) -> Result<(), String> {
    info!("🔧 Initializing environment variables...");

    if let Err(e) = crate::environment::load_bundled_environment(app_handle.clone()).await {
        warn!("Failed to load bundled environment: {}", e);
        info!("Using environment variables from system environment or development .env file");
    } else {
        info!("Successfully loaded environment variables from bundled resources");
    }

    Ok(())
}

/// Initialize keyboard shortcuts from configuration
async fn initialize_shortcuts(app_handle: &AppHandle) -> Result<(), String> {
    info!("⌨️ Initializing keyboard shortcuts...");

    let app_state = app_handle.state::<AppState>();

    // Load keyboard shortcuts from persistent storage
    if let Err(e) = crate::commands::shortcuts::load_shortcuts_from_store(app_handle, &app_state).await {
        warn!("Failed to load keyboard shortcuts: {} - using defaults", e);
    }

    // Load agent trigger mode from persistent storage
    if let Err(e) = crate::commands::core::load_agent_trigger_mode_from_store(app_handle, &app_state).await {
        warn!("Failed to load agent trigger mode: {} - using defaults", e);
    }

    // Register global shortcuts after loading configuration
    if let Err(e) = crate::commands::shortcuts::update_global_shortcuts(app_handle, &app_state).await {
        warn!("Failed to register global shortcuts: {} - continuing without shortcuts", e);
    }

    Ok(())
}

/// Initialize MCP servers from configuration
async fn initialize_mcp_servers(app_handle: &AppHandle) -> Result<(), String> {
    info!("🔌 Initializing MCP servers...");

    let app_state = app_handle.state::<AppState>();
    if let Err(e) = app_state.initialize_mcp_servers().await {
        warn!("Failed to initialize MCP servers: {}", e);
        info!("MCP servers can be configured and started via Settings");
    } else {
        info!("Successfully initialized MCP servers");
    }

    // Start MCP error recovery background task
    start_mcp_error_recovery_task(app_handle.clone()).await;

    Ok(())
}

/// Start MCP error recovery background task
async fn start_mcp_error_recovery_task(app_handle: AppHandle) {
    tauri::async_runtime::spawn(async move {
        let app_state = app_handle.state::<AppState>();
        let mut interval = tokio::time::interval(std::time::Duration::from_secs(60));

        loop {
            interval.tick().await;
            if let Err(e) = app_state.retry_failed_mcp_servers().await {
                tracing::debug!("MCP retry check failed: {}", e);
            }
        }
    });
}

/// Initialize cloud services
async fn initialize_cloud_services(app_handle: &AppHandle) -> Result<(), String> {
    info!("☁️ Initializing cloud services...");

    let app_state = app_handle.state::<AppState>();

    // Initialize cloud client configuration
    if let Err(e) = app_state.init_cloud_client(app_handle).await {
        error!("Failed to initialize cloud client: {}", e);
    } else {
        info!("Cloud client configuration initialized");

        // Start cloud client if enabled
        if app_state.is_cloud_enabled() {
            if let Err(e) = app_state.start_cloud_client().await {
                error!("Failed to start cloud client: {}", e);
            } else {
                info!("Cloud client started successfully");
            }
        } else {
            info!("Cloud connectivity is disabled in configuration");
        }
    }

    Ok(())
}

/// Initialize monitoring systems (dictation and agent monitors)
async fn initialize_monitoring_systems(app_handle: &AppHandle) -> Result<(), String> {
    info!("📊 Initializing monitoring systems...");

    // Initialize dictation input monitoring system
    if let Err(e) = crate::dictation_monitor::init_dictation_input_monitoring(app_handle.clone()).await {
        error!("Failed to initialize dictation input monitoring: {}", e);
    } else {
        info!("Dictation input monitoring system initialized successfully");
    }

    // Start agent monitor task for hold behavior
    let _agent_monitor_handle = crate::agent_monitor::start_agent_monitor_task(app_handle.clone());
    info!("Agent monitor task started successfully");

    Ok(())
}

/// Initialize floating bar manager
async fn initialize_floating_bar_manager(app_handle: &AppHandle) -> Result<(), String> {
    info!("🎛️ Initializing floating bar manager...");

    commands::floating_bar::initialize_bar_manager(app_handle.clone()).await;
    info!("Floating bar manager initialized successfully");

    Ok(())
}

/// Initialize multi-agent orchestrator
async fn initialize_orchestrator(app_handle: &AppHandle) -> Result<(), String> {
    info!("🎭 Initializing orchestrator...");

    if let Err(e) = commands::orchestrator::init_orchestrator_with_app_handle(app_handle.clone()).await {
        error!("Failed to initialize orchestrator system: {}", e);
    } else {
        info!("Multi-agent orchestrator system initialized successfully");
    }

    Ok(())
}

/// Initialize autostart configuration
async fn initialize_autostart(app_handle: &AppHandle) -> Result<(), String> {
    info!("🚀 Initializing autostart configuration...");

    commands::autostart::init_autostart(app_handle);
    info!("Autostart configuration initialized successfully");

    Ok(())
}

/// Play application boot sound
async fn play_boot_sound(app_handle: &AppHandle) -> Result<(), String> {
    info!("🔊 Playing boot sound...");

    // Small delay to ensure UI is ready
    tokio::time::sleep(tokio::time::Duration::from_millis(1000)).await;

    let state = app_handle.state::<AppState>();
    if let Err(e) = crate::commands::sound::play_boot_sound(app_handle.clone(), state).await {
        warn!("Failed to play boot sound: {}", e);
    } else {
        info!("Boot sound played successfully from backend");
    }

    Ok(())
}

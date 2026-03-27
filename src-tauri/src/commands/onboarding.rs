use crate::settings::{manager::SettingsManager, OnboardingSettings};
use tauri::{AppHandle, Manager};
use tracing::{error, info, warn};

/// Check if we're running in development mode
fn is_development_mode() -> bool {
    #[cfg(debug_assertions)]
    {
        true
    }
    #[cfg(not(debug_assertions))]
    {
        false
    }
}

/// Check if the user has completed onboarding
/// In development mode, this will always return false to show onboarding
#[tauri::command]
pub async fn check_onboarding_status(app: AppHandle) -> Result<bool, String> {
    // In development mode, always show onboarding
    if is_development_mode() {
        info!("Development mode detected - onboarding will always be shown");
        return Ok(false);
    }

    let settings_manager = SettingsManager::new(app).map_err(|e| e.to_string())?;
    let onboarding_settings = settings_manager
        .get_onboarding_settings()
        .await
        .map_err(|e| e.to_string())?;

    Ok(onboarding_settings.completed)
}

/// Mark onboarding as completed
#[tauri::command]
pub async fn complete_onboarding(app: AppHandle) -> Result<(), String> {
    let settings_manager = SettingsManager::new(app.clone()).map_err(|e| e.to_string())?;
    let now = chrono::Utc::now().to_rfc3339();

    let onboarding_settings = OnboardingSettings {
        completed: true,
        completed_at: Some(now.clone()),
        skipped: false,
        skip_count: 0,
    };

    settings_manager
        .set_onboarding_settings(&onboarding_settings)
        .await
        .map_err(|e| e.to_string())?;

    info!("Onboarding marked as completed at {}", now);

    // Clear onboarding active state so shortcut handlers resume normal behavior
    if let Err(e) = set_onboarding_active(app.clone(), false).await {
        warn!("Failed to clear onboarding active state on completion: {}", e);
    }

    // Show the main window now that onboarding is done
    if let Err(e) = crate::window_management::open_main_window(app.clone()).await {
        warn!("Failed to show main window after onboarding completion: {}", e);
    }

    Ok(())
}

/// Mark onboarding as skipped (still counts as completed)
#[tauri::command]
pub async fn skip_onboarding(app: AppHandle) -> Result<(), String> {
    let settings_manager = SettingsManager::new(app.clone()).map_err(|e| e.to_string())?;
    let now = chrono::Utc::now().to_rfc3339();

    // Get current settings to preserve skip count
    let current_settings = settings_manager
        .get_onboarding_settings()
        .await
        .map_err(|e| e.to_string())?;

    let onboarding_settings = OnboardingSettings {
        completed: true,
        completed_at: Some(now.clone()),
        skipped: true,
        skip_count: current_settings.skip_count + 1,
    };

    settings_manager
        .set_onboarding_settings(&onboarding_settings)
        .await
        .map_err(|e| e.to_string())?;

    info!(
        "Onboarding skipped at {} (skip count: {})",
        now, onboarding_settings.skip_count
    );

    // Clear onboarding active state so shortcut handlers resume normal behavior
    if let Err(e) = set_onboarding_active(app.clone(), false).await {
        warn!("Failed to clear onboarding active state on skip: {}", e);
    }

    // Show the main window now that onboarding is done (skipped)
    if let Err(e) = crate::window_management::open_main_window(app.clone()).await {
        warn!("Failed to show main window after onboarding skip: {}", e);
    }

    Ok(())
}

/// Reset onboarding (for testing/development and user-requested restart)
#[tauri::command]
pub async fn reset_onboarding(app: AppHandle) -> Result<(), String> {
    let settings_manager = SettingsManager::new(app.clone()).map_err(|e| e.to_string())?;

    let onboarding_settings = OnboardingSettings {
        completed: false,
        completed_at: None,
        skipped: false,
        skip_count: 0,
    };

    settings_manager
        .set_onboarding_settings(&onboarding_settings)
        .await
        .map_err(|e| e.to_string())?;

    // Reset permissions state so the permissions flow can be shown again during onboarding
    let app_state = app.state::<crate::state::AppState>();

    // Clear the permissions state in the app state
    app_state
        .update_permissions_state(crate::commands::permissions::PermissionsState {
            accessibility: crate::commands::permissions::PermissionStatus {
                permission_type: "accessibility".to_string(),
                granted: false,
                required: true,
                description: "Accessibility permission needs to be rechecked".to_string(),
                instructions: "Grant accessibility permission during onboarding".to_string(),
            },
            screen_recording: crate::commands::permissions::PermissionStatus {
                permission_type: "screen_recording".to_string(),
                granted: false,
                required: true,
                description: "Screen recording permission needs to be rechecked".to_string(),
                instructions: "Grant screen recording permission during onboarding".to_string(),
            },
            microphone: crate::commands::permissions::PermissionStatus {
                permission_type: "microphone".to_string(),
                granted: false,
                required: false,
                description: "Microphone permission needs to be rechecked".to_string(),
                instructions: "Grant microphone permission if needed".to_string(),
            },
            input_monitoring: crate::commands::permissions::PermissionStatus {
                permission_type: "input_monitoring".to_string(),
                granted: false,
                required: true,
                description: "Input monitoring permission needs to be rechecked".to_string(),
                instructions: "Grant input monitoring permission during onboarding".to_string(),
            },
            all_granted: false,
            app_name: app.package_info().name.clone(),
        })
        .await;

    // Mark permissions as not checked so they will be re-evaluated
    // Reset the permissions checked flag
    if let Ok(mut checked_guard) = app_state.permissions_checked.lock() {
        *checked_guard = false;
    }

    info!("Onboarding reset - permissions state also cleared for fresh onboarding experience");
    Ok(())
}

/// Restart onboarding flow (reset and open onboarding window)
#[tauri::command]
pub async fn restart_onboarding(app: AppHandle) -> Result<(), String> {
    info!("Restarting onboarding flow...");

    // Reset onboarding status
    reset_onboarding(app.clone()).await?;

    // Open the onboarding window
    if let Err(e) = crate::window_management::open_onboarding_window(app.clone()).await {
        warn!("Failed to open onboarding window: {}", e);
        return Err(format!("Failed to open onboarding window: {}", e));
    }

    info!("Onboarding flow restarted successfully");
    Ok(())
}

/// Get detailed onboarding information
#[tauri::command]
pub async fn get_onboarding_info(app: AppHandle) -> Result<serde_json::Value, String> {
    let settings_manager = SettingsManager::new(app.clone()).map_err(|e| e.to_string())?;
    let onboarding_settings = settings_manager
        .get_onboarding_settings()
        .await
        .map_err(|e| e.to_string())?;

    // Get current keyboard shortcuts for the onboarding display
    let app_state = app.state::<crate::state::AppState>();
    let shortcuts = app_state
        .get_keyboard_shortcuts()
        .map_err(|e| format!("Failed to get keyboard shortcuts: {}", e))?;

    Ok(serde_json::json!({
        "completed": onboarding_settings.completed,
        "skip_count": onboarding_settings.skip_count,
        "completed_at": onboarding_settings.completed_at,
        "is_development_mode": is_development_mode(),
        "shortcuts": {
            "agent_mode": shortcuts.agent_mode,
            "dictation_input": shortcuts.dictation_input,
            "stop_current_task": shortcuts.stop_current_task
        }
    }))
}

/// Test if global shortcuts are working during onboarding
#[tauri::command]
pub async fn test_global_shortcuts_working(app: AppHandle) -> Result<bool, String> {
    // Check if we have Input Monitoring permissions first
    #[cfg(target_os = "macos")]
    {
        let has_permissions =
            crate::commands::shortcuts::check_input_monitoring_permissions().unwrap_or(false);

        if !has_permissions {
            info!("Input Monitoring permissions not granted - shortcuts won't work");
            return Ok(false);
        }
    }

    // Check if global shortcuts are registered
    let app_state = app.state::<crate::state::AppState>();
    let shortcuts = app_state
        .get_keyboard_shortcuts()
        .map_err(|e| format!("Failed to get keyboard shortcuts: {}", e))?;

    // Attempt to parse the shortcuts to see if they're valid
    let agent_shortcut_valid =
        crate::events::shortcuts::parse_shortcut_string(&shortcuts.agent_mode).is_some();
    let dictation_shortcut_valid =
        crate::events::shortcuts::parse_shortcut_string(&shortcuts.dictation_input).is_some();

    Ok(agent_shortcut_valid && dictation_shortcut_valid)
}

/// Set onboarding as active and start a listen-only escape key monitor.
/// Controls whether shortcut handlers suppress their normal actions (agent mode,
/// dictation, stop) and only emit visual feedback.
///
/// Uses a `CGEventTap` with `kCGEventTapOptionListenOnly` instead of a global
/// shortcut. This lets the Rust backend detect escape while the key still passes
/// through to HTML dropdowns, dialogs, and other applications.
///
/// Called by the frontend when the onboarding component mounts, and also by
/// `initialize_onboarding_system` in the backend. Idempotent — safe to call
/// multiple times with the same `active` value.
#[tauri::command]
pub async fn set_onboarding_active(app: AppHandle, active: bool) -> Result<(), String> {
    let app_state = app.state::<crate::state::AppState>();
    let was_active = app_state.is_onboarding_active();

    // Update the flag — shortcut handlers check this to suppress actions during onboarding
    app_state.set_onboarding_active(active);

    // Start/stop the listen-only escape key monitor on state transitions
    if active && !was_active {
        if let Err(e) = crate::platform::escape_key_monitor::start(&app) {
            error!("[Onboarding] Failed to start escape key monitor: {}", e);
        }
    } else if !active && was_active {
        crate::platform::escape_key_monitor::stop();
    }

    info!("[Onboarding] Active state set to: {} (was: {})", active, was_active);
    Ok(())
}

/// Initialize the onboarding system and check if onboarding should be shown
pub async fn initialize_onboarding_system(app_handle: AppHandle) -> Result<(), String> {
    info!("Initializing onboarding system...");

    // Check if onboarding has been completed (respects development mode)
    let onboarding_completed = check_onboarding_status(app_handle.clone()).await?;

    if !onboarding_completed {
        let mode = if is_development_mode() {
            "development"
        } else {
            "production"
        };
        info!(
            "Onboarding not completed in {} mode, opening onboarding window",
            mode
        );

        // CRITICAL: Set onboarding_active in the backend BEFORE opening the window.
        // This ensures shortcut handlers (dictation, agent) will see onboarding as active
        // immediately, without waiting for the frontend to mount and call back via invoke().
        // The frontend useEffect call to set_onboarding_active(true) is a no-op (idempotent).
        if let Err(e) = set_onboarding_active(app_handle.clone(), true).await {
            error!("[Onboarding] Failed to set onboarding active during init: {}", e);
        }

        // Open the onboarding window and give it focus
        if let Err(e) = crate::window_management::open_onboarding_window(app_handle.clone()).await {
            warn!("Failed to open onboarding window: {}", e);
            return Err(format!("Failed to open onboarding window: {}", e));
        }
    } else {
        info!("Onboarding already completed, showing main window");

        // Hide the onboarding window (it starts visible from tauri.conf.json)
        if let Err(e) = crate::window_management::close_onboarding_window(app_handle.clone()).await
        {
            warn!("Failed to close onboarding window: {}", e);
        }

        // Show the main window now that we know onboarding is done
        if let Err(e) = crate::window_management::open_main_window(app_handle.clone()).await {
            warn!("Failed to open main window: {}", e);
        }
    }

    Ok(())
}

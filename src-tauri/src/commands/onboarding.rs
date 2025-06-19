use tauri::{AppHandle, Manager};
use tauri_plugin_store::StoreExt;
use tracing::{info, warn};
use serde_json::Value;

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

    let store = app.store("onboarding.json").map_err(|e| e.to_string())?;

    // Check if onboarding has been completed
    let completed = store.get("completed").unwrap_or(Value::Bool(false));

    match completed {
        Value::Bool(true) => Ok(true),
        _ => Ok(false),
    }
}

/// Mark onboarding as completed
#[tauri::command]
pub async fn complete_onboarding(app: AppHandle) -> Result<(), String> {
    let store = app.store("onboarding.json").map_err(|e| e.to_string())?;
    let now = chrono::Utc::now().to_rfc3339();

    // Set onboarding as completed
    store.set("completed", Value::Bool(true));
    store.set("completed_at", Value::String(now.clone()));
    store.set("skipped", Value::Bool(false));

    // Save the store
    store.save().map_err(|e| e.to_string())?;

    info!("Onboarding marked as completed at {}", now);
    Ok(())
}

/// Mark onboarding as skipped (still counts as completed)
#[tauri::command]
pub async fn skip_onboarding(app: AppHandle) -> Result<(), String> {
    let store = app.store("onboarding.json").map_err(|e| e.to_string())?;
    let now = chrono::Utc::now().to_rfc3339();

    // Set onboarding as completed but skipped
    store.set("completed", Value::Bool(true));
    store.set("completed_at", Value::String(now.clone()));
    store.set("skipped", Value::Bool(true));

    // Save the store
    store.save().map_err(|e| e.to_string())?;

    info!("Onboarding skipped at {}", now);
    Ok(())
}

/// Reset onboarding (for testing/development and user-requested restart)
#[tauri::command]
pub async fn reset_onboarding(app: AppHandle) -> Result<(), String> {
    let store = app.store("onboarding.json").map_err(|e| e.to_string())?;

    // Reset all onboarding values
    store.set("completed", Value::Bool(false));
    store.set("skipped", Value::Bool(false));
    store.set("completed_at", Value::Null);

    // Save the store
    store.save().map_err(|e| e.to_string())?;

    // Reset permissions state so the permissions flow can be shown again during onboarding
    let app_state = app.state::<crate::state::AppState>();

    // Clear the permissions state in the app state
    app_state.update_permissions_state(crate::commands::permissions::PermissionsState {
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
    }).await;

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
    let store = app.store("onboarding.json").map_err(|e| e.to_string())?;

    let completed = store.get("completed").unwrap_or(Value::Bool(false));
    let skip_count = store.get("skip_count").unwrap_or(Value::Number(0.into()));
    let completed_at = store.get("completed_at");

    // Get current keyboard shortcuts for the onboarding display
    let app_state = app.state::<crate::state::AppState>();
    let shortcuts = app_state.keyboard_shortcuts.lock()
        .map_err(|e| format!("Failed to get keyboard shortcuts: {}", e))?
        .clone();

    Ok(serde_json::json!({
        "completed": completed,
        "skip_count": skip_count,
        "completed_at": completed_at,
        "is_development_mode": is_development_mode(),
        "shortcuts": {
            "agent_mode_toggle": shortcuts.agent_mode_toggle,
            "dictation_input": shortcuts.dictation_input
        }
    }))
}

/// Test if global shortcuts are working during onboarding
#[tauri::command]
pub async fn test_global_shortcuts_working(app: AppHandle) -> Result<bool, String> {
    // Check if we have Input Monitoring permissions first
    #[cfg(target_os = "macos")]
    {
        let has_permissions = crate::commands::shortcuts::check_input_monitoring_permissions()
            .unwrap_or(false);

        if !has_permissions {
            info!("Input Monitoring permissions not granted - shortcuts won't work");
            return Ok(false);
        }
    }

    // Check if global shortcuts are registered
    let app_state = app.state::<crate::state::AppState>();
    let shortcuts = app_state.keyboard_shortcuts.lock()
        .map_err(|e| format!("Failed to get keyboard shortcuts: {}", e))?
        .clone();

    // Attempt to parse the shortcuts to see if they're valid
    let agent_shortcut_valid = crate::events::shortcuts::parse_shortcut_string(&shortcuts.agent_mode_toggle).is_some();
    let dictation_shortcut_valid = crate::events::shortcuts::parse_shortcut_string(&shortcuts.dictation_input).is_some();

    Ok(agent_shortcut_valid && dictation_shortcut_valid)
}

/// Initialize the onboarding system and check if onboarding should be shown
pub async fn initialize_onboarding_system(app_handle: AppHandle) -> Result<(), String> {
    info!("Initializing onboarding system...");

    // Check if onboarding has been completed (respects development mode)
    let onboarding_completed = check_onboarding_status(app_handle.clone()).await?;

    if !onboarding_completed {
        let mode = if is_development_mode() { "development" } else { "production" };
        info!("Onboarding not completed in {} mode, opening onboarding window", mode);

        // Open the onboarding window
        if let Err(e) = crate::window_management::open_onboarding_window(app_handle.clone()).await {
            warn!("Failed to open onboarding window: {}", e);
            return Err(format!("Failed to open onboarding window: {}", e));
        }
    } else {
        info!("Onboarding already completed, skipping");
    }

    Ok(())
}

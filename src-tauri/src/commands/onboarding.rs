use tauri::{AppHandle, Manager};
use tauri_plugin_store::StoreExt;
use tracing::{info, warn};
use serde_json::Value;

/// Check if the user has completed onboarding
#[tauri::command]
pub async fn check_onboarding_status(app: AppHandle) -> Result<bool, String> {
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

/// Reset onboarding (for testing/development)
#[tauri::command]
pub async fn reset_onboarding(app: AppHandle) -> Result<(), String> {
    let store = app.store("onboarding.json").map_err(|e| e.to_string())?;

    // Reset all onboarding values
    store.set("completed", Value::Bool(false));
    store.set("skipped", Value::Bool(false));
    store.set("completed_at", Value::Null);

    // Save the store
    store.save().map_err(|e| e.to_string())?;

    info!("Onboarding reset");
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

    // Check if onboarding has been completed
    let onboarding_completed = check_onboarding_status(app_handle.clone()).await?;

    if !onboarding_completed {
        info!("Onboarding not completed, opening onboarding window");

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

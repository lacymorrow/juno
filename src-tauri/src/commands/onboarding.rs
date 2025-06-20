//! # Onboarding Commands
//!
//! Simple onboarding state management.

use crate::settings::SettingsManager;
use tauri::AppHandle;
use tracing::{info, error};

#[tauri::command]
pub async fn get_onboarding_state(app: AppHandle) -> Result<serde_json::Value, String> {
    let settings_manager = SettingsManager::new(app);
    let settings = settings_manager.get_settings();

    Ok(serde_json::json!({
        "completed": settings.onboarding.completed,
        "current_step": settings.onboarding.current_step
    }))
}

#[tauri::command]
pub async fn update_onboarding_step(
    app: AppHandle,
    step: u32,
) -> Result<(), String> {
    let settings_manager = SettingsManager::new(app);

    let onboarding_value = serde_json::json!({
        "completed": false,
        "current_step": step
    });

    settings_manager.update_section("onboarding", onboarding_value).await?;
    info!("✅ Onboarding step updated to: {}", step);
    Ok(())
}

#[tauri::command]
pub async fn complete_onboarding(app: AppHandle) -> Result<(), String> {
    let settings_manager = SettingsManager::new(app);

    let onboarding_value = serde_json::json!({
        "completed": true,
        "current_step": 0
    });

    settings_manager.update_section("onboarding", onboarding_value).await?;
    info!("✅ Onboarding completed");
    Ok(())
}

#[tauri::command]
pub async fn reset_onboarding(app: AppHandle) -> Result<(), String> {
    let settings_manager = SettingsManager::new(app);

    let onboarding_value = serde_json::json!({
        "completed": false,
        "current_step": 0
    });

    settings_manager.update_section("onboarding", onboarding_value).await?;
    info!("✅ Onboarding reset");
    Ok(())
}

#[tauri::command]
pub async fn get_onboarding_completion_status(app: AppHandle) -> Result<bool, String> {
    let settings_manager = SettingsManager::new(app);
    let settings = settings_manager.get_settings();
    Ok(settings.onboarding.completed)
}

/// Initialize onboarding system
pub async fn initialize_onboarding_system(app: AppHandle) -> Result<(), String> {
    info!("🎯 Initializing onboarding system");

    // Load current onboarding state
    let settings_manager = SettingsManager::new(app);
    let settings = settings_manager.get_settings();

    info!("📋 Onboarding state - completed: {}, step: {}",
          settings.onboarding.completed,
          settings.onboarding.current_step);

    info!("✅ Onboarding system initialized");
    Ok(())
}

//! The bridge between the update engine and the Settings window.
//!
//! Five commands, no logic. Every decision (which feed, when to look, whether
//! to install, what the person is told) belongs to `crate::updater`, so the
//! same behaviour holds with no window open at all.

use tauri::AppHandle;
use tracing::info;

use crate::settings::manager::SettingsManager;
use crate::settings::UpdateSettings;
use crate::updater::{self, UpdateChannel, UpdateStatus};

/// Where the update flow is right now. Cheap, and never touches the network,
/// so a window opening mid-download renders the real state immediately.
#[tauri::command]
pub async fn get_update_status() -> Result<UpdateStatus, String> {
    Ok(updater::current_status())
}

/// Look now, whatever the schedule says.
///
/// Returns as soon as the check is kicked off. Progress arrives on the
/// `update-status` event, which is the same channel the scheduled checks use,
/// so the UI has one path to render instead of two.
#[tauri::command]
pub async fn check_for_updates_now(app_handle: AppHandle) -> Result<(), String> {
    let (_, channel) = updater::settings_for(&app_handle).await?;
    info!(
        "[Updater] manual check against the {} channel",
        channel.as_str()
    );
    tauri::async_runtime::spawn(async move {
        updater::check_and_install(&app_handle, channel).await;
    });
    Ok(())
}

/// Relaunch into a version already installed on disk.
///
/// Only meaningful once the status reads `readyToRestart`. `restart` does not
/// return, so nothing after it runs.
#[tauri::command]
pub async fn restart_to_update(app_handle: AppHandle) -> Result<(), String> {
    info!("[Updater] restarting into the installed version");
    // `restart` diverges, so the Ok arm below is unreachable by construction.
    app_handle.restart()
}

#[tauri::command]
pub async fn get_update_settings(app_handle: AppHandle) -> Result<UpdateSettings, String> {
    SettingsManager::new(app_handle)?
        .get_update_settings()
        .await
}

/// Write the auto-check flag and the channel.
///
/// The channel is normalised on the way in, so what comes back out of the
/// store is always one the engine recognises: the UI cannot write a value
/// that silently disables updates.
#[tauri::command]
pub async fn set_update_settings(
    app_handle: AppHandle,
    auto_check_enabled: bool,
    channel: String,
) -> Result<UpdateSettings, String> {
    let normalised = UpdateChannel::from_stored(&channel);
    let settings = UpdateSettings {
        auto_check_enabled,
        channel: normalised.as_str().to_string(),
    };

    SettingsManager::new(app_handle)?
        .set_update_settings(&settings)
        .await?;

    info!(
        "[Updater] channel is {}, automatic checks are {}",
        normalised.as_str(),
        if auto_check_enabled { "on" } else { "off" }
    );
    Ok(settings)
}

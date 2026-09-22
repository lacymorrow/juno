//! App-side live-partial transcription setting. The STT model itself (which
//! engine, which file, downloads) lives in `stt_models.rs`; this file keeps the
//! one dictation display mode that is persisted in the central store and
//! re-applied at startup.

use std::sync::{Arc, Mutex};
use tauri::{AppHandle, Manager};
use tracing::{error, info, warn};

use tauri_plugin_voice_transcription::VoiceController;

use crate::settings::manager::SettingsManager;

/// Whether live streaming partial transcription is enabled (persisted).
#[tauri::command]
pub async fn get_live_partial_transcription(app: AppHandle) -> Result<bool, String> {
    let settings_manager = SettingsManager::new(app.clone())
        .map_err(|e| format!("Failed to create settings manager: {}", e))?;
    let voice_settings = settings_manager
        .get_voice_transcription_settings()
        .await
        .map_err(|e| format!("Failed to get voice settings: {}", e))?;
    Ok(voice_settings.live_partial_transcription)
}

/// Persist and apply the live-partial display mode. Display-only: live partials
/// render in Juno's own bar and are never typed into the focused app.
#[tauri::command]
pub async fn set_live_partial_transcription(enabled: bool, app: AppHandle) -> Result<(), String> {
    info!(
        "[STT] set_live_partial_transcription called with enabled: {}",
        enabled
    );

    let settings_manager = SettingsManager::new(app.clone())
        .map_err(|e| format!("Failed to create settings manager: {}", e))?;
    let mut voice_settings = settings_manager
        .get_voice_transcription_settings()
        .await
        .map_err(|e| format!("Failed to get voice settings: {}", e))?;
    voice_settings.live_partial_transcription = enabled;
    settings_manager
        .set_voice_transcription_settings(&voice_settings)
        .await
        .map_err(|e| format!("Failed to save voice settings: {}", e))?;

    apply_live_partial(&app, enabled);
    Ok(())
}

/// Apply the persisted live-partial mode at startup (best effort).
pub async fn apply_persisted_live_partial(app: &AppHandle) {
    let settings_manager = match SettingsManager::new(app.clone()) {
        Ok(sm) => sm,
        Err(e) => {
            warn!("[STT] Could not build settings manager at startup: {}", e);
            return;
        }
    };
    match settings_manager.get_voice_transcription_settings().await {
        Ok(s) => apply_live_partial(app, s.live_partial_transcription),
        Err(e) => warn!("[STT] Could not read voice settings at startup: {}", e),
    }
}

fn apply_live_partial(app: &AppHandle, enabled: bool) {
    if let Some(vc_state) = app.try_state::<Arc<Mutex<VoiceController>>>() {
        match vc_state.try_lock() {
            Ok(mut vc) => vc.set_live_partial(enabled),
            Err(std::sync::TryLockError::WouldBlock) => {
                warn!("[STT] VoiceController busy - live-partial applies on next recording");
            }
            Err(std::sync::TryLockError::Poisoned(e)) => {
                error!("[STT] VoiceController mutex poisoned: {}", e);
            }
        }
    }
}

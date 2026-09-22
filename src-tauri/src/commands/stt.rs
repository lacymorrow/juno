//! App-side live-partial transcription setting. The STT model itself (which
//! engine, which file, downloads) lives in `stt_models.rs`; this file keeps the
//! one dictation display mode that is persisted in the central store and
//! re-applied at startup.

use std::sync::{Arc, Mutex};
use tauri::{AppHandle, Listener, Manager};
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

    apply_live_partial(&app, enabled).await;
    Ok(())
}

/// Re-apply the persisted live-partial mode when the voice plugin reports its
/// engine ready, on top of the startup pass in `lib.rs`.
///
/// The plugin installs a *fresh* `VoiceController` from a background task once
/// its engine has loaded. Applying the saved flag only on a fixed timer raced
/// that swap: whenever the model loaded slowly, the flag landed on the
/// placeholder controller and the swap threw it away, so live transcription
/// stayed off for every recording until the toggle was flipped by hand. The
/// controller now carries the flag across the swap (`VoiceController::adopt`),
/// and this listener covers the opposite order — engine ready before the flag
/// was ever read. Applying it twice is idempotent.
pub fn apply_persisted_live_partial_when_engine_ready(app: &AppHandle) {
    let ready_app = app.clone();
    app.listen(
        crate::constants::events::voice_trigger::ENGINE_READY,
        move |_| {
            let app = ready_app.clone();
            tauri::async_runtime::spawn(async move {
                info!("[STT] Voice engine ready - re-applying persisted live-partial mode");
                apply_persisted_live_partial(&app).await;
            });
        },
    );
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
        Ok(s) => apply_live_partial(app, s.live_partial_transcription).await,
        Err(e) => warn!("[STT] Could not read voice settings at startup: {}", e),
    }
}

/// Set the live-partial flag on the voice controller. Waits for the lock off
/// the async runtime rather than giving up on a busy controller: the lock is
/// only held for long while a recording stops (the final decode), and a flag
/// dropped on `WouldBlock` was a toggle that quietly never took effect.
async fn apply_live_partial(app: &AppHandle, enabled: bool) {
    let Some(vc_state) = app.try_state::<Arc<Mutex<VoiceController>>>() else {
        warn!("[STT] VoiceController not managed - live-partial not applied");
        return;
    };
    let controller = Arc::clone(vc_state.inner());
    let applied = tauri::async_runtime::spawn_blocking(move || match controller.lock() {
        Ok(mut vc) => vc.set_live_partial(enabled),
        Err(e) => error!("[STT] VoiceController mutex poisoned: {}", e),
    })
    .await;
    if let Err(e) = applied {
        error!("[STT] Applying live-partial flag panicked: {}", e);
    }
}

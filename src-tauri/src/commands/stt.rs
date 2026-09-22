//! App-side STT (speech-to-text) engine commands.
//!
//! These wrap the voice-transcription plugin so the frontend can switch the STT
//! engine (Whisper / Parakeet) and the live-partial display mode with a single
//! bare `invoke`, matching Juno's `set_whisper_model` convention. Unlike the raw
//! plugin commands, these ALSO persist the choice to the central settings store
//! so it survives a restart, and the app honors the persisted value at startup
//! (see `lib.rs`). The plugin owns the live engine swap; the app owns settings.

use std::sync::{Arc, Mutex};
use tauri::{AppHandle, Emitter, Manager};
use tracing::{error, info, warn};

use tauri_plugin_voice_transcription::{
    resolve_model_path, AlwaysListeningController, EngineManager, SttProvider, TranscriptionEngine,
    VoiceController, VoiceTranscriptionConfig,
};

use crate::settings::manager::SettingsManager;

/// The persisted STT provider ("whisper" | "parakeet"). Read from the central
/// store so the UI shows the saved choice, which is what boots at startup.
#[tauri::command]
pub async fn get_stt_provider(app: AppHandle) -> Result<String, String> {
    let settings_manager = SettingsManager::new(app.clone())
        .map_err(|e| format!("Failed to create settings manager: {}", e))?;
    let voice_settings = settings_manager
        .get_voice_transcription_settings()
        .await
        .map_err(|e| format!("Failed to get voice settings: {}", e))?;
    Ok(voice_settings.stt_provider)
}

/// Persist the STT provider and swap the live engine. Persisting first means the
/// choice survives a restart even if the live swap fails (e.g. Parakeet model
/// files are not present yet); startup will retry the swap.
#[tauri::command]
pub async fn set_stt_provider(provider: String, app: AppHandle) -> Result<String, String> {
    info!("[STT] set_stt_provider called with provider: {}", provider);

    let stt_provider = match provider.to_lowercase().as_str() {
        "whisper" => SttProvider::Whisper,
        "parakeet" => SttProvider::Parakeet,
        other => return Err(format!("Unknown STT provider: '{}'", other)),
    };

    // Persist first.
    let settings_manager = SettingsManager::new(app.clone())
        .map_err(|e| format!("Failed to create settings manager: {}", e))?;
    let mut voice_settings = settings_manager
        .get_voice_transcription_settings()
        .await
        .map_err(|e| format!("Failed to get voice settings: {}", e))?;
    voice_settings.stt_provider = stt_provider.as_str().to_string();
    settings_manager
        .set_voice_transcription_settings(&voice_settings)
        .await
        .map_err(|e| format!("Failed to save voice settings: {}", e))?;

    // Live swap using the currently selected whisper model for the whisper path.
    let engine = switch_engine(&app, stt_provider, &voice_settings.model_path)?;
    let name = engine.name().to_string();
    apply_engine_to_controllers(&app, engine);

    let _ = app.emit(
        "stt-provider-switched",
        serde_json::json!({ "provider": name }),
    );
    info!("[STT] STT engine switched to '{}'", name);
    Ok(name)
}

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

/// The CPU architecture of this build (e.g. "aarch64", "x86_64"). The Models
/// pane uses this to hide the Parakeet (ONNX/ort) rows on non-arm64 builds.
#[tauri::command]
pub fn get_system_arch() -> String {
    std::env::consts::ARCH.to_string()
}

/// Apply the persisted STT provider and live-partial mode at startup. Best
/// effort: a failed engine swap (e.g. missing Parakeet files) is logged and the
/// default Whisper engine that the plugin already booted stays active.
pub async fn apply_persisted_stt_settings(app: &AppHandle) {
    let settings_manager = match SettingsManager::new(app.clone()) {
        Ok(sm) => sm,
        Err(e) => {
            warn!("[STT] Could not build settings manager at startup: {}", e);
            return;
        }
    };
    let voice_settings = match settings_manager.get_voice_transcription_settings().await {
        Ok(s) => s,
        Err(e) => {
            warn!("[STT] Could not read voice settings at startup: {}", e);
            return;
        }
    };

    apply_live_partial(app, voice_settings.live_partial_transcription);

    // The plugin boots Whisper by default, so only swap when a different engine
    // was chosen. Parakeet without model files will fail here; that is fine.
    if voice_settings.stt_provider.eq_ignore_ascii_case("parakeet") {
        match switch_engine(app, SttProvider::Parakeet, &voice_settings.model_path) {
            Ok(engine) => {
                info!("[STT] Honoring persisted provider 'parakeet' at startup");
                apply_engine_to_controllers(app, engine);
            }
            Err(e) => {
                warn!(
                    "[STT] Persisted provider 'parakeet' unavailable at startup ({}); staying on whisper",
                    e
                );
            }
        }
    }
}

fn switch_engine(
    app: &AppHandle,
    provider: SttProvider,
    whisper_model_path: &str,
) -> Result<Arc<dyn TranscriptionEngine>, String> {
    let config = VoiceTranscriptionConfig::default();
    let whisper_path = resolve_model_path(app, whisper_model_path);
    let parakeet_dir = resolve_model_path(app, &config.parakeet_model_dir);
    EngineManager::switch(provider, &whisper_path, Some(&parakeet_dir))
        .map_err(|e| format!("Failed to switch STT engine: {}", e))
}

fn apply_engine_to_controllers(app: &AppHandle, engine: Arc<dyn TranscriptionEngine>) {
    if let Some(vc_state) = app.try_state::<Arc<Mutex<VoiceController>>>() {
        match vc_state.try_lock() {
            Ok(mut vc) => {
                if let Err(e) = vc.update_engine(engine.clone()) {
                    warn!("[STT] Failed to update VoiceController engine: {}", e);
                }
            }
            Err(std::sync::TryLockError::WouldBlock) => {
                warn!("[STT] VoiceController busy - engine swap applies on next recording");
            }
            Err(std::sync::TryLockError::Poisoned(e)) => {
                error!("[STT] VoiceController mutex poisoned: {}", e);
            }
        }
    }

    if let Some(al_state) = app.try_state::<Arc<Mutex<AlwaysListeningController>>>() {
        match al_state.try_lock() {
            Ok(mut al) => {
                if let Err(e) = al.update_engine(engine) {
                    warn!(
                        "[STT] Failed to update AlwaysListeningController engine: {}",
                        e
                    );
                }
            }
            Err(std::sync::TryLockError::WouldBlock) => {
                warn!("[STT] AlwaysListeningController busy - engine swap applies on next session");
            }
            Err(std::sync::TryLockError::Poisoned(e)) => {
                error!("[STT] AlwaysListeningController mutex poisoned: {}", e);
            }
        }
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

use std::sync::{Arc, Mutex};
use tauri::{
    plugin::{Builder, TauriPlugin},
    Emitter, Manager, Runtime,
};

pub mod always_listening;
pub mod commands;
pub mod config;
pub mod constants;
pub mod controller;
pub mod engine;
pub mod engine_manager;
pub mod engine_parakeet;
pub mod engine_whisper;
pub mod error;
pub mod mic_permissions;
pub mod shared_whisper;
pub mod utils;

pub use always_listening::AlwaysListeningController;
pub use config::VoiceTranscriptionConfig;
pub use controller::VoiceController;
pub use engine::{SttProvider, TranscriptionEngine, TranscriptionSession};
pub use engine_manager::EngineManager;
pub use error::{Error, Result};
pub use shared_whisper::SharedWhisperManager;
pub use utils::resolve_model_path;

/// Initialize the Voice Transcription plugin
pub fn init<R: Runtime + 'static>() -> TauriPlugin<R> {
    Builder::<R>::new("voice-transcription")
        .invoke_handler(tauri::generate_handler![
            commands::start_dictation,
            commands::stop_dictation,
            commands::toggle_dictation,
            commands::get_dictation_status,
            commands::get_initialization_status,
            commands::transcribe_file,
            commands::set_model_path,
            commands::get_model_path,
            commands::start_always_listening,
            commands::stop_always_listening,
            commands::toggle_always_listening,
            commands::get_always_listening_status,
            commands::set_always_listening_sensitivity,
            commands::get_always_listening_sensitivity,
            commands::set_always_listening_wake_words,
            commands::get_always_listening_wake_words,
            commands::set_transcription_debugging,
            commands::set_audio_level_monitoring,
            commands::test_whisper_model,
            commands::force_transcription_test,
            commands::check_microphone_permission,
            commands::request_microphone_permission,
            commands::ensure_microphone_ready,
            commands::get_stt_provider,
            commands::set_stt_provider,
            commands::get_parakeet_model_status,
        ])
        .setup(move |app, _api| {
            tracing::info!("=== Voice Transcription Plugin Initialization Starting ===");

            // Check microphone permissions first
            tracing::info!("Checking microphone permissions...");
            let permission_status = mic_permissions::check_microphone_permission();
            match permission_status {
                mic_permissions::MicrophonePermissionStatus::Granted => {
                    tracing::info!("✅ Microphone permission is already granted");
                }
                mic_permissions::MicrophonePermissionStatus::Denied => {
                    tracing::warn!("⚠️ Microphone permission is denied. Voice features will not work until permission is granted.");
                    tracing::warn!("💡 Please grant microphone access in System Settings > Privacy & Security > Microphone");
                }
                mic_permissions::MicrophonePermissionStatus::Undetermined => {
                    tracing::info!("🔔 Microphone permission not yet requested. Will request when voice features are first used.");
                }
                mic_permissions::MicrophonePermissionStatus::NotApplicable => {
                    tracing::info!("ℹ️ Microphone permission check not applicable on this platform");
                }
            }

            // Get model path from config or use default
            let config = VoiceTranscriptionConfig::default();
            tracing::info!("Default config model path: {}", config.model_path);
            tracing::info!("Default STT provider: {}", config.stt_provider);

            // Try to resolve the model path for both development and production
            let resolved_model_path = resolve_model_path(app, &config.model_path);
            tracing::info!("Resolved model path: {}", resolved_model_path);

            // Check if resolved path exists before trying to create controller
            let model_path_exists = std::path::Path::new(&resolved_model_path).exists();
            tracing::info!("Model path exists: {}", model_path_exists);

            if !model_path_exists {
                tracing::warn!("Preferred model not found at: {}", resolved_model_path);
                // List available files in the models directory for debugging
                if let Ok(entries) = std::fs::read_dir("models") {
                    tracing::info!("Available files in models directory:");
                    for entry in entries.flatten() {
                        tracing::info!("  - {}", entry.path().display());
                    }
                } else {
                    tracing::warn!("Could not read models directory");
                }
                // Notify frontend so it can offer to download the preferred model
                let _ = app.emit("whisper-model-not-found", serde_json::json!({
                    "preferred": &config.model_path,
                    "resolved": &resolved_model_path
                }));
            }

            // Fallback: if preferred model missing, try tiny.en as a working fallback
            let active_model_path = if !model_path_exists {
                let fallback_path = resolve_model_path(app, "models/ggml-tiny.en.bin");
                if std::path::Path::new(&fallback_path).exists() {
                    tracing::info!("Falling back to tiny model: {}", fallback_path);
                    fallback_path
                } else {
                    tracing::warn!("Fallback tiny model also missing — voice will be unavailable until a model is downloaded");
                    resolved_model_path.clone()
                }
            } else {
                resolved_model_path.clone()
            };

            // Resolve Parakeet model directory
            let parakeet_model_dir = resolve_model_path(app, &config.parakeet_model_dir);

            // Manage uninitialized controllers immediately so Tauri state is always
            // valid (commands won't panic on missing state) even before the engine loads.
            let voice_arc = Arc::new(Mutex::new(VoiceController::new_uninitialized(
                &active_model_path,
                "Engine initializing in background".to_string(),
            )));
            let alc_arc = Arc::new(Mutex::new(AlwaysListeningController::new_uninitialized(
                &active_model_path,
            )));
            app.manage(voice_arc.clone());
            app.manage(alc_arc.clone());

            // Load the STT model in a background task — model files can be >1 GB
            // and would freeze the Tauri startup if loaded on the setup thread.
            let provider = config.stt_provider;
            let model_path_bg = active_model_path.clone();
            let parakeet_dir_bg = parakeet_model_dir.clone();
            let app_handle_bg = app.clone();

            tracing::info!("Spawning background task to initialize '{}' engine...", provider);
            tauri::async_runtime::spawn(async move {
                let model_path_bl = model_path_bg.clone();
                let parakeet_bl = parakeet_dir_bg.clone();

                let engine = match tokio::task::spawn_blocking(move || {
                    EngineManager::initialize(provider, &model_path_bl, Some(&parakeet_bl))
                })
                .await
                {
                    Ok(Ok(engine)) => engine,
                    Ok(Err(e)) => {
                        tracing::error!(
                            "[VoicePlugin] Failed to initialize '{}' engine: {}. Voice features unavailable.",
                            provider, e
                        );
                        if let Ok(mut vc) = voice_arc.lock() {
                            *vc = VoiceController::new_uninitialized(&model_path_bg, e);
                        }
                        return;
                    }
                    Err(join_err) => {
                        tracing::error!("[VoicePlugin] Engine init task panicked: {}", join_err);
                        return;
                    }
                };

                let engine_name = engine.name();
                tracing::info!("[VoicePlugin] Engine '{}' ready — updating controllers", engine_name);

                match VoiceController::new_with_engine(&model_path_bg, engine.clone()) {
                    Ok(new_vc) => {
                        if let Ok(mut vc) = voice_arc.lock() {
                            *vc = new_vc;
                            tracing::info!("[VoicePlugin] VoiceController initialized");
                        }
                    }
                    Err(e) => tracing::error!("[VoicePlugin] VoiceController creation failed: {}", e),
                }

                match AlwaysListeningController::new_with_engine(&model_path_bg, engine) {
                    Ok(new_alc) => {
                        if let Ok(mut alc) = alc_arc.lock() {
                            *alc = new_alc;
                            tracing::info!("[VoicePlugin] AlwaysListeningController initialized");
                        }
                    }
                    Err(e) => tracing::error!("[VoicePlugin] AlwaysListeningController creation failed: {}", e),
                }

                tracing::info!("=== Voice Transcription Plugin Initialization Complete (background) ===");
                let _ = app_handle_bg.emit(
                    "voice-engine-ready",
                    serde_json::json!({ "provider": engine_name }),
                );
            });

            Ok(())
        })
        .build()
}

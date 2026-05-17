use tauri::{plugin::{Builder, TauriPlugin}, Emitter, Manager, Runtime};
use std::sync::{Arc, Mutex};

pub mod controller;
pub mod commands;
pub mod error;
pub mod config;
pub mod utils;
pub mod always_listening;
pub mod shared_whisper;
pub mod constants;
pub mod mic_permissions;
pub mod engine;
pub mod engine_whisper;
pub mod engine_parakeet;
pub mod engine_manager;

pub use config::VoiceTranscriptionConfig;
pub use error::{Error, Result};
pub use controller::VoiceController;
pub use always_listening::AlwaysListeningController;
pub use utils::resolve_model_path;
pub use shared_whisper::SharedWhisperManager;
pub use engine::{SttProvider, TranscriptionEngine, TranscriptionSession};
pub use engine_manager::EngineManager;



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

            // Initialize STT engine via EngineManager (defaults to Whisper)
            tracing::info!("Initializing STT engine: '{}'", config.stt_provider);
            let engine = match EngineManager::initialize(
                config.stt_provider,
                &active_model_path,
                Some(&parakeet_model_dir),
            ) {
                Ok(engine) => {
                    tracing::info!("STT engine '{}' initialized successfully", engine.name());
                    engine
                }
                Err(e) => {
                    tracing::error!(
                        "Failed to initialize STT engine '{}': {}. Voice features will be unavailable.",
                        config.stt_provider, e
                    );

                    // Create uninitialized controllers so Tauri state is always managed
                    let uninitialized_controller =
                        VoiceController::new_uninitialized(&active_model_path, e.to_string());
                    app.manage(Arc::new(Mutex::new(uninitialized_controller)));

                    tracing::warn!("Skipping AlwaysListeningController initialization due to engine failure");
                    return Ok(());
                }
            };

            // Initialize VoiceController with the engine
            tracing::info!("Creating VoiceController with '{}' engine", engine.name());
            let controller = match VoiceController::new_with_engine(&active_model_path, engine.clone()) {
                Ok(c) => {
                    tracing::info!("VoiceController initialized");
                    c
                }
                Err(e) => {
                    tracing::error!("Failed to initialize VoiceController: {}. Creating uninitialized controller.", e);
                    VoiceController::new_uninitialized(&active_model_path, e.to_string())
                }
            };
            app.manage(Arc::new(Mutex::new(controller)));

            // Initialize AlwaysListeningController with the same engine
            tracing::info!("Creating AlwaysListeningController with '{}' engine", engine.name());
            let always_listening_controller =
                match AlwaysListeningController::new_with_engine(&active_model_path, engine) {
                    Ok(c) => {
                        tracing::info!("AlwaysListeningController initialized");
                        c
                    }
                    Err(e) => {
                        tracing::error!(
                            "Failed to initialize AlwaysListeningController: {}. Registering uninitialized controller.",
                            e
                        );
                        AlwaysListeningController::new_uninitialized(&active_model_path)
                    }
                };
            app.manage(Arc::new(Mutex::new(always_listening_controller)));

            tracing::info!("=== Voice Transcription Plugin Initialization Complete ===");
            tracing::info!("💡 Both controllers share the '{}' engine", EngineManager::current_provider_name());
            Ok(())
        })
        .build()
}

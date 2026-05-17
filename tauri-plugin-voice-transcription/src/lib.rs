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

pub use config::VoiceTranscriptionConfig;
pub use error::{Error, Result};
pub use controller::VoiceController;
pub use always_listening::AlwaysListeningController;
pub use utils::resolve_model_path;
pub use shared_whisper::SharedWhisperManager;



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

            // Initialize shared Whisper context ONCE for both controllers
            tracing::info!("Initializing shared Whisper context with path: {}", active_model_path);
            let shared_context = match SharedWhisperManager::initialize(&active_model_path) {
                Ok(context) => {
                    tracing::info!("Shared Whisper context initialized successfully");
                    let perf_info = SharedWhisperManager::get_performance_info();
                    tracing::info!("Performance: {}", serde_json::to_string_pretty(&perf_info).unwrap_or_default());
                    context
                }
                Err(e) => {
                    tracing::error!("Failed to initialize shared Whisper context: {}. Voice features will be unavailable.", e);

                    // Create uninitialized controllers
                    let uninitialized_controller = VoiceController::new_uninitialized(&active_model_path, e.to_string());
                    app.manage(Arc::new(Mutex::new(uninitialized_controller)));

                    tracing::warn!("Skipping AlwaysListeningController initialization due to shared context failure");
                    return Ok(());
                }
            };

            // Initialize voice controller using shared context
            tracing::info!("Creating VoiceController with shared Whisper context");
            let controller = match VoiceController::new_with_shared_context(&active_model_path, shared_context.clone()) {
                Ok(controller) => {
                    tracing::info!("VoiceController initialized with shared context");
                    controller
                }
                Err(e) => {
                    tracing::error!("Failed to initialize voice controller: {}. Creating uninitialized controller.", e);
                    VoiceController::new_uninitialized(&active_model_path, e.to_string())
                }
            };

            app.manage(Arc::new(Mutex::new(controller)));

            // Initialize always listening controller using shared context
            tracing::info!("Creating AlwaysListeningController with shared Whisper context");
            let always_listening_controller = match AlwaysListeningController::new_with_shared_context(&active_model_path, shared_context) {
                Ok(always_listening_controller) => {
                    tracing::info!("AlwaysListeningController initialized with shared context");
                    always_listening_controller
                }
                Err(e) => {
                    tracing::error!("Failed to initialize always listening controller: {}. Registering uninitialized controller.", e);
                    AlwaysListeningController::new_uninitialized(&active_model_path)
                }
            };

            app.manage(Arc::new(Mutex::new(always_listening_controller)));

            // Final status check
            let status = SharedWhisperManager::get_status();
            tracing::info!("🎯 Final shared Whisper status: {}", serde_json::to_string_pretty(&status).unwrap_or_default());

            tracing::info!("=== Voice Transcription Plugin Initialization Complete ===");
            tracing::info!("💡 Both controllers now share the same Whisper model instance - memory optimized!");
            Ok(())
        })
        .build()
}

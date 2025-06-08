use tauri::{plugin::{Builder, TauriPlugin}, Manager, Runtime};
use std::sync::{Arc, Mutex};

pub mod controller;
pub mod commands;
pub mod error;
pub mod config;
pub mod utils;
pub mod always_listening;

pub use config::VoiceTranscriptionConfig;
pub use error::{Error, Result};
pub use controller::VoiceController;
pub use always_listening::AlwaysListeningController;
pub use utils::resolve_model_path;



/// Initialize the Voice Transcription plugin
pub fn init<R: Runtime + 'static>() -> TauriPlugin<R> {
    Builder::<R>::new("voice-transcription")
        .invoke_handler(tauri::generate_handler![
            commands::start_dictation,
            commands::stop_dictation,
            commands::toggle_dictation,
            commands::get_dictation_status,
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
        ])
        .setup(move |app, _api| {
            tracing::info!("=== Voice Transcription Plugin Initialization Starting ===");
            
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
                tracing::error!("Model file does not exist at resolved path: {}", resolved_model_path);
                // List available files in the models directory for debugging
                if let Ok(entries) = std::fs::read_dir("models") {
                    tracing::info!("Available files in models directory:");
                    for entry in entries {
                        if let Ok(entry) = entry {
                            tracing::info!("  - {}", entry.path().display());
                        }
                    }
                } else {
                    tracing::warn!("Could not read models directory");
                }
            }

            // Initialize voice controller with resolved model path
            tracing::info!("Attempting to create VoiceController with path: {}", resolved_model_path);
            match VoiceController::new(&resolved_model_path) {
                Ok(controller) => {
                    app.manage(Arc::new(Mutex::new(controller)));
                    tracing::info!("✅ Voice transcription plugin initialized successfully with model: {}", resolved_model_path);
                }
                Err(e) => {
                    tracing::error!("❌ Failed to initialize voice controller: {}. Voice transcription will be unavailable.", e);
                    tracing::error!("Error details: {:?}", e);
                    // Note: We don't insert a controller here, so commands will need to handle the missing state
                }
            }

            // Initialize always listening controller with the same model path
            tracing::info!("Attempting to create AlwaysListeningController with path: {}", resolved_model_path);
            match AlwaysListeningController::new(&resolved_model_path) {
                Ok(always_listening_controller) => {
                    app.manage(Arc::new(Mutex::new(always_listening_controller)));
                    tracing::info!("✅ Always listening controller initialized successfully with model: {}", resolved_model_path);
                }
                Err(e) => {
                    tracing::error!("❌ Failed to initialize always listening controller: {}. Always listening will be unavailable.", e);
                    tracing::error!("Error details: {:?}", e);
                }
            }

            tracing::info!("=== Voice Transcription Plugin Initialization Complete ===");
            Ok(())
        })
        .build()
}

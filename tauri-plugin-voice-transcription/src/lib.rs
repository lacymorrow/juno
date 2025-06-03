use tauri::{plugin::{Builder, TauriPlugin}, Manager, Runtime};
use std::sync::{Arc, Mutex};

pub mod controller;
pub mod commands;
pub mod error;
pub mod config;

pub use config::VoiceTranscriptionConfig;
pub use error::{Error, Result};

pub use controller::VoiceController;

/// Resolve the model path for both development and production environments
fn resolve_model_path<R: Runtime>(app: &tauri::AppHandle<R>, default_path: &str) -> String {
    // First try the default path (for development)
    if std::path::Path::new(default_path).exists() {
        return default_path.to_string();
    }

    // Try to resolve as a bundled resource (for production)
    // The bundled resources are in _up_/models/ subdirectory
    if let Ok(resource_path) = app.path().resolve("_up_/models/ggml-tiny.en.bin", tauri::path::BaseDirectory::Resource) {
        if resource_path.exists() {
            if let Some(path_str) = resource_path.to_str() {
                return path_str.to_string();
            }
        }
    }

    // Also try the direct filename in case the structure changes
    if let Ok(resource_path) = app.path().resolve("ggml-tiny.en.bin", tauri::path::BaseDirectory::Resource) {
        if resource_path.exists() {
            if let Some(path_str) = resource_path.to_str() {
                return path_str.to_string();
            }
        }
    }

    // Fallback to the original path (will likely fail, but preserves error handling)
    default_path.to_string()
}

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
        ])
        .setup(move |app, _api| {
            // Get model path from config or use default
            let config = VoiceTranscriptionConfig::default();

            // Try to resolve the model path for both development and production
            let model_path = resolve_model_path(app, &config.model_path);

            // Initialize voice controller with resolved model path
            match VoiceController::new(&model_path) {
                Ok(controller) => {
                    app.manage(Arc::new(Mutex::new(controller)));
                    tracing::info!("Voice transcription plugin initialized with model: {}", model_path);
                }
                Err(e) => {
                    tracing::error!("Failed to initialize voice controller: {}. Voice transcription will be unavailable.", e);
                    // Note: We don't insert a controller here, so commands will need to handle the missing state
                }
            }

            Ok(())
        })
        .build()
}

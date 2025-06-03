use tauri::{plugin::{Builder, TauriPlugin}, Manager, Runtime};
use std::sync::{Arc, Mutex};

pub mod controller;
pub mod commands;
pub mod error;
pub mod config;

pub use config::VoiceTranscriptionConfig;
pub use error::{Error, Result};

pub use controller::VoiceController;

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

            // Initialize voice controller with model path from config
            match VoiceController::new(&config.model_path) {
                Ok(controller) => {
                    app.manage(Arc::new(Mutex::new(controller)));
                    tracing::info!("Voice transcription plugin initialized with model: {}", config.model_path);
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

//! Voice control module refactored for better organization and reduced complexity
//!
//! This module demonstrates how to break down a large, complex file (voice_control.rs)
//! into smaller, focused modules with clear responsibilities.

// pub mod audio_capture;
// pub mod transcription;
// pub mod resampling;
// pub mod controller;
pub mod types;

// Re-export the main controller for backwards compatibility
// pub use controller::VoiceController;

// Re-export common types
pub use types::{VoiceControllerConfig, AudioThreadMessage, TranscriptionResult};

use std::path::Path;
use crate::constants::audio;

/// Create a new VoiceController with default configuration
// pub fn new_voice_controller(model_path: &str) -> Result<VoiceController, String> {
//     let config = VoiceControllerConfig::default();
//     VoiceController::new_with_config(model_path, config)
// }

/// Validate that a Whisper model file exists and is readable
pub fn validate_model_path(model_path: &str) -> Result<(), String> {
    let path = Path::new(model_path);
    if !path.exists() {
        return Err(format!("Model path does not exist: {}", model_path));
    }

    if !path.is_file() {
        return Err(format!("Model path is not a file: {}", model_path));
    }

    // Try to open the file to check readability
    if let Err(e) = std::fs::File::open(path) {
        return Err(format!("Cannot read model file: {}", e));
    }

    Ok(())
}

/// Get the recommended sample rate for Whisper transcription
pub fn whisper_sample_rate() -> u32 {
    audio::WHISPER_SAMPLE_RATE
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_whisper_sample_rate() {
        assert_eq!(whisper_sample_rate(), audio::WHISPER_SAMPLE_RATE);
    }

    #[test]
    fn test_validate_model_path() {
        // Test with non-existent path
        assert!(validate_model_path("/path/that/does/not/exist").is_err());

        // Test with current file (should exist)
        let current_file = file!();
        assert!(validate_model_path(current_file).is_ok());
    }
}

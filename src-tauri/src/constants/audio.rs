//! # Audio Constants
//!
//! Audio processing and voice-related constants.

// Whisper configuration
pub const WHISPER_SAMPLE_RATE: u32 = 16000;
pub const SOUND_DEBOUNCE_MS: u64 = 300;
pub const DEFAULT_SENSITIVITY: f32 = 0.5;
pub const AUDIO_RECV_TIMEOUT_MS: u64 = 100;

// Audio processing configuration
pub mod processing {
    pub const SINC_LENGTH: usize = 256;
    pub const OVERSAMPLING_FACTOR: usize = 256;
    pub const AUDIO_RECV_TIMEOUT_MS: u64 = 100;
}

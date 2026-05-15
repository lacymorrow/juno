//! # Plugin Event Constants
//!
//! Local event constants for the voice transcription plugin.
//! These are the same as the main crate constants but defined locally
//! since the plugin is a separate crate.

/// Voice transcription events (from plugin)
pub mod voice_transcription {
    pub const FINAL_RESULT: &str = "voice-transcription:final-result";
    pub const DICTATION_STOPPED: &str = "voice-transcription:dictation-stopped";
    pub const ERROR: &str = "voice-transcription:error";
    // Plugin-specific events
    pub const DICTATION_STARTED: &str = "voice-transcription:dictation-started";
    pub const PARTIAL_RESULT: &str = "voice-transcription:partial-result";
    // Real-time audio level during active recording (0.0–1.0, emitted every ~70ms)
    pub const AUDIO_LEVEL: &str = "voice-audio-level";
}

/// Plugin system events
pub mod plugin {
    pub const VOICE_TRANSCRIPTION_DICTATION_STARTED: &str = "plugin:voice-transcription:dictation-started";
    pub const VOICE_TRANSCRIPTION_DICTATION_STOPPED: &str = "plugin:voice-transcription:dictation-stopped";
    pub const ALWAYS_LISTENING_STARTED: &str = "plugin:always-listening:started";
    pub const ALWAYS_LISTENING_STOPPED: &str = "plugin:always-listening:stopped";
}

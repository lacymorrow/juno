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
    pub const AUDIO_LEVEL: &str = "voice-transcription:audio-level";
}

/// Plugin system events
pub mod plugin {
    pub const VOICE_TRANSCRIPTION_DICTATION_STARTED: &str =
        "plugin:voice-transcription:dictation-started";
    pub const VOICE_TRANSCRIPTION_DICTATION_STOPPED: &str =
        "plugin:voice-transcription:dictation-stopped";
    pub const ALWAYS_LISTENING_STARTED: &str = "plugin:always-listening:started";
    pub const ALWAYS_LISTENING_STOPPED: &str = "plugin:always-listening:stopped";
}

/// Always-listening (wake word) events. The host app listens for these by the
/// same names; a test in src-tauri asserts the two copies agree.
pub mod always_listening {
    pub const STARTED: &str = "always-listening:started";
    pub const ACTIVATED: &str = "always-listening:activated";
    pub const DEACTIVATED: &str = "always-listening:deactivated";
    pub const STOP_REQUESTED: &str = "always-listening:stop-requested";
    pub const TRANSCRIPTION: &str = "always-listening:transcription";
    pub const COMMAND_PROCESSED: &str = "always-listening:command-processed";
    pub const EVENT: &str = "always-listening-event";
}

/// Speech engine lifecycle events
pub mod engine {
    /// The speech engine finished loading in the background.
    pub const READY: &str = "voice-engine-ready";
    /// No Whisper model file was found at startup.
    pub const WHISPER_MODEL_NOT_FOUND: &str = "whisper-model-not-found";
}

/// The host app's bundle identifier, which names its Application Support
/// folder. Owned by src-tauri/tauri.conf.json; a test in src-tauri asserts
/// this copy matches.
pub const HOST_BUNDLE_IDENTIFIER: &str = "com.juno.desktop";

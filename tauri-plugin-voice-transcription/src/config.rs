use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VoiceTranscriptionConfig {
    /// Path to the Whisper model file
    #[serde(default = "default_model_path")]
    pub model_path: String,

    /// Sample rate for audio recording (Hz)
    #[serde(default = "default_sample_rate")]
    pub sample_rate: u32,

    /// Number of channels in the audio recording
    #[serde(default = "default_channels")]
    pub channels: u16,

    /// Buffer duration for partial transcriptions (ms)
    #[serde(default = "default_buffer_duration_ms")]
    pub buffer_duration_ms: u64,

    /// Interval between partial transcriptions (ms)
    #[serde(default = "default_partial_interval_ms")]
    pub partial_interval_ms: u64,

    /// Enable partial transcription results
    #[serde(default = "default_enable_partial_transcription")]
    pub enable_partial_transcription: bool,

    /// Enable playback of the transcription
    #[serde(default = "default_enable_playback")]
    pub enable_playback: bool,
}

impl Default for VoiceTranscriptionConfig {
    fn default() -> Self {
        Self {
            model_path: default_model_path(),
            sample_rate: default_sample_rate(),
            channels: default_channels(),
            buffer_duration_ms: default_buffer_duration_ms(),
            partial_interval_ms: default_partial_interval_ms(),
            enable_partial_transcription: default_enable_partial_transcription(),
            enable_playback: default_enable_playback(),
        }
    }
}

impl VoiceTranscriptionConfig {
    /// Create configuration from centralized settings values
    pub fn from_centralized_settings(
        model_path: String,
        sample_rate: u32,
        channels: u16,
        buffer_duration_ms: u64,
        partial_interval_ms: u64,
        enable_partial_transcription: bool,
        enable_playback: bool,
    ) -> Self {
        Self {
            model_path,
            sample_rate,
            channels,
            buffer_duration_ms,
            partial_interval_ms,
            enable_partial_transcription,
            enable_playback,
        }
    }
}

fn default_model_path() -> String {
    "models/ggml-large-v3-turbo-q5_0.bin".to_string()
}

fn default_sample_rate() -> u32 {
    16000
}

fn default_channels() -> u16 {
    1
}

fn default_buffer_duration_ms() -> u64 {
    1500
}

fn default_partial_interval_ms() -> u64 {
    500
}

fn default_enable_partial_transcription() -> bool {
    true
}

fn default_enable_playback() -> bool {
    true
}

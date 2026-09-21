use serde::{Deserialize, Serialize};

/// Which STT backend to use. Serializes to lowercase strings for Tauri Store.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum SttProvider {
    #[default]
    Whisper,
    Parakeet,
}

impl SttProvider {
    pub fn as_str(self) -> &'static str {
        match self {
            SttProvider::Whisper => "whisper",
            SttProvider::Parakeet => "parakeet",
        }
    }
}

impl std::fmt::Display for SttProvider {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Per-recording stateful session — analogous to `whisper_rs::WhisperState`.
///
/// Created by `TranscriptionEngine::create_session` and lives for the duration
/// of one recording. Audio threads own an exclusive `Box<dyn TranscriptionSession>`
/// so there are no concurrent calls to the same session.
pub trait TranscriptionSession: Send {
    /// Fast partial transcription (Greedy quality). Returns `None` if there is
    /// no meaningful text in the audio.
    fn transcribe_partial(&mut self, audio: &[f32]) -> Result<Option<String>, String>;

    /// High-quality final transcription for the full session audio (BeamSearch
    /// quality for Whisper; full-batch ONNX pass for Parakeet).
    fn transcribe_final(&mut self, audio: &[f32]) -> Result<String, String>;
}

/// Pluggable STT engine. Shared across controllers via `Arc<dyn TranscriptionEngine>`.
///
/// Implementors: `WhisperEngine`, `ParakeetEngine`.
/// Both hold immutable (or internally-mutex-guarded) model state and are `Send + Sync`.
pub trait TranscriptionEngine: Send + Sync {
    /// Short identifier used in logging and settings persistence.
    fn name(&self) -> &'static str;

    /// Whether this engine supports native streaming (chunk-by-chunk) transcription.
    fn supports_streaming(&self) -> bool;

    /// Whether the engine loaded its model successfully and is ready to use.
    fn is_initialized(&self) -> bool;

    /// Create a fresh per-recording session. Cheap for Whisper (creates WhisperState
    /// from shared weights); may involve model warm-up for Parakeet.
    fn create_session(&self) -> Result<Box<dyn TranscriptionSession>, String>;
}

#[cfg(test)]
mod tests {
    use super::*;

    // The whole Intel story rests on this: a user who chose Parakeet on Apple
    // Silicon has "parakeet" written to their Tauri Store. That value must still
    // deserialize on every architecture (including Intel, or after a config
    // sync) so the settings load never fails. The variant is deliberately not
    // cfg-gated; only its engine implementation is. This test locks that in.
    #[test]
    fn parakeet_setting_deserializes_on_every_arch() {
        let provider: SttProvider =
            serde_json::from_str("\"parakeet\"").expect("stored \"parakeet\" must deserialize");
        assert_eq!(provider, SttProvider::Parakeet);
        assert_eq!(provider.as_str(), "parakeet");
    }

    #[test]
    fn stt_provider_round_trips_through_json() {
        for provider in [SttProvider::Whisper, SttProvider::Parakeet] {
            let json = serde_json::to_string(&provider).expect("serialize");
            let back: SttProvider = serde_json::from_str(&json).expect("deserialize");
            assert_eq!(provider, back);
        }
    }

    #[test]
    fn default_provider_is_whisper() {
        // The safety argument for making Parakeet arm64-only depends on Whisper
        // being the default, so Intel users who never touched the setting are
        // unaffected.
        assert_eq!(SttProvider::default(), SttProvider::Whisper);
    }
}

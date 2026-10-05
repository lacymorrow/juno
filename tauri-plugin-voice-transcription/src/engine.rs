use serde::{Deserialize, Serialize};

/// Which STT backend to use. Serializes to lowercase strings for Tauri Store.
///
/// Every variant exists on every architecture, deliberately. Parakeet only
/// *runs* on Apple Silicon (see [`crate::parakeet_model`]), but a person who
/// chose it there has `"parakeet"` written to their Tauri Store, and that
/// value has to keep deserializing on an Intel Mac rather than failing the
/// whole settings load. Only the implementation is gated; the choice is
/// always a legal thing to have saved.
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

    /// Final-quality transcription of one segment of a longer utterance, cut
    /// at a pause (see [`crate::streaming_commit`]). `context` is the text
    /// already transcribed before this segment; an engine that can condition
    /// on prior text (Whisper's initial prompt) should, so the seams read
    /// like one decode. The default ignores it.
    fn transcribe_final_segment(&mut self, audio: &[f32], context: &str) -> Result<String, String> {
        let _ = context;
        self.transcribe_final(audio)
    }
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

/// Which engine to boot at startup.
///
/// `saved` is the provider name the host app has persisted (`None` when the
/// setting is absent, unreadable, or the host handed us no reader at all).
/// `parakeet_ready` says whether Parakeet can actually be loaded: this build
/// contains it (Apple Silicon only) *and* every model file is on disk. See
/// [`crate::parakeet_model::parakeet_ready`], which is what callers pass.
///
/// Anything unrecognised, and any Parakeet request we cannot honour, resolves
/// to Whisper — the engine this plugin has always booted. Getting this right
/// up front is the whole point: booting the wrong engine means loading a
/// model, allocating its Metal buffers and warming it up, only to free all of
/// it seconds later when the app applies the saved preference.
pub fn startup_provider(saved: Option<&str>, parakeet_ready: bool) -> SttProvider {
    match saved.map(str::trim) {
        Some(name) if name.eq_ignore_ascii_case("parakeet") => {
            if parakeet_ready {
                SttProvider::Parakeet
            } else {
                SttProvider::Whisper
            }
        }
        _ => SttProvider::Whisper,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn saved_parakeet_boots_parakeet_when_the_model_is_on_disk() {
        assert_eq!(
            startup_provider(Some("parakeet"), true),
            SttProvider::Parakeet
        );
    }

    #[test]
    fn saved_parakeet_falls_back_to_whisper_when_the_model_is_missing() {
        assert_eq!(
            startup_provider(Some("parakeet"), false),
            SttProvider::Whisper
        );
    }

    #[test]
    fn saved_whisper_boots_whisper() {
        assert_eq!(
            startup_provider(Some("whisper"), true),
            SttProvider::Whisper
        );
    }

    #[test]
    fn no_saved_preference_boots_whisper() {
        assert_eq!(startup_provider(None, true), SttProvider::Whisper);
    }

    #[test]
    fn unknown_or_empty_names_boot_whisper() {
        for name in ["", "   ", "Parrakeet", "nvidia-parakeet", "null", "42"] {
            assert_eq!(
                startup_provider(Some(name), true),
                SttProvider::Whisper,
                "{name:?} should not have been recognised"
            );
        }
    }

    #[test]
    fn casing_and_whitespace_do_not_lose_the_preference() {
        for name in ["Parakeet", "PARAKEET", "  parakeet  "] {
            assert_eq!(
                startup_provider(Some(name), true),
                SttProvider::Parakeet,
                "{name:?} should have been recognised"
            );
        }
    }
}

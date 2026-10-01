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

/// Whether this build can run Parakeet at all.
///
/// Parakeet reaches ONNX Runtime through `parakeet-rs` -> `ort-sys`, whose
/// prebuilt distribution has no `x86_64-apple-darwin` entry, so the loader is
/// compiled on Apple Silicon only (see `Cargo.toml`). On Intel the model files
/// can still be described and even sit on disk; nothing can load them.
///
/// This is a separate question from "is the model downloaded". Conflating them
/// would report an Intel Mac as one download away from an engine it will never
/// have.
pub const fn parakeet_is_supported() -> bool {
    cfg!(target_arch = "aarch64")
}

/// Which engine to boot at startup.
///
/// `saved` is the provider name the host app has persisted (`None` when the
/// setting is absent, unreadable, or the host handed us no reader at all).
/// `parakeet_ready` says whether every Parakeet model file is on disk.
///
/// Anything unrecognised, and any Parakeet request we cannot honour, resolves
/// to Whisper — the engine this plugin has always booted. Getting this right
/// up front is the whole point: booting the wrong engine means loading a
/// model, allocating its Metal buffers and warming it up, only to free all of
/// it seconds later when the app applies the saved preference.
///
/// "Cannot honour" now includes the architecture. A settings store synced from
/// an Apple Silicon Mac says `"parakeet"` on an Intel one, and the files may
/// have come across with it; booting Parakeet there would log an engine that
/// does not exist and then fall back anyway.
pub fn startup_provider(saved: Option<&str>, parakeet_ready: bool) -> SttProvider {
    match saved.map(str::trim) {
        Some(name) if name.eq_ignore_ascii_case("parakeet") => {
            if parakeet_ready && parakeet_is_supported() {
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

    /// What a saved "parakeet" with its files on disk must resolve to on *this*
    /// machine: the engine on Apple Silicon, Whisper on Intel.
    ///
    /// Not a tautology — if `startup_provider` stopped consulting
    /// `parakeet_is_supported`, the Intel build of this test would get Parakeet
    /// and fail. Written against the architecture rather than hardcoded so the
    /// Intel half is genuinely checked, instead of passing only because CI runs
    /// on arm64.
    fn expected_when_parakeet_is_saved_and_downloaded() -> SttProvider {
        if parakeet_is_supported() {
            SttProvider::Parakeet
        } else {
            SttProvider::Whisper
        }
    }

    /// The case a synced settings store creates: an Intel Mac that inherited
    /// both the preference and the 612 MB of model files from an Apple Silicon
    /// one still boots Whisper.
    #[test]
    fn saved_parakeet_boots_parakeet_only_where_it_is_supported() {
        assert_eq!(
            startup_provider(Some("parakeet"), true),
            expected_when_parakeet_is_saved_and_downloaded()
        );
    }

    /// Parakeet is Apple Silicon only, and this is the line that says so.
    #[test]
    fn parakeet_support_follows_the_architecture() {
        assert_eq!(parakeet_is_supported(), cfg!(target_arch = "aarch64"));
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
        let expected = expected_when_parakeet_is_saved_and_downloaded();
        for name in ["Parakeet", "PARAKEET", "  parakeet  "] {
            assert_eq!(
                startup_provider(Some(name), true),
                expected,
                "{name:?} should have been recognised"
            );
        }
    }
}

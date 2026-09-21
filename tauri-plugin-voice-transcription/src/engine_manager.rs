use once_cell::sync::Lazy;
use std::sync::Arc;
use std::sync::RwLock;
use tracing::{info, warn};

use crate::engine::{SttProvider, TranscriptionEngine};
// Parakeet only compiles on aarch64 (see Cargo.toml / lib.rs), so its type and
// the std::path::Path it needs are pulled in only there. On Intel the Parakeet
// arm below never touches these, which is why they would otherwise be unused.
#[cfg(target_arch = "aarch64")]
use crate::engine_parakeet::ParakeetEngine;
#[cfg(target_arch = "aarch64")]
use std::path::Path;
use crate::engine_whisper::WhisperEngine;
use crate::shared_whisper::SharedWhisperManager;

static ACTIVE_ENGINE: Lazy<RwLock<Option<Arc<dyn TranscriptionEngine>>>> =
    Lazy::new(|| RwLock::new(None));

/// Global STT engine manager — analogous to `SharedWhisperManager` but provider-agnostic.
///
/// Both `VoiceController` and `AlwaysListeningController` call `EngineManager::get()`
/// to receive an `Arc<dyn TranscriptionEngine>` they hold for their lifetime. A hot-swap
/// via `EngineManager::switch()` replaces the global; existing clones keep the old engine
/// until they are reconstructed (usually on next `start_dictation`).
pub struct EngineManager;

impl EngineManager {
    /// Initialize the engine for `provider`. Returns the existing engine unchanged if
    /// it is already of the requested type — avoids reloading a 1+ GB model on startup.
    pub fn initialize(
        provider: SttProvider,
        whisper_model_path: &str,
        parakeet_model_dir: Option<&str>,
    ) -> Result<Arc<dyn TranscriptionEngine>, String> {
        // Fast path: engine already active and is the right type.
        {
            let guard = ACTIVE_ENGINE
                .read()
                .map_err(|e| format!("EngineManager read lock poisoned: {}", e))?;
            if let Some(engine) = guard.as_ref() {
                if engine.name() == provider.as_str() {
                    info!(
                        "[EngineManager] Engine '{}' already initialized",
                        engine.name()
                    );
                    return Ok(engine.clone());
                }
            }
        }

        Self::switch(provider, whisper_model_path, parakeet_model_dir)
    }

    /// Force a provider switch — loads the new engine and installs it globally.
    /// Existing `Arc` clones held by audio threads keep the old engine until their
    /// recording finishes; new recordings pick up the new engine.
    pub fn switch(
        provider: SttProvider,
        whisper_model_path: &str,
        parakeet_model_dir: Option<&str>,
    ) -> Result<Arc<dyn TranscriptionEngine>, String> {
        info!("[EngineManager] Switching to '{}' engine", provider);
        let engine = Self::build_engine(provider, whisper_model_path, parakeet_model_dir)?;

        let mut guard = ACTIVE_ENGINE
            .write()
            .map_err(|e| format!("EngineManager write lock poisoned: {}", e))?;
        *guard = Some(engine.clone());

        info!("[EngineManager] Engine switched to '{}'", engine.name());
        Self::warm_up(engine.clone());
        Ok(engine)
    }

    /// Run one throwaway decode on a second of silence so first-use costs (Metal shader
    /// compile, ONNX graph init, memory mapping the weights) land now instead of on the
    /// user's first dictation.
    fn warm_up(engine: Arc<dyn TranscriptionEngine>) {
        std::thread::Builder::new()
            .name("stt-warm-up".into())
            .spawn(move || {
                let started = std::time::Instant::now();
                match engine.create_session() {
                    Ok(mut session) => {
                        let silence = vec![0.0f32; 16_000];
                        let _ = session.transcribe_partial(&silence);
                        info!(
                            "[EngineManager] '{}' warm-up finished in {:?}",
                            engine.name(),
                            started.elapsed()
                        );
                    }
                    Err(e) => warn!("[EngineManager] warm-up skipped: {}", e),
                }
            })
            .ok();
    }

    /// Return the currently active engine. Returns an error if none has been initialized.
    pub fn get() -> Result<Arc<dyn TranscriptionEngine>, String> {
        let guard = ACTIVE_ENGINE
            .read()
            .map_err(|e| format!("EngineManager read lock poisoned: {}", e))?;
        guard.as_ref().cloned().ok_or_else(|| {
            "No STT engine initialized. Call EngineManager::initialize() first.".to_string()
        })
    }

    pub fn is_initialized() -> bool {
        ACTIVE_ENGINE.read().map(|g| g.is_some()).unwrap_or(false)
    }

    /// Current engine name, or "none" if not initialized.
    pub fn current_provider_name() -> &'static str {
        ACTIVE_ENGINE
            .read()
            .ok()
            .and_then(|g| g.as_ref().map(|e| e.name()))
            .unwrap_or("none")
    }

    fn build_engine(
        provider: SttProvider,
        whisper_model_path: &str,
        parakeet_model_dir: Option<&str>,
    ) -> Result<Arc<dyn TranscriptionEngine>, String> {
        // `parakeet_model_dir` is consumed only by the aarch64 Parakeet arm; on
        // Intel the Parakeet arm ignores it and falls back to Whisper, so mark it
        // used to keep the shared signature warning-free.
        #[cfg(not(target_arch = "aarch64"))]
        let _ = parakeet_model_dir;

        match provider {
            SttProvider::Whisper => {
                let ctx = SharedWhisperManager::initialize(whisper_model_path)
                    .map_err(|e| e.to_string())?;
                Ok(Arc::new(WhisperEngine::new(ctx)))
            }
            // Apple Silicon: build the real Parakeet engine.
            #[cfg(target_arch = "aarch64")]
            SttProvider::Parakeet => {
                let dir = parakeet_model_dir.ok_or_else(|| {
                    "Parakeet model directory not configured. \
                     Set `parakeet_model_dir` in voice transcription config."
                        .to_string()
                })?;
                if !ParakeetEngine::model_files_present(Path::new(dir)) {
                    warn!(
                        "[EngineManager] Parakeet model files not yet downloaded at '{}'. \
                         Use the download_parakeet_model command to fetch them.",
                        dir
                    );
                }
                let engine = ParakeetEngine::new(Path::new(dir))?;
                Ok(Arc::new(engine))
            }
            // Intel: parakeet-rs has no x86_64 build, so a stored "parakeet"
            // selection is honoured as best we can by falling back to Whisper.
            // The setting is accepted (the variant still deserializes), it just
            // cannot be run here, the same way an unavailable model falls back.
            #[cfg(not(target_arch = "aarch64"))]
            SttProvider::Parakeet => {
                warn!("Parakeet is Apple Silicon only; using Whisper on this Mac");
                let ctx = SharedWhisperManager::initialize(whisper_model_path)
                    .map_err(|e| e.to_string())?;
                Ok(Arc::new(WhisperEngine::new(ctx)))
            }
        }
    }
}

#[cfg(all(test, not(target_arch = "aarch64")))]
mod intel_fallback_tests {
    use super::*;

    // On Intel there is no Parakeet build, so selecting Parakeet must resolve to
    // the Whisper path. We prove which arm ran by the error surface: the Whisper
    // arm fails trying to load a missing model file, never with the Parakeet
    // "model directory not configured" message the aarch64 arm would emit. This
    // avoids loading a multi-GB model just to assert the routing.
    #[test]
    fn parakeet_selection_falls_back_to_whisper_on_intel() {
        let err = EngineManager::build_engine(
            SttProvider::Parakeet,
            "/nonexistent/juno-whisper-model.bin",
            None,
        )
        .expect_err("a missing whisper model path should error");
        assert!(
            !err.contains("Parakeet model directory"),
            "Intel must take the Whisper fallback arm, not the Parakeet arm; got: {err}"
        );
    }
}

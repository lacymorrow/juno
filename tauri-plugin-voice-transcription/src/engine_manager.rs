use once_cell::sync::Lazy;
use std::path::Path;
use std::sync::Arc;
use std::sync::RwLock;
use tracing::{info, warn};

use crate::engine::{SttProvider, TranscriptionEngine};
use crate::engine_parakeet::{missing_parakeet_files, ParakeetEngine};
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

    /// Load a different Whisper model file and install it as the active engine.
    /// `switch(Whisper, ..)` keeps whatever context is already loaded (so a
    /// restart does not reload a 1+ GB file); this is the path that replaces it.
    pub fn reload_whisper(
        whisper_model_path: &str,
    ) -> Result<Arc<dyn TranscriptionEngine>, String> {
        info!(
            "[EngineManager] Reloading Whisper with '{}'",
            whisper_model_path
        );
        if !Path::new(whisper_model_path).is_file() {
            return Err(format!(
                "Whisper model file not found at {}",
                whisper_model_path
            ));
        }
        let ctx = SharedWhisperManager::reinitialize(whisper_model_path)
            .map_err(|e| format!("Whisper could not load {}: {}", whisper_model_path, e))?;
        let engine: Arc<dyn TranscriptionEngine> = Arc::new(WhisperEngine::new(ctx));

        let mut guard = ACTIVE_ENGINE
            .write()
            .map_err(|e| format!("EngineManager write lock poisoned: {}", e))?;
        *guard = Some(engine.clone());
        drop(guard);

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
        match provider {
            SttProvider::Whisper => {
                if !Path::new(whisper_model_path).is_file() {
                    return Err(format!(
                        "Whisper model file not found at {}. Download it from Settings > Models.",
                        whisper_model_path
                    ));
                }
                let ctx = SharedWhisperManager::initialize(whisper_model_path)
                    .map_err(|e| format!("Whisper could not load {}: {}", whisper_model_path, e))?;
                Ok(Arc::new(WhisperEngine::new(ctx)))
            }
            SttProvider::Parakeet => {
                let dir = parakeet_model_dir.ok_or_else(|| {
                    "Parakeet model directory not configured. \
                     Set `parakeet_model_dir` in voice transcription config."
                        .to_string()
                })?;
                let missing = missing_parakeet_files(Path::new(dir));
                if !missing.is_empty() {
                    // Say which files, so the caller (and the log) can tell a
                    // never-downloaded model from a half-finished one.
                    return Err(format!(
                        "Parakeet is not downloaded (missing {} in {}). Download it from Settings > Models.",
                        missing.join(", "),
                        dir
                    ));
                }
                let engine = ParakeetEngine::new(Path::new(dir))?;
                Ok(Arc::new(engine))
            }
        }
    }
}

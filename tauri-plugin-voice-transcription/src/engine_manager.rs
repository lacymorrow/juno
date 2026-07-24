use once_cell::sync::Lazy;
use std::path::Path;
use std::sync::Arc;
use std::sync::RwLock;
use tracing::{info, warn};

use crate::engine::{SttProvider, TranscriptionEngine};
use crate::engine_parakeet::ParakeetEngine;
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
        Ok(engine)
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
                let ctx = SharedWhisperManager::initialize(whisper_model_path)
                    .map_err(|e| e.to_string())?;
                Ok(Arc::new(WhisperEngine::new(ctx)))
            }
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
        }
    }
}

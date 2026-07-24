use crate::engine::{TranscriptionEngine, TranscriptionSession};
use parakeet_rs::{Parakeet, Transcriber};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use tracing::{error, info};

/// STT engine backed by NVIDIA Parakeet CTC 0.6B via ONNX Runtime.
///
/// Model directory must contain: `model.onnx`, `model.onnx_data`, `tokenizer.json`.
/// Download from: https://huggingface.co/onnx-community/parakeet-ctc-0.6b-ONNX/tree/main/onnx
///
/// The inner `Parakeet` is guarded by a Mutex because `transcribe_samples` takes
/// `&mut self`. In practice only one recording is active at a time, so contention
/// is not a concern.
pub struct ParakeetEngine {
    model: Arc<Mutex<Option<Parakeet>>>,
    pub model_dir: PathBuf,
}

impl ParakeetEngine {
    /// Load the Parakeet CTC model from `model_dir`. Returns an error if the
    /// directory does not exist or model files are missing.
    pub fn new(model_dir: &Path) -> Result<Self, String> {
        info!(
            "[ParakeetEngine] Loading Parakeet CTC model from {:?}",
            model_dir
        );

        if !model_dir.exists() {
            return Err(format!(
                "Parakeet model directory not found: {}. \
                 Download the ONNX model from \
                 https://huggingface.co/onnx-community/parakeet-ctc-0.6b-ONNX/tree/main/onnx",
                model_dir.display()
            ));
        }

        let model = Parakeet::from_pretrained(model_dir, None)
            .map_err(|e| format!("Failed to load Parakeet model from {:?}: {}", model_dir, e))?;

        info!("[ParakeetEngine] Parakeet model loaded successfully");

        Ok(Self {
            model: Arc::new(Mutex::new(Some(model))),
            model_dir: model_dir.to_path_buf(),
        })
    }

    /// Check whether the required model files are present without loading them.
    pub fn model_files_present(model_dir: &Path) -> bool {
        model_dir.exists()
            && model_dir.join("model.onnx").exists()
            && model_dir.join("tokenizer.json").exists()
    }
}

impl TranscriptionEngine for ParakeetEngine {
    fn name(&self) -> &'static str {
        "parakeet"
    }

    fn supports_streaming(&self) -> bool {
        false
    }

    fn is_initialized(&self) -> bool {
        self.model.lock().map(|g| g.is_some()).unwrap_or(false)
    }

    fn create_session(&self) -> Result<Box<dyn TranscriptionSession>, String> {
        Ok(Box::new(ParakeetSession {
            model: Arc::clone(&self.model),
        }))
    }
}

/// Per-recording Parakeet session. Shares the loaded ONNX model via Arc<Mutex<>>
/// so initialization cost is paid once per engine, not once per recording.
pub struct ParakeetSession {
    model: Arc<Mutex<Option<Parakeet>>>,
}

impl TranscriptionSession for ParakeetSession {
    fn transcribe_partial(&mut self, audio: &[f32]) -> Result<Option<String>, String> {
        if audio.is_empty() {
            return Ok(None);
        }

        let mut guard = self
            .model
            .lock()
            .map_err(|e| format!("Parakeet model lock poisoned: {}", e))?;

        let model = guard
            .as_mut()
            .ok_or_else(|| "Parakeet model not loaded".to_string())?;

        match model.transcribe_samples(audio.to_vec(), 16000, 1, None) {
            Ok(result) => {
                if result.text.trim().is_empty() {
                    Ok(None)
                } else {
                    Ok(Some(result.text))
                }
            }
            Err(e) => {
                error!("[ParakeetSession] Partial transcription failed: {}", e);
                Err(format!("Parakeet partial transcription failed: {}", e))
            }
        }
    }

    fn transcribe_final(&mut self, audio: &[f32]) -> Result<String, String> {
        if audio.is_empty() {
            return Ok(String::new());
        }

        let mut guard = self
            .model
            .lock()
            .map_err(|e| format!("Parakeet model lock poisoned: {}", e))?;

        let model = guard
            .as_mut()
            .ok_or_else(|| "Parakeet model not loaded".to_string())?;

        model
            .transcribe_samples(audio.to_vec(), 16000, 1, None)
            .map(|r| r.text)
            .map_err(|e| format!("Parakeet final transcription failed: {}", e))
    }
}

/// Metadata about the Parakeet model download state.
#[derive(Debug, serde::Serialize)]
pub struct ParakeetModelStatus {
    pub downloaded: bool,
    pub model_dir: String,
    pub files_present: Vec<String>,
    pub files_missing: Vec<String>,
}

impl ParakeetModelStatus {
    pub fn check(model_dir: &Path) -> Self {
        let required = ["model.onnx", "model.onnx_data", "tokenizer.json"];
        let mut present = Vec::new();
        let mut missing = Vec::new();

        for &file in &required {
            if model_dir.join(file).exists() {
                present.push(file.to_string());
            } else {
                missing.push(file.to_string());
            }
        }

        Self {
            downloaded: missing.is_empty(),
            model_dir: model_dir.to_string_lossy().into_owned(),
            files_present: present,
            files_missing: missing,
        }
    }
}

use crate::engine::{TranscriptionEngine, TranscriptionSession};
use parakeet_rs::{Parakeet, Transcriber};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use tracing::{error, info};

/// The HuggingFace repo the Parakeet CTC ONNX export is fetched from.
pub const PARAKEET_HF_REPO: &str = "onnx-community/parakeet-ctc-0.6b-ONNX";

/// Pinned revision of that repo. The byte counts in `PARAKEET_MODEL_FILES` are
/// verified after download, so the download must always fetch the exact commit
/// they were measured against, never a moving `main`.
pub const PARAKEET_HF_REVISION: &str = "7df2cab7aed886b8b7f80d68a8214007e4847601";

/// One file `Parakeet::from_pretrained(model_dir)` needs on disk.
#[derive(Debug, Clone, Copy)]
pub struct ParakeetFile {
    /// File name inside the model directory (what the loader looks for).
    pub name: &'static str,
    /// Path inside the HuggingFace repo.
    pub remote_path: &'static str,
    /// Exact size of the file at `PARAKEET_HF_REVISION`.
    pub bytes: u64,
}

/// The int8 export of Parakeet CTC 0.6B: 612 MB on disk, the same weight class
/// as Whisper large-v3-turbo q5_0, and the one parakeet-rs documents as the
/// quantized variant. The fp32 export (`model.onnx` + 2.4 GB of weights) loads
/// too, but it is four times the download for the same word error rate class,
/// which is the wrong default for something that fetches itself on first run.
///
/// `parakeet-rs` looks for `model.onnx`, then `model_fp16.onnx`, then
/// `model_int8.onnx` in the directory; with only the int8 files present it
/// picks the int8 graph, which loads its weights from `model_int8.onnx_data`
/// next to it.
pub const PARAKEET_MODEL_FILES: &[ParakeetFile] = &[
    ParakeetFile {
        name: "model_int8.onnx",
        remote_path: "onnx/model_int8.onnx",
        bytes: 1_303_007,
    },
    ParakeetFile {
        name: "model_int8.onnx_data",
        remote_path: "onnx/model_int8.onnx_data",
        bytes: 610_974_468,
    },
    ParakeetFile {
        name: "tokenizer.json",
        remote_path: "tokenizer.json",
        bytes: 412_363,
    },
];

/// Total download size of `PARAKEET_MODEL_FILES`.
pub fn parakeet_total_bytes() -> u64 {
    PARAKEET_MODEL_FILES.iter().map(|f| f.bytes).sum()
}

/// Download URL for one manifest entry at the pinned revision.
pub fn parakeet_file_url(file: &ParakeetFile) -> String {
    format!(
        "https://huggingface.co/{}/resolve/{}/{}",
        PARAKEET_HF_REPO, PARAKEET_HF_REVISION, file.remote_path
    )
}

/// Names of the manifest files that are not in `model_dir` (or are the wrong
/// size, which means a partial or foreign file the loader would choke on).
pub fn missing_parakeet_files(model_dir: &Path) -> Vec<&'static str> {
    PARAKEET_MODEL_FILES
        .iter()
        .filter(|f| {
            std::fs::metadata(model_dir.join(f.name))
                .map(|m| m.len() != f.bytes)
                .unwrap_or(true)
        })
        .map(|f| f.name)
        .collect()
}

/// STT engine backed by NVIDIA Parakeet CTC 0.6B via ONNX Runtime.
///
/// Model directory must contain every file in `PARAKEET_MODEL_FILES`; the app's
/// `download_dictation_model` command fetches them.
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

        let missing = missing_parakeet_files(model_dir);
        if !missing.is_empty() {
            return Err(format!(
                "Parakeet model files missing in {}: {}. Download Parakeet from Settings > Models.",
                model_dir.display(),
                missing.join(", ")
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

    /// Check whether the required model files are present (and complete)
    /// without loading them.
    pub fn model_files_present(model_dir: &Path) -> bool {
        missing_parakeet_files(model_dir).is_empty()
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

/// On-disk state of the Parakeet model directory.
#[derive(Debug, serde::Serialize)]
pub struct ParakeetModelStatus {
    pub downloaded: bool,
    pub model_dir: String,
    pub files_present: Vec<String>,
    pub files_missing: Vec<String>,
    /// Bytes the complete model occupies once downloaded.
    pub total_bytes: u64,
}

impl ParakeetModelStatus {
    pub fn check(model_dir: &Path) -> Self {
        let missing = missing_parakeet_files(model_dir);
        let present = PARAKEET_MODEL_FILES
            .iter()
            .map(|f| f.name)
            .filter(|name| !missing.contains(name))
            .map(str::to_string)
            .collect();

        Self {
            downloaded: missing.is_empty(),
            model_dir: model_dir.to_string_lossy().into_owned(),
            files_present: present,
            files_missing: missing.into_iter().map(str::to_string).collect(),
            total_bytes: parakeet_total_bytes(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn manifest_is_the_int8_export_the_loader_finds() {
        let names: Vec<_> = PARAKEET_MODEL_FILES.iter().map(|f| f.name).collect();
        assert_eq!(
            names,
            ["model_int8.onnx", "model_int8.onnx_data", "tokenizer.json"]
        );
        // 612 MB, the number the Models pane shows.
        assert_eq!(parakeet_total_bytes(), 612_689_838);
        assert!(parakeet_file_url(&PARAKEET_MODEL_FILES[0]).contains(PARAKEET_HF_REVISION));
    }

    #[test]
    fn a_partial_file_counts_as_missing() {
        let dir = std::env::temp_dir().join(format!("juno-parakeet-test-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        assert_eq!(missing_parakeet_files(&dir).len(), 3);

        std::fs::write(dir.join("tokenizer.json"), b"{}").unwrap();
        let missing = missing_parakeet_files(&dir);
        assert!(
            missing.contains(&"tokenizer.json"),
            "wrong size is not downloaded"
        );

        let status = ParakeetModelStatus::check(&dir);
        assert!(!status.downloaded);
        assert_eq!(status.files_missing.len(), 3);
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// Real load + transcription against downloaded files. Run once by hand:
    /// `JUNO_PARAKEET_DIR=<dir> JUNO_PARAKEET_WAV=<16 kHz mono wav> cargo test -p tauri-plugin-voice-transcription parakeet_loads -- --ignored --nocapture`
    #[test]
    #[ignore]
    fn parakeet_loads_and_transcribes_downloaded_files() {
        let dir = std::env::var("JUNO_PARAKEET_DIR").expect("JUNO_PARAKEET_DIR");
        let wav = std::env::var("JUNO_PARAKEET_WAV").expect("JUNO_PARAKEET_WAV");
        let engine = ParakeetEngine::new(Path::new(&dir)).expect("engine loads");
        assert!(engine.is_initialized());

        let mut reader = hound::WavReader::open(&wav).expect("wav opens");
        assert_eq!(reader.spec().sample_rate, 16_000);
        let samples: Vec<f32> = reader
            .samples::<i16>()
            .map(|s| s.unwrap() as f32 / i16::MAX as f32)
            .collect();

        let mut session = engine.create_session().expect("session");
        let text = session.transcribe_final(&samples).expect("transcribes");
        println!("parakeet said: {text:?}");
        assert!(text.to_lowercase().contains("fox"), "got {text:?}");
    }
}

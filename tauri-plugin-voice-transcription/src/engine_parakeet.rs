//! The Parakeet engine itself: loading the ONNX graph and running audio
//! through it.
//!
//! Compiled on Apple Silicon only. `parakeet-rs` pulls in `ort-sys`, which
//! ships a prebuilt ONNX Runtime for `aarch64-apple-darwin` and nothing else,
//! so this module (and that dependency) are gated on `target_arch =
//! "aarch64"` and the crate is built universal without them. Everything that
//! does not touch the loader lives in [`crate::parakeet_model`] and is
//! compiled everywhere: the download manifest, what is on disk, and whether
//! this build supports Parakeet at all.

use crate::engine::{TranscriptionEngine, TranscriptionSession};
use crate::parakeet_model::{missing_parakeet_files, PARAKEET_MODEL_FILES};
use parakeet_rs::{ExecutionConfig, Parakeet, Transcriber};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use tracing::{error, info, warn};

/// Where ONNX Runtime's optimised graph is kept, inside the model directory
/// so `from_pretrained` finds `tokenizer.json` beside it in file mode.
///
/// Deliberately not one of the names `find_model_file` looks for
/// (`model.onnx`, `model_fp16.onnx`, `model_int8.onnx`, `model_q4.onnx`), so
/// a directory-mode load never picks it up by accident.
const OPTIMIZED_MODEL_FILE: &str = "model_optimized.onnx";

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

        let model = Self::load_model(model_dir)?;

        info!("[ParakeetEngine] Parakeet model loaded successfully");

        Ok(Self {
            model: Arc::new(Mutex::new(Some(model))),
            model_dir: model_dir.to_path_buf(),
        })
    }

    /// Load the model, reusing ONNX Runtime's optimised graph when one has
    /// been written.
    ///
    /// ORT re-runs its entire optimisation pipeline on every launch — the
    /// hundreds of `GraphTransformer ... modified` and `Removing initializer`
    /// lines in a startup log are that work, and it dominates the ~17s before
    /// dictation is usable. `with_optimized_model_path` makes ORT serialise
    /// the result while it is already doing the work, costing nothing on the
    /// run that writes it, so later launches can load the finished graph.
    ///
    /// Every failure here falls back to loading the model the original way. A
    /// cache that cannot be read is deleted rather than retried: it is a
    /// derived artefact and regenerating it is cheaper than reasoning about
    /// why it went bad.
    fn load_model(model_dir: &Path) -> Result<Parakeet, String> {
        let optimized = model_dir.join(OPTIMIZED_MODEL_FILE);

        if Self::cache_is_stale(&optimized, model_dir) {
            info!("[ParakeetEngine] Source model is newer than the optimised graph; rebuilding");
            let _ = std::fs::remove_file(&optimized);
        }

        if optimized.is_file() {
            match Parakeet::from_pretrained(&optimized, None) {
                Ok(model) => {
                    info!("[ParakeetEngine] Loaded the pre-optimised graph");
                    return Ok(model);
                }
                Err(e) => {
                    warn!(
                        "[ParakeetEngine] Optimised graph unusable ({}); rebuilding it",
                        e
                    );
                    let _ = std::fs::remove_file(&optimized);
                }
            }
        }

        // Ask ORT to write the optimised graph out as a side effect of the
        // optimising it does anyway. The closure's argument type is inferred
        // from the trait bound, which is what keeps `ort` out of this crate's
        // dependencies.
        let write_to = optimized.clone();
        let config = ExecutionConfig::new()
            // `with_optimized_model_path` returns `Error<SessionBuilder>`, which
            // hands the builder back on failure; `with_custom_configure` wants
            // the plain `Error<()>`. ort provides the conversion.
            .with_custom_configure(move |b| {
                b.with_optimized_model_path(&write_to).map_err(Into::into)
            });

        match Parakeet::from_pretrained(model_dir, Some(config)) {
            Ok(model) => {
                info!(
                    "[ParakeetEngine] Loaded and wrote an optimised graph to {:?}",
                    optimized
                );
                Ok(model)
            }
            Err(e) => {
                warn!(
                    "[ParakeetEngine] Could not write an optimised graph ({}); loading plain",
                    e
                );
                Parakeet::from_pretrained(model_dir, None).map_err(|e| {
                    format!("Failed to load Parakeet model from {:?}: {}", model_dir, e)
                })
            }
        }
    }

    /// True when the optimised graph predates the model it was built from.
    ///
    /// Re-downloading the model leaves a cache describing the old weights,
    /// which would load happily and transcribe with whatever was there
    /// before. Modification time is the cheap check; a missing timestamp on
    /// either side counts as stale, because guessing wrong in that direction
    /// only costs one slow launch.
    fn cache_is_stale(optimized: &Path, model_dir: &Path) -> bool {
        let Ok(cache_time) = std::fs::metadata(optimized).and_then(|m| m.modified()) else {
            return false; // no cache at all is not staleness
        };
        PARAKEET_MODEL_FILES.iter().any(|f| {
            std::fs::metadata(model_dir.join(f.name))
                .and_then(|m| m.modified())
                .map(|source_time| source_time > cache_time)
                .unwrap_or(true)
        })
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

#[cfg(test)]
mod tests {
    use super::*;

    /// Where a dogfooding install keeps the downloaded Parakeet model.
    fn installed_model_dir() -> Option<PathBuf> {
        if let Ok(dir) = std::env::var("JUNO_PARAKEET_DIR") {
            return Some(PathBuf::from(dir));
        }
        let dir = dirs_home()?
            .join("Library/Application Support")
            .join(crate::constants::HOST_BUNDLE_IDENTIFIER)
            .join("models/parakeet-ctc");
        crate::parakeet_model::model_files_present(&dir).then_some(dir)
    }

    fn dirs_home() -> Option<PathBuf> {
        std::env::var_os("HOME").map(PathBuf::from)
    }

    #[test]
    fn a_missing_cache_is_not_stale() {
        // Nothing to invalidate. Reporting staleness here would make the
        // first launch try to delete a file that was never written.
        let dir = std::env::temp_dir().join("juno-stale-none");
        let _ = std::fs::create_dir_all(&dir);
        assert!(!ParakeetEngine::cache_is_stale(
            &dir.join("model_optimized.onnx"),
            &dir
        ));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_cache_older_than_the_model_is_stale() {
        // The case that matters: the model was re-downloaded and the cache
        // now describes weights that are gone. Loading it would transcribe
        // with the old model and look entirely healthy.
        let dir = std::env::temp_dir().join("juno-stale-old");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("temp dir");

        let cache = dir.join("model_optimized.onnx");
        std::fs::write(&cache, b"old").expect("cache");
        std::thread::sleep(std::time::Duration::from_millis(1100));
        for f in PARAKEET_MODEL_FILES {
            std::fs::write(dir.join(f.name), b"new").expect("model file");
        }

        assert!(ParakeetEngine::cache_is_stale(&cache, &dir));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_cache_newer_than_the_model_is_fresh() {
        let dir = std::env::temp_dir().join("juno-stale-fresh");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("temp dir");

        for f in PARAKEET_MODEL_FILES {
            std::fs::write(dir.join(f.name), b"model").expect("model file");
        }
        std::thread::sleep(std::time::Duration::from_millis(1100));
        let cache = dir.join("model_optimized.onnx");
        std::fs::write(&cache, b"built after").expect("cache");

        assert!(!ParakeetEngine::cache_is_stale(&cache, &dir));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_vanished_model_file_counts_as_stale() {
        // Cannot prove the cache still matches, so rebuild. One slow launch
        // is the whole cost of being wrong in this direction.
        let dir = std::env::temp_dir().join("juno-stale-gone");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("temp dir");
        let cache = dir.join("model_optimized.onnx");
        std::fs::write(&cache, b"cache").expect("cache");

        assert!(ParakeetEngine::cache_is_stale(&cache, &dir));
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// The only guard against a silent transcription regression.
    ///
    /// Everything else in this suite checks structure — files present, paths
    /// correct. This one checks that the model still turns audio into the
    /// right words, which is what breaks when the execution provider or the
    /// model changes underneath it, and which nothing else here would notice.
    ///
    /// It has already earned that. An attempt to cut Parakeet's ~17s startup
    /// by moving to the CoreML execution provider passed fmt, clippy and
    /// every other test, and failed here:
    ///
    ///     ONNX Runtime error: ... running CoreMLExecutionProvider_..._3
    ///     node. Unable to compute the prediction using a neural network
    ///     model ... broken/unsupported model (error code: -1)
    ///
    /// CoreML accepted `model_int8.onnx` at load and only failed at
    /// inference, so a fallback around the constructor never fired. Without
    /// this test that ships as "Juno starts, you speak, nothing comes back".
    ///
    /// Runs automatically wherever the model is downloaded, which is any
    /// machine actually running Juno. CI has no model, so it skips rather
    /// than fails — a skip is honest, a green light on an untested path is
    /// not.
    ///
    /// The fixture is macOS `say` output at 16 kHz mono, committed so the
    /// test needs no setup:
    ///   say -o fox.aiff "The quick brown fox jumps over the lazy dog"
    ///   afconvert -f WAVE -d LEI16@16000 -c 1 fox.aiff fox.wav
    #[test]
    fn parakeet_still_turns_audio_into_the_right_words() {
        let Some(dir) = installed_model_dir() else {
            eprintln!("skipping: no Parakeet model installed (this is fine in CI)");
            return;
        };
        let wav = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/fox-16k-mono.wav");

        let engine = ParakeetEngine::new(&dir).expect("engine loads");
        assert!(engine.is_initialized());

        let mut reader = hound::WavReader::open(&wav).expect("fixture opens");
        assert_eq!(reader.spec().sample_rate, 16_000, "fixture must be 16 kHz");
        let samples: Vec<f32> = reader
            .samples::<i16>()
            .filter_map(|s| s.ok())
            .map(|s| s as f32 / i16::MAX as f32)
            .collect();

        let mut session = engine.create_session().expect("session");
        let text = session.transcribe_final(&samples).expect("transcribes");
        let lower = text.to_lowercase();
        println!("parakeet said: {text:?}");

        // Not an exact match: decoding varies slightly and always has. These
        // four content words are what a working model gets right and a broken
        // execution provider does not.
        for word in ["quick", "brown", "fox", "lazy"] {
            assert!(lower.contains(word), "expected {word:?} in {text:?}");
        }
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

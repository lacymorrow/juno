//! The Parakeet engine itself: loading the ONNX graphs and running audio
//! through them.
//!
//! Compiled on Apple Silicon only. `parakeet-rs` pulls in `ort-sys`, which
//! ships a prebuilt ONNX Runtime for `aarch64-apple-darwin` and nothing else,
//! so this module (and that dependency) are gated on `target_arch =
//! "aarch64"` and the crate is built universal without them. Everything that
//! does not touch the loader lives in [`crate::parakeet_model`] and is
//! compiled everywhere: the download manifests, what is on disk, and whether
//! this build supports Parakeet at all.
//!
//! One engine, two models. The directory decides which (see
//! [`ParakeetVariant::for_dir`]): CTC is one graph and types lowercase, TDT
//! is two graphs and types capitals and punctuation. Past loading they are
//! the same to the rest of the plugin: audio in, text out.

use crate::engine::{TranscriptionEngine, TranscriptionSession};
use crate::parakeet_model::ParakeetVariant;
use parakeet_rs::{ExecutionConfig, Parakeet, ParakeetTDT, Transcriber};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Instant, SystemTime};
use tracing::{error, info, warn};

/// Where ONNX Runtime's optimised CTC graph is kept, inside the model
/// directory so `from_pretrained` finds `tokenizer.json` beside it in file
/// mode.
///
/// Deliberately not one of the names `find_model_file` looks for
/// (`model.onnx`, `model_fp16.onnx`, `model_int8.onnx`, `model_q4.onnx`), so
/// a directory-mode load never picks it up by accident.
const CTC_OPTIMIZED_FILE: &str = "model_optimized.onnx";

/// Where ONNX Runtime's optimised TDT graphs are kept: a directory inside the
/// model directory, laid out exactly as the loader wants a model directory
/// (its first-choice file names, plus the vocabulary), so later launches
/// simply load from here instead of from the download.
const TDT_OPTIMIZED_DIR: &str = "optimized";
const TDT_OPTIMIZED_ENCODER: &str = "encoder-model.onnx";
const TDT_OPTIMIZED_DECODER: &str = "decoder_joint-model.onnx";
const TDT_VOCAB: &str = "vocab.txt";

/// Whichever Parakeet model is in memory. The two `parakeet-rs` types share
/// the `Transcriber` trait, but that trait has generic methods and cannot be
/// boxed, so the choice is an enum rather than a `dyn`.
enum Loaded {
    Ctc(Parakeet),
    Tdt(ParakeetTDT),
}

impl Loaded {
    fn transcribe(&mut self, audio: &[f32]) -> Result<String, parakeet_rs::Error> {
        let result = match self {
            Loaded::Ctc(model) => model.transcribe_samples(audio.to_vec(), 16000, 1, None)?,
            Loaded::Tdt(model) => model.transcribe_samples(audio.to_vec(), 16000, 1, None)?,
        };
        Ok(result.text)
    }
}

/// STT engine backed by NVIDIA Parakeet 0.6B (CTC or TDT) via ONNX Runtime.
///
/// Model directory must contain every file in the variant's manifest; the
/// app's `download_stt_model` command fetches them.
///
/// The inner model is guarded by a Mutex because `transcribe_samples` takes
/// `&mut self`. In practice only one recording is active at a time, so
/// contention is not a concern.
pub struct ParakeetEngine {
    model: Arc<Mutex<Option<Loaded>>>,
    pub model_dir: PathBuf,
    variant: ParakeetVariant,
}

impl ParakeetEngine {
    /// Load the Parakeet model `model_dir` is named for. Returns an error if
    /// the directory does not exist or model files are missing.
    pub fn new(model_dir: &Path) -> Result<Self, String> {
        let variant = ParakeetVariant::for_dir(model_dir);
        info!(
            "[ParakeetEngine] Loading Parakeet {:?} model from {:?}",
            variant, model_dir
        );

        let missing = variant.missing_files(model_dir);
        if !missing.is_empty() {
            return Err(format!(
                "Parakeet model files missing in {}: {}. Download Parakeet from Settings > Models.",
                model_dir.display(),
                missing.join(", ")
            ));
        }

        let started = Instant::now();
        let model = match variant {
            ParakeetVariant::Ctc => Loaded::Ctc(Self::load_ctc(model_dir)?),
            ParakeetVariant::Tdt => Loaded::Tdt(Self::load_tdt(model_dir)?),
        };

        info!(
            "[ParakeetEngine] Parakeet {:?} model loaded in {} ms",
            variant,
            started.elapsed().as_millis()
        );

        Ok(Self {
            model: Arc::new(Mutex::new(Some(model))),
            model_dir: model_dir.to_path_buf(),
            variant,
        })
    }

    pub fn variant(&self) -> ParakeetVariant {
        self.variant
    }

    /// Load the CTC model, reusing ONNX Runtime's optimised graph when one
    /// has been written.
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
    fn load_ctc(model_dir: &Path) -> Result<Parakeet, String> {
        let optimized = model_dir.join(CTC_OPTIMIZED_FILE);

        if cache_is_stale(&optimized, model_dir, ParakeetVariant::Ctc) {
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

    /// Load the TDT model, with the same optimised-graph cache as CTC, in a
    /// different shape.
    ///
    /// TDT is two ONNX sessions (encoder, then the joined decoder), and
    /// `parakeet-rs` applies one `ExecutionConfig` to both, so one output
    /// path would make the second session overwrite the first. The closure
    /// counts its calls instead: the loader builds the encoder first and the
    /// decoder second, so call 0 writes the encoder's graph and call 1 the
    /// decoder's, into `optimized/` under the loader's first-choice names.
    /// The vocabulary is linked in beside them, and a later launch loads
    /// `optimized/` as if it were the model directory.
    ///
    /// A cache that is incomplete, stale or unreadable is removed and
    /// rebuilt; nothing here can stop the model loading plain.
    fn load_tdt(model_dir: &Path) -> Result<ParakeetTDT, String> {
        let cache = model_dir.join(TDT_OPTIMIZED_DIR);
        let cache_files = [
            cache.join(TDT_OPTIMIZED_ENCODER),
            cache.join(TDT_OPTIMIZED_DECODER),
            cache.join(TDT_VOCAB),
        ];

        if cache_files
            .iter()
            .any(|f| cache_is_stale(f, model_dir, ParakeetVariant::Tdt))
        {
            info!("[ParakeetEngine] Source model is newer than the optimised graphs; rebuilding");
            let _ = std::fs::remove_dir_all(&cache);
        }

        if cache_files.iter().all(|f| f.is_file()) {
            match ParakeetTDT::from_pretrained(&cache, None) {
                Ok(model) => {
                    info!("[ParakeetEngine] Loaded the pre-optimised graphs");
                    return Ok(model);
                }
                Err(e) => {
                    warn!(
                        "[ParakeetEngine] Optimised graphs unusable ({}); rebuilding them",
                        e
                    );
                    let _ = std::fs::remove_dir_all(&cache);
                }
            }
        } else if cache.exists() {
            // Half a cache (a crash mid-write) is worth nothing; start over.
            let _ = std::fs::remove_dir_all(&cache);
        }

        if let Err(e) = std::fs::create_dir_all(&cache) {
            warn!(
                "[ParakeetEngine] Could not create {:?} ({}); loading plain",
                cache, e
            );
            return Self::load_tdt_plain(model_dir);
        }

        let write_dir = cache.clone();
        let calls = Arc::new(AtomicUsize::new(0));
        let config = ExecutionConfig::new().with_custom_configure(move |b| {
            let target = match calls.fetch_add(1, Ordering::SeqCst) {
                0 => TDT_OPTIMIZED_ENCODER,
                _ => TDT_OPTIMIZED_DECODER,
            };
            b.with_optimized_model_path(write_dir.join(target))
                .map_err(Into::into)
        });

        match ParakeetTDT::from_pretrained(model_dir, Some(config)) {
            Ok(model) => {
                let vocab_src = model_dir.join(TDT_VOCAB);
                let vocab_dst = cache.join(TDT_VOCAB);
                if std::fs::hard_link(&vocab_src, &vocab_dst).is_err() {
                    if let Err(e) = std::fs::copy(&vocab_src, &vocab_dst) {
                        // Without the vocabulary the cache is incomplete and
                        // the next launch rebuilds it; this launch is fine.
                        warn!(
                            "[ParakeetEngine] Could not place the vocabulary beside the optimised graphs: {}",
                            e
                        );
                    }
                }
                info!(
                    "[ParakeetEngine] Loaded and wrote optimised graphs to {:?}",
                    cache
                );
                Ok(model)
            }
            Err(e) => {
                warn!(
                    "[ParakeetEngine] Could not write optimised graphs ({}); loading plain",
                    e
                );
                let _ = std::fs::remove_dir_all(&cache);
                Self::load_tdt_plain(model_dir)
            }
        }
    }

    fn load_tdt_plain(model_dir: &Path) -> Result<ParakeetTDT, String> {
        ParakeetTDT::from_pretrained(model_dir, None).map_err(|e| {
            format!(
                "Failed to load Parakeet TDT model from {:?}: {}",
                model_dir, e
            )
        })
    }
}

/// True when an optimised graph predates the model it was built from.
///
/// Re-downloading the model leaves a cache describing the old weights,
/// which would load happily and transcribe with whatever was there
/// before. Modification time is the cheap check; a missing timestamp on
/// the model's side counts as stale, because guessing wrong in that
/// direction only costs one slow launch. A missing cache file is not
/// staleness: there is nothing to invalidate.
fn cache_is_stale(optimized: &Path, model_dir: &Path, variant: ParakeetVariant) -> bool {
    let Ok(cache_time) = std::fs::metadata(optimized).and_then(|m| m.modified()) else {
        return false;
    };
    variant.files().iter().any(|f| {
        std::fs::metadata(model_dir.join(f.name))
            .and_then(|m| m.modified())
            .map(|source_time: SystemTime| source_time > cache_time)
            .unwrap_or(true)
    })
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

    fn model_dir(&self) -> Option<String> {
        Some(self.model_dir.to_string_lossy().into_owned())
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
    model: Arc<Mutex<Option<Loaded>>>,
}

impl ParakeetSession {
    fn run(&mut self, audio: &[f32]) -> Result<String, String> {
        let mut guard = self
            .model
            .lock()
            .map_err(|e| format!("Parakeet model lock poisoned: {}", e))?;

        let model = guard
            .as_mut()
            .ok_or_else(|| "Parakeet model not loaded".to_string())?;

        model.transcribe(audio).map_err(|e| e.to_string())
    }
}

impl TranscriptionSession for ParakeetSession {
    fn transcribe_partial(&mut self, audio: &[f32]) -> Result<Option<String>, String> {
        if audio.is_empty() {
            return Ok(None);
        }
        match self.run(audio) {
            Ok(text) if text.trim().is_empty() => Ok(None),
            Ok(text) => Ok(Some(text)),
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
        self.run(audio)
            .map_err(|e| format!("Parakeet final transcription failed: {}", e))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Where a dogfooding install keeps a downloaded Parakeet model. TDT
    /// first: it is the recommended one, and the one whose output has
    /// casing to check.
    fn installed_model_dir() -> Option<PathBuf> {
        if let Ok(dir) = std::env::var("JUNO_PARAKEET_DIR") {
            return Some(PathBuf::from(dir));
        }
        let models = dirs_home()?
            .join("Library/Application Support")
            .join(crate::constants::HOST_BUNDLE_IDENTIFIER)
            .join("models");
        [ParakeetVariant::Tdt, ParakeetVariant::Ctc]
            .into_iter()
            .map(|v| models.join(v.dir_leaf()))
            .find(|dir| crate::parakeet_model::model_files_present(dir))
    }

    fn dirs_home() -> Option<PathBuf> {
        std::env::var_os("HOME").map(PathBuf::from)
    }

    fn write_model_files(dir: &Path, variant: ParakeetVariant, contents: &[u8]) {
        for f in variant.files() {
            std::fs::write(dir.join(f.name), contents).expect("model file");
        }
    }

    #[test]
    fn a_missing_cache_is_not_stale() {
        // Nothing to invalidate. Reporting staleness here would make the
        // first launch try to delete a file that was never written.
        let dir = std::env::temp_dir().join("juno-stale-none");
        let _ = std::fs::create_dir_all(&dir);
        assert!(!cache_is_stale(
            &dir.join(CTC_OPTIMIZED_FILE),
            &dir,
            ParakeetVariant::Ctc
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

        let cache = dir.join(CTC_OPTIMIZED_FILE);
        std::fs::write(&cache, b"old").expect("cache");
        std::thread::sleep(std::time::Duration::from_millis(1100));
        write_model_files(&dir, ParakeetVariant::Ctc, b"new");

        assert!(cache_is_stale(&cache, &dir, ParakeetVariant::Ctc));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_cache_newer_than_the_model_is_fresh() {
        let dir = std::env::temp_dir().join("juno-stale-fresh");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("temp dir");

        write_model_files(&dir, ParakeetVariant::Ctc, b"model");
        std::thread::sleep(std::time::Duration::from_millis(1100));
        let cache = dir.join(CTC_OPTIMIZED_FILE);
        std::fs::write(&cache, b"built after").expect("cache");

        assert!(!cache_is_stale(&cache, &dir, ParakeetVariant::Ctc));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_vanished_model_file_counts_as_stale() {
        // Cannot prove the cache still matches, so rebuild. One slow launch
        // is the whole cost of being wrong in this direction.
        let dir = std::env::temp_dir().join("juno-stale-gone");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("temp dir");
        let cache = dir.join(CTC_OPTIMIZED_FILE);
        std::fs::write(&cache, b"cache").expect("cache");

        assert!(cache_is_stale(&cache, &dir, ParakeetVariant::Ctc));
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// The TDT cache is judged against the TDT manifest, in its own
    /// directory. A re-downloaded encoder invalidates it the same way.
    #[test]
    fn a_tdt_cache_older_than_its_model_is_stale() {
        let dir = std::env::temp_dir()
            .join("juno-stale-tdt")
            .join("parakeet-tdt");
        let _ = std::fs::remove_dir_all(&dir);
        let cache_dir = dir.join(TDT_OPTIMIZED_DIR);
        std::fs::create_dir_all(&cache_dir).expect("temp dir");

        let cache = cache_dir.join(TDT_OPTIMIZED_ENCODER);
        std::fs::write(&cache, b"old").expect("cache");
        std::thread::sleep(std::time::Duration::from_millis(1100));
        write_model_files(&dir, ParakeetVariant::Tdt, b"new");

        assert!(cache_is_stale(&cache, &dir, ParakeetVariant::Tdt));
        let _ = std::fs::remove_dir_all(dir.parent().expect("parent"));
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
    /// Runs automatically wherever a model is downloaded, which is any
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
        // TDT is in the catalog for its casing and punctuation; a TDT that
        // types lowercase is the CTC experience with a bigger download.
        if engine.variant() == ParakeetVariant::Tdt {
            assert!(
                text.trim_start().starts_with('T'),
                "TDT should capitalise the sentence: {text:?}"
            );
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

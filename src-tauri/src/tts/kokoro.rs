//! Kokoro-82M, kept warm while it is the engine in force.
//!
//! Loading the model is the slow part of Kokoro: an 82MB parse and a move onto
//! the GPU, plus a download the very first time. Done lazily, on the first
//! thing Juno says, that is several seconds of silence between the answer
//! appearing and Juno saying it. So when Kokoro is the active engine the model
//! is loaded in the background, at startup and the moment somebody switches to
//! it, and released when they switch away.
//!
//! The lifecycle lives in [`ModelCell`], which is generic over the model so
//! the rules are testable without loading anything:
//!
//! - **One load at a time.** The slot is `Empty`, `Loading` or `Ready`, and
//!   only the caller that moves it from `Empty` to `Loading` loads. A second
//!   preload while one is in flight is a no-op; a speaking call that arrives
//!   during a load waits for that load instead of starting its own.
//! - **No lock is held while loading or synthesising.** The mutex guards a few
//!   words of state and is released before the slow work, and the model is an
//!   `Arc`, so a release in the middle of an utterance cannot pull the model
//!   out from under it: the utterance holds its own reference.
//! - **A switch that lands mid-load wins.** Switching away marks the model
//!   unwanted. A background load that finishes after that drops what it
//!   loaded instead of keeping 82MB nobody asked for, unless a speaking call
//!   is already waiting on it.
//! - **Nothing here awaits.** Every lock is a `std` mutex held for a few
//!   instructions on a blocking thread or in a plain function, never across an
//!   `.await`.

use base64::{engine::general_purpose::STANDARD as BASE64_STANDARD, Engine as _};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Condvar, Mutex as StdMutex, MutexGuard, OnceLock};
use tauri::AppHandle;
use tracing::{error, info, warn};

/// The loaded model. `TtsModel` is `Send + Sync`, so sharing one is sound.
type KokoroModel = Arc<dyn any_tts::TtsModel>;

enum Slot<M> {
    Empty,
    Loading,
    Ready(M),
}

struct CellState<M> {
    slot: Slot<M>,
    /// Kokoro is the active engine, so the model should stay in memory.
    wanted: bool,
    /// Speaking calls parked until the load in flight finishes.
    waiting: usize,
}

/// A model slot that loads once, can be warmed ahead of need and released
/// when it is not.
pub struct ModelCell<M> {
    state: StdMutex<CellState<M>>,
    loaded: Condvar,
}

impl<M: Clone> Default for ModelCell<M> {
    fn default() -> Self {
        Self::new()
    }
}

impl<M: Clone> ModelCell<M> {
    pub const fn new() -> Self {
        Self {
            state: StdMutex::new(CellState {
                slot: Slot::Empty,
                wanted: false,
                waiting: 0,
            }),
            loaded: Condvar::new(),
        }
    }

    /// A poisoned lock here only means a load panicked. The state it guards
    /// is still a valid slot, so carry on rather than wedge Kokoro for the
    /// life of the process.
    fn lock(&self) -> MutexGuard<'_, CellState<M>> {
        self.state.lock().unwrap_or_else(|p| p.into_inner())
    }

    /// Say the model is wanted. Returns true when the caller must load it,
    /// which is exactly one caller per empty slot; a second preload while a
    /// load is in flight, or once the model is ready, returns false.
    pub fn begin_preload(&self) -> bool {
        let mut state = self.lock();
        state.wanted = true;
        if matches!(state.slot, Slot::Empty) {
            state.slot = Slot::Loading;
            true
        } else {
            false
        }
    }

    /// Store what a preload produced. Kept only if the model is still wanted
    /// or somebody is waiting to speak with it.
    pub fn finish_preload(&self, result: Result<M, String>) -> Result<M, String> {
        self.finish(result, false)
    }

    fn finish(&self, result: Result<M, String>, on_demand: bool) -> Result<M, String> {
        let mut state = self.lock();
        let outcome = match result {
            Ok(model) => {
                state.slot = if on_demand || state.wanted || state.waiting > 0 {
                    Slot::Ready(model.clone())
                } else {
                    Slot::Empty
                };
                Ok(model)
            }
            Err(e) => {
                state.slot = Slot::Empty;
                Err(e)
            }
        };
        self.loaded.notify_all();
        outcome
    }

    /// The model, loading it on this thread if nobody else is and waiting if
    /// somebody is. Blocking: call it from a blocking thread.
    pub fn get_or_load(&self, load: impl FnOnce() -> Result<M, String>) -> Result<M, String> {
        let mut state = self.lock();
        loop {
            if let Slot::Ready(model) = &state.slot {
                return Ok(model.clone());
            }
            if matches!(state.slot, Slot::Loading) {
                state.waiting += 1;
                state = self.loaded.wait(state).unwrap_or_else(|p| p.into_inner());
                state.waiting -= 1;
                continue;
            }
            state.slot = Slot::Loading;
            drop(state);
            // Kept whatever `wanted` says: something is about to speak with
            // it, and the fallback chain may ask again.
            return self.finish(load(), true);
        }
    }

    /// Kokoro is no longer the engine. Free the model; an utterance already
    /// speaking keeps its own reference until it finishes, and a load in
    /// flight drops its result when it lands.
    pub fn release(&self) {
        let mut state = self.lock();
        state.wanted = false;
        if matches!(state.slot, Slot::Ready(_)) {
            state.slot = Slot::Empty;
        }
    }

    pub fn is_ready(&self) -> bool {
        matches!(self.lock().slot, Slot::Ready(_))
    }

    #[cfg(test)]
    fn waiting(&self) -> usize {
        self.lock().waiting
    }
}

static KOKORO: ModelCell<KokoroModel> = ModelCell::new();

// -- the first download ------------------------------------------------------
//
// On a new Mac Kokoro has to download before it can say a word. When the
// Mac's voice is a Siri voice Juno does not go quiet for that: she speaks in
// it, says why once, and when her own voice lands she says so in it. When it
// is not, she stays quiet until Kokoro can speak, and says neither line.

/// Said once, in the Mac's voice, before the first thing she says while
/// Kokoro is still downloading.
pub const STAND_IN_LINE: &str = "One minute while I download my voice.";
/// Said in Kokoro once the download lands, if the stand-in was heard.
pub const BACK_LINE: &str = "Ah, that's better.";

static STAND_IN_HEARD: AtomicBool = AtomicBool::new(false);
static APP: OnceLock<AppHandle> = OnceLock::new();

/// Kept so a load that finishes on its own can speak.
pub fn remember_app(app: &AppHandle) {
    let _ = APP.set(app.clone());
}

/// Kokoro cannot speak without first downloading its weights. A model on disk
/// loads in seconds and is worth waiting for; a download is not.
pub fn downloading() -> bool {
    !KOKORO.is_ready() && !crate::tts::voices::kokoro_model_on_disk()
}

/// What the Mac's voice says in Kokoro's place: the stand-in line first, the
/// first time, then just the text. Also makes sure a download is under way,
/// so one that failed (offline at launch) is tried again.
pub fn stand_in(text: &str) -> String {
    keep_downloading();
    stand_in_text(&STAND_IN_HEARD, text)
}

/// Make sure a download is under way, so one that failed (offline at launch)
/// is tried again. Used on its own when nothing may stand in.
pub fn keep_downloading() {
    sync_with_engine("kokoro", || {});
}

fn stand_in_text(heard: &AtomicBool, text: &str) -> String {
    if heard.swap(true, Ordering::SeqCst) {
        text.to_string()
    } else {
        format!("{STAND_IN_LINE} {text}")
    }
}

/// The model is ready. If the person heard the stand-in, say so in Kokoro.
/// Queued like any other speech, so it waits its turn and stays quiet when
/// Juno is silent.
fn announce_return() {
    if !KOKORO.is_ready() || !STAND_IN_HEARD.swap(false, Ordering::SeqCst) {
        return;
    }
    if let Some(app) = APP.get() {
        info!("[Kokoro] Downloaded; back in Kokoro's voice");
        crate::tts::enqueue_speech_always(BACK_LINE.to_string(), app.clone());
    }
}

/// Why the last load failed, until one succeeds. The voice list says it, so a
/// Kokoro that cannot get ready reads as a reason rather than as a pane stuck
/// on "getting ready".
static LOAD_PROBLEM: StdMutex<Option<String>> = StdMutex::new(None);

fn set_load_problem(problem: Option<String>) {
    if let Ok(mut slot) = LOAD_PROBLEM.lock() {
        *slot = problem;
    }
}

/// Why Kokoro could not load, if the last attempt failed.
pub fn load_problem() -> Option<String> {
    LOAD_PROBLEM.lock().ok().and_then(|slot| slot.clone())
}

/// How long synthesis took for each second of speech it produced.
///
/// Below 1.0 the model writes audio faster than the audio plays, so a longer
/// sentence costs the listener nothing extra up front. At or above 1.0 the
/// wait before the first word is roughly the spoken length of that first
/// sentence, which is the shape of the Kokoro latency on LAC-5009.
///
/// `None` when there is no audio to divide by. An empty or failed synthesis is
/// not an infinitely fast one, and a NaN or infinite duration must not reach
/// the log as a factor either.
fn real_time_factor(synthesis_ms: u128, audio_secs: f32) -> Option<f32> {
    if !audio_secs.is_finite() || audio_secs <= 0.0 {
        return None;
    }
    Some(synthesis_ms as f32 / 1000.0 / audio_secs)
}

fn load_kokoro() -> Result<KokoroModel, String> {
    info!("[Kokoro] Loading Kokoro-82M (first run downloads ~82MB from HuggingFace Hub)");
    let started = std::time::Instant::now();
    // Metal where the Mac has it, CPU otherwise. Both architectures.
    let config = any_tts::TtsConfig::new(any_tts::ModelType::Kokoro).with_preferred_runtime();
    // Which device the preference actually resolved to. any-tts logs its own
    // "Metal device 0 initialized" without a prefix, so from Juno's log alone
    // there was no way to attribute it to Kokoro rather than to Parakeet,
    // which loads through candle too.
    let runtime = format!("{:?}/{:?}", config.device, config.dtype);
    match any_tts::load_model(config) {
        Ok(model) => {
            info!(
                "[Kokoro] Model loaded in {}ms on {}",
                started.elapsed().as_millis(),
                runtime
            );
            set_load_problem(None);
            Ok(Arc::from(model))
        }
        Err(e) => {
            error!("[Kokoro] Model load failed: {}", e);
            set_load_problem(Some(e.to_string()));
            Err(format!("Failed to load Kokoro-82M model: {}", e))
        }
    }
}

/// Match the model to the engine in force: warm it when Kokoro is speaking,
/// free it when it is not.
///
/// Called after a switch has been written, never before, so it acts on the
/// engine that is actually in force. Returns immediately; the load runs on a
/// blocking thread. `on_settled` runs once a load this call started has
/// finished either way, which is how the voice list learns there are voices
/// on disk now, or why there are not.
pub fn sync_with_engine(engine: &str, on_settled: impl FnOnce() + Send + 'static) {
    if !engine.eq_ignore_ascii_case("kokoro") {
        if KOKORO.is_ready() {
            info!("[Kokoro] Releasing the model: {} is speaking now", engine);
        }
        KOKORO.release();
        return;
    }

    if !KOKORO.begin_preload() {
        return;
    }

    tauri::async_runtime::spawn_blocking(move || {
        let result = load_kokoro();
        // A first synthesis compiles the GPU kernels. Do it here, with nobody
        // listening, rather than on the first real sentence.
        if let Ok(model) = &result {
            let warmup = any_tts::SynthesisRequest::new("Ready.")
                .with_voice(crate::tts::voices::KOKORO_DEFAULT_VOICE);
            if let Err(e) = model.synthesize(&warmup) {
                warn!("[Kokoro] Warm-up synthesis failed: {}", e);
            }
        }
        match KOKORO.finish_preload(result) {
            Ok(_) => announce_return(),
            Err(e) => warn!("[Kokoro] Preload failed: {}", e),
        }
        on_settled();
    });
}

/// Invoke Kokoro-82M TTS synthesis.
///
/// `voice` has already been resolved against the embeddings on disk by the
/// caller. `speed` is a factor, 1.0 being normal (see `tts::rate`). Returns base64-encoded WAV audio on success. afplay on macOS reads
/// format from magic bytes, not extension, so WAV bytes work fine in the .m4a
/// temp file that play_base64_audio_with_tracking creates.
pub async fn invoke_kokoro_tts(text: String, voice: String, speed: f64) -> Result<String, String> {
    info!(
        "[Kokoro] TTS requested: {} chars, voice: {}, speed: {:.2}",
        text.chars().count(),
        voice,
        speed
    );

    if crate::tts::is_tts_stop_requested() {
        info!("[Kokoro] Stop requested before start, aborting");
        return Ok("TTS_STOPPED_BY_USER".to_string());
    }

    // Candle inference is synchronous, so it runs off the async executor.
    let base64_audio = tokio::task::spawn_blocking(move || -> Result<String, String> {
        // Ready already when it was preloaded; otherwise this loads it, or
        // waits for the preload in flight rather than starting a second one.
        let model = KOKORO.get_or_load(load_kokoro)?;

        if crate::tts::is_tts_stop_requested() {
            return Ok("TTS_STOPPED_BY_USER".to_string());
        }

        // any-tts reads Kokoro's speed factor from `temperature` (1.0 normal).
        let request = any_tts::SynthesisRequest::new(text.as_str())
            .with_voice(voice.as_str())
            .with_temperature(speed);

        info!("[Kokoro] Synthesizing with voice '{}'", voice);
        let synthesis_started = std::time::Instant::now();
        let audio = model
            .synthesize(&request)
            .map_err(|e| format!("[Kokoro] Synthesis failed: {}", e))?;
        let synthesis_ms = synthesis_started.elapsed().as_millis();
        // The sample rate comes from the model rather than from an assumption,
        // because the real-time factor is the whole point of the line and a
        // guessed rate would move it.
        let audio_secs = audio.duration_secs();
        match real_time_factor(synthesis_ms, audio_secs) {
            Some(rtf) => info!(
                "[Kokoro] Synthesized {:.2}s of speech at {}Hz in {}ms (RTF {:.2})",
                audio_secs, audio.sample_rate, synthesis_ms, rtf
            ),
            None => warn!("[Kokoro] Synthesis returned no audio in {}ms", synthesis_ms),
        }

        if crate::tts::is_tts_stop_requested() {
            return Ok("TTS_STOPPED_BY_USER".to_string());
        }

        let wav_bytes = audio.get_wav();
        info!("[Kokoro] Generated {} WAV bytes", wav_bytes.len());

        Ok(BASE64_STANDARD.encode(&wav_bytes))
    })
    .await
    .map_err(|e| format!("[Kokoro] Blocking task panicked: {}", e))??;

    Ok(base64_audio)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The stand-in line is said once, before the first sentence, and not
    /// again until her own voice is back.
    #[test]
    fn the_stand_in_line_is_said_once() {
        let heard = AtomicBool::new(false);
        assert_eq!(
            stand_in_text(&heard, "It is sunny."),
            "One minute while I download my voice. It is sunny."
        );
        assert_eq!(stand_in_text(&heard, "And warm."), "And warm.");
        heard.store(false, Ordering::SeqCst);
        assert!(stand_in_text(&heard, "Hi.").starts_with(STAND_IN_LINE));
        assert!(!STAND_IN_LINE.contains('—') && !BACK_LINE.contains('—'));
    }
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::mpsc;
    use std::time::Duration;

    /// The number the Kokoro latency argument rests on, including the cases
    /// where there is no number to print.
    #[test]
    fn the_real_time_factor_divides_by_the_speech_it_made() {
        // 2855ms for 2.72s of speech: the measured Kokoro shape, just over 1,
        // which is why the wait is the length of the sentence.
        let rtf = real_time_factor(2855, 2.72).expect("real audio has a factor");
        assert!((rtf - 1.0496).abs() < 0.001, "got {rtf}");
        // Well under 1.0 is the system voice's shape: 58ms for a whole second.
        let fast = real_time_factor(58, 1.0).expect("fast audio has a factor");
        assert!((fast - 0.058).abs() < 0.0001, "got {fast}");
        // No audio is not infinitely fast, and no non-finite duration may
        // reach the log as a factor.
        assert_eq!(real_time_factor(2855, 0.0), None);
        assert_eq!(real_time_factor(2855, -1.0), None);
        assert_eq!(real_time_factor(2855, f32::NAN), None);
        assert_eq!(real_time_factor(2855, f32::INFINITY), None);
        assert_eq!(real_time_factor(0, 0.0), None);
    }

    #[test]
    fn preloading_twice_loads_once() {
        let cell: ModelCell<u32> = ModelCell::new();
        assert!(cell.begin_preload(), "the first preload owns the load");
        assert!(
            !cell.begin_preload(),
            "a second preload in flight is a no-op"
        );
        assert_eq!(cell.finish_preload(Ok(7)), Ok(7));
        assert!(cell.is_ready());
        assert!(!cell.begin_preload(), "a ready model is not loaded again");
    }

    #[test]
    fn speaking_after_a_preload_does_not_load_again() {
        let cell: ModelCell<u32> = ModelCell::new();
        assert!(cell.begin_preload());
        cell.finish_preload(Ok(1)).unwrap();
        let model = cell
            .get_or_load(|| panic!("the preloaded model must be reused"))
            .unwrap();
        assert_eq!(model, 1);
    }

    #[test]
    fn switching_away_mid_load_drops_the_result() {
        let cell: ModelCell<u32> = ModelCell::new();
        assert!(cell.begin_preload());
        cell.release();
        cell.finish_preload(Ok(1)).unwrap();
        assert!(!cell.is_ready(), "nobody wants it any more");
    }

    #[test]
    fn switching_away_and_back_mid_load_keeps_the_result() {
        let cell: ModelCell<u32> = ModelCell::new();
        assert!(cell.begin_preload());
        cell.release();
        assert!(!cell.begin_preload(), "the load in flight still counts");
        cell.finish_preload(Ok(1)).unwrap();
        assert!(cell.is_ready());
    }

    #[test]
    fn releasing_a_ready_model_frees_it_and_a_failed_load_can_be_retried() {
        let cell: ModelCell<u32> = ModelCell::new();
        assert!(cell.begin_preload());
        cell.finish_preload(Ok(1)).unwrap();
        cell.release();
        assert!(!cell.is_ready());

        assert!(cell.begin_preload());
        assert!(cell.finish_preload(Err("offline".into())).is_err());
        assert!(cell.begin_preload(), "a failed load leaves the slot empty");
    }

    #[test]
    fn a_speaking_call_during_a_preload_waits_for_it_instead_of_loading() {
        let cell: Arc<ModelCell<u32>> = Arc::new(ModelCell::new());
        assert!(cell.begin_preload());

        let loads = Arc::new(AtomicUsize::new(0));
        let (tx, rx) = mpsc::channel();
        let speaker = {
            let cell = Arc::clone(&cell);
            let loads = Arc::clone(&loads);
            std::thread::spawn(move || {
                let model = cell.get_or_load(|| {
                    loads.fetch_add(1, Ordering::SeqCst);
                    Ok(99)
                });
                tx.send(model).unwrap();
            })
        };

        // Still loading, so the speaker parks rather than loading its own.
        let parked_by = std::time::Instant::now() + Duration::from_secs(5);
        while cell.waiting() == 0 {
            assert!(
                std::time::Instant::now() < parked_by,
                "the speaker never parked"
            );
            std::thread::sleep(Duration::from_millis(5));
        }
        assert!(rx.try_recv().is_err());
        // Even a switch away mid-load keeps the model for the one waiting.
        cell.release();
        cell.finish_preload(Ok(5)).unwrap();

        assert_eq!(rx.recv_timeout(Duration::from_secs(5)).unwrap(), Ok(5));
        speaker.join().unwrap();
        assert_eq!(loads.load(Ordering::SeqCst), 0, "no second load");
    }

    #[test]
    fn concurrent_speaking_calls_load_once() {
        let cell: Arc<ModelCell<u32>> = Arc::new(ModelCell::new());
        let loads = Arc::new(AtomicUsize::new(0));
        let threads: Vec<_> = (0..8)
            .map(|_| {
                let cell = Arc::clone(&cell);
                let loads = Arc::clone(&loads);
                std::thread::spawn(move || {
                    cell.get_or_load(|| {
                        loads.fetch_add(1, Ordering::SeqCst);
                        std::thread::sleep(Duration::from_millis(20));
                        Ok(3)
                    })
                })
            })
            .collect();
        for thread in threads {
            assert_eq!(thread.join().unwrap(), Ok(3));
        }
        assert_eq!(loads.load(Ordering::SeqCst), 1);
    }
}

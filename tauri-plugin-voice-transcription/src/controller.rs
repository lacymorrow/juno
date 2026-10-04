use crate::constants;
use crate::engine::{TranscriptionEngine, TranscriptionSession};
use cpal::traits::DeviceTrait;
use cpal::SampleFormat;
use hound;
use rubato::{
    Resampler, SincFixedIn, SincInterpolationParameters, SincInterpolationType, WindowFunction,
};
use std::path::Path;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::mpsc::{channel, Sender, TryRecvError};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};
use tauri::{AppHandle, Emitter, Runtime};
use tracing::info;

use crate::always_listening::CaptureGuard;
use crate::capture_failure::{self, CaptureStartFailure};
use crate::devices;
use crate::error::{Error, Result};
use crate::utils::downmix_i16_to_mono;

const WHISPER_SAMPLE_RATE: u32 = 16000;
const SINC_LENGTH: usize = 256;
const OVERSAMPLING_FACTOR: usize = 256;

/// Calculate RMS volume of an audio chunk (0.0 = silence, higher = louder).
fn calculate_rms_volume(samples: &[f32]) -> f32 {
    if samples.is_empty() {
        return 0.0;
    }
    let sum_sq: f32 = samples.iter().map(|&s| s * s).sum();
    (sum_sq / samples.len() as f32).sqrt()
}

/// Create standard sinc interpolation parameters for audio resampling.
fn sinc_resampling_params() -> SincInterpolationParameters {
    SincInterpolationParameters {
        sinc_len: SINC_LENGTH,
        f_cutoff: 0.95,
        interpolation: SincInterpolationType::Linear,
        oversampling_factor: OVERSAMPLING_FACTOR,
        window: WindowFunction::BlackmanHarris2,
    }
}

/// Bring a window of captured audio down (or up) to the 16 kHz the engines
/// want, for the **partial** decodes only.
///
/// Partials cannot share the session's `SincFixedIn`, and sharing it is why
/// "show words as I speak" did nothing for most people. That resampler is
/// built for a fixed 1024-frame chunk and, given a longer buffer, rubato
/// silently uses only the first `chunk_size` frames of it
/// (`asynchro_sinc.rs`: `copy_from_slice(&wave_in[chan][..self.chunk_size])`).
/// So a ten-second live window reached the engine as the first 21 ms of
/// itself, decoded to nothing, and no partial was ever emitted — on every
/// microphone that does not already run at 16 kHz. The final decode builds
/// its own correctly sized resampler, which is why the finished sentence was
/// always right and only the live words were missing. It looked like the
/// model's fault; it was the microphone's sample rate.
///
/// A fixed-chunk resampler is also stateful, and the live window re-sends
/// overlapping audio on every cadence, which a stateful resampler cannot be
/// fed correctly. Hence a pure, per-call conversion: stateless by
/// construction, sized to whatever it is handed, and cheap enough for a
/// 600 ms cadence. Partials are display-only provisional text, so averaging
/// the input frames that fall inside each output frame (a crude anti-alias)
/// is accurate enough; the final decode keeps the sinc resampler.
fn resample_for_partial(audio: &[f32], from_rate: u32) -> Vec<f32> {
    if audio.is_empty() || from_rate == 0 || from_rate == WHISPER_SAMPLE_RATE {
        return audio.to_vec();
    }

    let out_len = (audio.len() as u64 * WHISPER_SAMPLE_RATE as u64 / from_rate as u64) as usize;
    if out_len == 0 {
        return Vec::new();
    }

    let step = f64::from(from_rate) / f64::from(WHISPER_SAMPLE_RATE);
    let last = audio.len() - 1;
    let mut out = Vec::with_capacity(out_len);
    for i in 0..out_len {
        let start = ((i as f64 * step).floor() as usize).min(last);
        let end = ((((i + 1) as f64) * step).ceil() as usize).min(audio.len());
        if end <= start {
            out.push(audio[start]);
            continue;
        }
        let span = &audio[start..end];
        out.push(span.iter().sum::<f32>() / span.len() as f32);
    }
    out
}

/// How often the live-partial window is re-decoded, at most.
const LIVE_PARTIAL_CADENCE: Duration = Duration::from_millis(600);
/// How much audio the live-partial window keeps; caps the per-decode cost.
const LIVE_PARTIAL_WINDOW_SECS: u64 = 10;

/// The bounded sliding window behind live streaming partials. Audio is
/// appended as it arrives and the oldest samples fall off past `capacity`;
/// `take_due` hands back the whole window when it is time to decode again.
/// Pure state with no I/O, so the window math is testable without a mic.
struct LivePartialWindow {
    buffer: Vec<f32>,
    capacity: usize,
    cadence: Duration,
    last_emit: Option<Instant>,
}

impl LivePartialWindow {
    fn new(capacity: usize, cadence: Duration) -> Self {
        Self {
            buffer: Vec::with_capacity(capacity),
            capacity,
            cadence,
            last_emit: None,
        }
    }

    fn push(&mut self, chunk: &[f32]) {
        self.buffer.extend_from_slice(chunk);
        if self.buffer.len() > self.capacity {
            let overflow = self.buffer.len() - self.capacity;
            self.buffer.drain(0..overflow);
        }
    }

    fn samples(&self) -> &[f32] {
        &self.buffer
    }

    /// The cumulative window to decode now, or None when nothing is due:
    /// no audio yet, or the cadence has not elapsed since the last decode.
    /// The first chunk is due immediately; the decode itself is what waits
    /// for enough speech to say something.
    fn take_due(&mut self, now: Instant) -> Option<&[f32]> {
        if self.buffer.is_empty() {
            return None;
        }
        if let Some(last) = self.last_emit {
            if now.duration_since(last) < self.cadence {
                return None;
            }
        }
        self.last_emit = Some(now);
        Some(&self.buffer)
    }
}

enum AudioThreadMessage {
    /// Finish: transcribe what was captured and emit the result.
    Stop,
    /// Throw it away: stop recording without transcribing and without
    /// emitting a result. "Stop" always finalised and submitted, so there was
    /// no way to change your mind mid-sentence.
    Discard,
}

/// Opens the dictation microphone on the audio thread and hands back whatever
/// keeps it open. Dropping that value is what stops capture. A cpal stream is
/// not `Send`, so it is created (and dropped) on the thread that owns it; a
/// test hands in a token instead of a real stream.
type CaptureOpen = Box<
    dyn FnOnce(Sender<Vec<f32>>) -> std::result::Result<Box<dyn std::any::Any>, CaptureStartFailure>
        + Send,
>;

pub struct VoiceController {
    engine: Option<Arc<dyn TranscriptionEngine>>,
    pub model_path: String,
    is_dictating: bool,
    audio_thread: Option<(thread::JoinHandle<()>, Sender<AudioThreadMessage>)>,
    last_processed_audio_buffer: Arc<Mutex<Option<Vec<f32>>>>,
    actual_recording_sample_rate: Arc<Mutex<Option<u32>>>,
    is_initialized: bool,
    initialization_error: Option<String>,
    /// "Show words as I speak". When true the audio thread keeps a sliding
    /// window and emits cumulative provisional text on a fast cadence. When
    /// false no partial is decoded or emitted at all. Display-only; never typed.
    /// Shared with the audio thread, which reads it on every pass, so a toggle
    /// takes effect inside a running session.
    live_partial: PartialGate,
    /// How many dictation microphone streams are open right now. Counted by
    /// the [`CaptureGuard`] that owns the stream on the audio thread, so "is
    /// the mic still open after this turn ended" is a number a test can read.
    live_captures: Arc<AtomicUsize>,
}

/// The one switch that decides whether a partial transcript may exist.
///
/// Cloning shares the flag. Every partial emission goes through
/// [`partial_event`], which asks this gate, so the setting cannot be bypassed
/// by a new code path that forgets to check it.
#[derive(Clone, Debug, Default)]
pub struct PartialGate(Arc<AtomicBool>);

impl PartialGate {
    pub fn set(&self, enabled: bool) {
        self.0.store(enabled, Ordering::SeqCst);
    }

    pub fn is_open(&self) -> bool {
        self.0.load(Ordering::SeqCst)
    }
}

/// The payload of a partial-result event, or `None` when the setting is off.
/// The only place a partial event is built.
fn partial_event(gate: &PartialGate, text: &str, provisional: bool) -> Option<serde_json::Value> {
    if !gate.is_open() {
        return None;
    }
    Some(serde_json::json!({ "text": text, "provisional": provisional }))
}

impl VoiceController {
    /// Create a VoiceController backed by an already-initialized STT engine.
    /// Preferred constructor — engine is provided by `EngineManager`.
    pub fn new_with_engine(
        model_path_str: &str,
        engine: Arc<dyn TranscriptionEngine>,
    ) -> Result<Self> {
        info!(
            "[VoiceController] Creating controller with '{}' engine",
            engine.name()
        );
        Ok(Self {
            engine: Some(engine),
            model_path: model_path_str.to_string(),
            is_dictating: false,
            audio_thread: None,
            last_processed_audio_buffer: Arc::new(Mutex::new(None)),
            actual_recording_sample_rate: Arc::new(Mutex::new(None)),
            is_initialized: true,
            initialization_error: None,
            live_partial: PartialGate::default(),
            live_captures: Arc::new(AtomicUsize::new(0)),
        })
    }

    /// Backward-compat constructor for callers that still pass a `WhisperContext` directly
    /// (e.g. the `set_model_path` command). Wraps the context in a `WhisperEngine`.
    pub fn new_with_shared_context(
        model_path_str: &str,
        shared_context: Arc<whisper_rs::WhisperContext>,
    ) -> Result<Self> {
        use crate::engine_whisper::WhisperEngine;
        info!("[VoiceController] Creating controller with shared WhisperContext (legacy path)");
        let engine: Arc<dyn TranscriptionEngine> = Arc::new(WhisperEngine::new(shared_context));
        Self::new_with_engine(model_path_str, engine)
    }

    /// Create an uninitialized controller that can be managed by Tauri but will return errors for operations
    pub fn new_uninitialized(model_path_str: &str, error_message: String) -> Self {
        Self {
            engine: None,
            model_path: model_path_str.to_string(),
            is_dictating: false,
            audio_thread: None,
            last_processed_audio_buffer: Arc::new(Mutex::new(None)),
            actual_recording_sample_rate: Arc::new(Mutex::new(None)),
            is_initialized: false,
            initialization_error: Some(error_message),
            live_partial: PartialGate::default(),
            live_captures: Arc::new(AtomicUsize::new(0)),
        }
    }

    /// Check if the controller was successfully initialized
    pub fn is_initialized(&self) -> bool {
        self.is_initialized
    }

    /// Toggle live streaming partial transcription. Takes effect on the next
    /// dictation session. Display-only; partials are never injected as keystrokes.
    pub fn set_live_partial(&mut self, enabled: bool) {
        info!(
            "[VoiceController] Live partial transcription set to {}",
            enabled
        );
        self.live_partial.set(enabled);
    }

    /// Whether live streaming partial transcription is on for the next session.
    pub fn live_partial(&self) -> bool {
        self.live_partial.is_open()
    }

    /// Replace this controller with `replacement`, keeping the user-facing
    /// settings that the constructors do not know about.
    ///
    /// The plugin builds a fresh controller from a background task once the
    /// STT engine has loaded, and the app applies the persisted live-partial
    /// flag on its own schedule. Whichever runs second used to win: a plain
    /// `*slot = new` silently reset `live_partial` to false, so a flag applied
    /// before the engine finished loading never reached a recording.
    pub fn adopt(&mut self, replacement: VoiceController) {
        if self.live_partial.is_open() {
            info!("[VoiceController] Carrying live_partial=true across controller replacement");
        }
        replacement.live_partial.set(self.live_partial.is_open());
        *self = replacement;
    }

    /// Get the initialization error if any
    pub fn get_initialization_error(&self) -> Option<&String> {
        self.initialization_error.as_ref()
    }

    /// Update the shared Whisper context and model path for this controller
    /// This allows updating the model without recreating the entire controller
    pub fn update_shared_context(
        &mut self,
        model_path: &str,
        shared_context: Arc<whisper_rs::WhisperContext>,
    ) -> Result<()> {
        info!(
            "[VoiceController] Updating shared Whisper context with new model: {}",
            model_path
        );

        // Stop dictation if it's currently active
        let was_dictating = self.is_dictating;
        if was_dictating {
            info!("[VoiceController] Stopping dictation before model update");
            self.stop_dictation()?;
        }

        // Update the model path and shared context (wrap legacy WhisperContext in WhisperEngine)
        use crate::engine_whisper::WhisperEngine;
        let engine: Arc<dyn TranscriptionEngine> = Arc::new(WhisperEngine::new(shared_context));
        self.model_path = model_path.to_string();
        self.engine = Some(engine);
        self.is_initialized = true;
        self.initialization_error = None;

        info!("[VoiceController] Successfully updated shared Whisper context (no model reload needed)");

        // If it was dictating before, we'll let the caller decide whether to restart
        if was_dictating {
            info!("[VoiceController] Controller was dictating before update - caller should restart if needed");
        }

        Ok(())
    }

    /// Swap the active STT engine without restarting the controller.
    pub fn update_engine(&mut self, engine: Arc<dyn TranscriptionEngine>) -> Result<()> {
        info!("[VoiceController] Updating engine to '{}'", engine.name());
        let was_dictating = self.is_dictating;
        if was_dictating {
            self.stop_dictation()?;
        }
        self.engine = Some(engine);
        self.is_initialized = true;
        self.initialization_error = None;
        if was_dictating {
            info!("[VoiceController] Stopped before engine swap - caller should restart if needed");
        }
        Ok(())
    }

    /// Helper method to check initialization before performing operations
    fn ensure_initialized(&self) -> Result<&Arc<dyn TranscriptionEngine>> {
        if !self.is_initialized {
            let error_msg = self
                .initialization_error
                .as_ref()
                .map(|e| e.to_string())
                .unwrap_or_else(|| "Voice controller not initialized".to_string());
            return Err(Error::InitializationError(error_msg));
        }
        self.engine.as_ref().ok_or(Error::NotInitialized)
    }

    pub fn transcribe_audio_file(
        &self,
        audio_path_str: &str,
    ) -> std::result::Result<String, String> {
        let engine = self.ensure_initialized().map_err(|e| e.to_string())?;

        let audio_path = Path::new(audio_path_str);
        if !audio_path.exists() {
            return Err(format!("Audio file not found: {}", audio_path_str));
        }

        let mut session = engine.create_session()?;

        let mut reader = hound::WavReader::open(audio_path_str)
            .map_err(|e| format!("Failed to open audio file: {}", e))?;

        let hound::WavSpec {
            channels,
            sample_rate,
            bits_per_sample: _,
            sample_format: _,
        } = reader.spec();

        let mut samples_i16 = Vec::new();
        for sample_result in reader.samples::<i16>() {
            match sample_result {
                Ok(sample) => samples_i16.push(sample),
                Err(e) => return Err(format!("Failed to read audio sample: {}", e)),
            }
        }

        // i16 → f32 normalization ([-32768, 32767] → [-1.0, 1.0]), averaging
        // however many channels the file carries down to the single channel
        // Whisper wants.
        let mut processed_audio = downmix_i16_to_mono(&samples_i16, channels as usize);

        // Resample if needed
        if sample_rate != WHISPER_SAMPLE_RATE {
            let resample_params = sinc_resampling_params();

            let mut resampler = SincFixedIn::new(
                WHISPER_SAMPLE_RATE as f64 / sample_rate as f64,
                2.0,
                resample_params,
                processed_audio.len(),
                1,
            )
            .map_err(|e| format!("Failed to create resampler: {:?}", e))?;

            let waves_in = vec![processed_audio];
            let waves_out = resampler
                .process(&waves_in, None)
                .map_err(|e| format!("Resampling failed: {:?}", e))?;

            if waves_out.is_empty() || waves_out[0].is_empty() {
                return Err("Resampling produced empty audio".to_string());
            }

            processed_audio = waves_out
                .into_iter()
                .next()
                .ok_or_else(|| "Resampling failed to produce audio data".to_string())?;
        }

        session.transcribe_final(&processed_audio)
    }

    pub fn start_dictation<R: Runtime + 'static>(
        &mut self,
        app_handle: &AppHandle<R>,
    ) -> Result<()> {
        // Check if controller is initialized before starting dictation
        self.ensure_initialized()?;

        if self.is_dictating {
            return Err(Error::AlreadyDictating);
        }

        // Check microphone permission
        info!("[VoiceController] Checking microphone permission before starting dictation...");
        let permission_status = crate::mic_permissions::check_microphone_permission();
        match permission_status {
            crate::mic_permissions::MicrophonePermissionStatus::Granted => {
                info!("[VoiceController] Microphone permission is granted");
            }
            crate::mic_permissions::MicrophonePermissionStatus::Denied => {
                return Err(Error::MicrophonePermissionDenied);
            }
            crate::mic_permissions::MicrophonePermissionStatus::Undetermined => {
                // Permission will be requested when we try to use the microphone
                info!("[VoiceController] Microphone permission is undetermined, will be requested");
            }
            crate::mic_permissions::MicrophonePermissionStatus::NotApplicable => {
                // Non-macOS platform, continue
                info!(
                    "[VoiceController] Microphone permission check not applicable on this platform"
                );
            }
        }

        info!("[VoiceController] Starting dictation...");

        // Emit dictation started event
        app_handle
            .emit(constants::voice_transcription::DICTATION_STARTED, ())
            .map_err(|e| Error::Tauri(e.to_string()))?;

        // Clear the last processed audio buffer
        if let Ok(mut buffer_guard) = self.last_processed_audio_buffer.lock() {
            *buffer_guard = None;
        }

        // The microphone the person picked, or the system's, or the system's
        // standing in for one that was unplugged. A failure here is a sentence
        // the UI can show, which is why it comes from `CaptureStartFailure`
        // rather than being written out again.
        let resolved = devices::resolve_input_device(devices::preferred_input_device().as_deref())
            .map_err(|failure| Error::AudioDevice(failure.message()))?;
        if let Some(requested) = &resolved.substituted_for {
            tracing::warn!(
                "[VoiceController] {requested} is not connected; dictating through {}",
                resolved.name
            );
            let _ = app_handle.emit(
                constants::voice_capture::DEVICE_SUBSTITUTED,
                serde_json::json!({ "requested": requested, "used": &resolved.name }),
            );
        }
        let device_name = resolved.name;
        let device = resolved.device;

        let mut supported_configs_iter = device.supported_input_configs().map_err(|e| {
            Error::AudioDevice(format!(
                "Failed to get input device configs (microphone permission may be denied): {:?}",
                e
            ))
        })?;

        let selected_config_range = supported_configs_iter
            // No channel filter. Any count downmixes to mono below, and
            // filtering for 1 or 2 used to leave a device that only offers
            // three with "No suitable input config found", which reads as a
            // broken microphone rather than as a limit we imposed.
            .find(|c| {
                (c.min_sample_rate().0..=c.max_sample_rate().0).contains(&16000)
                    && (c.sample_format() == SampleFormat::F32
                        || c.sample_format() == SampleFormat::I16)
            });

        let supported_config = if let Some(conf_range) = selected_config_range {
            conf_range.with_sample_rate(cpal::SampleRate(16000))
        } else {
            device
                .supported_input_configs()
                .map_err(|e| {
                    Error::AudioDevice(format!(
                        "Failed to get device configs for fallback: {:?}",
                        e
                    ))
                })?
                .find(|c| {
                    c.sample_format() == SampleFormat::F32 || c.sample_format() == SampleFormat::I16
                })
                .map(|c| {
                    if (c.min_sample_rate().0..=c.max_sample_rate().0).contains(&16000) {
                        c.with_sample_rate(cpal::SampleRate(16000))
                    } else {
                        c.with_sample_rate(c.min_sample_rate())
                    }
                })
                .ok_or_else(|| Error::AudioDevice("No suitable input config found.".to_string()))?
        };

        let config = supported_config.config();
        let sample_format = supported_config.sample_format();
        let actual_rate = config.sample_rate.0;
        let channels = config.channels;

        // Store the actual sample rate safely
        if let Ok(mut rate_guard) = self.actual_recording_sample_rate.lock() {
            *rate_guard = Some(actual_rate);
        } else {
            tracing::error!(
                "Failed to acquire lock for actual_recording_sample_rate - lock may be poisoned"
            );
        }

        let open: CaptureOpen = Box::new(move |audio_data_tx: Sender<Vec<f32>>| {
            let stream = devices::start_mono_stream(
                &device,
                &device_name,
                &config,
                sample_format,
                audio_data_tx,
            )?;
            info!("[AudioThread] Audio stream started on {device_name}.");
            Ok(Box::new((stream, device)) as Box<dyn std::any::Any>)
        });

        self.spawn_audio_worker(app_handle, open, actual_rate, channels)
    }

    /// Start the thread that owns the microphone. Everything after device
    /// resolution, so a test can run it with a fake microphone.
    fn spawn_audio_worker<R: Runtime + 'static>(
        &mut self,
        app_handle: &AppHandle<R>,
        open: CaptureOpen,
        actual_rate: u32,
        channels: u16,
    ) -> Result<()> {
        let (control_tx, control_rx) = channel::<AudioThreadMessage>();
        let (audio_data_tx, audio_data_rx) = channel::<Vec<f32>>();

        let last_buffer_arc_for_thread = Arc::clone(&self.last_processed_audio_buffer);
        let app_handle_for_thread = app_handle.clone();
        let live_partial_for_thread = self.live_partial.clone();
        let live_captures_for_thread = Arc::clone(&self.live_captures);

        let engine = self
            .engine
            .as_ref()
            .ok_or_else(|| Error::InitializationError("STT engine not initialized".to_string()))?
            .clone();

        let audio_thread_handle = thread::spawn(move || {
            Self::audio_thread_worker(
                engine,
                last_buffer_arc_for_thread,
                actual_rate,
                channels,
                app_handle_for_thread,
                control_rx,
                audio_data_tx,
                audio_data_rx,
                open,
                live_partial_for_thread,
                live_captures_for_thread,
            );
        });

        self.audio_thread = Some((audio_thread_handle, control_tx));
        self.is_dictating = true;

        Ok(())
    }

    /// How many dictation microphone streams are open right now.
    ///
    /// Zero whenever no dictation or spoken query is recording. Anything else
    /// after a stop or a cancel is a hot microphone nobody asked for.
    pub fn active_capture_handles(&self) -> usize {
        self.live_captures.load(Ordering::SeqCst)
    }

    /// Stop claiming to be recording, and say why.
    ///
    /// [`capture_failure::report`] puts the sentence where the person can read
    /// it; `DICTATION_STOPPED` is the event the bar and the app already use to
    /// unwind a session, so a capture that never started unwinds exactly like
    /// one that ended rather than leaving the UI mid-dictation forever.
    fn give_up_on_capture<R: Runtime>(app_handle: &AppHandle<R>, failure: CaptureStartFailure) {
        capture_failure::report(app_handle, &failure);
        if let Err(e) = app_handle.emit(constants::voice_transcription::DICTATION_STOPPED, ()) {
            tracing::error!("[AudioThread] Could not report the stop: {e}");
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn audio_thread_worker<R: Runtime + 'static>(
        engine: Arc<dyn TranscriptionEngine>,
        last_buffer_arc: Arc<Mutex<Option<Vec<f32>>>>,
        actual_rate: u32,
        channels: u16,
        app_handle: AppHandle<R>,
        control_rx: std::sync::mpsc::Receiver<AudioThreadMessage>,
        audio_data_tx: std::sync::mpsc::Sender<Vec<f32>>,
        audio_data_rx: std::sync::mpsc::Receiver<Vec<f32>>,
        open: CaptureOpen,
        live_partial: PartialGate,
        live_captures: Arc<AtomicUsize>,
    ) {
        info!(
            "[AudioThread] Thread started. Engine: '{}', live_partial: {}",
            engine.name(),
            live_partial.is_open()
        );

        let mut session = match engine.create_session() {
            Ok(s) => s,
            Err(e) => {
                Self::give_up_on_capture(
                    &app_handle,
                    CaptureStartFailure::EngineUnavailable {
                        detail: e.to_string(),
                    },
                );
                return;
            }
        };

        // A stop or cancel that is already waiting (a quick press whose release
        // beat this thread here) is honoured before any input is opened.
        // Opening the device only to close it again still flashes the orange
        // indicator, and nothing would be recorded anyway.
        match control_rx.try_recv() {
            Ok(AudioThreadMessage::Stop) => {
                info!("[AudioThread] Stopped before the microphone opened.");
                Self::process_final_audio(
                    session.as_mut(),
                    &[],
                    actual_rate,
                    &app_handle,
                    &last_buffer_arc,
                );
                return;
            }
            Ok(AudioThreadMessage::Discard) => {
                info!("[AudioThread] Cancelled before the microphone opened.");
                let _ = app_handle.emit(constants::voice_transcription::DICTATION_STOPPED, ());
                return;
            }
            Err(TryRecvError::Disconnected) => {
                info!("[AudioThread] Control channel gone before the microphone opened.");
                return;
            }
            Err(TryRecvError::Empty) => {}
        }

        // The one exit between here and a running microphone, and it tells the
        // person why. Before this, each of these failures logged an error and
        // returned, leaving the bar drawn as if it were recording.
        let keepalive = match open(audio_data_tx.clone()) {
            Ok(keepalive) => keepalive,
            Err(failure) => {
                // On macOS a refused stream is usually microphone access, and
                // the cached answer is now stale.
                crate::mic_permissions::invalidate_permission_cache();
                Self::give_up_on_capture(&app_handle, failure);
                return;
            }
        };
        // Held for as long as the loop runs, and dropped on every way out of
        // this function. Dropping it stops the microphone, and the end path
        // joins this thread, so by the time a stop or cancel returns the
        // stream is gone.
        let _capture = CaptureGuard::new(keepalive, live_captures);

        info!("[AudioThread] Recording at {} Hz with {} channel(s), will resample to {} Hz for Whisper.",
              actual_rate, channels, WHISPER_SAMPLE_RATE);

        let mut raw_full_session_audio: Vec<f32> = Vec::new();

        // Audio level emission: throttled to ~70ms to match Clicky's sampling rate
        let level_emit_interval = Duration::from_millis(70);
        let mut last_level_emit = Instant::now() - level_emit_interval;

        // Live streaming partial mode: emit cumulative provisional text on a fast
        // cadence and keep a bounded sliding window so decode cost stays capped.
        let mut live_window = LivePartialWindow::new(
            (actual_rate as u64 * LIVE_PARTIAL_WINDOW_SECS) as usize,
            LIVE_PARTIAL_CADENCE,
        );

        loop {
            // Check for control messages
            match control_rx.try_recv() {
                Ok(AudioThreadMessage::Stop) => {
                    info!("[AudioThread] Stop message received.");
                    info!(
                        "[AudioThread] Live window: {} samples",
                        live_window.samples().len()
                    );
                    info!(
                        "[AudioThread] Raw session audio size: {} samples ({:.2} seconds)",
                        raw_full_session_audio.len(),
                        raw_full_session_audio.len() as f32 / actual_rate as f32
                    );

                    // Process final audio
                    Self::process_final_audio(
                        session.as_mut(),
                        &raw_full_session_audio,
                        actual_rate,
                        &app_handle,
                        &last_buffer_arc,
                    );

                    // Reset waveform to baseline when recording ends
                    let _ = app_handle.emit(
                        constants::voice_transcription::AUDIO_LEVEL,
                        serde_json::json!({ "level": 0.0_f32 }),
                    );

                    break;
                }
                Ok(AudioThreadMessage::Discard) => {
                    info!(
                        "[AudioThread] Discard message received; dropping {} samples unheard.",
                        raw_full_session_audio.len()
                    );
                    // No transcription, no FINAL_RESULT: emitting one is what
                    // submits the query downstream, and this path exists
                    // precisely so nothing is submitted.
                    let _ = app_handle.emit(
                        constants::voice_transcription::AUDIO_LEVEL,
                        serde_json::json!({ "level": 0.0_f32 }),
                    );
                    let _ = app_handle.emit(constants::voice_transcription::DICTATION_STOPPED, ());
                    break;
                }
                Err(TryRecvError::Empty) => {}
                Err(TryRecvError::Disconnected) => {
                    info!("[AudioThread] Control channel disconnected.");
                    break;
                }
            }

            // Process audio data
            if let Ok(audio_chunk) = audio_data_rx.recv_timeout(Duration::from_millis(100)) {
                raw_full_session_audio.extend_from_slice(&audio_chunk);
                // Read live, every pass: the setting can change mid-session.
                let live = live_partial.is_open();
                if live {
                    live_window.push(&audio_chunk);
                }

                // Emit audio level at ~70ms intervals for waveform visualization
                if last_level_emit.elapsed() >= level_emit_interval {
                    let rms = calculate_rms_volume(&audio_chunk);
                    // Scale: typical speech RMS 0.02–0.1 maps to ~0.2–1.0 display range
                    let level = (rms * 10.0_f32).min(1.0_f32);
                    let _ = app_handle.emit(
                        constants::voice_transcription::AUDIO_LEVEL,
                        serde_json::json!({ "level": level }),
                    );
                    last_level_emit = Instant::now();
                }

                // Process partial transcriptions. Off means off: nothing is
                // decoded, so nothing can be emitted.
                if live {
                    // Decode the whole bounded window (never cleared) and emit
                    // the cumulative text as provisional. Display-only.
                    if let Some(window) = live_window.take_due(Instant::now()) {
                        Self::process_partial_transcription(
                            session.as_mut(),
                            window,
                            actual_rate,
                            &app_handle,
                            &live_partial,
                        );
                    }
                }
            }
        }
    }

    fn process_partial_transcription<R: Runtime>(
        session: &mut dyn TranscriptionSession,
        audio_buffer: &[f32],
        actual_rate: u32,
        app_handle: &AppHandle<R>,
        gate: &PartialGate,
    ) {
        if !gate.is_open() {
            return;
        }
        let audio_to_transcribe = resample_for_partial(audio_buffer, actual_rate);

        if audio_to_transcribe.is_empty() {
            return;
        }

        match session.transcribe_partial(&audio_to_transcribe) {
            Ok(Some(text)) if !text.is_empty() => {
                tracing::debug!(
                    "[AudioThread] Live partial ({} samples): '{}'",
                    audio_to_transcribe.len(),
                    text
                );
                if let Some(payload) = partial_event(gate, &text, true) {
                    let _ =
                        app_handle.emit(constants::voice_transcription::PARTIAL_RESULT, payload);
                }
            }
            Ok(_) => {}
            Err(e) => tracing::error!("[AudioThread] Error transcribing partial chunk: {}", e),
        }
    }

    fn process_final_audio<R: Runtime>(
        session: &mut dyn TranscriptionSession,
        raw_full_session_audio: &[f32],
        actual_rate: u32,
        app_handle: &AppHandle<R>,
        last_buffer_arc: &Arc<Mutex<Option<Vec<f32>>>>,
    ) {
        // Store raw audio for potential playback
        if let Ok(mut buffer_guard) = last_buffer_arc.lock() {
            *buffer_guard = Some(raw_full_session_audio.to_vec());
        }

        // Prepare audio for final transcription
        let audio_for_transcription = if actual_rate != WHISPER_SAMPLE_RATE {
            if !raw_full_session_audio.is_empty() {
                let params = SincInterpolationParameters {
                    sinc_len: 256,
                    f_cutoff: 0.95,
                    interpolation: SincInterpolationType::Linear,
                    oversampling_factor: 256,
                    window: WindowFunction::BlackmanHarris2,
                };

                let mut final_resampler = match SincFixedIn::new(
                    WHISPER_SAMPLE_RATE as f64 / actual_rate as f64,
                    2.0,
                    params,
                    raw_full_session_audio.len(),
                    1,
                ) {
                    Ok(r) => r,
                    Err(e) => {
                        tracing::error!("Failed to create final resampler: {:?}", e);
                        return;
                    }
                };

                let waves_in = vec![raw_full_session_audio.to_vec()];
                match final_resampler.process(&waves_in, None) {
                    Ok(mut resampled_waves) => {
                        if resampled_waves.is_empty() || resampled_waves[0].is_empty() {
                            Vec::new()
                        } else {
                            resampled_waves.remove(0)
                        }
                    }
                    Err(e) => {
                        tracing::error!("Error during final resampling: {:?}", e);
                        Vec::new()
                    }
                }
            } else {
                Vec::new()
            }
        } else {
            raw_full_session_audio.to_vec()
        };

        // Perform final transcription
        if !audio_for_transcription.is_empty() {
            info!("[AudioThread] Performing final transcription on {} samples ({:.2} seconds at 16kHz)",
                  audio_for_transcription.len(),
                  audio_for_transcription.len() as f32 / WHISPER_SAMPLE_RATE as f32);

            match session.transcribe_final(&audio_for_transcription) {
                Ok(transcription_text) => {
                    info!(
                        "[AudioThread] Final transcription result: '{}'",
                        transcription_text
                    );
                    let _ = app_handle.emit(
                        constants::voice_transcription::FINAL_RESULT,
                        serde_json::json!({ "text": transcription_text }),
                    );
                    let _ = app_handle.emit(constants::voice_transcription::DICTATION_STOPPED, ());
                }
                Err(e) => {
                    tracing::error!("Final transcription failed: {}", e);
                    let _ = app_handle.emit(
                        constants::voice_transcription::ERROR,
                        serde_json::json!({
                            "type": "transcription_failed",
                            "message": format!("Final transcription failed: {}", e)
                        }),
                    );
                    let _ = app_handle.emit(constants::voice_transcription::DICTATION_STOPPED, ());
                }
            }
        } else {
            // An empty transcript is still this session's transcript, and it
            // has to be emitted.
            //
            // The app registers a voice session when the microphone opens and
            // retires it when the transcript arrives. A stop moves it to
            // "finishing" and leaves it registered, because a transcript is
            // still owed. This branch used to owe one and never pay it: it
            // announced the stop and nothing else, so the session stayed
            // registered forever — and a session standing is what makes the
            // app refuse every later start. Say nothing at all into a
            // dictation and dictation was dead until the app restarted, with
            // the liveness flag already down so nothing looked wrong.
            //
            // So the rule is: a session that was asked to finalise always gets
            // exactly one result, even when there was nothing to decode.
            info!("[AudioThread] No audio to transcribe (empty buffer); emitting an empty result so the session is closed out");
            let _ = app_handle.emit(
                constants::voice_transcription::FINAL_RESULT,
                serde_json::json!({ "text": "" }),
            );
            let _ = app_handle.emit(constants::voice_transcription::DICTATION_STOPPED, ());
        }
    }

    pub fn stop_dictation(&mut self) -> Result<bool> {
        self.end_dictation(AudioThreadMessage::Stop)
    }

    /// Stop listening and throw the audio away, so nothing is transcribed and
    /// nothing is submitted.
    pub fn cancel_dictation(&mut self) -> Result<bool> {
        self.end_dictation(AudioThreadMessage::Discard)
    }

    fn end_dictation(&mut self, message: AudioThreadMessage) -> Result<bool> {
        if !self.is_dictating {
            return Ok(false);
        }

        self.is_dictating = false;

        if let Some((thread_handle, control_tx)) = self.audio_thread.take() {
            let _ = control_tx.send(message);

            match thread_handle.join() {
                Ok(_) => {
                    info!("[VoiceController] Audio thread joined successfully.");
                    Ok(true)
                }
                Err(_) => {
                    tracing::error!("[VoiceController] Failed to join audio thread.");
                    Err(Error::Other("Failed to join audio thread".to_string()))
                }
            }
        } else {
            Ok(false)
        }
    }

    pub fn toggle_dictation<R: Runtime + 'static>(
        &mut self,
        app_handle: AppHandle<R>,
    ) -> Result<bool> {
        if self.is_dictating {
            self.stop_dictation()?;
            Ok(false)
        } else {
            self.start_dictation(&app_handle)?;
            Ok(true)
        }
    }

    pub fn is_dictating(&self) -> bool {
        self.is_dictating
    }

    pub fn get_last_processed_audio_buffer(&self) -> Option<(Vec<f32>, u32)> {
        let buffer = self.last_processed_audio_buffer.lock().ok()?.clone()?;
        let rate = (*self.actual_recording_sample_rate.lock().ok()?)?;
        Some((buffer, rate))
    }
}

/// The one end path for a recording, shared by stop (finalise) and cancel
/// (discard). Waits for the controller rather than giving up when it is busy.
///
/// The commands used to `try_lock` and answer `Ok(false)` when another call
/// held the controller ("cancel will land when the lock frees"). Nothing ever
/// landed it: every caller took `Ok` as done, retired the session, and put the
/// bar back to rest while the audio thread kept the microphone open. With no
/// session left to own it, no later stop or cancel reached it either, which is
/// the stuck orange indicator. Joining the audio thread is what drops the
/// stream, so when this returns the microphone is closed. A poisoned lock still
/// holds the controller, so it is recovered rather than left recording.
pub fn end_dictation_waiting(controller: &Mutex<VoiceController>, discard: bool) -> Result<bool> {
    let mut guard = controller.lock().unwrap_or_else(|p| p.into_inner());
    if discard {
        guard.cancel_dictation()
    } else {
        guard.stop_dictation()
    }
}

impl Drop for VoiceController {
    fn drop(&mut self) {
        if self.is_dictating {
            tracing::info!("[VoiceController] Drop: stopping active dictation");
            self.is_dictating = false;
            if let Some((thread_handle, control_tx)) = self.audio_thread.take() {
                let _ = control_tx.send(AudioThreadMessage::Stop);
                // Give the thread a moment to finish, but don't block indefinitely
                let _ = thread_handle.join();
            }
        }
    }
}

// SAFETY NOTE: VoiceController is always wrapped in Arc<Mutex<VoiceController>> at call sites
// (see lib.rs). The Mutex provides the necessary synchronization, so we only need Send.
// All fields are individually Send (Arc, Mutex, Option, bool, String).
// We do NOT impl Sync because mutable fields (is_dictating, audio_thread) lack interior
// mutability — the wrapping Mutex handles thread safety.
unsafe impl Send for VoiceController {}

#[cfg(test)]
mod tests {
    use super::*;

    // The bug these pin: the old partial path shared a `SincFixedIn` built for
    // 1024-frame chunks, which silently used only the first 1024 frames of
    // whatever it was handed. A ten-second window became 21 ms of audio and
    // decoded to nothing, so "show words as I speak" did nothing on every
    // microphone that does not run at 16 kHz.

    #[test]
    fn a_partial_window_keeps_its_whole_duration_at_48k() {
        // Ten seconds at 48 kHz must come back as ten seconds at 16 kHz, not
        // as the first 1024 input frames.
        let ten_seconds = vec![0.25_f32; 48_000 * 10];
        let out = resample_for_partial(&ten_seconds, 48_000);
        assert_eq!(out.len(), 16_000 * 10);
    }

    #[test]
    fn a_partial_window_at_whisper_rate_is_passed_through_untouched() {
        let audio: Vec<f32> = (0..100).map(|i| i as f32).collect();
        assert_eq!(resample_for_partial(&audio, WHISPER_SAMPLE_RATE), audio);
    }

    #[test]
    fn a_partial_window_keeps_its_signal_rather_than_its_first_frame() {
        // Averaging, not decimation-by-dropping: a ramp stays a ramp.
        let audio: Vec<f32> = (0..48).map(|i| i as f32).collect();
        let out = resample_for_partial(&audio, 48_000);
        assert_eq!(out.len(), 16);
        assert_eq!(out[0], 1.0, "mean of 0,1,2");
        assert_eq!(out[15], 46.0, "mean of 45,46,47");
    }

    #[test]
    fn a_partial_window_survives_rates_that_do_not_divide_evenly() {
        // 44.1 kHz is not a whole multiple of 16 kHz; the window must still
        // come back whole and in range rather than empty or panicking.
        let one_second = vec![0.1_f32; 44_100];
        let out = resample_for_partial(&one_second, 44_100);
        assert_eq!(out.len(), 16_000);
        assert!(out.iter().all(|s| (*s - 0.1).abs() < 1e-6));
    }

    #[test]
    fn a_partial_window_at_a_lower_rate_is_brought_up_not_dropped() {
        let audio: Vec<f32> = (0..8).map(|i| i as f32).collect();
        let out = resample_for_partial(&audio, 8_000);
        assert_eq!(out.len(), 16);
        assert!(out.iter().all(|s| s.is_finite()));
    }

    #[test]
    fn an_empty_or_nonsense_rate_yields_nothing_rather_than_panicking() {
        assert!(resample_for_partial(&[], 48_000).is_empty());
        assert_eq!(resample_for_partial(&[1.0, 2.0], 0), vec![1.0, 2.0]);
        // Fewer input frames than one output frame is worth: no output, no panic.
        assert!(resample_for_partial(&[1.0], 48_000).is_empty());
    }

    #[test]
    fn live_window_keeps_only_the_newest_samples() {
        let mut window = LivePartialWindow::new(5, LIVE_PARTIAL_CADENCE);
        window.push(&[1.0, 2.0, 3.0, 4.0]);
        window.push(&[5.0, 6.0, 7.0]);
        assert_eq!(window.samples(), &[3.0, 4.0, 5.0, 6.0, 7.0][..]);
    }

    #[test]
    fn live_window_at_mic_rate_is_capped_at_ten_seconds() {
        let rate = 16_000usize;
        let mut window = LivePartialWindow::new(
            rate * LIVE_PARTIAL_WINDOW_SECS as usize,
            LIVE_PARTIAL_CADENCE,
        );
        // Thirty seconds of audio in cpal-sized chunks.
        for _ in 0..(30 * rate / 512) {
            window.push(&[0.25; 512]);
        }
        assert_eq!(window.samples().len(), rate * 10);
    }

    #[test]
    fn live_window_never_decodes_an_empty_window() {
        let mut window = LivePartialWindow::new(10, LIVE_PARTIAL_CADENCE);
        assert!(window.take_due(Instant::now()).is_none());
    }

    #[test]
    fn live_window_decodes_at_most_once_per_cadence_and_always_cumulatively() {
        let cadence = Duration::from_millis(600);
        let mut window = LivePartialWindow::new(100, cadence);
        let t0 = Instant::now();

        window.push(&[0.1; 8]);
        assert!(
            window.take_due(t0).is_some(),
            "the first chunk is decoded right away"
        );
        assert!(window.take_due(t0 + Duration::from_millis(100)).is_none());

        window.push(&[0.1; 8]);
        assert!(window.take_due(t0 + Duration::from_millis(599)).is_none());
        let due = window
            .take_due(t0 + cadence)
            .expect("due again once the cadence has elapsed");
        assert_eq!(
            due.len(),
            16,
            "the cumulative window, not only the new chunk"
        );
    }

    #[test]
    fn no_partial_is_built_while_the_setting_is_off_and_one_is_when_on() {
        let gate = PartialGate::default();
        assert!(
            partial_event(&gate, "hello", true).is_none(),
            "off by default: no partial event exists"
        );

        gate.set(true);
        let on = partial_event(&gate, "hello", true).expect("on: a partial is built");
        assert_eq!(on["text"], "hello");
        assert_eq!(on["provisional"], true);

        gate.set(false);
        assert!(partial_event(&gate, "hello", true).is_none());
    }

    #[test]
    fn the_gate_is_read_live_through_a_clone_held_by_the_audio_thread() {
        let mut controller = VoiceController::new_uninitialized("m.bin", "x".into());
        let held_by_audio_thread = controller.live_partial.clone();

        controller.set_live_partial(true);
        assert!(
            held_by_audio_thread.is_open(),
            "toggle reaches a running session"
        );
        controller.set_live_partial(false);
        assert!(!held_by_audio_thread.is_open());
    }

    /// Pins the link: the setting is consulted at the decision point, and the
    /// event name is used nowhere else, so no new path can emit around it.
    #[test]
    fn partial_results_are_only_emitted_through_the_gate() {
        let src = include_str!("controller.rs");
        let production = src.split("#[cfg(test)]").next().unwrap();
        assert_eq!(
            production
                .matches("voice_transcription::PARTIAL_RESULT")
                .count(),
            1,
            "one emit site for partials"
        );
        let emit = production
            .find("voice_transcription::PARTIAL_RESULT")
            .unwrap();
        let before = &production[..emit];
        let tail = &before[before.len().saturating_sub(400)..];
        assert!(
            tail.contains("partial_event(gate"),
            "the emit is fed by partial_event, which reads the setting"
        );
        assert!(
            production.contains("let live = live_partial.is_open();"),
            "the audio loop reads the setting on every pass"
        );
    }

    #[test]
    fn adopt_carries_live_partial_across_controller_replacement() {
        let mut slot = VoiceController::new_uninitialized("models/a.bin", "loading".into());
        slot.set_live_partial(true);

        // What the plugin does once its background engine load finishes.
        slot.adopt(VoiceController::new_uninitialized(
            "models/b.bin",
            "still loading".into(),
        ));

        assert!(
            slot.live_partial(),
            "the flag applied before the swap survives it"
        );
        assert_eq!(
            slot.model_path, "models/b.bin",
            "everything else is the replacement's"
        );

        let mut off = VoiceController::new_uninitialized("models/a.bin", "loading".into());
        off.adopt(VoiceController::new_uninitialized(
            "models/b.bin",
            "x".into(),
        ));
        assert!(
            !off.live_partial(),
            "a flag that was never on does not turn on"
        );
    }

    // ── Every way a recording ends drops the microphone ──
    //
    // A fake microphone stands in for cpal: a token the audio thread holds the
    // way it holds the real stream, counted by the same guard. "Is the mic
    // still open" is then a number, read the moment the end path returns.

    struct SilentEngine;
    struct SilentSession;

    impl TranscriptionSession for SilentSession {
        fn transcribe_partial(
            &mut self,
            _audio: &[f32],
        ) -> std::result::Result<Option<String>, String> {
            Ok(None)
        }
        fn transcribe_final(&mut self, _audio: &[f32]) -> std::result::Result<String, String> {
            Ok(String::new())
        }
    }

    impl TranscriptionEngine for SilentEngine {
        fn name(&self) -> &'static str {
            "silent"
        }
        fn supports_streaming(&self) -> bool {
            false
        }
        fn is_initialized(&self) -> bool {
            true
        }
        fn create_session(&self) -> std::result::Result<Box<dyn TranscriptionSession>, String> {
            Ok(Box::new(SilentSession))
        }
    }

    type OpenResult = std::result::Result<Box<dyn std::any::Any>, CaptureStartFailure>;

    fn fake_microphone() -> CaptureOpen {
        Box::new(|tx: Sender<Vec<f32>>| -> OpenResult {
            let _ = tx.send(vec![0.0; 1600]);
            Ok(Box::new(tx) as Box<dyn std::any::Any>)
        })
    }

    fn controller() -> VoiceController {
        VoiceController::new_with_engine("test", Arc::new(SilentEngine)).expect("controller")
    }

    fn start_fake<R: Runtime + 'static>(c: &mut VoiceController, app: &AppHandle<R>) {
        c.spawn_audio_worker(app, fake_microphone(), WHISPER_SAMPLE_RATE, 1)
            .expect("start");
        let deadline = Instant::now() + Duration::from_secs(5);
        while c.active_capture_handles() != 1 && Instant::now() < deadline {
            thread::sleep(Duration::from_millis(5));
        }
        assert_eq!(c.active_capture_handles(), 1, "the fake microphone opened");
    }

    #[test]
    fn send_leaves_no_microphone_open() {
        let app = tauri::test::mock_app();
        let mut c = controller();
        start_fake(&mut c, app.handle());
        assert!(c.stop_dictation().expect("stop"));
        assert_eq!(c.active_capture_handles(), 0);
        assert!(!c.is_dictating());
    }

    #[test]
    fn cancel_leaves_no_microphone_open() {
        let app = tauri::test::mock_app();
        let mut c = controller();
        start_fake(&mut c, app.handle());
        assert!(c.cancel_dictation().expect("cancel"));
        assert_eq!(c.active_capture_handles(), 0);
        // A second end (X after Send, Escape after X) is harmless.
        assert!(!c.cancel_dictation().expect("cancel again"));
        assert!(!c.stop_dictation().expect("stop after cancel"));
        assert_eq!(c.active_capture_handles(), 0);
    }

    #[test]
    fn dropping_the_controller_leaves_no_microphone_open() {
        let app = tauri::test::mock_app();
        let mut c = controller();
        start_fake(&mut c, app.handle());
        let live = Arc::clone(&c.live_captures);
        drop(c);
        assert_eq!(live.load(Ordering::SeqCst), 0);
    }

    #[test]
    fn a_microphone_that_never_opened_counts_nothing_and_ends_cleanly() {
        let app = tauri::test::mock_app();
        let mut c = controller();
        c.spawn_audio_worker(
            app.handle(),
            Box::new(|_tx: Sender<Vec<f32>>| -> OpenResult {
                Err(CaptureStartFailure::NoInputDevice)
            }),
            WHISPER_SAMPLE_RATE,
            1,
        )
        .expect("start");
        assert!(c.cancel_dictation().is_ok());
        assert_eq!(c.active_capture_handles(), 0);
        assert!(!c.is_dictating());
    }

    /// The stuck-mic bug. The plugin's stop and cancel used to `try_lock` and
    /// return `Ok(false)` while anything else held the controller; the caller
    /// took that as done and the microphone stayed open with no session left
    /// to own it. The shared end path now waits, for both verbs.
    #[test]
    fn ending_while_the_controller_is_busy_still_closes_the_microphone() {
        for discard in [true, false] {
            let app = tauri::test::mock_app();
            let mut c = controller();
            start_fake(&mut c, app.handle());
            let live = Arc::clone(&c.live_captures);
            let shared = Arc::new(Mutex::new(c));

            let (held_tx, held_rx) = channel::<()>();
            let holder = {
                let shared = Arc::clone(&shared);
                thread::spawn(move || {
                    let _guard = shared.lock().unwrap();
                    held_tx.send(()).unwrap();
                    thread::sleep(Duration::from_millis(150));
                })
            };
            held_rx.recv().unwrap();

            let ended = end_dictation_waiting(&shared, discard).expect("end");
            assert!(
                ended,
                "the end waited for the controller instead of giving up"
            );
            assert_eq!(
                live.load(Ordering::SeqCst),
                0,
                "no capture handle survives the end path (discard={discard})"
            );
            holder.join().unwrap();
        }
    }

    // ── The device itself is stopped, in every order a quick press can take ──
    //
    // The tests above count the guard that owns the stream. On macOS that was
    // not the same as the microphone: cpal's stream survived its own drop, so
    // the count said zero while the orange indicator stayed up. These use a
    // fake device that, like cpal's, keeps running when merely dropped, and
    // assert on the device and on a count of open inputs, for a stop and a
    // cancel, at each point the release can land.

    use crate::devices::fake_input;

    /// Blocks `create_session` until released, so an end can be queued
    /// before the audio thread reaches the microphone.
    struct GatedEngine {
        go: Mutex<Option<std::sync::mpsc::Receiver<()>>>,
    }

    impl TranscriptionEngine for GatedEngine {
        fn name(&self) -> &'static str {
            "gated"
        }
        fn supports_streaming(&self) -> bool {
            false
        }
        fn is_initialized(&self) -> bool {
            true
        }
        fn create_session(&self) -> std::result::Result<Box<dyn TranscriptionSession>, String> {
            if let Some(go) = self.go.lock().unwrap().take() {
                let _ = go.recv_timeout(Duration::from_secs(5));
            }
            Ok(Box::new(SilentSession))
        }
    }

    struct FakeDevice {
        open: Arc<AtomicUsize>,
        running: Arc<AtomicBool>,
        ever_opened: Arc<AtomicBool>,
    }

    impl FakeDevice {
        fn new() -> Self {
            Self {
                open: Arc::new(AtomicUsize::new(0)),
                running: Arc::new(AtomicBool::new(false)),
                ever_opened: Arc::new(AtomicBool::new(false)),
            }
        }

        /// Opens the fake device. With `entered`, says so and then waits for
        /// `go` before finishing, to hold the audio thread mid-open.
        fn microphone(
            &self,
            entered: Option<Sender<()>>,
            go: Option<std::sync::mpsc::Receiver<()>>,
        ) -> CaptureOpen {
            let open = Arc::clone(&self.open);
            let running = Arc::clone(&self.running);
            let ever_opened = Arc::clone(&self.ever_opened);
            Box::new(move |tx: Sender<Vec<f32>>| -> OpenResult {
                ever_opened.store(true, Ordering::SeqCst);
                let input = fake_input::start(&open, &running);
                if let Some(entered) = entered {
                    let _ = entered.send(());
                }
                if let Some(go) = go {
                    let _ = go.recv_timeout(Duration::from_secs(5));
                }
                let _ = tx.send(vec![0.0; 1600]);
                Ok(Box::new((input, tx)) as Box<dyn std::any::Any>)
            })
        }

        fn assert_closed(&self, c: &VoiceController, case: &str) {
            assert_eq!(
                self.open.load(Ordering::SeqCst),
                0,
                "{case}: an input stream is still open"
            );
            assert!(
                !self.running.load(Ordering::SeqCst),
                "{case}: the device is still running (orange indicator)"
            );
            assert_eq!(c.active_capture_handles(), 0, "{case}: capture handle");
            assert!(!c.is_dictating(), "{case}: still dictating");
        }
    }

    fn end(c: &mut VoiceController, discard: bool) -> bool {
        if discard {
            c.cancel_dictation().expect("cancel")
        } else {
            c.stop_dictation().expect("stop")
        }
    }

    /// Held long enough to open, then let go.
    #[test]
    fn an_end_after_the_microphone_opened_stops_the_device() {
        for discard in [true, false] {
            let app = tauri::test::mock_app();
            let device = FakeDevice::new();
            let mut c = controller();
            c.spawn_audio_worker(app.handle(), device.microphone(None, None), 16000, 1)
                .expect("start");
            let deadline = Instant::now() + Duration::from_secs(5);
            while device.open.load(Ordering::SeqCst) != 1 && Instant::now() < deadline {
                thread::sleep(Duration::from_millis(5));
            }
            assert_eq!(device.open.load(Ordering::SeqCst), 1, "it opened");
            assert!(end(&mut c, discard));
            device.assert_closed(&c, &format!("after open, discard={discard}"));
        }
    }

    /// Let go while the device was still opening.
    #[test]
    fn an_end_while_the_microphone_is_opening_stops_the_device() {
        for discard in [true, false] {
            let app = tauri::test::mock_app();
            let device = FakeDevice::new();
            let (entered_tx, entered_rx) = channel::<()>();
            let (go_tx, go_rx) = channel::<()>();
            let mut c = controller();
            c.spawn_audio_worker(
                app.handle(),
                device.microphone(Some(entered_tx), Some(go_rx)),
                16000,
                1,
            )
            .expect("start");
            entered_rx
                .recv_timeout(Duration::from_secs(5))
                .expect("the audio thread reached the device");
            let releaser = thread::spawn(move || {
                thread::sleep(Duration::from_millis(50));
                let _ = go_tx.send(());
            });
            assert!(end(&mut c, discard));
            releaser.join().unwrap();
            device.assert_closed(&c, &format!("mid-open, discard={discard}"));
        }
    }

    /// Let go before the audio thread reached the device at all: it must not
    /// be opened, not even for a moment.
    #[test]
    fn an_end_before_the_microphone_is_reached_never_opens_it() {
        for discard in [true, false] {
            let app = tauri::test::mock_app();
            let device = FakeDevice::new();
            let (go_tx, go_rx) = channel::<()>();
            let mut c = VoiceController::new_with_engine(
                "test",
                Arc::new(GatedEngine {
                    go: Mutex::new(Some(go_rx)),
                }),
            )
            .expect("controller");
            c.spawn_audio_worker(app.handle(), device.microphone(None, None), 16000, 1)
                .expect("start");
            let releaser = thread::spawn(move || {
                thread::sleep(Duration::from_millis(50));
                let _ = go_tx.send(());
            });
            assert!(end(&mut c, discard));
            releaser.join().unwrap();
            device.assert_closed(&c, &format!("before open, discard={discard}"));
            assert!(
                !device.ever_opened.load(Ordering::SeqCst),
                "an end that was already waiting opened the microphone (discard={discard})"
            );
        }
    }

    /// The end waited on a busy controller (#687, #694), with the real
    /// device check this time.
    #[test]
    fn an_end_behind_a_busy_controller_stops_the_device() {
        for discard in [true, false] {
            let app = tauri::test::mock_app();
            let device = FakeDevice::new();
            let mut c = controller();
            c.spawn_audio_worker(app.handle(), device.microphone(None, None), 16000, 1)
                .expect("start");
            let deadline = Instant::now() + Duration::from_secs(5);
            while device.open.load(Ordering::SeqCst) != 1 && Instant::now() < deadline {
                thread::sleep(Duration::from_millis(5));
            }
            let shared = Arc::new(Mutex::new(c));
            let (held_tx, held_rx) = channel::<()>();
            let holder = {
                let shared = Arc::clone(&shared);
                thread::spawn(move || {
                    let _guard = shared.lock().unwrap();
                    held_tx.send(()).unwrap();
                    thread::sleep(Duration::from_millis(100));
                })
            };
            held_rx.recv().unwrap();
            assert!(end_dictation_waiting(&shared, discard).expect("end"));
            holder.join().unwrap();
            let c = shared.lock().unwrap();
            device.assert_closed(&c, &format!("busy controller, discard={discard}"));
        }
    }
}

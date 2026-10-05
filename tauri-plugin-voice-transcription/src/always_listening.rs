use crate::engine::{TranscriptionEngine, TranscriptionSession};
use cpal::traits::DeviceTrait;
use rubato::{
    Resampler, SincFixedIn, SincInterpolationParameters, SincInterpolationType, WindowFunction,
};
use serde_json;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::mpsc::{channel, Sender, TryRecvError};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};
use tauri::{AppHandle, Emitter, Runtime};
use tracing::{debug, error, info, warn};

use crate::capture_failure::{self, CaptureStartFailure};
use crate::devices;
use crate::error::{Error, Result};
use crate::utils::filter_transcription_text;

// Audio processing constants (matching main crate's constants)
const WHISPER_SAMPLE_RATE: u32 = 16000;
const SINC_LENGTH: usize = 256;
const OVERSAMPLING_FACTOR: usize = 256;
const INTENT_DETECTION_BUFFER_MS: u64 = 3000; // Buffer for intent detection (increased from 1500)
const VOLUME_THRESHOLD: f32 = 0.01; // Increased from 0.003 to reduce false triggers
const VOLUME_THRESHOLD_END: f32 = 0.005; // Increased from 0.002
const SILENCE_TIMEOUT_MS: u64 = 3000; // Return to monitoring after silence
const VOLUME_DROP_TOLERANCE_MS: u64 = 200; // Allow brief volume drops during activity
const MIN_SPEECH_VOLUME: f32 = 0.02; // Minimum volume required for speech processing

/// How much unbroken speech goes by before we look for a wake word in it, and
/// how often we look again while someone keeps talking.
///
/// This used to be a second, and a second is longer than the word. Somebody who
/// says "juno" and stops is done in about half of one, so the old deadline was
/// never reached: the volume fell, the tolerance below expired, and the
/// utterance was thrown away unexamined. Six hundred milliseconds catches a
/// wake word spoken inside a longer sentence, and the end-of-speech check in
/// the worker catches the short one on its own.
const WAKE_WORD_CHECK_INTERVAL_MS: u64 = 600;

/// How much audio is kept after a check that found nothing.
///
/// The buffer used to be emptied, which chopped any word that happened to
/// straddle two checks in half and left the next check with too little audio to
/// transcribe at all. Keeping the tail means a wake word is always heard whole
/// by one check or the other, and that the following check still clears
/// Whisper's one-second floor.
const WAKE_WORD_PREROLL_MS: u64 = 600;

/// The least audio worth transcribing at all. Below this there is no word in
/// there, only a door or a keyboard.
const MIN_WAKE_WORD_AUDIO_MS: u64 = 350;

/// Whisper returns no segments at all for audio shorter than one second: its
/// decoder needs a hundred mel frames before it will look. Short buffers are
/// padded with silence up to this rather than discarded, which is what lets a
/// wake word be caught the moment it is finished instead of a second later.
const WHISPER_MIN_AUDIO_MS: u64 = 1000;

// New constants for intelligent filtering
const MIN_MEANINGFUL_CONTENT_LENGTH: usize = 3; // Minimum characters for meaningful content
const MAX_AGENT_CALLS_PER_MINUTE: u32 = 5; // Rate limit for agent calls

const COMMAND_COMPLETION_TIMEOUT_MS: u64 = 5000; // Return to wake word after command completion

// Sensitivity bounds
const MIN_SENSITIVITY: f32 = 0.1;
const MAX_SENSITIVITY: f32 = 2.0;
const DEFAULT_SENSITIVITY: f32 = 0.5;

// Default wake words for activation.
//
// "joono" used to sit in this list: a hand-spelled second spelling, added
// because Whisper hears the name several ways and the matcher compared
// literal strings. The matcher is phonetic now, so "joono" and "juneau" both
// reach "juno" on their own. Spelling out homophones here would be guessing
// at Whisper's output one transcription at a time, and every guess that
// missed looked to the person like Juno simply not answering.
const DEFAULT_WAKE_WORDS: &[&str] = &["hey juno", "juno", "computer", "hey computer"];

// Stop words that should end always listening mode
const STOP_WORDS: &[&str] = &[
    "stop",
    "nevermind",
    "never mind",
    "cancel",
    "quit",
    "exit",
    "done",
    "that's all",
    "thats all",
    "end",
    "finish",
    "enough",
];

// Single word noise patterns that should only match complete words
const NOISE_WORDS: &[&str] = &[
    "um", "uh", "hmm", "ah", "er", "mm", "mhm",
    // Single letter transcriptions are usually noise
    "a", "i", "o", "e", "u",
];

/// Create standard sinc interpolation parameters for audio resampling.
/// Used consistently across all resampling operations in this module.
fn sinc_resampling_params() -> SincInterpolationParameters {
    SincInterpolationParameters {
        sinc_len: SINC_LENGTH,
        f_cutoff: 0.95,
        interpolation: SincInterpolationType::Linear,
        oversampling_factor: OVERSAMPLING_FACTOR,
        window: WindowFunction::BlackmanHarris2,
    }
}

enum AlwaysListeningMessage {
    Stop,
    UpdateSensitivity(f32),
    UpdateWakeWords(Vec<String>),
    SetTranscriptionDebugging(bool),
    SetAudioLevelMonitoring(bool),
    ForceTranscriptionTest,
    ReturnToWakeWordMode, // New message for returning to wake word detection
}

#[derive(Debug, Clone)]
pub enum AlwaysListeningState {
    Monitoring,         // Continuously monitoring for intent
    Activated,          // Intent detected, actively transcribing
    Processing,         // Processing detected speech
    WaitingForWakeWord, // Waiting for wake word after command completion
}

/// A running microphone, and what it took to get one.
pub(crate) struct OpenCapture {
    /// The stream (and its device), held only to be dropped: dropping it is
    /// what stops capture and lets macOS take the orange indicator down. Boxed
    /// so a test can stand in for cpal without opening a real microphone.
    keepalive: Box<dyn std::any::Any>,
    sample_rate: u32,
    device_name: String,
    /// The microphone the person asked for, when it was not there and another
    /// one stood in.
    substituted_for: Option<String>,
}

/// How the worker opens its microphone. Runs on the worker thread, because a
/// cpal stream is not `Send` and must be created and dropped where it lives.
pub(crate) type CaptureOpener = Arc<
    dyn Fn(Sender<Vec<f32>>) -> std::result::Result<OpenCapture, CaptureStartFailure> + Send + Sync,
>;

fn default_capture_opener() -> CaptureOpener {
    Arc::new(open_capture_stream)
}

/// One open microphone stream, counted.
///
/// The controller is the only owner of the always-listening stream, and this
/// guard is the stream: it exists exactly as long as capture does. The count
/// it keeps is what `active_capture_handles` reports, so "is the mic still
/// open after I turned this off" is a number a test can read rather than an
/// orange dot somebody has to look at.
pub(crate) struct CaptureGuard {
    keepalive: Option<Box<dyn std::any::Any>>,
    live: Arc<AtomicUsize>,
}

impl CaptureGuard {
    pub(crate) fn new(keepalive: Box<dyn std::any::Any>, live: Arc<AtomicUsize>) -> Self {
        live.fetch_add(1, Ordering::SeqCst);
        Self {
            keepalive: Some(keepalive),
            live,
        }
    }
}

impl Drop for CaptureGuard {
    fn drop(&mut self) {
        // Stop the stream first, then say so.
        drop(self.keepalive.take());
        self.live.fetch_sub(1, Ordering::SeqCst);
    }
}

/// Open the microphone always-listening should use.
///
/// Every failure is a [`CaptureStartFailure`], so the caller has a sentence to
/// show the person. This is the whole point of the module: before it, each of
/// these cases logged an error and returned out of the worker thread, and the
/// settings window went on saying Juno was listening.
fn open_capture_stream(
    audio_data_tx: Sender<Vec<f32>>,
) -> std::result::Result<OpenCapture, CaptureStartFailure> {
    let resolved = devices::resolve_input_device(devices::preferred_input_device().as_deref())?;

    let supported = resolved.device.default_input_config().map_err(|e| {
        CaptureStartFailure::DeviceConfigUnavailable {
            device: resolved.name.clone(),
            detail: format!("{e:?}"),
        }
    })?;
    let sample_rate = supported.sample_rate().0;
    let sample_format = supported.sample_format();

    let stream = devices::start_mono_stream(
        &resolved.device,
        &resolved.name,
        &supported.config(),
        sample_format,
        audio_data_tx,
    )?;

    Ok(OpenCapture {
        keepalive: Box::new((stream, resolved.device)),
        sample_rate,
        device_name: resolved.name,
        substituted_for: resolved.substituted_for,
    })
}

pub struct AlwaysListeningController {
    engine: Option<Arc<dyn TranscriptionEngine>>,
    model_path: String,
    is_active: bool,
    state: AlwaysListeningState,
    audio_thread: Option<(thread::JoinHandle<()>, Sender<AlwaysListeningMessage>)>,
    sensitivity: f32,
    wake_words: Vec<String>,
    last_activity: Arc<Mutex<Option<Instant>>>,
    capture_opener: CaptureOpener,
    live_captures: Arc<AtomicUsize>,
}

impl AlwaysListeningController {
    /// Create an AlwaysListeningController backed by an already-initialized STT engine.
    pub fn new_with_engine(
        model_path_str: &str,
        engine: Arc<dyn TranscriptionEngine>,
    ) -> Result<Self> {
        info!(
            "[AlwaysListeningController] Creating controller with '{}' engine",
            engine.name()
        );
        Ok(Self {
            engine: Some(engine),
            model_path: model_path_str.to_string(),
            is_active: false,
            state: AlwaysListeningState::Monitoring,
            audio_thread: None,
            sensitivity: DEFAULT_SENSITIVITY,
            wake_words: DEFAULT_WAKE_WORDS.iter().map(|s| s.to_string()).collect(),
            last_activity: Arc::new(Mutex::new(None)),
            capture_opener: default_capture_opener(),
            live_captures: Arc::new(AtomicUsize::new(0)),
        })
    }

    /// Backward-compat constructor that wraps a `WhisperContext` in a `WhisperEngine`.
    pub fn new_with_shared_context(
        model_path_str: &str,
        shared_context: Arc<whisper_rs::WhisperContext>,
    ) -> Result<Self> {
        use crate::engine_whisper::WhisperEngine;
        info!("[AlwaysListeningController] Creating controller with shared WhisperContext (legacy path)");
        let engine: Arc<dyn TranscriptionEngine> = Arc::new(WhisperEngine::new(shared_context));
        Self::new_with_engine(model_path_str, engine)
    }

    /// Create an uninitialized controller that can be managed by Tauri but will return errors for operations
    pub fn new_uninitialized(model_path_str: &str) -> Self {
        Self {
            engine: None,
            model_path: model_path_str.to_string(),
            is_active: false,
            state: AlwaysListeningState::Monitoring,
            audio_thread: None,
            sensitivity: DEFAULT_SENSITIVITY,
            wake_words: DEFAULT_WAKE_WORDS.iter().map(|s| s.to_string()).collect(),
            last_activity: Arc::new(Mutex::new(None)),
            capture_opener: default_capture_opener(),
            live_captures: Arc::new(AtomicUsize::new(0)),
        }
    }

    /// How many microphone streams this controller has open right now.
    ///
    /// Zero whenever always-listening is off. Anything else after a stop is
    /// the privacy bug this exists to catch: a hot mic nobody asked for.
    pub fn active_capture_handles(&self) -> usize {
        self.live_captures.load(Ordering::SeqCst)
    }

    pub fn start_always_listening<R: Runtime + 'static>(
        &mut self,
        app_handle: &AppHandle<R>,
    ) -> Result<()> {
        // "Already active" is decided in `spawn_worker`, which can tell a live
        // worker from one that has ended.

        // Check microphone permission
        info!("[AlwaysListeningController] Checking microphone permission before starting...");
        let permission_status = crate::mic_permissions::check_microphone_permission();
        match permission_status {
            crate::mic_permissions::MicrophonePermissionStatus::Granted => {
                info!("[AlwaysListeningController] Microphone permission is granted");
            }
            crate::mic_permissions::MicrophonePermissionStatus::Denied => {
                return Err(Error::MicrophonePermissionDenied);
            }
            crate::mic_permissions::MicrophonePermissionStatus::Undetermined => {
                // Permission will be requested when we try to use the microphone
                info!("[AlwaysListeningController] Microphone permission is undetermined, will be requested");
            }
            crate::mic_permissions::MicrophonePermissionStatus::NotApplicable => {
                // Non-macOS platform, continue
                info!("[AlwaysListeningController] Microphone permission check not applicable on this platform");
            }
        }

        self.spawn_worker(app_handle)
    }

    /// Start the worker that owns the microphone. Everything after the
    /// permission check, so a test can run it without asking macOS.
    fn spawn_worker<R: Runtime + 'static>(&mut self, app_handle: &AppHandle<R>) -> Result<()> {
        // A worker that already ended (its microphone would not open) still
        // looks active from here. Reap it, or "already active" would keep
        // every later start from doing anything.
        if self
            .audio_thread
            .as_ref()
            .is_some_and(|(handle, _)| handle.is_finished())
        {
            let _ = self.stop_always_listening();
        }
        if self.is_active {
            return Ok(());
        }

        info!("[AlwaysListeningController] Starting always listening mode...");

        // Validate engine BEFORE setting is_active to prevent stale state
        let engine = self
            .engine
            .as_ref()
            .ok_or_else(|| Error::InitializationError("STT engine not initialized".to_string()))?
            .clone();

        // Emit always listening started event
        app_handle
            .emit(crate::constants::always_listening::STARTED, ())
            .map_err(|e| Error::Tauri(e.to_string()))?;

        let (control_tx, control_rx) = channel::<AlwaysListeningMessage>();
        let app_handle_for_thread = app_handle.clone();
        let sensitivity = self.sensitivity;
        let wake_words = self.wake_words.clone();
        let last_activity_arc = Arc::clone(&self.last_activity);
        let capture_opener = Arc::clone(&self.capture_opener);
        let live_captures = Arc::clone(&self.live_captures);

        let audio_thread_handle = thread::spawn(move || {
            Self::always_listening_worker(
                engine,
                app_handle_for_thread,
                control_rx,
                sensitivity,
                wake_words,
                last_activity_arc,
                capture_opener,
                live_captures,
            );
        });

        self.audio_thread = Some((audio_thread_handle, control_tx));
        // Set is_active AFTER all fallible operations succeed — prevents stale state
        self.is_active = true;
        self.state = AlwaysListeningState::Monitoring;

        info!("[AlwaysListeningController] Always listening mode started");
        Ok(())
    }

    #[allow(clippy::too_many_arguments)]
    fn always_listening_worker<R: Runtime + 'static>(
        engine: Arc<dyn TranscriptionEngine>,
        app_handle: AppHandle<R>,
        control_rx: std::sync::mpsc::Receiver<AlwaysListeningMessage>,
        mut sensitivity: f32,
        mut wake_words: Vec<String>,
        last_activity: Arc<Mutex<Option<Instant>>>,
        capture_opener: CaptureOpener,
        live_captures: Arc<AtomicUsize>,
    ) {
        info!(
            "[AlwaysListening] Worker thread started. Engine: '{}'",
            engine.name()
        );

        let mut session = match engine.create_session() {
            Ok(s) => s,
            Err(e) => {
                capture_failure::report(
                    &app_handle,
                    &CaptureStartFailure::EngineUnavailable {
                        detail: e.to_string(),
                    },
                );
                return;
            }
        };

        let (audio_data_tx, audio_data_rx) = channel::<Vec<f32>>();

        // Every way the microphone can refuse to open is a named failure the
        // person is told about, not a log line and a dead thread. Keep it that
        // way: the one `return` below is the only exit, and it reports first.
        let opened = match capture_opener(audio_data_tx) {
            Ok(opened) => opened,
            Err(failure) => {
                capture_failure::report(&app_handle, &failure);
                return;
            }
        };
        let OpenCapture {
            keepalive,
            sample_rate,
            device_name,
            substituted_for,
        } = opened;
        // Held for as long as the loop runs. Dropping it stops the microphone,
        // and it drops on every way out of this function, which is the only
        // place the always-listening stream lives.
        let _capture = CaptureGuard::new(keepalive, live_captures);

        if let Some(requested) = &substituted_for {
            let _ = app_handle.emit(
                crate::constants::voice_capture::DEVICE_SUBSTITUTED,
                serde_json::json!({ "requested": requested, "used": &device_name }),
            );
        }

        info!("[AlwaysListening] Audio monitoring started on {device_name}");

        // Resampling will be done on-demand with custom resamplers

        let mut audio_buffer: Vec<f32> = Vec::new();
        let mut current_state = AlwaysListeningState::Monitoring;
        let buffer_capacity = (sample_rate as u64 * INTENT_DETECTION_BUFFER_MS / 1000) as usize;
        let min_wake_word_samples = (sample_rate as u64 * MIN_WAKE_WORD_AUDIO_MS / 1000) as usize;
        let wake_word_preroll_samples = (sample_rate as u64 * WAKE_WORD_PREROLL_MS / 1000) as usize;
        let mut audio_activity_start: Option<Instant> = None;
        let mut last_volume_drop: Option<Instant> = None;
        // Waveform level emission is opt-in: this loop runs for as long as
        // always-listening is enabled, and an idle app should not pay for
        // event traffic nobody draws. The flag flips with the mic session
        // via SetAudioLevelMonitoring (LAC-4080).
        let mut audio_level_monitoring = false;
        let level_emit_interval = Duration::from_millis(70);
        let mut last_level_emit = Instant::now() - level_emit_interval;

        loop {
            // Check for control messages
            match control_rx.try_recv() {
                Ok(AlwaysListeningMessage::Stop) => {
                    info!("[AlwaysListening] Stop message received");
                    break;
                }
                Ok(AlwaysListeningMessage::UpdateSensitivity(new_sensitivity)) => {
                    sensitivity = new_sensitivity;
                    debug!("[AlwaysListening] Sensitivity updated to: {}", sensitivity);
                }
                Ok(AlwaysListeningMessage::UpdateWakeWords(new_wake_words)) => {
                    wake_words = new_wake_words;
                    debug!("[AlwaysListening] Wake words updated");
                }
                Ok(AlwaysListeningMessage::SetTranscriptionDebugging(enabled)) => {
                    info!(
                        "[AlwaysListening] Transcription debugging set to: {}",
                        enabled
                    );
                }
                Ok(AlwaysListeningMessage::SetAudioLevelMonitoring(enabled)) => {
                    info!(
                        "[AlwaysListening] Audio level monitoring set to: {}",
                        enabled
                    );
                    audio_level_monitoring = enabled;
                    if !enabled {
                        // Park every waveform at baseline when the session ends.
                        let _ = app_handle.emit(
                            crate::constants::voice_transcription::AUDIO_LEVEL,
                            serde_json::json!({ "level": 0.0_f32 }),
                        );
                    }
                }
                Ok(AlwaysListeningMessage::ForceTranscriptionTest) => {
                    info!("[AlwaysListening] Force transcription test requested");
                }
                Ok(AlwaysListeningMessage::ReturnToWakeWordMode) => {
                    info!("[AlwaysListening] Returning to wake word mode");
                    current_state = AlwaysListeningState::WaitingForWakeWord;
                }
                Err(TryRecvError::Empty) => {}
                Err(TryRecvError::Disconnected) => {
                    info!("[AlwaysListening] Control channel disconnected");
                    break;
                }
            }

            // Process audio data
            match audio_data_rx.recv_timeout(Duration::from_millis(100)) {
                Ok(audio_chunk) => {
                    // Echo guard: while Juno is speaking (TTS playing), throw away
                    // everything the mic hears so she never wakes on her own voice.
                    // Clearing the rolling buffer and resetting the activity trackers
                    // means nothing captured mid-speech replays the instant she stops.
                    if crate::is_capture_suppressed() {
                        if !audio_buffer.is_empty() {
                            audio_buffer.clear();
                        }
                        audio_activity_start = None;
                        last_volume_drop = None;
                        continue;
                    }
                    audio_buffer.extend_from_slice(&audio_chunk);

                    // Calculate volume level
                    let volume = Self::calculate_rms_volume(&audio_chunk);

                    // Emit audio level for waveform visualization while a mic
                    // session has monitoring on, throttled like the dictation
                    // thread, on the same meter curve.
                    if audio_level_monitoring && last_level_emit.elapsed() >= level_emit_interval {
                        let level = crate::controller::meter_level(volume);
                        let _ = app_handle.emit(
                            crate::constants::voice_transcription::AUDIO_LEVEL,
                            serde_json::json!({ "level": level }),
                        );
                        last_level_emit = Instant::now();
                    }

                    // Log audio chunk reception occasionally for debugging
                    thread_local! {
                        static LAST_CHUNK_LOG: std::cell::RefCell<Option<Instant>> = const { std::cell::RefCell::new(None) };
                    }

                    LAST_CHUNK_LOG.with(|last_log| {
                        let mut last_log = last_log.borrow_mut();
                        if last_log.is_none_or(|last| last.elapsed().as_secs() > 10) {
                            info!(
                                "[AlwaysListening] Audio chunk: {} samples, RMS volume: {:.6}",
                                audio_chunk.len(),
                                volume
                            );
                            *last_log = Some(Instant::now());
                        }
                    });

                    match current_state {
                        AlwaysListeningState::Monitoring => {
                            // Check for intent to activate
                            // Divide by sensitivity so higher sensitivity = lower threshold = triggers more easily
                            let volume_threshold = VOLUME_THRESHOLD / sensitivity;

                            // Two different things ask for a wake word check.
                            // Speech that has been running a while may have one
                            // buried in it, and speech that has just stopped may
                            // have been nothing but one. The second case is the
                            // one that was missing: a word as short as "juno" is
                            // over before any running-speech deadline arrives,
                            // so waiting for the deadline meant never looking.
                            let mut check_for_wake_word = false;

                            if volume > volume_threshold {
                                // Mark the start of audio activity if not already tracking
                                if audio_activity_start.is_none() {
                                    audio_activity_start = Some(Instant::now());
                                    info!("[AlwaysListening] Audio activity started - volume: {:.6} > {:.6}", volume, volume_threshold);
                                }

                                // Keep a rolling buffer for intent detection
                                if audio_buffer.len() > buffer_capacity {
                                    audio_buffer.drain(0..audio_buffer.len() - buffer_capacity);
                                }

                                if let Some(start_time) = audio_activity_start {
                                    if start_time.elapsed().as_millis()
                                        >= WAKE_WORD_CHECK_INTERVAL_MS as u128
                                    {
                                        check_for_wake_word = true;
                                    }
                                }
                            } else {
                                // Volume below threshold - use hysteresis for ending activity
                                let end_threshold = VOLUME_THRESHOLD_END / sensitivity;

                                if audio_activity_start.is_some() && volume < end_threshold {
                                    // Check if we should tolerate brief volume drops
                                    if last_volume_drop.is_none() {
                                        last_volume_drop = Some(Instant::now());
                                        debug!("[AlwaysListening] Volume drop detected, starting tolerance timer - volume: {:.6} < {:.6}", volume, end_threshold);
                                    } else if let Some(drop_time) = last_volume_drop {
                                        if drop_time.elapsed().as_millis()
                                            > VOLUME_DROP_TOLERANCE_MS as u128
                                        {
                                            info!("[AlwaysListening] Audio activity ended after tolerance period - volume: {:.6} < {:.6}", volume, end_threshold);
                                            // Look at what was just said before
                                            // letting go of it. This is the whole
                                            // of a quickly spoken wake word.
                                            check_for_wake_word = true;
                                            audio_activity_start = None;
                                            last_volume_drop = None;
                                        }
                                    }
                                } else if audio_activity_start.is_some() {
                                    // Volume above end threshold, reset drop tracking
                                    last_volume_drop = None;
                                }

                                // Log volume levels more frequently for debugging
                                thread_local! {
                                    static LAST_VOLUME_LOG: std::cell::RefCell<Option<Instant>> = const { std::cell::RefCell::new(None) };
                                }

                                LAST_VOLUME_LOG.with(|last_log| {
                                    let mut last_log = last_log.borrow_mut();
                                    if last_log.is_none_or(|last| last.elapsed().as_secs() > 5) { // Reduced frequency
                                        debug!("[AlwaysListening] Volume monitoring: {:.6} < {:.6} (start threshold, sensitivity: {:.1})", volume, volume_threshold, sensitivity);
                                        *last_log = Some(Instant::now());
                                    }
                                });

                                // Maintain rolling buffer during monitoring
                                if audio_buffer.len() > buffer_capacity {
                                    audio_buffer.drain(0..audio_buffer.len() - buffer_capacity);
                                }
                            }

                            if check_for_wake_word && audio_buffer.len() >= min_wake_word_samples {
                                // Check if the accumulated audio has sufficient volume for speech
                                let buffer_volume = Self::calculate_rms_volume(&audio_buffer);

                                if buffer_volume >= MIN_SPEECH_VOLUME {
                                    info!(
                                        "[AlwaysListening] Checking for a wake word: {} samples, volume: {:.6}",
                                        audio_buffer.len(),
                                        buffer_volume
                                    );

                                    // Check for wake words or speech
                                    if let Some(matched_phrase) = Self::detect_intent(
                                        session.as_mut(),
                                        &audio_buffer,
                                        sample_rate,
                                        &wake_words,
                                        &app_handle,
                                    ) {
                                        current_state = AlwaysListeningState::Activated;
                                        info!("[AlwaysListening] Intent detected (phrase: '{}') - activating transcription", matched_phrase);

                                        // Update last activity
                                        if let Ok(mut activity) = last_activity.lock() {
                                            *activity = Some(Instant::now());
                                        }

                                        // Emit activation event carrying the matched wake
                                        // phrase so the app can route to the right target.
                                        if let Err(e) = app_handle.emit(
                                            crate::constants::always_listening::ACTIVATED,
                                            matched_phrase.clone(),
                                        ) {
                                            error!("[AlwaysListening] Failed to emit activation event: {}", e);
                                        }

                                        // Start active transcription
                                        audio_buffer.clear();
                                        audio_activity_start = None; // Reset activity tracking
                                        last_volume_drop = None; // Reset drop tracking
                                    } else {
                                        // Nothing in it, but keep the tail: the
                                        // next check needs the run-up, both so a
                                        // word split across two checks is heard
                                        // whole and so Whisper has its second of
                                        // audio to work with.
                                        Self::keep_tail(
                                            &mut audio_buffer,
                                            wake_word_preroll_samples,
                                        );
                                        audio_activity_start = None; // Reset activity tracking
                                        last_volume_drop = None; // Reset drop tracking
                                    }
                                } else {
                                    debug!("[AlwaysListening] Audio accumulated but volume too low for speech: {:.6} < {:.6}",
                                           buffer_volume, MIN_SPEECH_VOLUME);
                                    // Too quiet to be speech, so there is nothing
                                    // in it worth keeping as run-up either.
                                    audio_buffer.clear();
                                    audio_activity_start = None;
                                    last_volume_drop = None;
                                }
                            }
                        }
                        AlwaysListeningState::Activated => {
                            // Actively transcribing - check for silence to return to monitoring
                            let end_threshold = VOLUME_THRESHOLD_END / sensitivity;

                            if volume < end_threshold {
                                // Lower threshold for ending activity
                                if let Ok(activity) = last_activity.lock() {
                                    if let Some(last_time) = *activity {
                                        if last_time.elapsed().as_millis()
                                            > SILENCE_TIMEOUT_MS as u128
                                        {
                                            current_state = AlwaysListeningState::Monitoring;
                                            info!("[AlwaysListening] Silence timeout - returning to monitoring (volume: {:.6} < {:.6})", volume, end_threshold);

                                            // Emit deactivation event
                                            if let Err(e) = app_handle.emit(
                                                crate::constants::always_listening::DEACTIVATED,
                                                (),
                                            ) {
                                                error!("[AlwaysListening] Failed to emit deactivation event: {}", e);
                                            }

                                            audio_buffer.clear();
                                            audio_activity_start = None;
                                            last_volume_drop = None;
                                            continue;
                                        }
                                    }
                                }
                            } else {
                                // Update activity timestamp
                                if let Ok(mut activity) = last_activity.lock() {
                                    *activity = Some(Instant::now());
                                }
                            }

                            // Process transcription for activated mode
                            if audio_buffer.len() >= buffer_capacity {
                                Self::process_active_transcription(
                                    session.as_mut(),
                                    &audio_buffer,
                                    sample_rate,
                                    &app_handle,
                                );
                                audio_buffer.clear();
                            }
                        }
                        AlwaysListeningState::Processing => {
                            // This state is currently unused but could be used for more complex processing
                            current_state = AlwaysListeningState::Monitoring;
                        }
                        AlwaysListeningState::WaitingForWakeWord => {
                            // Waiting for wake word after command completion
                            let end_threshold = VOLUME_THRESHOLD_END / sensitivity;

                            if volume < end_threshold {
                                // Lower threshold for ending activity
                                if let Ok(activity) = last_activity.lock() {
                                    if let Some(last_time) = *activity {
                                        if last_time.elapsed().as_millis()
                                            > COMMAND_COMPLETION_TIMEOUT_MS as u128
                                        {
                                            current_state = AlwaysListeningState::Monitoring;
                                            info!("[AlwaysListening] Command completion timeout - returning to monitoring (volume: {:.6} < {:.6})", volume, end_threshold);

                                            // Emit deactivation event
                                            if let Err(e) = app_handle.emit(
                                                crate::constants::always_listening::DEACTIVATED,
                                                (),
                                            ) {
                                                error!("[AlwaysListening] Failed to emit deactivation event: {}", e);
                                            }

                                            audio_buffer.clear();
                                            audio_activity_start = None;
                                            last_volume_drop = None;
                                            continue;
                                        }
                                    }
                                }
                            } else {
                                // Update activity timestamp
                                if let Ok(mut activity) = last_activity.lock() {
                                    *activity = Some(Instant::now());
                                }
                            }

                            // Process transcription for waiting mode
                            if audio_buffer.len() >= buffer_capacity {
                                Self::process_waiting_transcription(
                                    session.as_mut(),
                                    &audio_buffer,
                                    sample_rate,
                                    &app_handle,
                                );
                                audio_buffer.clear();
                            }
                        }
                    }
                }
                Err(_) => {
                    // Timeout - continue monitoring
                }
            }
        }

        info!("[AlwaysListening] Worker thread finished");
    }

    /// Drop everything but the last `keep` samples, so the next wake word check
    /// still has the run-up to whatever is said next.
    fn keep_tail(audio_buffer: &mut Vec<f32>, keep: usize) {
        if audio_buffer.len() > keep {
            audio_buffer.drain(0..audio_buffer.len() - keep);
        }
    }

    /// Put silence in front of audio that is shorter than Whisper will look at.
    ///
    /// whisper.cpp refuses anything under a second: it needs a hundred mel
    /// frames before it decodes, and returns no segments at all below that,
    /// which reads from the outside as "nothing was said". Leading silence
    /// costs nothing, since the decoder pads the clip out to thirty seconds
    /// either way, and it means a word that took half a second to say is still
    /// a word Whisper will read.
    fn pad_to_whisper_minimum(audio: Vec<f32>) -> Vec<f32> {
        let minimum = (WHISPER_SAMPLE_RATE as u64 * WHISPER_MIN_AUDIO_MS / 1000) as usize;
        if audio.len() >= minimum {
            return audio;
        }
        let mut padded = vec![0.0_f32; minimum - audio.len()];
        padded.extend_from_slice(&audio);
        padded
    }

    fn calculate_rms_volume(audio_chunk: &[f32]) -> f32 {
        if audio_chunk.is_empty() {
            return 0.0;
        }

        let sum_of_squares: f32 = audio_chunk.iter().map(|&sample| sample * sample).sum();
        (sum_of_squares / audio_chunk.len() as f32).sqrt()
    }

    fn detect_intent<R: Runtime>(
        session: &mut dyn TranscriptionSession,
        audio_buffer: &[f32],
        sample_rate: u32,
        wake_words: &[String],
        _app_handle: &AppHandle<R>,
    ) -> Option<String> {
        if audio_buffer.is_empty() {
            debug!("[AlwaysListening] detect_intent: Audio buffer is empty");
            return None;
        }

        let audio_duration_ms = (audio_buffer.len() as f32 / sample_rate as f32 * 1000.0) as u32;

        info!(
            "[AlwaysListening] detect_intent: Processing {} samples ({}ms) for {} wake words",
            audio_buffer.len(),
            audio_duration_ms,
            wake_words.len()
        );

        // There is no word in a clip this short, only a noise.
        if audio_duration_ms < MIN_WAKE_WORD_AUDIO_MS as u32 {
            info!("[AlwaysListening] detect_intent: Audio duration too short ({}ms < {}ms), skipping transcription",
                   audio_duration_ms, MIN_WAKE_WORD_AUDIO_MS);
            return None;
        }

        // Check audio quality - ensure it has sufficient volume for speech
        let avg_volume = Self::calculate_rms_volume(audio_buffer);
        if avg_volume < MIN_SPEECH_VOLUME {
            info!("[AlwaysListening] detect_intent: Audio volume too low for speech ({:.6} < {:.6}), skipping transcription",
                   avg_volume, MIN_SPEECH_VOLUME);
            return None;
        }

        // Resample if necessary
        info!(
            "[AlwaysListening] Sample rate check: {} -> {} (needs resampling: {})",
            sample_rate,
            WHISPER_SAMPLE_RATE,
            sample_rate != WHISPER_SAMPLE_RATE
        );
        let audio_to_process = if sample_rate != WHISPER_SAMPLE_RATE {
            // Create a custom resampler for this specific buffer size
            let config = sinc_resampling_params();

            match SincFixedIn::new(
                WHISPER_SAMPLE_RATE as f64 / sample_rate as f64,
                2.0,
                config,
                audio_buffer.len(), // Use exact buffer size as chunk size
                1,
            ) {
                Ok(mut custom_resampler) => {
                    match custom_resampler.process(&[audio_buffer.to_vec()], None) {
                        Ok(mut resampled) if !resampled.is_empty() => {
                            info!(
                                "[AlwaysListening] Audio resampled: {} -> {} samples",
                                audio_buffer.len(),
                                resampled[0].len()
                            );
                            resampled.remove(0)
                        }
                        Ok(_) => {
                            warn!("[AlwaysListening] Resampling produced empty output");
                            return None;
                        }
                        Err(e) => {
                            warn!("[AlwaysListening] Resampling failed: {:?}", e);
                            return None;
                        }
                    }
                }
                Err(e) => {
                    warn!(
                        "[AlwaysListening] Failed to create custom resampler: {:?}",
                        e
                    );
                    return None;
                }
            }
        } else {
            audio_buffer.to_vec()
        };

        let resampled_duration_ms =
            (audio_to_process.len() as f32 / WHISPER_SAMPLE_RATE as f32 * 1000.0) as u32;

        // Volume is measured before padding, because padding is silence and
        // would drag the average down towards the floor we are testing against.
        let resampled_volume = Self::calculate_rms_volume(&audio_to_process);
        if resampled_volume < MIN_SPEECH_VOLUME * 0.5 {
            // Allow slightly lower volume after resampling
            info!("[AlwaysListening] detect_intent: Resampled audio volume too low ({:.6} < {:.6}), skipping transcription",
                   resampled_volume, MIN_SPEECH_VOLUME * 0.5);
            return None;
        }

        let audio_to_process = Self::pad_to_whisper_minimum(audio_to_process);

        info!("[AlwaysListening] Running transcription for wake word detection ({}ms of speech, volume: {:.6})",
               resampled_duration_ms, resampled_volume);

        match session.transcribe_partial(&audio_to_process) {
            Ok(Some(transcribed_text)) => {
                let text_lower = transcribed_text.trim().to_lowercase();
                info!(
                    "[AlwaysListening] Transcription result: '{}' (length: {})",
                    text_lower,
                    text_lower.len()
                );

                // Check for wake words. Return the matched phrase (lowercased)
                // so the app can route this activation to the right target
                // (e.g. "juno" -> agent, "transcribe" -> dictation).
                //
                // The comparison is on how the words sound, not on how Whisper
                // spelled them: it writes down its best guess at a name it does
                // not know, so "juno" comes back as "Juneau." and a literal
                // comparison misses a person who said exactly the right word.
                if let Some(matched) = crate::wake_word::match_wake_phrase(&text_lower, wake_words)
                {
                    info!(
                        "[AlwaysListening] ✅ WAKE WORD DETECTED: '{}' heard in '{}'",
                        matched, text_lower
                    );
                    return Some(matched);
                }

                // No wake words configured — any speech activates (empty match).
                if wake_words.is_empty() {
                    info!("[AlwaysListening] Speech activity detected (no wake words configured): '{}'", text_lower);
                    return Some(String::new());
                }

                info!(
                    "[AlwaysListening] ❌ No wake words detected in: '{}'",
                    text_lower
                );
                None
            }
            Ok(None) => {
                warn!("[AlwaysListening] Empty transcription despite audio presence — check model and audio format");
                None
            }
            Err(e) => {
                error!("[AlwaysListening] Transcription failed: {}", e);
                None
            }
        }
    }

    fn process_active_transcription<R: Runtime>(
        session: &mut dyn TranscriptionSession,
        audio_buffer: &[f32],
        sample_rate: u32,
        app_handle: &AppHandle<R>,
    ) {
        if audio_buffer.is_empty() {
            return;
        }

        let audio_to_transcribe = if sample_rate != WHISPER_SAMPLE_RATE {
            let config = sinc_resampling_params();
            match SincFixedIn::new(
                WHISPER_SAMPLE_RATE as f64 / sample_rate as f64,
                2.0,
                config,
                audio_buffer.len(),
                1,
            ) {
                Ok(mut custom_resampler) => {
                    match custom_resampler.process(&[audio_buffer.to_vec()], None) {
                        Ok(mut resampled) if !resampled.is_empty() => resampled.remove(0),
                        _ => return,
                    }
                }
                Err(_) => return,
            }
        } else {
            audio_buffer.to_vec()
        };

        match session.transcribe_partial(&audio_to_transcribe) {
            Ok(Some(transcribed_text)) => {
                let cleaned_text = filter_transcription_text(transcribed_text.trim());
                if !cleaned_text.is_empty() {
                    info!(
                        "[AlwaysListening] Active transcription result: '{}'",
                        cleaned_text
                    );

                    if Self::should_process_with_agent(&cleaned_text) {
                        if Self::contains_stop_words(&cleaned_text) {
                            info!("[AlwaysListening] Stop word detected: '{}' - stopping always listening", cleaned_text);
                            if let Err(e) = app_handle.emit(
                                crate::constants::always_listening::STOP_REQUESTED,
                                serde_json::json!({ "reason": "stop_word", "text": cleaned_text }),
                            ) {
                                error!(
                                    "[AlwaysListening] Failed to emit stop-requested event: {}",
                                    e
                                );
                            }
                            return;
                        }

                        if Self::is_agent_call_allowed() {
                            Self::record_agent_call();
                            if let Err(e) = app_handle.emit(
                                crate::constants::always_listening::TRANSCRIPTION,
                                serde_json::json!({ "text": cleaned_text }),
                            ) {
                                error!(
                                    "[AlwaysListening] Failed to emit transcription event: {}",
                                    e
                                );
                            }
                            if let Err(e) = app_handle
                                .emit(crate::constants::always_listening::COMMAND_PROCESSED, ())
                            {
                                error!(
                                    "[AlwaysListening] Failed to emit command-processed event: {}",
                                    e
                                );
                            }
                        } else {
                            warn!(
                                "[AlwaysListening] Agent call rate limit exceeded, skipping: '{}'",
                                cleaned_text
                            );
                        }
                    } else {
                        info!(
                            "[AlwaysListening] Content filtered out (noise/meaningless): '{}'",
                            cleaned_text
                        );
                    }
                } else {
                    debug!("[AlwaysListening] Empty transcription result, continuing monitoring");
                }
            }
            Ok(None) => {
                debug!("[AlwaysListening] Empty transcription result, continuing monitoring");
            }
            Err(e) => {
                debug!("[AlwaysListening] Active transcription failed: {}", e);
            }
        }
    }

    // Intelligent content filtering functions
    fn should_process_with_agent(text: &str) -> bool {
        let text_lower = text.to_lowercase();
        let mut text_trimmed = text_lower.trim().to_string();

        // Filter out empty or very short content
        if text_trimmed.len() < MIN_MEANINGFUL_CONTENT_LENGTH {
            info!(
                "[AlwaysListening] Content too short: '{}' (length: {})",
                text_trimmed,
                text_trimmed.len()
            );
            return false;
        }

        // Remove phrase-level noise markers (e.g. [BLANK_AUDIO], [SILENCE]) but don't reject the
        // entire text — this allows "Open Spotify [BLANK_AUDIO]" to become "Open Spotify"
        let filtered = filter_transcription_text(&text_trimmed);
        if filtered != text_trimmed {
            info!(
                "[AlwaysListening] Removed noise markers from text, remaining: '{}'",
                filtered
            );
            text_trimmed = filtered;
        }

        // Re-check length after noise pattern removal
        if text_trimmed.len() < MIN_MEANINGFUL_CONTENT_LENGTH {
            info!(
                "[AlwaysListening] Content too short after noise removal: '{}' (length: {})",
                text_trimmed,
                text_trimmed.len()
            );
            return false;
        }

        // Split into words for word-level analysis
        let words: Vec<&str> = text_trimmed.split_whitespace().collect();

        // Filter out if text consists entirely of noise words
        let noise_word_count = words
            .iter()
            .filter(|word| NOISE_WORDS.contains(word))
            .count();

        if noise_word_count == words.len() && !words.is_empty() {
            info!(
                "[AlwaysListening] Text consists entirely of noise words: '{}'",
                text_trimmed
            );
            return false;
        }

        // Filter out repeated single characters (like "a a a a")
        if words.len() > 2 {
            let unique_words: std::collections::HashSet<&str> = words.iter().cloned().collect();
            if unique_words.len() == 1 && words[0].len() == 1 {
                info!(
                    "[AlwaysListening] Repetitive single character detected: '{}'",
                    text_trimmed
                );
                return false;
            }
        }

        // Filter out content that is mostly punctuation or numbers
        let letter_count = text_trimmed.chars().filter(|c| c.is_alphabetic()).count();
        let total_meaningful_chars = text_trimmed
            .chars()
            .filter(|c| c.is_alphanumeric() || c.is_whitespace())
            .count();

        if total_meaningful_chars > 0 && (letter_count as f32 / total_meaningful_chars as f32) < 0.3
        {
            info!(
                "[AlwaysListening] Content mostly non-alphabetic: '{}'",
                text_trimmed
            );
            return false;
        }

        // Check for minimum meaningful word count (exclude noise words from meaningful words)
        let meaningful_words: Vec<&str> = words
            .iter()
            .filter(|word| word.len() > 2 && !NOISE_WORDS.contains(word))
            .cloned()
            .collect();

        if meaningful_words.is_empty() {
            info!(
                "[AlwaysListening] No meaningful words found: '{}'",
                text_trimmed
            );
            return false;
        }

        info!(
            "[AlwaysListening] Content passed filtering: '{}' (meaningful words: {})",
            text_trimmed,
            meaningful_words.len()
        );
        true
    }

    fn contains_stop_words(text: &str) -> bool {
        let text_lower = text.to_lowercase();
        let words: Vec<&str> = text_lower.split_whitespace().collect();
        for stop_word in STOP_WORDS {
            // Multi-word stop phrases: check if text contains the exact phrase
            if stop_word.contains(' ') {
                if text_lower.contains(stop_word) {
                    return true;
                }
            } else {
                // Single-word stop words: match whole words only to avoid
                // false positives like "end" matching "pending" or "friend"
                if words.contains(stop_word) {
                    return true;
                }
            }
        }
        false
    }

    fn is_agent_call_allowed() -> bool {
        use std::sync::{Mutex, OnceLock};

        // Thread-safe replacement for static mut using OnceLock and Mutex
        static CALL_TIMES: OnceLock<Mutex<Vec<std::time::SystemTime>>> = OnceLock::new();

        let call_times = CALL_TIMES.get_or_init(|| Mutex::new(Vec::new()));

        match call_times.lock() {
            Ok(mut times) => {
                let now = std::time::SystemTime::now();
                let one_minute_ago = now - std::time::Duration::from_secs(60);

                // Remove calls older than 1 minute
                times.retain(|&time| time > one_minute_ago);

                // Check if we're under the limit
                if times.len() < MAX_AGENT_CALLS_PER_MINUTE as usize {
                    times.push(now);
                    true
                } else {
                    false
                }
            }
            Err(e) => {
                error!("Failed to acquire agent call rate limiting lock: {}", e);
                false // Safe fallback - deny the call
            }
        }
    }

    fn record_agent_call() {
        // Update command processing count and last meaningful command timestamp
        // This is a simplified implementation
        info!("[AlwaysListening] Agent call recorded");
    }

    fn process_waiting_transcription<R: Runtime>(
        session: &mut dyn TranscriptionSession,
        audio_buffer: &[f32],
        sample_rate: u32,
        app_handle: &AppHandle<R>,
    ) {
        if audio_buffer.is_empty() {
            return;
        }

        let audio_to_transcribe = if sample_rate != WHISPER_SAMPLE_RATE {
            let config = sinc_resampling_params();
            match SincFixedIn::new(
                WHISPER_SAMPLE_RATE as f64 / sample_rate as f64,
                2.0,
                config,
                audio_buffer.len(),
                1,
            ) {
                Ok(mut custom_resampler) => {
                    match custom_resampler.process(&[audio_buffer.to_vec()], None) {
                        Ok(mut resampled) if !resampled.is_empty() => resampled.remove(0),
                        _ => return,
                    }
                }
                Err(_) => return,
            }
        } else {
            audio_buffer.to_vec()
        };

        match session.transcribe_partial(&audio_to_transcribe) {
            Ok(Some(transcribed_text)) => {
                let cleaned_text = filter_transcription_text(transcribed_text.trim());
                if !cleaned_text.is_empty() {
                    info!(
                        "[AlwaysListening] Waiting transcription result: '{}'",
                        cleaned_text
                    );

                    if Self::should_process_with_agent(&cleaned_text) {
                        if Self::contains_stop_words(&cleaned_text) {
                            info!("[AlwaysListening] Stop word detected in waiting mode: '{}' - stopping", cleaned_text);
                            if let Err(e) = app_handle.emit(
                                crate::constants::always_listening::STOP_REQUESTED,
                                serde_json::json!({ "reason": "stop_word", "text": cleaned_text }),
                            ) {
                                error!(
                                    "[AlwaysListening] Failed to emit stop-requested event: {}",
                                    e
                                );
                            }
                            return;
                        }

                        if Self::is_agent_call_allowed() {
                            Self::record_agent_call();
                            if let Err(e) = app_handle.emit(
                                crate::constants::always_listening::TRANSCRIPTION,
                                serde_json::json!({ "text": cleaned_text }),
                            ) {
                                error!(
                                    "[AlwaysListening] Failed to emit transcription event: {}",
                                    e
                                );
                            }
                        } else {
                            warn!("[AlwaysListening] Agent call rate limit exceeded in waiting mode, skipping: '{}'", cleaned_text);
                        }
                    } else {
                        info!("[AlwaysListening] Content filtered out in waiting mode (noise/meaningless): '{}'", cleaned_text);
                    }
                }
            }
            Ok(None) => {
                debug!("[AlwaysListening] Empty result in waiting transcription");
            }
            Err(e) => {
                debug!("[AlwaysListening] Waiting transcription failed: {}", e);
            }
        }
    }

    /// The one stop path for the always-listening microphone.
    ///
    /// Keyed on the worker, not on `is_active`: if a worker exists, it is told
    /// to stop and joined, and joining is what guarantees its stream has been
    /// dropped by the time this returns. Returns whether anything was running.
    pub fn stop_always_listening(&mut self) -> Result<bool> {
        let was_running = self.audio_thread.is_some();
        if !was_running {
            self.is_active = false;
            return Ok(false);
        }

        info!("[AlwaysListeningController] Stopping always listening mode...");

        if let Some((thread_handle, control_tx)) = self.audio_thread.take() {
            // Send stop message
            if let Err(e) = control_tx.send(AlwaysListeningMessage::Stop) {
                warn!(
                    "[AlwaysListeningController] Failed to send stop message: {:?}",
                    e
                );
            }

            // Wait for thread to finish with timeout
            match thread_handle.join() {
                Ok(_) => info!("[AlwaysListeningController] Audio thread finished cleanly"),
                Err(e) => error!(
                    "[AlwaysListeningController] Audio thread join error: {:?}",
                    e
                ),
            }
        }

        self.is_active = false;
        self.state = AlwaysListeningState::Monitoring;

        info!("[AlwaysListeningController] Always listening mode stopped");
        Ok(was_running)
    }

    pub fn is_active(&self) -> bool {
        self.is_active
    }

    pub fn get_state(&self) -> AlwaysListeningState {
        self.state.clone()
    }

    pub fn set_sensitivity(&mut self, sensitivity: f32) -> Result<()> {
        self.sensitivity = sensitivity.clamp(MIN_SENSITIVITY, MAX_SENSITIVITY);

        if let Some((_, control_tx)) = &self.audio_thread {
            control_tx
                .send(AlwaysListeningMessage::UpdateSensitivity(self.sensitivity))
                .map_err(|e| {
                    Error::ControlError(format!("Failed to update sensitivity: {:?}", e))
                })?;
        }

        Ok(())
    }

    pub fn get_sensitivity(&self) -> f32 {
        self.sensitivity
    }

    pub fn set_wake_words(&mut self, wake_words: Vec<String>) -> Result<()> {
        self.wake_words = wake_words.clone();

        if let Some((_, control_tx)) = &self.audio_thread {
            control_tx
                .send(AlwaysListeningMessage::UpdateWakeWords(wake_words))
                .map_err(|e| {
                    Error::ControlError(format!("Failed to update wake words: {:?}", e))
                })?;
        }

        Ok(())
    }

    pub fn get_wake_words(&self) -> Vec<String> {
        self.wake_words.clone()
    }

    /// Update the shared Whisper context and model path for this controller.
    /// Kept for backward-compat with `set_model_path` command — wraps the context in a `WhisperEngine`.
    pub fn update_shared_context(
        &mut self,
        model_path: &str,
        shared_context: Arc<whisper_rs::WhisperContext>,
    ) -> Result<()> {
        info!(
            "[AlwaysListeningController] Updating shared Whisper context with new model: {}",
            model_path
        );

        let was_active = self.is_active;
        if was_active {
            info!("[AlwaysListeningController] Stopping controller before model update");
            self.stop_always_listening()?;
        }

        use crate::engine_whisper::WhisperEngine;
        let engine: Arc<dyn TranscriptionEngine> = Arc::new(WhisperEngine::new(shared_context));
        self.model_path = model_path.to_string();
        self.engine = Some(engine);

        info!("[AlwaysListeningController] Successfully updated shared Whisper context (no model reload needed)");

        if was_active {
            info!("[AlwaysListeningController] Controller was active before update - caller should restart if needed");
        }

        Ok(())
    }

    /// Swap the active STT engine without restarting the controller.
    pub fn update_engine(&mut self, engine: Arc<dyn TranscriptionEngine>) -> Result<()> {
        info!(
            "[AlwaysListeningController] Updating engine to '{}'",
            engine.name()
        );
        let was_active = self.is_active;
        if was_active {
            self.stop_always_listening()?;
        }
        self.engine = Some(engine);
        if was_active {
            info!("[AlwaysListeningController] Stopped before engine swap - caller should restart if needed");
        }
        Ok(())
    }

    // Enhanced Debugging Methods

    pub fn set_transcription_debugging<R: Runtime>(
        &mut self,
        enabled: bool,
        app_handle: &AppHandle<R>,
    ) -> Result<()> {
        info!(
            "[AlwaysListeningController] Setting transcription debugging to: {}",
            enabled
        );

        if let Some((_, control_tx)) = &self.audio_thread {
            control_tx
                .send(AlwaysListeningMessage::SetTranscriptionDebugging(enabled))
                .map_err(|e| {
                    Error::ControlError(format!("Failed to set transcription debugging: {:?}", e))
                })?;
        }

        if enabled {
            // Emit an event to confirm debugging is enabled
            app_handle
                .emit(
                    crate::constants::always_listening::EVENT,
                    serde_json::json!({
                        "type": "transcription_debug",
                        "payload": { "enabled": true }
                    }),
                )
                .map_err(|e| {
                    Error::EventError(format!("Failed to emit debugging enabled event: {}", e))
                })?;
        }

        Ok(())
    }

    pub fn set_audio_level_monitoring<R: Runtime>(
        &mut self,
        enabled: bool,
        app_handle: &AppHandle<R>,
    ) -> Result<()> {
        info!(
            "[AlwaysListeningController] Setting audio level monitoring to: {}",
            enabled
        );

        if let Some((_, control_tx)) = &self.audio_thread {
            control_tx
                .send(AlwaysListeningMessage::SetAudioLevelMonitoring(enabled))
                .map_err(|e| {
                    Error::ControlError(format!("Failed to set audio level monitoring: {:?}", e))
                })?;
        }

        if enabled {
            // Emit an event to confirm monitoring is enabled
            app_handle
                .emit(
                    crate::constants::always_listening::EVENT,
                    serde_json::json!({
                        "type": "audio_level",
                        "payload": { "enabled": true }
                    }),
                )
                .map_err(|e| {
                    Error::EventError(format!("Failed to emit monitoring enabled event: {}", e))
                })?;
        }

        Ok(())
    }

    pub fn test_whisper_model(&self) -> Result<serde_json::Value> {
        info!(
            "[AlwaysListening] Testing STT engine at path: {}",
            self.model_path
        );

        let engine = self
            .engine
            .as_ref()
            .ok_or_else(|| Error::Whisper("STT engine not available".to_string()))?;

        let sample_rate = WHISPER_SAMPLE_RATE;
        let duration_samples = sample_rate as usize;
        let frequency = 440.0_f32;
        let mut test_audio: Vec<f32> = Vec::with_capacity(duration_samples);
        for i in 0..duration_samples {
            let t = i as f32 / sample_rate as f32;
            test_audio.push((2.0 * std::f32::consts::PI * frequency * t).sin() * 0.1);
        }

        let test_volume = Self::calculate_rms_volume(&test_audio);

        let transcription_result = match engine.create_session() {
            Ok(mut session) => match session.transcribe_partial(&test_audio) {
                Ok(Some(text)) => format!("SUCCESS: text: '{}'", text.trim()),
                Ok(None) => "SUCCESS: (no speech detected in tone)".to_string(),
                Err(e) => format!("FAILED: {}", e),
            },
            Err(e) => format!("FAILED to create session: {}", e),
        };

        let test_result = serde_json::json!({
            "model_path": self.model_path,
            "model_exists": std::path::Path::new(&self.model_path).exists(),
            "engine_name": engine.name(),
            "engine_initialized": engine.is_initialized(),
            "test_audio_samples": test_audio.len(),
            "test_audio_duration_ms": (test_audio.len() as f32 / sample_rate as f32 * 1000.0) as u32,
            "test_audio_volume": test_volume,
            "volume_threshold": VOLUME_THRESHOLD,
            "min_speech_volume": MIN_SPEECH_VOLUME,
            "min_transcription_duration_ms": WHISPER_MIN_AUDIO_MS,
            "wake_word_check_interval_ms": WAKE_WORD_CHECK_INTERVAL_MS,
            "transcription_test": transcription_result,
            "wake_words": self.wake_words,
            "sensitivity": self.sensitivity,
            "status": "Model test completed"
        });

        info!(
            "[AlwaysListening] Model test result: {}",
            serde_json::to_string_pretty(&test_result).unwrap_or_default()
        );
        Ok(test_result)
    }

    pub fn force_transcription_test<R: Runtime>(
        &mut self,
        _app_handle: &AppHandle<R>,
    ) -> Result<serde_json::Value> {
        info!("[AlwaysListeningController] Starting force transcription test...");

        if let Some((_, control_tx)) = &self.audio_thread {
            control_tx
                .send(AlwaysListeningMessage::ForceTranscriptionTest)
                .map_err(|e| {
                    Error::ControlError(format!("Failed to send force transcription test: {:?}", e))
                })?;

            // Wait a moment for the test to process
            std::thread::sleep(std::time::Duration::from_millis(100));

            Ok(serde_json::json!({
                "status": "requested",
                "message": "Force transcription test requested. Check logs and events for results.",
                "test_type": "live_audio_capture"
            }))
        } else {
            Ok(serde_json::json!({
                "status": "error",
                "error": "Always listening is not active",
                "test_type": "live_audio_capture"
            }))
        }
    }

    pub fn force_threshold_test<R: Runtime>(
        &mut self,
        _app_handle: &AppHandle<R>,
    ) -> Result<serde_json::Value> {
        info!("[AlwaysListeningController] Starting force threshold test...");

        if let Some((_, control_tx)) = &self.audio_thread {
            // Temporarily set very low sensitivity for testing
            control_tx
                .send(AlwaysListeningMessage::UpdateSensitivity(0.1))
                .map_err(|e| {
                    Error::ControlError(format!("Failed to set test sensitivity: {:?}", e))
                })?;

            std::thread::sleep(std::time::Duration::from_millis(100));

            Ok(serde_json::json!({
                "status": "test_started",
                "message": "Force threshold test started with sensitivity 0.1. Speak now and check logs.",
                "test_type": "volume_threshold",
                "instructions": "This test sets extremely low threshold. Speak normally and check for 'Audio activity started' messages."
            }))
        } else {
            Ok(serde_json::json!({
                "status": "error",
                "error": "Always listening is not active",
                "test_type": "volume_threshold"
            }))
        }
    }

    #[allow(dead_code)] // Method for future audio input status checking
    pub(crate) async fn get_audio_input_status<R: Runtime>(
        &self,
        _app_handle: &AppHandle<R>,
    ) -> Result<serde_json::Value> {
        // Implementation of the method
        Ok(serde_json::json!({
            "status": "not_implemented",
            "error": "This method is not implemented"
        }))
    }

    // New method to return to wake word mode
    pub fn return_to_wake_word_mode(&mut self) -> Result<()> {
        if let Some((_, control_tx)) = &self.audio_thread {
            control_tx
                .send(AlwaysListeningMessage::ReturnToWakeWordMode)
                .map_err(|e| {
                    Error::ControlError(format!("Failed to return to wake word mode: {:?}", e))
                })?;
        }
        Ok(())
    }
}

impl Drop for AlwaysListeningController {
    fn drop(&mut self) {
        if self.audio_thread.is_some() {
            tracing::info!("[AlwaysListeningController] Drop: stopping active listening");
            let _ = self.stop_always_listening();
        }
    }
}

// SAFETY NOTE: AlwaysListeningController is always wrapped in Arc<Mutex<AlwaysListeningController>>
// at call sites (see lib.rs). The Mutex provides the necessary synchronization, so we only need Send.
// All fields are individually Send (Arc, Mutex, Option, Vec, bool, f32, String).
// We do NOT impl Sync because mutable fields (is_active, state, audio_thread, etc.) lack interior
// mutability — the wrapping Mutex handles thread safety.
unsafe impl Send for AlwaysListeningController {}

#[cfg(test)]
mod tests {
    use super::*;

    /// An engine that hears nothing, so the worker never transcribes.
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

    /// A stand-in microphone: a token in place of the cpal stream, and a
    /// chunk of silence so the worker loop has something to chew on.
    fn fake_microphone() -> CaptureOpener {
        Arc::new(
            |tx: Sender<Vec<f32>>| -> std::result::Result<OpenCapture, CaptureStartFailure> {
                let _ = tx.send(vec![0.0; 1600]);
                Ok(OpenCapture {
                    keepalive: Box::new(tx),
                    sample_rate: WHISPER_SAMPLE_RATE,
                    device_name: "Test Mic".to_string(),
                    substituted_for: None,
                })
            },
        )
    }

    fn controller_with_fake_mic() -> AlwaysListeningController {
        let mut c = AlwaysListeningController::new_with_engine("test", Arc::new(SilentEngine))
            .expect("controller");
        c.capture_opener = fake_microphone();
        c
    }

    fn wait_for_handles(c: &AlwaysListeningController, want: usize) {
        let deadline = Instant::now() + Duration::from_secs(5);
        while c.active_capture_handles() != want && Instant::now() < deadline {
            thread::sleep(Duration::from_millis(10));
        }
        assert_eq!(c.active_capture_handles(), want);
    }

    #[test]
    fn stopping_leaves_no_microphone_open() {
        let app = tauri::test::mock_app();
        let mut c = controller_with_fake_mic();

        c.spawn_worker(app.handle()).expect("start");
        wait_for_handles(&c, 1);

        assert!(c.stop_always_listening().expect("stop"));
        // Not eventually: the stop joins the worker, so the stream is gone by
        // the time it returns.
        assert_eq!(c.active_capture_handles(), 0);
        assert!(!c.is_active());
    }

    #[test]
    fn stopping_twice_is_harmless_and_restarting_opens_exactly_one() {
        let app = tauri::test::mock_app();
        let mut c = controller_with_fake_mic();

        c.spawn_worker(app.handle()).expect("start");
        // A second start while running must not open a second microphone.
        c.spawn_worker(app.handle()).expect("start again");
        wait_for_handles(&c, 1);

        c.stop_always_listening().expect("stop");
        assert!(!c.stop_always_listening().expect("stop again"));
        assert_eq!(c.active_capture_handles(), 0);

        c.spawn_worker(app.handle()).expect("restart");
        wait_for_handles(&c, 1);
        c.stop_always_listening().expect("stop");
        assert_eq!(c.active_capture_handles(), 0);
    }

    #[test]
    fn swapping_the_engine_releases_the_microphone() {
        let app = tauri::test::mock_app();
        let mut c = controller_with_fake_mic();

        c.spawn_worker(app.handle()).expect("start");
        wait_for_handles(&c, 1);

        c.update_engine(Arc::new(SilentEngine)).expect("swap");
        assert_eq!(c.active_capture_handles(), 0);
        assert!(!c.is_active());
    }

    #[test]
    fn a_worker_whose_microphone_never_opened_does_not_block_the_next_start() {
        let app = tauri::test::mock_app();
        let mut c = controller_with_fake_mic();
        c.capture_opener = Arc::new(
            |_tx: Sender<Vec<f32>>| -> std::result::Result<OpenCapture, CaptureStartFailure> {
                Err(CaptureStartFailure::NoInputDevice)
            },
        );

        c.spawn_worker(app.handle()).expect("start");
        let deadline = Instant::now() + Duration::from_secs(5);
        while !c
            .audio_thread
            .as_ref()
            .is_some_and(|(handle, _)| handle.is_finished())
            && Instant::now() < deadline
        {
            thread::sleep(Duration::from_millis(10));
        }
        assert_eq!(c.active_capture_handles(), 0);

        // The microphone is back. Starting again must open it rather than
        // trust a stale "already active".
        c.capture_opener = fake_microphone();
        c.spawn_worker(app.handle()).expect("restart");
        wait_for_handles(&c, 1);
        c.stop_always_listening().expect("stop");
        assert_eq!(c.active_capture_handles(), 0);
    }

    #[test]
    fn dropping_the_controller_releases_the_microphone() {
        let app = tauri::test::mock_app();
        let mut c = controller_with_fake_mic();
        let live = Arc::clone(&c.live_captures);

        c.spawn_worker(app.handle()).expect("start");
        wait_for_handles(&c, 1);

        drop(c);
        assert_eq!(live.load(Ordering::SeqCst), 0);
    }

    /// The wake engine's stop must stop the device, not just drop the
    /// handle: the fake keeps running when merely dropped, as cpal's stream
    /// does on macOS.
    #[test]
    fn stopping_stops_the_device_itself() {
        use crate::devices::fake_input;
        use std::sync::atomic::AtomicBool;

        let app = tauri::test::mock_app();
        let open = Arc::new(AtomicUsize::new(0));
        let running = Arc::new(AtomicBool::new(false));
        let mut c = controller_with_fake_mic();
        {
            let open = Arc::clone(&open);
            let running = Arc::clone(&running);
            c.capture_opener = Arc::new(
                move |tx: Sender<Vec<f32>>| -> std::result::Result<OpenCapture, CaptureStartFailure> {
                    let input = fake_input::start(&open, &running);
                    let _ = tx.send(vec![0.0; 1600]);
                    Ok(OpenCapture {
                        keepalive: Box::new((input, tx)),
                        sample_rate: WHISPER_SAMPLE_RATE,
                        device_name: "Test Mic".to_string(),
                        substituted_for: None,
                    })
                },
            );
        }

        c.spawn_worker(app.handle()).expect("start");
        wait_for_handles(&c, 1);
        assert_eq!(open.load(Ordering::SeqCst), 1);

        assert!(c.stop_always_listening().expect("stop"));
        assert_eq!(open.load(Ordering::SeqCst), 0, "no input stream left open");
        assert!(
            !running.load(Ordering::SeqCst),
            "the device was stopped, not just dropped"
        );
    }
}

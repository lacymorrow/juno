use whisper_rs::{FullParams, WhisperContext, WhisperContextParameters};
use std::path::Path;
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::SampleFormat;
use std::sync::mpsc::{channel, Sender, TryRecvError};
use std::thread;
use std::time::{Duration, Instant};
use std::sync::{Arc, Mutex};
use rubato::{Resampler, SincFixedIn, SincInterpolationType, SincInterpolationParameters, WindowFunction};
use tauri::{AppHandle, Emitter, Runtime};
use tracing::{info, warn, error, debug};
use serde_json;
use juno::constants::events;

use crate::error::{Error, Result};

// Audio processing constants (matching main crate's constants)
const WHISPER_SAMPLE_RATE: u32 = 16000;
const SINC_LENGTH: usize = 256;
const OVERSAMPLING_FACTOR: usize = 256;
const INTENT_DETECTION_BUFFER_MS: u64 = 3000; // Buffer for intent detection (increased from 1500)
const VOLUME_THRESHOLD: f32 = 0.01; // Increased from 0.003 to reduce false triggers
const VOLUME_THRESHOLD_END: f32 = 0.005; // Increased from 0.002
const SILENCE_TIMEOUT_MS: u64 = 3000; // Return to monitoring after silence
const MIN_TRANSCRIPTION_DURATION_MS: u64 = 1000; // Increased from 500ms to 1000ms for better speech capture
const VOLUME_DROP_TOLERANCE_MS: u64 = 200; // Allow brief volume drops during activity
const MIN_SPEECH_VOLUME: f32 = 0.02; // Minimum volume required for speech processing

// New constants for intelligent filtering
const MIN_MEANINGFUL_CONTENT_LENGTH: usize = 3; // Minimum characters for meaningful content
const MAX_AGENT_CALLS_PER_MINUTE: u32 = 5; // Rate limit for agent calls
const AUTO_STOP_TIMEOUT_MS: u64 = 30000; // Auto-stop always listening after 30 seconds of processing
const COMMAND_COMPLETION_TIMEOUT_MS: u64 = 5000; // Return to wake word after command completion

// Stop words that should end always listening mode
const STOP_WORDS: &[&str] = &[
    "stop", "nevermind", "never mind", "cancel", "quit", "exit",
    "done", "that's all", "thats all", "end", "finish", "enough"
];

// Noise patterns that should be ignored
const NOISE_PATTERNS: &[&str] = &[
    "[blank_audio]", "[BLANK_AUDIO]", "blank audio",
    "[music]", "[noise]", "[silence]",
];

// Single word noise patterns that should only match complete words
const NOISE_WORDS: &[&str] = &[
    "um", "uh", "hmm", "ah", "er", "mm", "mhm",
    // Single letter transcriptions are usually noise
    "a", "i", "o", "e", "u"
];

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
    Monitoring,   // Continuously monitoring for intent
    Activated,    // Intent detected, actively transcribing
    Processing,   // Processing detected speech
    WaitingForWakeWord, // Waiting for wake word after command completion
}

pub struct AlwaysListeningController {
    shared_whisper_context: Option<Arc<WhisperContext>>,
    model_path: String,
    is_active: bool,
    state: AlwaysListeningState,
    audio_thread: Option<(thread::JoinHandle<()>, Sender<AlwaysListeningMessage>)>,
    sensitivity: f32,
    wake_words: Vec<String>,
    last_activity: Arc<Mutex<Option<Instant>>>,
    // New fields for intelligent filtering
    #[allow(dead_code)] // Reserved for future intelligent filtering
    agent_call_timestamps: Arc<Mutex<Vec<Instant>>>,
    #[allow(dead_code)] // Reserved for future intelligent filtering
    last_meaningful_command: Arc<Mutex<Option<Instant>>>,
    #[allow(dead_code)] // Reserved for future intelligent filtering
    command_processing_count: Arc<Mutex<u32>>,
}

impl AlwaysListeningController {
    pub fn new(model_path_str: &str) -> Result<Self> {
        let model_path = Path::new(model_path_str);
        if !model_path.exists() {
            return Err(Error::ModelNotFound(model_path_str.to_string()));
        }

        // Load the Whisper model once at initialization
        let whisper_context = WhisperContext::new_with_params(model_path_str, WhisperContextParameters::default())
            .map_err(|e| Error::Whisper(format!("Failed to create WhisperContext: {:?}", e)))?;

        Ok(Self {
            shared_whisper_context: Some(Arc::new(whisper_context)),
            model_path: model_path_str.to_string(),
            is_active: false,
            state: AlwaysListeningState::Monitoring,
            audio_thread: None,
            sensitivity: 0.5,
            wake_words: vec!["hey juno".to_string(), "juno".to_string(), "joono".to_string(), "computer".to_string(), "hey computer".to_string()],
            last_activity: Arc::new(Mutex::new(None)),
            agent_call_timestamps: Arc::new(Mutex::new(Vec::new())),
            last_meaningful_command: Arc::new(Mutex::new(None)),
            command_processing_count: Arc::new(Mutex::new(0)),
        })
    }

    /// Create an AlwaysListeningController using a pre-loaded shared WhisperContext
    /// This eliminates duplicate model loading when multiple controllers need the same model
    pub fn new_with_shared_context(model_path_str: &str, shared_context: Arc<WhisperContext>) -> Result<Self> {
        info!("[AlwaysListeningController] Creating controller with shared Whisper context (no model reload)");

        Ok(Self {
            shared_whisper_context: Some(shared_context),
            model_path: model_path_str.to_string(),
            is_active: false,
            state: AlwaysListeningState::Monitoring,
            audio_thread: None,
            sensitivity: 0.5,
            wake_words: vec!["hey juno".to_string(), "juno".to_string(), "joono".to_string(), "computer".to_string(), "hey computer".to_string()],
            last_activity: Arc::new(Mutex::new(None)),
            agent_call_timestamps: Arc::new(Mutex::new(Vec::new())),
            last_meaningful_command: Arc::new(Mutex::new(None)),
            command_processing_count: Arc::new(Mutex::new(0)),
        })
    }

    pub fn start_always_listening<R: Runtime + 'static>(&mut self, app_handle: &AppHandle<R>) -> Result<()> {
        if self.is_active {
            return Ok(()); // Already active
        }

        info!("[AlwaysListeningController] Starting always listening mode...");

        // Emit always listening started event
        app_handle.emit("always-listening:started", ())
            .map_err(|e| Error::Tauri(e.to_string()))?;

        self.is_active = true;
        self.state = AlwaysListeningState::Monitoring;

        let (control_tx, control_rx) = channel::<AlwaysListeningMessage>();
        let shared_context = self.shared_whisper_context.as_ref().unwrap().clone();
        let app_handle_for_thread = app_handle.clone();
        let sensitivity = self.sensitivity;
        let wake_words = self.wake_words.clone();
        let last_activity_arc = Arc::clone(&self.last_activity);

        let audio_thread_handle = thread::spawn(move || {
            Self::always_listening_worker(
                shared_context,
                app_handle_for_thread,
                control_rx,
                sensitivity,
                wake_words,
                last_activity_arc,
            );
        });

        self.audio_thread = Some((audio_thread_handle, control_tx));

        info!("[AlwaysListeningController] Always listening mode started");
        Ok(())
    }

    fn always_listening_worker<R: Runtime + 'static>(
        shared_whisper_context: Arc<WhisperContext>,
        app_handle: AppHandle<R>,
        control_rx: std::sync::mpsc::Receiver<AlwaysListeningMessage>,
        mut sensitivity: f32,
        mut wake_words: Vec<String>,
        last_activity: Arc<Mutex<Option<Instant>>>,
    ) {
        info!("[AlwaysListening] Worker thread started with shared Whisper context (no model reload needed)");

        let mut whisper_state = match shared_whisper_context.create_state() {
            Ok(state) => state,
            Err(e) => {
                error!("Failed to create WhisperState from shared context: {:?}", e);
                return;
            }
        };

        // Set up audio capture
        let host = cpal::default_host();
        let device = match host.default_input_device() {
            Some(dev) => dev,
            None => {
                error!("No default input device found for always listening");
                return;
            }
        };

        let config = device.default_input_config().unwrap();
        let sample_format = config.sample_format();
        let sample_rate = config.sample_rate().0;

        let (audio_data_tx, audio_data_rx) = channel::<Vec<f32>>();

        let stream = match sample_format {
            SampleFormat::F32 => device.build_input_stream(
                &config.config(),
                move |data: &[f32], _: &cpal::InputCallbackInfo| {
                    if let Err(e) = audio_data_tx.send(data.to_vec()) {
                        error!("Failed to send audio data: {:?}", e);
                    }
                },
                move |err| {
                    error!("Audio stream error: {}", err);
                },
                None
            ).expect("Failed to build f32 input stream"),
            SampleFormat::I16 => device.build_input_stream(
                &config.config(),
                move |data: &[i16], _: &cpal::InputCallbackInfo| {
                    let mut audio_f32: Vec<f32> = vec![0.0f32; data.len()];
                    if let Err(e) = whisper_rs::convert_integer_to_float_audio(data, &mut audio_f32) {
                        error!("Failed to convert i16 to f32: {:?}", e);
                        return;
                    }
                    if let Err(e) = audio_data_tx.send(audio_f32) {
                        error!("Failed to send converted audio data: {:?}", e);
                    }
                },
                move |err| {
                    error!("Audio stream error: {}", err);
                },
                None
            ).expect("Failed to build i16 input stream"),
            _ => {
                error!("Unsupported sample format {:?}", sample_format);
                return;
            }
        };

        if let Err(e) = stream.play() {
            error!("Failed to start audio stream: {:?}", e);
            return;
        }

        info!("[AlwaysListening] Audio monitoring started");

        // Resampling will be done on-demand with custom resamplers

        let mut audio_buffer: Vec<f32> = Vec::new();
        let mut current_state = AlwaysListeningState::Monitoring;
        let buffer_capacity = (sample_rate as u64 * INTENT_DETECTION_BUFFER_MS / 1000) as usize;
        let min_transcription_samples = (sample_rate as u64 * MIN_TRANSCRIPTION_DURATION_MS / 1000) as usize;
        let mut audio_activity_start: Option<Instant> = None;
        let mut last_volume_drop: Option<Instant> = None;

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
                    info!("[AlwaysListening] Transcription debugging set to: {}", enabled);
                }
                Ok(AlwaysListeningMessage::SetAudioLevelMonitoring(enabled)) => {
                    info!("[AlwaysListening] Audio level monitoring set to: {}", enabled);
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
                    audio_buffer.extend_from_slice(&audio_chunk);

                    // Calculate volume level
                    let volume = Self::calculate_rms_volume(&audio_chunk);

                                        // Log audio chunk reception occasionally for debugging
                    thread_local! {
                        static LAST_CHUNK_LOG: std::cell::RefCell<Option<Instant>> = std::cell::RefCell::new(None);
                    }

                    LAST_CHUNK_LOG.with(|last_log| {
                        let mut last_log = last_log.borrow_mut();
                        if last_log.map_or(true, |last| last.elapsed().as_secs() > 10) {
                            info!("[AlwaysListening] Audio chunk: {} samples, RMS volume: {:.6}", audio_chunk.len(), volume);
                            *last_log = Some(Instant::now());
                        }
                    });

                    match current_state {
                        AlwaysListeningState::Monitoring => {
                            // Check for intent to activate
                            let volume_threshold = VOLUME_THRESHOLD * sensitivity;

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

                                // Only attempt transcription if we have sufficient audio duration and samples
                                if let Some(start_time) = audio_activity_start {
                                    let activity_duration = start_time.elapsed().as_millis();

                                    if activity_duration >= MIN_TRANSCRIPTION_DURATION_MS as u128 &&
                                       audio_buffer.len() >= min_transcription_samples {

                                        // Check if the accumulated audio has sufficient volume for speech
                                        let buffer_volume = Self::calculate_rms_volume(&audio_buffer);

                                        if buffer_volume >= MIN_SPEECH_VOLUME {
                                            info!("[AlwaysListening] Sufficient audio accumulated: {}ms, {} samples, volume: {:.6}",
                                                   activity_duration, audio_buffer.len(), buffer_volume);

                                            // Check for wake words or speech
                                            if Self::detect_intent(&mut whisper_state, &audio_buffer, sample_rate, &wake_words, &app_handle) {
                                                current_state = AlwaysListeningState::Activated;
                                                info!("[AlwaysListening] Intent detected - activating transcription");

                                                // Update last activity
                                                if let Ok(mut activity) = last_activity.lock() {
                                                    *activity = Some(Instant::now());
                                                }

                                                // Emit activation event
                                                if let Err(e) = app_handle.emit("always-listening:activated", ()) {
                                                    error!("[AlwaysListening] Failed to emit activation event: {}", e);
                                                }

                                                // Start active transcription
                                                audio_buffer.clear();
                                                audio_activity_start = None; // Reset activity tracking
                                                last_volume_drop = None; // Reset drop tracking
                                            } else {
                                                // No wake word detected, keep monitoring but maintain shorter buffer
                                                audio_buffer.clear();
                                                audio_activity_start = None; // Reset activity tracking
                                                last_volume_drop = None; // Reset drop tracking
                                            }
                                        } else {
                                            info!("[AlwaysListening] Audio accumulated but volume too low for speech: {:.6} < {:.6}",
                                                   buffer_volume, MIN_SPEECH_VOLUME);
                                            // Reset and wait for higher volume audio
                                            audio_buffer.clear();
                                            audio_activity_start = None;
                                            last_volume_drop = None;
                                        }
                                    }
                                }
                            } else {
                                // Volume below threshold - use hysteresis for ending activity
                                let end_threshold = VOLUME_THRESHOLD_END * sensitivity;

                                if audio_activity_start.is_some() && volume < end_threshold {
                                    // Check if we should tolerate brief volume drops
                                    if last_volume_drop.is_none() {
                                        last_volume_drop = Some(Instant::now());
                                        debug!("[AlwaysListening] Volume drop detected, starting tolerance timer - volume: {:.6} < {:.6}", volume, end_threshold);
                                    } else if let Some(drop_time) = last_volume_drop {
                                        if drop_time.elapsed().as_millis() > VOLUME_DROP_TOLERANCE_MS as u128 {
                                            info!("[AlwaysListening] Audio activity ended after tolerance period - volume: {:.6} < {:.6}", volume, end_threshold);
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
                                    static LAST_VOLUME_LOG: std::cell::RefCell<Option<Instant>> = std::cell::RefCell::new(None);
                                }

                                LAST_VOLUME_LOG.with(|last_log| {
                                    let mut last_log = last_log.borrow_mut();
                                    if last_log.map_or(true, |last| last.elapsed().as_secs() > 5) { // Reduced frequency
                                        debug!("[AlwaysListening] Volume monitoring: {:.6} < {:.6} (start threshold, sensitivity: {:.1})", volume, volume_threshold, sensitivity);
                                        *last_log = Some(Instant::now());
                                    }
                                });

                                // Maintain rolling buffer during monitoring
                                if audio_buffer.len() > buffer_capacity {
                                    audio_buffer.drain(0..audio_buffer.len() - buffer_capacity);
                                }
                            }
                        }
                        AlwaysListeningState::Activated => {
                            // Actively transcribing - check for silence to return to monitoring
                            let end_threshold = VOLUME_THRESHOLD_END * sensitivity;

                            if volume < end_threshold { // Lower threshold for ending activity
                                if let Ok(activity) = last_activity.lock() {
                                    if let Some(last_time) = *activity {
                                        if last_time.elapsed().as_millis() > SILENCE_TIMEOUT_MS as u128 {
                                            current_state = AlwaysListeningState::Monitoring;
                                            info!("[AlwaysListening] Silence timeout - returning to monitoring (volume: {:.6} < {:.6})", volume, end_threshold);

                                            // Emit deactivation event
                                            if let Err(e) = app_handle.emit("always-listening:deactivated", ()) {
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
                                Self::process_active_transcription(&mut whisper_state, &audio_buffer, sample_rate, &app_handle);
                                audio_buffer.clear();
                            }
                        }
                        AlwaysListeningState::Processing => {
                            // This state is currently unused but could be used for more complex processing
                            current_state = AlwaysListeningState::Monitoring;
                        }
                        AlwaysListeningState::WaitingForWakeWord => {
                            // Waiting for wake word after command completion
                            let end_threshold = VOLUME_THRESHOLD_END * sensitivity;

                            if volume < end_threshold { // Lower threshold for ending activity
                                if let Ok(activity) = last_activity.lock() {
                                    if let Some(last_time) = *activity {
                                        if last_time.elapsed().as_millis() > COMMAND_COMPLETION_TIMEOUT_MS as u128 {
                                            current_state = AlwaysListeningState::Monitoring;
                                            info!("[AlwaysListening] Command completion timeout - returning to monitoring (volume: {:.6} < {:.6})", volume, end_threshold);

                                            // Emit deactivation event
                                            if let Err(e) = app_handle.emit("always-listening:deactivated", ()) {
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
                                Self::process_waiting_transcription(&mut whisper_state, &audio_buffer, sample_rate, &app_handle);
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

    fn calculate_rms_volume(audio_chunk: &[f32]) -> f32 {
        if audio_chunk.is_empty() {
            return 0.0;
        }

        let sum_of_squares: f32 = audio_chunk.iter().map(|&sample| sample * sample).sum();
        (sum_of_squares / audio_chunk.len() as f32).sqrt()
    }

    fn detect_intent<R: Runtime>(
        whisper_state: &mut whisper_rs::WhisperState,
        audio_buffer: &[f32],
        sample_rate: u32,
        wake_words: &[String],
        _app_handle: &AppHandle<R>,
    ) -> bool {
        if audio_buffer.is_empty() {
            debug!("[AlwaysListening] detect_intent: Audio buffer is empty");
            return false;
        }

        let audio_duration_ms = (audio_buffer.len() as f32 / sample_rate as f32 * 1000.0) as u32;
        let min_duration_for_transcription = MIN_TRANSCRIPTION_DURATION_MS as u32;

        info!("[AlwaysListening] detect_intent: Processing {} samples ({}ms) for {} wake words",
               audio_buffer.len(),
               audio_duration_ms,
               wake_words.len());

        // Ensure we have sufficient audio duration for meaningful transcription
        if audio_duration_ms < min_duration_for_transcription {
            info!("[AlwaysListening] detect_intent: Audio duration too short ({}ms < {}ms), skipping transcription",
                   audio_duration_ms, min_duration_for_transcription);
            return false;
        }

        // Check audio quality - ensure it has sufficient volume for speech
        let avg_volume = Self::calculate_rms_volume(audio_buffer);
        if avg_volume < MIN_SPEECH_VOLUME {
            info!("[AlwaysListening] detect_intent: Audio volume too low for speech ({:.6} < {:.6}), skipping transcription",
                   avg_volume, MIN_SPEECH_VOLUME);
            return false;
        }

        // Resample if necessary
        info!("[AlwaysListening] Sample rate check: {} -> {} (needs resampling: {})",
              sample_rate, WHISPER_SAMPLE_RATE, sample_rate != WHISPER_SAMPLE_RATE);
        let audio_to_process = if sample_rate != WHISPER_SAMPLE_RATE {
            // Create a custom resampler for this specific buffer size
            let config = SincInterpolationParameters {
                sinc_len: SINC_LENGTH,
                f_cutoff: 0.95,
                interpolation: SincInterpolationType::Linear,
                oversampling_factor: OVERSAMPLING_FACTOR,
                window: WindowFunction::BlackmanHarris2,
            };

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
                            info!("[AlwaysListening] Audio resampled: {} -> {} samples", audio_buffer.len(), resampled[0].len());
                            resampled.remove(0)
                        },
                        Ok(_) => {
                            warn!("[AlwaysListening] Resampling produced empty output");
                            return false;
                        },
                        Err(e) => {
                            warn!("[AlwaysListening] Resampling failed: {:?}", e);
                            return false;
                        }
                    }
                },
                Err(e) => {
                    warn!("[AlwaysListening] Failed to create custom resampler: {:?}", e);
                    return false;
                }
            }
        } else {
            audio_buffer.to_vec()
        };

        // Ensure resampled audio also meets minimum duration and quality
        let resampled_duration_ms = (audio_to_process.len() as f32 / WHISPER_SAMPLE_RATE as f32 * 1000.0) as u32;
        if resampled_duration_ms < min_duration_for_transcription {
            info!("[AlwaysListening] detect_intent: Resampled audio duration too short ({}ms < {}ms), skipping transcription",
                   resampled_duration_ms, min_duration_for_transcription);
            return false;
        }

        let resampled_volume = Self::calculate_rms_volume(&audio_to_process);
        if resampled_volume < MIN_SPEECH_VOLUME * 0.5 { // Allow slightly lower volume after resampling
            info!("[AlwaysListening] detect_intent: Resampled audio volume too low ({:.6} < {:.6}), skipping transcription",
                   resampled_volume, MIN_SPEECH_VOLUME * 0.5);
            return false;
        }

        // Quick transcription for wake word detection
        let mut params = FullParams::new(whisper_rs::SamplingStrategy::Greedy { best_of: 1 });
        params.set_n_threads(4); // Increased from 2 for better performance
        params.set_print_special(false);
        params.set_print_progress(false);
        params.set_print_realtime(false);
        params.set_print_timestamps(false);
        params.set_language(Some("en"));
        params.set_translate(false);
        params.set_no_context(true); // Improve reliability for short audio
        params.set_single_segment(false); // Allow multiple segments

        info!("[AlwaysListening] Starting Whisper transcription with {}ms of audio (volume: {:.6})...",
               resampled_duration_ms, resampled_volume);

        match whisper_state.full(params, &audio_to_process) {
            Ok(_) => {
                let num_segments = whisper_state.full_n_segments().unwrap_or(0);
                let mut transcribed_text = String::new();

                info!("[AlwaysListening] Whisper processing completed, {} segments found", num_segments);

                for i in 0..num_segments {
                    if let Ok(segment_text) = whisper_state.full_get_segment_text(i) {
                        let segment_string = segment_text.to_string();
                        info!("[AlwaysListening] Segment {}: '{}'", i, segment_string);
                        transcribed_text.push_str(&segment_string);
                        transcribed_text.push(' ');
                    }
                }

                let text_lower = transcribed_text.trim().to_lowercase();
                info!("[AlwaysListening] Transcription result: '{}' (length: {})", text_lower, text_lower.len());

                // If we get no transcription, this might indicate a model issue
                if text_lower.is_empty() {
                    warn!("[AlwaysListening] Empty transcription result despite audio presence - check model and audio format");
                    return false;
                }

                // Check for wake words
                for wake_word in wake_words {
                    let wake_word_lower = wake_word.to_lowercase();
                    if text_lower.contains(&wake_word_lower) {
                        info!("[AlwaysListening] ✅ WAKE WORD DETECTED: '{}' found in '{}'", wake_word, text_lower);
                        return true;
                    } else {
                        debug!("[AlwaysListening] Wake word '{}' not found in '{}'", wake_word_lower, text_lower);
                    }
                }

                // Check for general speech activity (fallback if no wake words)
                if wake_words.is_empty() && !text_lower.trim().is_empty() {
                    info!("[AlwaysListening] Speech activity detected (no wake words configured): '{}'", text_lower);
                    return true;
                }

                // Log near misses (similar words)
                if !text_lower.is_empty() {
                    info!("[AlwaysListening] ❌ No wake words detected in: '{}'", text_lower);
                }

                false
            }
            Err(e) => {
                error!("[AlwaysListening] Whisper transcription failed: {:?}", e);
                false
            }
        }
    }

    fn process_active_transcription<R: Runtime>(
        whisper_state: &mut whisper_rs::WhisperState,
        audio_buffer: &[f32],
        sample_rate: u32,
        app_handle: &AppHandle<R>,
    ) {
        if audio_buffer.is_empty() {
            return;
        }

        let audio_to_transcribe = if sample_rate != WHISPER_SAMPLE_RATE {
            // Create a custom resampler for this specific buffer size
            let config = SincInterpolationParameters {
                sinc_len: SINC_LENGTH,
                f_cutoff: 0.95,
                interpolation: SincInterpolationType::Linear,
                oversampling_factor: OVERSAMPLING_FACTOR,
                window: WindowFunction::BlackmanHarris2,
            };

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
                },
                Err(_) => return,
            }
        } else {
            audio_buffer.to_vec()
        };

        let mut params = FullParams::new(whisper_rs::SamplingStrategy::Greedy { best_of: 0 });
        params.set_n_threads(4);
        params.set_print_special(false);
        params.set_print_progress(false);
        params.set_print_realtime(false);
        params.set_print_timestamps(false);

        match whisper_state.full(params, &audio_to_transcribe) {
            Ok(_) => {
                let num_segments = whisper_state.full_n_segments().unwrap_or(0);
                let mut transcribed_text = String::new();

                for i in 0..num_segments {
                    if let Ok(segment_text) = whisper_state.full_get_segment_text(i) {
                        let segment_string = segment_text.to_string();
                        info!("[AlwaysListening] Segment {}: '{}'", i, segment_string);
                        transcribed_text.push_str(&segment_string);
                    }
                }

                let cleaned_text = transcribed_text.trim();
                if !cleaned_text.is_empty() {
                    info!("[AlwaysListening] Active transcription result: '{}'", cleaned_text);

                    // Apply intelligent filtering before calling agent
                    if Self::should_process_with_agent(cleaned_text) {
                        // Check for stop words
                        if Self::contains_stop_words(cleaned_text) {
                            info!("[AlwaysListening] Stop word detected: '{}' - stopping always listening", cleaned_text);

                            // Emit stop event to main app
                            if let Err(e) = app_handle.emit("always-listening:stop-requested",
                                serde_json::json!({ "reason": "stop_word", "text": cleaned_text })) {
                                error!("[AlwaysListening] Failed to emit stop-requested event: {}", e);
                            }
                            return;
                        }

                        // Check agent call rate limiting
                        if Self::is_agent_call_allowed() {
                            // Mark successful agent call
                            Self::record_agent_call();

                            // Emit the transcription result for agent processing
                            if let Err(e) = app_handle.emit("always-listening:transcription",
                                serde_json::json!({ "text": cleaned_text })) {
                                error!("[AlwaysListening] Failed to emit transcription event: {}", e);
                            }

                            // Transition to waiting for wake word mode after processing
                            if let Err(e) = app_handle.emit("always-listening:command-processed", ()) {
                                error!("[AlwaysListening] Failed to emit command-processed event: {}", e);
                            }
                        } else {
                            warn!("[AlwaysListening] Agent call rate limit exceeded, skipping: '{}'", cleaned_text);
                        }
                    } else {
                        info!("[AlwaysListening] Content filtered out (noise/meaningless): '{}'", cleaned_text);
                    }
                } else {
                    debug!("[AlwaysListening] Empty transcription result, continuing monitoring");
                }
            }
            Err(e) => {
                debug!("[AlwaysListening] Active transcription failed: {:?}", e);
            }
        }
    }

        // Intelligent content filtering functions
    fn should_process_with_agent(text: &str) -> bool {
        let text_lower = text.to_lowercase();
        let mut text_trimmed = text_lower.trim().to_string();

        // Filter out empty or very short content
        if text_trimmed.len() < MIN_MEANINGFUL_CONTENT_LENGTH {
            info!("[AlwaysListening] Content too short: '{}' (length: {})", text_trimmed, text_trimmed.len());
            return false;
        }

        // Remove phrase-level noise patterns but don't reject the entire text
        // This allows "Open Spotify [BLANK_AUDIO]" to become "Open Spotify"
        let mut found_noise_patterns = Vec::new();
        for pattern in NOISE_PATTERNS {
            if text_trimmed.contains(pattern) {
                found_noise_patterns.push(pattern);
                text_trimmed = text_trimmed.replace(pattern, " ");
            }
        }

        // Clean up extra whitespace after pattern removal
        text_trimmed = text_trimmed.split_whitespace().collect::<Vec<&str>>().join(" ");

        if !found_noise_patterns.is_empty() {
            info!("[AlwaysListening] Removed noise patterns {:?} from text, remaining: '{}'", found_noise_patterns, text_trimmed);
        }

        // Re-check length after noise pattern removal
        if text_trimmed.len() < MIN_MEANINGFUL_CONTENT_LENGTH {
            info!("[AlwaysListening] Content too short after noise removal: '{}' (length: {})", text_trimmed, text_trimmed.len());
            return false;
        }

        // Split into words for word-level analysis
        let words: Vec<&str> = text_trimmed.split_whitespace().collect();

        // Filter out if text consists entirely of noise words
        let noise_word_count = words.iter()
            .filter(|word| NOISE_WORDS.contains(word))
            .count();

        if noise_word_count == words.len() && words.len() > 0 {
            info!("[AlwaysListening] Text consists entirely of noise words: '{}'", text_trimmed);
            return false;
        }

        // Filter out repeated single characters (like "a a a a")
        if words.len() > 2 {
            let unique_words: std::collections::HashSet<&str> = words.iter().cloned().collect();
            if unique_words.len() == 1 && words[0].len() == 1 {
                info!("[AlwaysListening] Repetitive single character detected: '{}'", text_trimmed);
                return false;
            }
        }

        // Filter out content that is mostly punctuation or numbers
        let letter_count = text_trimmed.chars().filter(|c| c.is_alphabetic()).count();
        let total_meaningful_chars = text_trimmed.chars().filter(|c| c.is_alphanumeric() || c.is_whitespace()).count();

        if total_meaningful_chars > 0 && (letter_count as f32 / total_meaningful_chars as f32) < 0.3 {
            info!("[AlwaysListening] Content mostly non-alphabetic: '{}'", text_trimmed);
            return false;
        }

        // Check for minimum meaningful word count (exclude noise words from meaningful words)
        let meaningful_words: Vec<&str> = words.iter()
            .filter(|word| word.len() > 2 && !NOISE_WORDS.contains(word))
            .cloned()
            .collect();

        if meaningful_words.is_empty() {
            info!("[AlwaysListening] No meaningful words found: '{}'", text_trimmed);
            return false;
        }

        info!("[AlwaysListening] Content passed filtering: '{}' (meaningful words: {})", text_trimmed, meaningful_words.len());
        true
    }

    fn contains_stop_words(text: &str) -> bool {
        let text_lower = text.to_lowercase();
        for stop_word in STOP_WORDS {
            if text_lower.contains(stop_word) {
                return true;
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
        whisper_state: &mut whisper_rs::WhisperState,
        audio_buffer: &[f32],
        sample_rate: u32,
        app_handle: &AppHandle<R>,
    ) {
        if audio_buffer.is_empty() {
            return;
        }

        let audio_to_transcribe = if sample_rate != WHISPER_SAMPLE_RATE {
            // Create a custom resampler for this specific buffer size
            let config = SincInterpolationParameters {
                sinc_len: SINC_LENGTH,
                f_cutoff: 0.95,
                interpolation: SincInterpolationType::Linear,
                oversampling_factor: OVERSAMPLING_FACTOR,
                window: WindowFunction::BlackmanHarris2,
            };

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
                },
                Err(_) => return,
            }
        } else {
            audio_buffer.to_vec()
        };

        let mut params = FullParams::new(whisper_rs::SamplingStrategy::Greedy { best_of: 0 });
        params.set_n_threads(4);
        params.set_print_special(false);
        params.set_print_progress(false);
        params.set_print_realtime(false);
        params.set_print_timestamps(false);

        match whisper_state.full(params, &audio_to_transcribe) {
            Ok(_) => {
                let num_segments = whisper_state.full_n_segments().unwrap_or(0);
                let mut transcribed_text = String::new();

                for i in 0..num_segments {
                    if let Ok(segment_text) = whisper_state.full_get_segment_text(i) {
                        let segment_string = segment_text.to_string();
                        info!("[AlwaysListening] Segment {}: '{}'", i, segment_string);
                        transcribed_text.push_str(&segment_string);
                    }
                }

                if !transcribed_text.trim().is_empty() {
                    info!("[AlwaysListening] Waiting transcription result: '{}'", transcribed_text);

                    // Emit the transcription result
                    if let Err(e) = app_handle.emit("always-listening:transcription",
                        serde_json::json!({ "text": transcribed_text })) {
                        error!("[AlwaysListening] Failed to emit transcription event: {}", e);
                    }
                }
            }
            Err(e) => {
                debug!("[AlwaysListening] Waiting transcription failed: {:?}", e);
            }
        }
    }

    pub fn stop_always_listening(&mut self) -> Result<bool> {
        if !self.is_active {
            return Ok(false);
        }

        info!("[AlwaysListeningController] Stopping always listening mode...");

        if let Some((thread_handle, control_tx)) = self.audio_thread.take() {
            // Send stop message
            if let Err(e) = control_tx.send(AlwaysListeningMessage::Stop) {
                warn!("[AlwaysListeningController] Failed to send stop message: {:?}", e);
            }

            // Wait for thread to finish with timeout
            match thread_handle.join() {
                Ok(_) => info!("[AlwaysListeningController] Audio thread finished cleanly"),
                Err(e) => error!("[AlwaysListeningController] Audio thread join error: {:?}", e),
            }
        }

        self.is_active = false;
        self.state = AlwaysListeningState::Monitoring;

        info!("[AlwaysListeningController] Always listening mode stopped");
        Ok(true)
    }

    pub fn is_active(&self) -> bool {
        self.is_active
    }

    pub fn get_state(&self) -> AlwaysListeningState {
        self.state.clone()
    }

    pub fn set_sensitivity(&mut self, sensitivity: f32) -> Result<()> {
        self.sensitivity = sensitivity.clamp(0.1, 2.0);

        if let Some((_, control_tx)) = &self.audio_thread {
            control_tx.send(AlwaysListeningMessage::UpdateSensitivity(self.sensitivity))
                .map_err(|e| Error::ControlError(format!("Failed to update sensitivity: {:?}", e)))?;
        }

        Ok(())
    }

    pub fn get_sensitivity(&self) -> f32 {
        self.sensitivity
    }

    pub fn set_wake_words(&mut self, wake_words: Vec<String>) -> Result<()> {
        self.wake_words = wake_words.clone();

        if let Some((_, control_tx)) = &self.audio_thread {
            control_tx.send(AlwaysListeningMessage::UpdateWakeWords(wake_words))
                .map_err(|e| Error::ControlError(format!("Failed to update wake words: {:?}", e)))?;
        }

        Ok(())
    }

    pub fn get_wake_words(&self) -> Vec<String> {
        self.wake_words.clone()
    }

    /// Update the shared Whisper context and model path for this controller
    /// This allows updating the model without recreating the entire controller
    pub fn update_shared_context(&mut self, model_path: &str, shared_context: Arc<WhisperContext>) -> Result<()> {
        info!("[AlwaysListeningController] Updating shared Whisper context with new model: {}", model_path);

        // Stop the controller if it's currently active
        let was_active = self.is_active;
        if was_active {
            info!("[AlwaysListeningController] Stopping controller before model update");
            self.stop_always_listening()?;
        }

        // Update the model path and shared context
        self.model_path = model_path.to_string();
        self.shared_whisper_context = Some(shared_context);

        info!("[AlwaysListeningController] Successfully updated shared Whisper context (no model reload needed)");

        // If it was active before, we'll let the caller decide whether to restart it
        // This gives more control over the restart timing
        if was_active {
            info!("[AlwaysListeningController] Controller was active before update - caller should restart if needed");
        }

        Ok(())
    }

    // Enhanced Debugging Methods

    pub fn set_transcription_debugging<R: Runtime>(&mut self, enabled: bool, app_handle: &AppHandle<R>) -> Result<()> {
        info!("[AlwaysListeningController] Setting transcription debugging to: {}", enabled);

        if let Some((_, control_tx)) = &self.audio_thread {
            control_tx.send(AlwaysListeningMessage::SetTranscriptionDebugging(enabled))
                .map_err(|e| Error::ControlError(format!("Failed to set transcription debugging: {:?}", e)))?;
        }

        if enabled {
            // Emit an event to confirm debugging is enabled
            app_handle.emit("always-listening-event", serde_json::json!({
                "type": "transcription_debug",
                "payload": { "enabled": true }
            })).map_err(|e| Error::EventError(format!("Failed to emit debugging enabled event: {}", e)))?;
        }

        Ok(())
    }

    pub fn set_audio_level_monitoring<R: Runtime>(&mut self, enabled: bool, app_handle: &AppHandle<R>) -> Result<()> {
        info!("[AlwaysListeningController] Setting audio level monitoring to: {}", enabled);

        if let Some((_, control_tx)) = &self.audio_thread {
            control_tx.send(AlwaysListeningMessage::SetAudioLevelMonitoring(enabled))
                .map_err(|e| Error::ControlError(format!("Failed to set audio level monitoring: {:?}", e)))?;
        }

        if enabled {
            // Emit an event to confirm monitoring is enabled
            app_handle.emit("always-listening-event", serde_json::json!({
                "type": "audio_level",
                "payload": { "enabled": true }
            })).map_err(|e| Error::EventError(format!("Failed to emit monitoring enabled event: {}", e)))?;
        }

        Ok(())
    }

    pub fn test_whisper_model(&self) -> Result<serde_json::Value> {
        info!("[AlwaysListening] Testing shared Whisper model at path: {}", self.model_path);

        // Use the shared context instead of loading a new one
        let whisper_context = self.shared_whisper_context.as_ref()
            .ok_or_else(|| Error::Whisper("Shared Whisper context not available".to_string()))?;

        let mut whisper_state = whisper_context.create_state()
            .map_err(|e| Error::Whisper(format!("Failed to create Whisper state: {:?}", e)))?;

        // Create test audio - a simple sine wave that should be detectable
        let sample_rate = WHISPER_SAMPLE_RATE;
        let duration_samples = sample_rate as usize; // 1 second
        let frequency = 440.0; // A4 note
        let mut test_audio: Vec<f32> = Vec::with_capacity(duration_samples);

        for i in 0..duration_samples {
            let t = i as f32 / sample_rate as f32;
            let sample = (2.0 * std::f32::consts::PI * frequency * t).sin() * 0.1; // Low amplitude sine wave
            test_audio.push(sample);
        }

        // Test volume calculation
        let test_volume = Self::calculate_rms_volume(&test_audio);

        // Test transcription with the test audio
        let mut params = FullParams::new(whisper_rs::SamplingStrategy::Greedy { best_of: 1 });
        params.set_n_threads(4);
        params.set_print_special(false);
        params.set_print_progress(false);
        params.set_print_realtime(false);
        params.set_print_timestamps(false);
        params.set_language(Some("en"));
        params.set_translate(false);

        let transcription_result = match whisper_state.full(params, &test_audio) {
            Ok(_) => {
                let num_segments = whisper_state.full_n_segments().unwrap_or(0);
                let mut transcribed_text = String::new();

                for i in 0..num_segments {
                    if let Ok(segment_text) = whisper_state.full_get_segment_text(i) {
                        let segment_string = segment_text.to_string();
                        info!("[AlwaysListening] Segment {}: '{}'", i, segment_string);
                        transcribed_text.push_str(&segment_string);
                        transcribed_text.push(' ');
                    }
                }

                format!("SUCCESS: {} segments, text: '{}'", num_segments, transcribed_text.trim())
            }
            Err(e) => format!("FAILED: {:?}", e)
        };

        // Create test result
        let test_result = serde_json::json!({
            "model_path": self.model_path,
            "model_exists": std::path::Path::new(&self.model_path).exists(),
            "model_loaded": true,
            "state_created": true,
            "test_audio_samples": test_audio.len(),
            "test_audio_duration_ms": (test_audio.len() as f32 / sample_rate as f32 * 1000.0) as u32,
            "test_audio_volume": test_volume,
            "volume_threshold": VOLUME_THRESHOLD,
            "min_speech_volume": MIN_SPEECH_VOLUME,
            "min_transcription_duration_ms": MIN_TRANSCRIPTION_DURATION_MS,
            "transcription_test": transcription_result,
            "wake_words": self.wake_words,
            "sensitivity": self.sensitivity,
            "status": "Model test completed"
        });

        info!("[AlwaysListening] Model test result: {}", serde_json::to_string_pretty(&test_result).unwrap_or_default());
        Ok(test_result)
    }

    pub fn force_transcription_test<R: Runtime>(&mut self, _app_handle: &AppHandle<R>) -> Result<serde_json::Value> {
        info!("[AlwaysListeningController] Starting force transcription test...");

        if let Some((_, control_tx)) = &self.audio_thread {
            control_tx.send(AlwaysListeningMessage::ForceTranscriptionTest)
                .map_err(|e| Error::ControlError(format!("Failed to send force transcription test: {:?}", e)))?;

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

    pub fn force_threshold_test<R: Runtime>(&mut self, _app_handle: &AppHandle<R>) -> Result<serde_json::Value> {
        info!("[AlwaysListeningController] Starting force threshold test...");

        if let Some((_, control_tx)) = &self.audio_thread {
            // Temporarily set very low sensitivity for testing
            control_tx.send(AlwaysListeningMessage::UpdateSensitivity(0.1))
                .map_err(|e| Error::ControlError(format!("Failed to set test sensitivity: {:?}", e)))?;

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
            control_tx.send(AlwaysListeningMessage::ReturnToWakeWordMode)
                .map_err(|e| Error::ControlError(format!("Failed to return to wake word mode: {:?}", e)))?;
        }
        Ok(())
    }
}

unsafe impl Send for AlwaysListeningController {}
unsafe impl Sync for AlwaysListeningController {}

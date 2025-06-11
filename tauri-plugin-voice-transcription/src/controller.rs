use whisper_rs::{FullParams, WhisperContext, WhisperContextParameters};
use std::path::Path;
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::SampleFormat;
use std::sync::mpsc::{channel, Sender, TryRecvError};
use std::thread;
use std::time::Duration;
use std::sync::{Arc, Mutex, RwLock};
use rubato::{Resampler, SincFixedIn, SincInterpolationType, SincInterpolationParameters, WindowFunction};
use hound;
use tauri::{AppHandle, Emitter, Runtime};
use tracing::{info, warn, error};

use crate::error::{Error, Result};

const WHISPER_SAMPLE_RATE: u32 = 16000;

// CRITICAL MEMORY MANAGEMENT CONSTANTS
const MAX_AUDIO_BUFFER_SIZE: usize = 240_000; // 15 seconds at 16kHz for dictation (more than always listening)
const MAX_MEMORY_USAGE_MB: usize = 100; // Higher limit for dictation sessions
const AUDIO_CHUNK_SIZE_LIMIT: usize = 24_000; // 1.5 seconds at 16kHz
const MEMORY_CHECK_INTERVAL_MS: u64 = 2000; // Check memory every 2 seconds for dictation

enum AudioThreadMessage {
    Stop,
    ForceMemoryCleanup, // Add memory cleanup message
}

// Enhanced memory tracker with dictation-specific limits
#[derive(Debug)]
struct VoiceMemoryTracker {
    buffer_usage: Arc<RwLock<usize>>,
    model_usage: Arc<RwLock<usize>>,
    peak_usage: Arc<RwLock<usize>>,
    session_start: Arc<RwLock<std::time::Instant>>,
}

impl VoiceMemoryTracker {
    fn new() -> Self {
        Self {
            buffer_usage: Arc::new(RwLock::new(0)),
            model_usage: Arc::new(RwLock::new(0)),
            peak_usage: Arc::new(RwLock::new(0)),
            session_start: Arc::new(RwLock::new(std::time::Instant::now())),
        }
    }

    fn add_buffer_usage(&self, bytes: usize) {
        if let Ok(mut buffer) = self.buffer_usage.write() {
            *buffer += bytes;
            self.update_peak();
        }
    }

    fn remove_buffer_usage(&self, bytes: usize) {
        if let Ok(mut buffer) = self.buffer_usage.write() {
            *buffer = buffer.saturating_sub(bytes);
        }
    }

    fn set_model_usage(&self, bytes: usize) {
        if let Ok(mut model) = self.model_usage.write() {
            *model = bytes;
            self.update_peak();
        }
    }

    fn update_peak(&self) {
        let total = self.get_total_mb();
        if let Ok(mut peak) = self.peak_usage.write() {
            if total > *peak {
                *peak = total;
            }
        }
    }

    fn get_total_mb(&self) -> usize {
        let buffer_bytes = self.buffer_usage.read().unwrap_or_default();
        let model_bytes = self.model_usage.read().unwrap_or_default();
        (*buffer_bytes + *model_bytes) / (1024 * 1024)
    }

    fn should_cleanup(&self) -> bool {
        let total_mb = self.get_total_mb();
        let session_duration = self.session_start.read()
            .map(|t| t.elapsed().as_secs())
            .unwrap_or(0);
        
        // Cleanup if memory exceeds limit or session is very long
        total_mb > MAX_MEMORY_USAGE_MB || session_duration > 300 // 5 minutes
    }

    fn reset_session(&self) {
        if let Ok(mut start) = self.session_start.write() {
            *start = std::time::Instant::now();
        }
    }
}

pub struct VoiceController {
    ctx: Option<WhisperContext>,
    pub model_path: String,
    is_dictating: bool,
    audio_thread: Option<(thread::JoinHandle<()>, Sender<AudioThreadMessage>)>,
    last_processed_audio_buffer: Arc<Mutex<Option<Vec<f32>>>>,
    actual_recording_sample_rate: Arc<Mutex<Option<u32>>>,
    is_initialized: bool,
    initialization_error: Option<String>,
    memory_tracker: VoiceMemoryTracker, // Add memory tracking
}

impl VoiceController {
    pub fn new(model_path_str: &str) -> Result<Self> {
        let model_path = Path::new(model_path_str);
        if !model_path.exists() {
            return Err(Error::ModelNotFound(model_path_str.to_string()));
        }

        let context_params = WhisperContextParameters::default();
        let ctx = WhisperContext::new_with_params(model_path_str, context_params)
            .map_err(|e| Error::Whisper(format!("Failed to create WhisperContext: {:?}", e)))?;

        let memory_tracker = VoiceMemoryTracker::new();
        // Track model memory usage (approximate 77MB for tiny.en model)
        memory_tracker.set_model_usage(77 * 1024 * 1024);

        Ok(Self {
            ctx: Some(ctx),
            model_path: model_path_str.to_string(),
            is_dictating: false,
            audio_thread: None,
            last_processed_audio_buffer: Arc::new(Mutex::new(None)),
            actual_recording_sample_rate: Arc::new(Mutex::new(None)),
            is_initialized: true,
            initialization_error: None,
            memory_tracker,
        })
    }

    /// Create an uninitialized controller that can be managed by Tauri but will return errors for operations
    pub fn new_uninitialized(model_path_str: &str, error_message: String) -> Self {
        Self {
            ctx: None,
            model_path: model_path_str.to_string(),
            is_dictating: false,
            audio_thread: None,
            last_processed_audio_buffer: Arc::new(Mutex::new(None)),
            actual_recording_sample_rate: Arc::new(Mutex::new(None)),
            is_initialized: false,
            initialization_error: Some(error_message),
            memory_tracker: VoiceMemoryTracker::new(),
        }
    }

    /// Check if the controller was successfully initialized
    pub fn is_initialized(&self) -> bool {
        self.is_initialized
    }

    /// Get the initialization error if any
    pub fn get_initialization_error(&self) -> Option<&String> {
        self.initialization_error.as_ref()
    }

    /// Helper method to check initialization before performing operations
    fn ensure_initialized(&self) -> Result<&WhisperContext> {
        if !self.is_initialized {
            let error_msg = self.initialization_error
                .as_ref()
                .map(|e| format!("Voice controller not initialized: {}", e))
                .unwrap_or_else(|| "Voice controller not initialized".to_string());
            return Err(Error::NotInitialized);
        }
        self.ctx.as_ref().ok_or(Error::NotInitialized)
    }

    pub fn transcribe_audio_file(&self, audio_path_str: &str) -> std::result::Result<String, String> {
        let ctx = match self.ensure_initialized() {
            Ok(ctx) => ctx,
            Err(e) => return Err(e.to_string()),
        };

        let audio_path = Path::new(audio_path_str);
        if !audio_path.exists() {
            return Err(format!("Audio file not found: {}", audio_path_str));
        }

        let mut state = ctx.create_state()
            .map_err(|e| format!("Failed to create WhisperState: {:?}", e))?;

        let params = FullParams::new(whisper_rs::SamplingStrategy::Greedy { best_of: 0 });

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

        let mut audio_f32: Vec<f32> = vec![0.0f32; samples_i16.len()];
        whisper_rs::convert_integer_to_float_audio(&samples_i16, &mut audio_f32)
            .map_err(|e| format!("Failed to convert audio to f32: {:?}", e))?;

        let mut processed_audio = audio_f32;

        if channels == 2 {
            processed_audio = whisper_rs::convert_stereo_to_mono_audio(&processed_audio)
                .map_err(|e| format!("Failed to convert stereo to mono: {:?}", e))?;
        } else if channels != 1 {
            return Err(format!(
                "Unsupported number of channels: {}. Only mono (1) or stereo (2) is supported.",
                channels
            ));
        }

        // Resample if needed
        if sample_rate != WHISPER_SAMPLE_RATE {
            let params = SincInterpolationParameters {
                sinc_len: 256,
                f_cutoff: 0.95,
                interpolation: SincInterpolationType::Linear,
                oversampling_factor: 256,
                window: WindowFunction::BlackmanHarris2,
            };

            let mut resampler = SincFixedIn::new(
                WHISPER_SAMPLE_RATE as f64 / sample_rate as f64,
                2.0,
                params,
                processed_audio.len(),
                1,
            ).map_err(|e| format!("Failed to create resampler: {:?}", e))?;

            let waves_in = vec![processed_audio];
            let waves_out = resampler.process(&waves_in, None)
                .map_err(|e| format!("Resampling failed: {:?}", e))?;

            if waves_out.is_empty() || waves_out[0].is_empty() {
                return Err("Resampling produced empty audio".to_string());
            }

            processed_audio = waves_out.into_iter().next()
                .ok_or_else(|| "Resampling failed to produce audio data".to_string())?;
        }

        state.full(params, &processed_audio[..])
            .map_err(|e| format!("Failed to run full transcription: {:?}", e))?;

        let num_segments = state.full_n_segments()
            .map_err(|e| format!("Failed to get number of segments: {:?}", e))?;

        let mut full_text = String::new();
        for i in 0..num_segments {
            let segment = state.full_get_segment_text(i)
                .map_err(|e| format!("Failed to get segment text: {:?}", e))?;
            full_text.push_str(&segment);
        }
        Ok(full_text)
    }

    pub fn start_dictation<R: Runtime + 'static>(&mut self, app_handle: &AppHandle<R>) -> Result<()> {
        // Check if controller is initialized before starting dictation
        self.ensure_initialized()?;
        
        if self.is_dictating {
            return Err(Error::AlreadyDictating);
        }

        info!("[VoiceController] Starting dictation...");

        // Emit dictation started event
        app_handle.emit("voice-transcription:dictation-started", ())
            .map_err(|e| Error::Tauri(e.to_string()))?;

        // Clear the last processed audio buffer
        if let Ok(mut buffer_guard) = self.last_processed_audio_buffer.lock() {
            *buffer_guard = None;
        }

        let host = cpal::default_host();
        let device = host.default_input_device()
            .ok_or_else(|| Error::AudioDevice("Failed to find a default input device.".to_string()))?;

        let supported_configs_iter = device.supported_input_configs()
            .map_err(|e| Error::AudioDevice(format!("Failed to get device configs: {:?}", e)))?;

        let selected_config_range = supported_configs_iter
            .filter(|c| c.channels() == 1)
            .find(|c| {
                (c.min_sample_rate().0..=c.max_sample_rate().0).contains(&16000) &&
                (c.sample_format() == SampleFormat::F32 || c.sample_format() == SampleFormat::I16)
            });

        let supported_config = if let Some(conf_range) = selected_config_range {
            conf_range.with_sample_rate(cpal::SampleRate(16000))
        } else {
            device.supported_input_configs()
                .map_err(|e| Error::AudioDevice(format!("Failed to get device configs for fallback: {:?}", e)))?
                .filter(|c| c.channels() == 1)
                .find(|c| c.sample_format() == SampleFormat::F32 || c.sample_format() == SampleFormat::I16)
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

        // Store the actual sample rate safely
        if let Ok(mut rate_guard) = self.actual_recording_sample_rate.lock() {
            *rate_guard = Some(actual_rate);
        } else {
            tracing::error!("Failed to acquire lock for actual_recording_sample_rate - lock may be poisoned");
        }

        let (control_tx, control_rx) = channel::<AudioThreadMessage>();
        let (audio_data_tx, audio_data_rx) = channel::<Vec<f32>>();

        let model_path_for_thread = self.model_path.clone();
        let last_buffer_arc_for_thread = Arc::clone(&self.last_processed_audio_buffer);
        let actual_rate_for_thread = actual_rate;
        let app_handle_for_thread = app_handle.clone();

        let audio_thread_handle = thread::spawn(move || {
            Self::audio_thread_worker(
                model_path_for_thread,
                last_buffer_arc_for_thread,
                actual_rate_for_thread,
                app_handle_for_thread,
                control_rx,
                audio_data_tx,
                audio_data_rx,
                device,
                config,
                sample_format,
            );
        });

        // Start the audio stream
        self.audio_thread = Some((audio_thread_handle, control_tx));
        self.is_dictating = true;

        Ok(())
    }

    fn audio_thread_worker<R: Runtime + 'static>(
        model_path: String,
        last_buffer_arc: Arc<Mutex<Option<Vec<f32>>>>,
        actual_rate: u32,
        app_handle: AppHandle<R>,
        control_rx: std::sync::mpsc::Receiver<AudioThreadMessage>,
        audio_data_tx: std::sync::mpsc::Sender<Vec<f32>>,
        audio_data_rx: std::sync::mpsc::Receiver<Vec<f32>>,
        device: cpal::Device,
        config: cpal::StreamConfig,
        sample_format: SampleFormat,
    ) {
        info!("[AudioThread] Thread started. Initializing Whisper context and state.");

        // Initialize memory tracker for this thread
        let memory_tracker = VoiceMemoryTracker::new();
        let mut last_memory_check = std::time::Instant::now();

        let whisper_context = match WhisperContext::new_with_params(&model_path, WhisperContextParameters::default()) {
            Ok(ctx) => {
                // Track model memory usage
                memory_tracker.set_model_usage(77 * 1024 * 1024);
                ctx
            },
            Err(e) => {
                error!("Failed to create WhisperContext in audio thread: {:?}", e);
                return;
            }
        };

        let mut whisper_state = match whisper_context.create_state() {
            Ok(state) => {
                // Track additional state memory
                memory_tracker.add_buffer_usage(10 * 1024 * 1024);
                state
            },
            Err(e) => {
                error!("Failed to create WhisperState in audio thread: {:?}", e);
                return;
            }
        };

        // Create stream with memory-safe callbacks
        let stream = match sample_format {
            SampleFormat::F32 => {
                match device.build_input_stream(
                    &config,
                    move |data: &[f32], _: &cpal::InputCallbackInfo| {
                        // CRITICAL: Limit chunk size to prevent memory exhaustion
                        let chunk_to_send = if data.len() > AUDIO_CHUNK_SIZE_LIMIT {
                            warn!("[AudioThread] Audio chunk too large: {} samples, truncating to {}", 
                                  data.len(), AUDIO_CHUNK_SIZE_LIMIT);
                            &data[..AUDIO_CHUNK_SIZE_LIMIT]
                        } else {
                            data
                        };
                        
                        if let Err(e) = audio_data_tx.send(chunk_to_send.to_vec()) {
                            error!("Failed to send audio data: {:?}", e);
                        }
                    },
                    move |err| {
                        error!("An error occurred on the input stream: {}", err);
                    },
                    None
                ) {
                    Ok(stream) => stream,
                    Err(e) => {
                        error!("Failed to build f32 input stream: {:?}", e);
                        return;
                    }
                }
            },
            SampleFormat::I16 => {
                match device.build_input_stream(
                    &config,
                    move |data: &[i16], _: &cpal::InputCallbackInfo| {
                        // CRITICAL: Limit chunk size to prevent memory exhaustion
                        let chunk_to_process = if data.len() > AUDIO_CHUNK_SIZE_LIMIT {
                            warn!("[AudioThread] Audio chunk too large: {} samples, truncating to {}", 
                                  data.len(), AUDIO_CHUNK_SIZE_LIMIT);
                            &data[..AUDIO_CHUNK_SIZE_LIMIT]
                        } else {
                            data
                        };
                        
                        let mut audio_f32: Vec<f32> = vec![0.0f32; chunk_to_process.len()];
                        if let Err(e) = whisper_rs::convert_integer_to_float_audio(chunk_to_process, &mut audio_f32) {
                            error!("Failed to convert i16 to f32: {:?}", e);
                            return;
                        }
                        if let Err(e) = audio_data_tx.send(audio_f32) {
                            error!("Failed to send converted audio data: {:?}", e);
                        }
                    },
                    move |err| {
                        error!("An error occurred on the input stream: {}", err);
                    },
                    None
                ) {
                    Ok(stream) => stream,
                    Err(e) => {
                        error!("Failed to build i16 input stream: {:?}", e);
                        return;
                    }
                }
            },
            _ => {
                error!("Unsupported sample format {:?}", sample_format);
                return;
            }
        };

        if let Err(e) = stream.play() {
            error!("Failed to start audio stream: {:?}", e);
            return;
        }

        info!("[AudioThread] Audio stream started.");
        info!("[AudioThread] Recording at {} Hz, will resample to {} Hz for Whisper.", actual_rate, WHISPER_SAMPLE_RATE);

        // Enhanced resampler setup with error handling
        let mut chunk_resampler: Option<SincFixedIn<f32>> = None;
        if actual_rate != WHISPER_SAMPLE_RATE {
            let params = SincInterpolationParameters {
                sinc_len: 256,
                f_cutoff: 0.95,
                interpolation: SincInterpolationType::Linear,
                oversampling_factor: 256,
                window: WindowFunction::BlackmanHarris2,
            };
            
            match SincFixedIn::new(
                WHISPER_SAMPLE_RATE as f64 / actual_rate as f64,
                2.0,
                params,
                1024,
                1,
            ) {
                Ok(resampler) => {
                    chunk_resampler = Some(resampler);
                    info!("[AudioThread] Resampler initialized successfully");
                },
                Err(e) => {
                    error!("[AudioThread] Failed to create resampler: {:?}", e);
                    return;
                }
            }
        }

        // CRITICAL: Use bounded buffers to prevent memory exhaustion
        let mut audio_buffer_for_whisper_chunks: Vec<f32> = Vec::new();
        let mut raw_full_session_audio: Vec<f32> = Vec::new();

        // CRITICAL: Bound the buffer capacities
        let partial_buffer_capacity_samples = std::cmp::min(
            (actual_rate as u64 * 1500 / 1000) as usize,
            MAX_AUDIO_BUFFER_SIZE / 4 // Use 1/4 of max for partial processing
        );
        
        let max_session_audio_samples = std::cmp::min(
            (actual_rate as u64 * 300 / 1000) as usize, // 5 minutes max
            MAX_AUDIO_BUFFER_SIZE
        );

        // Pre-allocate buffers to avoid frequent reallocations
        audio_buffer_for_whisper_chunks.reserve(partial_buffer_capacity_samples);
        raw_full_session_audio.reserve(max_session_audio_samples);

        // Track initial buffer allocations
        memory_tracker.add_buffer_usage(
            (partial_buffer_capacity_samples + max_session_audio_samples) * std::mem::size_of::<f32>()
        );

        info!("[AudioThread] Memory-bounded buffers initialized: partial={} samples, session={} samples max", 
              partial_buffer_capacity_samples, max_session_audio_samples);

        loop {
            // CRITICAL: Periodic memory monitoring and cleanup
            if last_memory_check.elapsed().as_millis() > MEMORY_CHECK_INTERVAL_MS as u128 {
                let current_mb = memory_tracker.get_total_mb();
                if current_mb > 0 {
                    info!("[AudioThread] Memory usage: {}MB", current_mb);
                }

                if memory_tracker.should_cleanup() {
                    warn!("[AudioThread] Memory limit exceeded ({}MB), forcing cleanup", current_mb);
                    
                    // Force cleanup: clear buffers and reset tracking
                    let old_buffer_size = (audio_buffer_for_whisper_chunks.len() + raw_full_session_audio.len()) 
                        * std::mem::size_of::<f32>();
                    
                    audio_buffer_for_whisper_chunks.clear();
                    audio_buffer_for_whisper_chunks.shrink_to_fit();
                    
                    // Keep only recent session audio (last 30 seconds)
                    let keep_samples = std::cmp::min(actual_rate as usize * 30, raw_full_session_audio.len());
                    if raw_full_session_audio.len() > keep_samples {
                        raw_full_session_audio.drain(0..raw_full_session_audio.len() - keep_samples);
                    }
                    raw_full_session_audio.shrink_to_fit();
                    
                    memory_tracker.remove_buffer_usage(old_buffer_size);
                    memory_tracker.reset_session();
                    
                    // Emit cleanup event
                    if let Err(e) = app_handle.emit("voice-transcription:memory-cleanup", ()) {
                        error!("[AudioThread] Failed to emit memory cleanup event: {}", e);
                    }
                    
                    info!("[AudioThread] Memory cleanup completed");
                }
                
                last_memory_check = std::time::Instant::now();
            }

            // Check for control messages
            match control_rx.try_recv() {
                Ok(AudioThreadMessage::Stop) => {
                    info!("[AudioThread] Stop message received.");
                    info!("[AudioThread] Final audio buffer size: {} samples", audio_buffer_for_whisper_chunks.len());
                    info!("[AudioThread] Raw session audio size: {} samples ({:.2} seconds)",
                          raw_full_session_audio.len(),
                          raw_full_session_audio.len() as f32 / actual_rate as f32);

                    // Process final audio
                    Self::process_final_audio(
                        &mut whisper_state,
                        &audio_buffer_for_whisper_chunks,
                        &raw_full_session_audio,
                        actual_rate,
                        chunk_resampler.as_mut(),
                        &app_handle,
                        &last_buffer_arc,
                    );

                    break;
                }
                Ok(AudioThreadMessage::ForceMemoryCleanup) => {
                    info!("[AudioThread] Force memory cleanup requested");
                    
                    // Immediate cleanup
                    let old_buffer_size = (audio_buffer_for_whisper_chunks.len() + raw_full_session_audio.len()) 
                        * std::mem::size_of::<f32>();
                    
                    audio_buffer_for_whisper_chunks.clear();
                    audio_buffer_for_whisper_chunks.shrink_to_fit();
                    raw_full_session_audio.clear();
                    raw_full_session_audio.shrink_to_fit();
                    
                    memory_tracker.remove_buffer_usage(old_buffer_size);
                    memory_tracker.reset_session();
                }
                Err(TryRecvError::Empty) => {}
                Err(TryRecvError::Disconnected) => {
                    info!("[AudioThread] Control channel disconnected.");
                    break;
                }
            }

            // Process audio data with enhanced memory management
            match audio_data_rx.recv_timeout(Duration::from_millis(100)) {
                Ok(audio_chunk) => {
                    // CRITICAL: Validate chunk size before processing
                    if audio_chunk.len() > AUDIO_CHUNK_SIZE_LIMIT {
                        warn!("[AudioThread] Received oversized audio chunk: {} samples, skipping", audio_chunk.len());
                        continue;
                    }

                    // CRITICAL: Check session audio size before adding
                    if raw_full_session_audio.len() + audio_chunk.len() > max_session_audio_samples {
                        warn!("[AudioThread] Session audio would exceed limit, removing old data");
                        
                        let keep_size = max_session_audio_samples / 2;
                        if raw_full_session_audio.len() > keep_size {
                            let removed_size = raw_full_session_audio.len() - keep_size;
                            raw_full_session_audio.drain(0..removed_size);
                            memory_tracker.remove_buffer_usage(removed_size * std::mem::size_of::<f32>());
                        }
                    }

                    // Track memory for new chunk
                    memory_tracker.add_buffer_usage(audio_chunk.len() * std::mem::size_of::<f32>());
                    
                    // Add to buffers
                    raw_full_session_audio.extend_from_slice(&audio_chunk);
                    audio_buffer_for_whisper_chunks.extend_from_slice(&audio_chunk);

                    // Process partial transcriptions with bounds checking
                    if audio_buffer_for_whisper_chunks.len() >= partial_buffer_capacity_samples {
                        info!("[AudioThread] Processing partial transcription. Buffer size: {} samples, threshold: {} samples",
                              audio_buffer_for_whisper_chunks.len(), partial_buffer_capacity_samples);
                        
                        Self::process_partial_transcription(
                            &mut whisper_state,
                            &audio_buffer_for_whisper_chunks,
                            actual_rate,
                            chunk_resampler.as_mut(),
                            &app_handle,
                        );
                        
                        // Clear and track memory cleanup
                        let cleared_size = audio_buffer_for_whisper_chunks.len() * std::mem::size_of::<f32>();
                        audio_buffer_for_whisper_chunks.clear();
                        memory_tracker.remove_buffer_usage(cleared_size);
                    }
                }
                Err(_) => {
                    // Timeout - continue processing
                }
            }
        }

        info!("[AudioThread] Worker thread finished with memory cleanup");
        
        // Final cleanup
        let final_cleanup_size = (audio_buffer_for_whisper_chunks.len() + raw_full_session_audio.len()) 
            * std::mem::size_of::<f32>();
        memory_tracker.remove_buffer_usage(final_cleanup_size);
    }

    fn process_partial_transcription<R: Runtime>(
        whisper_state: &mut whisper_rs::WhisperState,
        audio_buffer: &[f32],
        actual_rate: u32,
        resampler: Option<&mut SincFixedIn<f32>>,
        app_handle: &AppHandle<R>,
    ) {
        let audio_to_transcribe = if actual_rate != WHISPER_SAMPLE_RATE {
            if let Some(r) = resampler {
                match r.process(&[audio_buffer.to_vec()], None) {
                    Ok(mut resampled) if !resampled.is_empty() => resampled.remove(0),
                    _ => return,
                }
            } else {
                return;
            }
        } else {
            audio_buffer.to_vec()
        };

        if audio_to_transcribe.is_empty() {
            return;
        }

        let mut params = FullParams::new(whisper_rs::SamplingStrategy::Greedy { best_of: 0 });
        params.set_n_threads(4);
        params.set_print_special(false);
        params.set_print_progress(false);
        params.set_print_realtime(false);
        params.set_print_timestamps(false);

        match whisper_state.full(params, &audio_to_transcribe[..]) {
            Ok(_) => {
                let num_segments = whisper_state.full_n_segments().unwrap_or(0);
                let mut partial_text = String::new();
                for i in 0..num_segments {
                    if let Ok(segment) = whisper_state.full_get_segment_text(i) {
                        partial_text.push_str(&segment);
                    }
                }
                if !partial_text.is_empty() {
                    let _ = app_handle.emit("voice-transcription:partial-result",
                        serde_json::json!({ "text": partial_text }));
                }
            }
            Err(e) => tracing::error!("[AudioThread] Error transcribing partial chunk: {:?}", e),
        }
    }

    fn process_final_audio<R: Runtime>(
        whisper_state: &mut whisper_rs::WhisperState,
        audio_buffer: &[f32],
        raw_full_session_audio: &[f32],
        actual_rate: u32,
        mut resampler: Option<&mut SincFixedIn<f32>>,
        app_handle: &AppHandle<R>,
        last_buffer_arc: &Arc<Mutex<Option<Vec<f32>>>>,
    ) {
        // Process any remaining audio in buffer first
        if !audio_buffer.is_empty() {
            Self::process_partial_transcription(
                whisper_state,
                audio_buffer,
                actual_rate,
                resampler.as_deref_mut(),
                app_handle,
            );
        }

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

            let mut params = FullParams::new(whisper_rs::SamplingStrategy::BeamSearch { beam_size: 5, patience: 1.0 });
            params.set_temperature(0.0);

            match whisper_state.full(params, &audio_for_transcription[..]) {
                Ok(_) => {
                    let num_segments = whisper_state.full_n_segments().unwrap_or(0);
                    info!("[AudioThread] Transcription completed. Number of segments: {}", num_segments);

                    let mut transcription_text = String::new();
                    for i in 0..num_segments {
                        if let Ok(segment) = whisper_state.full_get_segment_text(i) {
                            info!("[AudioThread] Segment {}: '{}'", i, segment);
                            transcription_text.push_str(&segment);
                        }
                    }

                    info!("[AudioThread] Final transcription result: '{}'", transcription_text);
                    let _ = app_handle.emit("voice-transcription:final-result",
                        serde_json::json!({ "text": transcription_text }));
                    let _ = app_handle.emit("voice-transcription:dictation-stopped", ());
                }
                Err(e) => {
                    tracing::error!("Final transcription failed: {:?}", e);
                    // Emit transcription error event for backend to handle
                    let _ = app_handle.emit("voice-transcription:error", serde_json::json!({
                        "type": "transcription_failed",
                        "message": format!("Final transcription failed: {:?}", e)
                    }));
                    let _ = app_handle.emit("voice-transcription:dictation-stopped", ());
                }
            }
        } else {
            info!("[AudioThread] No audio to transcribe (empty buffer)");
            let _ = app_handle.emit("voice-transcription:dictation-stopped", ());
        }
    }

    pub fn stop_dictation(&mut self) -> Result<bool> {
        if !self.is_dictating {
            return Ok(false);
        }

        self.is_dictating = false;

        if let Some((thread_handle, control_tx)) = self.audio_thread.take() {
            let _ = control_tx.send(AudioThreadMessage::Stop);

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

    pub fn toggle_dictation<R: Runtime + 'static>(&mut self, app_handle: AppHandle<R>) -> Result<bool> {
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
        let rate = self.actual_recording_sample_rate.lock().ok()?.clone()?;
        Some((buffer, rate))
    }
}

// Ensure the controller is thread-safe
unsafe impl Send for VoiceController {}
unsafe impl Sync for VoiceController {}

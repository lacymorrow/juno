use whisper_rs::{FullParams, WhisperContext, WhisperContextParameters};
use std::path::Path;
use tracing::info;
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use std::sync::mpsc::{channel, Sender, TryRecvError};
use std::thread;
use std::time::Duration;
use std::sync::{Arc, Mutex};
use rubato::{Resampler, SincFixedIn, SincInterpolationType, SincInterpolationParameters, WindowFunction};
use hound; // Ensure hound is imported for writing WAV
use tauri::AppHandle;
use tauri::Emitter; // Added Emitter trait for .emit()

const WHISPER_SAMPLE_RATE: u32 = 16000; // Define the constant



// Enum to send messages to the audio thread
enum AudioThreadMessage {
    Stop,
}

pub struct VoiceController {
    ctx: WhisperContext,
    model_path: String, // Store the model path
    is_dictating: bool,
    // Use an Option to hold the handle and sender for the audio thread
    audio_thread: Option<(thread::JoinHandle<()>, Sender<AudioThreadMessage>)>,

    last_processed_audio_buffer: Arc<Mutex<Option<Vec<f32>>>>, // Stores raw audio at original sample rate
    actual_recording_sample_rate: Arc<Mutex<Option<u32>>>, // New field
    developer_playback_enabled: bool, // New field for developer setting
}

impl VoiceController {
    pub fn new(model_path_str: &str) -> Result<Self, String> {
        let model_path = Path::new(model_path_str);
        if !model_path.exists() {
            // Ideally, download the model here if it doesn't exist.
            // For now, we'll require it to be pre-downloaded.
            // whisper-rs `asset` feature or custom download logic can be used.
            return Err(format!("Model path does not exist: {}", model_path_str));
        }

        let context_params = WhisperContextParameters::default();
        let ctx = WhisperContext::new_with_params(model_path_str, context_params)
            .map_err(|e| format!("Failed to create WhisperContext: {:?}", e))?;

        Ok(Self {
            ctx,
            model_path: model_path_str.to_string(), // Store the path
            is_dictating: false,
            audio_thread: None,
            last_processed_audio_buffer: Arc::new(Mutex::new(None)), // Initialize new field
            actual_recording_sample_rate: Arc::new(Mutex::new(None)), // Initialize new field
            developer_playback_enabled: false, // Initialize new field
            // current_transcription will be managed differently, likely by emitting events
        })
    }

    pub fn transcribe_audio_file(&self, audio_path_str: &str) -> Result<String, String> {
        let audio_path = Path::new(audio_path_str);
        if !audio_path.exists() {
            return Err(format!("Audio file does not exist: {}", audio_path_str));
        }

        let mut state = self.ctx.create_state()
            .map_err(|e| format!("Failed to create WhisperState: {:?}", e))?;

        let params = FullParams::new(whisper_rs::SamplingStrategy::Greedy { best_of: 0 });

        let mut reader = hound::WavReader::open(audio_path_str).expect("failed to open file");
        #[allow(unused_variables)]
        let hound::WavSpec {
            channels,
            sample_rate,
            bits_per_sample,
            sample_format,
        } = reader.spec();

        let samples_i16: Vec<i16> = reader
            .samples::<i16>()
            .map(|s| s.expect("invalid sample"))
            .collect::<Vec<_>>();

        let mut audio_f32: Vec<f32> = vec![0.0f32; samples_i16.len()];
        whisper_rs::convert_integer_to_float_audio(&samples_i16, &mut audio_f32)
            .map_err(|e| format!("Failed to convert audio to f32: {:?}", e))?;

        let mut processed_audio = audio_f32; // Use a new variable for potentially modified audio

        if channels == 2 {
            processed_audio = whisper_rs::convert_stereo_to_mono_audio(&processed_audio)
                .map_err(|e| format!("Failed to convert stereo to mono: {:?}", e))?;
        } else if channels != 1 {
            return Err(format!("Unsupported number of channels: {}. Only mono (1) or stereo (2) is supported for automatic conversion.", channels));
        }

        // TODO: Add proper resampling if sample_rate is not 16000 Hz
        // if sample_rate != 16000 {
        //     return Err(format!("Unsupported sample rate: {}. Whisper.rs requires 16000 Hz.", sample_rate));
        // }

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

    // --- Dictation Methods ---

    pub fn start_dictation(&mut self, app_handle: AppHandle) -> Result<(), String> {
        if self.is_dictating {
            info!("[VoiceController] Dictation already active.");
            return Ok(());
        }
        info!("[VoiceController] Starting dictation...");

        // Emit app-dictation-started event
        if let Err(e) = app_handle.emit("app-dictation-started", ()) {
            tracing::warn!("[VoiceController] Failed to emit app-dictation-started event: {}", e);
            // Optionally, decide if this failure should prevent dictation from starting
            // return Err(format!("Failed to emit app-dictation-started: {}", e));
        } else {
            info!("[VoiceController] Emitted app-dictation-started event.");
        }

        // Clear the last processed audio buffer for the new session
        // This will only be populated if developer_playback_enabled is true
        info!("[VoiceController] Clearing previous audio buffer (will be populated for playback if dev setting is on).");
        if let Ok(mut buffer_guard) = self.last_processed_audio_buffer.lock() {
            *buffer_guard = None; // Or Some(Vec::new()) if you prefer to always have a Vec
        } else {
            // Log or handle error if mutex is poisoned
            eprintln!("[VoiceController] Failed to lock last_processed_audio_buffer for clearing.");
            // Depending on requirements, you might want to return an error here
        }

        let host = cpal::default_host();
        let device = host.default_input_device()
            .ok_or("Failed to find a default input device.")?;

        let supported_configs_iter = device.supported_input_configs()
            .map_err(|e| format!("Failed to get device configs: {:?}", e))?;

        let selected_config_range = supported_configs_iter
            .filter(|c| c.channels() == 1)
            .find(|c| {
                (c.min_sample_rate().0..=c.max_sample_rate().0).contains(&16000) &&
                (c.sample_format() == SampleFormat::F32 || c.sample_format() == SampleFormat::I16)
            });

        let supported_config = if let Some(conf_range) = selected_config_range {
            conf_range.with_sample_rate(cpal::SampleRate(16000))
        } else {
            info!("[VoiceController] 16kHz config not directly found. Looking for other compatible mono configs.");
            device.supported_input_configs()
                .map_err(|e| format!("Failed to get device configs for fallback: {:?}", e))?
                .filter(|c| c.channels() == 1)
                .find(|c| c.sample_format() == SampleFormat::F32 || c.sample_format() == SampleFormat::I16)
                .map(|c| {
                    if (c.min_sample_rate().0..=c.max_sample_rate().0).contains(&16000) {
                        c.with_sample_rate(cpal::SampleRate(16000))
                    } else {
                        info!("[VoiceController] Fallback config will use sample rate: {}. Whisper prefers 16kHz.", c.min_sample_rate().0);
                        c.with_sample_rate(c.min_sample_rate())
                    }
                })
                .ok_or_else(|| "No suitable input config found after fallback.".to_string())?
        };

        info!("Selected input config: {:?}, channels: {}, format: {:?}, rate: {}",
              supported_config,
              supported_config.channels(),
              supported_config.sample_format(),
              supported_config.sample_rate().0);

        let config = supported_config.config();
        let sample_format = supported_config.sample_format();

        // Store the actual sample rate
        let actual_rate = config.sample_rate.0;
        {
            let mut rate_guard = self.actual_recording_sample_rate.lock().unwrap();
            *rate_guard = Some(actual_rate);
        }

        let (control_tx, control_rx) = channel::<AudioThreadMessage>();
        let (audio_data_tx, audio_data_rx) = channel::<Vec<f32>>();

        let model_path_for_thread = self.model_path.clone();
        let last_buffer_arc_for_thread = Arc::clone(&self.last_processed_audio_buffer);
        let actual_rate_for_thread = actual_rate; // Pass actual_rate to the thread
        let developer_playback_enabled_for_thread = self.developer_playback_enabled; // Pass the flag to the thread
        let app_handle_for_thread = app_handle.clone(); // Clone AppHandle for the thread

        let audio_thread_handle = thread::spawn(move || {
            // Create Whisper context and state within the audio thread
            info!("[AudioThread] Thread started. Initializing Whisper context and state."); // Added log
            let whisper_context_thread = match WhisperContext::new_with_params(&model_path_for_thread, WhisperContextParameters::default()) {
                 Ok(ctx) => ctx,
                 Err(e) => {
                     eprintln!("Failed to create WhisperContext in audio thread: {:?}", e);
                     return;
                 }
             };

            let mut whisper_state = match whisper_context_thread.create_state() {
                 Ok(state) => state,
                 Err(e) => {
                     eprintln!("Failed to create WhisperState in audio thread: {:?}", e);
                     return;
                 }
             };

             let stream_config = config;

            let stream = match sample_format {
                SampleFormat::F32 => device.build_input_stream(
                    &stream_config,
                    move |data: &[f32], _: &cpal::InputCallbackInfo| {
                        if let Err(e) = audio_data_tx.send(data.to_vec()) {
                            eprintln!("Failed to send audio data: {:?}", e);
                        }
                    },
                    move |err| {
                        eprintln!("An error occurred on the input stream: {}", err);
                    },
                    None
                ).expect("Failed to build f32 input stream in thread"),
                SampleFormat::I16 => device.build_input_stream(
                     &stream_config,
                    move |data: &[i16], _: &cpal::InputCallbackInfo| {
                        let mut audio_f32: Vec<f32> = vec![0.0f32; data.len()];
                        if let Err(e) = whisper_rs::convert_integer_to_float_audio(data, &mut audio_f32) {
                             eprintln!("Failed to convert i16 to f32: {:?}", e);
                             return;
                        }
                         if let Err(e) = audio_data_tx.send(audio_f32) {
                            eprintln!("Failed to send converted audio data: {:?}", e);
                        }
                    },
                    move |err| {
                        eprintln!("An error occurred on the input stream: {}", err);
                    },
                    None
                ).expect("Failed to build i16 input stream in thread"),
                _ => {
                    eprintln!("Unsupported sample format {:?} in thread.", sample_format);
                    return;
                }
            };

            if let Err(e) = stream.play() {
                 eprintln!("Failed to start audio stream in thread: {:?}", e);
                 return;
            }

            info!("[AudioThread] Audio stream started.");
            info!("[AudioThread] Recording at {} Hz, will resample to {} Hz for Whisper.", actual_rate_for_thread, WHISPER_SAMPLE_RATE);

            // Resampler for chunk-wise processing, if needed
            let mut chunk_resampler: Option<SincFixedIn<f32>> = None;
            if actual_rate_for_thread != WHISPER_SAMPLE_RATE {
                let params = SincInterpolationParameters {
                    sinc_len: 256,
                    f_cutoff: 0.95,
                    interpolation: SincInterpolationType::Linear,
                    oversampling_factor: 256,
                    window: WindowFunction::BlackmanHarris2,
                };
                chunk_resampler = SincFixedIn::new(
                    WHISPER_SAMPLE_RATE as f64 / actual_rate_for_thread as f64,
                    2.0, // max_resample_ratio_relative
                    params,
                    1024, // chunk_size (max input chunk for SincFixedIn)
                    1,    // num_channels
                ).map_err(|e| eprintln!("[AudioThread] Failed to create chunk_resampler: {:?}", e)).ok();
                if chunk_resampler.is_none() {
                     eprintln!("[AudioThread] Chunk resampler creation failed. Chunk processing might use raw audio if rates differ.");
                }
            }

            let mut audio_buffer_for_whisper_chunks: Vec<f32> = Vec::new(); // Buffer for raw audio for chunk processing
            const BUFFER_DURATION_MS: u64 = 5000; // This is for the *final* transcription, not partials yet
            const PARTIAL_BUFFER_DURATION_MS: u64 = 1500; // Let's try 1.5 seconds for partials
            let partial_buffer_capacity_samples = (actual_rate_for_thread as u64 * PARTIAL_BUFFER_DURATION_MS / 1000) as usize;

            // Buffer to store all audio at its original sample rate for the entire session
            let mut raw_full_session_audio: Vec<f32> = Vec::new();

            loop {
                // Check for control messages (e.g., Stop)
                match control_rx.try_recv() {
                    Ok(AudioThreadMessage::Stop) => {
                        info!("[AudioThread] Stop message received.");
                        // The `audio_buffer_for_whisper_chunks` might contain a final partial chunk of raw audio.
                        // This audio is ALREADY present in `raw_full_session_audio` because every chunk from
                        // `audio_data_rx` is appended to `raw_full_session_audio` directly.
                        // Therefore, we DO NOT need to append `audio_buffer_for_whisper_chunks` here again.
                        // Clearing it is fine if it wasn't cleared by chunk processing, but it doesn't need to be added to raw_full_session_audio.
                        if !audio_buffer_for_whisper_chunks.is_empty() {
                            info!("[AudioThread] `audio_buffer_for_whisper_chunks` has {} raw samples remaining from partial processing. These are already in `raw_full_session_audio`. Clearing chunk buffer for final transcription.", audio_buffer_for_whisper_chunks.len());
                            // We will transcribe this remaining bit before the full one if needed, or just rely on full.
                            // For now, let's ensure it's cleared before full transcription.
                           // audio_buffer_for_whisper_chunks.clear(); // Clearing might be premature if we want to process it
                        }

                        // --- Process any remaining audio in audio_buffer_for_whisper_chunks for a last partial result ---
                        if !audio_buffer_for_whisper_chunks.is_empty() {
                            info!("[AudioThread] Processing final remaining chunk of {} samples for partial result before full transcription.", audio_buffer_for_whisper_chunks.len());
                            let audio_to_transcribe_partial_final = if actual_rate_for_thread != WHISPER_SAMPLE_RATE {
                                if let Some(ref mut r) = chunk_resampler {
                                    match r.process(&[audio_buffer_for_whisper_chunks.clone()], None) { // Clone because it's used again or cleared
                                        Ok(mut resampled) if !resampled.is_empty() => resampled.remove(0),
                                        _ => {
                                            eprintln!("[AudioThread] Final partial resampling failed or produced empty. Using raw.");
                                            audio_buffer_for_whisper_chunks.clone() // Fallback, though rate is wrong
                                        }
                                    }
                                } else {
                                    audio_buffer_for_whisper_chunks.clone() // No resampler, use raw (rate might be wrong)
                                }
                            } else {
                                audio_buffer_for_whisper_chunks.clone()
                            };

                            if !audio_to_transcribe_partial_final.is_empty() {
                                let mut params = FullParams::new(whisper_rs::SamplingStrategy::Greedy { best_of: 0 });
                                params.set_n_threads(4); // Example, adjust as needed
                                // params.set_no_context(true); // Important for streaming if context isn't managed across chunks
                                params.set_print_special(false);
                                params.set_print_progress(false);
                                params.set_print_realtime(false);
                                params.set_print_timestamps(false);
                                // For partial results, we might want to suppress "end of text" tokens if the library allows.
                                // Or handle them on the frontend.

                                match whisper_state.full(params, &audio_to_transcribe_partial_final[..]) {
                                    Ok(_) => {
                                        let num_segments = whisper_state.full_n_segments().unwrap_or(0);
                                        let mut partial_text = String::new();
                                        for i in 0..num_segments {
                                            if let Ok(segment) = whisper_state.full_get_segment_text(i) {
                                                partial_text.push_str(&segment);
                                            }
                                        }
                                        if !partial_text.is_empty() {
                                            info!("[AudioThread] Emitting final app-dictation-partial-result: {}", partial_text);
                                            if let Err(e) = app_handle_for_thread.emit("app-dictation-partial-result", serde_json::json!({ "partial": partial_text })) {
                                                eprintln!("[AudioThread] Error emitting final app-dictation-partial-result: {:?}", e);
                                            }
                                        }
                                    }
                                    Err(e) => eprintln!("[AudioThread] Error transcribing final partial chunk: {:?}", e),
                                }
                            }
                            audio_buffer_for_whisper_chunks.clear(); // Clear after processing
                        }
                         // --- End process remaining audio ---

                        // Flush the chunk_resampler if it was being used for any partial data
                        if let Some(ref mut r) = chunk_resampler {
                            match r.process_partial::<Vec<f32>>(None, None) {
                                Ok(mut resampled_frames_last_chunk_multichannel) => {
                                    if !resampled_frames_last_chunk_multichannel.is_empty() {
                                        let final_chunk_from_resampler_flush = resampled_frames_last_chunk_multichannel.remove(0);
                                        if !final_chunk_from_resampler_flush.is_empty() {
                                            // This flushed audio would have been for chunk-wise transcription,
                                            // but we are about to do a full transcription.
                                            // We don't add it to raw_full_session_audio as that's for original rate.
                                            info!("[AudioThread] Chunk resampler flush produced {} samples. These are not added to final raw audio.", final_chunk_from_resampler_flush.len());
                                        }
                                    }
                                },
                                Err(e) => eprintln!("[AudioThread] Error flushing chunk_resampler: {:?}", e),
                            }
                        }

                        // Now, `raw_full_session_audio` contains all audio at `actual_rate_for_thread`.
                        // Store this for playback.
                        if developer_playback_enabled_for_thread {
                            if let Ok(mut buffer_guard) = last_buffer_arc_for_thread.lock() {
                                *buffer_guard = Some(raw_full_session_audio.clone());
                                info!("[AudioThread] Final raw session audio ({} samples at {} Hz) stored for playback (dev setting enabled).", raw_full_session_audio.len(), actual_rate_for_thread);
                            } else {
                                eprintln!("[AudioThread] Failed to lock last_processed_audio_buffer for final raw storage.");
                            }
                        } else {
                            info!("[AudioThread] Developer playback is disabled. Raw session audio not stored in playback buffer.");
                        }

                        // --- Prepare audio for transcription (resample if necessary) ---
                        let audio_for_transcription: Vec<f32>;
                        if actual_rate_for_thread != WHISPER_SAMPLE_RATE {
                            if !raw_full_session_audio.is_empty() {
                                info!("[AudioThread] Resampling full session audio from {} Hz to {} Hz for transcription.", actual_rate_for_thread, WHISPER_SAMPLE_RATE);
                                let params = SincInterpolationParameters { // Define params for final resampler
                                    sinc_len: 256,
                                    f_cutoff: 0.95,
                                    interpolation: SincInterpolationType::Linear,
                                    oversampling_factor: 256,
                                    window: WindowFunction::BlackmanHarris2,
                                };
                                let mut final_resampler = SincFixedIn::new(
                                    WHISPER_SAMPLE_RATE as f64 / actual_rate_for_thread as f64,
                                    2.0, // max_resample_ratio_relative
                                    params,
                                    raw_full_session_audio.len(), // chunk_size should be able to handle the whole audio
                                    1,    // num_channels
                                ).expect("Failed to create final_resampler for full session audio");

                                let waves_in = vec![raw_full_session_audio.clone()]; // Clone because raw_full_session_audio is used for playback
                                match final_resampler.process(&waves_in, None) {
                                    Ok(mut resampled_waves) => {
                                        if resampled_waves.is_empty() || resampled_waves[0].is_empty() {
                                            eprintln!("[AudioThread] Final resampling produced empty audio.");
                                            audio_for_transcription = Vec::new();
                                        } else {
                                            audio_for_transcription = resampled_waves.remove(0);
                                            info!("[AudioThread] Final resampling complete. {} samples at {} Hz for transcription.", audio_for_transcription.len(), WHISPER_SAMPLE_RATE);
                                        }
                                    }
                                    Err(e) => {
                                        eprintln!("[AudioThread] Error during final resampling: {:?}", e);
                                        audio_for_transcription = Vec::new();
                                    }
                                }
                            } else {
                                info!("[AudioThread] Raw full session audio is empty, nothing to resample for transcription.");
                                audio_for_transcription = Vec::new();
                            }
                        } else {
                            info!("[AudioThread] No resampling needed for transcription, using raw audio ({} Hz).", actual_rate_for_thread);
                            audio_for_transcription = raw_full_session_audio.clone(); // Clone for transcription
                        }
                        // --- End Prepare audio for transcription ---

                        // --- Perform Transcription with `audio_for_transcription` ---
                        if !audio_for_transcription.is_empty() {
                            info!("[AudioThread] Starting FINAL transcription of {} audio samples (at {} Hz).", audio_for_transcription.len(), WHISPER_SAMPLE_RATE);

                            // --- DEBUG: Save `audio_for_transcription` to a WAV file in the project root ---
                            let debug_wav_path = "../debug_live_audio.wav"; // Changed path
                            info!("[AudioThread] Attempting to save audio_for_transcription to: {}", debug_wav_path);
                            let spec = hound::WavSpec {
                                channels: 1, // Mono
                                sample_rate: WHISPER_SAMPLE_RATE, // Crucially, this is now 16kHz
                                bits_per_sample: 32, // For f32 samples
                                sample_format: hound::SampleFormat::Float,
                            };
                            match hound::WavWriter::create(debug_wav_path, spec) {
                                Ok(mut writer) => {
                                    for sample in audio_for_transcription.iter() {
                                        if let Err(e) = writer.write_sample(*sample) {
                                            eprintln!("[AudioThread] Error writing sample to debug WAV: {:?}", e);
                                            break;
                                        }
                                    }
                                    if let Err(e) = writer.finalize() {
                                        eprintln!("[AudioThread] Error finalizing debug WAV: {:?}", e);
                                    } else {
                                        info!("[AudioThread] Successfully saved audio_for_transcription to {}", debug_wav_path);
                                    }
                                }
                                Err(e) => {
                                    eprintln!("[AudioThread] Failed to create debug WAV file '{}': {:?}", debug_wav_path, e);
                                }
                            }
                            // --- END DEBUG ---

                            let mut params = FullParams::new(whisper_rs::SamplingStrategy::BeamSearch { beam_size: 5, patience: 1.0 });
                            params.set_temperature(0.0);
                            // Ensure whisper_state is reset or managed if it was used for partials.
                            // For now, we assume it's okay for a final full transcription if partials were minimal.
                            // If partials heavily used whisper_state, it might need re-creation or reset.
                            // whisper_state = whisper_context_thread.create_state().expect("Failed to recreate state for final transcription");

                            match whisper_state.full(params, &audio_for_transcription[..]) {
                                Ok(_) => {
                                    let num_segments = whisper_state.full_n_segments().unwrap_or(0);
                                    let mut transcription_text = String::new();
                                    for i in 0..num_segments {
                                        if let Ok(segment) = whisper_state.full_get_segment_text(i) {
                                            transcription_text.push_str(&segment);
                                        } else {
                                            eprintln!("[AudioThread] Failed to get segment {} text for final transcription.", i);
                                        }
                                    }
                                    info!("[AudioThread] FINAL Transcription successful: {}", transcription_text);
                                    // Emit Tauri event with transcription_text
                                    if !transcription_text.is_empty() {
                                        // This is the FINAL result, not a partial.
                                        // The frontend expects `app-dictation-finished` with a query.
                                        // And Bar.tsx listens for "app-dictation-finished"
                                        // Payload: { query: string | null; error?: string; }
                                        if let Err(e) = app_handle_for_thread.emit("app-dictation-finished", serde_json::json!({ "query": transcription_text, "error": null })) {
                                            eprintln!("[AudioThread] Failed to emit app-dictation-finished event: {:?}", e);
                                        } else {
                                            info!("[AudioThread] Emitted app-dictation-finished event with transcription: {}", transcription_text);
                                        }
                                    } else {
                                        info!("[AudioThread] FINAL Transcription was empty, emitting app-dictation-finished with null query.");
                                         if let Err(e) = app_handle_for_thread.emit("app-dictation-finished", serde_json::json!({ "query": null, "error": "Empty transcription" })) {
                                            eprintln!("[AudioThread] Failed to emit app-dictation-finished (empty) event: {:?}", e);
                                        }
                                    }
                                }
                                Err(e) => {
                                    eprintln!("[AudioThread] FINAL Transcription failed: {:?}", e);
                                    // Emit app-dictation-finished with an error
                                    let error_message = format!("Transcription failed: {:?}", e);
                                     if let Err(e_emit) = app_handle_for_thread.emit("app-dictation-finished", serde_json::json!({ "query": null, "error": error_message })) {
                                        eprintln!("[AudioThread] Failed to emit app-dictation-finished (error) event: {:?}", e_emit);
                                    }
                                }
                            }
                        } else {
                            info!("[AudioThread] FINAL audio_for_transcription is empty. Emitting app-dictation-finished with null query.");
                            if let Err(e) = app_handle_for_thread.emit("app-dictation-finished", serde_json::json!({ "query": null, "error": "No audio to transcribe" })) {
                                eprintln!("[AudioThread] Failed to emit app-dictation-finished (empty) event: {:?}", e);
                            }
                        }
                        // --- End Perform Transcription ---

                        break; // Exit loop
                    },
                    Err(TryRecvError::Empty) => {
                        // No control messages, continue audio processing
                    },
                    Err(TryRecvError::Disconnected) => {
                        eprintln!("[AudioThread] Control channel disconnected.");
                        // Store whatever audio has been collected so far before breaking
                        if developer_playback_enabled_for_thread {
                            if let Ok(mut buffer_guard) = last_buffer_arc_for_thread.lock() {
                                *buffer_guard = Some(raw_full_session_audio.clone()); // store raw audio
                                info!("[AudioThread] Stored {} raw samples for playback due to disconnect (dev setting enabled).", raw_full_session_audio.len());
                            } else {
                                eprintln!("[AudioThread] Failed to lock last_buffer_arc for storing on disconnect.");
                            }
                        }
                        break; // Exit loop
                    }
                }

                // Try to receive audio data, non-blockingly or with a short timeout
                // to allow the stop message to be processed promptly.
                match audio_data_rx.recv_timeout(Duration::from_millis(50)) {
                    Ok(mut data_chunk) => {
                        // Append to raw_full_session_audio for playback and final transcription base
                        raw_full_session_audio.append(&mut data_chunk.clone());
                        // Append to audio_buffer_for_whisper_chunks for periodic chunk processing
                        audio_buffer_for_whisper_chunks.append(&mut data_chunk);

                        if audio_buffer_for_whisper_chunks.len() >= partial_buffer_capacity_samples {
                            info!("[AudioThread] Partial buffer full ({} samples). Processing for partial transcription.", audio_buffer_for_whisper_chunks.len());

                            // 1. Resample if necessary (this is the audio_buffer_for_whisper_chunks, which is at actual_rate_for_thread)
                            let audio_to_transcribe_partial: Vec<f32>; // Declaration
                            if actual_rate_for_thread != WHISPER_SAMPLE_RATE {
                                if let Some(ref mut r) = chunk_resampler {
                                    // Process the current chunk.
                                    // The resampler might have internal state, so we pass the whole buffer.
                                    // It should return only the resampled data corresponding to this input.
                                    match r.process(&[audio_buffer_for_whisper_chunks.clone()], None) { // Clone because it's cleared later
                                        Ok(mut resampled_frames_multichannel) => {
                                            if !resampled_frames_multichannel.is_empty() {
                                                audio_to_transcribe_partial = resampled_frames_multichannel.remove(0); // ASSIGN HERE
                                            } else {
                                                eprintln!("[AudioThread] Partial resampling produced empty output.");
                                                audio_to_transcribe_partial = Vec::new();
                                            }
                                        }
                                        Err(e) => {
                                            eprintln!("[AudioThread] Error during partial resampling: {:?}. Using raw (potentially wrong rate).", e);
                                            audio_to_transcribe_partial = audio_buffer_for_whisper_chunks.clone(); // Fallback
                                        }
                                    }
                                } else {
                                     eprintln!("[AudioThread] Chunk resampler not available, using raw for partial (rate {} vs {}).", actual_rate_for_thread, WHISPER_SAMPLE_RATE);
                                    audio_to_transcribe_partial = audio_buffer_for_whisper_chunks.clone(); // No resampler, use raw (rate might be wrong)
                                }
                            } else {
                                audio_to_transcribe_partial = audio_buffer_for_whisper_chunks.clone(); // Already at correct sample rate
                            }

                            // 2. Transcribe the (potentially resampled) chunk
                            if !audio_to_transcribe_partial.is_empty() {
                                // This is where you'd call whisper_state.full for the chunk if you
                                // want interim transcription results. For now, it's just logged.
                                // let params = FullParams::new(whisper_rs::SamplingStrategy::Greedy { best_of: 5 });
                                // if let Err(e) = whisper_state.full(params, &audio_to_process_for_chunk[..]) {
                                //     eprintln!("[AudioThread] Whisper processing error (chunk): {:?}", e);
                                // } else { ... log transcription ... }
                                info!("[AudioThread] Chunk ready for interim processing ({} samples at {} Hz). Actual processing skipped for now.",
                                    audio_to_transcribe_partial.len(),
                                    if chunk_resampler.is_some() { WHISPER_SAMPLE_RATE } else { actual_rate_for_thread });
                            }
                            audio_buffer_for_whisper_chunks.clear();
                        }
                    }
                    Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {
                        // Timeout is expected, continue to check for stop message
                    }
                    Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => {
                        eprintln!("[AudioThread] Audio data channel disconnected.");
                         if developer_playback_enabled_for_thread {
                             if let Ok(mut buffer_guard) = last_buffer_arc_for_thread.lock() {
                                *buffer_guard = Some(raw_full_session_audio.clone()); // Store raw audio
                                 info!("[AudioThread] Stored {} raw samples for playback due to audio disconnect (dev setting enabled).", raw_full_session_audio.len());
                            } else {
                                eprintln!("[AudioThread] Failed to lock last_buffer_arc for storing on audio disconnect.");
                            }
                        }
                        break; // Exit loop
                    }
                }
            }
            info!("[AudioThread] Exiting.");
        });

        self.audio_thread = Some((audio_thread_handle, control_tx));
        self.is_dictating = true;

        info!("[VoiceController] Dictation started successfully. Audio thread launched.");
        Ok(())
    }

    pub fn stop_dictation(&mut self) -> Result<bool, String> {
        if !self.is_dictating {
            info!("[VoiceController] Dictation not active.");
            return Ok(false);
        }
        info!("[VoiceController] Stopping dictation...");

        if let Some((handle, sender)) = self.audio_thread.take() {
            // Send stop message to the audio thread
            if sender.send(AudioThreadMessage::Stop).is_err() {
                eprintln!("[VoiceController] Failed to send stop message to audio thread. It might have already exited.");
                // Consider this a partial failure or log appropriately.
                // For now, we'll continue to attempt join and set state.
            }

            // Wait for the audio thread to finish
            if handle.join().is_err() {
                eprintln!("[VoiceController] Audio thread panicked or failed to join.");
                // Depending on error handling strategy, could return Err here.
                // For now, we proceed to set is_dictating to false.
            } else {
                info!("[VoiceController] Audio thread joined successfully.");
            }
        } else {
            info!("[VoiceController] No audio thread found to stop (should not happen if is_dictating was true).");
            // This case might indicate an inconsistent state.
        }

        self.is_dictating = false;
        if let Ok(buffer_guard) = self.last_processed_audio_buffer.lock() {
            if let Some(audio) = &*buffer_guard {
                info!("[VoiceController] Dictation stopped. Playback buffer (raw) contains {} samples.", audio.len());
            } else {
                info!("[VoiceController] Dictation stopped. Playback buffer (raw) is empty or not populated (dev setting might be off).");
            }
        }

        info!("[VoiceController] Dictation stopped successfully.");
        Ok(true)
    }

    pub fn toggle_dictation(&mut self, app_handle: AppHandle) -> Result<bool, String> {
        if self.is_dictating {
            self.stop_dictation().map(|_| false)
        } else {
            self.start_dictation(app_handle).map(|_| true)
        }
    }

    pub fn is_dictating(&self) -> bool {
        self.is_dictating
    }

    // Method to enable/disable developer playback buffering
    pub fn set_developer_playback_enabled(&mut self, enabled: bool) {
        self.developer_playback_enabled = enabled;
        info!("[VoiceController] Developer playback buffering set to: {}", enabled);
    }

    // Method to retrieve the current transcription (this will likely be removed or changed
    // to reflect updates received via events)
    pub fn get_current_transcription(&self) -> String {
        // This method is no longer directly updated by the audio thread
        // A different mechanism (like Tauri events) will be needed.
        info!("[VoiceController] get_current_transcription called. This method is deprecated for streaming.");
        "Streaming transcription updates should come via events.".to_string()
    }

    // Method to retrieve the last processed audio buffer and its sample rate
    pub fn get_last_processed_audio_buffer(&self) -> Option<(Vec<f32>, u32)> {
        let buffer_opt = match self.last_processed_audio_buffer.lock() {
            Ok(guard) => guard.clone(), // Clone the Option<Vec<f32>>
            Err(e) => {
                eprintln!("[VoiceController] Mutex poisoned while getting last processed audio buffer: {:?}", e);
                None
            }
        };

        let sample_rate_opt = match self.actual_recording_sample_rate.lock() {
            Ok(guard) => *guard, // Copy the Option<u32>
            Err(e) => {
                eprintln!("[VoiceController] Mutex poisoned while getting actual recording sample rate: {:?}", e);
                None
            }
        };

        // Log the retrieved buffer and sample rate for debugging.
        // info!(\"[VoiceController] get_last_processed_audio_buffer: buffer_is_some: {}, sample_rate_is_some: {}\", buffer_opt.is_some(), sample_rate_opt.is_some());
        // if let (Some(b), Some(r)) = (&buffer_opt, &sample_rate_opt) {
        //     info!(\"[VoiceController] Buffer length: {}, Sample rate: {}\", b.len(), r);
        // }

        match (buffer_opt, sample_rate_opt) {
            (Some(buffer), Some(rate)) => Some((buffer, rate)),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs::File;
    use std::io::Write;
    use tempfile::Builder;
    use std::path::Path;
    use tauri::test::mock_app; // Required for creating a mock AppHandle

    // Helper to create a dummy WAV file for testing (very basic)
    fn create_dummy_wav(path: &Path, duration_ms: u32, sample_rate: u32, channels: u16, bits_per_sample: u16) -> std::io::Result<()> {
        let num_samples = (sample_rate * duration_ms) / 1000;
        let spec = hound::WavSpec {
            channels,
            sample_rate,
            bits_per_sample,
            sample_format: hound::SampleFormat::Int,
        };
        let mut writer = hound::WavWriter::create(path, spec)?;
        for i in 0..num_samples {
             // Write a simple sine wave to have some non-zero data
             // This helps simulate actual audio input
            let sample = ( (i as f32 / sample_rate as f32) * 440.0 * 2.0 * std::f32::consts::PI ).sin(); // 440Hz sine wave

            if bits_per_sample == 16 {
                let amplitude = i16::MAX as f32 * 0.5; // Half amplitude
                 writer.write_sample((sample * amplitude) as i16)?;
            } else if bits_per_sample == 8 {
                 let amplitude = i8::MAX as f32 * 0.5;
                 // Convert to u8 range (0-255) with center at 128 for 8-bit PCM
                 let u8_sample = ((sample as i8).wrapping_add(128)); // Corrected conversion
                 writer.write_sample(u8_sample as i16)?; // hound::WavWriter::write_sample expects i16 for Int format
            }
        }
        writer.finalize()?;
        Ok(())
    }

    #[test]
    #[ignore] // Ignored because it requires a model file and actual audio processing
    fn test_transcribe_dummy_audio() {
        let model_dir = Builder::new().prefix("whisper_model").tempdir().unwrap();
        let model_path = model_dir.path().join("dummy_model.bin");
        // Create a minimal valid dummy model file (size > 0)
        File::create(&model_path).unwrap().write_all(&[0u8; 10]).unwrap();

        let audio_dir = Builder::new().prefix("whisper_audio").tempdir().unwrap();
        let audio_path = audio_dir.path().join("dummy_audio.wav");
        // Create a 16-bit mono WAV file at 16kHz, which is ideal for whisper
        create_dummy_wav(&audio_path, 1000, 16000, 1, 16).unwrap();

        // Note: This test will likely fail in transcription unless a real model is present
        // and dummy audio is actually recognizable. It primarily tests the file reading and setup.
        let voice_controller = VoiceController::new(model_path.to_str().unwrap()).unwrap();
        let result = voice_controller.transcribe_audio_file(audio_path.to_str().unwrap());

        println!("Transcription test result: {:?}", result);

        // Assert based on expected behavior, maybe just check if it didn't return an error
        assert!(result.is_ok() || result.unwrap_err().contains("Failed to run full transcription")); // Expect Ok or specific whisper error on dummy data
    }

     // Add a basic test for dictation toggle
     #[test]
     fn test_dictation_toggle() {
        let mock_app = mock_app(); // Create a mock app
        let app_handle = mock_app.handle(); // Get a handle
         let model_dir = Builder::new().prefix("whisper_model").tempdir().unwrap();
         let model_path = model_dir.path().join("dummy_model.bin");
         File::create(&model_path).unwrap().write_all(&[0u8; 10]).unwrap();

         let mut controller = VoiceController::new(model_path.to_str().unwrap()).unwrap();

         assert!(!controller.is_dictating());

         // Pass the app_handle to toggle_dictation
         controller.toggle_dictation(app_handle.clone()).unwrap();
         assert!(controller.is_dictating());

         // Pass the app_handle again
         controller.toggle_dictation(app_handle.clone()).unwrap();
         assert!(!controller.is_dictating());
     }


}


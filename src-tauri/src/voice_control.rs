use whisper_rs::{FullParams, WhisperContext, WhisperContextParameters};
use std::path::Path;
use tracing::info;
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{SampleFormat}; // Removed unused SampleRate, SupportedStreamConfigRange
use std::sync::mpsc::{channel, Sender, TryRecvError};
use std::thread;
use std::time::Duration;
use std::sync::{Arc, Mutex};

// TODO: Define proper error types

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
    // TODO: Add state for recording if needed (e.g., a buffer or file handle)
    // Add state for handling transcription segments
    // Need a way to send transcription updates back to the main thread/frontend
    // This could be a channel, or emitting a Tauri event.
    // For now, we'll need a way to communicate the transcription out.
    // This will be handled by emitting Tauri events later.
    last_processed_audio_buffer: Arc<Mutex<Option<Vec<f32>>>>, // New field
    actual_recording_sample_rate: Arc<Mutex<Option<u32>>>, // New field
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

    pub fn start_dictation(&mut self) -> Result<(), String> {
        if self.is_dictating {
            info!("[VoiceController] Dictation already active.");
            return Ok(());
        }
        info!("[VoiceController] Starting dictation...");

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

        let audio_thread_handle = thread::spawn(move || {
            // Create Whisper context and state within the audio thread
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

            let mut audio_buffer: Vec<f32> = Vec::new();
            const TARGET_BUFFER_DURATION_MS: usize = 1500;
            const SAMPLE_RATE_HZ: usize = 16000;
            const BUFFER_THRESHOLD_SAMPLES: usize = (SAMPLE_RATE_HZ * TARGET_BUFFER_DURATION_MS) / 1000;

            loop {
                let mut stop_received = false;
                match control_rx.try_recv() {
                    Ok(AudioThreadMessage::Stop) => {
                        info!("[AudioThread] Stop signal received. Processing remaining audio...");
                        stop_received = true;
                    }
                    Err(TryRecvError::Empty) => {}
                    Err(TryRecvError::Disconnected) => {
                        info!("[AudioThread] Control sender disconnected. Stopping stream...");
                        stop_received = true;
                    }
                }

                match audio_data_rx.try_recv() {
                    Ok(chunk) => {
                        audio_buffer.extend(chunk);
                    }
                    Err(TryRecvError::Empty) => {
                        if !stop_received {
                            thread::sleep(Duration::from_millis(50));
                        }
                    }
                     Err(TryRecvError::Disconnected) => {
                         info!("[AudioThread] Audio data sender disconnected.");
                         stop_received = true;
                     }
                }

                if !audio_buffer.is_empty() && (audio_buffer.len() >= BUFFER_THRESHOLD_SAMPLES || (stop_received && !audio_buffer.is_empty())) {
                    info!("[AudioThread] Processing audio buffer of size: {} samples.", audio_buffer.len());

                    if let Ok(mut last_buf_opt) = last_buffer_arc_for_thread.lock() {
                        *last_buf_opt = Some(audio_buffer.clone());
                    }

                    let params = FullParams::new(whisper_rs::SamplingStrategy::Greedy { best_of: 5 });
                    let result = whisper_state.full(params, &audio_buffer[..]);
                    match result {
                        Ok(_) => {
                            let num_segments = whisper_state.full_n_segments().unwrap_or(0);
                            if num_segments > 0 {
                                let mut transcription_text = String::new();
                                for i in 0..num_segments {
                                    if let Ok(segment) = whisper_state.full_get_segment_text(i) {
                                        transcription_text.push_str(&segment);
                                    }
                                }
                                if !transcription_text.trim().is_empty() {
                                    info!("Transcription update (in thread): {}", transcription_text);
                                } else {
                                    info!("[AudioThread] Transcription resulted in empty text for this chunk.");
                                }
                            } else {
                                info!("[AudioThread] No segments transcribed for this chunk.");
                            }
                        }
                        Err(e) => {
                            eprintln!("Whisper processing error (in thread): {:?}", e);
                        }
                    }
                    audio_buffer.clear();
                }

                if stop_received {
                    break;
                }
            }

            info!("[AudioThread] Audio thread stopped.");
        });

        self.audio_thread = Some((audio_thread_handle, control_tx));
        self.is_dictating = true;

        info!("[VoiceController] Dictation started.");
        Ok(())
    }

    pub fn stop_dictation(&mut self) -> Result<(), String> {
        if !self.is_dictating {
            info!("[VoiceController] Dictation not active.");
            return Ok(());
        }
        info!("[VoiceController] Stopping dictation...");

        // Send stop signal to the audio thread and wait for it to finish
        if let Some((handle, tx)) = self.audio_thread.take() {
            if let Err(e) = tx.send(AudioThreadMessage::Stop) {
                eprintln!("Failed to send stop signal to audio thread: {:?}", e);
            }
            // Attempt to join the thread, but don't block indefinitely
            // In a real application, you might want a timeout or more graceful shutdown.
            let _timeout = Duration::from_secs(2);
            match handle.join() {
                Ok(_) => info!("[VoiceController] Audio thread joined successfully."),
                Err(e) => eprintln!("[VoiceController] Failed to join audio thread: {:?}", e),
            }
        }

        self.is_dictating = false;

        // TODO: Finalize whisper processing for any remaining buffer (handled in thread loop before exit?)
        // TODO: Emit transcription_finalized event

        info!("[VoiceController] Dictation stopped.");
        Ok(())
    }

    pub fn toggle_dictation(&mut self) -> Result<bool, String> {
        if self.is_dictating {
            self.stop_dictation()?;
        } else {
            self.start_dictation()?;
        }
        Ok(self.is_dictating)
    }

    pub fn is_dictating(&self) -> bool {
        self.is_dictating
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
        let buffer_lock = self.last_processed_audio_buffer.lock().unwrap();
        let rate_lock = self.actual_recording_sample_rate.lock().unwrap();

        if let (Some(buffer), Some(rate)) = ((*buffer_lock).clone(), *rate_lock) {
            if !buffer.is_empty() {
                Some((buffer, rate))
            } else {
                None
            }
        } else {
            None
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
         let model_dir = Builder::new().prefix("whisper_model").tempdir().unwrap();
         let model_path = model_dir.path().join("dummy_model.bin");
         File::create(&model_path).unwrap().write_all(&[0u8; 10]).unwrap();

         let mut controller = VoiceController::new(model_path.to_str().unwrap()).unwrap();

         assert!(!controller.is_dictating());

         controller.toggle_dictation().unwrap();
         assert!(controller.is_dictating());

         controller.toggle_dictation().unwrap();
         assert!(!controller.is_dictating());
     }

     // TODO: Add more sophisticated tests requiring actual audio devices or mocks
}


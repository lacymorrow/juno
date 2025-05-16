use tauri::{State, AppHandle};
use crate::state::AppState;
use crate::voice_control::VoiceController;
use tracing::info;
use std::sync::Arc;
use cpal::traits::{HostTrait, DeviceTrait, StreamTrait};
use rubato::{Resampler, FastFixedIn, PolynomialDegree};
use cpal::{SampleRate, SampleFormat, SupportedStreamConfigRange, StreamError};

#[tauri::command]
pub async fn start_dictation_command(app_state: State<'_, AppState>, app_handle: AppHandle) -> Result<(), String> {
    info!("[Command] start_dictation_command called");
    let voice_controller_arc = app_state.get::<Arc<std::sync::Mutex<VoiceController>>>()
        .ok_or_else(|| "VoiceController not found in AppState".to_string())?;
    let mut voice_controller = voice_controller_arc.lock().map_err(|e| format!("Failed to lock VoiceController: {}", e))?;
    voice_controller.start_dictation(app_handle)
}

#[tauri::command]
pub async fn stop_dictation_command(app_state: State<'_, AppState>, _app_handle: AppHandle) -> Result<(), String> {
    info!("[Command] stop_dictation_command called");
    let voice_controller_arc = app_state.get::<Arc<std::sync::Mutex<VoiceController>>>()
        .ok_or_else(|| "VoiceController not found in AppState".to_string())?;
    let mut voice_controller = voice_controller_arc.lock().map_err(|e| format!("Failed to lock VoiceController: {}", e))?;
    match voice_controller.stop_dictation() {
        Ok(actively_stopped) => {
            if actively_stopped {
                info!("[Command] Dictation actively stopped.");
            } else {
                info!("[Command] stop_dictation_command called, but no active dictation was stopped (already stopped or no thread).");
            }
            Ok(())
        }
        Err(e) => Err(e),
    }
}

#[tauri::command]
pub async fn toggle_dictation_command(app_state: State<'_, AppState>, app_handle: AppHandle) -> Result<bool, String> {
    info!("[Command] toggle_dictation_command called");
    let voice_controller_arc = app_state.get::<Arc<std::sync::Mutex<VoiceController>>>()
        .ok_or_else(|| "VoiceController not found in AppState".to_string())?;
    let mut voice_controller = voice_controller_arc.lock().map_err(|e| format!("Failed to lock VoiceController: {}", e))?;
    voice_controller.toggle_dictation(app_handle)
}

#[tauri::command]
pub async fn get_dictation_status_command(app_state: State<'_, AppState>) -> Result<bool, String> {
    info!("[Command] get_dictation_status_command called");
    let voice_controller_arc = app_state.get::<Arc<std::sync::Mutex<VoiceController>>>()
        .ok_or_else(|| "VoiceController not found in AppState".to_string())?;
    let voice_controller = voice_controller_arc.lock().map_err(|e| format!("Failed to lock VoiceController: {}", e))?;
    Ok(voice_controller.is_dictating())
}

#[tauri::command]
pub async fn set_developer_playback_enabled_command(app_state: State<'_, AppState>, enabled: bool) -> Result<(), String> {
    info!("[Command] set_developer_playback_enabled_command called with: {}", enabled);
    let voice_controller_arc = app_state.get::<Arc<std::sync::Mutex<VoiceController>>>()
        .ok_or_else(|| "VoiceController not found in AppState".to_string())?;
    let mut voice_controller = voice_controller_arc.lock().map_err(|e| format!("Failed to lock VoiceController: {}", e))?;
    voice_controller.set_developer_playback_enabled(enabled);
    Ok(())
}

#[tauri::command]
pub async fn playback_last_audio_chunk(app_state: State<'_, AppState>) -> Result<String, String> {
    info!("[Command] playback_last_audio_chunk called");
    let voice_controller_arc = app_state.get::<Arc<std::sync::Mutex<VoiceController>>>()
        .ok_or_else(|| "VoiceController not found in AppState".to_string())?;

    let audio_data_and_rate_option = {
        let voice_controller = voice_controller_arc.lock().map_err(|e| format!("Failed to lock VoiceController: {}", e))?;
        voice_controller.get_last_processed_audio_buffer()
    };

    if let Some((mut audio_data, actual_input_sample_rate)) = audio_data_and_rate_option {
        if audio_data.is_empty() {
            return Ok("No audio data in the last processed buffer.".to_string());
        }
        info!("[Playback] Original audio: {} Hz, Mono, {} samples.", actual_input_sample_rate, audio_data.len());

        // Diagnostic: Log min/max of initial audio data
        if !audio_data.is_empty() {
            let min_sample = audio_data.iter().fold(f32::MAX, |a, &b| a.min(b));
            let max_sample = audio_data.iter().fold(f32::MIN, |a, &b| a.max(b));
            info!("[Playback Diagnostic] Initial audio data min: {}, max: {}", min_sample, max_sample);
        }

        tokio::task::spawn_blocking(move || -> Result<String, String> {
            let host = cpal::default_host();
            let device = host.default_output_device().ok_or_else(|| "Failed to get default output device.".to_string())?;
            info!("[Playback] Output device: {}", device.name().unwrap_or_else(|_| "Unknown".to_string()));

            let input_sample_rate = actual_input_sample_rate;
            let input_channels = 1u16; // Assuming mono, which VoiceController aims for

            let supported_configs: Vec<SupportedStreamConfigRange> = device.supported_output_configs()
                .map_err(|e| format!("Error querying configs: {:?}", e))?
                .collect();

            if supported_configs.is_empty() {
                return Err("No supported output configs found.".to_string());
            }

            let target_config_exact = supported_configs.iter().find(|c| {
                c.channels() == input_channels &&
                c.sample_format() == SampleFormat::F32 &&
                (c.min_sample_rate().0..=c.max_sample_rate().0).contains(&input_sample_rate)
            });

            let (final_config, needs_resampling, audio_data_needs_stereo_conversion_for_output) =
                if let Some(conf_range) = target_config_exact {
                    (conf_range.with_sample_rate(SampleRate(input_sample_rate)).config(), false, false)
                } else {
                    info!("[Playback] {} Hz F32 Mono not directly supported. Searching for a compatible config.", input_sample_rate);
                    let preferred_rates = [48000u32, 44100u32];
                    let mut best_match: Option<(cpal::StreamConfig, bool, bool)> = None; // config, needs_resampling, needs_stereo_conversion

                    for rate_val in preferred_rates.iter() {
                        if let Some(conf_range) = supported_configs.iter().find(|c| {
                            (c.channels() == 1 || c.channels() == 2) &&
                            c.sample_format() == SampleFormat::F32 &&
                            (c.min_sample_rate().0..=c.max_sample_rate().0).contains(rate_val)
                        }) {
                            let selected_rate = SampleRate(*rate_val);
                            best_match = Some((
                                conf_range.with_sample_rate(selected_rate).config(),
                                selected_rate.0 != input_sample_rate,
                                conf_range.channels() == 2 && input_channels == 1
                            ));
                            break;
                        }
                    }

                    if best_match.is_none() {
                        if let Some(conf_range) = supported_configs.iter().find(|c| {
                            (c.channels() == 1 || c.channels() == 2) && c.sample_format() == SampleFormat::F32
                        }) {
                            let selected_rate = if (conf_range.min_sample_rate().0..=conf_range.max_sample_rate().0).contains(&input_sample_rate) {
                                SampleRate(input_sample_rate)
                            } else {
                                conf_range.min_sample_rate()
                            };
                            best_match = Some((
                                conf_range.with_sample_rate(selected_rate).config(),
                                selected_rate.0 != input_sample_rate,
                                conf_range.channels() == 2 && input_channels == 1
                            ));
                        }
                    }
                    best_match.ok_or_else(|| format!("No suitable F32 output config found. Available: {:?}", supported_configs))?
                };

            info!("[Playback] Selected output config: {:?}. Resampling needed: {}, Mono-to-Stereo needed: {}", final_config, needs_resampling, audio_data_needs_stereo_conversion_for_output);

            if needs_resampling {
                let mut resampler = FastFixedIn::<f32>::new(
                    final_config.sample_rate.0 as f64 / input_sample_rate as f64,
                    1.0,
                    PolynomialDegree::Linear,
                    audio_data.len(),
                    1
                ).map_err(|e| format!("Failed to create FastFixedIn resampler: {}", e))?;

                let waves_in = vec![audio_data];
                let waves_out = resampler.process(&waves_in, None)
                    .map_err(|e| format!("Resampling failed: {}", e))?;
                audio_data = waves_out.into_iter().next().unwrap_or_default();
                info!("[Playback] After resampling: audio_data.len() = {} (expected mono samples at {} Hz)", audio_data.len(), final_config.sample_rate.0);
            }

            if audio_data_needs_stereo_conversion_for_output {
                let mut stereo_data = Vec::with_capacity(audio_data.len() * 2);
                for sample in audio_data.iter() {
                    stereo_data.push(*sample);
                    stereo_data.push(*sample);
                }
                audio_data = stereo_data;
                info!("[Playback] After main stereo conversion: audio_data.len() = {} (expected stereo interleaved samples for {} Hz)", audio_data.len(), final_config.sample_rate.0);
            } else if final_config.channels == 2 && input_channels == 1 && !audio_data_needs_stereo_conversion_for_output {
                // This safeguard block implies a potential logic error if hit, as the primary conversion should have been flagged.
                info!("[Playback] Safeguard: Output config is stereo, input was mono, but main stereo conversion was NOT flagged. Checking audio_data state.");
                // At this point, audio_data should be mono (either original or resampled mono).
                // If audio_data.len() is odd, it cannot be stereo. If it's even, it *could* be stereo from somewhere else, or mono.
                // The crucial thing is that it *should* be mono here if the flags were consistent.
                if audio_data.len() > 0 && (audio_data.len() % (input_channels as usize) == 0) { // Check if it's frame-aligned mono
                    info!("[Playback] Safeguard: Attempting stereo conversion. audio_data.len() = {} (mono samples before)", audio_data.len());
                    let mut stereo_data = Vec::with_capacity(audio_data.len() * 2);
                    for sample in audio_data.iter() {
                        stereo_data.push(*sample);
                        stereo_data.push(*sample);
                    }
                    audio_data = stereo_data;
                    info!("[Playback] After SAFEGUARD stereo conversion: audio_data.len() = {}", audio_data.len());
                } else {
                    info!("[Playback] Safeguard: audio_data.len() = {} is not typical for mono before stereo conversion, or it's empty. Skipping safeguard conversion.", audio_data.len());
                }
            }

            if audio_data.is_empty() {
                return Ok("Audio data became empty after processing. Nothing to play.".to_string());
            }

            info!("[Playback] Finalizing audio data before stream: audio_data.len() = {}, final_config.channels = {}", audio_data.len(), final_config.channels);

            let audio_data_arc = Arc::new(std::sync::Mutex::new(audio_data));
            let position_arc = Arc::new(std::sync::Mutex::new(0usize));

            let stream_data_ref = Arc::clone(&audio_data_arc);
            let stream_pos_ref = Arc::clone(&position_arc);
            let output_channels_count = final_config.channels as usize;

            let stream_playing = Arc::new(std::sync::atomic::AtomicBool::new(true));
            let stream_playing_data_cb = Arc::clone(&stream_playing);
            let stream_playing_err_cb = Arc::clone(&stream_playing);

            let stream_has_errored = Arc::new(std::sync::Mutex::new(None::<StreamError>));
            let stream_has_errored_writer = Arc::clone(&stream_has_errored);
            let first_callback_logged = Arc::new(std::sync::atomic::AtomicBool::new(false)); // Logging flag

            let stream = device.build_output_stream(
                &final_config,
                move |output: &mut [f32], _: &cpal::OutputCallbackInfo| {
                    let data_guard = stream_data_ref.lock().unwrap();
                    let mut pos_guard = stream_pos_ref.lock().unwrap();

                    if !first_callback_logged.swap(true, std::sync::atomic::Ordering::Relaxed) {
                        info!("[Playback Callback - First Call] data_guard.len() = {}, output_channels_count = {}, output.len() = {}", data_guard.len(), output_channels_count, output.len());
                    }

                    let total_frames_in_buffer = if output_channels_count > 0 { data_guard.len() / output_channels_count } else { 0 };
                    let mut all_source_frames_consumed = *pos_guard >= total_frames_in_buffer;

                    for output_frame_idx in 0..(if output_channels_count > 0 { output.len() / output_channels_count } else { 0 }) {
                        if all_source_frames_consumed {
                            // Fill rest of output buffer with silence
                            for ch in 0..output_channels_count {
                                let output_sample_idx = output_frame_idx * output_channels_count + ch;
                                if output_sample_idx < output.len() {
                                   output[output_sample_idx] = 0.0;
                                }
                            }
                        } else {
                            // Copy data for the current frame
                            for ch in 0..output_channels_count {
                                let data_sample_idx = *pos_guard * output_channels_count + ch;
                                let output_sample_idx = output_frame_idx * output_channels_count + ch;

                                if data_sample_idx < data_guard.len() && output_sample_idx < output.len() {
                                    output[output_sample_idx] = data_guard[data_sample_idx];
                                } else if output_sample_idx < output.len() {
                                     output[output_sample_idx] = 0.0; // Should not happen if logic is correct
                                }
                            }
                            *pos_guard += 1; // Move to next frame in source
                            if *pos_guard >= total_frames_in_buffer {
                                all_source_frames_consumed = true;
                            }
                        }
                    }

                    if all_source_frames_consumed {
                        stream_playing_data_cb.store(false, std::sync::atomic::Ordering::SeqCst);
                    }
                },
                move |err| {
                    eprintln!("[Playback] Error in output stream: {:?}", err);
                    let mut err_writer = stream_has_errored_writer.lock().unwrap();
                    *err_writer = Some(err);
                    stream_playing_err_cb.store(false, std::sync::atomic::Ordering::SeqCst); // Use the correct clone
                },
                None // Timeout
            ).map_err(|e| format!("Failed to build output stream: {:?}", e))?;

            stream.play().map_err(|e| format!("Failed to play stream: {:?}", e))?;

            let final_sample_rate = final_config.sample_rate.0;
            let num_frames = {
                let data_guard = audio_data_arc.lock().unwrap();
                if output_channels_count > 0 { data_guard.len() / output_channels_count } else { 0 }
            };

            let duration_ms = if final_sample_rate > 0 && num_frames > 0 {
                (num_frames as f32 / final_sample_rate as f32 * 1000.0) as u64
            } else {
                0
            };

            info!("[Playback] Effective audio duration: {} ms ({} frames at {} Hz, {} channels). Streaming started.", duration_ms, num_frames, final_sample_rate, output_channels_count);

            // Wait for playback to finish or an error to occur
            let mut wait_time_ms = duration_ms + 500; // Add buffer time
            if wait_time_ms == 500 { wait_time_ms = 1000; } // Min wait if duration is 0

            let start_wait = std::time::Instant::now();
            while stream_playing.load(std::sync::atomic::Ordering::SeqCst) && start_wait.elapsed().as_millis() < wait_time_ms as u128 {
                if stream_has_errored.lock().unwrap().is_some() { break; }
                std::thread::sleep(std::time::Duration::from_millis(50));
            }

            if let Some(err) = stream_has_errored.lock().unwrap().take() {
                return Err(format!("Playback stream error: {}", err));
            }

            info!("[Playback] Playback attempt finished.");
            Ok("Playback attempt finished.".to_string())

        }).await.map_err(|e| format!("Playback task error: {}", e))? // Result from spawn_blocking
                 .map_err(|e| format!("Playback internal error: {}",e)) // Result from inner closure

    } else {
        Ok("No audio data has been processed yet.".to_string())
    }
}

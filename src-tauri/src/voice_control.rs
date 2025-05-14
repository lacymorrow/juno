use whisper_rs::{FullParams, WhisperContext, WhisperContextParameters};
use std::path::Path;

// TODO: Define proper error types

pub struct VoiceController {
    ctx: WhisperContext,
    // TODO: Add state for recording if needed
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

        Ok(Self { ctx })
    }

    pub fn transcribe_audio_file(&self, audio_path_str: &str) -> Result<String, String> {
        let audio_path = Path::new(audio_path_str);
        if !audio_path.exists() {
            return Err(format!("Audio file does not exist: {}", audio_path_str));
        }

        let mut state = self.ctx.create_state()
            .map_err(|e| format!("Failed to create WhisperState: {:?}", e))?;

        // Create a params object
        // Note that there are many ways to configure the params object.
        // Refer to the documentation for more information.
        // https://github.com/Gadersd/whisper-rs/blob/master/src/whisper_params.rs#L72
        let params = FullParams::new(whisper_rs::SamplingStrategy::Greedy { best_of: 0 });

        // Edit the params to fit your needs here:
        // params.set_n_threads(1);
        // params.set_translate(true);
        // params.set_language(Some("en"));
        // params.set_print_special(false);
        // params.set_print_progress(false);
        // params.set_print_realtime(false);
        // params.set_print_timestamps(false);

        // Open the audio file.
        let mut reader = hound::WavReader::open(audio_path_str).expect("failed to open file");
        #[allow(unused_variables)]
        let hound::WavSpec {
            channels,
            sample_rate,
            bits_per_sample,
            sample_format,
        } = reader.spec();

        // Convert the audio samples to f32.
        // Assuming a 16-bit signed integer format.
        let samples_i16: Vec<i16> = reader
            .samples::<i16>()
            .map(|s| s.expect("invalid sample"))
            .collect::<Vec<_>>();

        let mut audio_f32: Vec<f32> = vec![0.0f32; samples_i16.len()];
        whisper_rs::convert_integer_to_float_audio(&samples_i16, &mut audio_f32)
            .map_err(|e| format!("Failed to convert audio to f32: {:?}", e))?;

        // whisper-rs requires 16kHz mono audio.
        // We'll need to resample and convert to mono if it's not already.
        // This is a complex step and typically requires a dedicated audio library.
        // For now, we'll assume the input audio is already in the correct format
        // or `whisper-rs` can handle some deviations (which it might for sample rate, but not channels).

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
}


#[cfg(test)]
mod tests {
    use super::*;
    use std::fs::File;
    use std::io::Write;
    use tempfile::Builder;

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
        for _ in 0..num_samples {
            if bits_per_sample == 16 {
                writer.write_sample(0i16)?;
            } else if bits_per_sample == 8 {
                 // For 8-bit, hound expects u8 if SampleFormat::Int is used and bits_per_sample is 8.
                 // However, whisper expects f32, and our conversion path is i16 -> f32.
                 // Sticking to 16-bit for dummy WAV to align with typical audio and whisper examples.
                 // If 8-bit is truly needed, the sample writing and conversion needs adjustment.
                writer.write_sample(0i16)?; // Write as i16 for simplicity, even if spec says 8-bit
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
        File::create(&model_path).unwrap().write_all(b"dummy").unwrap();

        let audio_dir = Builder::new().prefix("whisper_audio").tempdir().unwrap();
        let audio_path = audio_dir.path().join("dummy_audio.wav");
        // Create a 16-bit mono WAV file at 16kHz, which is ideal for whisper
        create_dummy_wav(&audio_path, 1000, 16000, 1, 16).unwrap();

        let controller_result = VoiceController::new(model_path.to_str().unwrap());

        if let Err(e) = &controller_result {
            println!("Model loading failed (as might be expected with a dummy file): {:?}", e);
            // Depending on how robust model loading is, this might be the expected path with a truly dummy file.
            // If whisper-rs tries to parse the model and fails gracefully, this is fine.
            // If it panics, the test setup for the model needs to be more realistic or this test skipped.
            return;
        }

        let controller = controller_result.unwrap();
        let result = controller.transcribe_audio_file(audio_path.to_str().unwrap());

        // With silent audio, Whisper should produce an empty or near-empty transcription.
        assert!(result.is_ok(), "Transcription failed: {:?}", result.err());
        let transcript = result.unwrap();
        // Allow for whisper sometimes outputting a newline or space for silence.
        assert!(transcript.trim().is_empty(), "Expected empty transcript for silence, got: {}", transcript);
    }
}

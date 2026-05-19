use std::sync::Arc;
use whisper_rs::{FullParams, WhisperContext};
use crate::engine::{TranscriptionEngine, TranscriptionSession};
use crate::utils::filter_transcription_text;

/// STT engine backed by whisper-rs (ggml). Wraps a shared `Arc<WhisperContext>`
/// so two controllers share the same loaded model weights without a second allocation.
pub struct WhisperEngine {
    ctx: Arc<WhisperContext>,
}

// WhisperContext is immutable model weights accessed through an opaque C pointer.
// The library guarantees it is safe to use from multiple threads simultaneously.
unsafe impl Send for WhisperEngine {}
unsafe impl Sync for WhisperEngine {}

impl WhisperEngine {
    pub fn new(ctx: Arc<WhisperContext>) -> Self {
        Self { ctx }
    }
}

impl TranscriptionEngine for WhisperEngine {
    fn name(&self) -> &'static str {
        "whisper"
    }

    fn supports_streaming(&self) -> bool {
        false
    }

    fn is_initialized(&self) -> bool {
        true
    }

    fn create_session(&self) -> Result<Box<dyn TranscriptionSession>, String> {
        Ok(Box::new(WhisperSession { ctx: self.ctx.clone() }))
    }
}

/// Per-recording Whisper session. Holds `Arc<WhisperContext>` and creates a fresh
/// `WhisperState` per transcription call — avoids the `WhisperState<'a>` lifetime
/// constraint that would prevent storing it in `Box<dyn TranscriptionSession>`.
pub struct WhisperSession {
    ctx: Arc<WhisperContext>,
}

// WhisperContext is Send+Sync (immutable model weights behind a C pointer).
unsafe impl Send for WhisperSession {}

impl WhisperSession {
    fn run_params(ctx: &WhisperContext, params: FullParams, audio: &[f32]) -> Result<String, String> {
        let mut state = ctx
            .create_state()
            .map_err(|e| format!("Failed to create WhisperState: {:?}", e))?;

        state
            .full(params, audio)
            .map_err(|e| format!("Whisper transcription failed: {:?}", e))?;

        let n = state.full_n_segments().unwrap_or(0);
        let mut text = String::new();
        for i in 0..n {
            if let Ok(seg) = state.full_get_segment_text(i) {
                text.push_str(&seg);
            }
        }
        Ok(text)
    }
}

impl TranscriptionSession for WhisperSession {
    fn transcribe_partial(&mut self, audio: &[f32]) -> Result<Option<String>, String> {
        if audio.is_empty() {
            return Ok(None);
        }

        let mut params = FullParams::new(whisper_rs::SamplingStrategy::Greedy { best_of: 0 });
        params.set_n_threads(4);
        params.set_print_special(false);
        params.set_print_progress(false);
        params.set_print_realtime(false);
        params.set_print_timestamps(false);

        let text = Self::run_params(&self.ctx, params, audio)?;
        if text.is_empty() { Ok(None) } else { Ok(Some(text)) }
    }

    fn transcribe_final(&mut self, audio: &[f32]) -> Result<String, String> {
        if audio.is_empty() {
            return Ok(String::new());
        }

        let mut params =
            FullParams::new(whisper_rs::SamplingStrategy::BeamSearch { beam_size: 5, patience: 1.0 });
        params.set_temperature(0.0);
        params.set_print_special(false);
        params.set_print_progress(false);
        params.set_print_realtime(false);
        params.set_print_timestamps(false);

        let text = Self::run_params(&self.ctx, params, audio)?;
        Ok(filter_transcription_text(&text))
    }
}

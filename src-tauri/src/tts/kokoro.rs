use base64::{engine::general_purpose::STANDARD as BASE64_STANDARD, Engine as _};
use std::sync::{Mutex as StdMutex, OnceLock};
use tracing::{error, info};

// Kokoro-82M model singleton.
//
// Loading is expensive (~82MB + 2-5s disk parse). OnceLock<Mutex<Option<...>>> lets
// us lazy-init with proper error handling and retry — the OnceLock just creates the
// mutex shell; the Option tracks whether the model actually loaded.
//
// TtsModel: Send + Sync, so Box<dyn TtsModel> is safe in a static.
static KOKORO_MODEL: OnceLock<StdMutex<Option<Box<dyn any_tts::TtsModel>>>> = OnceLock::new();

/// Invoke Kokoro-82M TTS synthesis.
///
/// `voice` is read from the centralized settings manager (via AppState) by the
/// caller in `invoke_tts_for_provider`, keeping runtime configuration in the
/// Tauri Store rather than env vars. Returns base64-encoded WAV audio on success.
/// Model is lazily loaded on first call (downloads ~82MB from HuggingFace Hub if
/// not cached). afplay on macOS reads format from magic bytes, not extension, so
/// WAV bytes work fine in the .m4a temp file that play_base64_audio_with_tracking
/// creates.
pub async fn invoke_kokoro_tts(text: String, voice: String) -> Result<String, String> {
    info!(
        "[Kokoro] TTS requested: {} chars, voice: {}",
        text.chars().count(),
        voice
    );

    if crate::tts::is_tts_stop_requested() {
        info!("[Kokoro] Stop requested before start, aborting");
        return Ok("TTS_STOPPED_BY_USER".to_string());
    }

    // Candle inference is synchronous — must run off the async executor
    let base64_audio = tokio::task::spawn_blocking(move || -> Result<String, String> {
        if crate::tts::is_tts_stop_requested() {
            return Ok("TTS_STOPPED_BY_USER".to_string());
        }

        // Get or create the mutex shell (infallible — just wraps None)
        let model_mutex = KOKORO_MODEL.get_or_init(|| StdMutex::new(None));

        // Lazy model load — retryable: release guard before synthesizing so a
        // failed load doesn't poison the lock for the next call.
        {
            let mut guard = model_mutex
                .lock()
                .map_err(|_| "[Kokoro] Model mutex poisoned during init".to_string())?;

            if guard.is_none() {
                info!(
                    "[Kokoro] Loading Kokoro-82M (first run downloads ~82MB from HuggingFace Hub)"
                );
                let config =
                    any_tts::TtsConfig::new(any_tts::ModelType::Kokoro).with_preferred_runtime(); // Metal → CPU auto-selection

                match any_tts::load_model(config) {
                    Ok(model) => {
                        info!("[Kokoro] Model loaded successfully");
                        *guard = Some(model);
                    }
                    Err(e) => {
                        error!("[Kokoro] Model load failed: {}", e);
                        return Err(format!("Failed to load Kokoro-82M model: {}", e));
                    }
                }
            }
        } // guard released here — synthesis lock acquired separately below

        if crate::tts::is_tts_stop_requested() {
            return Ok("TTS_STOPPED_BY_USER".to_string());
        }

        let guard = model_mutex
            .lock()
            .map_err(|_| "[Kokoro] Model mutex poisoned during synthesis".to_string())?;
        let model = guard
            .as_ref()
            .ok_or_else(|| "[Kokoro] Model not present after successful load".to_string())?;

        let request = any_tts::SynthesisRequest::new(text.as_str()).with_voice(voice.as_str());

        info!("[Kokoro] Synthesizing with voice '{}'", voice);
        let audio = model
            .synthesize(&request)
            .map_err(|e| format!("[Kokoro] Synthesis failed: {}", e))?;

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

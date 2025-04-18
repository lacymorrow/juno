pub mod elevenlabs;
pub mod replicate;
pub mod system;

use tauri::State;
use crate::state::AppState;
use tracing::{info, warn};

// Placeholder for stopping speech playback if needed
#[allow(dead_code)] // Allow dead code as this function is not yet implemented/used
pub fn stop_speech() {
    // Implementation to stop any ongoing TTS playback
    println!("[TTS] Stop speech requested (not implemented).");
}

// Central TTS invocation function (used by Tauri commands, selects based on env var)
#[tauri::command]
pub async fn invoke_tts(
    text: String,
    state: State<'_, AppState>,
) -> Result<String, String> {
    // Select provider based on env var or config
    let provider = std::env::var("TTS_PROVIDER").unwrap_or_else(|_| "system".to_string());
    info!("Using TTS provider from environment: {}", provider);
    invoke_tts_for_provider(text, Some(state), &provider).await
}

// Invoke TTS for a specific provider name (used by CLI test and invoke_tts)
pub async fn invoke_tts_for_provider(
    text: String,
    _state: Option<State<'_, AppState>>,
    provider: &str,
) -> Result<String, String> {
    info!("Invoking TTS for provider: {}", provider);
    match provider.to_lowercase().as_str() {
        "elevenlabs" => elevenlabs::invoke_elevenlabs_tts(text).await,
        "replicate" => replicate::invoke_replicate_tts(text).await,
        "system" => system::invoke_system_tts(text).await,
        _ => {
            warn!("Unknown TTS provider specified: '{}'. Cannot invoke.", provider);
            Err(format!("Unknown TTS provider: {}", provider))
        }
    }
}

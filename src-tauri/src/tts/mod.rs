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

// Central TTS invocation function
pub async fn invoke_tts(
    text: String,
    state: State<'_, AppState>,
) -> Result<String, String> {
    // TODO: Add logic to select provider (e.g., based on env var or config)
    // For now, defaulting to system TTS.
    let provider = std::env::var("TTS_PROVIDER").unwrap_or_else(|_| "system".to_string());
    info!("Using TTS provider: {}", provider);

    match provider.to_lowercase().as_str() {
        "elevenlabs" => elevenlabs::invoke_elevenlabs_tts(text, state).await,
        "replicate" => replicate::invoke_replicate_tts(text, state).await,
        "system" => system::invoke_system_tts(text, state).await,
        _ => {
            warn!("Unknown TTS_PROVIDER: '{}'. Defaulting to system TTS.", provider);
            system::invoke_system_tts(text, state).await
        }
    }
}

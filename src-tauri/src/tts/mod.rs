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

// New command to set TTS provider
#[tauri::command]
pub async fn set_tts_provider_command(
    provider: String,
    state: State<'_, AppState>,
) -> Result<(), String> {
    let mut current_provider = state.tts_provider.lock().map_err(|e| format!("Failed to lock tts_provider: {}", e))?;
    *current_provider = provider.clone();
    info!("TTS provider set to: {}", provider);
    Ok(())
}

// New command to get current TTS provider
#[tauri::command]
pub async fn get_tts_provider_command(
    state: State<'_, AppState>,
) -> Result<String, String> {
    let provider = state.tts_provider.lock().map_err(|e| format!("Failed to lock tts_provider: {}", e))?.clone();
    info!("Current TTS provider: {}", provider);
    Ok(provider)
}

// Central TTS invocation function
#[tauri::command]
pub async fn invoke_tts(
    text: String,
    state: State<'_, AppState>,
) -> Result<String, String> {
    let provider_from_state = state.tts_provider.lock().map_err(|e| format!("Failed to lock tts_provider for invoke_tts: {}", e))?.clone();

    if provider_from_state.is_empty() || provider_from_state.to_lowercase() == "off" {
        let short_text = text.chars().take(30).collect::<String>();
        info!("TTS is set to '{}'. Skipping TTS for text: {}...", provider_from_state, short_text);
        return Ok("TTS_DISABLED_BY_SETTING".to_string());
    }

    info!("Using TTS provider from state: {}", provider_from_state);
    // The _state argument in invoke_tts_for_provider is not strictly needed now since the provider is explicit,
    // but keeping it for now to minimize changes to that function's signature if it's used elsewhere.
    invoke_tts_for_provider(text, Some(state), &provider_from_state).await
}

// Invoke TTS for a specific provider name
pub async fn invoke_tts_for_provider(
    text: String,
    _state: Option<State<'_, AppState>>, // _state might not be needed if provider is always passed
    provider: &str,
) -> Result<String, String> {
    info!("Invoking TTS for provider: {}", provider);
    match provider.to_lowercase().as_str() {
        "elevenlabs" => elevenlabs::invoke_elevenlabs_tts(text).await,
        "replicate" => replicate::invoke_replicate_tts(text).await,
        "system" => system::invoke_system_tts(text).await,
        "off" => { // Explicitly handle "off" here as well, though invoke_tts should catch it.
             warn!("invoke_tts_for_provider called with 'off', this should ideally be handled by invoke_tts. Skipping.");
             Ok("TTS_DISABLED_BY_SETTING".to_string())
        }
        _ => {
            warn!("Unknown TTS provider specified: '{}'. Cannot invoke.", provider);
            Err(format!("Unknown TTS provider: {}", provider))
        }
    }
}

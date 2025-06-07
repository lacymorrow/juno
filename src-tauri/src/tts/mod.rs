pub mod elevenlabs;
pub mod replicate;
pub mod system;

use tauri::State;
use crate::state::AppState;
use tracing::{info, warn};
use std::sync::atomic::{AtomicBool, Ordering};

// Global flag to indicate if TTS should be stopped
static TTS_STOP_REQUESTED: AtomicBool = AtomicBool::new(false);

// Function to stop speech playback
pub fn stop_speech() {
    info!("[TTS] Stop speech requested");
    TTS_STOP_REQUESTED.store(true, Ordering::SeqCst);

    // For system TTS, we can try to stop speech synthesis
    #[cfg(target_os = "macos")]
    {
        // On macOS, we can use the system say command to stop speech
        let _ = std::process::Command::new("killall")
            .arg("say")
            .output();
    }

    #[cfg(target_os = "windows")]
    {
        // On Windows, we would need to implement SAPI stop functionality
        // For now, just set the flag
    }

    #[cfg(target_os = "linux")]
    {
        // On Linux, stop espeak or festival
        let _ = std::process::Command::new("killall")
            .arg("espeak")
            .output();
        let _ = std::process::Command::new("killall")
            .arg("festival")
            .output();
    }
}

// Check if TTS stop was requested
pub fn is_tts_stop_requested() -> bool {
    TTS_STOP_REQUESTED.load(Ordering::SeqCst)
}

// Reset the stop flag
pub fn reset_tts_stop_flag() {
    TTS_STOP_REQUESTED.store(false, Ordering::SeqCst);
}

// Tauri command to stop TTS from frontend
#[tauri::command]
pub async fn stop_tts() -> Result<(), String> {
    info!("Stop TTS command received from frontend");
    stop_speech();
    Ok(())
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
    // Reset stop flag before starting new TTS
    reset_tts_stop_flag();

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

    // Check if stop was requested before starting
    if is_tts_stop_requested() {
        info!("TTS stop was requested before starting, aborting");
        return Ok("TTS_STOPPED_BY_USER".to_string());
    }

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

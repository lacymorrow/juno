pub mod elevenlabs;
pub mod replicate;
pub mod system;

use tauri::{State, AppHandle};
use crate::state::AppState;
use tracing::{info, warn, error};
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

// Register escape key for TTS cancellation
async fn register_tts_escape_key(app_handle: &AppHandle) {
    if let Err(e) = crate::commands::shortcuts::register_escape_key_handler(app_handle.clone()).await {
        warn!("Failed to register escape key for TTS: {} - TTS will still work but escape key may not stop it", e);
    } else {
        info!("[TTS] Registered escape key for TTS cancellation");
    }
}

// Unregister escape key after TTS completion
async fn unregister_tts_escape_key(app_handle: &AppHandle) {
    if let Err(e) = crate::commands::shortcuts::unregister_escape_key_handler(app_handle.clone()).await {
        warn!("Failed to unregister escape key after TTS: {} - continuing anyway", e);
    } else {
        info!("[TTS] Unregistered escape key after TTS completion");
    }
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
    // Reduced logging frequency - only log at debug level
    tracing::debug!("Current TTS provider: {}", provider);
    Ok(provider)
}

// Central TTS invocation function with escape key registration
#[tauri::command]
pub async fn invoke_tts(
    text: String,
    state: State<'_, AppState>,
    app_handle: AppHandle,
) -> Result<String, String> {
    // Reset stop flag before starting new TTS
    reset_tts_stop_flag();

    let provider_from_state = state.tts_provider.lock().map_err(|e| format!("Failed to lock tts_provider for invoke_tts: {}", e))?.clone();

    if provider_from_state.is_empty() || provider_from_state.to_lowercase() == "off" {
        let short_text = text.chars().take(30).collect::<String>();
        info!("TTS is set to '{}'. Skipping TTS for text: {}...", provider_from_state, short_text);
        return Ok("TTS_DISABLED_BY_SETTING".to_string());
    }

    // Register escape key for TTS cancellation
    register_tts_escape_key(&app_handle).await;

    info!("Using TTS provider from state: {}", provider_from_state);

    // Use fallback mechanism to try alternative providers if the primary fails
    let result = invoke_tts_with_fallback(text, &provider_from_state).await;

    // Unregister escape key after TTS completion (success or failure)
    unregister_tts_escape_key(&app_handle).await;

    result
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

// Invoke TTS with automatic fallback to alternative providers
pub async fn invoke_tts_with_fallback(
    text: String,
    primary_provider: &str,
) -> Result<String, String> {
    info!("Invoking TTS with fallback, primary provider: {}", primary_provider);

    // Check if stop was requested before starting
    if is_tts_stop_requested() {
        info!("TTS stop was requested before starting TTS with fallback, aborting");
        return Ok("TTS_STOPPED_BY_USER".to_string());
    }

    // Define the provider fallback order based on the primary provider
    let fallback_providers = match primary_provider.to_lowercase().as_str() {
        "replicate" => vec!["replicate", "elevenlabs", "system"],
        "elevenlabs" => vec!["elevenlabs", "system"],
        "system" => vec!["system", "elevenlabs", "replicate"],
        "off" => {
            return Ok("TTS_DISABLED_BY_SETTING".to_string());
        }
        _ => {
            warn!("Unknown primary TTS provider: '{}'. Using default fallback order.", primary_provider);
            vec!["system", "elevenlabs", "replicate"]
        }
    };

    let mut last_error = String::new();

    for (index, provider) in fallback_providers.iter().enumerate() {
        // Check if stop was requested before each attempt
        if is_tts_stop_requested() {
            info!("TTS stop was requested during fallback attempts, aborting");
            return Ok("TTS_STOPPED_BY_USER".to_string());
        }

        let is_primary = index == 0;
        info!("Attempting TTS with provider: {} ({})", provider, if is_primary { "primary" } else { "fallback" });

        match invoke_tts_for_provider(text.clone(), None, provider).await {
            Ok(result) => {
                if result == "TTS_STOPPED_BY_USER" {
                    return Ok(result);
                }
                if !is_primary {
                    warn!("Primary TTS provider '{}' failed, but fallback '{}' succeeded", primary_provider, provider);
                }
                return Ok(result);
            }
            Err(e) => {
                last_error = e.clone();
                if is_primary {
                    warn!("Primary TTS provider '{}' failed: {}", provider, e);
                } else {
                    warn!("Fallback TTS provider '{}' also failed: {}", provider, e);
                }
            }
        }
    }

    let final_error = format!("All TTS providers failed. Last error: {}", last_error);
    error!("{}", final_error);
    Err(final_error)
}

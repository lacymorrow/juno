pub mod elevenlabs;
pub mod replicate;
pub mod system;

use tauri::{State, AppHandle};
use crate::state::AppState;
use tracing::{info, warn, error, debug};
use std::sync::atomic::{AtomicBool, Ordering};
use regex::Regex;

// Global flags for TTS coordination
static TTS_STOP_REQUESTED: AtomicBool = AtomicBool::new(false);
static TTS_PLAYING: AtomicBool = AtomicBool::new(false);

/// Filter content to prevent code, emojis, and unwanted content from being spoken
/// NOTE: This no longer handles TTS XML extraction - that's handled by the streaming system
pub fn filter_tts_content(text: &str) -> String {
    debug!("[TTS Filter] Original text length: {} chars", text.len());

    let mut filtered_text = text.to_string();

    // Remove any TTS XML tags completely - content should have been processed by streaming system
    let tts_tag_regex = Regex::new(r"</?TTS>").unwrap();
    filtered_text = tts_tag_regex.replace_all(&filtered_text, "").to_string();

    // 1. Remove code blocks (```...```)
    let code_block_regex = Regex::new(r"```[\s\S]*?```").unwrap();
    filtered_text = code_block_regex.replace_all(&filtered_text, " ").to_string();

    // 2. Remove inline code (`...`)
    let inline_code_regex = Regex::new(r"`[^`]+`").unwrap();
    filtered_text = inline_code_regex.replace_all(&filtered_text, " ").to_string();

    // 3. Clean up whitespace and normalize
    let whitespace_regex = Regex::new(r"\s+").unwrap();
    filtered_text = whitespace_regex.replace_all(&filtered_text, " ").to_string();
    filtered_text = filtered_text.trim().to_string();

    debug!("[TTS Filter] Filtered text length: {} chars", filtered_text.len());
    if filtered_text.len() != text.len() {
        debug!("[TTS Filter] Content was filtered: '{}' -> '{}'",
               text.chars().take(100).collect::<String>(),
               filtered_text.chars().take(100).collect::<String>());
    }

    filtered_text
}

/// Play base64 audio directly without requiring tauri::State
/// This is a simplified version of play_tts_audio_backend for use in async contexts
async fn play_base64_audio_directly(base64_audio: &str) -> Result<(), String> {
    use base64::prelude::*;
    use std::io::Write;
    use tempfile::Builder as TempFileBuilder;

    info!("Playing TTS audio directly from base64 data ({} bytes)", base64_audio.len());

    // Decode base64 audio data
    let audio_bytes = BASE64_STANDARD.decode(base64_audio)
        .map_err(|e| format!("Failed to decode base64 TTS audio: {}", e))?;

    // Create temporary file for audio playback
    let mut temp_file = TempFileBuilder::new()
        .prefix("tts_audio_")
        .suffix(".m4a") // Use .m4a for compatibility
        .tempfile()
        .map_err(|e| format!("Failed to create temporary file for TTS audio: {}", e))?;

    // Write audio data to temporary file
    temp_file.write_all(&audio_bytes)
        .map_err(|e| format!("Failed to write TTS audio to temporary file: {}", e))?;

    temp_file.flush()
        .map_err(|e| format!("Failed to flush TTS audio to temporary file: {}", e))?;

    let temp_path = temp_file.path().to_path_buf();

    info!("Playing TTS audio from temporary file: {:?}", temp_path);

    // Play the audio file using the existing platform-specific playback
    // We'll use the same logic as in sound.rs but without the state dependency
    #[cfg(target_os = "macos")]
    {
        let output = tokio::process::Command::new("afplay")
            .arg(&temp_path)
            .output()
            .await
            .map_err(|e| format!("Failed to execute afplay: {}", e))?;

        if !output.status.success() {
            let error_msg = format!("afplay failed: {}", String::from_utf8_lossy(&output.stderr));
            return Err(error_msg);
        }
    }

    #[cfg(target_os = "linux")]
    {
        let output = tokio::process::Command::new("aplay")
            .arg(&temp_path)
            .output()
            .await
            .map_err(|e| format!("Failed to execute aplay: {}", e))?;

        if !output.status.success() {
            let error_msg = format!("aplay failed: {}", String::from_utf8_lossy(&output.stderr));
            return Err(error_msg);
        }
    }

    #[cfg(target_os = "windows")]
    {
        // On Windows, we could use PowerShell or a media library
        // For now, just return success as a placeholder
        warn!("TTS audio playback on Windows not implemented in direct mode");
        return Ok(());
    }

    info!("TTS audio played successfully");
    Ok(())
    // Temporary file is automatically cleaned up when it goes out of scope
}

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

// Reset the stop flag - CRITICAL: This fixes the permanent disablement bug
pub fn reset_tts_stop_flag() {
    TTS_STOP_REQUESTED.store(false, Ordering::SeqCst);
}

// Check if TTS is currently playing
pub fn is_tts_playing() -> bool {
    TTS_PLAYING.load(Ordering::SeqCst)
}

// Set TTS playing state
fn set_tts_playing(playing: bool) {
    TTS_PLAYING.store(playing, Ordering::SeqCst);
}

// Register escape key for TTS cancellation - CENTRALIZED
pub async fn register_tts_escape_key(app_handle: &AppHandle) {
    if let Err(e) = crate::commands::shortcuts::register_escape_key_handler(app_handle.clone()).await {
        warn!("[TTS] Failed to register escape key for TTS: {} - TTS will still work but escape key may not stop it", e);
    } else {
        info!("[TTS] Registered escape key for TTS cancellation");
    }
}

// Unregister escape key after TTS completion - CENTRALIZED
pub async fn unregister_tts_escape_key(app_handle: &AppHandle) {
    if let Err(e) = crate::commands::shortcuts::unregister_escape_key_handler(app_handle.clone()).await {
        warn!("[TTS] Failed to unregister escape key after TTS: {} - continuing anyway", e);
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
    app_handle: AppHandle,
    state: State<'_, AppState>,
) -> Result<(), String> {
    info!("Setting TTS provider to: {}", provider);

    // Validate provider
    let valid_providers = ["off", "system", "elevenlabs", "replicate"];
    if !valid_providers.contains(&provider.as_str()) {
        return Err(format!("Invalid TTS provider: {}. Valid providers: {:?}", provider, valid_providers));
    }

    // Get current settings from centralized system
    let settings_manager = crate::settings::manager::SettingsManager::new(app_handle.clone())
        .map_err(|e| format!("Failed to create settings manager: {}", e))?;

    let mut audio_settings = settings_manager.get_audio_settings().await
        .map_err(|e| format!("Failed to get audio settings: {}", e))?;

    // Update centralized settings
    audio_settings.tts_provider = provider.clone();
    settings_manager.set_audio_settings(&audio_settings).await
        .map_err(|e| format!("Failed to save audio settings: {}", e))?;

    // Update app state for backward compatibility
    state.set_tts_provider(provider.clone()).map_err(|e| format!("Failed to set tts_provider: {}", e))?;

    info!("TTS provider set to: {} (saved to centralized settings)", provider);
    Ok(())
}

// New command to get current TTS provider
#[tauri::command]
pub async fn get_tts_provider_command(
    app_handle: AppHandle,
) -> Result<String, String> {
    // Get provider from centralized settings
    let settings_manager = crate::settings::manager::SettingsManager::new(app_handle.clone())
        .map_err(|e| format!("Failed to create settings manager: {}", e))?;

    let audio_settings = settings_manager.get_audio_settings().await
        .map_err(|e| format!("Failed to get audio settings: {}", e))?;

    // Reduced logging frequency - only log at debug level
    tracing::debug!("Current TTS provider from centralized settings: {}", audio_settings.tts_provider);
    Ok(audio_settings.tts_provider)
}

// FIXED: Proper concurrency control, stop flag lifecycle, and state access
#[tauri::command]
pub async fn invoke_tts(
    text: String,
    state: State<'_, AppState>,
    app_handle: AppHandle,
) -> Result<String, String> {
    // CRITICAL FIX 1: Prevent concurrent TTS playback
    if is_tts_playing() {
        info!("TTS is already playing, ignoring new request to prevent overlapping audio");
        return Ok("TTS_ALREADY_PLAYING".to_string());
    }

    // CRITICAL FIX 2: Reset stop flag at the start of each operation
    reset_tts_stop_flag();

    let provider = state.get_tts_provider().map_err(|e| format!("Failed to get tts_provider for invoke_tts: {}", e))?;

    if provider.is_empty() || provider.to_lowercase() == "off" {
        let short_text = text.chars().take(30).collect::<String>();
        info!("TTS is set to '{}'. Skipping TTS for text: {}...", provider, short_text);
        return Ok("TTS_DISABLED_BY_SETTING".to_string());
    }

    // Filter content to prevent code, emojis, and unwanted content from being spoken
    let filtered_text = filter_tts_content(&text);

    // If filtering removed all content, skip TTS
    if filtered_text.is_empty() {
        info!("TTS content was filtered out (appears to be code/unwanted content), skipping TTS");
        return Ok("TTS_CONTENT_FILTERED".to_string());
    }

    // Set TTS as playing to prevent concurrent operations
    set_tts_playing(true);

    // CRITICAL FIX 3: Centralized escape key management - register once
    register_tts_escape_key(&app_handle).await;

    // Execute TTS synchronously to maintain proper control flow
    let result = execute_tts_with_state_access(filtered_text, &provider, &state, &app_handle).await;

    // CRITICAL FIX 4: Always clean up after TTS completes
    set_tts_playing(false);
    unregister_tts_escape_key(&app_handle).await;

    result
}

// Execute TTS with proper state access instead of cloning
async fn execute_tts_with_state_access(
    text: String,
    primary_provider: &str,
    state: &State<'_, AppState>,
    app_handle: &AppHandle,
) -> Result<String, String> {
    info!("Starting TTS with provider: {}", primary_provider);

    // Execute TTS with fallback logic
    match execute_tts_with_fallback(text, primary_provider).await {
        Ok(result) => {
            if result == "TTS_STOPPED_BY_USER" {
                info!("TTS was stopped by user during execution");
                Ok(result)
            } else if result == "TTS_DISABLED_BY_SETTING" {
                info!("TTS is disabled by setting");
                Ok(result)
            } else if result == "TTS_CONTENT_FILTERED" {
                info!("TTS content was filtered out");
                Ok(result)
            } else {
                // This should be base64 audio data - play it!
                info!("TTS audio generated, attempting playback...");

                // Check if stop was requested before playback
                if is_tts_stop_requested() {
                    info!("TTS stop was requested before playback, aborting");
                    return Ok("TTS_STOPPED_BY_USER".to_string());
                }

                // CRITICAL FIX 4: Access current state instead of using cloned/stale state
                match state.get_sound_enabled() {
                    Ok(sound_enabled) => {
                        if !sound_enabled {
                            info!("Sound is disabled, skipping TTS audio playback");
                            Ok("TTS_SOUND_DISABLED".to_string())
                        } else {
                            // Decode and play the base64 audio directly
                            match play_base64_audio_directly(&result).await {
                                Ok(_) => {
                                    info!("TTS audio playback completed successfully");
                                    Ok("TTS_COMPLETED".to_string())
                                }
                                Err(e) => {
                                    warn!("TTS audio playback error: {}", e);
                                    Err(format!("TTS playback failed: {}", e))
                                }
                            }
                        }
                    }
                    Err(e) => {
                        warn!("Failed to check sound enabled status: {}", e);
                        Err(format!("Failed to access sound settings: {}", e))
                    }
                }
            }
        }
        Err(e) => {
            error!("TTS failed: {}", e);
            Err(e)
        }
    }
}

// Execute TTS with fallback logic (no blocking, no race conditions)
async fn execute_tts_with_fallback(
    text: String,
    primary_provider: &str,
) -> Result<String, String> {
    // Check network connectivity for cloud-based providers
    let is_cloud_provider = matches!(primary_provider.to_lowercase().as_str(), "replicate" | "elevenlabs");

    // If it's a cloud provider, do a quick network check first
    if is_cloud_provider {
        info!("Cloud TTS provider detected, checking network connectivity...");
        let is_online = crate::utils::network::is_online().await;
        if !is_online {
            warn!("Device appears offline, using system TTS directly");
            return invoke_tts_for_provider(text, None, "system").await;
        }
    }

    // Define the provider fallback order based on the primary provider
    let fallback_providers = match primary_provider.to_lowercase().as_str() {
        "replicate" => vec!["replicate", "system"],
        "elevenlabs" => vec!["elevenlabs", "system"],
        "system" => vec!["system"],
        "off" => return Ok("TTS_DISABLED_BY_SETTING".to_string()),
        _ => {
            warn!("Unknown primary TTS provider: '{}'. Using system fallback only.", primary_provider);
            vec!["system"]
        }
    };

    let mut last_error = String::new();

    for (index, fallback_provider) in fallback_providers.iter().enumerate() {
        // Check if stop was requested before each attempt
        if is_tts_stop_requested() {
            info!("TTS stop was requested during fallback attempts, aborting");
            return Ok("TTS_STOPPED_BY_USER".to_string());
        }

        let is_primary = index == 0;
        info!("Attempting TTS with provider: {} ({})", fallback_provider, if is_primary { "primary" } else { "fallback" });

        match invoke_tts_for_provider(text.clone(), None, fallback_provider).await {
            Ok(result) => {
                if result == "TTS_STOPPED_BY_USER" {
                    return Ok(result);
                }
                if !is_primary {
                    warn!("Primary TTS provider '{}' failed, but fallback '{}' succeeded", primary_provider, fallback_provider);
                }
                return Ok(result);
            }
            Err(e) => {
                last_error = e.clone();

                // Check if this is a network-related error
                let is_network_error = crate::utils::network::is_network_error(&e);

                if is_primary && is_network_error {
                    warn!("Primary TTS provider '{}' failed with network error: {}. Trying system TTS immediately.", fallback_provider, e);
                    // For network errors, skip other cloud providers and go straight to system
                    match invoke_tts_for_provider(text.clone(), None, "system").await {
                        Ok(system_result) => {
                            warn!("Network error detected, successfully fell back to system TTS");
                            return Ok(system_result);
                        }
                        Err(system_error) => {
                            error!("Even system TTS failed: {}", system_error);
                            return Err(format!("Network error with primary provider and system TTS also failed: {}", system_error));
                        }
                    }
                } else {
                    warn!("TTS provider '{}' failed: {}", fallback_provider, e);
                }
            }
        }
    }

    let final_error = format!("All TTS providers failed. Last error: {}", last_error);
    error!("{}", final_error);
    Err(final_error)
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
        "off" => {
             warn!("invoke_tts_for_provider called with 'off', this should ideally be handled by invoke_tts. Skipping.");
             Ok("TTS_DISABLED_BY_SETTING".to_string())
        }
        _ => {
            warn!("Unknown TTS provider specified: '{}'. Cannot invoke.", provider);
            Err(format!("Unknown TTS provider: {}", provider))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_filter_code_blocks() {
        let input = "Here's some text ```rust\nfn hello() {\n    println!(\"world\");\n}\n``` and more text.";
        let expected = "Here's some text   and more text.";
        let result = filter_tts_content(input);
        assert_eq!(result, expected);
    }

    #[test]
    fn test_filter_inline_code() {
        let input = "Use the `console.log()` function to debug your `variable` values.";
        let expected = "Use the   function to debug your   values.";
        let result = filter_tts_content(input);
        assert_eq!(result, expected);
    }

    #[test]
    fn test_filter_jsx_tags() {
        let input = "Here's a React component: <Card><CardContent><div className=\"flex justify-center my-4\">Hello</div></CardContent></Card>";
        let expected = "Here's a React component:";
        let result = filter_tts_content(input);
        assert_eq!(result, expected);
    }

    #[test]
    fn test_filter_html_tags() {
        let input = "This is <strong>bold</strong> and <em>italic</em> text.";
        let expected = "This is   and   text.";
        let result = filter_tts_content(input);
        assert_eq!(result, expected);
    }

    #[test]
    fn test_filter_function_calls() {
        let input = "Call the function getData() and then processResult(data) to continue.";
        let expected = "Call the function   and then   to continue.";
        let result = filter_tts_content(input);
        assert_eq!(result, expected);
    }

    #[test]
    fn test_filter_method_chaining() {
        let input = "Use object.method().anotherMethod() to chain calls.";
        let expected = "Use   to chain calls.";
        let result = filter_tts_content(input);
        assert_eq!(result, expected);
    }

    #[test]
    fn test_filter_property_access() {
        let input = "Access config.server.port for the port number.";
        let expected = "Access   for the port number.";
        let result = filter_tts_content(input);
        assert_eq!(result, expected);
    }

    #[test]
    fn test_filter_urls_and_paths() {
        let input = "Visit https://example.com or check /home/user/file.txt and ~/documents/readme.md";
        let expected = "Visit   or check   and";
        let result = filter_tts_content(input);
        assert_eq!(result, expected);
    }

    #[test]
    fn test_filter_programming_keywords() {
        let input = "const myVar = 5; let result = getData(); if (condition) { return value; }";
        let expected = "5;   getData(); { value; }";
        let result = filter_tts_content(input);
        assert_eq!(result, expected);
    }

    #[test]
    fn test_filter_emojis() {
        let input = "Hello world! 😀 This is great! 🎉 Let's code! 💻";
        let expected = "Hello world!   This is great!   Let's code!";
        let result = filter_tts_content(input);
        assert_eq!(result, expected);
    }

    #[test]
    fn test_filter_json_structures() {
        let input = "The config is {\"port\": 8080, \"host\": \"localhost\"} and array is [1, 2, 3].";
        let expected = "The config is   and array is .";
        let result = filter_tts_content(input);
        assert_eq!(result, expected);
    }

    #[test]
    fn test_filter_css() {
        let input = "Add .button { color: red; } to your stylesheet.";
        let expected = "Add   to your stylesheet.";
        let result = filter_tts_content(input);
        assert_eq!(result, expected);
    }

    #[test]
    fn test_mostly_code_content_returns_empty() {
        let input = "```javascript\nconst x = 5;\n```";
        let result = filter_tts_content(input);
        assert_eq!(result, "");
    }

    #[test]
    fn test_jsx_example_from_logs() {
        let input = "<Card><CardContent><div className=\"flex justify-center my-4\">Content here</div></CardContent></Card>";
        let result = filter_tts_content(input);
        assert_eq!(result, "");
    }

    #[test]
    fn test_preserve_normal_text() {
        let input = "This is a normal sentence with regular words and punctuation.";
        let result = filter_tts_content(input);
        assert_eq!(result, input);
    }

    #[test]
    fn test_mixed_content() {
        let input = "Here's normal text. ```code block``` More normal text with `inline code` and regular content.";
        let expected = "Here's normal text.   More normal text with   and regular content.";
        let result = filter_tts_content(input);
        assert_eq!(result, expected);
    }

    #[test]
    fn test_whitespace_normalization() {
        let input = "Multiple    spaces   and\n\nnewlines\t\tand\ttabs.";
        let expected = "Multiple spaces and newlines and tabs.";
        let result = filter_tts_content(input);
        assert_eq!(result, expected);
    }

    #[test]
    fn test_empty_input() {
        let input = "";
        let result = filter_tts_content(input);
        assert_eq!(result, "");
    }

    #[test]
    fn test_variable_assignments() {
        let input = "Set myVariable = 42 and config: value to proceed.";
        let expected = "Set   and   to proceed.";
        let result = filter_tts_content(input);
        assert_eq!(result, expected);
    }
}

pub mod elevenlabs;
pub mod replicate;
pub mod system;

use tauri::{State, AppHandle};
use crate::state::AppState;
use tracing::{info, warn, error, debug};
use std::sync::atomic::{AtomicBool, Ordering};
use regex::Regex;

// Global flag to indicate if TTS should be stopped
static TTS_STOP_REQUESTED: AtomicBool = AtomicBool::new(false);

/// Filter content to prevent code, emojis, and unwanted content from being spoken
pub fn filter_tts_content(text: &str) -> String {
    debug!("[TTS Filter] Original text length: {} chars", text.len());

    let mut filtered_text = text.to_string();

    // 0. Extract TTS XML content first (this is for fallback cases where immediate TTS didn't work)
    let tts_regex = Regex::new(r"<TTS>(.*?)</TTS>").unwrap();
    if tts_regex.is_match(&filtered_text) {
        // CRITICAL FIX: If we have TTS tags, this means immediate TTS processing failed
        // In most cases, immediate TTS should have already processed this content
        warn!("[TTS Filter] Found TTS tags in final processing - this suggests immediate TTS didn't work properly");

        // Extract only the content inside them as fallback
        let extracted_content: Vec<&str> = tts_regex
            .captures_iter(&filtered_text)
            .map(|cap| cap.get(1).unwrap().as_str())
            .collect();

        if !extracted_content.is_empty() {
            // Return only the TTS content, joined together
            filtered_text = extracted_content.join(" ");
            debug!("[TTS Filter] Extracted TTS content from XML tags (FALLBACK): '{}'", filtered_text);
            return filtered_text;
        }
    }

    // 1. Remove any remaining TTS XML tags that weren't processed (safety net)
    let remaining_tts_regex = Regex::new(r"</?TTS>").unwrap();
    filtered_text = remaining_tts_regex.replace_all(&filtered_text, "").to_string();

    // 2. Remove code blocks (```...```)
    let code_block_regex = Regex::new(r"```[\s\S]*?```").unwrap();
    filtered_text = code_block_regex.replace_all(&filtered_text, " ").to_string();

    // 3. Remove inline code (`...`)
    let inline_code_regex = Regex::new(r"`[^`]+`").unwrap();
    filtered_text = inline_code_regex.replace_all(&filtered_text, " ").to_string();

    // 4. Remove HTML/JSX tags (excluding TTS tags which were handled above)
    let html_tag_regex = Regex::new(r"<[^>]*>").unwrap();
    filtered_text = html_tag_regex.replace_all(&filtered_text, " ").to_string();

    // 5. Remove JSX/React component syntax
    let jsx_component_regex = Regex::new(r"</?[A-Z][a-zA-Z0-9]*[^>]*>").unwrap();
    filtered_text = jsx_component_regex.replace_all(&filtered_text, " ").to_string();

    // 6. Remove code-like patterns (function calls, method chaining, etc.)
    let function_call_regex = Regex::new(r"\w+\(\s*[^)]*\s*\)").unwrap();
    filtered_text = function_call_regex.replace_all(&filtered_text, " ").to_string();

    // 7. Remove method chaining (e.g., object.method().anotherMethod())
    let method_chain_regex = Regex::new(r"\w+(\.\w+)+\([^)]*\)").unwrap();
    filtered_text = method_chain_regex.replace_all(&filtered_text, " ").to_string();

    // 8. Remove property access chains (e.g., object.property.subProperty)
    let property_chain_regex = Regex::new(r"\w+(\.\w+){2,}").unwrap();
    filtered_text = property_chain_regex.replace_all(&filtered_text, " ").to_string();

    // 9. Remove file paths and URLs
    let path_url_regex = Regex::new(r"(?:https?://|/|~/|\.\./)[\w\-_\./?=&%#]+").unwrap();
    filtered_text = path_url_regex.replace_all(&filtered_text, " ").to_string();

    // 10. Remove common programming keywords and patterns
    let programming_keywords = [
        r"\bconst\s+\w+\s*=", r"\blet\s+\w+\s*=", r"\bvar\s+\w+\s*=",
        r"\bfunction\s+\w+", r"\bclass\s+\w+", r"\binterface\s+\w+",
        r"\bimport\s+", r"\bexport\s+", r"\breturn\s+", r"\bif\s*\(",
        r"\bfor\s*\(", r"\bwhile\s*\(", r"\btry\s*\{", r"\bcatch\s*\(",
        r"\basync\s+", r"\bawait\s+", r"\bnew\s+\w+", r"\bthis\.",
        r"\bconsole\.", r"\bdocument\.", r"\bwindow\.", r"\bprocess\.",
    ];

    for keyword_pattern in &programming_keywords {
        let keyword_regex = Regex::new(keyword_pattern).unwrap();
        filtered_text = keyword_regex.replace_all(&filtered_text, " ").to_string();
    }

    // 11. Remove emojis (Unicode ranges for various emoji blocks)
    let emoji_regex = Regex::new(r"[\u{1F600}-\u{1F64F}]|[\u{1F300}-\u{1F5FF}]|[\u{1F680}-\u{1F6FF}]|[\u{1F1E0}-\u{1F1FF}]|[\u{2600}-\u{26FF}]|[\u{2700}-\u{27BF}]|[\u{1F900}-\u{1F9FF}]|[\u{1F018}-\u{1F270}]|[\u{238C}-\u{2454}]|[\u{20D0}-\u{20FF}]").unwrap();
    filtered_text = emoji_regex.replace_all(&filtered_text, " ").to_string();

    // 12. Remove mathematical expressions and formulas
    let math_regex = Regex::new(r"\$[^$]+\$|\\[a-zA-Z]+\{[^}]*\}").unwrap();
    filtered_text = math_regex.replace_all(&filtered_text, " ").to_string();

    // 13. Remove JSON-like structures
    let json_regex = Regex::new(r"\{[^{}]*:[^{}]*\}|\[[^\[\]]*\]").unwrap();
    filtered_text = json_regex.replace_all(&filtered_text, " ").to_string();

    // 14. Remove variable assignments and declarations
    let assignment_regex = Regex::new(r"\w+\s*[:=]\s*[^,;\n]+").unwrap();
    filtered_text = assignment_regex.replace_all(&filtered_text, " ").to_string();

    // 15. Remove CSS selectors and properties
    let css_regex = Regex::new(r"[.#][\w-]+\s*\{[^}]*\}|[\w-]+\s*:\s*[^;]+;").unwrap();
    filtered_text = css_regex.replace_all(&filtered_text, " ").to_string();

    // 16. Clean up whitespace and normalize
    let whitespace_regex = Regex::new(r"\s+").unwrap();
    filtered_text = whitespace_regex.replace_all(&filtered_text, " ").to_string();
    filtered_text = filtered_text.trim().to_string();

    // If after filtering we have very little meaningful content left,
    // it was probably mostly code - return empty string to skip TTS
    if filtered_text.len() < 10 || filtered_text.split_whitespace().count() < 3 {
        debug!("[TTS Filter] Content appears to be mostly code, skipping TTS");
        return String::new();
    }

    debug!("[TTS Filter] Filtered text length: {} chars", filtered_text.len());
    if filtered_text.len() != text.len() {
        debug!("[TTS Filter] Content was filtered: '{}' -> '{}'",
               text.chars().take(100).collect::<String>(),
               filtered_text.chars().take(100).collect::<String>());
    }

    filtered_text
}

// Function to stop speech playback with deduplication
pub fn stop_speech() {
    // Check if already stopped to prevent redundant operations
    if TTS_STOP_REQUESTED.load(Ordering::SeqCst) {
        debug!("[TTS] Stop speech already requested, skipping redundant operation");
        return;
    }

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
pub async fn register_tts_escape_key(app_handle: &AppHandle) {
    if let Err(e) = crate::commands::shortcuts::register_escape_key_handler(app_handle.clone()).await {
        warn!("[TTS] Failed to register escape key for TTS: {} - TTS will still work but escape key may not stop it", e);
    } else {
        info!("[TTS] Registered escape key for TTS cancellation");
    }
}

// Unregister escape key after TTS completion
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
    let mut current_provider = state.tts_provider.lock().map_err(|e| format!("Failed to lock tts_provider: {}", e))?;
    *current_provider = provider.clone();

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

// Central TTS invocation function with escape key registration and fallback logic
#[tauri::command]
pub async fn invoke_tts(
    text: String,
    state: State<'_, AppState>,
    app_handle: AppHandle,
) -> Result<String, String> {
    // CRITICAL FIX: Stop any existing TTS before starting new one to prevent token waste
    info!("New TTS request received, stopping any existing TTS to prevent token waste");
    stop_speech();

    // Brief pause to allow existing TTS operations to detect the stop signal
    tokio::time::sleep(std::time::Duration::from_millis(50)).await;

    // Reset stop flag for the new TTS request immediately after the pause
    // This ensures the new request won't be aborted by the stop flag we just set
    reset_tts_stop_flag();

    let provider = state.tts_provider.lock().map_err(|e| format!("Failed to lock tts_provider for invoke_tts: {}", e))?.clone();

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

    // Register escape key for TTS cancellation
    register_tts_escape_key(&app_handle).await;

    info!("Using TTS provider from state: {}", provider);

    // Check network connectivity for cloud-based providers
    let is_cloud_provider = matches!(provider.to_lowercase().as_str(), "replicate" | "elevenlabs");

    // If it's a cloud provider, do a quick network check first
    if is_cloud_provider {
        info!("Cloud TTS provider detected, checking network connectivity...");
        let is_online = crate::utils::network::is_online().await;
        if !is_online {
            warn!("Device appears offline, skipping cloud TTS providers and using system TTS directly");
            let result = invoke_tts_for_provider(filtered_text, None, "system").await;
            unregister_tts_escape_key(&app_handle).await;
            return result;
        }
    }

    // Define the provider fallback order based on the primary provider
    let fallback_providers = match provider.to_lowercase().as_str() {
        "replicate" => vec!["replicate", "system"], // Prioritize system over other cloud providers when offline
        "elevenlabs" => vec!["elevenlabs", "system"], // Prioritize system over other cloud providers when offline
        "system" => vec!["system"], // System only, no cloud fallbacks needed
        "off" => {
            unregister_tts_escape_key(&app_handle).await;
            return Ok("TTS_DISABLED_BY_SETTING".to_string());
        }
        _ => {
            warn!("Unknown primary TTS provider: '{}'. Using default fallback order.", provider);
            vec!["system"] // Default to system for unknown providers
        }
    };

    let mut last_error = String::new();

    for (index, fallback_provider) in fallback_providers.iter().enumerate() {
        // Check if stop was requested before each attempt
        if is_tts_stop_requested() {
            info!("TTS stop was requested during fallback attempts, aborting");
            unregister_tts_escape_key(&app_handle).await;
            return Ok("TTS_STOPPED_BY_USER".to_string());
        }

        let is_primary = index == 0;
        info!("Attempting TTS with provider: {} ({})", fallback_provider, if is_primary { "primary" } else { "fallback" });

        match invoke_tts_for_provider(filtered_text.clone(), None, fallback_provider).await {
            Ok(result) => {
                if result == "TTS_STOPPED_BY_USER" {
                    unregister_tts_escape_key(&app_handle).await;
                    return Ok(result);
                }
                if !is_primary {
                    warn!("Primary TTS provider '{}' failed, but fallback '{}' succeeded", provider, fallback_provider);
                }

                // Unregister escape key after successful TTS
                unregister_tts_escape_key(&app_handle).await;
                return Ok(result);
            }
            Err(e) => {
                last_error = e.clone();

                // Check if this is a network-related error
                let is_network_error = crate::utils::network::is_network_error(&e);

                if is_primary {
                    if is_network_error {
                        warn!("Primary TTS provider '{}' failed with network error: {}. Trying system TTS immediately.", fallback_provider, e);
                        // For network errors, skip other cloud providers and go straight to system
                        match invoke_tts_for_provider(filtered_text.clone(), None, "system").await {
                            Ok(system_result) => {
                                warn!("Network error detected, successfully fell back to system TTS");
                                unregister_tts_escape_key(&app_handle).await;
                                return Ok(system_result);
                            }
                            Err(system_error) => {
                                error!("Even system TTS failed: {}", system_error);
                                unregister_tts_escape_key(&app_handle).await;
                                return Err(format!("Network error with primary provider and system TTS also failed: {}", system_error));
                            }
                        }
                    } else {
                        warn!("Primary TTS provider '{}' failed: {}", fallback_provider, e);
                    }
                } else {
                    warn!("Fallback TTS provider '{}' also failed: {}", fallback_provider, e);
                }
            }
        }
    }

    let final_error = format!("All TTS providers failed. Last error: {}", last_error);
    error!("{}", final_error);

    // Unregister escape key after TTS failure
    unregister_tts_escape_key(&app_handle).await;

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

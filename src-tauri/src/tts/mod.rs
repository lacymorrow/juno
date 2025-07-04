pub mod elevenlabs;
pub mod replicate;
pub mod system;

use tauri::{State, AppHandle};
use crate::state::AppState;
use tracing::{info, warn, error, debug};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tokio::sync::Mutex;
use regex::Regex;

// Global flags for TTS coordination
static TTS_STOP_REQUESTED: AtomicBool = AtomicBool::new(false);
static TTS_PLAYING: AtomicBool = AtomicBool::new(false);

// Global mutex for preventing concurrent TTS operations
static TTS_MUTEX: Mutex<()> = Mutex::const_new(());

// Structure to track audio playback completion with error propagation
#[derive(Debug)]
struct AudioPlaybackHandle {
    completion_notify: Arc<tokio::sync::Notify>,
    error_notify: Arc<tokio::sync::Notify>,
    playback_error: Arc<Mutex<Option<String>>>,
    start_time: std::time::Instant,
    playback_started: Arc<AtomicBool>,
    // Keep the spawn handle alive to prevent task cancellation
    _task_handle: tokio::task::JoinHandle<()>,
}

impl AudioPlaybackHandle {
    async fn wait_for_completion(&self) -> Result<(), String> {
        // Wait for either completion or error notification from the background task
        tokio::select! {
            _ = self.completion_notify.notified() => {
                // Check if there was an error even after completion
                if let Some(error) = self.playback_error.lock().await.as_ref() {
                    return Err(error.clone());
                }
            }
            _ = self.error_notify.notified() => {
                // Error occurred, propagate it
                if let Some(error) = self.playback_error.lock().await.as_ref() {
                    return Err(error.clone());
                } else {
                    return Err("Unknown audio playback error occurred".to_string());
                }
            }
        }

        let elapsed = self.start_time.elapsed();
        // OPTIMIZATION: Use event-driven completion instead of hardcoded minimum duration
        // Only add minimal delay if audio completed suspiciously fast (< 50ms)
        if elapsed < std::time::Duration::from_millis(50) {
            let safety_delay = std::time::Duration::from_millis(25);
            info!("Audio completed very quickly ({}ms), adding safety delay of {}ms",
                  elapsed.as_millis(), safety_delay.as_millis());
            tokio::time::sleep(safety_delay).await;
        }

        info!("Audio playback completion confirmed after {}ms", elapsed.as_millis());
        Ok(())
    }
}

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

    // 3. Remove HTML/JSX tags (including self-closing tags)
    let html_tag_regex = Regex::new(r"<[^>]*>").unwrap();
    filtered_text = html_tag_regex.replace_all(&filtered_text, " ").to_string();

    // 12. Clean up whitespace and normalize
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

/// Play base64 audio with proper completion tracking and error handling
/// Returns an AudioPlaybackHandle that can be awaited for completion
async fn play_base64_audio_with_tracking(base64_audio: &str) -> Result<AudioPlaybackHandle, String> {
    use base64::prelude::*;
    use std::io::Write;
    use tempfile::Builder as TempFileBuilder;

    info!("Playing TTS audio with completion tracking ({} bytes)", base64_audio.len());

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
    let completion_notify = Arc::new(tokio::sync::Notify::new());
    let completion_notify_clone = completion_notify.clone();
    let playback_started = Arc::new(AtomicBool::new(false));
    let error_notify = Arc::new(tokio::sync::Notify::new());
    let error_notify_clone = error_notify.clone();
    let playback_error = Arc::new(Mutex::new(Option::<String>::None));
    let playback_error_clone = playback_error.clone();

    info!("Playing TTS audio from temporary file: {:?}", temp_path);

    // Platform-specific audio playback with proper completion tracking and error propagation
    let task_handle = {
        #[cfg(target_os = "macos")]
        {
            let mut child = tokio::process::Command::new("afplay")
                .arg(&temp_path)
                .spawn()
                .map_err(|e| format!("Failed to spawn afplay: {}", e))?;

            let playback_started_clone = playback_started.clone();

            tokio::spawn(async move {
                tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
                playback_started_clone.store(true, Ordering::SeqCst);

                let result = child.wait().await;

                match result {
                    Ok(status) => {
                        if status.success() {
                            info!("macOS afplay completed successfully");
                        } else {
                            let error_msg = format!("macOS afplay exited with non-zero status: {}", status);
                            error!("{}", error_msg);
                            *playback_error_clone.lock().await = Some(error_msg);
                            error_notify_clone.notify_one();
                        }
                    }
                    Err(e) => {
                        let error_msg = format!("Failed to wait for macOS afplay process: {}", e);
                        error!("{}", error_msg);
                        *playback_error_clone.lock().await = Some(error_msg);
                        error_notify_clone.notify_one();
                    }
                }
                completion_notify_clone.notify_one();
                debug!("macOS afplay task completed and notified");
                drop(temp_file);
            })
        }

        #[cfg(target_os = "linux")]
        {
            let mut child = tokio::process::Command::new("aplay")
                .arg(&temp_path)
                .spawn()
                .map_err(|e| format!("Failed to spawn aplay: {}", e))?;

            let playback_started_clone = playback_started.clone();

            tokio::spawn(async move {
                tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
                playback_started_clone.store(true, Ordering::SeqCst);

                let result = child.wait().await;

                match result {
                    Ok(status) => {
                        if status.success() {
                            info!("Linux aplay completed successfully");
                        } else {
                            let error_msg = format!("Linux aplay exited with non-zero status: {}", status);
                            error!("{}", error_msg);
                            *playback_error_clone.lock().await = Some(error_msg);
                            error_notify_clone.notify_one();
                        }
                    }
                    Err(e) => {
                        let error_msg = format!("Failed to wait for linux aplay process: {}", e);
                        error!("{}", error_msg);
                        *playback_error_clone.lock().await = Some(error_msg);
                        error_notify_clone.notify_one();
                    }
                }
                completion_notify_clone.notify_one();
                debug!("Linux aplay task completed and notified");
                drop(temp_file);
            })
        }

        #[cfg(not(any(target_os = "macos", target_os = "linux")))]
        {
            return Err("Audio playback is only supported on macOS and Linux".to_string());
        }
    };

    Ok(AudioPlaybackHandle {
        completion_notify,
        error_notify,
        playback_error,
        start_time: std::time::Instant::now(),
        playback_started,
        _task_handle: task_handle,
    })
}

/// Stop any ongoing TTS audio playback.
#[tauri::command]
pub fn stop_speech() {
    TTS_STOP_REQUESTED.store(true, Ordering::SeqCst);
    info!("TTS stop requested. Killing audio playback processes.");

    #[cfg(target_os = "macos")]
    let process_name = "afplay";
    #[cfg(target_os = "linux")]
    let process_name = "aplay";

    #[cfg(any(target_os = "macos", target_os = "linux"))]
    match std::process::Command::new("pkill").arg("-f").arg(process_name).output() {
        Ok(output) => {
            if output.status.success() {
                info!("Successfully terminated {} processes.", process_name);
            } else {
                let stderr = String::from_utf8_lossy(&output.stderr);
                if !stderr.contains("No matching processes") {
                    warn!(
                        "pkill for {} existed with status {} and error: {}",
                        process_name, output.status, stderr
                    );
                }
            }
        }
        Err(e) => {
            error!("Failed to execute pkill for {}: {}", process_name, e);
        }
    }
}

/// Check if a TTS stop has been requested.
pub fn is_tts_stop_requested() -> bool {
    TTS_STOP_REQUESTED.load(Ordering::SeqCst)
}

/// Reset the TTS stop flag.
#[tauri::command]
pub fn reset_tts_stop_flag() {
    TTS_STOP_REQUESTED.store(false, Ordering::SeqCst);
}

/// Check if TTS is currently playing.
pub fn is_tts_playing() -> bool {
    TTS_PLAYING.load(Ordering::SeqCst)
}

/// Set the TTS playing status.
fn set_tts_playing(playing: bool) {
    TTS_PLAYING.store(playing, Ordering::SeqCst);
}

#[tauri::command]
pub async fn stop_tts() -> Result<(), String> {
    stop_speech();
    Ok(())
}

/// Set the preferred TTS provider
#[tauri::command]
pub async fn set_tts_provider_command(
    provider: String,
    state: State<'_, AppState>,
) -> Result<(), String> {
    let mut config = state.tts_config.lock().await;
    config.provider = provider;
    Ok(())
}

/// Get the preferred TTS provider
#[tauri::command]
pub async fn get_tts_provider_command(
    state: State<'_, AppState>,
) -> Result<String, String> {
    let config = state.tts_config.lock().await;
    Ok(config.provider.clone())
}

#[tauri::command]
pub async fn invoke_tts(
    text: String,
    state: State<'_, AppState>,
    app_handle: AppHandle,
) -> Result<String, String> {
    let config = state.tts_config.lock().await;
    execute_tts_with_completion_tracking(text, &config.provider, &state, &app_handle).await
}

async fn execute_tts_with_completion_tracking(
    text: String,
    primary_provider: &str,
    state: &State<'_, AppState>,
    app_handle: &AppHandle,
) -> Result<String, String> {
    let _tts_guard = TTS_MUTEX.lock().await;
    info!("Starting TTS with provider: {}", primary_provider);
    reset_tts_stop_flag();
    set_tts_playing(true);

    let escape_key_coordinator = state.escape_key_coordinator.clone();
    escape_key_coordinator.register_key().await;

    let result = execute_tts_with_fallback(text, primary_provider, state, app_handle).await;

    escape_key_coordinator.deregister_key().await;
    set_tts_playing(false);
    info!("Finished TTS playback.");
    result
}

async fn execute_tts_with_fallback(
    text: String,
    primary_provider: &str,
    state: &State<'_, AppState>,
    _app_handle: &AppHandle,
) -> Result<String, String> {
    let filtered_text = filter_tts_content(&text);
    if filtered_text.is_empty() {
        return Ok("Content was filtered out, nothing to speak.".to_string());
    }

    match invoke_tts_for_provider(filtered_text.clone(), primary_provider).await {
        Ok(base64_audio) => {
            let playback_handle = play_base64_audio_with_tracking(&base64_audio).await?;
            playback_handle.wait_for_completion().await?;
            Ok(format!("Successfully played audio from {}", primary_provider))
        }
        Err(e) => {
            warn!(
                "Primary TTS provider {} failed: {}. Falling back to system TTS.",
                primary_provider, e
            );
            let fallback_provider = "system";
            match invoke_tts_for_provider(filtered_text, fallback_provider).await {
                Ok(base64_audio) => {
                    let playback_handle = play_base64_audio_with_tracking(&base64_audio).await?;
                    playback_handle.wait_for_completion().await?;
                    Ok(format!("Successfully played audio from fallback provider {}", fallback_provider))
                }
                Err(e) => Err(format!("All TTS providers failed: {}", e)),
            }
        }
    }
}

pub async fn invoke_tts_for_provider(
    text: String,
    provider: &str,
) -> Result<String, String> {
    info!("Invoking TTS for provider: {}", provider);
    match provider {
        "system" => system::speak(&text).await,
        "replicate" => replicate::speak_with_replicate(&text).await,
        "elevenlabs" => elevenlabs::speak_with_elevenlabs(&text).await,
        _ => Err(format!("Unknown TTS provider: {}", provider)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_filter_code_blocks() {
        let input = "Here is some code ```rust\nlet x = 5;\n``` and some text.";
        let expected = "Here is some code and some text.";
        assert_eq!(filter_tts_content(input), expected);
    }

    #[test]
    fn test_filter_inline_code() {
        let input = "Use the `println!` macro for printing.";
        let expected = "Use the macro for printing.";
        assert_eq!(filter_tts_content(input), expected);
    }

    #[test]
    fn test_filter_jsx_tags() {
        let input = "Here is a component <MyComponent prop=\"value\" />.";
        let expected = "Here is a component.";
        assert_eq!(filter_tts_content(input), expected);
    }

    #[test]
    fn test_filter_html_tags() {
        let input = "<p>This is a <strong>test</strong>.</p>";
        let expected = "This is a test.";
        assert_eq!(filter_tts_content(input), expected);
    }

    #[test]
    fn test_mixed_content() {
        let input = "Hello `world`! Check this ```js\nconsole.log('hi');\n``` out. And <Component />";
        let expected = "Hello! Check this out. And";
        assert_eq!(filter_tts_content(input), expected);
    }

    #[test]
    fn test_whitespace_normalization() {
        let input = "This has    extra   \n   spacing.";
        let expected = "This has extra spacing.";
        assert_eq!(filter_tts_content(input), expected);
    }

    #[test]
    fn test_empty_input() {
        let input = "";
        let expected = "";
        assert_eq!(filter_tts_content(input), expected);
    }

    #[test]
    fn test_only_code_input() {
        let input = "```python\nprint('hello')\n```";
        let expected = "";
        assert_eq!(filter_tts_content(input), expected);
    }
}

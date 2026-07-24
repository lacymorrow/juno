pub mod elevenlabs;
pub mod kokoro;
pub mod replicate;
pub mod supertonic;
pub mod system;

use crate::state::AppState;
use regex::Regex;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex as StdMutex, OnceLock};
use tauri::{AppHandle, State};
use tokio::sync::Mutex;
use tracing::{debug, error, info, warn};

// Global flags for TTS coordination
static TTS_STOP_REQUESTED: AtomicBool = AtomicBool::new(false);
static TTS_PLAYING: AtomicBool = AtomicBool::new(false);

// Global mutex for preventing concurrent TTS operations
static TTS_MUTEX: Mutex<()> = Mutex::const_new(());

// Global registry of PIDs for Juno-spawned audio processes (not system-wide killall)
static JUNO_AUDIO_PIDS: OnceLock<StdMutex<Vec<u32>>> = OnceLock::new();

fn audio_pid_registry() -> &'static StdMutex<Vec<u32>> {
    JUNO_AUDIO_PIDS.get_or_init(|| StdMutex::new(Vec::new()))
}

fn register_audio_pid(pid: u32) {
    match audio_pid_registry().lock() {
        Ok(mut pids) => {
            pids.push(pid);
        }
        Err(e) => {
            warn!("[TTS] Failed to register audio PID {}: {}", pid, e);
        }
    }
}

fn unregister_audio_pid(pid: u32) {
    match audio_pid_registry().lock() {
        Ok(mut pids) => {
            pids.retain(|&p| p != pid);
        }
        Err(e) => {
            warn!("[TTS] Failed to unregister audio PID {}: {}", pid, e);
        }
    }
}

// Structure to track audio playback completion with error propagation
#[derive(Debug)]
struct AudioPlaybackHandle {
    completion_notify: Arc<tokio::sync::Notify>,
    error_notify: Arc<tokio::sync::Notify>,
    playback_error: Arc<Mutex<Option<String>>>,
    start_time: std::time::Instant,
    #[allow(dead_code)] // May be used for future playback status checking
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
            info!(
                "Audio completed very quickly ({}ms), adding safety delay of {}ms",
                elapsed.as_millis(),
                safety_delay.as_millis()
            );
            tokio::time::sleep(safety_delay).await;
        }

        info!(
            "Audio playback completion confirmed after {}ms",
            elapsed.as_millis()
        );
        Ok(())
    }
}

/// Minimal TTS filtering: remove explicit TTS tags and normalize whitespace only.
/// The AI is expected to produce speakable text; avoid deterministic content stripping here.
pub fn filter_tts_content(text: &str) -> String {
    debug!("[TTS Filter] Original text length: {} chars", text.len());

    let mut filtered_text = text.to_string();

    // Remove only TTS tags (if present); do not strip other content
    filtered_text = match Regex::new(r"</?TTS>") {
        Ok(regex) => regex.replace_all(&filtered_text, "").to_string(),
        Err(e) => {
            warn!("Failed to compile regex '</?TTS>': {}", e);
            filtered_text
        }
    };

    // // 4. Remove function calls and method chaining (e.g., getData(), object.method())
    // let function_call_regex = Regex::new(r"\w+\([^)]*\)").unwrap();
    // filtered_text = function_call_regex.replace_all(&filtered_text, " ").to_string();

    // // 5. Remove property access patterns (e.g., object.property, config.server.port)
    // let property_access_regex = Regex::new(r"\w+\.\w+(\.\w+)*").unwrap();
    // filtered_text = property_access_regex.replace_all(&filtered_text, " ").to_string();

    // // 6. Remove URLs and file paths
    // let url_regex = Regex::new(r"https?://[^\s]+").unwrap();
    // filtered_text = url_regex.replace_all(&filtered_text, " ").to_string();
    // let path_regex = Regex::new(r"[/~][^\s]+").unwrap();
    // filtered_text = path_regex.replace_all(&filtered_text, " ").to_string();

    // // 7. Remove programming keywords and operators
    // let programming_regex = Regex::new(r"\b(const|let|var|if|else|function|return|class|import|export|from|async|await|try|catch|throw|new|this|super|extends|implements|interface|type|enum|namespace|module|public|private|protected|static|abstract|override|readonly|keyof|typeof|instanceof|in|of|for|while|do|switch|case|default|break|continue|finally|with|debugger|delete|void|yield|get|set)\b").unwrap();
    // filtered_text = programming_regex.replace_all(&filtered_text, " ").to_string();

    // // 8. Remove variable assignments and declarations
    // let assignment_regex = Regex::new(r"\w+\s*[=:]\s*").unwrap();
    // filtered_text = assignment_regex.replace_all(&filtered_text, " ").to_string();

    // // 9. Remove JSON structures
    // let json_regex = Regex::new(r"\{[^}]*\}|\[[^\]]*\]").unwrap();
    // filtered_text = json_regex.replace_all(&filtered_text, " ").to_string();

    // // 10. Remove CSS selectors and rules
    // let css_regex = Regex::new(r"\.[a-zA-Z-]+\s*\{[^}]*\}|#[a-zA-Z-]+\s*\{[^}]*\}|\w+\s*\{[^}]*\}").unwrap();
    // filtered_text = css_regex.replace_all(&filtered_text, " ").to_string();

    // // 11. Remove emojis (Unicode ranges for common emoji blocks)
    // let emoji_regex = Regex::new(r"[\u{1f600}-\u{1f64f}]|[\u{1f300}-\u{1f5ff}]|[\u{1f680}-\u{1f6ff}]|[\u{1f1e0}-\u{1f1ff}]|[\u{2600}-\u{26ff}]|[\u{2700}-\u{27bf}]").unwrap();
    // filtered_text = emoji_regex.replace_all(&filtered_text, " ").to_string();

    // Clean up whitespace and normalize
    match Regex::new(r"\s+") {
        Ok(whitespace_regex) => {
            filtered_text = whitespace_regex
                .replace_all(&filtered_text, " ")
                .to_string();
        }
        Err(e) => {
            tracing::warn!("Failed to compile whitespace regex: {}", e);
            // Fallback to simple space normalization
            filtered_text = filtered_text
                .split_whitespace()
                .collect::<Vec<_>>()
                .join(" ");
        }
    }
    filtered_text = filtered_text.trim().to_string();

    debug!(
        "[TTS Filter] Filtered text length: {} chars",
        filtered_text.len()
    );
    if filtered_text.len() != text.len() {
        debug!(
            "[TTS Filter] Content was filtered: '{}' -> '{}'",
            text.chars().take(100).collect::<String>(),
            filtered_text.chars().take(100).collect::<String>()
        );
    }

    filtered_text
}

/// Play base64 audio with proper completion tracking and error handling
/// Returns an AudioPlaybackHandle that can be awaited for completion
async fn play_base64_audio_with_tracking(
    base64_audio: &str,
) -> Result<AudioPlaybackHandle, String> {
    use base64::prelude::*;
    use std::io::Write;
    use tempfile::Builder as TempFileBuilder;

    info!(
        "Playing TTS audio with completion tracking ({} bytes)",
        base64_audio.len()
    );

    // Decode base64 audio data
    let audio_bytes = BASE64_STANDARD
        .decode(base64_audio)
        .map_err(|e| format!("Failed to decode base64 TTS audio: {}", e))?;

    // Create temporary file for audio playback
    let mut temp_file = TempFileBuilder::new()
        .prefix("tts_audio_")
        .suffix(".m4a") // Use .m4a for compatibility
        .tempfile()
        .map_err(|e| format!("Failed to create temporary file for TTS audio: {}", e))?;

    // Write audio data to temporary file
    temp_file
        .write_all(&audio_bytes)
        .map_err(|e| format!("Failed to write TTS audio to temporary file: {}", e))?;

    temp_file
        .flush()
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

            // Capture PID before moving child into the task so we can kill it precisely
            let child_pid = child.id();
            if let Some(pid) = child_pid {
                register_audio_pid(pid);
            }

            let playback_started_clone = playback_started.clone();

            // FIXED: Move temp_file into the spawned task to ensure proper lifecycle management
            tokio::spawn(async move {
                // Add a small delay to ensure afplay has time to start
                tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
                playback_started_clone.store(true, Ordering::SeqCst);

                let result = child.wait().await;

                // Unregister PID now that the process has exited
                if let Some(pid) = child_pid {
                    unregister_audio_pid(pid);
                }

                match result {
                    Ok(status) => {
                        if status.success() {
                            info!("macOS afplay completed successfully");
                        } else {
                            let error_msg =
                                format!("macOS afplay exited with non-zero status: {}", status);
                            error!("{}", error_msg);

                            // Store error for propagation
                            *playback_error_clone.lock().await = Some(error_msg);
                            error_notify_clone.notify_one();

                            // Check if it failed immediately (before actually playing audio)
                            let pid_check =
                                std::process::Command::new("pgrep").arg("afplay").output();

                            if let Ok(output) = pid_check {
                                if output.stdout.is_empty() {
                                    warn!(
                                        "afplay process not found - audio may have failed to start"
                                    );
                                }
                            }
                        }
                    }
                    Err(e) => {
                        let error_msg = format!("Failed to wait for macOS afplay process: {}", e);
                        error!("{}", error_msg);

                        // Store error for propagation
                        *playback_error_clone.lock().await = Some(error_msg);
                        error_notify_clone.notify_one();
                    }
                }

                // Notify completion regardless of success/failure
                completion_notify_clone.notify_one();
                debug!("macOS afplay task completed and notified");

                // FIXED: Keep temp file alive until task completes - prevents race condition
                // temp_file is now owned by this task and will be dropped here
                drop(temp_file);
            })
        }

        #[cfg(target_os = "linux")]
        {
            let mut child = tokio::process::Command::new("aplay")
                .arg(&temp_path)
                .spawn()
                .map_err(|e| format!("Failed to spawn aplay: {}", e))?;

            // Capture PID before moving child into the task so we can kill it precisely
            let child_pid = child.id();
            if let Some(pid) = child_pid {
                register_audio_pid(pid);
            }

            let playback_started_clone = playback_started.clone();

            // FIXED: Move temp_file into the spawned task to ensure proper lifecycle management
            tokio::spawn(async move {
                // Add a small delay to ensure aplay has time to start
                tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
                playback_started_clone.store(true, Ordering::SeqCst);

                let result = child.wait().await;

                // Unregister PID now that the process has exited
                if let Some(pid) = child_pid {
                    unregister_audio_pid(pid);
                }

                match result {
                    Ok(status) => {
                        if status.success() {
                            info!("Linux aplay completed successfully");
                        } else {
                            let error_msg =
                                format!("Linux aplay exited with non-zero status: {}", status);
                            error!("{}", error_msg);

                            // Store error for propagation
                            *playback_error_clone.lock().await = Some(error_msg);
                            error_notify_clone.notify_one();

                            // Check if it failed immediately
                            let pid_check =
                                std::process::Command::new("pgrep").arg("aplay").output();

                            if let Ok(output) = pid_check {
                                if output.stdout.is_empty() {
                                    warn!(
                                        "aplay process not found - audio may have failed to start"
                                    );
                                }
                            }
                        }
                    }
                    Err(e) => {
                        let error_msg = format!("Failed to wait for Linux aplay process: {}", e);
                        error!("{}", error_msg);

                        // Store error for propagation
                        *playback_error_clone.lock().await = Some(error_msg);
                        error_notify_clone.notify_one();
                    }
                }

                completion_notify_clone.notify_one();
                debug!("Linux aplay task completed and notified");

                // FIXED: Keep temp file alive until task completes - prevents race condition
                // temp_file is now owned by this task and will be dropped here
                drop(temp_file);
            })
        }

        #[cfg(not(any(target_os = "macos", target_os = "linux")))]
        {
            return Err("Audio playback is only supported on macOS and Linux".to_string());
        }
    };

    // FIXED: Return handle with actual task handle and error propagation support
    Ok(AudioPlaybackHandle {
        completion_notify,
        error_notify,
        playback_error,
        start_time: std::time::Instant::now(),
        playback_started,
        _task_handle: task_handle,
    })
}

/// Legacy wrapper for compatibility - now properly waits for completion with error propagation
#[allow(dead_code)] // Direct audio playback function - kept for future use
async fn play_base64_audio_directly(base64_audio: &str) -> Result<(), String> {
    let handle = play_base64_audio_with_tracking(base64_audio).await?;
    handle.wait_for_completion().await?;
    info!("TTS audio playback completed");
    Ok(())
}

// Stop speech playback by sending SIGTERM only to PIDs that Juno spawned.
// This replaces the previous `killall afplay/say/aplay` approach, which terminated
// all system-wide instances and interfered with other apps' audio.
pub fn stop_speech() {
    info!("[TTS] Stop speech requested - killing Juno-owned audio processes");
    TTS_STOP_REQUESTED.store(true, Ordering::SeqCst);

    let pids_to_kill: Vec<u32> = match audio_pid_registry().lock() {
        Ok(pids) => pids.clone(),
        Err(e) => {
            warn!("[TTS] Failed to read audio PID registry: {}", e);
            vec![]
        }
    };

    if pids_to_kill.is_empty() {
        debug!("[TTS] No Juno-owned audio processes to stop");
        return;
    }

    info!(
        "[TTS] Stopping {} Juno audio process(es): {:?}",
        pids_to_kill.len(),
        pids_to_kill
    );

    #[cfg(unix)]
    for pid in pids_to_kill {
        let result = unsafe { libc::kill(pid as libc::pid_t, libc::SIGTERM) };
        if result == 0 {
            debug!("[TTS] Sent SIGTERM to Juno audio process PID {}", pid);
        } else {
            // ESRCH (errno 3) means process already exited — not an error
            debug!(
                "[TTS] kill({}) returned error: {} (process may have already exited)",
                pid,
                std::io::Error::last_os_error()
            );
        }
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
    let coordinator = crate::commands::escape_key_coordinator::get_escape_key_coordinator();
    if let Err(e) = coordinator
        .register_escape_user(app_handle, "tts_playback")
        .await
    {
        warn!("[TTS] Failed to register escape key for TTS: {} - TTS will still work but escape key may not stop it", e);
    } else {
        info!("[TTS] Registered escape key for TTS cancellation");
    }
}

// Unregister escape key after TTS completion - CENTRALIZED
pub async fn unregister_tts_escape_key(app_handle: &AppHandle) {
    let coordinator = crate::commands::escape_key_coordinator::get_escape_key_coordinator();
    if let Err(e) = coordinator
        .unregister_escape_user(app_handle, "tts_playback")
        .await
    {
        warn!(
            "[TTS] Failed to unregister escape key after TTS: {} - continuing anyway",
            e
        );
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
    let valid_providers = [
        "off",
        "system",
        "elevenlabs",
        "replicate",
        "kokoro",
        "chatterbox",
        "supertonic",
    ];
    if !valid_providers.contains(&provider.as_str()) {
        return Err(format!(
            "Invalid TTS provider: {}. Valid providers: {:?}",
            provider, valid_providers
        ));
    }

    // Get current settings from centralized system
    let settings_manager = crate::settings::manager::SettingsManager::new(app_handle.clone())
        .map_err(|e| format!("Failed to create settings manager: {}", e))?;

    let mut audio_settings = settings_manager
        .get_audio_settings()
        .await
        .map_err(|e| format!("Failed to get audio settings: {}", e))?;

    // Update centralized settings
    audio_settings.tts_provider = provider.clone();
    settings_manager
        .set_audio_settings(&audio_settings)
        .await
        .map_err(|e| format!("Failed to save audio settings: {}", e))?;

    // Update app state for backward compatibility
    state
        .set_tts_provider(provider.clone())
        .map_err(|e| format!("Failed to set tts_provider: {}", e))?;

    info!(
        "TTS provider set to: {} (saved to centralized settings)",
        provider
    );
    Ok(())
}

// Command to get the Kokoro voice from centralized settings
#[tauri::command]
pub async fn get_kokoro_voice_command(app_handle: AppHandle) -> Result<String, String> {
    let settings_manager = crate::settings::manager::SettingsManager::new(app_handle.clone())
        .map_err(|e| format!("Failed to create settings manager: {}", e))?;
    let audio_settings = settings_manager
        .get_audio_settings()
        .await
        .map_err(|e| format!("Failed to get audio settings: {}", e))?;
    Ok(audio_settings.kokoro_voice)
}

// Command to set the Kokoro voice in centralized settings
#[tauri::command]
pub async fn set_kokoro_voice_command(
    voice: String,
    app_handle: AppHandle,
    state: State<'_, AppState>,
) -> Result<(), String> {
    info!("Setting Kokoro voice to: {}", voice);

    let settings_manager = crate::settings::manager::SettingsManager::new(app_handle.clone())
        .map_err(|e| format!("Failed to create settings manager: {}", e))?;

    let mut audio_settings = settings_manager
        .get_audio_settings()
        .await
        .map_err(|e| format!("Failed to get audio settings: {}", e))?;

    audio_settings.kokoro_voice = voice.clone();
    settings_manager
        .set_audio_settings(&audio_settings)
        .await
        .map_err(|e| format!("Failed to save audio settings: {}", e))?;

    state
        .set_kokoro_voice(voice.clone())
        .map_err(|e| format!("Failed to set kokoro_voice in state: {}", e))?;

    info!("Kokoro voice set to: {}", voice);
    Ok(())
}

// Command to get Chatterbox settings
#[tauri::command]
pub async fn get_chatterbox_settings_command(
    app_handle: AppHandle,
) -> Result<serde_json::Value, String> {
    let settings_manager = crate::settings::manager::SettingsManager::new(app_handle)
        .map_err(|e| format!("Failed to create settings manager: {}", e))?;
    let audio_settings = settings_manager
        .get_audio_settings()
        .await
        .map_err(|e| format!("Failed to get audio settings: {}", e))?;
    Ok(serde_json::json!({
        "reference_audio_url": audio_settings.chatterbox_reference_audio_url,
        "exaggeration": audio_settings.chatterbox_exaggeration,
        "use_hd": audio_settings.chatterbox_use_hd,
    }))
}

// Command to update Chatterbox settings
#[tauri::command]
pub async fn set_chatterbox_settings_command(
    reference_audio_url: Option<String>,
    exaggeration: f32,
    use_hd: bool,
    app_handle: AppHandle,
    state: State<'_, AppState>,
) -> Result<(), String> {
    info!(
        "Setting Chatterbox settings: ref_audio={:?}, exaggeration={:.2}, hd={}",
        reference_audio_url, exaggeration, use_hd
    );

    if !(0.0..=2.0).contains(&exaggeration) {
        return Err(format!(
            "Chatterbox exaggeration must be between 0.0 and 2.0, got {}",
            exaggeration
        ));
    }

    let settings_manager = crate::settings::manager::SettingsManager::new(app_handle)
        .map_err(|e| format!("Failed to create settings manager: {}", e))?;

    let mut audio_settings = settings_manager
        .get_audio_settings()
        .await
        .map_err(|e| format!("Failed to get audio settings: {}", e))?;

    audio_settings.chatterbox_reference_audio_url = reference_audio_url.clone();
    audio_settings.chatterbox_exaggeration = exaggeration;
    audio_settings.chatterbox_use_hd = use_hd;

    settings_manager
        .set_audio_settings(&audio_settings)
        .await
        .map_err(|e| format!("Failed to save Chatterbox settings: {}", e))?;

    state
        .set_chatterbox_reference_audio_url(reference_audio_url)
        .map_err(|e| {
            format!(
                "Failed to set Chatterbox reference audio URL in state: {}",
                e
            )
        })?;
    state
        .set_chatterbox_exaggeration(exaggeration)
        .map_err(|e| format!("Failed to set Chatterbox exaggeration in state: {}", e))?;
    state
        .set_chatterbox_use_hd(use_hd)
        .map_err(|e| format!("Failed to set Chatterbox HD mode in state: {}", e))?;

    info!("Chatterbox settings saved");
    Ok(())
}

// New command to get current TTS provider
#[tauri::command]
pub async fn get_tts_provider_command(app_handle: AppHandle) -> Result<String, String> {
    // Get provider from centralized settings
    let settings_manager = crate::settings::manager::SettingsManager::new(app_handle.clone())
        .map_err(|e| format!("Failed to create settings manager: {}", e))?;

    let audio_settings = settings_manager
        .get_audio_settings()
        .await
        .map_err(|e| format!("Failed to get audio settings: {}", e))?;

    // Reduced logging frequency - only log at debug level
    tracing::debug!(
        "Current TTS provider from centralized settings: {}",
        audio_settings.tts_provider
    );
    Ok(audio_settings.tts_provider)
}

// FIXED: Proper concurrency control, stop flag lifecycle, and state access
#[tauri::command]
pub async fn invoke_tts(
    text: String,
    state: State<'_, AppState>,
    app_handle: AppHandle,
) -> Result<String, String> {
    // CRITICAL FIX 1: Use mutex to prevent any race conditions in concurrent access
    let _guard = TTS_MUTEX.lock().await;

    // CRITICAL FIX 2: Double-check TTS playing state after acquiring mutex
    if is_tts_playing() {
        info!("TTS is already playing, ignoring new request to prevent overlapping audio");
        return Ok("TTS_ALREADY_PLAYING".to_string());
    }

    // CRITICAL FIX 3: Reset stop flag at the start of each operation
    reset_tts_stop_flag();

    let provider = state
        .get_tts_provider()
        .map_err(|e| format!("Failed to get tts_provider for invoke_tts: {}", e))?;

    if provider.is_empty() || provider.to_lowercase() == "off" {
        let short_text = text.chars().take(30).collect::<String>();
        info!(
            "TTS is set to '{}'. Skipping TTS for text: {}...",
            provider, short_text
        );
        return Ok("TTS_DISABLED_BY_SETTING".to_string());
    }

    // Filter content to prevent code, emojis, and unwanted content from being spoken
    let filtered_text = filter_tts_content(&text);

    // If filtering removed all content, skip TTS
    if filtered_text.is_empty() {
        info!("TTS content was filtered out (appears to be code/unwanted content), skipping TTS");
        return Ok("TTS_CONTENT_FILTERED".to_string());
    }

    // CRITICAL FIX 4: Set TTS as playing AFTER acquiring mutex to prevent race conditions
    set_tts_playing(true);

    // CRITICAL FIX 5: Register escape key management
    register_tts_escape_key(&app_handle).await;

    // Execute TTS with proper completion tracking
    let result =
        execute_tts_with_completion_tracking(filtered_text, &provider, &state, &app_handle).await;

    // CRITICAL FIX 6: Cleanup happens in execute_tts_with_completion_tracking after actual audio completion
    result
}

// Execute TTS with proper completion tracking and cleanup
async fn execute_tts_with_completion_tracking(
    text: String,
    primary_provider: &str,
    state: &State<'_, AppState>,
    app_handle: &AppHandle,
) -> Result<String, String> {
    info!("Starting TTS with provider: {}", primary_provider);

    // Clone AppState (cheap — all fields are Arc<>) so settings propagate to provider dispatch
    let app_state = (**state).clone();

    // Execute TTS with fallback logic
    let result = match execute_tts_with_fallback(text, primary_provider, app_state).await {
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
                // This should be base64 audio data - play it with completion tracking!
                info!("TTS audio generated, attempting playback with completion tracking...");

                // Check if stop was requested before playback
                if is_tts_stop_requested() {
                    info!("TTS stop was requested before playback, aborting");
                    return Ok("TTS_STOPPED_BY_USER".to_string());
                }

                // Access current state instead of using cloned/stale state
                match state.get_sound_enabled() {
                    Ok(sound_enabled) => {
                        if !sound_enabled {
                            info!("Sound is disabled, skipping TTS audio playback");
                            Ok("TTS_SOUND_DISABLED".to_string())
                        } else {
                            // CRITICAL FIX: Use completion tracking with enhanced error detection and proper error propagation
                            match play_base64_audio_with_tracking(&result).await {
                                Ok(handle) => {
                                    info!("TTS audio playback started, waiting for completion...");

                                    // Enhanced completion tracking with safeguards and error propagation
                                    let completion_start = std::time::Instant::now();

                                    // Wait for actual audio completion or stop signal
                                    let playback_result = tokio::select! {
                                        completion_result = handle.wait_for_completion() => {
                                            match completion_result {
                                                Ok(()) => {
                                                    let total_duration = completion_start.elapsed();
                                                    info!("TTS audio playback completed successfully after {}ms", total_duration.as_millis());

                                                    // SAFEGUARD: If completion happened too quickly, check if audio actually played
                                                    if total_duration < std::time::Duration::from_millis(300) {
                                                        warn!("Audio completed very quickly ({}ms), verifying no audio processes are still running", total_duration.as_millis());

                                                        // Give any remaining audio processes time to complete
                                                        tokio::time::sleep(std::time::Duration::from_millis(500)).await;

                                                        // Check for remaining audio processes
                                                        #[cfg(target_os = "macos")]
                                                        {
                                                            let afplay_check = std::process::Command::new("pgrep")
                                                                .arg("afplay")
                                                                .output();

                                                            if let Ok(output) = afplay_check {
                                                                if !output.stdout.is_empty() {
                                                                    info!("Found running afplay processes, waiting for them to complete...");
                                                                    tokio::time::sleep(std::time::Duration::from_millis(1000)).await;
                                                                }
                                                            }
                                                        }
                                                    }

                                                    Ok("TTS_COMPLETED".to_string())
                                                }
                                                Err(playback_error) => {
                                                    error!("TTS audio playback failed: {}", playback_error);
                                                    Err(format!("Audio playback failed: {}", playback_error))
                                                }
                                            }
                                        }
                                        _ = async {
                                            // Poll for stop signal every 100ms
                                            loop {
                                                if is_tts_stop_requested() {
                                                    info!("TTS stop requested during playback");
                                                    stop_speech(); // Kill any running audio processes
                                                    break;
                                                }
                                                tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
                                            }
                                        } => {
                                            info!("TTS playback was stopped by user");
                                            Ok("TTS_STOPPED_BY_USER".to_string())
                                        }
                                    };

                                    playback_result
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
    };

    // CRITICAL FIX: Always clean up after actual completion (not just function completion)
    set_tts_playing(false);
    unregister_tts_escape_key(app_handle).await;
    info!("TTS operation completed, flags and escape key cleaned up");

    result
}

// Execute TTS with fallback logic (no blocking, no race conditions)
async fn execute_tts_with_fallback(
    text: String,
    primary_provider: &str,
    app_state: AppState,
) -> Result<String, String> {
    // Check network connectivity for cloud-based providers
    let is_cloud_provider = matches!(
        primary_provider.to_lowercase().as_str(),
        "replicate" | "elevenlabs" | "chatterbox"
    );

    // If it's a cloud provider, do a quick network check first
    if is_cloud_provider {
        info!("Cloud TTS provider detected, checking network connectivity...");
        let is_online = crate::utils::network::is_online().await;
        if !is_online {
            warn!("Device appears offline, using system TTS directly");
            return invoke_tts_for_provider(text, Some(app_state), "system").await;
        }
    }

    // Define the provider fallback order based on the primary provider
    let fallback_providers = match primary_provider.to_lowercase().as_str() {
        "replicate" => vec!["replicate", "kokoro", "system"],
        "elevenlabs" => vec!["elevenlabs", "kokoro", "system"],
        "chatterbox" => vec!["chatterbox", "kokoro", "system"],
        "supertonic" => vec!["supertonic", "kokoro", "system"],
        "kokoro" => vec!["kokoro", "system"],
        "system" => vec!["system"],
        "off" => return Ok("TTS_DISABLED_BY_SETTING".to_string()),
        _ => {
            warn!(
                "Unknown primary TTS provider: '{}'. Using system fallback only.",
                primary_provider
            );
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
        info!(
            "Attempting TTS with provider: {} ({})",
            fallback_provider,
            if is_primary { "primary" } else { "fallback" }
        );

        match invoke_tts_for_provider(text.clone(), Some(app_state.clone()), fallback_provider)
            .await
        {
            Ok(result) => {
                if result == "TTS_STOPPED_BY_USER" {
                    return Ok(result);
                }
                if !is_primary {
                    warn!(
                        "Primary TTS provider '{}' failed, but fallback '{}' succeeded",
                        primary_provider, fallback_provider
                    );
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
                    match invoke_tts_for_provider(text.clone(), Some(app_state.clone()), "system")
                        .await
                    {
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
    _state: Option<AppState>,
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
        "kokoro" => {
            let voice = _state
                .as_ref()
                .and_then(|s| s.get_kokoro_voice().ok())
                .unwrap_or_else(|| "af_bella".to_string());
            kokoro::invoke_kokoro_tts(text, voice).await
        }
        "replicate" => replicate::invoke_replicate_tts(text).await,
        "chatterbox" => {
            let (ref_url, exaggeration, use_hd) = _state
                .as_ref()
                .map(|s| {
                    (
                        s.get_chatterbox_reference_audio_url().ok().flatten(),
                        s.get_chatterbox_exaggeration().unwrap_or(0.5),
                        s.get_chatterbox_use_hd().unwrap_or(false),
                    )
                })
                .unwrap_or((None, 0.5, false));
            replicate::invoke_chatterbox_tts(text, ref_url, exaggeration, use_hd).await
        }
        "supertonic" => {
            let (server_url, voice, speed) = _state
                .as_ref()
                .map(|s| {
                    (
                        s.get_supertonic_server_url()
                            .unwrap_or_else(|_| supertonic::DEFAULT_SERVER_URL.to_string()),
                        s.get_supertonic_voice()
                            .unwrap_or_else(|_| supertonic::DEFAULT_VOICE.to_string()),
                        s.get_supertonic_speed()
                            .unwrap_or(supertonic::DEFAULT_SPEED),
                    )
                })
                .unwrap_or((
                    supertonic::DEFAULT_SERVER_URL.to_string(),
                    supertonic::DEFAULT_VOICE.to_string(),
                    supertonic::DEFAULT_SPEED,
                ));
            supertonic::invoke_supertonic_tts(text, server_url, voice, speed).await
        }
        "system" => system::invoke_system_tts(text).await,
        "off" => {
            warn!("invoke_tts_for_provider called with 'off', this should ideally be handled by invoke_tts. Skipping.");
            Ok("TTS_DISABLED_BY_SETTING".to_string())
        }
        _ => {
            warn!(
                "Unknown TTS provider specified: '{}'. Cannot invoke.",
                provider
            );
            Err(format!("Unknown TTS provider: {}", provider))
        }
    }
}

#[tauri::command]
pub async fn get_supertonic_settings_command(
    app_handle: AppHandle,
) -> Result<serde_json::Value, String> {
    let settings_manager = crate::settings::manager::SettingsManager::new(app_handle)
        .map_err(|e| format!("Failed to create settings manager: {}", e))?;
    let audio_settings = settings_manager
        .get_audio_settings()
        .await
        .map_err(|e| format!("Failed to get audio settings: {}", e))?;
    Ok(serde_json::json!({
        "server_url": audio_settings.supertonic_server_url,
        "voice": audio_settings.supertonic_voice,
        "speed": audio_settings.supertonic_speed,
    }))
}

#[tauri::command]
pub async fn set_supertonic_settings_command(
    server_url: String,
    voice: String,
    speed: f64,
    app_handle: AppHandle,
    state: State<'_, AppState>,
) -> Result<(), String> {
    info!(
        "Setting Supertonic settings: server_url={}, voice={}, speed={:.2}",
        server_url, voice, speed
    );

    if !(0.5..=2.0).contains(&speed) {
        return Err(format!(
            "Supertonic speed must be between 0.5 and 2.0, got {}",
            speed
        ));
    }

    let settings_manager = crate::settings::manager::SettingsManager::new(app_handle)
        .map_err(|e| format!("Failed to create settings manager: {}", e))?;

    let mut audio_settings = settings_manager
        .get_audio_settings()
        .await
        .map_err(|e| format!("Failed to get audio settings: {}", e))?;

    audio_settings.supertonic_server_url = server_url.clone();
    audio_settings.supertonic_voice = voice.clone();
    audio_settings.supertonic_speed = speed;

    settings_manager
        .set_audio_settings(&audio_settings)
        .await
        .map_err(|e| format!("Failed to save Supertonic settings: {}", e))?;

    state
        .set_supertonic_server_url(server_url)
        .map_err(|e| format!("Failed to set Supertonic server URL in state: {}", e))?;
    state
        .set_supertonic_voice(voice)
        .map_err(|e| format!("Failed to set Supertonic voice in state: {}", e))?;
    state
        .set_supertonic_speed(speed)
        .map_err(|e| format!("Failed to set Supertonic speed in state: {}", e))?;

    info!("Supertonic settings saved");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_filter_code_blocks() {
        let input = "Here's some text ```rust\nfn hello() {\n    println!(\"world\");\n}\n``` and more text.";
        // Minimal filtering retains code; only whitespace normalized
        let result = filter_tts_content(input);
        assert!(result.contains("```"));
    }

    #[test]
    fn test_filter_inline_code() {
        let input = "Use the `console.log()` function to debug your `variable` values.";
        let result = filter_tts_content(input);
        assert!(result.contains("`console.log()`"));
    }

    #[test]
    fn test_filter_jsx_tags() {
        let input = "Here's a React component: <Card><CardContent><div className=\"flex justify-center my-4\">Hello</div></CardContent></Card>";
        let result = filter_tts_content(input);
        assert!(result.contains("<Card>"));
    }

    #[test]
    fn test_filter_html_tags() {
        let input = "This is <strong>bold</strong> and <em>italic</em> text.";
        let result = filter_tts_content(input);
        assert!(result.contains("<strong>bold</strong>"));
    }

    #[test]
    fn test_filter_function_calls() {
        let input = "Call the function getData() and then processResult(data) to continue.";
        let result = filter_tts_content(input);
        assert!(result.contains("getData()"));
    }

    #[test]
    fn test_filter_method_chaining() {
        let input = "Use object.method().anotherMethod() to chain calls.";
        let result = filter_tts_content(input);
        assert!(result.contains("object.method().anotherMethod()"));
    }

    #[test]
    fn test_filter_property_access() {
        let input = "Access config.server.port for the port number.";
        let result = filter_tts_content(input);
        assert!(result.contains("config.server.port"));
    }

    #[test]
    fn test_filter_urls_and_paths() {
        let input =
            "Visit https://example.com or check /home/user/file.txt and ~/documents/readme.md";
        let result = filter_tts_content(input);
        assert!(result.contains("https://example.com"));
    }

    #[test]
    fn test_filter_programming_keywords() {
        let input = "const myVar = 5; let result = getData(); if (condition) { return value; }";
        let result = filter_tts_content(input);
        assert!(result.contains("const myVar = 5;"));
    }

    #[test]
    fn test_filter_emojis() {
        let input = "Hello world! 😀 This is great! 🎉 Let's code! 💻";
        let result = filter_tts_content(input);
        assert!(result.contains("😀"));
    }

    #[test]
    fn test_filter_json_structures() {
        let input =
            "The config is {\"port\": 8080, \"host\": \"localhost\"} and array is [1, 2, 3].";
        let result = filter_tts_content(input);
        assert!(result.contains("\"port\": 8080"));
    }

    #[test]
    fn test_filter_css() {
        let input = "Add .button { color: red; } to your stylesheet.";
        let result = filter_tts_content(input);
        assert!(result.contains(".button { color: red; }"));
    }

    #[test]
    fn test_mostly_code_content_returns_not_empty() {
        let input = "```javascript\nconst x = 5;\n```";
        // Minimal filtering keeps content
        let result = filter_tts_content(input);
        assert!(!result.is_empty());
    }

    #[test]
    fn test_jsx_example_from_logs() {
        let input = "<Card><CardContent><div className=\"flex justify-center my-4\">Content here</div></CardContent></Card>";
        let result = filter_tts_content(input);
        assert!(result.contains("Content here"));
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
        let result = filter_tts_content(input);
        // Ensure content is largely preserved and whitespace normalized
        assert!(result.contains("```code block```"));
        assert!(result.contains("inline code"));
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
        let result = filter_tts_content(input);
        assert!(result.contains("myVariable = 42"));
        assert!(result.contains("config: value"));
    }
}

//! The Mac's own voice: `say` with no `-v`, and nothing else.
//!
//! `say` with no voice speaks the System Voice chosen under System Settings >
//! Accessibility > Read & Speak, Siri voices included. Nothing else on the Mac
//! reaches a Siri voice: AVFoundation lists none to a third-party app and
//! cannot build one by id. So Juno never names a Mac voice. A `-v` can only
//! ever pick a MacinTalk or Vocalizer voice (Samantha and friends), which is
//! exactly the voice Juno must never speak in. Whether the System Voice is
//! allowed at all is decided in `voices::mac_voice_is_siri_or_unknown`.
//!
//! The cost is latency: each `say` spends 0.8 to 2.0 s before its first sound
//! (`docs/research/say-latency.md`). A Siri voice is worth it.

#[cfg(target_os = "macos")]
use crate::constants::errors::templates;
#[cfg(target_os = "macos")]
use base64::{engine::general_purpose::STANDARD as BASE64_STANDARD, Engine as _};
#[cfg(target_os = "macos")]
use std::fs;
#[cfg(target_os = "macos")]
use std::process::Command;
#[cfg(target_os = "macos")]
use tempfile::NamedTempFile;
#[cfg(target_os = "macos")]
use tracing::{error, info};

// Helper function for error formatting - properly handles template substitution
fn format_error(template: &str, context: &str, error: impl std::fmt::Display) -> String {
    template
        .replacen("{}", context, 1)
        .replacen("{}", &error.to_string(), 1)
}

/// The `say` arguments for one utterance.
///
/// Pure so the speaker a person chose in Audio settings can be proved to
/// reach the command. There is deliberately no voice parameter: `-v` is how a
/// non-Siri voice gets spoken, so it cannot be built here at all.
///
/// `-a` is the output device by name, omitted when nothing was chosen, which
/// leaves `say` on the Mac's own output.
pub fn say_arguments(text: &str, device: Option<&str>) -> Vec<String> {
    say_arguments_at(text, device, None)
}

/// [`say_arguments`] with a pace: `-r` is words per minute, and `None` leaves
/// `say` at the Mac's own.
pub fn say_arguments_at(
    text: &str,
    device: Option<&str>,
    words_per_minute: Option<u32>,
) -> Vec<String> {
    let mut args = Vec::new();
    if let Some(wpm) = words_per_minute {
        args.push("-r".to_string());
        args.push(wpm.to_string());
    }
    if let Some(device) = device.map(str::trim).filter(|d| !d.is_empty()) {
        args.push("-a".to_string());
        args.push(device.to_string());
    }
    args.push(text.to_string());
    args
}

/// Speak straight out of `say`, in the System Voice.
///
/// `say` starts speaking as soon as it has loaded and streams as it goes, so
/// nothing is rendered to a file first.
#[cfg(target_os = "macos")]
pub async fn speak_with_say(
    text: String,
    device: Option<String>,
    words_per_minute: Option<u32>,
) -> Result<String, String> {
    if crate::tts::is_tts_stop_requested() {
        return Ok("TTS_STOPPED_BY_USER".to_string());
    }

    info!(
        "Speaking via system TTS: {} chars, the System Voice, out of {}",
        text.chars().count(),
        device.as_deref().unwrap_or("the system output")
    );

    let spawned = std::time::Instant::now();
    let status = run_say(say_arguments_at(&text, device.as_deref(), words_per_minute)).await?;
    // `say` gives no signal when sound begins, so only the whole run is known.
    info!(
        "[SpeechTiming] engine=say first_audio_ms=- total_ms={} chars={}",
        spawned.elapsed().as_millis(),
        text.chars().count()
    );

    // A stop arrives as SIGTERM to that pid, so a non-success exit right after
    // one is the user pressing Escape rather than a failure worth reporting.
    if crate::tts::is_tts_stop_requested() {
        return Ok("TTS_STOPPED_BY_USER".to_string());
    }
    if status.success() {
        return Ok("TTS_COMPLETED".to_string());
    }
    Err(format!("'say' exited with {}", status))
}

/// Run one `say` and wait for it.
///
/// The child's pid joins the same registry afplay's does, so Escape stops this
/// exactly the way it stops everything else Juno is playing.
#[cfg(target_os = "macos")]
async fn run_say(args: Vec<String>) -> Result<std::process::ExitStatus, String> {
    let mut child = tokio::process::Command::new("say")
        .args(args)
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .kill_on_drop(true)
        .spawn()
        .map_err(|e| format!("Failed to start 'say': {}", e))?;

    let pid = child.id();
    if let Some(pid) = pid {
        crate::tts::register_audio_pid(pid);
    }

    // `say` plays out of its own process, so its samples are never visible
    // here. The mouth gets a talking rhythm that starts when `say` starts
    // sounding and stops the moment it exits or is killed.
    let level_session = crate::tts::speech_level::begin(
        crate::tts::speech_level::LevelSource::Synthetic,
        std::time::Duration::from_millis(crate::tts::speech_level::SAY_LEAD_MS),
    );
    let status = child.wait().await;
    drop(level_session);

    if let Some(pid) = pid {
        crate::tts::unregister_audio_pid(pid);
    }

    status.map_err(|e| format!("Failed to wait for 'say': {}", e))
}

#[cfg(target_os = "macos")]
#[tauri::command]
pub async fn invoke_system_tts(text: String) -> Result<String, String> {
    info!("Invoking macOS system TTS for text: {}", text);

    // Check if stop was requested before starting
    if crate::tts::is_tts_stop_requested() {
        info!("TTS stop was requested before starting system TTS, aborting");
        return Ok("TTS_STOPPED_BY_USER".to_string());
    }

    let temp_file = NamedTempFile::new()
        .map_err(|e| format_error(templates::FAILED_TO_CREATE, "temporary file", e))?;
    let temp_path = temp_file.path().to_path_buf();

    // Use .m4a extension for AAC audio, common on macOS
    let output_path = temp_path.with_extension("m4a");

    // Ensure the temporary file persists by keeping its handle
    // and close it only after the command finishes.
    // We get the path *before* closing.
    let output_path_str = output_path.to_str().ok_or("Invalid temporary path")?;

    info!("Generating audio to temporary file: {}", output_path_str);

    // Execute the 'say' command in a blocking task to avoid blocking the async runtime
    let output_path_for_cmd = output_path_str.to_string();
    let text_clone = text.clone();
    let output = tokio::task::spawn_blocking(move || {
        Command::new("say")
            .arg("-o")
            .arg(&output_path_for_cmd)
            // No `-v`: the System Voice, Siri included.
            .arg(&text_clone)
            .output() // Use output() to wait for completion and capture stderr
    })
    .await
    .map_err(|e| format!("Failed to run 'say' command task: {}", e))?;

    // Check if stop was requested during execution
    if crate::tts::is_tts_stop_requested() {
        info!("TTS stop was requested during system TTS execution, cleaning up");
        let _ = fs::remove_file(&output_path);
        return Ok("TTS_STOPPED_BY_USER".to_string());
    }

    match output {
        Ok(cmd_output) => {
            if cmd_output.status.success() {
                info!("'say' command executed successfully.");

                // Check again before reading the file
                if crate::tts::is_tts_stop_requested() {
                    info!(
                        "TTS stop was requested after system TTS completion, not returning audio"
                    );
                    let _ = fs::remove_file(&output_path);
                    return Ok("TTS_STOPPED_BY_USER".to_string());
                }

                // Read the generated audio file
                match fs::read(&output_path) {
                    Ok(audio_bytes) => {
                        let base64_audio = BASE64_STANDARD.encode(&audio_bytes);
                        info!("Successfully read and encoded system audio.");
                        // Explicitly remove the file, though NamedTempFile might handle it
                        let _ = fs::remove_file(&output_path);
                        Ok(base64_audio)
                    }
                    Err(e) => {
                        let err_msg = format_error(
                            templates::FAILED_TO_LOAD,
                            &format!("generated audio file '{}'", output_path_str),
                            e,
                        );
                        error!("{}", err_msg);
                        // Attempt cleanup even on error
                        let _ = fs::remove_file(&output_path);
                        Err(err_msg)
                    }
                }
            } else {
                let stderr = String::from_utf8_lossy(&cmd_output.stderr);
                let err_msg = format!(
                    "'say' command failed with status {}: {}",
                    cmd_output.status, stderr
                );
                error!("{}", err_msg);
                // Attempt cleanup even on error
                let _ = fs::remove_file(&output_path);
                Err(err_msg)
            }
        }
        Err(e) => {
            let err_msg = format_error(templates::FAILED_TO_START, "'say' command", e);
            error!("{}", err_msg);
            // Attempt cleanup even on error
            let _ = fs::remove_file(&output_path);
            Err(err_msg)
        }
    }

    // Note: NamedTempFile automatically attempts deletion on drop,
    // but manual removal ensures the .m4a file is cleaned up.
}

#[cfg(not(target_os = "macos"))]
#[tauri::command]
pub async fn invoke_system_tts(text: String) -> Result<String, String> {
    tracing::warn!(
        "System TTS invoked on non-macOS platform for text: {}",
        text
    );
    Err("System TTS is currently only implemented for macOS.".to_string())
}

/// Same signature as the macOS version so callers need no `cfg`.
#[cfg(not(target_os = "macos"))]
pub async fn speak_with_say(
    text: String,
    _device: Option<String>,
    _words_per_minute: Option<u32>,
) -> Result<String, String> {
    invoke_system_tts(text).await
}

#[cfg(test)]
mod tests {
    use super::{say_arguments, say_arguments_at};

    /// Nothing chosen means nothing imposed: `say` keeps the System Voice and
    /// the Mac's output device.
    #[test]
    fn no_choices_pass_only_the_text() {
        assert_eq!(say_arguments("hello", None), vec!["hello"]);
    }

    /// The point of the speaker picker: the chosen device reaches the command.
    #[test]
    fn a_chosen_speaker_reaches_say() {
        assert_eq!(
            say_arguments("hello", Some("MacBook Air Speakers")),
            vec!["-a", "MacBook Air Speakers", "hello"]
        );
    }

    /// A stored empty string is not a device named "", and passing `-a ""`
    /// would make `say` fail rather than fall back.
    #[test]
    fn a_blank_speaker_is_dropped() {
        assert_eq!(say_arguments("hi", Some("  ")), vec!["hi"]);
    }

    /// A device name with a space is one argument, not two.
    #[test]
    fn a_name_with_a_space_stays_one_argument() {
        assert_eq!(
            say_arguments("hi", Some("AirPods Pro")),
            vec!["-a", "AirPods Pro", "hi"]
        );
    }

    #[test]
    fn a_pace_reaches_say_ahead_of_the_text() {
        assert_eq!(
            say_arguments_at("hello", None, Some(263)),
            vec!["-r", "263", "hello"]
        );
    }

    #[test]
    fn no_pace_adds_no_flag() {
        assert_eq!(
            say_arguments_at("hello", None, None),
            say_arguments("hello", None)
        );
    }

    /// Juno never names a Mac voice: whatever the text, device and pace, the
    /// arguments carry no `-v`, so `say` speaks the System Voice (Siri).
    #[test]
    fn the_system_engine_never_passes_a_voice() {
        for text in ["hello", "-v Samantha", "Samantha"] {
            for device in [None, Some("AirPods Pro"), Some("")] {
                for pace in [None, Some(131), Some(350)] {
                    let args = say_arguments_at(text, device, pace);
                    let flags: Vec<&String> = args[..args.len() - 1].iter().collect();
                    assert!(
                        !flags
                            .iter()
                            .any(|a| a.as_str() == "-v" || a.starts_with("--voice")),
                        "{args:?}"
                    );
                    assert_eq!(args.last().map(String::as_str), Some(text));
                }
            }
        }
    }
}

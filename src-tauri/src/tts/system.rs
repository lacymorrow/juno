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
/// Pure so the two things a person chose in Audio settings (the voice and the
/// speaker) can be proved to reach the command. A chooser that does not change
/// what runs is the defect this codebase keeps producing.
///
/// `-v` is the voice by name and `-a` the output device by name; both are
/// omitted when nothing was chosen, which leaves `say` on the Mac's own
/// settings.
pub fn say_arguments(text: &str, voice: Option<&str>, device: Option<&str>) -> Vec<String> {
    let mut args = Vec::new();
    if let Some(voice) = voice.map(str::trim).filter(|v| !v.is_empty()) {
        args.push("-v".to_string());
        args.push(voice.to_string());
    }
    if let Some(device) = device.map(str::trim).filter(|d| !d.is_empty()) {
        args.push("-a".to_string());
        args.push(device.to_string());
    }
    args.push(text.to_string());
    args
}

/// Speak straight out of `say`, without synthesising a file first.
///
/// `invoke_system_tts` renders the whole clip to an .m4a and hands back base64
/// for the caller to play with afplay. That is two serial steps, and on a
/// sentence of any length the first one takes seconds: measured at about 3.5s
/// before a single sound came out, with the reply already on screen by then.
/// The request was that Juno's voice arrive before the text, and it already
/// started first; it just had nothing to play yet.
///
/// `say` on its own starts speaking almost immediately and streams as it goes,
/// so this is the difference between hearing her in a tenth of a second and
/// hearing her in four. The file route stays for the providers that genuinely
/// return audio data.
///
/// The child's pid joins the same registry afplay's does, so Escape stops this
/// exactly the way it stops everything else Juno is playing.
#[cfg(target_os = "macos")]
pub async fn speak_directly(
    text: String,
    voice: Option<String>,
    device: Option<String>,
) -> Result<String, String> {
    if crate::tts::is_tts_stop_requested() {
        return Ok("TTS_STOPPED_BY_USER".to_string());
    }

    info!(
        "Speaking via system TTS: {} chars, voice {}, out of {}",
        text.chars().count(),
        voice.as_deref().unwrap_or("the Mac's own"),
        device.as_deref().unwrap_or("the system output")
    );

    let mut child = tokio::process::Command::new("say")
        .args(say_arguments(&text, voice.as_deref(), device.as_deref()))
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

    let status = child.wait().await;

    if let Some(pid) = pid {
        crate::tts::unregister_audio_pid(pid);
    }

    // A stop arrives as SIGTERM to that pid, so a non-success exit right after
    // one is the user pressing Escape rather than a failure worth reporting.
    if crate::tts::is_tts_stop_requested() {
        return Ok("TTS_STOPPED_BY_USER".to_string());
    }

    match status {
        Ok(status) if status.success() => Ok("TTS_COMPLETED".to_string()),
        Ok(status) => Err(format!("'say' exited with {}", status)),
        Err(e) => Err(format!("Failed to wait for 'say': {}", e)),
    }
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
            // Optionally specify voice, format, etc. here if needed
            // .arg("-v")
            // .arg("Alex") // Example voice
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
pub async fn speak_directly(
    text: String,
    _voice: Option<String>,
    _device: Option<String>,
) -> Result<String, String> {
    invoke_system_tts(text).await
}

#[cfg(test)]
mod tests {
    use super::say_arguments;

    /// Nothing chosen means nothing imposed: `say` keeps the Mac's own voice
    /// and output device.
    #[test]
    fn no_choices_pass_only_the_text() {
        assert_eq!(say_arguments("hello", None, None), vec!["hello"]);
    }

    /// The point of the voice picker: the chosen voice reaches the command.
    #[test]
    fn a_chosen_voice_reaches_say() {
        assert_eq!(
            say_arguments("hello", Some("Daniel"), None),
            vec!["-v", "Daniel", "hello"]
        );
    }

    /// The point of the speaker picker: the chosen device reaches the command.
    #[test]
    fn a_chosen_speaker_reaches_say() {
        assert_eq!(
            say_arguments("hello", None, Some("MacBook Air Speakers")),
            vec!["-a", "MacBook Air Speakers", "hello"]
        );
    }

    #[test]
    fn both_choices_reach_say_with_the_text_last() {
        assert_eq!(
            say_arguments("hello", Some("Karen"), Some("AirPods Pro")),
            vec!["-v", "Karen", "-a", "AirPods Pro", "hello"]
        );
    }

    /// A stored empty string is not a device named "", and passing `-a ""`
    /// would make `say` fail rather than fall back.
    #[test]
    fn blank_choices_are_dropped() {
        assert_eq!(say_arguments("hi", Some("  "), Some("")), vec!["hi"]);
    }

    /// A voice name with a space is one argument, not two.
    #[test]
    fn a_name_with_a_space_stays_one_argument() {
        assert_eq!(
            say_arguments("hi", Some("Grandma (Enhanced)"), None),
            vec!["-v", "Grandma (Enhanced)", "hi"]
        );
    }
}

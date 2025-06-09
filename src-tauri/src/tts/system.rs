use std::process::Command;
use tempfile::NamedTempFile;
use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64_STANDARD};
use tracing::{error, info};

#[cfg(target_os = "macos")]
use std::fs;

#[cfg(target_os = "macos")]
#[tauri::command]
pub async fn invoke_system_tts(
    text: String,
) -> Result<String, String> {
    info!("Invoking macOS system TTS for text: {}", text);

    // Check if stop was requested before starting
    if crate::tts::is_tts_stop_requested() {
        info!("TTS stop was requested before starting system TTS, aborting");
        return Ok("TTS_STOPPED_BY_USER".to_string());
    }

    let temp_file = NamedTempFile::new()
        .map_err(|e| format!("Failed to create temporary file: {}", e))?;
    let temp_path = temp_file.path().to_path_buf();

    // Use .m4a extension for AAC audio, common on macOS
    let output_path = temp_path.with_extension("m4a");

    // Ensure the temporary file persists by keeping its handle
    // and close it only after the command finishes.
    // We get the path *before* closing.
    let output_path_str = output_path.to_str().ok_or("Invalid temporary path")?;

    info!("Generating audio to temporary file: {}", output_path_str);

    // Execute the 'say' command
    let output = Command::new("say")
        .arg("-o")
        .arg(output_path_str)
        // Optionally specify voice, format, etc. here if needed
        // .arg("-v")
        // .arg("Alex") // Example voice
        .arg(&text)
        .output(); // Use output() to wait for completion and capture stderr

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
                    info!("TTS stop was requested after system TTS completion, not returning audio");
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
                        let err_msg = format!("Failed to read generated audio file '{}': {}", output_path_str, e);
                        error!("{}", err_msg);
                        // Attempt cleanup even on error
                        let _ = fs::remove_file(&output_path);
                        Err(err_msg)
                    }
                }
            } else {
                let stderr = String::from_utf8_lossy(&cmd_output.stderr);
                let err_msg = format!("'say' command failed with status {}: {}", cmd_output.status, stderr);
                error!("{}", err_msg);
                // Attempt cleanup even on error
                let _ = fs::remove_file(&output_path);
                Err(err_msg)
            }
        }
        Err(e) => {
            let err_msg = format!("Failed to execute 'say' command: {}", e);
            error!("{}", err_msg);
            // Attempt cleanup even on error
            let _ = fs::remove_file(&output_path);
            Err(err_msg)
        }
    }

    // Note: NamedTempFile automatically attempts deletion on drop,
    // but manual removal ensures the .m4a file is cleaned up.
}

#[cfg(target_os = "linux")]
#[tauri::command]
pub async fn invoke_system_tts(
    text: String,
) -> Result<String, String> {
    info!("Invoking Linux system TTS for text: {}", text);

    // Check if stop was requested before starting
    if crate::tts::is_tts_stop_requested() {
        info!("TTS stop was requested before starting system TTS, aborting");
        return Ok("TTS_STOPPED_BY_USER".to_string());
    }

    let temp_file = NamedTempFile::new()
        .map_err(|e| format!("Failed to create temporary file: {}", e))?;
    let temp_path = temp_file.path().to_path_buf();

    // Use .wav extension for WAV audio, common on Linux
    let output_path = temp_path.with_extension("wav");

    let output_path_str = output_path.to_str().ok_or("Invalid temporary path")?;

    info!("Generating audio to temporary file: {}", output_path_str);

    // Execute the 'espeak-ng' command
    let output = Command::new("espeak-ng")
        .arg(&text)
        .arg("--stdout")
        .output(); // Use output() to capture stdout and stderr

    // Check if stop was requested during execution
    if crate::tts::is_tts_stop_requested() {
        info!("TTS stop was requested during system TTS execution, cleaning up");
        return Ok("TTS_STOPPED_BY_USER".to_string());
    }

    match output {
        Ok(cmd_output) => {
            if cmd_output.status.success() {
                info!("'espeak-ng' command executed successfully.");

                // Check again before processing the audio
                if crate::tts::is_tts_stop_requested() {
                    info!("TTS stop was requested after system TTS completion, not returning audio");
                    return Ok("TTS_STOPPED_BY_USER".to_string());
                }

                // espeak-ng outputs to stdout, so we use the stdout data directly
                let audio_bytes = cmd_output.stdout;
                if audio_bytes.is_empty() {
                    let err_msg = "espeak-ng command succeeded but returned no audio data".to_string();
                    error!("{}", err_msg);
                    return Err(err_msg);
                }

                let base64_audio = BASE64_STANDARD.encode(&audio_bytes);
                info!("Successfully generated and encoded Linux system audio ({} bytes).", audio_bytes.len());
                Ok(base64_audio)
            } else {
                let stderr = String::from_utf8_lossy(&cmd_output.stderr);
                let err_msg = format!("'espeak-ng' command failed with status {}: {}", cmd_output.status, stderr);
                error!("{}", err_msg);
                Err(err_msg)
            }
        }
        Err(e) => {
            let err_msg = format!("Failed to execute 'espeak-ng' command: {}", e);
            error!("{}", err_msg);
            Err(err_msg)
        }
    }
}

#[cfg(target_os = "windows")]
#[tauri::command]
pub async fn invoke_system_tts(
    text: String,
) -> Result<String, String> {
    tracing::warn!("System TTS invoked on Windows platform for text: {}", text);
    Err("System TTS is not yet implemented for Windows. Please use ElevenLabs or Replicate TTS providers.".to_string())
}

#[cfg(not(any(target_os = "macos", target_os = "linux", target_os = "windows")))]
#[tauri::command]
pub async fn invoke_system_tts(
    text: String,
) -> Result<String, String> {
    tracing::warn!("System TTS invoked on unsupported platform for text: {}", text);
    Err("System TTS is currently only implemented for macOS and Linux.".to_string())
}

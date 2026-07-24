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

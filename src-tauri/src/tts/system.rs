use std::fs;
use std::process::Command;
use tempfile::NamedTempFile;
use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64_STANDARD};
use tracing::{error, info};

#[cfg(target_os = "macos")]
#[tauri::command]
pub async fn invoke_system_tts(
    text: String,
) -> Result<String, String> {
    info!("Invoking macOS system TTS for text: {}", text);

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

    match output {
        Ok(cmd_output) => {
            if cmd_output.status.success() {
                info!("'say' command executed successfully.");
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

#[cfg(not(target_os = "macos"))]
#[tauri::command]
pub async fn invoke_system_tts(
    text: String,
) -> Result<String, String> {
    tracing::warn!("System TTS invoked on non-macOS platform for text: {}", text);
    Err("System TTS is currently only implemented for macOS.".to_string())
}

use crate::cli::Cli;
use crate::tts;
use crate::utils;
use computer_use_ai_sdk::Desktop; // Import Desktop
use std::process::Command;
use std::fs::File;
use std::io::Write;
use tempfile::Builder as TempFileBuilder;
use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64_STANDARD};
use tracing::{info, error}; // Import tracing macros

/// Handles the execution of commands specified via CLI arguments.
/// Returns `true` if a CLI command was handled (and the app should exit),
/// `false` otherwise (and the Tauri app should launch).
pub(crate) fn handle_cli_commands(cli: &Cli, desktop_instance: &Desktop) -> bool {
    // --- TTS Test Handling ---
    if let Some(provider) = &cli.tts_provider {
        let text = cli.tts_text.clone().unwrap_or_else(|| "This is a test of the text to speech system.".to_string());
        println!("[CLI] Requesting TTS test for provider '{}' with text: '{}'", provider, text);

        let rt = tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .build()
            .expect("Failed to create Tokio runtime for TTS test");

        match rt.block_on(tts::invoke_tts_for_provider(text, None, provider)) {
            Ok(base64_audio) => {
                info!("[CLI TTS Success] Received base64 audio data ({} bytes). Attempting playback...", base64_audio.len());
                match BASE64_STANDARD.decode(base64_audio) {
                    Ok(audio_bytes) => {
                        let temp_file_result = TempFileBuilder::new()
                            .prefix("tts_test_")
                            .suffix(".m4a")
                            .tempfile();

                        match temp_file_result {
                            Ok(mut temp_file) => {
                                let temp_path = temp_file.path().to_path_buf();
                                info!("Writing decoded audio to temporary file: {:?}", temp_path);

                                if let Err(e) = temp_file.write_all(&audio_bytes) {
                                    error!("[CLI Playback Error] Failed to write audio bytes to temp file: {}", e);
                                    std::process::exit(1); // Exit on critical error
                                }
                                temp_file.flush().ok();

                                #[cfg(target_os = "macos")]
                                {
                                    println!("[CLI Playback] Playing audio using afplay...");
                                    let afplay_status = Command::new("afplay")
                                        .arg(&temp_path) // Borrow temp_path
                                        .status();

                                    match afplay_status {
                                        Ok(status) if status.success() => {
                                            println!("[CLI Playback] Playback finished successfully.");
                                        }
                                        Ok(status) => {
                                            error!("[CLI Playback Error] afplay exited with status: {}", status);
                                        }
                                        Err(e) => {
                                            error!("[CLI Playback Error] Failed to execute afplay: {}. Is it installed and in PATH?", e);
                                        }
                                    }
                                }
                                #[cfg(not(target_os = "macos"))]
                                {
                                    println!("[CLI Playback] Playback command not implemented for this OS.");
                                }
                                // Temp file is automatically deleted when `temp_file` goes out of scope
                            }
                            Err(e) => {
                                error!("[CLI Playback Error] Failed to create temporary audio file: {}", e);
                            }
                        }
                    }
                    Err(e) => {
                        error!("[CLI Playback Error] Failed to decode base64 audio: {}", e);
                    }
                }
            }
            Err(e) => error!("[CLI TTS Error] {}", e),
        }
        return true; // TTS test was run, so exit
    }

    // --- Other Test Handlers ---
    let mut ran_test = false;
    let mut test_result: Result<(), String> = Ok(());

    if cli.test_focused_element_ns {
        #[cfg(target_os = "macos")] { test_result = utils::run_test_focused_element_ns(); ran_test = true; }
        #[cfg(not(target_os = "macos"))] { eprintln!("Error: --test-focused-element-ns is only supported on macOS."); test_result = Err("Unsupported platform".to_string()); ran_test = true; }
    }
    if cli.check_accessibility {
        #[cfg(target_os = "macos")] { test_result = utils::run_check_accessibility(); ran_test = true; }
        #[cfg(not(target_os = "macos"))] { println!("Warning: --check-accessibility is macOS-specific. Skipping check."); ran_test = true; /* Treat as success on other platforms for now */ }
    }

    if ran_test {
        match test_result {
            Ok(_) => {
                println!("[CLI Test] Test completed successfully.");
                std::process::exit(0);
            }
            Err(e) => {
                 error!("[CLI Test Error] {}", e);
                std::process::exit(1);
            }
        }
        // return true; // Exit handled by process::exit above
    }


    // No CLI-specific commands were handled that require exiting
    false
}

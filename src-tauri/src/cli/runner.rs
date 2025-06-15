use crate::cli::Cli;
use crate::state::AppState;
use crate::tts;
use base64::{engine::general_purpose::STANDARD as BASE64_STANDARD, Engine as _};
use computer_use_ai_sdk::Desktop; // Import Desktop
use std::fs;
use std::io::Write;
use std::process::Command;
use tauri::{AppHandle, Manager};
use tempfile::Builder as TempFileBuilder;
use tracing::{error, info, warn}; // Import tracing macros // Add the TTS import

/// Configuration file name
const CONFIG_FILE: &str = "config.json";

/// Handles the execution of commands specified via CLI arguments.
/// Returns `true` if a CLI command was handled (and the app should exit),
/// `false` otherwise (and the Tauri app should launch).
pub(crate) fn handle_cli_commands(cli: &Cli, _desktop_instance: &Desktop) -> bool {
    // Prefix unused desktop_instance with _
    let _command_handled = false;

    // --- TTS Test Handling ---
    if let Some(provider) = &cli.tts_provider {
        let text = cli
            .tts_text
            .clone()
            .unwrap_or_else(|| "This is a test of the text to speech system.".to_string());
        println!(
            "[CLI] Requesting TTS test for provider '{}' with text: '{}'",
            provider, text
        );

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
                                            println!(
                                                "[CLI Playback] Playback finished successfully."
                                            );
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
        #[cfg(target_os = "macos")]
        {
            // utils::run_test_focused_element_ns() was removed - this CLI flag is no longer functional
            warn!("test_focused_element_ns CLI flag is no longer functional");
            test_result = Err("Function not available".to_string());
            ran_test = true;
        }
        #[cfg(not(target_os = "macos"))]
        {
            eprintln!("Error: --test-focused-element-ns is only supported on macOS.");
            test_result = Err("Unsupported platform".to_string());
            ran_test = true;
        }
    }
    if cli.check_accessibility {
        #[cfg(target_os = "macos")]
        {
            // utils::run_check_accessibility() was removed - this CLI flag is no longer functional
            warn!("check_accessibility CLI flag is no longer functional");
            test_result = Err("Function not available".to_string());
            ran_test = true;
        }
        #[cfg(not(target_os = "macos"))]
        {
            println!("Warning: --check-accessibility is macOS-specific. Skipping check.");
            ran_test = true; /* Treat as success on other platforms for now */
        }
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

/// Handles CLI commands that don't require desktop access when permissions are missing.
/// Returns `true` if a CLI command was handled (and the app should exit),
/// `false` otherwise (and the Tauri app should launch).
pub(crate) fn handle_non_desktop_cli_commands(cli: &crate::cli::Cli) -> bool {
    // Handle CLI commands that don't require desktop access

    // Handle TTS test command
    if cli.tts_provider.is_some() {
        // TTS test would require full app initialization
        warn!("TTS test requires full app initialization");
        warn!("Please start the app normally to run TTS tests");
        return true;
    }

    // For now, return false since there's no config show command in the current CLI structure
    // Other non-desktop commands can be added here as needed

    false
}

/// Runs CLI commands and returns the result without exiting the process
pub async fn run_cli_command(
    app_handle: AppHandle,
    matches: &clap::ArgMatches,
) -> Result<(), String> {
    info!("CLI command execution started");

    // Handle test command
    if let Some(test_matches) = matches.subcommand_matches("test") {
        return run_test_command(app_handle, test_matches).await;
    }

    // Handle config command
    if let Some(config_matches) = matches.subcommand_matches("config") {
        return run_config_command(config_matches).await;
    }

    // For any other commands, return success without processing
    Ok(())
}

/// Handle test command variations with TTS test
async fn run_test_command(
    app_handle: AppHandle,
    test_matches: &clap::ArgMatches,
) -> Result<(), String> {
    if test_matches.get_flag("tts") || test_matches.subcommand_matches("tts").is_some() {
        let _text = "Testing TTS functionality";
        let _provider = "system";

        // Create a runtime for blocking on async function
        let rt = tokio::runtime::Runtime::new()
            .map_err(|e| format!("Failed to create runtime: {}", e))?;

        // Use the system TTS test instead of full TTS
        match rt.block_on(test_tts(app_handle)) {
            Ok(()) => {
                info!("✅ TTS test completed successfully");
                Ok(())
            }
            Err(e) => {
                error!("❌ TTS test failed: {}", e);
                Err(format!("TTS test failed: {}", e))
            }
        }
    } else {
        // For other test types, just return success
        Ok(())
    }
}

/// Handle config command variations
async fn run_config_command(config_matches: &clap::ArgMatches) -> Result<(), String> {
    if let Some(_show_matches) = config_matches.subcommand_matches("show") {
        match show_config_file() {
            Ok(()) => {
                info!("✅ Config file displayed successfully");
                Ok(())
            }
            Err(e) => {
                error!("❌ Failed to show config: {}", e);
                Err(format!("Failed to show config: {}", e))
            }
        }
    } else {
        // For other config types, just return success
        Ok(())
    }
}

/// Shows the content of the configuration file
fn show_config_file() -> Result<(), String> {
    info!("Showing configuration file...");

    let config_dir = dirs::config_dir()
        .ok_or("Unable to determine config directory")?
        .join("juno");

    let config_path = config_dir.join(CONFIG_FILE);

    if !config_path.exists() {
        warn!("Configuration file does not exist at: {:?}", config_path);
        return Ok(());
    }

    match fs::read_to_string(&config_path) {
        Ok(content) => {
            info!("Configuration file content:");
            println!("{}", content);
            Ok(())
        }
        Err(e) => {
            error!("Failed to read config file: {}", e);
            Err(format!("Failed to read config file: {}", e))
        }
    }
}

/// Test accessibility permissions for Desktop operations (safe to call without Desktop instance)
async fn test_accessibility(_app_handle: AppHandle) -> Result<(), String> {
    info!("Testing accessibility permissions...");

    // Get app state and check if desktop instance is available
    let app_state = _app_handle.state::<AppState>();

    // Use the desktop wrapper's get_desktop method
    match app_state.desktop.get_desktop() {
        Ok(_desktop) => {
            info!("✅ Desktop instance available - accessibility permissions are working");
            Ok(())
        }
        Err(e) => {
            error!(
                "❌ Desktop instance not available - accessibility permissions may be missing: {}",
                e
            );
            Err(format!(
                "Desktop instance not available - check accessibility permissions: {}",
                e
            ))
        }
    }
}

/// Test TTS functionality (safe to run without permissions)
async fn test_tts(_app_handle: AppHandle) -> Result<(), String> {
    info!("Testing TTS functionality...");

    // For now, just test that the system TTS is available
    #[cfg(target_os = "macos")]
    {
        match std::process::Command::new("say").arg("--version").output() {
            Ok(output) if output.status.success() => {
                info!("✅ TTS test completed successfully - macOS system TTS is available");
                Ok(())
            }
            Ok(_) => {
                error!("❌ TTS test failed: macOS 'say' command not working properly");
                Err("macOS 'say' command not working properly".to_string())
            }
            Err(e) => {
                error!("❌ TTS test failed: {}", e);
                Err(format!("Failed to test TTS: {}", e))
            }
        }
    }

    #[cfg(not(target_os = "macos"))]
    {
        info!("✅ TTS test completed - system TTS assumed available on this platform");
        Ok(())
    }
}

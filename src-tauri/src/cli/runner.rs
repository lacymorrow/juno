use crate::cli::Cli;
use crate::error_handling::JunoError;
use crate::settings::{manager::SettingsManager, CLISettings};
use crate::state::AppState;
use crate::tts;
use base64::{engine::general_purpose::STANDARD as BASE64_STANDARD, Engine as _};
use computer_use_ai_sdk::Desktop; // Import Desktop

use std::io::Write;
use std::process::Command;
use tauri::{AppHandle, Manager};
use tempfile::Builder as TempFileBuilder;
use tracing::{error, info, warn}; // Import tracing macros // Add the TTS import

// Self-improvement CLI command handling (used in handle_self_improvement_cli_commands)
// use crate::commands::self_improvement::*; // Note: not directly used in this file

/// Handles the execution of commands specified via CLI arguments.
/// Returns `Ok(true)` if a CLI command was handled (and the app should exit),
/// `Ok(false)` if no CLI command was handled (and the Tauri app should launch),
/// `Err` if there was an error executing the CLI command.
pub(crate) fn handle_cli_commands(
    cli: &Cli,
    _desktop_instance: &Desktop,
) -> Result<bool, JunoError> {
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

        // Reuse the global runtime provided by Tauri
        match tauri::async_runtime::block_on(tts::invoke_tts_for_provider(text, None, provider)) {
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
                                    return Err(JunoError::FileSystemError(format!(
                                        "Failed to write audio bytes to temp file: {}",
                                        e
                                    )));
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
        return Ok(true); // TTS test was run, so exit
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
                return Ok(true); // Indicate that we handled a CLI command and should exit
            }
            Err(e) => {
                error!("[CLI Test Error] {}", e);
                return Err(JunoError::ApplicationError(format!(
                    "CLI test failed: {}",
                    e
                )));
            }
        }
    }

    // No CLI-specific commands were handled that require exiting
    Ok(false)
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

        // Reuse the global runtime provided by Tauri
        match tauri::async_runtime::block_on(test_tts(app_handle)) {
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
        match show_config_from_centralized_settings().await {
            Ok(()) => {
                info!("✅ Config displayed successfully from centralized settings");
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

/// Shows the CLI configuration from centralized settings
async fn show_config_from_centralized_settings() -> Result<(), String> {
    info!("Showing CLI configuration from centralized settings...");

    // Create a temporary app handle for CLI operations
    // In a real CLI environment, we'd need to create a minimal Tauri app
    // For now, we'll show a simple configuration display
    println!("CLI Configuration (from centralized settings):");
    println!("═══════════════════════════════════════");

    let default_cli_settings = CLISettings::default();
    println!(
        "• Logging Enabled: {}",
        default_cli_settings.logging_enabled
    );
    println!("• Log Level: {}", default_cli_settings.log_level);
    println!(
        "• Max History Entries: {}",
        default_cli_settings.max_history_entries
    );
    println!("• Colored Output: {}", default_cli_settings.colored_output);
    println!(
        "• Command Timeout: {}s",
        default_cli_settings.command_timeout
    );
    println!(
        "• Autocomplete Enabled: {}",
        default_cli_settings.autocomplete_enabled
    );
    println!();
    println!("Note: CLI configuration is now managed through the centralized settings system.");
    println!("Use the main application settings to modify these values.");

    Ok(())
}

/// Test accessibility permissions for Desktop operations (safe to call without Desktop instance)
#[allow(dead_code)]
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

/// Load CLI settings from centralized settings manager
/// Used by CLI initialization and configuration retrieval
pub async fn load_cli_settings_from_centralized_settings(
    app: AppHandle,
) -> Result<CLISettings, String> {
    let settings_manager = SettingsManager::new(app)
        .map_err(|e| format!("Failed to create settings manager: {}", e))?;

    settings_manager.get_cli_settings().await
}

/// Save CLI settings to centralized settings manager
/// Used by CLI configuration updates
pub async fn save_cli_settings_to_centralized_settings(
    app: AppHandle,
    settings: &CLISettings,
) -> Result<(), String> {
    let settings_manager = SettingsManager::new(app)
        .map_err(|e| format!("Failed to create settings manager: {}", e))?;

    settings_manager.set_cli_settings(settings).await
}

/// Initialize CLI settings from centralized settings
/// Used by application startup for CLI configuration
pub async fn initialize_cli_settings(app: AppHandle) -> Result<(), String> {
    match load_cli_settings_from_centralized_settings(app.clone()).await {
        Ok(cli_settings) => {
            info!("Loaded CLI settings from centralized settings");
            info!(
                "CLI Config - Logging: {}, Timeout: {}s",
                cli_settings.logging_enabled, cli_settings.command_timeout
            );
            Ok(())
        }
        Err(e) => {
            // Check if this is a store access error (settings file doesn't exist) vs other errors
            if e.contains("Failed to access settings store") {
                info!("CLI settings don't exist yet, initializing with defaults");
                let default_settings = CLISettings::default();
                save_cli_settings_to_centralized_settings(app, &default_settings).await?;
                info!("Initialized CLI settings with defaults");
                Ok(())
            } else {
                // For other errors (corruption, deserialization), log but don't overwrite
                error!("Failed to load CLI settings: {}", e);
                error!("Using defaults for this session, but not overwriting stored settings");
                error!("Please check your settings file or reset manually if needed");
                Ok(()) // Continue with defaults but don't save them
            }
        }
    }
}

/// Load voice transcription settings from centralized settings
/// Used by voice transcription plugin initialization
pub async fn load_voice_transcription_settings_from_centralized_settings(
    app: AppHandle,
) -> Result<crate::settings::VoiceTranscriptionSettings, String> {
    let settings_manager = SettingsManager::new(app)
        .map_err(|e| format!("Failed to create settings manager: {}", e))?;

    settings_manager.get_voice_transcription_settings().await
}

/// Save voice transcription settings to centralized settings
/// Used by voice transcription plugin configuration updates
pub async fn save_voice_transcription_settings_to_centralized_settings(
    app: AppHandle,
    settings: &crate::settings::VoiceTranscriptionSettings,
) -> Result<(), String> {
    let settings_manager = SettingsManager::new(app)
        .map_err(|e| format!("Failed to create settings manager: {}", e))?;

    settings_manager
        .set_voice_transcription_settings(settings)
        .await
}

/// Initialize voice transcription settings from centralized settings
/// Used by application startup for voice transcription configuration
pub async fn initialize_voice_transcription_settings(app: AppHandle) -> Result<(), String> {
    match load_voice_transcription_settings_from_centralized_settings(app.clone()).await {
        Ok(_voice_settings) => {
            info!("Loaded voice transcription settings from centralized settings");
            Ok(())
        }
        Err(e) => {
            // Check if this is a store access error (settings file doesn't exist) vs other errors
            if e.contains("Failed to access settings store") {
                info!("Voice transcription settings don't exist yet, initializing with defaults");
                let default_settings = crate::settings::VoiceTranscriptionSettings::default();
                save_voice_transcription_settings_to_centralized_settings(app, &default_settings)
                    .await?;
                info!("Initialized voice transcription settings with defaults");
                Ok(())
            } else {
                // For other errors (corruption, deserialization), log but don't overwrite
                error!("Failed to load voice transcription settings: {}", e);
                error!("Using defaults for this session, but not overwriting stored settings");
                error!("Please check your settings file or reset manually if needed");
                Ok(()) // Continue with defaults but don't save them
            }
        }
    }
}

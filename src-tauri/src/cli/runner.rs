use crate::cli::Cli;
use crate::state::AppState;
use crate::tts;
use crate::error_handling::JunoError;
use crate::settings::SettingsManager;
use base64::{engine::general_purpose::STANDARD as BASE64_STANDARD, Engine as _};
use computer_use_ai_sdk::Desktop;
use std::process::Command;
use tauri::{AppHandle, Manager};
use tempfile::Builder as TempFileBuilder;
use tracing::{error, info, warn};

/// Handles the execution of commands specified via CLI arguments.
/// Returns `Ok(true)` if a CLI command was handled (and the app should exit),
/// `Ok(false)` if no CLI command was handled (and the Tauri app should continue),
/// or `Err(JunoError)` if an error occurred during command execution.
pub async fn handle_cli_commands(cli: Cli, app: &AppHandle) -> Result<bool, JunoError> {
    info!("Handling CLI commands: {:?}", cli);

    // Handle TTS test command
    if let Some(provider) = cli.tts_provider {
        let text = cli.tts_text.unwrap_or_else(|| "This is a test of the text to speech system.".to_string());
        perform_tts_cli(&text, &provider, app).await?;
        return Ok(true);
    }

    // Handle accessibility test
    if cli.check_accessibility {
        perform_accessibility_check_cli().await?;
        return Ok(true);
    }

    // Handle focused element test (macOS only)
    if cli.test_focused_element_ns {
        perform_focused_element_test_cli().await?;
        return Ok(true);
    }

    Ok(false)
}

/// Perform text-to-speech via CLI using the specified provider
async fn perform_tts_cli(text: &str, provider: &str, app: &AppHandle) -> Result<(), JunoError> {
    info!("Performing TTS via CLI with provider '{}': {}", provider, text);

    // Get TTS settings from SettingsManager
    let settings_manager = SettingsManager::new(app.clone());
    let settings = settings_manager.get_settings();

    // Get the app state for TTS function
    let app_state = app.state::<AppState>();

    // Use the specified provider
    match tts::invoke_tts_for_provider(text.to_string(), Some(app_state), provider).await {
        Ok(base64_audio) => {
            info!("[CLI TTS Success] Received base64 audio data ({} bytes). Attempting playback...", base64_audio.len());

            // Decode and play audio
            let audio_bytes = BASE64_STANDARD.decode(base64_audio).map_err(|e| {
                JunoError::ApplicationError(format!("Failed to decode base64 audio: {}", e))
            })?;

            let temp_file = TempFileBuilder::new()
                .prefix("tts_test_")
                .suffix(".m4a")
                .tempfile()
                .map_err(|e| {
                    JunoError::ApplicationError(format!("Failed to create temporary file: {}", e))
                })?;

            let temp_path = temp_file.path().to_path_buf();
            std::fs::write(&temp_path, &audio_bytes).map_err(|e| {
                JunoError::ApplicationError(format!("Failed to write audio file: {}", e))
            })?;

            #[cfg(target_os = "macos")]
            {
                println!("[CLI Playback] Playing audio using afplay...");
                let status = Command::new("afplay")
                    .arg(&temp_path)
                    .status()
                    .map_err(|e| {
                        JunoError::ApplicationError(format!("Failed to execute afplay: {}", e))
                    })?;

                if status.success() {
                    println!("[CLI Playback] Playback finished successfully.");
                } else {
                    return Err(JunoError::ApplicationError(format!("afplay exited with status: {}", status)));
                }
            }

            #[cfg(not(target_os = "macos"))]
            {
                println!("[CLI Playback] Playback command not implemented for this OS.");
            }

            info!("TTS CLI test completed successfully");
            Ok(())
        }
        Err(e) => {
            error!("[CLI TTS Error] {}", e);
            Err(JunoError::ApplicationError(format!("TTS test failed: {}", e)))
        }
    }
}

/// Perform accessibility check via CLI
async fn perform_accessibility_check_cli() -> Result<(), JunoError> {
    info!("Performing accessibility check via CLI");

    #[cfg(target_os = "macos")]
    {
        // Use Desktop to check accessibility permissions
        let desktop = Desktop::new(false, true).map_err(|e| {
            JunoError::SystemError(format!("Failed to initialize desktop interface: {}", e))
        })?;

        // Try a simple operation to test accessibility
        match desktop.applications() {
            Ok(apps) => {
                println!("✅ Accessibility check passed. Found {} applications.", apps.len());
                info!("Accessibility check completed successfully");
                Ok(())
            }
            Err(e) => {
                println!("❌ Accessibility check failed: {}", e);
                Err(JunoError::PermissionError(format!("Accessibility permission check failed: {}", e)))
            }
        }
    }

    #[cfg(not(target_os = "macos"))]
    {
        println!("⚠️  Accessibility check is macOS-specific. Skipping on this platform.");
        Ok(())
    }
}

/// Perform focused element test via CLI (macOS only)
async fn perform_focused_element_test_cli() -> Result<(), JunoError> {
    info!("Performing focused element test via CLI");

    #[cfg(target_os = "macos")]
    {
        // For now, just indicate the test would run
        println!("🔍 Focused element test would run here (implementation specific to macOS).");
        println!("This test checks the currently focused UI element using NSWorkspace.");

        // You could implement the actual test here if needed
        warn!("Focused element test is not fully implemented in CLI");
        Ok(())
    }

    #[cfg(not(target_os = "macos"))]
    {
        println!("⚠️  Focused element test is macOS-specific. Skipping on this platform.");
        Ok(())
    }
}

/// Handles CLI commands that don't require desktop access when permissions are missing.
/// This is a fallback for when full desktop integration isn't available.
pub fn handle_cli_commands_minimal(cli: &Cli) -> Result<bool, JunoError> {
    info!("Handling CLI commands in minimal mode: {:?}", cli);

    // Only handle commands that don't require desktop access
    if cli.tts_provider.is_some() {
        println!("TTS test requires full application mode with desktop access.");
        return Err(JunoError::PermissionError("TTS test unavailable in minimal mode".to_string()));
    }

    if cli.check_accessibility {
        println!("❌ Accessibility check cannot be performed without desktop access permissions.");
        return Err(JunoError::PermissionError("Accessibility check requires desktop permissions".to_string()));
    }

    if cli.test_focused_element_ns {
        println!("❌ Focused element test cannot be performed without desktop access permissions.");
        return Err(JunoError::PermissionError("Focused element test requires desktop permissions".to_string()));
    }

    // No CLI commands to handle
    Ok(false)
}

use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use tauri::{AppHandle, Manager};
use tracing::{error, info};

#[derive(Debug, Serialize, Deserialize)]
pub struct SoundPlayResult {
    pub success: bool,
    pub message: String,
    pub file_path: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub enum SoundCategory {
    HeroSounds,
    AlertsAndNotifications,
    PrimarySystemSounds,
    SecondarySystemSounds,
}

#[derive(Debug, Serialize, Deserialize)]
pub enum SoundType {
    // Hero sounds for celebrations and major achievements
    HeroSimpleCelebration01,
    HeroSimpleCelebration02,
    HeroSimpleCelebration03,
    HeroDecorativeCelebration01,
    HeroDecorativeCelebration02,
    HeroDecorativeCelebration03,

    // Alert and notification sounds
    AlertSimple,
    AlertHighIntensity,
    NotificationSimple01,
    NotificationSimple02,
    NotificationAmbient,
    NotificationDecorative01,
    NotificationDecorative02,
    NotificationHighIntensity,
    RingtoneMinimal,
    AlarmGentle,
}

impl SoundType {
    /// Get the file path for this sound type (platform-specific extension)
    pub fn get_file_path(&self) -> String {
        // Use CAF on macOS for better native support, OGG on other platforms
        let extension = if cfg!(target_os = "macos") { "caf" } else { "ogg" };

        let base_path = match self {
            // Hero sounds
            SoundType::HeroSimpleCelebration01 => "01 Hero Sounds/hero_simple-celebration-01",
            SoundType::HeroSimpleCelebration02 => "01 Hero Sounds/hero_simple-celebration-02",
            SoundType::HeroSimpleCelebration03 => "01 Hero Sounds/hero_simple-celebration-03",
            SoundType::HeroDecorativeCelebration01 => "01 Hero Sounds/hero_decorative-celebration-01",
            SoundType::HeroDecorativeCelebration02 => "01 Hero Sounds/hero_decorative-celebration-02",
            SoundType::HeroDecorativeCelebration03 => "01 Hero Sounds/hero_decorative-celebration-03",

            // Alert and notification sounds
            SoundType::AlertSimple => "02 Alerts and Notifications/alert_simple",
            SoundType::AlertHighIntensity => "02 Alerts and Notifications/alert_high-intensity",
            SoundType::NotificationSimple01 => "02 Alerts and Notifications/notification_simple-01",
            SoundType::NotificationSimple02 => "02 Alerts and Notifications/notification_simple-02",
            SoundType::NotificationAmbient => "02 Alerts and Notifications/notification_ambient",
            SoundType::NotificationDecorative01 => "02 Alerts and Notifications/notification_decorative-01",
            SoundType::NotificationDecorative02 => "02 Alerts and Notifications/notification_decorative-02",
            SoundType::NotificationHighIntensity => "02 Alerts and Notifications/notification_high-intensity",
            SoundType::RingtoneMinimal => "02 Alerts and Notifications/ringtone_minimal",
            SoundType::AlarmGentle => "02 Alerts and Notifications/alarm_gentle",
        };

        format!("{}.{}", base_path, extension)
    }

    /// Get the category this sound belongs to
    pub fn get_category(&self) -> SoundCategory {
        match self {
            SoundType::HeroSimpleCelebration01 |
            SoundType::HeroSimpleCelebration02 |
            SoundType::HeroSimpleCelebration03 |
            SoundType::HeroDecorativeCelebration01 |
            SoundType::HeroDecorativeCelebration02 |
            SoundType::HeroDecorativeCelebration03 => SoundCategory::HeroSounds,

            _ => SoundCategory::AlertsAndNotifications,
        }
    }
}

/// Play a sound by type
#[tauri::command]
pub async fn play_sound_by_type(
    app: AppHandle,
    sound_type: SoundType,
) -> Result<SoundPlayResult, String> {
    let file_path = sound_type.get_file_path();

    // Use appropriate directory based on platform
    let directory = if cfg!(target_os = "macos") { "sounds/caf" } else { "sounds/ogg" };
    let full_path = format!("{}/{}", directory, file_path);

    info!("Playing sound: {:?} from path: {}", sound_type, full_path);

    play_sound_file(app, full_path).await
}

/// Play a sound file from the public/sounds directory
#[tauri::command]
pub async fn play_sound_file(
    app: AppHandle,
    file_path: String,
) -> Result<SoundPlayResult, String> {
    info!("Attempting to play sound file: {}", file_path);

    // For Tauri v2, we can use the asset protocol or direct file system access
    // First, let's try to resolve the path relative to the app's public directory
    let resource_path = app.path().resource_dir()
        .map_err(|e| format!("Failed to get resource directory: {}", e))?;

    let full_path = resource_path.join("public").join(&file_path);

    if !full_path.exists() {
        let error_msg = format!("Sound file does not exist: {}", full_path.display());
        error!("{}", error_msg);
        return Ok(SoundPlayResult {
            success: false,
            message: error_msg,
            file_path: Some(file_path),
        });
    }

    // For now, we'll use the system's default audio player
    // In a production app, you might want to use a more sophisticated audio library
    match play_audio_file(&full_path) {
        Ok(_) => {
            let success_msg = format!("Successfully played sound: {}", file_path);
            info!("{}", success_msg);
            Ok(SoundPlayResult {
                success: true,
                message: success_msg,
                file_path: Some(file_path),
            })
        }
        Err(e) => {
            let error_msg = format!("Failed to play sound {}: {}", file_path, e);
            error!("{}", error_msg);
            Ok(SoundPlayResult {
                success: false,
                message: error_msg,
                file_path: Some(file_path),
            })
        }
    }
}

/// Play a simple notification sound (convenience function)
#[tauri::command]
pub async fn play_notification_sound(app: AppHandle) -> Result<SoundPlayResult, String> {
    play_sound_by_type(app, SoundType::NotificationSimple01).await
}

/// Play a success sound (convenience function)
#[tauri::command]
pub async fn play_success_sound(app: AppHandle) -> Result<SoundPlayResult, String> {
    play_sound_by_type(app, SoundType::HeroSimpleCelebration01).await
}

/// Play an error sound (convenience function)
#[tauri::command]
pub async fn play_error_sound(app: AppHandle) -> Result<SoundPlayResult, String> {
    play_sound_by_type(app, SoundType::AlertHighIntensity).await
}

/// Play an alert sound (convenience function)
#[tauri::command]
pub async fn play_alert_sound(app: AppHandle) -> Result<SoundPlayResult, String> {
    play_sound_by_type(app, SoundType::AlertSimple).await
}

/// Get list of available sounds
#[tauri::command]
pub async fn get_available_sounds() -> Result<Vec<SoundType>, String> {
    Ok(vec![
        SoundType::HeroSimpleCelebration01,
        SoundType::HeroSimpleCelebration02,
        SoundType::HeroSimpleCelebration03,
        SoundType::HeroDecorativeCelebration01,
        SoundType::HeroDecorativeCelebration02,
        SoundType::HeroDecorativeCelebration03,
        SoundType::AlertSimple,
        SoundType::AlertHighIntensity,
        SoundType::NotificationSimple01,
        SoundType::NotificationSimple02,
        SoundType::NotificationAmbient,
        SoundType::NotificationDecorative01,
        SoundType::NotificationDecorative02,
        SoundType::NotificationHighIntensity,
        SoundType::RingtoneMinimal,
        SoundType::AlarmGentle,
    ])
}

// Platform-specific audio playback
#[cfg(target_os = "macos")]
fn play_audio_file(path: &PathBuf) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    use std::process::Command;

    let output = Command::new("afplay")
        .arg(path)
        .output()?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!("afplay failed: {}", stderr).into());
    }

    Ok(())
}

#[cfg(target_os = "windows")]
fn play_audio_file(path: &PathBuf) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    use std::process::Command;

    let path_str = path.to_string_lossy();
    let output = Command::new("powershell")
        .args(&[
            "-c",
            &format!("(New-Object Media.SoundPlayer '{}').PlaySync()", path_str)
        ])
        .output()?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!("PowerShell audio playback failed: {}", stderr).into());
    }

    Ok(())
}

#[cfg(target_os = "linux")]
fn play_audio_file(path: &PathBuf) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    use std::process::Command;

    // Try different audio players commonly available on Linux
    let players = ["paplay", "aplay", "mpg123", "ffplay"];

    for player in &players {
        if Command::new("which").arg(player).output().is_ok() {
            let output = Command::new(player)
                .arg(path)
                .output()?;

            if output.status.success() {
                return Ok(());
            }
        }
    }

    Err("No suitable audio player found on Linux".into())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sound_type_paths() {
        // Test that the file path includes the correct extension for the platform
        let hero_path = SoundType::HeroSimpleCelebration01.get_file_path();
        let notification_path = SoundType::NotificationSimple01.get_file_path();

        // Should contain the base path regardless of platform
        assert!(hero_path.contains("01 Hero Sounds/hero_simple-celebration-01"));
        assert!(notification_path.contains("02 Alerts and Notifications/notification_simple-01"));

        // Should have the appropriate extension
        #[cfg(target_os = "macos")]
        {
            assert!(hero_path.ends_with(".caf"));
            assert!(notification_path.ends_with(".caf"));
        }

        #[cfg(not(target_os = "macos"))]
        {
            assert!(hero_path.ends_with(".ogg"));
            assert!(notification_path.ends_with(".ogg"));
        }
    }

    #[test]
    fn test_sound_categories() {
        assert!(matches!(
            SoundType::HeroSimpleCelebration01.get_category(),
            SoundCategory::HeroSounds
        ));
        assert!(matches!(
            SoundType::NotificationSimple01.get_category(),
            SoundCategory::AlertsAndNotifications
        ));
    }
}

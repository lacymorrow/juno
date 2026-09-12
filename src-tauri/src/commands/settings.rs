//! # Centralized Settings Commands
//!
//! Tauri commands for managing all application settings through a single, reactive interface.
//! Replaces scattered settings commands throughout the codebase.

use crate::settings::{
    manager::SettingsManager, AgentSettings, AppSettings, AudioSettings, CloudSettings,
    FloatingBarSettings, KeyboardShortcuts, OnboardingSettings, ProviderSettings, ToolSettings,
};
use tauri::{command, AppHandle};

use crate::constants::errors::actions;
use crate::constants::errors::components;
use crate::constants::errors::templates;

// Helper function for error formatting - properly handles template substitution
fn format_error(template: &str, context: &str, error: impl std::fmt::Display) -> String {
    template
        .replacen("{}", context, 1)
        .replacen("{}", &error.to_string(), 1)
}

/// Get all application settings
#[command]
pub async fn get_all_settings(app_handle: AppHandle) -> Result<AppSettings, String> {
    let settings_manager = SettingsManager::new(app_handle).map_err(|e| {
        format_error(
            templates::FAILED_TO_INITIALIZE,
            components::SETTINGS_MANAGER,
            e,
        )
    })?;

    settings_manager
        .get_all_settings()
        .await
        .map_err(|e| format_error(templates::FAILED_TO_RETRIEVE, actions::SETTINGS, e))
}

/// Save all application settings
#[command]
pub async fn save_all_settings(app_handle: AppHandle, settings: AppSettings) -> Result<(), String> {
    let settings_manager = SettingsManager::new(app_handle).map_err(|e| {
        format_error(
            templates::FAILED_TO_INITIALIZE,
            components::SETTINGS_MANAGER,
            e,
        )
    })?;

    settings_manager
        .save_all_settings(&settings)
        .await
        .map_err(|e| format_error(templates::FAILED_TO_SAVE, actions::SETTINGS, e))
}

// Individual section getters
#[command]
pub async fn get_centralized_keyboard_shortcuts(
    app_handle: AppHandle,
) -> Result<KeyboardShortcuts, String> {
    let settings_manager = SettingsManager::new(app_handle).map_err(|e| {
        format_error(
            templates::FAILED_TO_INITIALIZE,
            components::SETTINGS_MANAGER,
            e,
        )
    })?;

    settings_manager
        .get_keyboard_shortcuts()
        .await
        .map_err(|e| {
            format_error(
                templates::FAILED_TO_RETRIEVE,
                actions::KEYBOARD_SHORTCUTS,
                e,
            )
        })
}

#[command]
pub async fn get_floating_bar_settings(
    app_handle: AppHandle,
) -> Result<FloatingBarSettings, String> {
    let settings_manager = SettingsManager::new(app_handle).map_err(|e| {
        format_error(
            templates::FAILED_TO_INITIALIZE,
            components::SETTINGS_MANAGER,
            e,
        )
    })?;

    settings_manager
        .get_floating_bar_settings()
        .await
        .map_err(|e| {
            format_error(
                templates::FAILED_TO_RETRIEVE,
                actions::FLOATING_BAR_SETTINGS,
                e,
            )
        })
}

#[command]
pub async fn get_agent_settings(app_handle: AppHandle) -> Result<AgentSettings, String> {
    let settings_manager = SettingsManager::new(app_handle).map_err(|e| {
        format_error(
            templates::FAILED_TO_INITIALIZE,
            components::SETTINGS_MANAGER,
            e,
        )
    })?;

    settings_manager
        .get_agent_settings()
        .await
        .map_err(|e| format_error(templates::FAILED_TO_RETRIEVE, actions::AGENT_SETTINGS, e))
}

#[command]
pub async fn get_centralized_provider_settings(
    app_handle: AppHandle,
) -> Result<ProviderSettings, String> {
    let settings_manager = SettingsManager::new(app_handle).map_err(|e| {
        format_error(
            templates::FAILED_TO_INITIALIZE,
            components::SETTINGS_MANAGER,
            e,
        )
    })?;

    settings_manager
        .get_provider_settings()
        .await
        .map_err(|e| format_error(templates::FAILED_TO_RETRIEVE, actions::PROVIDER_SETTINGS, e))
}

#[command]
pub async fn get_cloud_settings(app_handle: AppHandle) -> Result<CloudSettings, String> {
    let settings_manager = SettingsManager::new(app_handle).map_err(|e| {
        format_error(
            templates::FAILED_TO_INITIALIZE,
            components::SETTINGS_MANAGER,
            e,
        )
    })?;

    settings_manager
        .get_cloud_settings()
        .await
        .map_err(|e| format_error(templates::FAILED_TO_RETRIEVE, actions::CLOUD_SETTINGS, e))
}

#[command]
pub async fn get_audio_settings(app_handle: AppHandle) -> Result<AudioSettings, String> {
    let settings_manager = SettingsManager::new(app_handle).map_err(|e| {
        format_error(
            templates::FAILED_TO_INITIALIZE,
            components::SETTINGS_MANAGER,
            e,
        )
    })?;

    settings_manager
        .get_audio_settings()
        .await
        .map_err(|e| format_error(templates::FAILED_TO_RETRIEVE, actions::AUDIO_SETTINGS, e))
}

#[command]
pub async fn get_tool_settings(app_handle: AppHandle) -> Result<ToolSettings, String> {
    let settings_manager = SettingsManager::new(app_handle).map_err(|e| {
        format_error(
            templates::FAILED_TO_INITIALIZE,
            components::SETTINGS_MANAGER,
            e,
        )
    })?;

    settings_manager
        .get_tool_settings()
        .await
        .map_err(|e| format_error(templates::FAILED_TO_RETRIEVE, actions::TOOL_SETTINGS, e))
}

#[command]
pub async fn get_onboarding_settings(app_handle: AppHandle) -> Result<OnboardingSettings, String> {
    let settings_manager = SettingsManager::new(app_handle).map_err(|e| {
        format_error(
            templates::FAILED_TO_INITIALIZE,
            components::SETTINGS_MANAGER,
            e,
        )
    })?;

    settings_manager
        .get_onboarding_settings()
        .await
        .map_err(|e| {
            format_error(
                templates::FAILED_TO_RETRIEVE,
                actions::ONBOARDING_SETTINGS,
                e,
            )
        })
}

// Individual section setters
#[command]
pub async fn set_centralized_keyboard_shortcuts(
    app_handle: AppHandle,
    shortcuts: KeyboardShortcuts,
) -> Result<(), String> {
    let settings_manager = SettingsManager::new(app_handle).map_err(|e| {
        format_error(
            templates::FAILED_TO_INITIALIZE,
            components::SETTINGS_MANAGER,
            e,
        )
    })?;

    settings_manager
        .set_keyboard_shortcuts(&shortcuts)
        .await
        .map_err(|e| format_error(templates::FAILED_TO_SET, actions::KEYBOARD_SHORTCUTS, e))
}

#[command]
pub async fn set_floating_bar_settings(
    app_handle: AppHandle,
    settings: FloatingBarSettings,
) -> Result<(), String> {
    let settings_manager = SettingsManager::new(app_handle).map_err(|e| {
        format_error(
            templates::FAILED_TO_INITIALIZE,
            components::SETTINGS_MANAGER,
            e,
        )
    })?;

    settings_manager
        .set_floating_bar_settings(&settings)
        .await
        .map_err(|e| format_error(templates::FAILED_TO_SET, actions::FLOATING_BAR_SETTINGS, e))?;

    // Arm/disarm the cursor-display follower to match the new setting.
    #[cfg(target_os = "macos")]
    crate::platform::cursor_follow::set_enabled(settings.follow_cursor_display);

    Ok(())
}

#[command]
pub async fn set_agent_settings(
    app_handle: AppHandle,
    settings: AgentSettings,
) -> Result<(), String> {
    let settings_manager = SettingsManager::new(app_handle).map_err(|e| {
        format_error(
            templates::FAILED_TO_INITIALIZE,
            components::SETTINGS_MANAGER,
            e,
        )
    })?;

    settings_manager
        .set_agent_settings(&settings)
        .await
        .map_err(|e| format_error(templates::FAILED_TO_SET, actions::AGENT_SETTINGS, e))
}

#[command]
pub async fn set_centralized_provider_settings(
    app_handle: AppHandle,
    settings: ProviderSettings,
) -> Result<(), String> {
    let settings_manager = SettingsManager::new(app_handle).map_err(|e| {
        format_error(
            templates::FAILED_TO_INITIALIZE,
            components::SETTINGS_MANAGER,
            e,
        )
    })?;

    settings_manager
        .set_provider_settings(&settings)
        .await
        .map_err(|e| format_error(templates::FAILED_TO_SET, actions::PROVIDER_SETTINGS, e))
}

#[command]
pub async fn set_cloud_settings(
    app_handle: AppHandle,
    settings: CloudSettings,
) -> Result<(), String> {
    let settings_manager = SettingsManager::new(app_handle).map_err(|e| {
        format_error(
            templates::FAILED_TO_INITIALIZE,
            components::SETTINGS_MANAGER,
            e,
        )
    })?;

    settings_manager
        .set_cloud_settings(&settings)
        .await
        .map_err(|e| format_error(templates::FAILED_TO_SET, actions::CLOUD_SETTINGS, e))
}

#[command]
pub async fn set_audio_settings(
    app_handle: AppHandle,
    settings: AudioSettings,
) -> Result<(), String> {
    let settings_manager = SettingsManager::new(app_handle).map_err(|e| {
        format_error(
            templates::FAILED_TO_INITIALIZE,
            components::SETTINGS_MANAGER,
            e,
        )
    })?;

    settings_manager
        .set_audio_settings(&settings)
        .await
        .map_err(|e| format_error(templates::FAILED_TO_SET, actions::AUDIO_SETTINGS, e))
}

#[command]
pub async fn set_tool_settings(
    app_handle: AppHandle,
    settings: ToolSettings,
) -> Result<(), String> {
    let settings_manager = SettingsManager::new(app_handle).map_err(|e| {
        format_error(
            templates::FAILED_TO_INITIALIZE,
            components::SETTINGS_MANAGER,
            e,
        )
    })?;

    settings_manager
        .set_tool_settings(&settings)
        .await
        .map_err(|e| format_error(templates::FAILED_TO_SET, actions::TOOL_SETTINGS, e))
}

#[command]
pub async fn set_onboarding_settings(
    app_handle: AppHandle,
    settings: OnboardingSettings,
) -> Result<(), String> {
    let settings_manager = SettingsManager::new(app_handle).map_err(|e| {
        format_error(
            templates::FAILED_TO_INITIALIZE,
            components::SETTINGS_MANAGER,
            e,
        )
    })?;

    settings_manager
        .set_onboarding_settings(&settings)
        .await
        .map_err(|e| format_error(templates::FAILED_TO_SET, actions::ONBOARDING_SETTINGS, e))
}

#[command]
pub async fn set_autostart_enabled(app_handle: AppHandle, enabled: bool) -> Result<(), String> {
    let settings_manager = SettingsManager::new(app_handle).map_err(|e| {
        format_error(
            templates::FAILED_TO_INITIALIZE,
            components::SETTINGS_MANAGER,
            e,
        )
    })?;

    settings_manager
        .set_autostart_enabled(enabled)
        .await
        .map_err(|e| format_error(templates::FAILED_TO_SET, actions::AUTOSTART_SETTING, e))
}

/// Whether the settings window shows every setting or only the basic set.
#[command]
pub async fn get_advanced_settings_enabled(app_handle: AppHandle) -> Result<bool, String> {
    let settings_manager = SettingsManager::new(app_handle).map_err(|e| {
        format_error(
            templates::FAILED_TO_INITIALIZE,
            components::SETTINGS_MANAGER,
            e,
        )
    })?;

    settings_manager
        .get_advanced_settings_enabled()
        .await
        .map_err(|e| {
            format_error(
                templates::FAILED_TO_RETRIEVE,
                actions::ADVANCED_SETTINGS_TOGGLE,
                e,
            )
        })
}

#[command]
pub async fn set_advanced_settings_enabled(
    app_handle: AppHandle,
    enabled: bool,
) -> Result<(), String> {
    let settings_manager = SettingsManager::new(app_handle).map_err(|e| {
        format_error(
            templates::FAILED_TO_INITIALIZE,
            components::SETTINGS_MANAGER,
            e,
        )
    })?;

    settings_manager
        .set_advanced_settings_enabled(enabled)
        .await
        .map_err(|e| {
            format_error(
                templates::FAILED_TO_SET,
                actions::ADVANCED_SETTINGS_TOGGLE,
                e,
            )
        })
}

/// Reset all settings to defaults
#[command]
pub async fn reset_centralized_settings(app_handle: AppHandle) -> Result<(), String> {
    let settings_manager = SettingsManager::new(app_handle).map_err(|e| {
        format_error(
            templates::FAILED_TO_INITIALIZE,
            components::SETTINGS_MANAGER,
            e,
        )
    })?;

    // Reset to defaults
    let default_settings = AppSettings::default();
    settings_manager
        .save_all_settings(&default_settings)
        .await
        .map_err(|e| format_error(templates::FAILED_TO_RESTORE, actions::SETTINGS, e))?;

    Ok(())
}

/// Export all settings as JSON string
#[command]
pub async fn export_settings(app_handle: AppHandle) -> Result<String, String> {
    let settings_manager = SettingsManager::new(app_handle).map_err(|e| {
        format_error(
            templates::FAILED_TO_INITIALIZE,
            components::SETTINGS_MANAGER,
            e,
        )
    })?;

    let settings = settings_manager
        .get_all_settings()
        .await
        .map_err(|e| format_error(templates::FAILED_TO_RETRIEVE, actions::SETTINGS, e))?;

    serde_json::to_string_pretty(&settings)
        .map_err(|e| format_error(templates::FAILED_TO_ENCODE, actions::SETTINGS_JSON, e))
}

/// Maximum accepted size for an imported settings payload. The exported
/// settings JSON is a few KB; anything near this limit is not a settings
/// file (security audit 2026-02-08, item #31).
const MAX_IMPORT_SETTINGS_BYTES: usize = 1_000_000;

/// Parse and validate an imported settings JSON payload
/// (security audit 2026-02-08, item #31):
/// - reject oversized payloads,
/// - reject unknown top-level sections (a typo'd or foreign file should not
///   silently import as all-defaults),
/// - parse into the typed [`AppSettings`] struct (serde enforces shape), and
/// - apply semantic per-field validation on values the UI setters constrain.
fn parse_and_validate_settings_json(settings_json: &str) -> Result<AppSettings, String> {
    if settings_json.len() > MAX_IMPORT_SETTINGS_BYTES {
        return Err(format!(
            "Settings import rejected: payload is {} bytes, larger than the {} byte limit",
            settings_json.len(),
            MAX_IMPORT_SETTINGS_BYTES
        ));
    }

    // Known top-level sections of AppSettings; imports with other top-level
    // keys are not settings exports and are rejected rather than ignored
    const KNOWN_TOP_LEVEL_KEYS: &[&str] = &[
        "keyboard_shortcuts",
        "floating_bar",
        "agent",
        "providers",
        "cloud",
        "audio",
        "tools",
        "prompts",
        "onboarding",
        "autostart_enabled",
        "advanced_settings_enabled",
        "cli",
        "voice_transcription",
        "triggers",
    ];

    let raw: serde_json::Value = serde_json::from_str(settings_json)
        .map_err(|e| format_error(templates::FAILED_TO_PARSE, actions::SETTINGS_JSON, e))?;

    let object = raw
        .as_object()
        .ok_or_else(|| "Settings import rejected: payload is not a JSON object".to_string())?;

    for key in object.keys() {
        if !KNOWN_TOP_LEVEL_KEYS.contains(&key.as_str()) {
            return Err(format!(
                "Settings import rejected: unknown top-level section '{}'",
                key
            ));
        }
    }

    let settings: AppSettings = serde_json::from_value(raw)
        .map_err(|e| format_error(templates::FAILED_TO_PARSE, actions::SETTINGS_JSON, e))?;

    validate_imported_settings(&settings)?;

    Ok(settings)
}

/// Semantic validation of imported settings values, mirroring the
/// constraints the settings UI applies before saving (audit item #31).
fn validate_imported_settings(settings: &AppSettings) -> Result<(), String> {
    // Floating bar
    let opacity = settings.floating_bar.opacity;
    if !(0.0..=1.0).contains(&opacity) || opacity.is_nan() {
        return Err(format!(
            "Settings import rejected: floating_bar.opacity {} is outside 0.0..=1.0",
            opacity
        ));
    }

    // Agent modes
    if !matches!(settings.agent.trigger_mode.as_str(), "tap" | "hold") {
        return Err(format!(
            "Settings import rejected: agent.trigger_mode '{}' is not 'tap' or 'hold'",
            settings.agent.trigger_mode
        ));
    }
    if !matches!(settings.agent.execution_mode.as_str(), "single" | "multi") {
        return Err(format!(
            "Settings import rejected: agent.execution_mode '{}' is not 'single' or 'multi'",
            settings.agent.execution_mode
        ));
    }

    // Audio
    if !matches!(
        settings.audio.dictation_trigger_mode.as_str(),
        "tap" | "hold"
    ) {
        return Err(format!(
            "Settings import rejected: audio.dictation_trigger_mode '{}' is not 'tap' or 'hold'",
            settings.audio.dictation_trigger_mode
        ));
    }
    let sensitivity = settings.audio.always_listening_sensitivity;
    if !(0.0..=1.0).contains(&sensitivity) || sensitivity.is_nan() {
        return Err(format!(
            "Settings import rejected: audio.always_listening_sensitivity {} is outside 0.0..=1.0",
            sensitivity
        ));
    }

    // Cloud: same URL and security-level rules the cloud settings surface
    // enforces (audit items #30/#31)
    if !settings.cloud.server_url.trim().is_empty() {
        crate::cloud::config::CloudConfig::validate_server_url(&settings.cloud.server_url)
            .map_err(|e| format!("Settings import rejected: cloud.server_url invalid: {}", e))?;
    }
    if !matches!(
        settings.cloud.security_level.as_str(),
        "low" | "medium" | "high"
    ) {
        return Err(format!(
            "Settings import rejected: cloud.security_level '{}' is not low/medium/high",
            settings.cloud.security_level
        ));
    }

    // Voice transcription
    if !(8_000..=192_000).contains(&settings.voice_transcription.sample_rate) {
        return Err(format!(
            "Settings import rejected: voice_transcription.sample_rate {} is outside 8000..=192000",
            settings.voice_transcription.sample_rate
        ));
    }
    if !(1..=2).contains(&settings.voice_transcription.channels) {
        return Err(format!(
            "Settings import rejected: voice_transcription.channels {} is not 1 or 2",
            settings.voice_transcription.channels
        ));
    }

    Ok(())
}

/// Import settings from JSON string.
///
/// The payload is size-capped, must contain only known settings sections,
/// must parse into the typed settings schema, and must pass the same
/// per-field validation the settings UI applies
/// (security audit 2026-02-08, item #31).
#[command]
pub async fn import_settings(app_handle: AppHandle, settings_json: String) -> Result<(), String> {
    let settings_manager = SettingsManager::new(app_handle).map_err(|e| {
        format_error(
            templates::FAILED_TO_INITIALIZE,
            components::SETTINGS_MANAGER,
            e,
        )
    })?;

    let settings = parse_and_validate_settings_json(&settings_json)?;

    settings_manager
        .save_all_settings(&settings)
        .await
        .map_err(|e| format_error(templates::FAILED_TO_SAVE, actions::SETTINGS, e))?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn valid_settings_json() -> String {
        serde_json::to_string(&AppSettings::default())
            .unwrap_or_else(|e| panic!("default settings must serialize: {}", e))
    }

    #[test]
    fn valid_export_round_trips() {
        let json = valid_settings_json();
        assert!(
            parse_and_validate_settings_json(&json).is_ok(),
            "a default settings export must import cleanly"
        );
    }

    #[test]
    fn oversized_payload_rejected() {
        let padding = " ".repeat(MAX_IMPORT_SETTINGS_BYTES);
        let json = format!("{}{}", valid_settings_json(), padding);
        assert!(parse_and_validate_settings_json(&json).is_err());
    }

    #[test]
    fn non_object_and_garbage_rejected() {
        assert!(parse_and_validate_settings_json("[]").is_err());
        assert!(parse_and_validate_settings_json("\"hi\"").is_err());
        assert!(parse_and_validate_settings_json("not json at all").is_err());
    }

    #[test]
    fn unknown_top_level_key_rejected() {
        let mut value: serde_json::Value = serde_json::from_str(&valid_settings_json())
            .unwrap_or_else(|e| panic!("parse failed: {}", e));
        value["totally_unknown_section"] = serde_json::json!({"a": 1});
        let json = value.to_string();
        assert!(parse_and_validate_settings_json(&json).is_err());
    }

    #[test]
    fn out_of_range_values_rejected() {
        let mut value: serde_json::Value = serde_json::from_str(&valid_settings_json())
            .unwrap_or_else(|e| panic!("parse failed: {}", e));

        let mut bad = value.clone();
        bad["floating_bar"]["opacity"] = serde_json::json!(4.2);
        assert!(parse_and_validate_settings_json(&bad.to_string()).is_err());

        let mut bad = value.clone();
        bad["agent"]["trigger_mode"] = serde_json::json!("yolo");
        assert!(parse_and_validate_settings_json(&bad.to_string()).is_err());

        let mut bad = value.clone();
        bad["cloud"]["security_level"] = serde_json::json!("off");
        assert!(parse_and_validate_settings_json(&bad.to_string()).is_err());

        let mut bad = value.clone();
        bad["voice_transcription"]["sample_rate"] = serde_json::json!(1);
        assert!(parse_and_validate_settings_json(&bad.to_string()).is_err());

        // Cloud server URL must be a secure, well-formed URL (#30)
        bad = value.clone();
        bad["cloud"]["server_url"] = serde_json::json!("javascript:alert(1)");
        assert!(parse_and_validate_settings_json(&bad.to_string()).is_err());

        // And the original value still passes
        value["floating_bar"]["opacity"] = serde_json::json!(0.8);
        assert!(parse_and_validate_settings_json(&value.to_string()).is_ok());
    }
}

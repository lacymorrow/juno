//! Which microphone Juno listens on, and which speaker it talks out of.
//!
//! Listing devices is read-only: it never opens one, so it asks for no
//! microphone access and makes no sound. Opening is the capture path's job
//! (`tauri_plugin_voice_transcription::devices`), and the choice stored here is
//! what it reads.
//!
//! The microphone choice takes effect immediately. If always-listening is
//! running when the choice changes, it is restarted on the new device, because
//! a picker whose effect waits for the next restart is a picker that appears
//! not to work.

use crate::settings::manager::SettingsManager;
use crate::state::AppState;
use cpal::traits::{DeviceTrait, HostTrait};
use serde::Serialize;
use std::collections::HashSet;
use tauri::{AppHandle, State};
use tracing::{info, warn};

/// One device, as the Audio pane lists it.
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct AudioDeviceEntry {
    /// The device name, which is also its stored id.
    pub name: String,
    /// True for the one macOS would choose on its own.
    pub is_default: bool,
}

/// Everything the Audio pane needs to draw both pickers.
#[derive(Debug, Clone, Serialize)]
pub struct AudioDeviceChoices {
    pub inputs: Vec<AudioDeviceEntry>,
    pub outputs: Vec<AudioDeviceEntry>,
    /// The microphone the person chose, or null for "follow the system".
    pub chosen_input: Option<String>,
    /// The speaker the person chose, or null for "follow the system".
    pub chosen_output: Option<String>,
    /// What the next dictation will actually open. Null means there is no
    /// microphone at all.
    pub effective_input: Option<String>,
    /// The chosen microphone, when it is not connected. The pane says so
    /// rather than showing a choice that is quietly not in effect.
    pub missing_input: Option<String>,
    /// The chosen speaker, when it is not connected.
    pub missing_output: Option<String>,
}

/// Every speaker this machine has.
fn list_output_devices() -> Vec<AudioDeviceEntry> {
    let host = cpal::default_host();
    let default_name = host.default_output_device().and_then(|d| d.name().ok());
    let Ok(devices) = host.output_devices() else {
        warn!("[AudioDevices] The system would not list output devices");
        return Vec::new();
    };

    let mut listed = Vec::new();
    let mut seen = HashSet::new();
    for device in devices {
        let Ok(name) = device.name() else { continue };
        if !seen.insert(name.clone()) {
            continue;
        }
        let is_default = default_name.as_deref() == Some(name.as_str());
        listed.push(AudioDeviceEntry { name, is_default });
    }
    listed
}

/// The chosen device, when it is not among the connected ones.
///
/// Pure: this is the sentence the pane shows, and it should not need hardware
/// to be right.
fn missing_choice(chosen: Option<&str>, present: &[AudioDeviceEntry]) -> Option<String> {
    let chosen = chosen.map(str::trim).filter(|c| !c.is_empty())?;
    if present.iter().any(|device| device.name == chosen) {
        None
    } else {
        Some(chosen.to_string())
    }
}

/// List both pickers and say which choice is in force.
#[tauri::command]
pub async fn list_audio_devices(app: AppHandle) -> Result<AudioDeviceChoices, String> {
    let settings_manager =
        SettingsManager::new(app).map_err(|e| format!("Failed to create settings manager: {e}"))?;
    let audio = settings_manager
        .get_audio_settings()
        .await
        .map_err(|e| format!("Failed to get audio settings: {e}"))?;

    let inputs: Vec<AudioDeviceEntry> = tauri_plugin_voice_transcription::list_input_devices()
        .into_iter()
        .map(|device| AudioDeviceEntry {
            name: device.name,
            is_default: device.is_default,
        })
        .collect();
    let outputs = list_output_devices();

    Ok(AudioDeviceChoices {
        missing_input: missing_choice(audio.input_device.as_deref(), &inputs),
        missing_output: missing_choice(audio.output_device.as_deref(), &outputs),
        chosen_input: audio.input_device,
        chosen_output: audio.output_device,
        effective_input: tauri_plugin_voice_transcription::effective_input_device_name(),
        inputs,
        outputs,
    })
}

/// Choose the microphone. `None` follows the system.
#[tauri::command]
pub async fn set_audio_input_device(
    name: Option<String>,
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<(), String> {
    let settings_manager = SettingsManager::new(app.clone())
        .map_err(|e| format!("Failed to create settings manager: {e}"))?;
    let mut audio = settings_manager
        .get_audio_settings()
        .await
        .map_err(|e| format!("Failed to get audio settings: {e}"))?;

    audio.input_device = name.clone();
    settings_manager
        .set_audio_settings(&audio)
        .await
        .map_err(|e| format!("Failed to save the microphone choice: {e}"))?;
    tauri_plugin_voice_transcription::set_preferred_input_device(name.clone());

    info!(
        "[AudioDevices] Microphone set to {}",
        name.as_deref().unwrap_or("the system default")
    );

    // A running listener is still on the old device. Restart it so the choice
    // is in force now rather than after the next launch.
    if state.get_always_listening_active().unwrap_or(false) {
        info!("[AudioDevices] Restarting always-listening on the new microphone");
        if let Err(e) = crate::commands::always_listening::stop_always_listening_mode(
            app.clone(),
            state.clone(),
        )
        .await
        {
            warn!("[AudioDevices] Could not stop the old listener: {e}");
        }
        if let Err(e) =
            crate::commands::always_listening::start_always_listening_mode(app, state).await
        {
            // The start path already told the UI it is not listening. Surface
            // the reason rather than reporting a success nobody got.
            return Err(e);
        }
    }

    Ok(())
}

/// Choose the speaker Juno's voice plays through. `None` follows the system.
#[tauri::command]
pub async fn set_audio_output_device(
    name: Option<String>,
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<(), String> {
    let settings_manager =
        SettingsManager::new(app).map_err(|e| format!("Failed to create settings manager: {e}"))?;
    let mut audio = settings_manager
        .get_audio_settings()
        .await
        .map_err(|e| format!("Failed to get audio settings: {e}"))?;

    audio.output_device = name.clone();
    settings_manager
        .set_audio_settings(&audio)
        .await
        .map_err(|e| format!("Failed to save the speaker choice: {e}"))?;
    state.set_output_device(name.clone())?;

    info!(
        "[AudioDevices] Speaker set to {}",
        name.as_deref().unwrap_or("the system default")
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{missing_choice, AudioDeviceEntry};

    fn present() -> Vec<AudioDeviceEntry> {
        vec![
            AudioDeviceEntry {
                name: "MacBook Air Microphone".to_string(),
                is_default: true,
            },
            AudioDeviceEntry {
                name: "Pilot Microphone".to_string(),
                is_default: false,
            },
        ]
    }

    #[test]
    fn following_the_system_is_never_reported_as_missing() {
        assert_eq!(missing_choice(None, &present()), None);
    }

    #[test]
    fn a_connected_choice_is_not_missing() {
        assert_eq!(missing_choice(Some("Pilot Microphone"), &present()), None);
    }

    /// The whole point of the line: the person is told their choice is not in
    /// effect instead of reading it back as if it were.
    #[test]
    fn an_unplugged_choice_is_named() {
        assert_eq!(
            missing_choice(Some("AirPods Pro"), &present()),
            Some("AirPods Pro".to_string())
        );
    }

    #[test]
    fn a_blank_choice_is_no_choice() {
        for blank in [Some(""), Some("   ")] {
            assert_eq!(missing_choice(blank, &present()), None, "{blank:?}");
        }
    }
}

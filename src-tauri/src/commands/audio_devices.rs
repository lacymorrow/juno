//! Which microphone Juno listens on, and which speaker it talks out of.
//!
//! Listing devices is read-only: it never opens one, so it asks for no
//! microphone access and makes no sound. Opening is the capture path's job
//! (`tauri_plugin_voice_transcription::devices`), and the choice stored here is
//! what it reads.
//!
//! A device is chosen and remembered by its CoreAudio UID, never by its
//! display name. The name is stored beside it only so the pane can say which
//! device is not connected.
//!
//! The microphone choice takes effect immediately. If always-listening is
//! running when the choice changes, it is restarted on the new device, because
//! a picker whose effect waits for the next restart is a picker that appears
//! not to work.

use crate::settings::manager::SettingsManager;
use crate::settings::AudioSettings;
use crate::state::AppState;
use cpal::traits::HostTrait;
use serde::Serialize;
use std::collections::HashSet;
use tauri::{AppHandle, Manager, State};
use tauri_plugin_voice_transcription::{
    device_audio_id, device_ref, device_uid, find_device, uid_for_legacy_name, DeviceChoice,
    DeviceRef,
};
use tracing::{info, warn};

/// One device, as the Audio pane lists it.
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct AudioDeviceEntry {
    /// The CoreAudio UID: the value a choice is stored and matched by.
    pub id: String,
    /// What the person sees. Display only.
    pub name: String,
    /// True for the one macOS would choose on its own.
    pub is_default: bool,
}

impl AudioDeviceEntry {
    fn device_ref(&self) -> DeviceRef {
        DeviceRef {
            id: self.id.clone(),
            name: self.name.clone(),
        }
    }
}

/// Everything the Audio pane needs to draw both pickers.
#[derive(Debug, Clone, Serialize)]
pub struct AudioDeviceChoices {
    pub inputs: Vec<AudioDeviceEntry>,
    pub outputs: Vec<AudioDeviceEntry>,
    /// The id of the microphone the person chose, or null for "follow the
    /// system".
    pub chosen_input: Option<String>,
    /// The id of the speaker the person chose, or null for "follow the
    /// system".
    pub chosen_output: Option<String>,
    /// What the next dictation will actually open. Null means there is no
    /// microphone at all.
    pub effective_input: Option<String>,
    /// The chosen microphone, when it is not connected. The pane says so
    /// rather than showing a choice that is quietly not in effect.
    pub missing_input: Option<AudioDeviceEntry>,
    /// The chosen speaker, when it is not connected.
    pub missing_output: Option<AudioDeviceEntry>,
}

/// Every speaker this machine has.
fn list_output_devices() -> Vec<AudioDeviceEntry> {
    let host = cpal::default_host();
    let default_id = host.default_output_device().map(|d| device_uid(&d));
    let Ok(devices) = host.output_devices() else {
        warn!("[AudioDevices] The system would not list output devices");
        return Vec::new();
    };

    let mut listed = Vec::new();
    let mut seen = HashSet::new();
    for device in devices {
        let Some(found) = device_ref(&device) else {
            continue;
        };
        // By identity, so two speakers that share a name are both offered.
        if !seen.insert(found.id.clone()) {
            continue;
        }
        let is_default = default_id.as_deref() == Some(found.id.as_str());
        listed.push(AudioDeviceEntry {
            id: found.id,
            name: found.name,
            is_default,
        });
    }
    listed
}

fn input_entries() -> Vec<AudioDeviceEntry> {
    tauri_plugin_voice_transcription::list_input_devices()
        .into_iter()
        .map(|device| AudioDeviceEntry {
            id: device.id,
            name: device.name,
            is_default: device.is_default,
        })
        .collect()
}

/// The microphone choice stored in `audio`, in the form the plugin takes.
pub fn input_choice(audio: &AudioSettings) -> Option<DeviceChoice> {
    DeviceChoice::from_stored(
        audio.input_device_uid.as_deref(),
        audio.input_device.as_deref(),
    )
}

/// The speaker choice stored in `audio`.
pub fn output_choice(audio: &AudioSettings) -> Option<DeviceChoice> {
    DeviceChoice::from_stored(
        audio.output_device_uid.as_deref(),
        audio.output_device.as_deref(),
    )
}

/// Which row of the picker is selected, and the chosen device when it is not
/// connected.
///
/// Pure: this is the sentence the pane shows, and it should not need hardware
/// to be right.
fn selection(
    chosen: Option<&DeviceChoice>,
    present: &[AudioDeviceEntry],
) -> (Option<String>, Option<AudioDeviceEntry>) {
    let Some(chosen) = chosen else {
        return (None, None);
    };
    if let Some(found) = present
        .iter()
        .find(|device| chosen.matches(&device.device_ref()))
    {
        return (Some(found.id.clone()), None);
    }
    // Not connected. The row still needs a value to hold the selection, and a
    // choice saved before UIDs were stored has none; that value is only a
    // key for the row, never compared with a device.
    let id = chosen
        .uid
        .clone()
        .unwrap_or_else(|| format!("unconverted:{}", chosen.name));
    let missing = AudioDeviceEntry {
        id: id.clone(),
        name: chosen.label(),
        is_default: false,
    };
    (Some(id), Some(missing))
}

/// Give a choice saved by name the UID of the device that name belongs to.
///
/// This is the only place a name stands in for an identity: builds before the
/// UID was stored kept the name alone. A name converts only when exactly one
/// connected device has it (see `uid_for_legacy_name`). Returns true when
/// something changed and the settings need saving.
pub(crate) fn convert_legacy_device_names(
    audio: &mut AudioSettings,
    inputs: &[DeviceRef],
    outputs: &[DeviceRef],
) -> bool {
    let mut changed = false;
    if let Some(uid) = uid_for_legacy_name(
        audio.input_device_uid.as_deref(),
        audio.input_device.as_deref(),
        inputs,
    ) {
        audio.input_device_uid = Some(uid);
        changed = true;
    }
    if let Some(uid) = uid_for_legacy_name(
        audio.output_device_uid.as_deref(),
        audio.output_device.as_deref(),
        outputs,
    ) {
        audio.output_device_uid = Some(uid);
        changed = true;
    }
    changed
}

/// True when a choice is stored by name alone, so a conversion could apply.
fn has_legacy_choice(audio: &AudioSettings) -> bool {
    let legacy = |uid: &Option<String>, name: &Option<String>| {
        uid.as_deref().is_none_or(|u| u.trim().is_empty())
            && name.as_deref().is_some_and(|n| !n.trim().is_empty())
    };
    legacy(&audio.input_device_uid, &audio.input_device)
        || legacy(&audio.output_device_uid, &audio.output_device)
}

/// At launch, convert a choice an older build saved by name. Listing devices
/// is slow enough to matter, so it only happens when there is something to
/// convert, and it happens once: after that the UID is stored.
pub async fn convert_legacy_device_names_at_startup(app: &AppHandle, audio: &mut AudioSettings) {
    if !has_legacy_choice(audio) {
        return;
    }
    let listed = tokio::task::spawn_blocking(|| (input_entries(), list_output_devices())).await;
    let Ok((inputs, outputs)) = listed else {
        warn!("[AudioDevices] Could not list devices to convert a saved choice");
        return;
    };
    let inputs: Vec<DeviceRef> = inputs.iter().map(AudioDeviceEntry::device_ref).collect();
    let outputs: Vec<DeviceRef> = outputs.iter().map(AudioDeviceEntry::device_ref).collect();
    if !convert_legacy_device_names(audio, &inputs, &outputs) {
        return;
    }
    info!("[AudioDevices] Saved a device choice by its stable id instead of its name");
    match SettingsManager::new(app.clone()) {
        Ok(manager) => {
            if let Err(e) = manager.set_audio_settings(audio).await {
                warn!("[AudioDevices] Could not save the converted device choice: {e}");
            }
        }
        Err(e) => warn!("[AudioDevices] Could not save the converted device choice: {e}"),
    }
}

/// List both pickers and say which choice is in force.
#[tauri::command]
pub async fn list_audio_devices(app: AppHandle) -> Result<AudioDeviceChoices, String> {
    let settings_manager = SettingsManager::new(app.clone())
        .map_err(|e| format!("Failed to create settings manager: {e}"))?;
    let mut audio = settings_manager
        .get_audio_settings()
        .await
        .map_err(|e| format!("Failed to get audio settings: {e}"))?;

    let inputs = input_entries();
    let outputs = list_output_devices();

    // A device saved by name that is connected now gets its UID, wherever the
    // list is read from, so the conversion does not wait for a restart.
    let input_refs: Vec<DeviceRef> = inputs.iter().map(AudioDeviceEntry::device_ref).collect();
    let output_refs: Vec<DeviceRef> = outputs.iter().map(AudioDeviceEntry::device_ref).collect();
    if convert_legacy_device_names(&mut audio, &input_refs, &output_refs) {
        settings_manager
            .set_audio_settings(&audio)
            .await
            .map_err(|e| format!("Failed to save the converted device choice: {e}"))?;
        tauri_plugin_voice_transcription::set_preferred_input_device(input_choice(&audio));
        if let Some(state) = app.try_state::<AppState>() {
            let _ = state.set_output_device(output_choice(&audio));
        }
    }

    let (chosen_input, missing_input) = selection(input_choice(&audio).as_ref(), &inputs);
    let (chosen_output, missing_output) = selection(output_choice(&audio).as_ref(), &outputs);

    Ok(AudioDeviceChoices {
        missing_input,
        missing_output,
        chosen_input,
        chosen_output,
        effective_input: tauri_plugin_voice_transcription::effective_input_device_name(),
        inputs,
        outputs,
    })
}

/// The connected device with `id`, as a choice to store.
fn choice_for(id: &str, present: &[AudioDeviceEntry], what: &str) -> Result<DeviceChoice, String> {
    present
        .iter()
        .find(|device| device.id == id)
        .map(|device| DeviceChoice {
            uid: Some(device.id.clone()),
            name: device.name.clone(),
        })
        .ok_or_else(|| format!("That {what} is not connected."))
}

/// Choose the microphone by its id. `None` follows the system.
#[tauri::command]
pub async fn set_audio_input_device(
    id: Option<String>,
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<(), String> {
    let settings_manager = SettingsManager::new(app.clone())
        .map_err(|e| format!("Failed to create settings manager: {e}"))?;
    let mut audio = settings_manager
        .get_audio_settings()
        .await
        .map_err(|e| format!("Failed to get audio settings: {e}"))?;

    let choice = match id.as_deref() {
        Some(id) => Some(choice_for(id, &input_entries(), "microphone")?),
        None => None,
    };
    audio.input_device = choice.as_ref().map(|c| c.name.clone());
    audio.input_device_uid = choice.as_ref().and_then(|c| c.uid.clone());
    settings_manager
        .set_audio_settings(&audio)
        .await
        .map_err(|e| format!("Failed to save the microphone choice: {e}"))?;

    info!(
        "[AudioDevices] Microphone set to {}",
        choice
            .as_ref()
            .map(DeviceChoice::label)
            .unwrap_or_else(|| "the system default".to_string())
    );
    tauri_plugin_voice_transcription::set_preferred_input_device(choice);

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
        // A failure here is the person's answer: the start path has already
        // told the UI it is not listening, and reporting a success nobody got
        // is the defect this whole PR is about.
        crate::commands::always_listening::start_always_listening_mode(app, state).await?;
    }

    Ok(())
}

/// Choose the speaker Juno's voice plays through, by its id. `None` follows
/// the system.
#[tauri::command]
pub async fn set_audio_output_device(
    id: Option<String>,
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<(), String> {
    let settings_manager =
        SettingsManager::new(app).map_err(|e| format!("Failed to create settings manager: {e}"))?;
    let mut audio = settings_manager
        .get_audio_settings()
        .await
        .map_err(|e| format!("Failed to get audio settings: {e}"))?;

    let choice = match id.as_deref() {
        Some(id) => Some(choice_for(id, &list_output_devices(), "speaker")?),
        None => None,
    };
    audio.output_device = choice.as_ref().map(|c| c.name.clone());
    audio.output_device_uid = choice.as_ref().and_then(|c| c.uid.clone());
    settings_manager
        .set_audio_settings(&audio)
        .await
        .map_err(|e| format!("Failed to save the speaker choice: {e}"))?;

    info!(
        "[AudioDevices] Speaker set to {}",
        choice
            .as_ref()
            .map(DeviceChoice::label)
            .unwrap_or_else(|| "the system default".to_string())
    );
    state.set_output_device(choice)?;
    Ok(())
}

/// What to hand `say -a` for the chosen speaker: its current CoreAudio id,
/// which `say` accepts, found by the choice's UID.
///
/// `None` when nothing is chosen or the chosen speaker is not connected, so
/// `say` speaks out of the system output rather than failing. The list is
/// `devices()` rather than `output_devices()` because this runs before every
/// utterance and the latter asks every device for its formats.
pub fn say_device_argument(choice: Option<&DeviceChoice>) -> Option<String> {
    let choice = choice?;
    let host = cpal::default_host();
    let devices = host.devices().ok()?;
    match find_device(devices, choice).and_then(|device| device_audio_id(&device)) {
        Some(id) => Some(id.to_string()),
        None => {
            warn!(
                "[AudioDevices] Speaker {} is not connected; using the system output",
                choice.label()
            );
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{convert_legacy_device_names, has_legacy_choice, selection, AudioDeviceEntry};
    use crate::settings::AudioSettings;
    use tauri_plugin_voice_transcription::{DeviceChoice, DeviceRef};

    fn entry(id: &str, name: &str, is_default: bool) -> AudioDeviceEntry {
        AudioDeviceEntry {
            id: id.to_string(),
            name: name.to_string(),
            is_default,
        }
    }

    fn present() -> Vec<AudioDeviceEntry> {
        vec![
            entry("uid-builtin", "MacBook Air Microphone", true),
            entry("uid-pilot", "Pilot Microphone", false),
        ]
    }

    fn picked(uid: &str, name: &str) -> DeviceChoice {
        DeviceChoice {
            uid: Some(uid.to_string()),
            name: name.to_string(),
        }
    }

    fn refs(entries: &[AudioDeviceEntry]) -> Vec<DeviceRef> {
        entries.iter().map(AudioDeviceEntry::device_ref).collect()
    }

    #[test]
    fn following_the_system_is_never_reported_as_missing() {
        assert_eq!(selection(None, &present()), (None, None));
    }

    #[test]
    fn a_connected_choice_is_selected_by_its_id_and_not_missing() {
        let (chosen, missing) = selection(Some(&picked("uid-pilot", "Pilot")), &present());
        assert_eq!(chosen.as_deref(), Some("uid-pilot"));
        assert_eq!(missing, None);
    }

    /// The whole point of the line: the person is told their choice is not in
    /// effect instead of reading it back as if it were.
    #[test]
    fn an_unplugged_choice_is_named() {
        let (chosen, missing) = selection(Some(&picked("uid-airpods", "AirPods Pro")), &present());
        assert_eq!(chosen.as_deref(), Some("uid-airpods"));
        assert_eq!(missing.map(|m| m.name), Some("AirPods Pro".to_string()));
    }

    /// Two microphones with one name: the one chosen is the one selected.
    #[test]
    fn a_choice_selects_the_right_one_of_two_with_the_same_name() {
        let twins = vec![
            entry("uid-1", "USB Microphone", false),
            entry("uid-2", "USB Microphone", false),
        ];
        let (chosen, missing) = selection(Some(&picked("uid-2", "USB Microphone")), &twins);
        assert_eq!(chosen.as_deref(), Some("uid-2"));
        assert_eq!(missing, None);
    }

    #[test]
    fn an_unconverted_unplugged_choice_still_gets_a_row() {
        let legacy = DeviceChoice {
            uid: None,
            name: "AirPods Pro".to_string(),
        };
        let (chosen, missing) = selection(Some(&legacy), &present());
        let missing = missing.map(|m| (m.id, m.name));
        assert_eq!(
            missing,
            Some((
                "unconverted:AirPods Pro".to_string(),
                "AirPods Pro".to_string()
            ))
        );
        assert_eq!(chosen.as_deref(), Some("unconverted:AirPods Pro"));
    }

    #[test]
    fn a_saved_name_gets_the_uid_of_the_connected_device() {
        let mut audio = AudioSettings {
            input_device: Some("Pilot Microphone".to_string()),
            output_device: Some("MacBook Air Speakers".to_string()),
            ..AudioSettings::default()
        };
        let outputs = vec![entry("uid-speakers", "MacBook Air Speakers", true)];
        assert!(has_legacy_choice(&audio));
        assert!(convert_legacy_device_names(
            &mut audio,
            &refs(&present()),
            &refs(&outputs)
        ));
        assert_eq!(audio.input_device_uid.as_deref(), Some("uid-pilot"));
        assert_eq!(audio.output_device_uid.as_deref(), Some("uid-speakers"));
        // The name stays: it is what the pane shows when the device is gone.
        assert_eq!(audio.input_device.as_deref(), Some("Pilot Microphone"));
        assert!(!has_legacy_choice(&audio));
    }

    #[test]
    fn a_saved_name_with_no_connected_device_waits() {
        let mut audio = AudioSettings {
            input_device: Some("AirPods Pro".to_string()),
            ..AudioSettings::default()
        };
        assert!(!convert_legacy_device_names(
            &mut audio,
            &refs(&present()),
            &[]
        ));
        assert_eq!(audio.input_device_uid, None);
        assert!(has_legacy_choice(&audio));
    }

    #[test]
    fn a_stored_uid_is_not_overwritten_by_a_name_match() {
        let mut audio = AudioSettings {
            input_device: Some("Pilot Microphone".to_string()),
            input_device_uid: Some("uid-other".to_string()),
            ..AudioSettings::default()
        };
        assert!(!convert_legacy_device_names(
            &mut audio,
            &refs(&present()),
            &[]
        ));
        assert_eq!(audio.input_device_uid.as_deref(), Some("uid-other"));
    }

    #[test]
    fn following_the_system_has_nothing_to_convert() {
        let mut audio = AudioSettings::default();
        assert!(!has_legacy_choice(&audio));
        assert!(!convert_legacy_device_names(
            &mut audio,
            &refs(&present()),
            &[]
        ));
    }

    /// Settings written before the UID fields existed must still load.
    #[test]
    fn an_older_settings_file_loads_without_the_uid_fields() {
        let mut stored = serde_json::to_value(AudioSettings::default()).expect("serializes");
        let object = stored.as_object_mut().expect("is an object");
        object.remove("input_device_uid");
        object.remove("output_device_uid");
        object.insert("input_device".to_string(), "Pilot Microphone".into());
        let loaded: AudioSettings = serde_json::from_value(stored).expect("loads");
        assert_eq!(loaded.input_device.as_deref(), Some("Pilot Microphone"));
        assert_eq!(loaded.input_device_uid, None);
    }
}

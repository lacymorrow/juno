//! Which microphone Juno listens on.
//!
//! cpal identifies a device by its name, so the name is also the stored
//! choice. The person's choice lives here rather than in the controllers
//! because both capture paths (dictation and always-listening) need it, and
//! both read it on the worker thread at the moment they open the stream. That
//! is the only moment it matters: a device unplugged between sessions is
//! noticed on the next one, not remembered as broken.

use crate::capture_failure::CaptureStartFailure;
use crate::utils::{downmix_f32_to_mono, downmix_i16_to_mono};
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::SampleFormat;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::sync::mpsc::Sender;
use std::sync::Mutex;

/// One microphone, as the settings window lists it.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AudioDeviceInfo {
    /// The device name, which is also its id.
    pub name: String,
    /// True for the one macOS would choose on its own.
    pub is_default: bool,
}

/// The microphone the person picked, or `None` for "whatever macOS is using".
///
/// A plain `None` default matters: an install that has never opened Audio
/// settings follows the system, which is what it did before this existed.
static PREFERRED_INPUT: Mutex<Option<String>> = Mutex::new(None);

/// Remember the microphone the person picked. `None` means follow the system.
pub fn set_preferred_input_device(name: Option<String>) {
    let cleaned = name.map(|n| n.trim().to_string()).filter(|n| !n.is_empty());
    match PREFERRED_INPUT.lock() {
        Ok(mut preferred) => {
            tracing::info!(
                "[VoiceDevices] Microphone preference set to {}",
                cleaned.as_deref().unwrap_or("the system default")
            );
            *preferred = cleaned;
        }
        Err(e) => tracing::error!("[VoiceDevices] Could not store the microphone choice: {e}"),
    }
}

/// The microphone the person picked, if any.
pub fn preferred_input_device() -> Option<String> {
    match PREFERRED_INPUT.lock() {
        Ok(preferred) => preferred.clone(),
        Err(e) => {
            tracing::error!("[VoiceDevices] Could not read the microphone choice: {e}");
            None
        }
    }
}

/// Every microphone this machine has, default first flagged.
///
/// Read-only: listing devices never opens one, so it does not ask for
/// microphone access and makes no sound.
pub fn list_input_devices() -> Vec<AudioDeviceInfo> {
    let host = cpal::default_host();
    let default_name = host.default_input_device().and_then(|d| d.name().ok());
    let Ok(devices) = host.input_devices() else {
        tracing::warn!("[VoiceDevices] The system would not list input devices");
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
        listed.push(AudioDeviceInfo { name, is_default });
    }
    listed
}

/// What listening settled on, which is not always what was asked for.
pub struct ResolvedInput {
    pub device: cpal::Device,
    /// The device actually opened.
    pub name: String,
    /// Set when the chosen device was not there and the system default stood
    /// in for it. Unplugged headphones must not brick dictation.
    pub substituted_for: Option<String>,
}

/// The decision, separated from the hardware so it can be tested.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum InputChoice {
    /// The device the person picked, and it is here.
    Chosen(String),
    /// The chosen device is gone; this is the system default standing in.
    Substituted { requested: String, used: String },
    /// No preference, so whatever macOS is using.
    SystemDefault(String),
    /// Nothing to listen on.
    Nothing { requested: Option<String> },
}

/// Which microphone to open, given the preference, what is plugged in, and
/// what the system would pick.
///
/// Pure, because the fallback is the part that matters and it must be provable
/// without a microphone.
pub(crate) fn choose_input(
    preferred: Option<&str>,
    available: &[String],
    system_default: Option<&str>,
) -> InputChoice {
    let wanted = preferred.map(str::trim).filter(|w| !w.is_empty());

    if let Some(wanted) = wanted {
        if available.iter().any(|name| name == wanted) {
            return InputChoice::Chosen(wanted.to_string());
        }
        // The chosen device went away. Fall back rather than refusing to
        // listen, and remember what was asked for so the person is told.
        return match system_default.filter(|d| !d.is_empty()) {
            Some(default) => InputChoice::Substituted {
                requested: wanted.to_string(),
                used: default.to_string(),
            },
            None => InputChoice::Nothing {
                requested: Some(wanted.to_string()),
            },
        };
    }

    match system_default.filter(|d| !d.is_empty()) {
        Some(default) => InputChoice::SystemDefault(default.to_string()),
        None => InputChoice::Nothing { requested: None },
    }
}

/// Fallback label for a device whose name the system will not give us. It is
/// still openable, so refusing to listen over a missing string would be worse.
const UNNAMED_DEVICE: &str = "your microphone";

/// Pull a device out of the enumerated list by name.
fn take_device(present: &mut Vec<(String, cpal::Device)>, wanted: &str) -> Option<cpal::Device> {
    present
        .iter()
        .position(|(name, _)| name == wanted)
        .map(|index| present.remove(index).1)
}

/// Open the microphone Juno should listen on.
pub fn resolve_input_device(
    preferred: Option<&str>,
) -> std::result::Result<ResolvedInput, CaptureStartFailure> {
    let host = cpal::default_host();

    let mut present: Vec<(String, cpal::Device)> = Vec::new();
    if let Ok(devices) = host.input_devices() {
        for device in devices {
            if let Ok(name) = device.name() {
                present.push((name, device));
            }
        }
    }

    let default_device = host.default_input_device();
    // A device whose name the system will not give us is still openable, so
    // it gets a label rather than being treated as absent.
    let default_name = default_device
        .as_ref()
        .map(|d| d.name().unwrap_or_else(|_| UNNAMED_DEVICE.to_string()));
    let names: Vec<String> = present.iter().map(|(name, _)| name.clone()).collect();

    match choose_input(preferred, &names, default_name.as_deref()) {
        InputChoice::Chosen(name) => {
            let device = take_device(&mut present, &name).ok_or_else(|| {
                CaptureStartFailure::ChosenDeviceGone {
                    requested: name.clone(),
                }
            })?;
            Ok(ResolvedInput {
                device,
                name,
                substituted_for: None,
            })
        }
        InputChoice::Substituted { requested, used } => {
            let device = take_device(&mut present, &used)
                .or(default_device)
                .ok_or_else(|| CaptureStartFailure::ChosenDeviceGone {
                    requested: requested.clone(),
                })?;
            tracing::warn!(
                "[VoiceDevices] {requested} is not connected; listening on {used} instead"
            );
            Ok(ResolvedInput {
                device,
                name: used,
                substituted_for: Some(requested),
            })
        }
        InputChoice::SystemDefault(name) => {
            let device = take_device(&mut present, &name)
                .or(default_device)
                .ok_or(CaptureStartFailure::NoInputDevice)?;
            Ok(ResolvedInput {
                device,
                name,
                substituted_for: None,
            })
        }
        InputChoice::Nothing { requested } => Err(match requested {
            Some(requested) => CaptureStartFailure::ChosenDeviceGone { requested },
            None => CaptureStartFailure::NoInputDevice,
        }),
    }
}

/// Open a microphone and start it, sending mono f32 chunks down `audio_data_tx`.
///
/// Any channel count is averaged to mono (see
/// [`crate::utils::downmix_f32_to_mono`]). There is deliberately no channel
/// filter: rejecting a three-channel device is what silently killed
/// always-listening before, and the person's own default microphone reports
/// three channels.
pub fn start_mono_stream(
    device: &cpal::Device,
    device_name: &str,
    config: &cpal::StreamConfig,
    sample_format: SampleFormat,
    audio_data_tx: Sender<Vec<f32>>,
) -> std::result::Result<cpal::Stream, CaptureStartFailure> {
    let channels = config.channels as usize;
    if channels > 1 {
        tracing::info!(
            "[VoiceDevices] {device_name} reports {channels} channels; averaging down to mono"
        );
    }

    let built = match sample_format {
        SampleFormat::F32 => {
            let tx = audio_data_tx;
            device.build_input_stream(
                config,
                move |data: &[f32], _: &cpal::InputCallbackInfo| {
                    if let Err(e) = tx.send(downmix_f32_to_mono(data, channels)) {
                        tracing::error!("[VoiceDevices] Could not pass on audio: {e}");
                    }
                },
                |err| tracing::error!("[VoiceDevices] Input stream error: {err}"),
                None,
            )
        }
        SampleFormat::I16 => {
            let tx = audio_data_tx;
            device.build_input_stream(
                config,
                move |data: &[i16], _: &cpal::InputCallbackInfo| {
                    if let Err(e) = tx.send(downmix_i16_to_mono(data, channels)) {
                        tracing::error!("[VoiceDevices] Could not pass on audio: {e}");
                    }
                },
                |err| tracing::error!("[VoiceDevices] Input stream error: {err}"),
                None,
            )
        }
        other => {
            return Err(CaptureStartFailure::UnsupportedSampleFormat {
                device: device_name.to_string(),
                format: format!("{other:?}"),
            })
        }
    };

    let stream = built.map_err(|e| CaptureStartFailure::StreamBuildFailed {
        device: device_name.to_string(),
        detail: format!("{e:?}"),
    })?;

    stream
        .play()
        .map_err(|e| CaptureStartFailure::StreamStartFailed {
            device: device_name.to_string(),
            detail: format!("{e:?}"),
        })?;

    Ok(stream)
}

/// The device the next capture will open, for the settings window to show
/// without opening anything.
pub fn effective_input_device_name() -> Option<String> {
    let preferred = preferred_input_device();
    let listed = list_input_devices();
    let names: Vec<String> = listed.iter().map(|d| d.name.clone()).collect();
    let default_name = listed
        .iter()
        .find(|d| d.is_default)
        .map(|d| d.name.as_str());
    match choose_input(preferred.as_deref(), &names, default_name) {
        InputChoice::Chosen(name) => Some(name),
        InputChoice::Substituted { used, .. } => Some(used),
        InputChoice::SystemDefault(name) => Some(name),
        InputChoice::Nothing { .. } => None,
    }
}

#[cfg(test)]
mod tests {
    use super::{choose_input, InputChoice, UNNAMED_DEVICE};

    fn devices() -> Vec<String> {
        vec![
            "MacBook Air Microphone".to_string(),
            "Shure MV7".to_string(),
        ]
    }

    #[test]
    fn no_preference_follows_the_system() {
        assert_eq!(
            choose_input(None, &devices(), Some("MacBook Air Microphone")),
            InputChoice::SystemDefault("MacBook Air Microphone".to_string())
        );
    }

    #[test]
    fn a_connected_choice_is_honoured() {
        assert_eq!(
            choose_input(
                Some("Shure MV7"),
                &devices(),
                Some("MacBook Air Microphone")
            ),
            InputChoice::Chosen("Shure MV7".to_string())
        );
    }

    /// Unplugging the headphones must not brick dictation.
    #[test]
    fn a_vanished_choice_falls_back_to_the_system_default() {
        assert_eq!(
            choose_input(
                Some("AirPods Pro"),
                &devices(),
                Some("MacBook Air Microphone")
            ),
            InputChoice::Substituted {
                requested: "AirPods Pro".to_string(),
                used: "MacBook Air Microphone".to_string(),
            }
        );
    }

    #[test]
    fn a_vanished_choice_with_nothing_left_is_a_failure() {
        assert_eq!(
            choose_input(Some("AirPods Pro"), &[], None),
            InputChoice::Nothing {
                requested: Some("AirPods Pro".to_string())
            }
        );
    }

    #[test]
    fn no_microphone_at_all_is_a_failure() {
        assert_eq!(
            choose_input(None, &[], None),
            InputChoice::Nothing { requested: None }
        );
    }

    /// A stored empty string is the same as no preference, not a device named
    /// "". Settings files written by hand have both.
    #[test]
    fn a_blank_preference_is_no_preference() {
        for blank in ["", "   "] {
            assert_eq!(
                choose_input(Some(blank), &devices(), Some("Shure MV7")),
                InputChoice::SystemDefault("Shure MV7".to_string()),
                "{blank:?} should mean no preference"
            );
        }
    }

    #[test]
    fn the_unnamed_device_label_reads_as_a_device() {
        // It is dropped into sentences such as "Juno could not open ...".
        assert!(!UNNAMED_DEVICE.is_empty());
        assert_eq!(UNNAMED_DEVICE, UNNAMED_DEVICE.to_lowercase());
    }
}

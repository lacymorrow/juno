//! Which audio device is which.
//!
//! A device is identified by its CoreAudio UID (`kAudioDevicePropertyDeviceUID`),
//! a string that survives reboots and reconnects and differs between two
//! devices that happen to share a display name. The name is only ever shown to
//! the person. Both the microphone and the speaker pickers use this, which is
//! why it lives apart from the capture code in `devices`.
//!
//! The one place a name still decides which device is meant is a choice saved
//! by an older build, which stored nothing else; see [`DeviceChoice::matches`].

use cpal::traits::DeviceTrait;
use serde::{Deserialize, Serialize};

/// One device that is present right now.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeviceRef {
    /// The stable identity: the CoreAudio UID.
    pub id: String,
    /// What the person sees. Never compared once a UID is stored.
    pub name: String,
}

/// The device the person picked.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct DeviceChoice {
    /// The CoreAudio UID. `None` only for a choice saved by an older build,
    /// which stored the name alone; it is converted the next time the device
    /// is seen.
    pub uid: Option<String>,
    /// The name it had when it was chosen, for saying "X is not connected".
    pub name: String,
}

impl DeviceChoice {
    /// Trimmed, or `None` when nothing is chosen. A stored empty string is the
    /// same as no choice, not a device named "".
    pub fn cleaned(self) -> Option<Self> {
        let uid = self
            .uid
            .map(|u| u.trim().to_string())
            .filter(|u| !u.is_empty());
        let name = self.name.trim().to_string();
        if uid.is_none() && name.is_empty() {
            return None;
        }
        Some(Self { uid, name })
    }

    /// Build a choice from the two stored fields, or `None` for "follow the
    /// system".
    pub fn from_stored(uid: Option<&str>, name: Option<&str>) -> Option<Self> {
        Self {
            uid: uid.map(str::to_string),
            name: name.unwrap_or_default().to_string(),
        }
        .cleaned()
    }

    /// What to call this choice in a sentence.
    pub fn label(&self) -> String {
        if !self.name.is_empty() {
            self.name.clone()
        } else {
            self.uid.clone().unwrap_or_default()
        }
    }

    /// True when `device` is the one chosen.
    ///
    /// By UID whenever one is stored. A choice with no UID was saved by a
    /// build that stored the name alone, so the name is all there is to match
    /// on until the host converts it. This is the only place a display name
    /// decides which device is meant.
    pub fn matches(&self, device: &DeviceRef) -> bool {
        match &self.uid {
            Some(uid) => *uid == device.id,
            None => !self.name.is_empty() && self.name == device.name,
        }
    }
}

/// The UID to store for a device saved by name, when it can be known.
///
/// Pure, so the conversion can be proved without hardware. Only a unique name
/// converts: if two present devices share the name there is no telling which
/// one the person meant, and guessing would silently pick the wrong one. The
/// legacy name keeps resolving until the ambiguity goes away.
pub fn uid_for_legacy_name(
    uid: Option<&str>,
    name: Option<&str>,
    present: &[DeviceRef],
) -> Option<String> {
    if uid.is_some_and(|u| !u.trim().is_empty()) {
        return None;
    }
    let name = name.map(str::trim).filter(|n| !n.is_empty())?;
    let mut matching = present.iter().filter(|d| d.name == name);
    let found = matching.next()?;
    if matching.next().is_some() {
        return None;
    }
    Some(found.id.clone())
}

/// The first device in `devices` that `choice` names.
pub fn find_device(
    devices: impl IntoIterator<Item = cpal::Device>,
    choice: &DeviceChoice,
) -> Option<cpal::Device> {
    devices
        .into_iter()
        .find(|device| device_ref(device).is_some_and(|d| choice.matches(&d)))
}

/// A device's identity and name, or `None` when the system will not give a
/// name (such a device is not offered, as before).
pub fn device_ref(device: &cpal::Device) -> Option<DeviceRef> {
    let name = device.name().ok()?;
    Some(DeviceRef {
        id: device_uid(device),
        name,
    })
}

/// The CoreAudio UID of a cpal device.
pub fn device_uid(device: &cpal::Device) -> String {
    imp::device_uid(device)
}

/// The numeric CoreAudio id of a cpal device, which `say -a` accepts. Not
/// stable across reconnects; only for handing to a command right now.
pub fn device_audio_id(device: &cpal::Device) -> Option<u32> {
    imp::audio_id(device)
}

#[cfg(target_os = "macos")]
pub(crate) use imp::object_id;

#[cfg(target_os = "macos")]
mod imp {
    use core_foundation::base::TCFType;
    use core_foundation::string::{CFString, CFStringRef};
    use coreaudio::sys::{
        kAudioDevicePropertyDeviceUID, kAudioObjectPropertyElementMaster,
        kAudioObjectPropertyScopeGlobal, AudioObjectGetPropertyData, AudioObjectID,
        AudioObjectPropertyAddress,
    };
    use std::ffi::c_void;
    use std::ptr::null;

    /// The CoreAudio id cpal holds for a device.
    pub fn object_id(device: &cpal::Device) -> AudioObjectID {
        let cpal::platform::DeviceInner::CoreAudio(device) = device.as_inner();
        device.audio_device_id()
    }

    pub fn audio_id(device: &cpal::Device) -> Option<u32> {
        Some(object_id(device))
    }

    /// The UID, or `id:<number>` in the case CoreAudio will not say. That
    /// stands for the device until it is next plugged in; it is never a name.
    pub fn device_uid(device: &cpal::Device) -> String {
        let id = object_id(device);
        read_uid(id).unwrap_or_else(|| format!("id:{id}"))
    }

    fn read_uid(id: AudioObjectID) -> Option<String> {
        let address = AudioObjectPropertyAddress {
            mSelector: kAudioDevicePropertyDeviceUID,
            mScope: kAudioObjectPropertyScopeGlobal,
            mElement: kAudioObjectPropertyElementMaster,
        };
        let mut raw: CFStringRef = null();
        let mut size = std::mem::size_of::<CFStringRef>() as u32;
        // SAFETY: `raw` is a pointer-sized slot and `size` says so. CoreAudio
        // writes a retained CFString into it, or leaves it null on failure.
        let status = unsafe {
            AudioObjectGetPropertyData(
                id,
                &address,
                0,
                null(),
                &mut size,
                &mut raw as *mut CFStringRef as *mut c_void,
            )
        };
        if status != 0 || raw.is_null() {
            tracing::warn!("[VoiceDevices] Could not read the UID of audio device {id} ({status})");
            return None;
        }
        // SAFETY: the caller owns the string this property returns, so it is
        // wrapped under the create rule: it is released when `uid` drops.
        let uid = unsafe { CFString::wrap_under_create_rule(raw) };
        Some(uid.to_string())
    }
}

#[cfg(not(target_os = "macos"))]
mod imp {
    use cpal::traits::DeviceTrait;

    pub fn audio_id(_device: &cpal::Device) -> Option<u32> {
        None
    }

    /// There is no CoreAudio here, so the name is the best identity cpal has.
    pub fn device_uid(device: &cpal::Device) -> String {
        device.name().unwrap_or_default()
    }
}

#[cfg(test)]
mod tests {
    use super::{uid_for_legacy_name, DeviceChoice, DeviceRef};

    fn dev(id: &str, name: &str) -> DeviceRef {
        DeviceRef {
            id: id.to_string(),
            name: name.to_string(),
        }
    }

    #[test]
    fn a_uid_choice_matches_by_uid_even_when_the_name_changed() {
        let choice = DeviceChoice {
            uid: Some("uid-a".into()),
            name: "Old Name".into(),
        };
        assert!(choice.matches(&dev("uid-a", "Renamed")));
        assert!(!choice.matches(&dev("uid-b", "Old Name")));
    }

    /// The bug this exists for: two microphones with one name.
    #[test]
    fn two_devices_with_one_name_are_told_apart_by_uid() {
        let choice = DeviceChoice {
            uid: Some("uid-2".into()),
            name: "USB Microphone".into(),
        };
        let first = dev("uid-1", "USB Microphone");
        let second = dev("uid-2", "USB Microphone");
        assert!(!choice.matches(&first));
        assert!(choice.matches(&second));
    }

    #[test]
    fn a_legacy_choice_matches_by_name() {
        let choice = DeviceChoice {
            uid: None,
            name: "Shure MV7".into(),
        };
        assert!(choice.matches(&dev("uid-x", "Shure MV7")));
        assert!(!choice.matches(&dev("uid-x", "Other")));
    }

    #[test]
    fn a_blank_choice_is_no_choice() {
        assert_eq!(DeviceChoice::from_stored(Some("  "), Some("")), None);
        assert_eq!(DeviceChoice::from_stored(None, Some("   ")), None);
        assert_eq!(
            DeviceChoice::from_stored(None, Some(" Shure MV7 ")),
            Some(DeviceChoice {
                uid: None,
                name: "Shure MV7".into()
            })
        );
    }

    #[test]
    fn a_legacy_name_converts_to_the_uid_of_the_one_device_with_it() {
        let present = [
            dev("uid-1", "MacBook Air Microphone"),
            dev("uid-2", "Shure MV7"),
        ];
        assert_eq!(
            uid_for_legacy_name(None, Some("Shure MV7"), &present),
            Some("uid-2".to_string())
        );
    }

    #[test]
    fn a_stored_uid_is_never_replaced() {
        let present = [dev("uid-2", "Shure MV7")];
        assert_eq!(
            uid_for_legacy_name(Some("uid-9"), Some("Shure MV7"), &present),
            None
        );
    }

    #[test]
    fn an_absent_or_blank_legacy_name_does_not_convert() {
        let present = [dev("uid-2", "Shure MV7")];
        assert_eq!(uid_for_legacy_name(None, Some("AirPods"), &present), None);
        assert_eq!(uid_for_legacy_name(None, Some("  "), &present), None);
        assert_eq!(uid_for_legacy_name(None, None, &present), None);
    }

    /// Guessing between two same-named devices would pick the wrong one half
    /// the time, so the name keeps resolving instead.
    #[test]
    fn an_ambiguous_legacy_name_does_not_convert() {
        let present = [
            dev("uid-1", "USB Microphone"),
            dev("uid-2", "USB Microphone"),
        ];
        assert_eq!(
            uid_for_legacy_name(None, Some("USB Microphone"), &present),
            None
        );
    }
}

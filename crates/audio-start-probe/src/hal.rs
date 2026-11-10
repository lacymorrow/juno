//! The two CoreAudio device properties the probe needs and cpal does not
//! expose: a device's nominal sample rate, and the rates it will accept.
//!
//! Reading the rate back after setting it is what makes the format-mismatch
//! row worth anything. Without it the probe would report a mismatch it may
//! never have created — CoreAudio applies a rate change asynchronously, and
//! refuses it outright while another client holds the device running.

use coreaudio::sys::{
    kAudioDevicePropertyAvailableNominalSampleRates, kAudioDevicePropertyNominalSampleRate,
    kAudioObjectPropertyElementMaster, kAudioObjectPropertyScopeGlobal, AudioObjectGetPropertyData,
    AudioObjectGetPropertyDataSize, AudioObjectID, AudioObjectPropertyAddress,
    AudioObjectSetPropertyData, AudioValueRange,
};
use std::ffi::c_void;
use std::time::{Duration, Instant};

/// Two rates count as the same one. CoreAudio carries them as `f64`.
const SAME_RATE: f64 = 0.5;

/// How long to wait for a rate change to show up on a read-back.
const RATE_CHANGE_TIMEOUT: Duration = Duration::from_millis(1000);

/// The CoreAudio id cpal holds for a device.
///
/// Same access path the shipped code uses (`device_identity::object_id`),
/// through the `audio_device_id()` accessor that is a JUNO PATCH on cpal
/// 0.15.3. `DeviceInner` has one variant on macOS, hence the irrefutable let.
pub fn device_id(device: &cpal::Device) -> AudioObjectID {
    let cpal::platform::DeviceInner::CoreAudio(inner) = device.as_inner();
    inner.audio_device_id()
}

fn address(selector: u32) -> AudioObjectPropertyAddress {
    AudioObjectPropertyAddress {
        mSelector: selector,
        mScope: kAudioObjectPropertyScopeGlobal,
        mElement: kAudioObjectPropertyElementMaster,
    }
}

fn check(status: i32, what: &str) -> Result<(), String> {
    if status == 0 {
        Ok(())
    } else {
        Err(format!("could not {what}: OSStatus {status}"))
    }
}

/// The rate the device is running at right now.
pub fn nominal_rate(device: AudioObjectID) -> Result<f64, String> {
    let address = address(kAudioDevicePropertyNominalSampleRate);
    let mut rate: f64 = 0.0;
    let mut size = std::mem::size_of::<f64>() as u32;
    let status = unsafe {
        AudioObjectGetPropertyData(
            device,
            &address,
            0,
            std::ptr::null(),
            &mut size,
            (&mut rate as *mut f64).cast::<c_void>(),
        )
    };
    check(status, "read the device's sample rate")?;
    Ok(rate)
}

/// The discrete rates the device accepts.
///
/// CoreAudio reports ranges; a device that names a true range rather than a
/// list of points is dropped here, because cpal only ever sets a rate it
/// found as a zero-width range (`set_sample_rate`, macos/mod.rs:800), so a
/// rate from inside a wide range is not one the shipped code could pick.
pub fn available_rates(device: AudioObjectID) -> Result<Vec<f64>, String> {
    let address = address(kAudioDevicePropertyAvailableNominalSampleRates);
    let mut size = 0u32;
    let status =
        unsafe { AudioObjectGetPropertyDataSize(device, &address, 0, std::ptr::null(), &mut size) };
    check(status, "size the device's rate list")?;

    let count = size as usize / std::mem::size_of::<AudioValueRange>();
    let mut ranges = vec![
        AudioValueRange {
            mMinimum: 0.0,
            mMaximum: 0.0,
        };
        count
    ];
    let status = unsafe {
        AudioObjectGetPropertyData(
            device,
            &address,
            0,
            std::ptr::null(),
            &mut size,
            ranges.as_mut_ptr().cast::<c_void>(),
        )
    };
    check(status, "read the device's rate list")?;

    Ok(ranges
        .iter()
        .filter(|range| (range.mMinimum - range.mMaximum).abs() < SAME_RATE)
        .map(|range| range.mMinimum)
        .collect())
}

/// Move the device to `rate`, and report the rate it is actually on
/// afterwards — which is not always the one asked for.
pub fn set_nominal_rate(device: AudioObjectID, rate: f64) -> Result<f64, String> {
    let current = nominal_rate(device)?;
    if (current - rate).abs() < SAME_RATE {
        return Ok(current);
    }

    let address = address(kAudioDevicePropertyNominalSampleRate);
    let status = unsafe {
        AudioObjectSetPropertyData(
            device,
            &address,
            0,
            std::ptr::null(),
            std::mem::size_of::<f64>() as u32,
            (&rate as *const f64).cast::<c_void>(),
        )
    };
    check(status, "set the device's sample rate")?;

    // The change lands asynchronously: the read-back reports the old rate for
    // a few milliseconds after a successful set.
    let deadline = Instant::now() + RATE_CHANGE_TIMEOUT;
    loop {
        let now = nominal_rate(device)?;
        if (now - rate).abs() < SAME_RATE || Instant::now() >= deadline {
            return Ok(now);
        }
        std::thread::sleep(Duration::from_millis(5));
    }
}

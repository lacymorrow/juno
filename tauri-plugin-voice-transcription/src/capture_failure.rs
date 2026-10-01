//! Why the microphone never started, in words a person can act on.
//!
//! The capture threads used to end every failure the same way: one
//! `tracing::error!` and a bare `return`. The thread was gone, nothing was
//! listening, and the settings window went on showing always-listening as on.
//! That is the dead-control pattern this codebase keeps producing: a switch
//! disconnected from the thing it names.
//!
//! So the failures are an enum instead of a log line. [`Self::code`] and
//! [`Self::message`] match on it exhaustively, which means a new exit path
//! cannot be added without writing the sentence the person reads, and
//! `variant_index` plus the test below mean it cannot be added without being
//! covered by a test either.

use serde_json::json;
use tauri::{Emitter, Runtime};

/// Every way starting microphone capture can fail.
///
/// `detail` fields hold the underlying library error. They go to the log, never
/// to the screen: "BuildStreamError(DeviceNotAvailable)" is not something to
/// show someone who wanted to dictate.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CaptureStartFailure {
    /// The machine reports no microphone at all.
    NoInputDevice,
    /// The chosen microphone is gone and there is no other one to use.
    ChosenDeviceGone { requested: String },
    /// The device is present but will not say what it can record.
    DeviceConfigUnavailable { device: String, detail: String },
    /// The device records in a sample format Juno cannot read.
    UnsupportedSampleFormat { device: String, format: String },
    /// Opening the stream failed. On macOS this is usually microphone access.
    StreamBuildFailed { device: String, detail: String },
    /// The stream opened and then refused to run.
    StreamStartFailed { device: String, detail: String },
    /// The speech engine had no session to transcribe into.
    EngineUnavailable { detail: String },
}

impl CaptureStartFailure {
    /// A stable tag for the UI and the logs. Never shown to a person.
    pub fn code(&self) -> &'static str {
        match self {
            Self::NoInputDevice => "no_input_device",
            Self::ChosenDeviceGone { .. } => "chosen_device_gone",
            Self::DeviceConfigUnavailable { .. } => "device_config_unavailable",
            Self::UnsupportedSampleFormat { .. } => "unsupported_sample_format",
            Self::StreamBuildFailed { .. } => "stream_build_failed",
            Self::StreamStartFailed { .. } => "stream_start_failed",
            Self::EngineUnavailable { .. } => "engine_unavailable",
        }
    }

    /// One sentence for the person, naming the device and what to do next.
    pub fn message(&self) -> String {
        match self {
            Self::NoInputDevice => {
                "Juno could not find a microphone. Connect one, then try again.".to_string()
            }
            Self::ChosenDeviceGone { requested } => format!(
                "{requested} is not connected any more, and Juno found no other microphone to use."
            ),
            Self::DeviceConfigUnavailable { device, .. } => format!(
                "{device} did not say what it can record, so Juno could not listen. Pick another microphone in Audio settings."
            ),
            Self::UnsupportedSampleFormat { device, .. } => format!(
                "{device} records in a format Juno cannot read. Pick another microphone in Audio settings."
            ),
            Self::StreamBuildFailed { device, .. } => format!(
                "Juno could not open {device}. Check that Juno is allowed to use the microphone in System Settings, under Privacy and Security."
            ),
            Self::StreamStartFailed { device, .. } => format!(
                "{device} would not start recording. Unplug it and plug it back in, or pick another microphone in Audio settings."
            ),
            Self::EngineUnavailable { .. } => {
                "Juno's speech engine is still loading, so there was nothing to listen with. Try again in a moment.".to_string()
            }
        }
    }

    /// The library error behind the failure, for the log only.
    pub fn detail(&self) -> Option<&str> {
        match self {
            Self::NoInputDevice | Self::ChosenDeviceGone { .. } => None,
            Self::DeviceConfigUnavailable { detail, .. }
            | Self::StreamBuildFailed { detail, .. }
            | Self::StreamStartFailed { detail, .. }
            | Self::EngineUnavailable { detail } => Some(detail),
            Self::UnsupportedSampleFormat { format, .. } => Some(format),
        }
    }

    /// The microphone this failure is about, when it is about one.
    pub fn device(&self) -> Option<&str> {
        match self {
            Self::NoInputDevice | Self::EngineUnavailable { .. } => None,
            Self::ChosenDeviceGone { requested } => Some(requested),
            Self::DeviceConfigUnavailable { device, .. }
            | Self::UnsupportedSampleFormat { device, .. }
            | Self::StreamBuildFailed { device, .. }
            | Self::StreamStartFailed { device, .. } => Some(device),
        }
    }

    /// The event payload. `listening: false` is the whole point: whatever the
    /// UI last heard, this says the microphone is not open.
    pub fn payload(&self) -> serde_json::Value {
        json!({
            "code": self.code(),
            "message": self.message(),
            "device": self.device(),
            "listening": false,
        })
    }

    /// A stable index per variant, used only to prove the test below covers
    /// every one of them. Exhaustive on purpose: a new variant has to be given
    /// an index here, and then `VARIANT_COUNT` and the test's sample list have
    /// to grow with it.
    #[cfg(test)]
    fn variant_index(&self) -> usize {
        match self {
            Self::NoInputDevice => 0,
            Self::ChosenDeviceGone { .. } => 1,
            Self::DeviceConfigUnavailable { .. } => 2,
            Self::UnsupportedSampleFormat { .. } => 3,
            Self::StreamBuildFailed { .. } => 4,
            Self::StreamStartFailed { .. } => 5,
            Self::EngineUnavailable { .. } => 6,
        }
    }

    #[cfg(test)]
    const VARIANT_COUNT: usize = 7;
}

/// Say out loud that capture stopped, and that nothing is listening.
///
/// This is the only way a capture thread is allowed to give up. The log line
/// keeps the library detail; the event carries the sentence and
/// `listening: false`, so the app can put the switch back where it belongs and
/// the person can read why.
pub fn report<R: Runtime>(app_handle: &tauri::AppHandle<R>, failure: &CaptureStartFailure) {
    match failure.detail() {
        Some(detail) => tracing::error!(
            "[VoiceCapture] {} ({}): {}",
            failure.message(),
            failure.code(),
            detail
        ),
        None => tracing::error!("[VoiceCapture] {} ({})", failure.message(), failure.code()),
    }

    if let Err(e) = app_handle.emit(crate::constants::voice_capture::FAILED, failure.payload()) {
        tracing::error!("[VoiceCapture] Could not report the failure to the UI: {e}");
    }
}

#[cfg(test)]
mod tests {
    use super::CaptureStartFailure as F;
    use std::collections::HashSet;

    /// One of every variant. The test below fails if a variant is missing.
    fn every_failure() -> Vec<F> {
        vec![
            F::NoInputDevice,
            F::ChosenDeviceGone {
                requested: "Shure MV7".to_string(),
            },
            F::DeviceConfigUnavailable {
                device: "Shure MV7".to_string(),
                detail: "DeviceNotAvailable".to_string(),
            },
            F::UnsupportedSampleFormat {
                device: "Shure MV7".to_string(),
                format: "U16".to_string(),
            },
            F::StreamBuildFailed {
                device: "Shure MV7".to_string(),
                detail: "BuildStreamError(DeviceNotAvailable)".to_string(),
            },
            F::StreamStartFailed {
                device: "Shure MV7".to_string(),
                detail: "PlayStreamError(DeviceNotAvailable)".to_string(),
            },
            F::EngineUnavailable {
                detail: "model still loading".to_string(),
            },
        ]
    }

    /// The pin. A new way for capture to give up has to appear here, which
    /// means it has to have a message and a code of its own.
    #[test]
    fn every_way_capture_can_fail_is_covered() {
        let indices: HashSet<usize> = every_failure().iter().map(F::variant_index).collect();
        assert_eq!(
            indices.len(),
            F::VARIANT_COUNT,
            "a capture failure variant is missing from every_failure()"
        );
        for index in 0..F::VARIANT_COUNT {
            assert!(indices.contains(&index), "variant {index} is not covered");
        }
    }

    #[test]
    fn no_two_failures_share_a_code() {
        let codes: HashSet<&str> = every_failure().iter().map(F::code).collect();
        assert_eq!(codes.len(), F::VARIANT_COUNT, "two failures share a code");
    }

    /// Every failure says that nothing is listening. This is the bug the whole
    /// module exists for: the UI kept claiming Juno was listening because
    /// nothing ever told it otherwise.
    #[test]
    fn every_failure_says_nothing_is_listening() {
        for failure in every_failure() {
            let payload = failure.payload();
            assert_eq!(
                payload["listening"],
                serde_json::Value::Bool(false),
                "{} did not report listening: false",
                failure.code()
            );
            assert_eq!(payload["code"], failure.code());
        }
    }

    /// A message is for a person, so it is a sentence, and it never leaks the
    /// library's words.
    #[test]
    fn every_message_is_plain_english() {
        for failure in every_failure() {
            let message = failure.message();
            assert!(!message.is_empty(), "{} has no message", failure.code());
            assert!(
                message.ends_with('.'),
                "{} is not a sentence: {message}",
                failure.code()
            );
            assert!(
                !message.contains('—'),
                "{} uses an em dash: {message}",
                failure.code()
            );
            for leak in ["cpal", "Err(", "Error", "None", "{", "unwrap"] {
                assert!(
                    !message.contains(leak),
                    "{} leaks '{leak}' into the UI: {message}",
                    failure.code()
                );
            }
        }
    }

    /// The device name is what makes the message actionable, so a failure
    /// about a specific microphone names it.
    #[test]
    fn a_failure_about_a_device_names_it() {
        for failure in every_failure() {
            if let Some(device) = failure.device() {
                assert!(
                    failure.message().contains(device),
                    "{} does not name {device}",
                    failure.code()
                );
            }
        }
    }
}

//! Grammar and punctuation pass for dictation, on Apple's on-device model.
//!
//! The words a person dictates are typed into another app. Before that, when
//! the Mac can run Apple's Foundation Models (Apple Silicon, macOS 26, Apple
//! Intelligence on), the transcript is handed to the model with a narrow
//! instruction: fix grammar, punctuation and capitalization, drop filler and
//! false starts, change nothing else. There is no download and no account.
//!
//! The pass is an improvement and never a dependency. Every way it can fail
//! (no model, a timeout, a refusal, an empty or wildly different answer)
//! ends the same way: the transcript as heard is typed, and the person is
//! told nothing. One log line per dictation says what happened:
//! `[DictationCleanup] in N chars, out M chars, X ms, outcome=...`.
//!
//! Only dictation goes through here (`commands::dictation::insert_dictation_text`).
//! A voice turn for the agent is a request, not prose to polish, and never does.
//!
//! The model is reached through a small Swift bridge (`swift/DictationCleanup.swift`,
//! built by `build.rs`) that links FoundationModels weakly so the app still
//! launches on macOS 12.3 to 25 and on Intel.

use crate::settings::manager::SettingsManager;
use serde::Serialize;
use std::time::{Duration, Instant};
use tauri::AppHandle;
use tracing::{info, warn};

/// How long generation may take before the raw transcript is typed instead.
///
/// The model is warmed when the key goes down, so release pays only
/// generation: roughly 20 to 55 tokens for a 15 to 40 word dictation, which
/// the M1 produces in about 0.5 to 1.2 s. 1.5 s covers that with margin and is
/// also about the longest a person waits between letting go of the key and
/// seeing words before it feels like Juno hung. A miss costs nothing but the
/// improvement, so the budget leans toward speed.
pub const CLEANUP_TIMEOUT_MS: i32 = 1500;

/// Utterances this short have nothing to fix, and a model asked to "clean up"
/// two words is more likely to answer them than polish them.
const MIN_WORDS: usize = 3;

/// Longer than this and generation cannot finish inside the budget; the
/// transcript is typed as heard rather than waiting to find out.
const MAX_WORDS: usize = 120;

/// The cleaned text must stay within this share of the original's word count.
/// A model that answers, summarizes or rambles falls outside it.
const MIN_WORD_RATIO: f32 = 0.5;
const MAX_WORD_RATIO: f32 = 1.5;

/// Can this Mac run the cleanup, and if not, why not.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Availability {
    Available,
    /// Hardware that cannot run Apple Intelligence (Intel, older Apple Silicon
    /// without support).
    DeviceNotEligible,
    /// The Mac could run it; the person has not turned Apple Intelligence on.
    AppleIntelligenceOff,
    /// Apple Intelligence is on and its model is still downloading.
    ModelNotReady,
    /// macOS before 26, an Intel build, or an SDK without the framework.
    Unsupported,
}

impl Availability {
    fn from_code(code: i32) -> Self {
        match code {
            0 => Availability::Available,
            1 => Availability::DeviceNotEligible,
            2 => Availability::AppleIntelligenceOff,
            3 => Availability::ModelNotReady,
            _ => Availability::Unsupported,
        }
    }
}

/// What the settings screen draws.
#[derive(Debug, Clone, Serialize)]
pub struct CleanupStatus {
    /// The saved toggle.
    pub enabled: bool,
    pub availability: Availability,
    /// One line for the person when the toggle is disabled; `None` when it
    /// can be used.
    pub reason: Option<String>,
    /// True when System Settings can fix it (Apple Intelligence is off).
    pub can_open_settings: bool,
}

impl CleanupStatus {
    pub fn new(enabled: bool, availability: Availability) -> Self {
        let reason = match availability {
            Availability::Available => None,
            Availability::DeviceNotEligible => {
                Some("This Mac can't run Apple's on-device model.".to_string())
            }
            Availability::AppleIntelligenceOff => {
                Some("Turn on Apple Intelligence in System Settings to use this.".to_string())
            }
            Availability::ModelNotReady => {
                Some("Apple Intelligence is still getting ready. Try again soon.".to_string())
            }
            Availability::Unsupported => {
                Some("Needs macOS 26 on a Mac with Apple silicon.".to_string())
            }
        };
        Self {
            enabled,
            availability,
            reason,
            can_open_settings: availability == Availability::AppleIntelligenceOff,
        }
    }
}

/// Why a dictation was or was not cleaned. The log line carries this verbatim.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Outcome {
    Cleaned,
    Timeout,
    Unavailable,
    Refused,
    Rejected,
    Skipped,
}

impl Outcome {
    pub fn as_str(self) -> &'static str {
        match self {
            Outcome::Cleaned => "cleaned",
            Outcome::Timeout => "timeout",
            Outcome::Unavailable => "unavailable",
            Outcome::Refused => "refused",
            Outcome::Rejected => "rejected",
            Outcome::Skipped => "skipped",
        }
    }
}

#[cfg(target_os = "macos")]
mod ffi {
    use std::os::raw::c_char;
    extern "C" {
        pub fn juno_fm_availability() -> i32;
        pub fn juno_fm_prewarm();
        pub fn juno_fm_cleanup(
            text: *const c_char,
            timeout_ms: i32,
            status: *mut i32,
        ) -> *mut c_char;
        pub fn juno_fm_free(ptr: *mut c_char);
    }
}

/// Ask the system model whether it can run. Blocking; call off the async
/// runtime.
pub fn availability() -> Availability {
    #[cfg(target_os = "macos")]
    {
        // SAFETY: no arguments, no pointers; the Swift side only reads
        // SystemLanguageModel.default.availability.
        Availability::from_code(unsafe { ffi::juno_fm_availability() })
    }
    #[cfg(not(target_os = "macos"))]
    {
        Availability::Unsupported
    }
}

/// The saved toggle, honoured only while Beta features is on: cleanup is a beta
/// feature and its row is hidden otherwise. Unreadable settings count as the
/// default (off).
pub async fn is_enabled(app_handle: &AppHandle) -> bool {
    if !crate::settings::beta_features_active(app_handle) {
        return false;
    }
    let settings = match SettingsManager::new(app_handle.clone()) {
        Ok(manager) => manager.get_audio_settings().await.ok(),
        Err(_) => None,
    };
    settings
        .map(|audio| audio.dictation_cleanup)
        .unwrap_or(crate::constants::settings::defaults::DICTATION_CLEANUP)
}

/// Warm the model for the dictation that just started. Returns at once; the
/// work runs on a blocking thread. Does nothing when the setting is off or the
/// model cannot run.
pub fn prewarm(app_handle: &AppHandle) {
    let app_handle = app_handle.clone();
    tauri::async_runtime::spawn(async move {
        if !is_enabled(&app_handle).await {
            return;
        }
        let _ = tokio::task::spawn_blocking(prewarm_blocking).await;
    });
}

fn prewarm_blocking() {
    #[cfg(target_os = "macos")]
    {
        // SAFETY: no arguments; the Swift side checks availability itself.
        unsafe { ffi::juno_fm_prewarm() }
    }
}

/// Open System Settings on Apple Intelligence & Siri.
pub fn open_settings_pane() -> Result<(), String> {
    #[cfg(target_os = "macos")]
    {
        std::process::Command::new("open")
            .arg("x-apple.systempreferences:com.apple.Siri-Settings.extension")
            .spawn()
            .map(|_| ())
            .map_err(|e| format!("Could not open System Settings: {}", e))
    }
    #[cfg(not(target_os = "macos"))]
    {
        Err("Only available on macOS".to_string())
    }
}

/// The text to type for a transcript: pause ellipses removed always, then the
/// grammar pass when it is on and can run. Never fails; the worst case is the
/// ellipsis-stripped transcript.
pub async fn prepare(app_handle: &AppHandle, text: &str) -> String {
    let stripped = tauri_plugin_voice_transcription::utils::strip_pause_ellipses(text);
    let stripped = stripped.trim().to_string();

    let started = Instant::now();
    let (result, outcome) = if !is_enabled(app_handle).await {
        (stripped.clone(), Outcome::Skipped)
    } else {
        clean(&stripped).await
    };
    log_outcome(&stripped, &result, started.elapsed(), outcome);
    result
}

/// Run the model over an already ellipsis-stripped transcript. Returns what to
/// type and why.
async fn clean(text: &str) -> (String, Outcome) {
    if !worth_cleaning(text) {
        return (text.to_string(), Outcome::Skipped);
    }
    let owned = text.to_string();
    // The bridge enforces the timeout itself; this outer one only guards
    // against the bridge not coming back at all.
    let guard = Duration::from_millis(CLEANUP_TIMEOUT_MS as u64 + 500);
    let job = tokio::task::spawn_blocking(move || run_model(&owned));
    let raw = match tokio::time::timeout(guard, job).await {
        Ok(Ok(raw)) => raw,
        Ok(Err(e)) => {
            warn!("[DictationCleanup] worker failed: {}", e);
            return (text.to_string(), Outcome::Unavailable);
        }
        Err(_) => return (text.to_string(), Outcome::Timeout),
    };
    match raw {
        ModelResult::Text(out) => match accept_output(text, &out) {
            Some(cleaned) => (cleaned, Outcome::Cleaned),
            None => (text.to_string(), Outcome::Rejected),
        },
        ModelResult::Failed(outcome) => (text.to_string(), outcome),
    }
}

enum ModelResult {
    Text(String),
    Failed(Outcome),
}

fn run_model(text: &str) -> ModelResult {
    #[cfg(target_os = "macos")]
    {
        let c_text = match std::ffi::CString::new(text) {
            Ok(c) => c,
            // A NUL inside a transcript is not something to hand to C.
            Err(_) => return ModelResult::Failed(Outcome::Rejected),
        };
        let mut status: i32 = 3;
        // SAFETY: `c_text` is a valid NUL-terminated string for the call;
        // `status` is a valid out pointer; a non-null return is a malloc'd
        // UTF-8 string that is copied and then freed through the bridge.
        let ptr = unsafe { ffi::juno_fm_cleanup(c_text.as_ptr(), CLEANUP_TIMEOUT_MS, &mut status) };
        if ptr.is_null() {
            return ModelResult::Failed(match status {
                1 => Outcome::Timeout,
                2 => Outcome::Refused,
                4 => Outcome::Unavailable,
                _ => Outcome::Rejected,
            });
        }
        // SAFETY: non-null, NUL-terminated, owned by us until freed below.
        let out = unsafe { std::ffi::CStr::from_ptr(ptr) }
            .to_string_lossy()
            .into_owned();
        // SAFETY: `ptr` came from `juno_fm_cleanup` and is freed exactly once.
        unsafe { ffi::juno_fm_free(ptr) };
        ModelResult::Text(out)
    }
    #[cfg(not(target_os = "macos"))]
    {
        let _ = text;
        ModelResult::Failed(Outcome::Unavailable)
    }
}

fn log_outcome(input: &str, output: &str, elapsed: Duration, outcome: Outcome) {
    info!(
        "[DictationCleanup] in {} chars, out {} chars, {} ms, outcome={}",
        input.chars().count(),
        output.chars().count(),
        elapsed.as_millis(),
        outcome.as_str()
    );
}

fn word_count(text: &str) -> usize {
    text.split_whitespace().count()
}

/// Is a transcript of this size worth a model call?
fn worth_cleaning(text: &str) -> bool {
    let words = word_count(text);
    (MIN_WORDS..=MAX_WORDS).contains(&words)
}

/// Take the model's answer only when it still looks like the same utterance
/// cleaned up. Returns the text to type, or `None` to fall back to the input.
fn accept_output(input: &str, output: &str) -> Option<String> {
    let mut out = output.trim();
    // A model sometimes echoes the delimiters it was given.
    for tag in ["<transcript>", "</transcript>"] {
        out = out.trim_start_matches(tag).trim_end_matches(tag).trim();
    }
    // Wrapping quotes the speaker did not say.
    let input_quoted = input.trim().starts_with('"');
    if !input_quoted {
        out = out.trim_matches('"').trim();
    }
    if out.is_empty() {
        return None;
    }
    let before = word_count(input) as f32;
    let after = word_count(out) as f32;
    if before == 0.0 {
        return None;
    }
    let ratio = after / before;
    if !(MIN_WORD_RATIO..=MAX_WORD_RATIO).contains(&ratio) {
        return None;
    }
    // One utterance in, one line out.
    if out.contains('\n') && !input.contains('\n') {
        return None;
    }
    Some(out.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn one_and_two_word_utterances_are_left_alone() {
        assert!(!worth_cleaning("yes"));
        assert!(!worth_cleaning("sounds good"));
        assert!(worth_cleaning("that sounds good"));
    }

    #[test]
    fn very_long_dictation_is_left_alone() {
        let long = "word ".repeat(MAX_WORDS + 1);
        assert!(!worth_cleaning(&long));
        let ok = "word ".repeat(MAX_WORDS);
        assert!(worth_cleaning(&ok));
    }

    #[test]
    fn a_tidied_sentence_is_accepted() {
        assert_eq!(
            accept_output(
                "um so i think we should uh ship it friday",
                "I think we should ship it Friday."
            ),
            Some("I think we should ship it Friday.".to_string())
        );
    }

    #[test]
    fn an_empty_answer_is_rejected() {
        assert_eq!(accept_output("ship it on friday ok", "   "), None);
    }

    #[test]
    fn an_answer_that_is_far_longer_is_rejected() {
        let input = "what is the capital of france";
        let answer = "The capital of France is Paris, a city known for the Eiffel Tower and more.";
        assert_eq!(accept_output(input, answer), None);
    }

    #[test]
    fn an_answer_that_is_far_shorter_is_rejected() {
        assert_eq!(
            accept_output("please send the report to the whole team today", "Sure."),
            None
        );
    }

    #[test]
    fn echoed_delimiters_and_wrapping_quotes_are_removed() {
        assert_eq!(
            accept_output(
                "ship it on friday ok",
                "<transcript>\n\"Ship it on Friday, OK.\"\n</transcript>"
            ),
            Some("Ship it on Friday, OK.".to_string())
        );
    }

    #[test]
    fn a_multi_line_answer_to_a_one_line_utterance_is_rejected() {
        assert_eq!(
            accept_output("ship it on friday ok", "Ship it.\nOn Friday.\nOK."),
            None
        );
    }

    #[test]
    fn status_explains_each_unavailable_reason() {
        let ok = CleanupStatus::new(true, Availability::Available);
        assert!(ok.reason.is_none() && !ok.can_open_settings);

        let off = CleanupStatus::new(true, Availability::AppleIntelligenceOff);
        assert!(off.reason.is_some() && off.can_open_settings);

        for a in [
            Availability::DeviceNotEligible,
            Availability::ModelNotReady,
            Availability::Unsupported,
        ] {
            let s = CleanupStatus::new(true, a);
            assert!(s.reason.is_some() && !s.can_open_settings);
        }
    }

    #[test]
    fn availability_codes_map_as_the_bridge_documents() {
        assert_eq!(Availability::from_code(0), Availability::Available);
        assert_eq!(Availability::from_code(1), Availability::DeviceNotEligible);
        assert_eq!(
            Availability::from_code(2),
            Availability::AppleIntelligenceOff
        );
        assert_eq!(Availability::from_code(3), Availability::ModelNotReady);
        assert_eq!(Availability::from_code(4), Availability::Unsupported);
        assert_eq!(Availability::from_code(99), Availability::Unsupported);
    }

    #[test]
    fn outcome_names_match_the_log_contract() {
        let names: Vec<&str> = [
            Outcome::Cleaned,
            Outcome::Timeout,
            Outcome::Unavailable,
            Outcome::Refused,
            Outcome::Rejected,
            Outcome::Skipped,
        ]
        .iter()
        .map(|o| o.as_str())
        .collect();
        assert_eq!(
            names,
            [
                "cleaned",
                "timeout",
                "unavailable",
                "refused",
                "rejected",
                "skipped"
            ]
        );
    }
}

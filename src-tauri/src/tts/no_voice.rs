//! Saying why nothing was said.
//!
//! `tts::NO_VOICE` means no engine was allowed to speak a sentence: the Mac's
//! own voice is positively not a Siri voice, so Juno will not use it, and
//! nothing else could stand in. It used to be returned, swallowed with a
//! comment and dropped (LAC-8231): nothing emitted, nothing at the person's
//! level in the log, no consumer anywhere in the frontend.
//!
//! That was survivable while Kokoro was the default engine, because its chain
//! is non-empty even with the Mac disallowed, so reaching the silence took
//! picking the Mac's voice by hand and reading the picker row while doing it.
//! LAC-7987 made the Mac's voice the default on macOS, so anyone whose Read &
//! Speak voice is Alex, Samantha or any Vocalizer voice, with no Kokoro model
//! on disk, now gets there having chosen nothing. Juno is a voice tool. Total
//! mute with no signal reads as a broken install.
//!
//! **The notification is the surface, and it is not an arbitrary choice.**
//! Every window in `tauri.conf.json` is declared `visible: false`, so a toast
//! in the main window is drawn into a window that is usually not on screen;
//! the bar window is on screen but is transparent with Rust-owned hit regions,
//! so a card outside the bar's own footprint is clipped and clicks through.
//! A macOS notification needs no window, and is the channel the scheduler, the
//! timers and a finished turn already use.
//!
//! The durable half already existed and is left alone: the Mac's row in
//! Settings > Voice reads "Pick a Siri voice in Read & Speak and Juno will use
//! it", draws itself as a row that does not speak, and carries the link that
//! opens the pane (`voices::READ_AND_SPEAK_LINK`).

use crate::commands::notifications;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::OnceLock;
use tauri::AppHandle;
use tracing::warn;

/// What the notification says happened, in the person's terms: they noticed
/// silence, not a status string. The cause lives here so the body can be the
/// thing to do about it.
pub const NO_VOICE_TITLE: &str = "Juno has no voice to speak with";

/// What to change, and where. One sentence, one action.
///
/// It is **one** action on purpose. The other way out (choose an engine that
/// is not the Mac's) already has a home in Settings > Voice, and someone who
/// never picked an engine is not the person to send engine shopping from a
/// banner. It is also the action rather than the cause because macOS gives a
/// banner body about two lines: a cause-first sentence truncates away the
/// only part that helps.
///
/// The wording is the picker's, not a second dialect of it: the Mac's row in
/// Settings > Voice reads "Pick a Siri voice in Read & Speak and Juno will
/// use it". The path is spelled out because the pane is two levels into
/// System Settings and nothing in Juno names it anywhere else.
pub const NO_VOICE_BODY: &str =
    "Pick a Siri voice in System Settings > Accessibility > Read & Speak.";

/// Said once per run. A reply is spoken a sentence at a time, so the silence
/// happens once per sentence; a notification per sentence is its own defect.
static TOLD: AtomicBool = AtomicBool::new(false);

/// Bound at launch. The playback path holds an `AppState` and no `AppHandle`,
/// and a notification needs one.
static APP: OnceLock<AppHandle> = OnceLock::new();

/// Remember the handle the silence will be reported through.
pub fn remember_app(app: &AppHandle) {
    let _ = APP.set(app.clone());
}

/// Whether this silence is the one the person hears about. Pure, so the
/// once-per-run rule is testable without a window or a notification.
fn is_first_silence(told: &AtomicBool) -> bool {
    !told.swap(true, Ordering::SeqCst)
}

/// Tell the person that nothing could speak, once per run.
///
/// Called from the one arm that turns `NO_VOICE` into an `Ok`, so a fourth
/// way to reach the silence is covered by construction.
pub fn report_silence() {
    if !is_first_silence(&TOLD) {
        return;
    }
    warn!("[TTS] {NO_VOICE_TITLE}: {NO_VOICE_BODY}");
    match APP.get() {
        Some(app) => notifications::notify(app, NO_VOICE_TITLE, NO_VOICE_BODY),
        // Before the handle is bound (headless, tests) the log is the signal.
        None => warn!("[TTS] No app handle; the silence was logged and not shown"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_silence_is_reported_once_per_run() {
        let told = AtomicBool::new(false);
        assert!(
            is_first_silence(&told),
            "the first sentence nothing could speak is the one that tells the person"
        );
        assert!(
            !is_first_silence(&told),
            "a notification per sentence is its own defect"
        );
        assert!(!is_first_silence(&told));
    }

    #[test]
    fn the_notice_names_the_setting_and_what_to_change() {
        assert!(
            NO_VOICE_BODY.contains("Read & Speak"),
            "the pane the System Voice is chosen in, by the name it has there"
        );
        assert!(
            NO_VOICE_BODY.contains("Siri"),
            "what the voice has to be for Juno to use it"
        );
        assert!(
            NO_VOICE_BODY.contains("System Settings > Accessibility"),
            "the pane is two levels in and nothing else in Juno names the path"
        );
    }

    /// macOS gives a banner body about two lines. Past that it truncates, and
    /// what truncates is the end, which is why the body is the action and not
    /// the cause. The ceiling is deliberately loose: it catches a paragraph
    /// growing back in here, not a word.
    #[test]
    fn the_notice_fits_in_a_banner() {
        assert!(
            NO_VOICE_BODY.chars().count() <= 100,
            "{} chars is past what a banner shows: {NO_VOICE_BODY}",
            NO_VOICE_BODY.chars().count()
        );
        assert!(
            NO_VOICE_TITLE.chars().count() <= 40,
            "a title that long is cut mid-word: {NO_VOICE_TITLE}"
        );
    }

    /// The notice is worth nothing if the arm it hangs off stops calling it,
    /// and that arm cannot be reached from a unit test: `play_rendered` wants
    /// an `AppState`, and the three ways to produce `NO_VOICE` want a Mac with
    /// a non-Siri System Voice. So the wiring is pinned in the source.
    ///
    /// The `expect` is the tripwire: if the arm is renamed or removed this
    /// test fails rather than scanning nothing and passing.
    #[test]
    fn the_no_voice_arm_reports_the_silence() {
        let source = include_str!("mod.rs");
        let (_, after) = source
            .split_once("result == NO_VOICE")
            .expect("play_rendered still has the arm that turns NO_VOICE into an Ok");
        let arm = after.split_once("} else if").map_or(after, |(arm, _)| arm);
        assert!(
            arm.contains("no_voice::report_silence()"),
            "the NO_VOICE arm has to tell the person, not just return Ok: {arm}"
        );
    }
}

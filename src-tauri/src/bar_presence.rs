//! The bar comes back for the work, and leaves again after it.
//!
//! Someone who put the bar away (Escape on the idle bar) and then dictates or
//! talks to Juno still needs to see that it is listening, transcribing and
//! answering. Before, a hidden bar stayed hidden through all of it: the only
//! sign of a dictation was the sound, and a reply was a voice with nothing on
//! screen. Dictation tools that live out of sight (Superwhisper, Wispr Flow)
//! all do the same thing here: appear for the recording, go once it is done.
//!
//! So the start of voice work shows a hidden bar, without focus, and notes
//! that it did. When the bar comes to rest afterwards it is put away again,
//! after a pause sized to the work: a dictation's text is already in the app,
//! so the bar goes almost at once; a reply stays long enough to be read. Only
//! a bar this module showed is ever hidden by it, and anything new in between
//! (more work, the person opening the composer) keeps it up.

use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Mutex;
use std::time::Duration;
use tauri::AppHandle;
use tracing::info;

use crate::constants::ui::window_labels;
use crate::state::VoiceTarget;
use crate::window_management::WindowManager;

/// The bar was off screen when the current work began, and this put it up.
static SHOWN_FOR_WORK: AtomicBool = AtomicBool::new(false);
/// Bumped by every start and every rest, so a pending hide from an earlier
/// rest never lands on later work.
static GENERATION: AtomicU64 = AtomicU64::new(0);
/// What the work was, which decides how long the bar stays after it.
static LAST_WORK: Mutex<Option<VoiceTarget>> = Mutex::new(None);

/// How long the bar stays once the work is over. A dictation's result is the
/// text it typed, elsewhere; a reply is on the bar, and has to be read.
fn linger_after(work: Option<VoiceTarget>) -> Duration {
    match work {
        Some(VoiceTarget::Dictation) => Duration::from_millis(600),
        Some(VoiceTarget::Agent) | None => Duration::from_secs(4),
    }
}

/// Voice work for `target` is starting: show the bar if it is hidden.
pub fn show_for_work(app: &AppHandle, target: VoiceTarget) {
    GENERATION.fetch_add(1, Ordering::SeqCst);
    if let Ok(mut last) = LAST_WORK.lock() {
        // A dictation inside a reply does not shorten the reply's stay.
        if *last != Some(VoiceTarget::Agent) || !SHOWN_FOR_WORK.load(Ordering::SeqCst) {
            *last = Some(target);
        }
    }
    if crate::window_management::onboarding_is_open(app)
        || WindowManager::is_window_visible(app, window_labels::FLOATING_BAR)
    {
        return;
    }
    info!("[BarPresence] The bar was hidden; showing it for this {target:?} work");
    SHOWN_FOR_WORK.store(true, Ordering::SeqCst);
    crate::window_management::show_bar(app);
}

/// The bar just came to rest. If it was shown for the work that ended, put it
/// away after that work's pause, unless something else has begun by then.
pub fn bar_at_rest(app: &AppHandle) {
    if !SHOWN_FOR_WORK.load(Ordering::SeqCst) {
        return;
    }
    let generation = GENERATION.fetch_add(1, Ordering::SeqCst) + 1;
    let linger = linger_after(LAST_WORK.lock().ok().and_then(|last| *last));
    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        tokio::time::sleep(linger).await;
        if GENERATION.load(Ordering::SeqCst) != generation {
            return;
        }
        if !crate::commands::ui_commands::bar_is_resting().await {
            return;
        }
        info!("[BarPresence] Work is over; putting the bar away again");
        SHOWN_FOR_WORK.store(false, Ordering::SeqCst);
        if let Ok(mut last) = LAST_WORK.lock() {
            *last = None;
        }
        if let Err(e) = WindowManager::hide_window(&app, window_labels::FLOATING_BAR).await {
            tracing::warn!("[BarPresence] Could not hide the bar: {e}");
        }
    });
}

/// The person put the bar away or brought it back themselves: whatever this
/// module was holding is theirs now.
pub fn forget() {
    GENERATION.fetch_add(1, Ordering::SeqCst);
    SHOWN_FOR_WORK.store(false, Ordering::SeqCst);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_dictation_leaves_quickly_and_a_reply_stays_to_be_read() {
        assert!(linger_after(Some(VoiceTarget::Dictation)) < Duration::from_secs(1));
        assert!(linger_after(Some(VoiceTarget::Agent)) >= Duration::from_secs(3));
    }
}

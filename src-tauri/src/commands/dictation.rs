use crate::constants::settings::dictation_insertion_modes;
use crate::settings::manager::SettingsManager;
use crate::state::{AppState, SessionClaim};
use std::sync::{Arc, Mutex};
use tauri::{AppHandle, Emitter, Manager, State};
use tauri_plugin_voice_transcription::VoiceController;
use tracing::{error, info, warn};

/// The escape-key ledger name every dictation session registers under.
///
/// One name, because the ledger is a set: a session that registered under one
/// spelling and was released under another leaves the stop key armed forever.
pub const DICTATION_ESCAPE_USER: &str = "dictation_events";

/// Close the microphone.
///
/// Cancel, never stop: stopping finalises the audio and emits a result, which
/// downstream types out a sentence nobody asked to keep.
///
/// One call. The plugin's cancel used to `try_lock` and answer `Ok(false)` when
/// the controller was busy, so this asked three times and hoped. It now waits
/// for the controller and joins the audio thread, so when it returns the
/// capture stream has been dropped.
async fn close_audio_session(app_handle: &AppHandle) {
    let Some(controller_state) = app_handle.try_state::<Arc<Mutex<VoiceController>>>() else {
        return;
    };
    if let Err(e) = tauri_plugin_voice_transcription::commands::cancel_dictation(
        app_handle.clone(),
        controller_state,
    )
    .await
    {
        error!("[Dictation] Could not end the audio session: {}", e);
    }
}

/// End the dictation session, whatever state it is in. The only way a
/// dictation session ends.
///
/// Ending a session is five things, not one: retire its identity in the
/// registry, put the dictation flag down, reset the input monitor, take the
/// bar out of dictation mode, and release the stop key. Every exit path used
/// to do its own subset of that list by hand, and the failure paths did the
/// smallest subsets:
///
/// * a microphone that never opened (`voice-capture:failed`) cleared the flag
///   but left the session standing, and `begin` refuses while one stands — so
///   one failed capture killed every later dictation for the life of the
///   process, with the bar still drawn in dictation mode and Escape declining
///   to act because nothing *looked* live;
/// * a transcription that failed retired the session but left the flag up, and
///   tap mode reads that flag to decide start from stop — so every later tap
///   took the stop branch, found no session, and returned.
///
/// Both are the same shape: half the list. So the list is a function, it is
/// idempotent, and it is what every exit path calls.
///
/// The session is retired *before* the audio is stopped, deliberately: a
/// transcript the engine finalises on the way down then finds no owner and is
/// dropped, rather than being typed into whatever is focused.
pub async fn end_dictation_session(app_handle: &AppHandle, reason: &str) {
    let app_state = app_handle.state::<AppState>();

    match app_state.claim_voice_discard(SessionClaim::Current) {
        Ok(session) => info!(
            "[Dictation] Ending voice session {} ({})",
            session.describe(),
            reason
        ),
        Err(rejection) => info!(
            "[Dictation] Nothing to retire ({}); unwinding anyway ({})",
            rejection.reason(),
            reason
        ),
    }

    close_audio_session(app_handle).await;

    if let Err(e) = app_state.set_dictation_active(false) {
        warn!("[Dictation] Could not clear the dictation flag: {}", e);
    }

    crate::dictation_monitor::force_reset_dictation_input_state().await;

    crate::commands::ui_commands::handle_dictation_mode_change(app_handle, false).await;

    let coordinator = crate::commands::escape_key_coordinator::get_escape_key_coordinator();
    if let Err(e) = coordinator
        .unregister_escape_user(app_handle, DICTATION_ESCAPE_USER)
        .await
    {
        warn!("[Dictation] Could not release the stop key: {}", e);
    }

    if let Err(e) = app_handle.emit(crate::constants::events::dictation::ACTIVE, false) {
        error!("[Dictation] Could not announce the end of dictation: {}", e);
    }
}

/// What one press of the stop key ends.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EscapeScope {
    /// A dictation is open and is the newest thing going: end it and leave
    /// everything else running.
    Dictation,
    /// A dictation is open, but Juno started a turn after it: stop her work
    /// and leave the dictation open.
    WorkAboveDictation,
    /// The same press, seen a second time (the bar's own keydown and the
    /// passive stop-key monitor both report one press while the bar is
    /// focused). It was already spent on one layer.
    SamePress,
    /// Nothing is dictating: the coordinated stop of everything.
    Everything,
}

/// Two reports of one Escape press arrive within this window.
const SAME_PRESS_WINDOW: std::time::Duration = std::time::Duration::from_millis(400);

/// What is going on when the stop key is pressed.
#[derive(Debug, Clone, Copy, Default)]
pub struct EscapeLayers {
    /// A dictation is open.
    pub dictation_open: bool,
    /// When that dictation claimed the stop key.
    pub dictation_since: Option<std::time::Instant>,
    /// When Juno last started a turn that still holds the stop key.
    pub turn_since: Option<std::time::Instant>,
    /// Time since a press last ended only a dictation.
    pub since_dictation_escape: Option<std::time::Duration>,
    /// Time since a press last ended only Juno's work.
    pub since_work_escape: Option<std::time::Duration>,
}

/// Decide what an Escape press ends.
///
/// Escape peels one layer, newest first, the way a Mac app hands Escape to
/// whatever opened most recently. A dictation opened while Juno works is the
/// thing the person is doing right now, so Escape cancels it and only it: the
/// run, Juno's voice, armed timers and the wake phrase all carry on, and a
/// second press stops those. A turn Juno started while the person was already
/// dictating is the newer layer, so it goes first and the dictation stays open.
///
/// A dictation with no claim on record is treated as the newest layer, which
/// is what this did before turns were ordered at all.
pub fn escape_scope(layers: EscapeLayers) -> EscapeScope {
    let within = |since: Option<std::time::Duration>| since.is_some_and(|e| e < SAME_PRESS_WINDOW);
    // Checked first: the press that stopped the work leaves the dictation
    // open, and its second report must not go on to end that too.
    if within(layers.since_work_escape) {
        return EscapeScope::SamePress;
    }
    if layers.dictation_open {
        return match (layers.dictation_since, layers.turn_since) {
            (Some(dictation), Some(turn)) if turn > dictation => EscapeScope::WorkAboveDictation,
            _ => EscapeScope::Dictation,
        };
    }
    if within(layers.since_dictation_escape) {
        EscapeScope::SamePress
    } else {
        EscapeScope::Everything
    }
}

static LAST_DICTATION_ESCAPE: Mutex<Option<std::time::Instant>> = Mutex::new(None);
static LAST_WORK_ESCAPE: Mutex<Option<std::time::Instant>> = Mutex::new(None);

fn elapsed_since(stamp: &Mutex<Option<std::time::Instant>>) -> Option<std::time::Duration> {
    stamp
        .lock()
        .ok()
        .and_then(|last| last.map(|at| at.elapsed()))
}

fn stamp_now(stamp: &Mutex<Option<std::time::Instant>>) {
    if let Ok(mut last) = stamp.lock() {
        *last = Some(std::time::Instant::now());
    }
}

/// The Escape step that comes before the coordinated stop. Returns `true` when
/// the press was spent here (one layer was ended, or this is a second report
/// of the press that ended it) and the caller must not stop anything else.
pub async fn escape_spent_on_dictation(app_handle: &AppHandle) -> bool {
    let app_state = app_handle.state::<AppState>();
    let dictation_open = app_state.is_dictation_active()
        || app_state
            .current_voice_session()
            .is_some_and(|session| session.target == crate::state::VoiceTarget::Dictation);
    let (dictation_since, turn_since) =
        crate::commands::escape_key_coordinator::get_escape_key_coordinator().claims_since();

    let layers = EscapeLayers {
        dictation_open,
        dictation_since,
        turn_since,
        since_dictation_escape: elapsed_since(&LAST_DICTATION_ESCAPE),
        since_work_escape: elapsed_since(&LAST_WORK_ESCAPE),
    };
    match escape_scope(layers) {
        EscapeScope::Everything => false,
        EscapeScope::SamePress => true,
        EscapeScope::Dictation => {
            info!("[Dictation] Escape cancels the open dictation only");
            stamp_now(&LAST_DICTATION_ESCAPE);
            end_dictation_session(app_handle, "Escape").await;
            crate::commands::triggers::resume_voice_after_dictation(app_handle).await;
            true
        }
        EscapeScope::WorkAboveDictation => {
            info!("[Dictation] Escape stops Juno's newer turn; the dictation stays open");
            stamp_now(&LAST_WORK_ESCAPE);
            let coordinator = crate::commands::stop_coordinator::get_stop_coordinator();
            if let Err(e) = coordinator
                .stop_work_keeping_dictation(app_handle, "Escape key pressed")
                .await
            {
                error!(
                    "[Dictation] Stopping the turn above the dictation failed: {}",
                    e
                );
            }
            true
        }
    }
}

// Command to set the dictation copy-to-clipboard toggle. The name predates
// the insertion-mode split: this toggle only controls whether the transcript
// is copied to the clipboard after a successful insert.
#[tauri::command]
pub async fn set_dictation_clipboard_enabled(
    app_handle: AppHandle,
    enabled: bool,
    state: State<'_, AppState>,
) -> Result<(), String> {
    let settings_manager = SettingsManager::new(app_handle)
        .map_err(|e| format!("Failed to initialize settings manager: {}", e))?;

    let mut audio_settings = settings_manager
        .get_audio_settings()
        .await
        .map_err(|e| format!("Failed to load audio settings: {}", e))?;

    audio_settings.set_dictation_copy_to_clipboard(enabled);

    settings_manager
        .set_audio_settings(&audio_settings)
        .await
        .map_err(|e| format!("Failed to save audio settings: {}", e))?;

    // Update state for backward compatibility
    state
        .set_dictation_clipboard_enabled(enabled)
        .map_err(|e| format!("Failed to set dictation_clipboard_enabled: {}", e))?;

    info!("Dictation copy-to-clipboard set to: {}", enabled);
    Ok(())
}

// Command to get the current dictation copy-to-clipboard setting
#[tauri::command]
pub async fn get_dictation_clipboard_enabled(
    app_handle: AppHandle,
    state: State<'_, AppState>,
) -> Result<bool, String> {
    let settings_manager = SettingsManager::new(app_handle)
        .map_err(|e| format!("Failed to initialize settings manager: {}", e))?;

    let audio_settings = settings_manager
        .get_audio_settings()
        .await
        .map_err(|e| format!("Failed to load audio settings: {}", e))?;

    let enabled = audio_settings.dictation_copy_to_clipboard();

    // Sync with state for backward compatibility
    state
        .set_dictation_clipboard_enabled(enabled)
        .map_err(|e| format!("Failed to set dictation_clipboard_enabled: {}", e))?;

    tracing::debug!("Current dictation copy-to-clipboard setting: {}", enabled);
    Ok(enabled)
}

/// Get the dictation text-insertion mode ("paste" or "clipboard_free").
#[tauri::command]
pub async fn get_dictation_insertion_mode(
    app_handle: AppHandle,
    state: State<'_, AppState>,
) -> Result<String, String> {
    let settings_manager = SettingsManager::new(app_handle)
        .map_err(|e| format!("Failed to initialize settings manager: {}", e))?;

    let audio_settings = settings_manager
        .get_audio_settings()
        .await
        .map_err(|e| format!("Failed to load audio settings: {}", e))?;

    state
        .set_dictation_insertion_mode(audio_settings.dictation_insertion_mode.clone())
        .map_err(|e| format!("Failed to sync dictation insertion mode: {}", e))?;

    Ok(audio_settings.dictation_insertion_mode)
}

/// Set the dictation text-insertion mode ("paste" or "clipboard_free").
#[tauri::command]
pub async fn set_dictation_insertion_mode(
    app_handle: AppHandle,
    mode: String,
    state: State<'_, AppState>,
) -> Result<(), String> {
    if mode != dictation_insertion_modes::PASTE && mode != dictation_insertion_modes::CLIPBOARD_FREE
    {
        return Err(format!(
            "Invalid dictation insertion mode '{}'; expected '{}' or '{}'",
            mode,
            dictation_insertion_modes::PASTE,
            dictation_insertion_modes::CLIPBOARD_FREE
        ));
    }

    let settings_manager = SettingsManager::new(app_handle)
        .map_err(|e| format!("Failed to initialize settings manager: {}", e))?;

    let mut audio_settings = settings_manager
        .get_audio_settings()
        .await
        .map_err(|e| format!("Failed to load audio settings: {}", e))?;

    audio_settings.dictation_insertion_mode = mode.clone();

    settings_manager
        .set_audio_settings(&audio_settings)
        .await
        .map_err(|e| format!("Failed to save audio settings: {}", e))?;

    state
        .set_dictation_insertion_mode(mode.clone())
        .map_err(|e| format!("Failed to sync dictation insertion mode: {}", e))?;

    info!("Dictation insertion mode set to: {}", mode);
    Ok(())
}

/// How one dictation insert behaves, decided from the two settings. The pure
/// decision is separated from the I/O so the whole matrix is unit-testable:
///
/// | insertion_mode | copy_to_clipboard | behaviour |
/// |---|---|---|
/// | paste | on  | write transcript, Cmd+V, no restore — transcript stays |
/// | paste | off | write transcript, Cmd+V, restore old clipboard (changeCount-guarded) |
/// | clipboard_free | on  | unicode CGEvent insert, then one clipboard write |
/// | clipboard_free | off | unicode CGEvent insert, clipboard untouched |
#[derive(Debug, PartialEq, Eq)]
struct InsertionPlan {
    /// Insert via unicode CGEvents instead of Cmd+V paste.
    clipboard_free: bool,
    /// Paste mode only: leave the transcript on the clipboard (no snapshot,
    /// no transient markers, no delayed restore).
    paste_retains_clipboard: bool,
    /// Write the transcript to the clipboard after a successful insert. Only
    /// the clipboard-free path needs an explicit copy; in paste mode the
    /// paste's own write already retains it.
    copy_after_insert: bool,
}

fn plan_insertion(insertion_mode: &str, copy_to_clipboard: bool) -> InsertionPlan {
    let clipboard_free = insertion_mode == dictation_insertion_modes::CLIPBOARD_FREE;
    InsertionPlan {
        clipboard_free,
        paste_retains_clipboard: !clipboard_free && copy_to_clipboard,
        copy_after_insert: clipboard_free && copy_to_clipboard,
    }
}

/// Is Juno allowed to post the keystrokes an insertion is made of?
///
/// Both insertion modes come down to posted CGEvents: the paste path posts a
/// synthetic Command V, the clipboard-free path posts the transcript as
/// unicode key events, and that path's fallback ladder reaches the
/// accessibility API for the same job. Without the Accessibility grant macOS
/// discards a posted event outright — no error, no return value, nothing to
/// check, because `CGEvent::post` returns `()`. That is the whole reason
/// "sometimes the text is not pasted" leaves no trace: there is no failure to
/// log, so every layer above reports success.
///
/// Worth checking on every insertion rather than once at startup: a grant can
/// go away while the app is running, and Juno replaces its own bundle every
/// time it installs a CI build, which is exactly when macOS quietly
/// invalidates a grant that System Settings still draws as on.
#[cfg(target_os = "macos")]
fn can_post_keystrokes() -> bool {
    matches!(
        computer_use_ai_sdk::platforms::macos::permissions::check_accessibility_permissions(false),
        Ok(true)
    )
}

#[cfg(not(target_os = "macos"))]
fn can_post_keystrokes() -> bool {
    true
}

/// Leave the transcript somewhere the person can get it back from.
///
/// The paste path with the copy toggle off snapshots the old clipboard and
/// restores it half a second after the write, guarded by the pasteboard's
/// change count. Writing the transcript here moves that count, so the restore
/// stands down and the words survive. Without this, an insertion that did not
/// land took the sentence with it.
async fn keep_transcript_recoverable(
    app_handle: &AppHandle,
    app_state: &State<'_, AppState>,
    text: &str,
) {
    if let Err(e) = crate::commands::core::set_clipboard(
        text.to_string(),
        app_handle.clone(),
        (*app_state).clone(),
    )
    .await
    {
        error!(
            "[Dictation] Could not even put the transcript on the clipboard: {}",
            e
        );
    }
}

/// Say, where a person can see it, that the words did not go in.
///
/// An insertion that fails is the user-visible bug even when the cause is a
/// genuinely flaky OS call, and until now it was an `error!` line in a log
/// nobody reads: the dictation simply appeared not to happen. The tray already
/// turns red on the `app-dictation-error` event, and a notification is the
/// one surface that works when the focused app is not Juno, which, for a
/// dictation, it never is.
fn report_insertion_failure(app_handle: &AppHandle, detail: &str, sentence: &str) {
    error!("[Dictation] Insertion did not land: {}", detail);

    if let Err(e) = tauri::Emitter::emit(
        app_handle,
        crate::constants::events::dictation::ERROR,
        serde_json::json!({ "message": sentence, "detail": detail }),
    ) {
        warn!("[Dictation] Could not announce the failed insertion: {}", e);
    }

    crate::commands::notifications::notify(app_handle, "Juno could not type that", sentence);
}

/// Insert a dictation transcript into the focused app using the configured
/// insertion mode and copy-to-clipboard toggle (see [`InsertionPlan`]).
///
/// The two settings are independent: an explicit copy happens after a
/// successful insert, never as a side effect of the insert path. Settings are
/// read from the store (not the state mirror) so the behaviour is correct
/// even when no frontend has run yet.
pub async fn insert_dictation_text(app_handle: &AppHandle, text: &str) -> Result<(), String> {
    let app_state = app_handle.state::<AppState>();

    let from_store = match SettingsManager::new(app_handle.clone()) {
        Ok(manager) => manager.get_audio_settings().await.map(|audio| {
            (
                audio.dictation_insertion_mode.clone(),
                audio.dictation_copy_to_clipboard(),
            )
        }),
        Err(e) => Err(e),
    };
    let (insertion_mode, copy_to_clipboard) = from_store.unwrap_or_else(|e| {
        warn!(
            "[Dictation] Falling back to state mirror for insertion settings: {}",
            e
        );
        (
            app_state
                .get_dictation_insertion_mode()
                .unwrap_or_else(|_| dictation_insertion_modes::PASTE.to_string()),
            app_state.get_dictation_clipboard_enabled().unwrap_or(true),
        )
    });

    let plan = plan_insertion(&insertion_mode, copy_to_clipboard);

    // Pause ellipses out, then the optional on-device grammar pass. Dictation
    // only: this function is never reached by a voice turn for the agent. The
    // bar keeps its processing state while this runs; it falls back to the
    // transcript as heard, so the worst case is a slower, uncleaned insert.
    let prepared = crate::dictation_cleanup::prepare(app_handle, text).await;
    let text = prepared.as_str();

    let started = std::time::Instant::now();
    let attempt = if plan.clipboard_free {
        let owned = text.to_string();
        match tokio::task::spawn_blocking(move || {
            computer_use_ai_sdk::insert_text_clipboard_free(&owned)
        })
        .await
        {
            Ok(Ok(path)) => Ok(format!("mode=clipboard_free, path={}", path)),
            Ok(Err(e)) => Err(format!("Clipboard-free insertion failed: {}", e)),
            Err(e) => Err(format!("Clipboard-free insertion task panicked: {}", e)),
        }
    } else {
        let owned = text.to_string();
        let retain = plan.paste_retains_clipboard;
        match tokio::task::spawn_blocking(move || {
            computer_use_ai_sdk::paste_text_global(&owned, retain)
        })
        .await
        {
            Ok(Ok(())) => Ok(format!("mode=paste, retain_clipboard={}", retain)),
            Ok(Err(e)) => Err(format!("Paste insertion failed: {}", e)),
            Err(e) => Err(format!("Paste insertion task panicked: {}", e)),
        }
    };

    // "Sent", deliberately not "inserted". Both paths finish by posting
    // CGEvents, and a posted event has no outcome to read: the paste path's
    // `CGEvent::post` returns `()`. The old line said "Inserted N chars", so
    // a log of a dictation that never landed read as a clean success, which
    // is how "sometimes the text is not pasted" stayed invisible.
    match attempt {
        Ok(how) => info!(
            "[Dictation] Sent {} chars to the focused app in {} ms ({})",
            text.chars().count(),
            started.elapsed().as_millis(),
            how
        ),
        Err(e) => {
            // The words are not lost. Whatever the insertion mode and the
            // copy toggle say, a failed insertion leaves the transcript on
            // the clipboard, because the alternative is a sentence the person
            // said out loud existing nowhere.
            keep_transcript_recoverable(app_handle, &app_state, text).await;
            report_insertion_failure(
                app_handle,
                &e,
                "Juno could not type that. The words are on the clipboard: press Command V to paste them.",
            );
            return Err(e);
        }
    }

    // An insertion that was *sent* is not an insertion that landed, and when
    // Accessibility is off it certainly did not: macOS drops a posted CGEvent
    // entirely, with no error and nothing to check. Say so, and keep the
    // words. Checked after the attempt rather than before it so this can
    // never be the reason an insertion that would have worked did not happen.
    if !can_post_keystrokes() {
        keep_transcript_recoverable(app_handle, &app_state, text).await;
        report_insertion_failure(
            app_handle,
            "Accessibility is not granted, so the keystrokes Juno posted were discarded by macOS",
            "Juno is not allowed to type into other apps, so nothing was pasted. The words are on the clipboard. Turn Juno on in System Settings, under Privacy and Security, Accessibility.",
        );
        return Err(
            "Accessibility permission is not granted, so the insertion could not land".to_string(),
        );
    }

    // Mirror global_type_text's key-press visualization so dictation looks
    // the same in the UI regardless of insertion path.
    if let Err(e) = tauri::Emitter::emit(
        app_handle,
        crate::constants::events::ui::KEY_PRESS_VISUALIZATION,
        serde_json::json!({
            "key": format!(
                "Global: {}",
                text.chars()
                    .take(crate::constants::ui::text_display::MAX_KEYPRESS_VISUALIZATION_TEXT_LENGTH)
                    .collect::<String>()
            ),
            "modifier": null
        }),
    ) {
        warn!("[Dictation] Failed to emit key press visualization: {}", e);
    }

    if plan.copy_after_insert {
        if let Err(e) = crate::commands::core::set_clipboard(
            text.to_string(),
            app_handle.clone(),
            app_state.clone(),
        )
        .await
        {
            // The insert itself succeeded; a failed copy should not read as a
            // failed dictation.
            error!(
                "[Dictation] Inserted, but failed to copy transcript to clipboard: {}",
                e
            );
        }
    }

    Ok(())
}

/// What the Dictation settings need to draw the cleanup toggle: whether it is
/// on, whether Apple's on-device model can run it, and why not when it cannot.
#[tauri::command]
pub async fn get_dictation_cleanup_status(
    app_handle: AppHandle,
) -> Result<crate::dictation_cleanup::CleanupStatus, String> {
    let enabled = crate::dictation_cleanup::is_enabled(&app_handle).await;
    let availability = tokio::task::spawn_blocking(crate::dictation_cleanup::availability)
        .await
        .map_err(|e| format!("Availability check failed: {}", e))?;
    Ok(crate::dictation_cleanup::CleanupStatus::new(
        enabled,
        availability,
    ))
}

/// Turn the dictation cleanup pass on or off.
#[tauri::command]
pub async fn set_dictation_cleanup_enabled(
    app_handle: AppHandle,
    enabled: bool,
) -> Result<(), String> {
    let settings_manager = SettingsManager::new(app_handle)
        .map_err(|e| format!("Failed to initialize settings manager: {}", e))?;
    let mut audio_settings = settings_manager
        .get_audio_settings()
        .await
        .map_err(|e| format!("Failed to load audio settings: {}", e))?;
    audio_settings.dictation_cleanup = enabled;
    settings_manager
        .set_audio_settings(&audio_settings)
        .await
        .map_err(|e| format!("Failed to save audio settings: {}", e))?;
    info!("Dictation cleanup set to: {}", enabled);
    Ok(())
}

/// Open System Settings on Apple Intelligence & Siri, where the on-device
/// model is switched on.
#[tauri::command]
pub async fn open_apple_intelligence_settings() -> Result<(), String> {
    crate::dictation_cleanup::open_settings_pane()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::constants::settings::dictation_insertion_modes::{CLIPBOARD_FREE, PASTE};

    fn ms(n: u64) -> Option<std::time::Duration> {
        Some(std::time::Duration::from_millis(n))
    }

    fn dictating() -> EscapeLayers {
        EscapeLayers {
            dictation_open: true,
            ..EscapeLayers::default()
        }
    }

    #[test]
    fn escape_during_a_dictation_ends_only_the_dictation() {
        // A dictation is open (the agent may well be working underneath):
        // Escape is spent on the dictation, not the coordinated stop.
        assert_eq!(escape_scope(dictating()), EscapeScope::Dictation);
        assert_eq!(
            escape_scope(EscapeLayers {
                since_dictation_escape: ms(10),
                ..dictating()
            }),
            EscapeScope::Dictation
        );
    }

    #[test]
    fn the_newest_layer_goes_first() {
        let earlier = std::time::Instant::now();
        let later = earlier + std::time::Duration::from_secs(5);
        // Juno was working, then the person started dictating: dictation first.
        assert_eq!(
            escape_scope(EscapeLayers {
                dictation_since: Some(later),
                turn_since: Some(earlier),
                ..dictating()
            }),
            EscapeScope::Dictation
        );
        // The person was dictating, then Juno started a turn: the turn first.
        assert_eq!(
            escape_scope(EscapeLayers {
                dictation_since: Some(earlier),
                turn_since: Some(later),
                ..dictating()
            }),
            EscapeScope::WorkAboveDictation
        );
    }

    #[test]
    fn the_press_that_stopped_the_turn_does_not_also_end_the_dictation() {
        assert_eq!(
            escape_scope(EscapeLayers {
                since_work_escape: ms(50),
                ..dictating()
            }),
            EscapeScope::SamePress
        );
        // A deliberate second press ends the dictation.
        assert_eq!(
            escape_scope(EscapeLayers {
                since_work_escape: Some(SAME_PRESS_WINDOW),
                ..dictating()
            }),
            EscapeScope::Dictation
        );
    }

    #[test]
    fn the_second_report_of_that_press_stops_nothing_else() {
        // The bar's keydown and the passive monitor report one press twice.
        // The second report must not go on to stop the agent.
        assert_eq!(
            escape_scope(EscapeLayers {
                since_dictation_escape: ms(50),
                ..EscapeLayers::default()
            }),
            EscapeScope::SamePress
        );
    }

    #[test]
    fn a_later_escape_with_no_dictation_stops_everything() {
        assert_eq!(
            escape_scope(EscapeLayers::default()),
            EscapeScope::Everything
        );
        assert_eq!(
            escape_scope(EscapeLayers {
                since_dictation_escape: Some(SAME_PRESS_WINDOW),
                ..EscapeLayers::default()
            }),
            EscapeScope::Everything,
            "a deliberate second press stops the run"
        );
    }

    // One case per row of the behaviour matrix in the issue.

    #[test]
    fn paste_with_copy_on_retains_the_clipboard() {
        // SuperWhisper default: transcript lives on the clipboard afterwards.
        assert_eq!(
            plan_insertion(PASTE, true),
            InsertionPlan {
                clipboard_free: false,
                paste_retains_clipboard: true,
                copy_after_insert: false,
            }
        );
    }

    #[test]
    fn paste_with_copy_off_restores_the_old_clipboard() {
        assert_eq!(
            plan_insertion(PASTE, false),
            InsertionPlan {
                clipboard_free: false,
                paste_retains_clipboard: false,
                copy_after_insert: false,
            }
        );
    }

    #[test]
    fn clipboard_free_with_copy_on_writes_the_clipboard_once_after_insert() {
        assert_eq!(
            plan_insertion(CLIPBOARD_FREE, true),
            InsertionPlan {
                clipboard_free: true,
                paste_retains_clipboard: false,
                copy_after_insert: true,
            }
        );
    }

    #[test]
    fn clipboard_free_with_copy_off_never_touches_the_clipboard() {
        assert_eq!(
            plan_insertion(CLIPBOARD_FREE, false),
            InsertionPlan {
                clipboard_free: true,
                paste_retains_clipboard: false,
                copy_after_insert: false,
            }
        );
    }
}

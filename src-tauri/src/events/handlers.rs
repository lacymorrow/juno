//! # Event Handlers
//!
//! This module contains all event listener handlers for voice transcription,
//! dictation events, timer events, and other application event management.

use std::sync::{Arc, Mutex};
use tauri::{AppHandle, Emitter, Listener, Manager};
use tauri_plugin_voice_transcription::controller::VoiceController;
use tracing::{error, info, warn};

use crate::agent::tools::timer_tools::TimerTask;
use crate::events::timer_handlers::TimerEventHandler;
use crate::state::{SessionClaim, VoiceStartMethod, VoiceTarget};
use crate::window_management::WindowManager;
use crate::{constants, state};

/// Setup all event listeners for the application
pub fn setup_event_listeners(app: &AppHandle) {
    setup_voice_transcription_listeners(app);
    setup_dictation_listeners(app);
    setup_timer_event_listeners(app);
    setup_menu_event_listeners(app);
}

/// Setup menu-triggered event listeners
fn setup_menu_event_listeners(app: &AppHandle) {
    let app_handle = app.clone();
    app.listen(
        constants::events::menu::TOGGLE_FLOATING_BAR_REQUESTED,
        move |_event| {
            let app_handle = app_handle.clone();
            tauri::async_runtime::spawn(async move {
                handle_toggle_floating_bar(app_handle).await;
            });
        },
    );
}

async fn handle_toggle_floating_bar(app_handle: AppHandle) {
    let label = constants::ui::window_labels::FLOATING_BAR;
    if WindowManager::is_window_visible(&app_handle, label) {
        if let Err(e) = WindowManager::hide_window(&app_handle, label).await {
            error!("[Menu] Failed to hide floating bar: {}", e);
        }
    } else if let Some(window) = app_handle.get_webview_window(label) {
        if let Err(e) = window.show() {
            error!("[Menu] Failed to show floating bar: {}", e);
        }
    } else {
        warn!("[Menu] Floating bar window not found");
    }
}

/// Setup voice transcription event listeners
fn setup_voice_transcription_listeners(app: &AppHandle) {
    // Listen for voice transcription final results
    let app_handle_for_listener = app.clone();
    app.listen(
        constants::events::voice_transcription::FINAL_RESULT,
        move |event| {
            let app_handle = app_handle_for_listener.clone();
            tauri::async_runtime::spawn(async move {
                handle_voice_transcription_final_result(app_handle, event.payload()).await;
            });
        },
    );

    // Listen for dictation stopped events
    let app_handle_for_listener = app.clone();
    app.listen(
        constants::events::voice_transcription::DICTATION_STOPPED,
        move |event| {
            let app_handle = app_handle_for_listener.clone();
            tauri::async_runtime::spawn(async move {
                handle_voice_transcription_dictation_stopped(
                    app_handle,
                    event.payload().to_string(),
                )
                .await;
            });
        },
    );

    // Listen for voice transcription errors
    let app_handle_for_error_listener = app.clone();
    app.listen(
        constants::events::voice_transcription::ERROR,
        move |_event| {
            let app_handle = app_handle_for_error_listener.clone();
            tauri::async_runtime::spawn(async move {
                handle_voice_transcription_error(app_handle).await;
            });
        },
    );
}

/// Setup dictation event listeners
fn setup_dictation_listeners(app: &AppHandle) {
    // Listen for dictation-transcription-start events
    let app_handle_for_dictation_start = app.clone();
    app.listen(
        constants::events::dictation::TRANSCRIPTION_START,
        move |event| {
            let app_handle = app_handle_for_dictation_start.clone();
            // The payload says how this session was triggered, so the session
            // identity can record it instead of the stop paths inferring it.
            let method = VoiceStartMethod::from_event_payload(event.payload());
            // A held key's start carries its hold id. Settling it when the
            // handler is done, however it ends, performs a release that
            // arrived while the microphone was still opening (see
            // `hold_gate`).
            let hold = crate::hold_gate::hold_from_payload(event.payload());
            tauri::async_runtime::spawn(async move {
                let _settle = crate::hold_gate::SettleOnDrop::new(
                    app_handle.clone(),
                    VoiceTarget::Dictation,
                    hold,
                );
                handle_dictation_transcription_start(app_handle, method, hold).await;
            });
        },
    );

    // Listen for dictation-cancel events
    let app_handle_for_dictation_cancel = app.clone();
    app.listen(
        constants::events::dictation::TRANSCRIPTION_CANCEL,
        move |_event| {
            let app_handle = app_handle_for_dictation_cancel.clone();
            tauri::async_runtime::spawn(async move {
                handle_dictation_cancel(app_handle).await;
            });
        },
    );

    // Listen for dictation-stop events
    let app_handle_for_dictation_stop = app.clone();
    app.listen(constants::events::dictation::STOP, move |_event| {
        let app_handle = app_handle_for_dictation_stop.clone();
        tauri::async_runtime::spawn(async move {
            handle_dictation_stop(app_handle).await;
        });
    });

    // Listen for force stop events
    let app_handle_for_force_stop = app.clone();
    app.listen(
        constants::events::force_stop::TRANSCRIPTION,
        move |_event| {
            let app_handle = app_handle_for_force_stop.clone();
            tauri::async_runtime::spawn(async move {
                handle_force_stop_transcription(app_handle).await;
            });
        },
    );
}

/// Setup timer event listeners for processing timer-expired events
fn setup_timer_event_listeners(app: &AppHandle) {
    info!("Setting up timer event listeners");

    // Listen for timer-expired events
    let app_handle_for_timer = app.clone();
    app.listen(constants::events::timer::EXPIRED, move |event| {
        let app_handle = app_handle_for_timer.clone();
        tauri::async_runtime::spawn(async move {
            handle_timer_expired_event(app_handle, event.payload()).await;
        });
    });

    // Listen for timer-queued events (for monitoring)
    let app_handle_for_queued = app.clone();
    app.listen(constants::events::timer::QUEUED, move |event| {
        let app_handle = app_handle_for_queued.clone();
        tauri::async_runtime::spawn(async move {
            handle_timer_queued_event(app_handle, event.payload()).await;
        });
    });

    info!("Timer event listeners registered successfully");
}

// Event handler functions now properly organized and with correct signatures

async fn handle_voice_transcription_final_result(app_handle: AppHandle, payload_str: &str) {
    info!(
        "[Event] Received voice-transcription:final-result event: {:?}",
        payload_str
    );

    // Ask the session who this text belongs to. Routing used to read
    // `dictation_active`, which always saw false here, because the transcript
    // is produced by stopping the session that would have set it, so every
    // dictation was submitted to the agent instead of being typed. The session
    // records its target when it starts and keeps it past the stop.
    let app_state = app_handle.state::<state::AppState>();
    let Some(session) = app_state.take_voice_transcript_owner() else {
        // Text with no owner is not delivered. Every path that opens the
        // microphone registers a session, so arriving here means the session
        // was cancelled and the engine finalised anyway. Delivering it would
        // mean typing or, worse, acting on a sentence the person asked to
        // throw away, which is the whole family of bugs this exists to end.
        warn!(
            "[Event] Final result arrived with no voice session to own it; discarding it rather than guessing where it should go"
        );
        return;
    };
    info!(
        "[Event] Final result belongs to voice session {}",
        session.describe()
    );
    let deliver_to_dictation = session.target == VoiceTarget::Dictation;

    // Extract text from payload
    let extracted_text = match serde_json::from_str::<serde_json::Value>(payload_str) {
        Ok(payload_json) => payload_json
            .get("text")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string()),
        Err(_) => None,
    };

    // Hand the text to the side of the app that its session belongs to.
    if deliver_to_dictation {
        handle_dictation_mode_result(app_handle, extracted_text).await;
    } else {
        handle_agent_mode_result(app_handle, extracted_text, payload_str.to_string()).await;
    }
}

async fn handle_dictation_mode_result(app_handle: AppHandle, extracted_text: Option<String>) {
    info!("[Event] Processing final result for Dictation Mode");

    if let Some(text) = extracted_text {
        let trimmed_text = text.trim();
        if !trimmed_text.is_empty() {
            // Insert using the configured insertion mode; the copy-to-clipboard
            // toggle is applied after a successful insert, inside the helper.
            match crate::commands::dictation::insert_dictation_text(&app_handle, trimmed_text).await
            {
                Ok(()) => {
                    // "Sent", not "inserted". See the note in
                    // `insert_dictation_text`: the keystrokes an insertion is
                    // made of are posted, and a posted event has no outcome
                    // to read.
                    info!(
                        "[Dictation Mode] Sent text to the focused app: '{}'",
                        trimmed_text
                    );
                }
                Err(e) => {
                    // `insert_dictation_text` has already said this where the
                    // person can see it and left the words on the clipboard.
                    error!("[Dictation Mode] Failed to insert transcribed text: {}", e);
                }
            }
        } else {
            // Nothing was said, or nothing the engine would commit to. Said
            // out loud so a report of "the text did not appear" can be told
            // apart from one of "there was no text": the two look identical
            // from the outside and used to look identical in the log too.
            info!("[Dictation Mode] The transcript was empty; nothing to insert");
        }
    } else {
        info!("[Dictation Mode] The final result carried no text; nothing to insert");
    }

    // Reset Dictation Mode state after processing
    let app_state = app_handle.state::<state::AppState>();
    if let Err(e) = app_state.set_dictation_active(false) {
        warn!("Failed to reset dictation active state: {}", e);
    }

    // Emit state change event for UI
    if let Err(e) = app_handle.emit(constants::events::dictation::ACTIVE, false) {
        error!(
            "[Dictation Mode] Failed to emit dictation-active event after final result: {}",
            e
        );
    }

    // Update floating bar manager for dictation mode completion
    // First update the mode state, then handle the finished event
    crate::commands::ui_commands::handle_dictation_mode_change(&app_handle, false).await;
    crate::commands::ui_commands::handle_dictation_finished(&app_handle, None).await;

    // Bring the wake-phrase engine back if this dictation paused it and voice
    // is still wanted. Not from the remembered flag alone: that is how voice
    // switched off mid-dictation came back on (see the function).
    crate::commands::triggers::resume_voice_after_dictation(&app_handle).await;

    info!("[Dictation Mode] Completed dictation successfully");
}

async fn handle_agent_mode_result(
    app_handle: AppHandle,
    _extracted_text: Option<String>,
    payload_str: String,
) {
    info!("[Event] Processing final result for AI Agent Mode");

    // Transform and emit the event for the integration layer to handle
    // This ensures a single, clean path for agent submission
    match serde_json::from_str::<serde_json::Value>(&payload_str) {
        Ok(payload_json) => {
            if let Some(text_value) = payload_json.get("text") {
                // Nothing said, nothing asked. The filter returns an empty
                // string when the whole utterance was Whisper narrating a
                // sound it heard, and waking the agent to answer silence is
                // a slow, expensive way to say nothing. Dictation already
                // guarded this; the agent did not.
                if text_value
                    .as_str()
                    .map(|t| t.trim().is_empty())
                    .unwrap_or(false)
                {
                    info!("[Event] Empty transcription, not waking the agent");
                    return;
                }

                let transformed_payload = serde_json::json!({
                    "query": text_value
                });

                // Use a different event for agent mode to avoid confusion with dictation mode
                info!("[Event] Emitting agent-query-ready event for agent processing");
                if let Err(e) = app_handle.emit(
                    crate::constants::events::agent::QUERY_READY,
                    transformed_payload,
                ) {
                    error!("[Event] Failed to emit agent-query-ready event: {}", e);
                }
            } else {
                error!(
                    "[Event] No 'text' field found in final-result payload: {}",
                    payload_str
                );
            }
        }
        Err(e) => {
            error!(
                "[Event] Failed to parse final-result payload as JSON: {}, payload: {}",
                e, payload_str
            );
            if let Err(e) =
                app_handle.emit(crate::constants::events::agent::QUERY_READY, payload_str)
            {
                error!(
                    "[Event] Failed to emit agent-query-ready event (fallback): {}",
                    e
                );
            }
        }
    }
}

async fn handle_voice_transcription_dictation_stopped(app_handle: AppHandle, _payload: String) {
    // Unregister escape key as dictation is complete
    {
        let coordinator = crate::commands::escape_key_coordinator::get_escape_key_coordinator();
        if let Err(e) = coordinator
            .unregister_escape_user(
                &app_handle,
                crate::commands::dictation::DICTATION_ESCAPE_USER,
            )
            .await
        {
            warn!(
                "Failed to unregister escape key after dictation: {} - continuing anyway",
                e
            );
        }
    }

    // NOTE: the stop cue is played on the key-up edge
    // (dictation_monitor::on_dictation_input_released / the tap handler), not
    // here — this handler only runs after speech-to-text finalization, which is
    // exactly the multi-second delay that made stopping feel unresponsive.

    // Bring the wake-phrase engine back if this dictation paused it and voice
    // is still wanted. Not from the remembered flag alone: that is how voice
    // switched off mid-dictation came back on (see the function).
    crate::commands::triggers::resume_voice_after_dictation(&app_handle).await;
}

async fn handle_voice_transcription_error(app_handle: AppHandle) {
    // A failed session produces no transcript, so nothing would close it out.
    // Left open it would claim the *next* session's result, and every stop
    // aimed at the next session would be refused as stale.
    let app_state = app_handle.state::<crate::state::AppState>();
    let open_target = app_state.current_voice_session().map(|s| s.target);

    match open_target {
        Some(VoiceTarget::Agent) => {
            // The agent path owns its own unwind; retiring the identity is all
            // that is ours to do here.
            match app_state.claim_voice_discard(SessionClaim::Current) {
                Ok(session) => warn!(
                    "[Event] Transcription failed; discarding voice session {}",
                    session.describe()
                ),
                Err(rejection) => info!(
                    "[Event] Transcription failed with nothing to discard: {}",
                    rejection.reason()
                ),
            }
        }
        Some(VoiceTarget::Dictation) => {
            // Retiring the session was never enough. The dictation flag stayed
            // up, and tap mode reads that flag to decide whether a press starts
            // or stops — so after one failed transcription every later tap took
            // the stop branch, found nothing to claim, and returned. One failure
            // left dictation stuck in dictation mode until the app restarted.
            crate::commands::dictation::end_dictation_session(&app_handle, "transcription failed")
                .await;
        }
        None => {
            // No session, but the flag can still be up if an earlier path put
            // it up and then failed. Unwind only when it is actually lying.
            if app_state.is_dictation_active() {
                warn!("[Event] Transcription failed with no session but the dictation flag up; unwinding it");
                crate::commands::dictation::end_dictation_session(
                    &app_handle,
                    "transcription failed with a stale dictation flag",
                )
                .await;
            } else {
                info!("[Event] Transcription failed with nothing to discard");
            }
        }
    }

    // Play voice error sound automatically when transcription fails
    let app_state = app_handle.state::<crate::state::AppState>();
    if let Err(e) =
        crate::commands::sound::play_voice_error_sound(app_handle.clone(), app_state).await
    {
        warn!("Failed to play voice error sound: {}", e);
    }
}

async fn handle_dictation_transcription_start(
    app_handle: AppHandle,
    method: VoiceStartMethod,
    hold: Option<crate::hold_gate::HoldId>,
) {
    let app_state = app_handle.state::<state::AppState>();

    // Give the session an identity before anything opens the microphone. Every
    // stop path from here on consults it rather than working out for itself
    // whether it owns what it is stopping.
    //
    // Refused while a session is already open, and refused before this handler
    // has touched a single flag. The microphone is already recording for
    // somebody, and everything below here (the dictation flag, the always
    // listening pause, the bar's dictation mode) describes a session that would
    // never exist.
    let session = match app_state.begin_voice_session(VoiceTarget::Dictation, method) {
        Ok(session) => session,
        Err(refused) => {
            info!(
                "[Dictation Mode] Ignoring a dictation start: {}",
                refused.reason()
            );
            // An agent voice turn holds the microphone; this key's release
            // must not commit or cancel it. See `StartGate::disown`.
            crate::hold_gate::disown_if_crossed(
                VoiceTarget::Dictation,
                refused.standing.target,
                hold,
            );
            return;
        }
    };
    let session_id = session.id;
    info!(
        "[Dictation Mode] Opened voice session {}",
        session.describe()
    );

    // Mark this as Dictation Mode in AppState BEFORE starting transcription.
    // This is a liveness flag for the UI only; the session above is what says
    // who the transcript belongs to.
    if let Err(e) = app_state.set_dictation_active(true) {
        warn!("Failed to set dictation active state: {}", e);
    }

    // Pause always listening mode if it's active
    let was_always_listening_active = app_state.get_always_listening_active().unwrap_or(false);

    // Store the state so we can restore it later
    if let Ok(mut audio_settings) = app_state.audio_settings.lock() {
        audio_settings.was_always_listening_active_before_dictation = was_always_listening_active;
    }

    if was_always_listening_active {
        info!("[Dictation Mode] Pausing always listening mode");
        if let Err(e) = crate::commands::always_listening::stop_always_listening_mode(
            app_handle.clone(),
            app_state.clone(),
        )
        .await
        {
            warn!(
                "[Dictation Mode] Failed to pause always listening mode: {}",
                e
            );
        }
    }

    // Update floating bar manager to set dictation mode
    let app_handle_for_bar = app_handle.clone();
    tauri::async_runtime::spawn(async move {
        crate::commands::ui_commands::handle_dictation_mode_change(&app_handle_for_bar, true).await;
    });

    // Use the plugin command to start dictation only if controller exists
    if let Some(controller_state) = app_handle.try_state::<Arc<Mutex<VoiceController>>>() {
        match tauri_plugin_voice_transcription::commands::start_dictation(
            app_handle.clone(),
            controller_state,
        )
        .await
        {
            Ok(()) => {
                // Was this session ended while the microphone was opening (the
                // stop key, a tap, the bar)? Then the end found no microphone
                // to close, and the one just opened belongs to nobody. Close
                // it and put the app back to rest, silently, the way the agent
                // start already does.
                let still_ours = app_state
                    .current_voice_session()
                    .is_some_and(|open| open.id == session_id);
                if !still_ours {
                    info!(
                        "[Dictation Mode] Voice session {} ended while it was starting; closing the microphone it opened",
                        session_id
                    );
                    crate::integration::close_unowned_audio_stream(&app_handle).await;
                    if app_state.current_voice_session().is_none() {
                        crate::commands::dictation::end_dictation_session(
                            &app_handle,
                            "ended while starting",
                        )
                        .await;
                    }
                    return;
                }

                info!("[Dictation Mode] Started immediate transcription successfully");

                if let Err(e) = app_handle.emit(constants::events::dictation::ACTIVE, true) {
                    error!(
                        "[Dictation Mode] Failed to emit dictation-active event: {}",
                        e
                    );
                }

                // Register escape key to cancel dictation
                {
                    let coordinator =
                        crate::commands::escape_key_coordinator::get_escape_key_coordinator();
                    if let Err(e) = coordinator
                        .register_escape_user(
                            &app_handle,
                            crate::commands::dictation::DICTATION_ESCAPE_USER,
                        )
                        .await
                    {
                        warn!(
                            "Failed to register escape key for dictation: {} - continuing anyway",
                            e
                        );
                    }
                }

                // NOTE: the start cue is played on the key-down edge
                // (dictation_monitor::on_dictation_input_pressed / the tap
                // handler), not here — this handler only runs after audio
                // capture has initialized, which is too late to feel instant.
            }
            Err(e) => {
                error!("[Dictation Mode] Failed to start dictation: {}", e);

                // The microphone never opened, so this session owns nothing.
                // Leaving it registered would make it claim the next
                // session's transcript.
                //
                // Only unwind what this start put up. If the session we minted
                // is not the open one any more, the dictation flag and the bar
                // are describing somebody else's session, and putting them
                // back to rest here is what leaves a live microphone with an
                // idle-looking app around it.
                if app_state
                    .claim_voice_discard(SessionClaim::Id(session_id))
                    .is_err()
                {
                    info!("[Dictation Mode] The failed start no longer owns its session; leaving the open one alone");
                    return;
                }

                // The one end-of-session unwind, same as cancel and same as
                // every failure path. The id-claim above is what makes it safe
                // to run: it proves this failed start still owned the session,
                // so none of this is describing somebody else's.
                crate::commands::dictation::end_dictation_session(
                    &app_handle,
                    "dictation failed to start",
                )
                .await;
            }
        }
    } else {
        error!("[Dictation Mode] Voice controller not found, cannot start dictation");

        // Nothing was ever recording, so retire the identity we just minted.
        let _ = app_state.claim_voice_discard(SessionClaim::Id(session_id));

        // Reset the dictation active flag
        if let Err(e) = app_state.set_dictation_active(false) {
            warn!("Failed to reset dictation active state: {}", e);
        }

        // Emit state change event for UI
        if let Err(e) = app_handle.emit(constants::events::dictation::ACTIVE, false) {
            error!(
                "[Dictation Mode] Failed to emit dictation-active event after error: {}",
                e
            );
        }
    }
}

async fn handle_dictation_cancel(app_handle: AppHandle) {
    info!("[Event] Cancelling dictation");

    let app_state = app_handle.state::<state::AppState>();

    // The verb is fixed, the session is not. If what is open is an agent
    // session, this cancel is aimed at the wrong state machine: reaching into
    // the shared voice controller here would silence the microphone while the
    // agent path still believed it was listening. Hand it to the path that
    // owns it instead, with the same verb.
    if let Some(session) = app_state.current_voice_session() {
        if session.target == VoiceTarget::Agent {
            info!(
                "[Dictation Cancel] The open session is {}; routing the cancel there",
                session.describe()
            );
            if let Err(e) = app_handle.emit(constants::events::agent::CANCEL, ()) {
                error!(
                    "[Dictation Cancel] Failed to hand the cancel to the agent: {}",
                    e
                );
            }
            return;
        }
    }

    // Cancel is one of the exit paths, not a cleanup routine of its own.
    // What it used to be was the full end-of-session checklist written out by
    // hand here — retire the session, cancel the audio, clear the flag, reset
    // the monitor, take the bar out of dictation mode, release the stop key —
    // which is the same checklist the failure paths each did a different
    // subset of. It is one function now, so cancel and every failure path end
    // a session identically, by construction rather than by remembering.
    //
    // Ungated on owning a session, as it was before: a cancel that owns
    // nothing still unwinds, because a microphone open with nothing to own it
    // is exactly the state this has to be able to close, and the unwind is
    // idempotent.
    crate::commands::dictation::end_dictation_session(&app_handle, "cancelled").await;

    // Bring the wake-phrase engine back if this dictation paused it and voice
    // is still wanted. Not from the remembered flag alone: that is how voice
    // switched off mid-dictation came back on (see the function).
    crate::commands::triggers::resume_voice_after_dictation(&app_handle).await;

    info!("[Dictation Cancel] Cleanup completed successfully");
}

async fn handle_dictation_stop(app_handle: AppHandle) {
    info!("[Event] Stopping dictation normally");

    let app_state = app_handle.state::<state::AppState>();

    // Same rule as cancel: the verb stays "commit", the session decides whose
    // audio it commits. An agent session finalised through here would have its
    // text typed instead of submitted.
    if let Some(session) = app_state.current_voice_session() {
        if session.target == VoiceTarget::Agent {
            info!(
                "[Dictation Stop] The open session is {}; routing the stop there",
                session.describe()
            );
            if let Err(e) = app_handle.emit(constants::events::agent::TRANSCRIPTION_STOP, ()) {
                error!(
                    "[Dictation Stop] Failed to hand the stop to the agent: {}",
                    e
                );
            }
            return;
        }
    }

    // Claim before doing anything visible or audible. A stop for a session
    // that is already gone (a key released after a cancel, a doubled stop
    // event) finds nothing to claim and does nothing at all, rather than
    // finalising and typing a session somebody else already ended.
    match app_state.claim_voice_commit(SessionClaim::Current) {
        Ok(session) => {
            info!(
                "[Dictation Stop] Committing voice session {}",
                session.describe()
            );
        }
        Err(rejection) => {
            info!("[Dictation Stop] Nothing to stop: {}", rejection.reason());
            // A stop that owns nothing is usually a doubled event, and a no-op
            // is the right answer. But if the app is still *claiming* to
            // dictate with no session behind the claim, this is the stuck
            // state: tap mode reads that same flag to decide start from stop,
            // so every later tap lands here and returns, and dictation can
            // never be started or stopped again. Reconcile rather than return.
            if app_state.is_dictation_active() {
                warn!("[Dictation Stop] The dictation flag outlived its session; ending what the app still claims");
                crate::commands::dictation::end_dictation_session(
                    &app_handle,
                    "a stop arrived with no session behind the dictation flag",
                )
                .await;
            }
            return;
        }
    }

    // Reset visible state FIRST, before the speech-to-text finalization below.
    // `stop_dictation()` blocks for however long the final transcription takes
    // (often a second or more); doing the state reset and bar update after it
    // left the bar visually stuck in dictation mode the whole time. Flip the
    // bar out of dictation mode now so the UI reacts to the key-up immediately,
    // then finalize the transcription.
    if let Err(e) = app_state.set_dictation_active(false) {
        warn!("Failed to reset dictation active state: {}", e);
    }

    // Update floating bar manager (leaves dictation mode).
    crate::commands::ui_commands::handle_dictation_mode_change(&app_handle, false).await;

    // Show the processing (transcribing) state while stop_dictation finalizes
    // below, matching the on-screen Stop button path. Without this the bar
    // jumped straight from listening to idle on a key-up; the final-result
    // handler returns it to idle once the text is produced.
    crate::commands::ui_commands::handle_dictation_partial(&app_handle, String::new(), false).await;

    if let Err(e) = app_handle.emit(constants::events::dictation::ACTIVE, false) {
        error!(
            "[Dictation Mode] Failed to emit dictation-active event: {}",
            e
        );
    }

    // Now finalize transcription (this is the slow part — it produces the text)
    if let Some(controller_state) = app_handle.try_state::<Arc<Mutex<VoiceController>>>() {
        let _ = tauri_plugin_voice_transcription::commands::stop_dictation(
            app_handle.clone(),
            controller_state,
        )
        .await;
    }

    // Bring the wake-phrase engine back if this dictation paused it and voice
    // is still wanted. Not from the remembered flag alone: that is how voice
    // switched off mid-dictation came back on (see the function).
    crate::commands::triggers::resume_voice_after_dictation(&app_handle).await;
}

async fn handle_force_stop_transcription(app_handle: AppHandle) {
    info!("[Event] Force stopping transcription");

    // A force path runs precisely when the rest of the state is not to be
    // trusted, so it is not gated on owning a session: it tears the controller
    // down either way. It does claim the session when there is one, because
    // this finalises the audio and an unowned transcript is dropped.
    let app_state = app_handle.state::<state::AppState>();
    match app_state.claim_voice_commit(SessionClaim::Current) {
        Ok(session) => info!(
            "[Force Stop] Finalising voice session {}",
            session.describe()
        ),
        Err(rejection) => warn!(
            "[Force Stop] Tearing down the controller with no session to claim: {}",
            rejection.reason()
        ),
    }

    if let Some(controller_state) = app_handle.try_state::<Arc<Mutex<VoiceController>>>() {
        let _ = tauri_plugin_voice_transcription::commands::stop_dictation(
            app_handle.clone(),
            controller_state,
        )
        .await;
    }
}

/// Handle timer-expired events with comprehensive processing
async fn handle_timer_expired_event(app_handle: AppHandle, payload: &str) {
    info!("[Timer Event] Received timer-expired event: {:?}", payload);

    // Parse timer data from payload
    let timer_data: TimerTask = match serde_json::from_str(payload) {
        Ok(data) => data,
        Err(e) => {
            error!("[Timer Event] Failed to parse timer-expired payload: {}", e);
            return;
        }
    };

    // Create timer event handler with default configuration
    let timer_handler = TimerEventHandler::new(app_handle.clone());

    // Process the timer expiration
    match timer_handler.handle_timer_expired(timer_data.clone()).await {
        Ok(()) => {
            info!(
                "[Timer Event] Successfully processed timer-expired event: {}",
                timer_data.id
            );

            // Emit success event for UI feedback
            if let Err(e) = app_handle.emit(
                constants::events::timer::PROCESSED,
                serde_json::json!({
                    "timer_id": timer_data.id,
                    "status": "success",
                    "description": timer_data.description
                }),
            ) {
                warn!(
                    "[Timer Event] Failed to emit timer-processed success event: {}",
                    e
                );
            }
        }
        Err(e) => {
            error!(
                "[Timer Event] Failed to process timer-expired event {}: {}",
                timer_data.id, e
            );

            // Emit error event for UI feedback
            if let Err(emit_err) = app_handle.emit(
                constants::events::timer::PROCESSED,
                serde_json::json!({
                    "timer_id": timer_data.id,
                    "status": "error",
                    "error": e.to_string(),
                    "description": timer_data.description
                }),
            ) {
                warn!(
                    "[Timer Event] Failed to emit timer-processed error event: {}",
                    emit_err
                );
            }
        }
    }
}

/// Handle timer-queued events (for monitoring and UI updates)
async fn handle_timer_queued_event(app_handle: AppHandle, payload: &str) {
    info!(
        "[Timer Event] Timer queued for later processing: {:?}",
        payload
    );

    // Parse timer data from payload
    let timer_data: TimerTask = match serde_json::from_str(payload) {
        Ok(data) => data,
        Err(e) => {
            warn!("[Timer Event] Failed to parse timer-queued payload: {}", e);
            return;
        }
    };

    // Emit UI event to show queued timer status
    if let Err(e) = app_handle.emit(
        constants::events::timer::STATUS_UPDATE,
        serde_json::json!({
            "timer_id": timer_data.id,
            "status": "queued",
            "description": timer_data.description,
            "queued_at": chrono::Utc::now().to_rfc3339()
        }),
    ) {
        warn!(
            "[Timer Event] Failed to emit timer-status-update event: {}",
            e
        );
    }
}

//! # Integration Module
//!
//! This module provides comprehensive integration patterns for the Juno application,
//! including component coordination, plugin setup, specialized event listeners,
//! and cross-module communication patterns.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::Instant;
use tauri::{AppHandle, Emitter, Listener, Manager};
use tauri_plugin_voice_transcription::controller::VoiceController;
use tracing::{error, info, warn};

use crate::constants::errors::{prefixes, templates};
use crate::constants::events;
use crate::format_error;
use crate::state::{SessionClaim, VoiceStartMethod, VoiceTarget};
use crate::state_management::CaptureBar;
use crate::utils::async_runtime::safe_spawn_async_task;
use crate::{commands, constants, state};

// Global deduplication cache for preventing duplicate agent submissions
lazy_static::lazy_static! {
    static ref SUBMISSION_CACHE: Arc<Mutex<HashMap<String, Instant>>> = Arc::new(Mutex::new(HashMap::new()));
}

// Check if a query submission is a duplicate within 1 second
fn is_duplicate_submission(query: &str) -> bool {
    let mut cache = match SUBMISSION_CACHE.lock() {
        Ok(guard) => guard,
        Err(poisoned) => {
            tracing::error!("Submission cache mutex was poisoned, recovering...");
            poisoned.into_inner()
        }
    };
    let now = Instant::now();

    // Clean up old entries (older than 5 seconds)
    cache.retain(|_, time| now.duration_since(*time).as_secs() < 5);

    // Check if this query was recently submitted
    if let Some(last_time) = cache.get(query) {
        if now.duration_since(*last_time).as_millis() < 1000 {
            warn!(
                "Duplicate query submission detected within 1 second, ignoring: '{}'",
                query
            );
            return true;
        }
    }

    // Record this submission
    cache.insert(query.to_string(), now);
    false
}

/// Setup comprehensive application integration including plugins, event coordination, and component initialization
pub fn setup_application_integration(app: &tauri::App) -> Result<(), Box<dyn std::error::Error>> {
    info!("🔧 Setting up application integration...");

    let app_handle = app.handle().clone();

    // Setup specialized voice transcription event listeners
    setup_specialized_voice_listeners(&app_handle);

    // Setup always listening integration
    setup_always_listening_integration(&app_handle);

    // Setup agent mode integration
    setup_agent_mode_integration(&app_handle);

    // Setup development mode integration (if debug build)
    #[cfg(debug_assertions)]
    setup_development_integration(&app_handle);

    // Boot sound is handled by app_setup module - removed duplicate call

    info!("✅ Application integration setup completed");
    Ok(())
}

/// Setup voice transcription event listeners for specialized behavior
fn setup_specialized_voice_listeners(app_handle: &AppHandle) {
    // Listen for voice transcription start events
    let app_handle_for_listener = app_handle.clone();
    app_handle.listen(
        constants::events::voice_transcription::DICTATION_STARTED,
        move |_event| {
            info!("[Event] Received voice-transcription:started event");
            let app_handle_clone = app_handle_for_listener.clone();
            safe_spawn_async_task(move || async move {
                // Check if Dictation Mode is active
                let app_state = app_handle_clone.state::<state::AppState>();
                let is_dictation_mode = app_state.is_dictation_active();

                // If it's dictation mode, set the flag in floating bar manager first
                if is_dictation_mode {
                    commands::ui_commands::handle_dictation_mode_change(&app_handle_clone, true)
                        .await;
                }

                // Then handle the dictation started event
                commands::ui_commands::handle_dictation_started(&app_handle_clone).await;
            });
        },
    );

    // Listen for app-dictation-finished events to trigger the agent (legacy UI path)
    let app_handle_for_agent_listener = app_handle.clone();
    app_handle.listen(constants::events::dictation::FINISHED, move |event| {
        info!("[Event] Received app-dictation-finished event - triggering agent");

        let app_handle_clone = app_handle_for_agent_listener.clone();
        safe_spawn_async_task(move || async move {
            // Parse the query from the event payload
            let payload_str = event.payload();
            match serde_json::from_str::<serde_json::Value>(payload_str) {
                Ok(payload_json) => {
                    // Images ride in the same payload, so a pasted picture
                    // reaches the model with the sentence it belongs to.
                    let images: Option<Vec<String>> = payload_json
                        .get("images")
                        .and_then(|v| v.as_array())
                        .map(|a| {
                            a.iter()
                                .filter_map(|v| v.as_str().map(str::to_string))
                                .collect()
                        })
                        .filter(|v: &Vec<String>| !v.is_empty());
                    if let Some(query_value) = payload_json.get("query") {
                        if let Some(query_text) = query_value.as_str() {
                            let trimmed_query = query_text.trim();
                            if !trimmed_query.is_empty() {
                                // Check for duplicate submission
                                if is_duplicate_submission(trimmed_query) {
                                    return;
                                }

                                info!("[Agent Mode] Submitting query to agent: '{}'", trimmed_query);

                                // Submit the query to the agent system
                                let app_handle_for_state = app_handle_clone.clone();
                                let app_state = app_handle_for_state.state::<crate::state::AppState>();

                                // Register escape key when agent processing starts
                                let coordinator = crate::commands::escape_key_coordinator::get_escape_key_coordinator();
                                if let Err(e) = coordinator.register_escape_user(&app_handle_clone, "agent_voice_input").await {
                                    warn!("[Agent Mode] Failed to register escape key for agent processing: {} - continuing without escape key cancellation", e);
                                }

                                // Submit the query to the agent system
                                let query_result = crate::anthropic::submit_query(
                                    trimmed_query.to_string(),
                                    images.clone(),
                                    app_state,
                                    app_handle_clone.clone()
                                ).await;

                                // Unregister escape key after agent completes (success or error)
                                let _ = coordinator.unregister_escape_user(&app_handle_clone, "agent_voice_input").await;

                                if let Err(e) = query_result {
                                    error!("[Agent Mode] Failed to submit query to agent: {}", e);
                                    crate::error_handling::utils::handle_agent_error(&app_handle_clone, &format!("Failed to submit query: {}", e)).await;
                                }
                            } else {
                                info!("[Agent Mode] Query text was empty - ignoring");
                            }
                        } else {
                            error!("[Agent Mode] Query field in payload is not a string: {:?}", query_value);
                        }
                    } else {
                        error!("[Agent Mode] No 'query' field found in app-dictation-finished payload: {}", payload_str);
                    }
                }
                Err(e) => {
                    error!("[Agent Mode] Failed to parse app-dictation-finished payload: {}", e);
                }
            }
        });
    });

    // Listen for unified agent query submission events (from agent mode voice transcription or UI)
    let app_handle_for_agent_query_listener = app_handle.clone();
    app_handle.listen(crate::constants::events::agent::QUERY_READY, move |event| {
        info!("[Event] Received agent-query-ready event - triggering agent from voice transcription");

        let app_handle_clone = app_handle_for_agent_query_listener.clone();
        safe_spawn_async_task(move || async move {
            // Parse the query from the event payload
            let payload_str = event.payload();
            match serde_json::from_str::<serde_json::Value>(payload_str) {
                Ok(payload_json) => {
                    if let Some(query_value) = payload_json.get("query") {
                        if let Some(query_text) = query_value.as_str() {
                            let trimmed_query = query_text.trim();
                            if !trimmed_query.is_empty() {
                                // Check for duplicate submission
                                if is_duplicate_submission(trimmed_query) {
                                    return;
                                }

                                info!("[Agent Mode] Submitting query to agent: '{}'", trimmed_query);

                                // Submit the query to the agent system
                                let app_handle_for_state = app_handle_clone.clone();
                                let app_state = app_handle_for_state.state::<crate::state::AppState>();

                                // Register escape key when agent processing starts
                                let coordinator = crate::commands::escape_key_coordinator::get_escape_key_coordinator();
                                if let Err(e) = coordinator.register_escape_user(&app_handle_clone, "agent_voice_input").await {
                                    warn!("[Agent Mode] Failed to register escape key for agent processing: {} - continuing without escape key cancellation", e);
                                }

                                // Submit the query to the agent system
                                let query_result = crate::anthropic::submit_query(
                                    trimmed_query.to_string(),
                                    None,
                                    app_state,
                                    app_handle_clone.clone()
                                ).await;

                                // Unregister escape key after agent completes (success or error)
                                let _ = coordinator.unregister_escape_user(&app_handle_clone, "agent_voice_input").await;

                                if let Err(e) = query_result {
                                    error!("[Agent Mode] Failed to submit query to agent: {}", e);
                                    crate::error_handling::utils::handle_agent_error(&app_handle_clone, &format!("Failed to submit query: {}", e)).await;
                                }
                            } else {
                                info!("[Agent Mode] Query text was empty - ignoring");
                            }
                        } else {
                            error!("[Agent Mode] Query field in payload is not a string: {:?}", query_value);
                        }
                    } else {
                        error!("[Agent Mode] No 'query' field found in agent-query-ready payload: {}", payload_str);
                    }
                }
                Err(e) => {
                    error!("[Agent Mode] Failed to parse agent-query-ready payload: {}", e);
                }
            }
        });
    });

    // Listen for voice transcription partial results
    let app_handle_for_listener = app_handle.clone();
    app_handle.listen(
        constants::events::voice_transcription::PARTIAL_RESULT,
        move |event| {
            info!(
                "[Event] Received voice-transcription:partial-result event: {:?}",
                event.payload()
            );

            // Extract partial text and update floating bar manager
            let payload_str = event.payload();
            if let Ok(payload_json) = serde_json::from_str::<serde_json::Value>(payload_str) {
                if let Some(text_value) = payload_json.get("text") {
                    if let Some(text) = text_value.as_str() {
                        let app_handle_clone = app_handle_for_listener.clone();
                        let partial_text = text.to_string();
                        safe_spawn_async_task(move || async move {
                            commands::ui_commands::handle_dictation_partial(
                                &app_handle_clone,
                                partial_text,
                            )
                            .await;
                        });
                    }
                }
            }
        },
    );

    // Setup force stop and cleanup event listeners
    setup_force_stop_listeners(app_handle);
}

/// Setup force stop and cleanup event listeners for voice transcription
fn setup_force_stop_listeners(app_handle: &AppHandle) {
    // Listen for force stop events (timeout/stuck transcription)
    let app_handle_for_force_stop = app_handle.clone();
    app_handle.listen(
        constants::events::dictation::TRANSCRIPTION_FORCE_STOP,
        move |_event| {
            warn!(
            "[Event] Received dictation-transcription-force-stop event - force stopping dictation"
        );

            let app_handle_clone = app_handle_for_force_stop.clone();
            safe_spawn_async_task(move || async move {
                handle_voice_controller_force_stop(&app_handle_clone).await;
            });
        },
    );

    // Listen for force cleanup events (stuck state recovery)
    let app_handle_for_force_cleanup = app_handle.clone();
    app_handle.listen(
        constants::events::dictation::TRANSCRIPTION_FORCE_CLEANUP,
        move |_event| {
            warn!(
            "[Event] Received dictation-transcription-force-cleanup event - recovering stuck state"
        );

            let app_handle_clone = app_handle_for_force_cleanup.clone();
            safe_spawn_async_task(move || async move {
                handle_dictation_state_cleanup(&app_handle_clone).await;
            });
        },
    );
}

/// Handle voice controller force stop with timeout protection
async fn handle_voice_controller_force_stop(app_handle: &AppHandle) {
    // Ungated, like every force path: it exists for when the state is already
    // wrong. Claiming matters because this finalises the audio, and a
    // transcript with no session to own it is dropped rather than delivered.
    let app_state = app_handle.state::<state::AppState>();
    match app_state.claim_voice_commit(SessionClaim::Current) {
        Ok(session) => info!(
            "[Dictation Mode] Force stop is finalising voice session {}",
            session.describe()
        ),
        Err(rejection) => warn!(
            "[Dictation Mode] Force stop with no session to claim: {}",
            rejection.reason()
        ),
    }

    // Force stop the voice controller with timeout only if it exists
    match app_handle.try_state::<Arc<Mutex<VoiceController>>>() {
        Some(controller_state) => {
            let stop_with_timeout = tokio::time::timeout(
                std::time::Duration::from_secs(2),
                tauri_plugin_voice_transcription::commands::stop_dictation(
                    app_handle.clone(),
                    controller_state,
                ),
            );

            match stop_with_timeout.await {
                Ok(Ok(_)) => {
                    info!("[Dictation Mode] Force stop completed successfully");
                }
                Ok(Err(e)) => {
                    error!("[Dictation Mode] Force stop failed: {}", e);
                }
                Err(_) => {
                    error!("[Dictation Mode] Force stop timed out - controller may be deadlocked");
                }
            }
        }
        None => {
            warn!("[Dictation Mode] Voice controller not available - cannot force stop");
        }
    }

    // Force clean up state
    if let Err(e) = app_state.set_dictation_active(false) {
        warn!("Failed to reset dictation active state: {}", e);
    }

    // Update floating bar manager
    let app_handle_for_bar = app_handle.clone();
    safe_spawn_async_task(move || async move {
        commands::ui_commands::handle_dictation_mode_change(&app_handle_for_bar, false).await;
    });

    if let Err(e) = app_handle.emit(constants::events::dictation::ACTIVE, false) {
        error!(
            "{} {}",
            prefixes::DICTATION_MODE,
            format_error(templates::FAILED_TO_EMIT, "dictation-active", e)
        );
    }
}

/// Handle dictation state cleanup for stuck state recovery
async fn handle_dictation_state_cleanup(app_handle: &AppHandle) {
    // Reset dictation input monitor state
    crate::dictation_monitor::force_reset_dictation_input_state().await;

    // Force clean up app state
    let app_state = app_handle.state::<state::AppState>();

    // Recovering from a stuck state means nothing that was open is still
    // wanted, so the session goes with it. Nothing here finalises audio, so
    // there is no transcript to keep an owner for.
    if let Ok(session) = app_state.claim_voice_discard(SessionClaim::Current) {
        warn!(
            "[Dictation Mode] Force cleanup is discarding voice session {}",
            session.describe()
        );
    }
    if let Err(e) = app_state.set_dictation_active(false) {
        warn!("Failed to reset dictation active state: {}", e);
    }

    // Update floating bar manager
    let app_handle_for_bar = app_handle.clone();
    safe_spawn_async_task(move || async move {
        commands::ui_commands::handle_dictation_mode_change(&app_handle_for_bar, false).await;
    });

    // Emit cleanup complete event
    if let Err(e) = app_handle.emit(constants::events::dictation::ACTIVE, false) {
        error!(
            "[Dictation Mode] Failed to emit dictation-active event: {}",
            e
        );
    }

    info!("[Dictation Mode] Force cleanup completed");
}

/// Which target the current wake-word activation should route to. Set when the
/// always-listening engine reports a matched phrase, read when the follow-up
/// transcription arrives. Defaults to the agent.
static PENDING_VOICE_TARGET: std::sync::Mutex<Option<crate::triggers::TriggerTarget>> =
    std::sync::Mutex::new(None);

/// Resolve which target a matched wake phrase belongs to by looking it up in the
/// enabled voice triggers. Falls back to the agent (historical behavior).
fn voice_target_for_phrase(
    app_state: &state::AppState,
    phrase: &str,
) -> crate::triggers::TriggerTarget {
    let phrase = phrase.trim().to_lowercase();
    app_state
        .get_triggers()
        .ok()
        .and_then(|triggers| {
            triggers
                .into_iter()
                .find(|t| {
                    t.enabled && t.is_voice() && t.voice_phrases().iter().any(|p| p == &phrase)
                })
                .map(|t| t.target)
        })
        .unwrap_or(crate::triggers::TriggerTarget::Agent)
}

/// Setup always listening integration with wake word detection and agent activation
fn setup_always_listening_integration(app_handle: &AppHandle) {
    info!("🔊 Setting up always listening integration...");

    // Listen for always listening wake word activation. The payload carries the
    // matched wake phrase, so we resolve which target (agent vs dictation) this
    // activation should drive and stash it for the follow-up transcription.
    let app_handle_for_wake_word = app_handle.clone();
    app_handle.listen(constants::events::always_listening::ACTIVATED, move |event| {
        let matched_phrase: String =
            serde_json::from_str(event.payload()).unwrap_or_default();

        let app_handle_clone = app_handle_for_wake_word.clone();
        safe_spawn_async_task(move || async move {
            // A gentle chime the moment the wake phrase lands, so Juno answers
            // "I'm listening" before a word of the request is spoken (Siri's
            // model: acknowledge, then capture).
            crate::commands::sound::play_cue(
                &app_handle_clone,
                crate::commands::sound::SoundType::NotificationAmbient,
            );

            let app_state = app_handle_clone.state::<state::AppState>();
            let target = voice_target_for_phrase(&app_state, &matched_phrase);
            if let Ok(mut pending) = PENDING_VOICE_TARGET.lock() {
                *pending = Some(target);
            }
            info!(
                "[AlwaysListening] Wake phrase '{}' detected -> target {:?}",
                matched_phrase, target
            );

            // Update floating bar to indicate activation is starting.
            commands::ui_commands::handle_always_listening_change(&app_handle_clone, true).await;

            // Tell the UI the phrase was heard. The payload carries the phrase
            // and the resolved target, because "a wake phrase was heard" and
            // "the engine is armed" are two different things to draw and the
            // bar was previously given the same signal for both.
            if let Err(e) = app_handle_clone.emit(
                events::always_listening::WAKE_WORD_DETECTED,
                serde_json::json!({
                    "phrase": matched_phrase,
                    "target": target,
                }),
            ) {
                error!("{} Failed to emit wake-word-detected event: {}", prefixes::ALWAYS_LISTENING, e);
            }

            info!("[AlwaysListening] Wake word activation handled - waiting for follow-up transcription");
        });
    });

    // Listen for always listening transcription results (after wake word)
    let app_handle_for_always_listening = app_handle.clone();
    app_handle.listen(
        constants::events::always_listening::TRANSCRIPTION,
        move |event| {
            info!(
                "[AlwaysListening] Received transcription after wake word: {:?}",
                event.payload()
            );

            let app_handle_clone = app_handle_for_always_listening.clone();
            safe_spawn_async_task(move || async move {
                handle_always_listening_transcription(&app_handle_clone, event.payload()).await;
            });
        },
    );

    // Setup always listening control listeners
    setup_always_listening_control_listeners(app_handle);
}

/// Handle always listening transcription results and agent activation
async fn handle_always_listening_transcription(app_handle: &AppHandle, payload_str: &str) {
    let app_state = app_handle.state::<state::AppState>();

    // Check if Dictation Mode is active - skip if so
    let is_dictation_active = app_state.is_dictation_active();

    if is_dictation_active {
        info!("[AlwaysListening] Dictation Mode is active - skipping agent activation");
        return;
    }

    // Parse the transcription result
    match serde_json::from_str::<serde_json::Value>(payload_str) {
        Ok(payload_json) => {
            if let Some(text_value) = payload_json.get("text") {
                if let Some(text) = text_value.as_str() {
                    let trimmed_text = text.trim();
                    info!(
                        "[AlwaysListening] Activating agent with query: '{}'",
                        trimmed_text
                    );

                    // Only act if we have meaningful content.
                    if !trimmed_text.is_empty() && trimmed_text.len() > 2 {
                        let target = PENDING_VOICE_TARGET
                            .lock()
                            .ok()
                            .and_then(|g| *g)
                            .unwrap_or(crate::triggers::TriggerTarget::Agent);

                        match target {
                            crate::triggers::TriggerTarget::Dictation => {
                                // Voice dictation: type what was said after the
                                // wake phrase, mirroring the dictation delivery path.
                                info!(
                                    "[AlwaysListening] Voice dictation -> typing: '{}'",
                                    trimmed_text
                                );
                                if let Err(e) = crate::commands::dictation::insert_dictation_text(
                                    app_handle,
                                    trimmed_text,
                                )
                                .await
                                {
                                    error!(
                                        "[AlwaysListening] Failed to insert dictated text: {}",
                                        e
                                    );
                                }
                            }
                            crate::triggers::TriggerTarget::Agent => {
                                // One voice turn at a time: drop a repeat of the
                                // same utterance inside 1s so a stray re-fire (or
                                // Juno catching a tail of her own prompt) can't
                                // stack a second agent run. The other submit paths
                                // already dedup; the always-listening path did not.
                                if is_duplicate_submission(trimmed_text) {
                                    info!(
                                        "[AlwaysListening] Duplicate voice query within 1s - ignoring: '{}'",
                                        trimmed_text
                                    );
                                } else if let Err(e) = crate::anthropic::submit_query(
                                    trimmed_text.to_string(),
                                    None,
                                    app_state,
                                    app_handle.clone(),
                                )
                                .await
                                {
                                    crate::error_handling::utils::log_and_emit_error(
                                        app_handle,
                                        "AlwaysListening",
                                        "agent_query_submission",
                                        &e.to_string(),
                                        true,
                                    );
                                }
                            }
                        }
                    } else {
                        info!("[AlwaysListening] Transcribed text was empty or too short - ignoring: '{}'", trimmed_text);
                    }
                } else {
                    warn!("[AlwaysListening] Text field in transcription payload is not a string");
                }
            } else {
                warn!("[AlwaysListening] No 'text' field found in transcription payload");
            }
        }
        Err(e) => {
            error!(
                "[AlwaysListening] Failed to parse transcription payload: {}",
                e
            );
        }
    }
}

/// Setup always listening control listeners for stop requests and mode management
fn setup_always_listening_control_listeners(app_handle: &AppHandle) {
    // Listen for always listening stop requests (from stop words)
    let app_handle_for_stop_request = app_handle.clone();
    app_handle.listen(
        constants::events::always_listening::STOP_REQUESTED,
        move |event| {
            info!(
                "[AlwaysListening] Received stop request: {:?}",
                event.payload()
            );

            // The stop-word payload carries the spoken text ({reason, text}); we
            // need it to tell "quit the app" from "stop listening".
            let spoken_text = serde_json::from_str::<serde_json::Value>(event.payload())
                .ok()
                .and_then(|v| v.get("text").and_then(|t| t.as_str()).map(String::from))
                .unwrap_or_default();

            let app_handle_clone = app_handle_for_stop_request.clone();
            safe_spawn_async_task(move || async move {
                handle_always_listening_stop_request(&app_handle_clone, &spoken_text).await;
            });
        },
    );

    // Listen for command processed events (to auto-stop or return to wake word mode)
    let app_handle_for_command_processed = app_handle.clone();
    app_handle.listen(
        constants::events::always_listening::COMMAND_PROCESSED,
        move |_event| {
            info!("[AlwaysListening] Command processed - considering auto-stop");

            let app_handle_clone = app_handle_for_command_processed.clone();
            safe_spawn_async_task(move || async move {
                handle_always_listening_command_processed(&app_handle_clone).await;
            });
        },
    );

    // Listen for return to wake word mode events
    let app_handle_for_wake_word_return = app_handle.clone();
    app_handle.listen(
        constants::events::always_listening::RETURN_TO_WAKE_WORD,
        move |_event| {
            info!("[AlwaysListening] Returning to wake word detection mode");

            let app_handle_clone = app_handle_for_wake_word_return.clone();
            safe_spawn_async_task(move || async move {
                handle_always_listening_return_to_wake_word(&app_handle_clone).await;
            });
        },
    );
}

/// Whether a post-wake command means "quit the whole app", as opposed to just
/// stopping listening. Quit on explicit quit words, or on "stop"/"close"/"bye"
/// only when Juno is named ("stop juno"). Bare "stop"/"cancel" must never be a
/// surprise app-quit, since they also mean "stop the agent" / "stop listening".
fn is_quit_app_command(text: &str) -> bool {
    let t = text.to_lowercase();
    let has_word = |w: &str| t.split_whitespace().any(|word| word == w);
    if has_word("quit")
        || has_word("exit")
        || has_word("goodbye")
        || has_word("shutdown")
        || t.contains("shut down")
    {
        return true;
    }
    // Addressed to Juno by name: "stop juno", "close juno", "bye juno".
    t.contains("juno") && (has_word("stop") || has_word("close") || has_word("bye"))
}

/// Handle always listening stop requests
async fn handle_always_listening_stop_request(app_handle: &AppHandle, spoken_text: &str) {
    // "Quit Juno" / "goodbye" / "shut down" (or a named "stop juno") ends the
    // app; a bare stop word only disarms listening (below).
    if is_quit_app_command(spoken_text) {
        info!(
            "[AlwaysListening] Quit command heard ('{}') - exiting Juno",
            spoken_text
        );
        app_handle.exit(0);
        return;
    }

    // Stop always listening mode
    let app_state = app_handle.state::<state::AppState>();
    match commands::always_listening::stop_always_listening_mode(app_handle.clone(), app_state)
        .await
    {
        Ok(_) => {
            info!("[AlwaysListening] Always listening stopped due to stop word");

            // Emit notification to UI
            if let Err(e) = app_handle.emit(events::always_listening::STOPPED_BY_COMMAND, ()) {
                error!(
                    "{} {}",
                    prefixes::ALWAYS_LISTENING,
                    format_error(templates::FAILED_TO_EMIT, "stopped-by-command", e)
                );
            }
        }
        Err(e) => {
            error!("[AlwaysListening] Failed to stop always listening: {}", e);
        }
    }
}

/// Handle always listening command processed events
async fn handle_always_listening_command_processed(app_handle: &AppHandle) {
    // Wait a bit for the command to complete processing
    tokio::time::sleep(tokio::time::Duration::from_millis(5000)).await;

    // Check if we should auto-stop always listening or return to wake word mode
    // For now, we'll return to wake word mode to allow for follow-up commands
    info!("[AlwaysListening] Returning to wake word detection mode after command processing");

    // Emit event to return to wake word mode
    if let Err(e) = app_handle.emit(events::always_listening::RETURN_TO_WAKE_WORD, ()) {
        error!(
            "{} {}",
            prefixes::ALWAYS_LISTENING,
            format_error(templates::FAILED_TO_EMIT, "return-to-wake-word", e)
        );
    }
}

/// Handle return to wake word mode
async fn handle_always_listening_return_to_wake_word(app_handle: &AppHandle) {
    // Update floating bar to indicate wake word mode
    commands::ui_commands::handle_always_listening_change(app_handle, false).await;

    // The always listening system will automatically return to monitoring mode
    // after processing the command, so we don't need to do anything else here
}

/// Setup agent mode integration with hold-based activation and transcription management
fn setup_agent_mode_integration(app_handle: &AppHandle) {
    info!("🤖 Setting up agent mode integration...");

    // Setup agent transcription event listeners
    setup_agent_transcription_listeners(app_handle);

    // Setup agent control event listeners
    setup_agent_control_listeners(app_handle);

    // Setup comprehensive agent stop event listener
    setup_agent_stop_all_listener(app_handle);
}

/// Setup agent transcription event listeners for hold mode
fn setup_agent_transcription_listeners(app_handle: &AppHandle) {
    // Listen for agent transcription start events (hold mode)
    let app_handle_for_agent_start = app_handle.clone();
    app_handle.listen(
        constants::events::agent::TRANSCRIPTION_START,
        move |event| {
            info!(
                "[Event] Received agent-transcription-start event - starting agent mode via hold"
            );

            let app_handle_clone = app_handle_for_agent_start.clone();
            // The payload says how this session was triggered, so the session
            // identity can record it instead of the stop paths inferring it.
            let method = VoiceStartMethod::from_event_payload(event.payload());
            safe_spawn_async_task(move || async move {
                handle_agent_transcription_start(&app_handle_clone, method).await;
            });
        },
    );

    // Listen for agent transcription stop events (hold mode - threshold reached)
    let app_handle_for_agent_transcription_stop = app_handle.clone();
    app_handle.listen(constants::events::agent::TRANSCRIPTION_STOP, move |_event| {
        info!("[Event] Received agent-transcription-stop event - stopping transcription to process result");

        let app_handle_clone = app_handle_for_agent_transcription_stop.clone();
        safe_spawn_async_task(move || async move {
            handle_agent_transcription_stop(&app_handle_clone).await;
        });
    });
}

/// Setup agent control event listeners for cancel and force stop
fn setup_agent_control_listeners(app_handle: &AppHandle) {
    // Note: agent-stop-all is handled by setup_agent_stop_all_listener() which does
    // comprehensive emergency cleanup. We don't register a duplicate listener here.

    // Listen for agent cancel events (hold mode - cancelled before threshold)
    let app_handle_for_agent_cancel = app_handle.clone();
    app_handle.listen(constants::events::agent::CANCEL, move |_event| {
        info!("[Event] Received agent-cancel event - cancelling agent mode via hold");

        let app_handle_clone = app_handle_for_agent_cancel.clone();
        safe_spawn_async_task(move || async move {
            handle_agent_cancel(&app_handle_clone).await;
        });
    });

    // Listen for agent force-stop events (hold mode - timeout or stuck)
    let app_handle_for_agent_force_stop = app_handle.clone();
    app_handle.listen(constants::events::agent::FORCE_STOP, move |_event| {
        info!("[Event] Received agent-force-stop event - force stopping agent mode");

        let app_handle_clone = app_handle_for_agent_force_stop.clone();
        safe_spawn_async_task(move || async move {
            handle_agent_force_stop(&app_handle_clone).await;
        });
    });
}

/// Setup comprehensive agent stop event listener for emergency situations
fn setup_agent_stop_all_listener(app_handle: &AppHandle) {
    // Listen for comprehensive agent-stop-all events (from stop button or emergency situations)
    let app_handle_for_agent_stop_all = app_handle.clone();
    app_handle.listen(constants::events::agent::STOP_ALL, move |_event| {
        info!("[Event] Received agent-stop-all event - performing comprehensive agent shutdown");

        let app_handle_clone = app_handle_for_agent_stop_all.clone();
        safe_spawn_async_task(move || async move {
            handle_agent_stop_all(&app_handle_clone).await;
        });
    });
}

/// Handle agent transcription start
///
/// Retries up to 3 times with a short delay to handle transient lock contention
/// (e.g., when stop_dictation from a prior operation is still releasing the lock).
async fn handle_agent_transcription_start(app_handle: &AppHandle, method: VoiceStartMethod) {
    const MAX_RETRIES: u32 = 3;
    const RETRY_DELAY_MS: u64 = 150;

    // Give the session an identity first, before the permission prompt and the
    // retries below, because a cancel can arrive during any of them. Once the
    // session exists, that cancel retires it and the checks further down see
    // that this start no longer owns anything.
    //
    // The registry refuses this if a session is already standing, which is the
    // whole of the double-press bug: the second press used to take ownership,
    // then fail at the audio engine with "Already dictating", then run a
    // teardown that put the tray, the capture flag and the input monitor back
    // to rest while the first session's microphone was still open.
    let app_state = app_handle.state::<state::AppState>();
    let session = match app_state.begin_voice_session(VoiceTarget::Agent, method) {
        Ok(session) => session,
        Err(refused) => {
            // A no-op, deliberately. The microphone is already open and a
            // second press of the same control means "I am already talking".
            // Ending the standing session here would throw away audio nobody
            // asked to throw away, and the engine would refuse the restart
            // anyway. The controls that really do mean "end this" are the bar's
            // stop and cancel, and they claim the session they end.
            info!("[Agent Mode] Ignoring an agent start: {}", refused.reason());
            // The bar-voice latch says "a bar-initiated agent query is open".
            // If what is standing is a dictation session it is not one, and
            // leaving the latch set would make the next activation edge end a
            // session the bar does not speak for. Same rule as the stop and
            // cancel paths below.
            if refused.standing.target == VoiceTarget::Dictation {
                crate::agent_monitor::set_bar_voice_active(false);
            }
            return;
        }
    };
    let session_id = session.id;
    info!("[Agent Mode] Opened voice session {}", session.describe());

    // Has this start been overtaken while it was waiting on something? Only a
    // stop or a cancel can take the session now, since a competing start is
    // refused rather than allowed to replace it.
    let still_ours = |app_handle: &AppHandle| {
        app_handle
            .state::<state::AppState>()
            .current_voice_session()
            .is_some_and(|session| session.id == session_id)
    };

    // Ask for the microphone before reaching for it. Without this the plugin
    // returns a denial as a string that ends up in a log file, the bar never
    // changes, and pressing the mic button looks like nothing happening at all.
    // Always answers, however many times the mic is pressed: this only runs
    // because a person pressed something, and the silent version of this is
    // exactly the bug.
    if let Err(message) = crate::permission_gate::require_for_user(
        app_handle,
        crate::permission_gate::Capability::Microphone,
        "listening",
    )
    .await
    {
        info!("[Agent Mode] Not starting dictation: {}", message);
        // The microphone never opened, so retire the identity we minted above
        // and unwind only what this start put up.
        abandon_agent_start(app_handle, session_id, &message).await;
        return;
    }

    // Capture the generation at the time this handler was dispatched.
    // If a cancel event fires while we're doing async work (permission checks, etc.),
    // the generation will change and we'll know to abort before starting dictation.
    let generation_at_start = crate::agent_monitor::current_agent_generation();

    // Verify voice controller is managed before entering retry loop
    if app_handle
        .try_state::<Arc<Mutex<VoiceController>>>()
        .is_none()
    {
        warn!("[Agent Mode] Voice controller not available - cannot start agent transcription");
        abandon_agent_start(
            app_handle,
            session_id,
            "the voice controller is not available",
        )
        .await;
        return;
    }

    let mut last_error = String::new();
    for attempt in 0..MAX_RETRIES {
        // Has this start been overtaken while we waited (a cancel during the
        // permission prompt, or between retries)? The session is the primary
        // answer: a cancel retires it, so it is no longer the current one.
        if !still_ours(app_handle) {
            info!(
                "[Agent Mode] Agent start aborted - voice session {} is no longer the open one",
                session_id
            );
            return;
        }

        // The generation counter still guards the sliver before the session
        // existed: a cancel emitted between the monitor's start event and the
        // line above finds no session to retire. Folding that in means minting
        // the session at the trigger edge, which lives in agent_monitor.
        if crate::agent_monitor::current_agent_generation() != generation_at_start {
            info!(
                "[Agent Mode] Agent start aborted - session was cancelled during startup (generation {} -> {})",
                generation_at_start,
                crate::agent_monitor::current_agent_generation()
            );
            let _ = app_state.claim_voice_discard(SessionClaim::Id(session_id));
            return;
        }

        // Re-acquire state each iteration to avoid lifetime issues with tauri::State
        let Some(controller_state) = app_handle.try_state::<Arc<Mutex<VoiceController>>>() else {
            last_error = "Voice controller disappeared unexpectedly".to_string();
            break;
        };
        match tauri_plugin_voice_transcription::commands::start_dictation(
            app_handle.clone(),
            controller_state,
        )
        .await
        {
            Ok(()) => {
                // Check again after the async start completed. If a cancel
                // arrived while we were inside start_dictation's async work
                // (mic permission check, Whisper init, etc.), close what we
                // just opened. Cancel means cancel, so the audio is discarded
                // rather than finalised: stopping it here would emit a final
                // result for a session nobody is waiting on.
                if !still_ours(app_handle)
                    || crate::agent_monitor::current_agent_generation() != generation_at_start
                {
                    info!(
                        "[Agent Mode] Voice session {} was cancelled during startup - closing the microphone we just opened",
                        session_id
                    );
                    let _ = app_state.claim_voice_discard(SessionClaim::Id(session_id));
                    if let Some(cs) = app_handle.try_state::<Arc<Mutex<VoiceController>>>() {
                        let _ = tauri_plugin_voice_transcription::commands::cancel_dictation(
                            app_handle.clone(),
                            cs,
                        )
                        .await;
                    }
                    return;
                }

                if attempt > 0 {
                    info!(
                        "[Agent Mode] Started agent transcription on retry {}",
                        attempt
                    );
                } else {
                    info!("[Agent Mode] Started agent transcription successfully");
                }

                // Register escape key so the user can cancel during the listening phase.
                // Without this, escape is not a registered global shortcut and pressing it
                // does nothing until the query is submitted to the agent.
                let coordinator =
                    crate::commands::escape_key_coordinator::get_escape_key_coordinator();
                if let Err(e) = coordinator
                    .register_escape_user(app_handle, "agent_transcription")
                    .await
                {
                    warn!("[Agent Mode] Failed to register escape key for agent transcription: {} - continuing anyway", e);
                }

                if let Err(e) = crate::state_management::handle_agent_capture_state_transition(
                    app_handle,
                    true,
                    CaptureBar::Follows,
                )
                .await
                {
                    error!(
                        "[Agent Mode] Failed to announce the agent capture phase: {}",
                        e
                    );
                }
                return;
            }
            Err(e) => {
                last_error = format!("{}", e);
                // Only retry on lock contention errors
                let is_lock_error =
                    last_error.contains("busy") || last_error.contains("Lock error");
                if is_lock_error && attempt + 1 < MAX_RETRIES {
                    info!(
                        "[Agent Mode] VoiceController busy, retrying in {}ms (attempt {}/{})",
                        RETRY_DELAY_MS,
                        attempt + 1,
                        MAX_RETRIES
                    );
                    tokio::time::sleep(std::time::Duration::from_millis(RETRY_DELAY_MS)).await;
                    continue;
                }
                break;
            }
        }
    }

    // All retries exhausted or non-retryable error. Nothing is recording, so
    // this session owns nothing and must not be left standing to claim the
    // next one's transcript.
    error!("[Agent Mode] Failed to start agent transcription: {last_error}");
    if !abandon_agent_start(app_handle, session_id, &last_error).await {
        // Someone else owns the microphone now, so there is nothing of ours to
        // report and nothing of theirs to interrupt with an error banner.
        return;
    }
    crate::error_handling::utils::handle_agent_error(
        app_handle,
        &format!("Failed to start agent transcription: {}", last_error),
    )
    .await;
}

/// Unwind a start that never got a microphone, and only what it put up.
///
/// The tray icon, the agent capture flag and the agent input monitor are
/// global: they describe whichever session is open, not the one that failed.
/// Resetting them on behalf of a start that no longer owns anything is how the
/// app came to say it was idle while another session's microphone was still
/// recording. The claim is the test, exactly as it is for a stop: if this start
/// is no longer the open session, somebody else is, and their state is not ours
/// to take down.
///
/// Answers whether this start still owned its session, so the caller knows
/// whether the failure is worth telling the person about.
async fn abandon_agent_start(
    app_handle: &AppHandle,
    session_id: crate::state::VoiceSessionId,
    reason: &str,
) -> bool {
    let app_state = app_handle.state::<state::AppState>();
    if let Err(rejection) = app_state.claim_voice_discard(SessionClaim::Id(session_id)) {
        info!(
            "[Agent Mode] Agent start #{session_id} gave up ({reason}), but it no longer owns the session ({}); leaving the open one alone",
            rejection.reason()
        );
        return false;
    }
    info!("[Agent Mode] Agent start #{session_id} gave up: {reason}");

    // Nothing owns the microphone now, so nothing may be left recording on it.
    close_unowned_audio_stream(app_handle).await;

    crate::agent_monitor::set_bar_voice_active(false);
    crate::agent_monitor::force_reset_agent_input_state().await;
    if let Err(e) = crate::state_management::handle_agent_capture_state_transition(
        app_handle,
        false,
        CaptureBar::Follows,
    )
    .await
    {
        error!(
            "[Agent Mode] Failed to close the agent capture phase after an abandoned start: {}",
            e
        );
    }
    true
}

/// Close a microphone that no voice session owns.
///
/// The registry is the only record of who owns the audio stream. A stream with
/// nothing standing in the registry can never be claimed: no stop will finalise
/// it, no cancel will discard it, and the partials it keeps producing drive a
/// bar for a session that no longer exists. Whatever it eventually transcribes
/// would be handed to whichever session happened to be open by then, which is
/// not the person who spoke it.
///
/// Discarded rather than stopped. A stop finalises, and a finalised transcript
/// is what gets submitted; nobody is waiting on this audio.
///
/// The registry is read again immediately before the cancel because a start can
/// register between the two reads. That start has not opened a microphone yet
/// (every start path registers before it touches the engine), so the second
/// read is what keeps this from closing a session that has only just begun.
pub(crate) async fn close_unowned_audio_stream(app_handle: &AppHandle) {
    let app_state = app_handle.state::<state::AppState>();
    if app_state.current_voice_session().is_some() {
        return;
    }

    let Some(controller_state) = app_handle.try_state::<Arc<Mutex<VoiceController>>>() else {
        return;
    };
    match tauri_plugin_voice_transcription::commands::get_dictation_status(controller_state).await {
        Ok(true) => {}
        Ok(false) => return,
        Err(e) => {
            warn!("[Voice] Could not tell whether the microphone is open: {e}");
            return;
        }
    }

    if app_state.current_voice_session().is_some() {
        return;
    }
    let Some(controller_state) = app_handle.try_state::<Arc<Mutex<VoiceController>>>() else {
        return;
    };
    warn!("[Voice] The microphone is open with no session to own it; discarding the audio rather than leaving it recording behind an idle bar");
    if let Err(e) = tauri_plugin_voice_transcription::commands::cancel_dictation(
        app_handle.clone(),
        controller_state,
    )
    .await
    {
        error!("[Voice] Failed to close the unowned microphone: {e}");
    }
}

/// Handle agent transcription stop (threshold reached)
///
/// Parallelizes screenshot capture with STT finalization (Whisper inference).
/// Both operations start at PTT release time and run concurrently:
/// - stop_dictation() blocks while Whisper processes the audio (~200-400ms)
/// - capture_screenshot_command() runs in parallel (~50-100ms)
///
/// The pre-captured screenshot is stored in AppState and consumed on the
/// agent's first `computer/screenshot` tool call, saving one full round-trip.
async fn handle_agent_transcription_stop(app_handle: &AppHandle) {
    let ptt_release_time = std::time::Instant::now();
    let app_state = app_handle.state::<state::AppState>();

    // The verb stays "commit", the session decides whose audio it commits. A
    // dictation finalised through here would have its text submitted to the
    // agent instead of typed, which is the first of the four bugs wearing a
    // different hat.
    if let Some(session) = app_state.current_voice_session() {
        if session.target == VoiceTarget::Dictation {
            info!(
                "[Agent Mode] The open session is {}; routing the stop there",
                session.describe()
            );
            // The bar-voice latch says "a bar-initiated agent query is open".
            // This session is not one, and leaving the latch set would make
            // the next activation edge end a session that was never there.
            crate::agent_monitor::set_bar_voice_active(false);
            if let Err(e) = app_handle.emit(constants::events::dictation::STOP, ()) {
                error!("[Agent Mode] Failed to hand the stop to dictation: {}", e);
            }
            return;
        }
    }

    // Claim before anything else. A stop for a session that is already gone
    // does nothing at all: this is the push-to-talk resurrection, where a key
    // released after its session had been cancelled emitted a stop that
    // finalised and resubmitted it.
    match app_state.claim_voice_commit(SessionClaim::Current) {
        Ok(session) => info!(
            "[Agent Mode] Committing voice session {}",
            session.describe()
        ),
        Err(rejection) => {
            info!("[Agent Mode] Nothing to stop: {}", rejection.reason());
            // The bar flag is cleared even so: it is a UI latch, not the
            // session, and leaving it set would swallow the next activation.
            crate::agent_monitor::set_bar_voice_active(false);
            return;
        }
    }

    // Any way this session ends (bar Stop, the agent or dictation shortcut,
    // a hotkey release) lands here — clear the bar-voice flag and show a
    // processing state immediately while STT finalizes and the agent starts.
    crate::agent_monitor::set_bar_voice_active(false);
    crate::commands::ui_commands::handle_dictation_partial(app_handle, String::new()).await;

    match app_handle.try_state::<Arc<Mutex<VoiceController>>>() {
        Some(controller_state) => {
            // Spawn screenshot capture concurrently with STT finalization.
            // The screenshot is captured at PTT release time, matching the screen
            // state the user saw when they spoke.
            let app_for_screenshot = app_handle.clone();
            let screenshot_task = tauri::async_runtime::spawn(async move {
                let state = app_for_screenshot.state::<state::AppState>();
                crate::commands::core::capture_screenshot_command(app_for_screenshot.clone(), state)
                    .await
            });

            // STT finalization: blocks while Whisper runs inference on the recorded audio.
            // This will emit voice-transcription:final-result when complete.
            let stt_result = tauri_plugin_voice_transcription::commands::stop_dictation(
                app_handle.clone(),
                controller_state,
            )
            .await;

            // Collect screenshot result (it runs concurrently so it should already be done).
            let screenshot_elapsed = ptt_release_time.elapsed().as_millis();
            match screenshot_task.await {
                Ok(Ok(screenshot)) => {
                    app_state.set_pending_ptt_screenshot(screenshot).await;
                    info!(
                        "[PTT Parallel] Screenshot captured and cached in {}ms total elapsed (STT + screenshot ran concurrently)",
                        screenshot_elapsed
                    );
                }
                Ok(Err(e)) => {
                    warn!("[PTT Parallel] Screenshot capture failed ({}ms elapsed): {} — agent will capture on demand", screenshot_elapsed, e);
                }
                Err(e) => {
                    warn!("[PTT Parallel] Screenshot task panicked ({}ms elapsed): {} — agent will capture on demand", screenshot_elapsed, e);
                }
            }

            match stt_result {
                Ok(_) => {
                    info!(
                        "[Agent Mode] STT finalization complete in {}ms — final result will be processed",
                        ptt_release_time.elapsed().as_millis()
                    );

                    // Unregister the escape key we registered during transcription start.
                    // The agent query submission path will re-register its own escape user.
                    let coordinator =
                        crate::commands::escape_key_coordinator::get_escape_key_coordinator();
                    let _ = coordinator
                        .unregister_escape_user(app_handle, "agent_transcription")
                        .await;

                    // The microphone is shut, so the capture phase is over even
                    // though the interaction is not: the words it captured are on
                    // their way to the agent. This is the one success path out of
                    // capture, and while capture and execution shared an event it
                    // was silent, because the `agent-active = true` from the start
                    // of capture was simply left standing until the run ended. A
                    // flag left standing the same way would pin the menu bar to the
                    // agent icon for the rest of the session, so say it here. The
                    // bar keeps whatever the submission has already put there.
                    if let Err(e) = crate::state_management::handle_agent_capture_state_transition(
                        app_handle,
                        false,
                        CaptureBar::Untouched,
                    )
                    .await
                    {
                        error!(
                            "[Agent Mode] Failed to close the agent capture phase after a successful stop: {}",
                            e
                        );
                    }
                }
                Err(e) => {
                    error!("[Agent Mode] Failed to stop transcription: {}", e);

                    // STT failed — no query will be submitted so discard the cached screenshot.
                    app_state.take_pending_ptt_screenshot().await;

                    // No transcript is coming, so close the session out. Left
                    // finishing, it would claim the next session's text.
                    let _ = app_state.claim_voice_discard(SessionClaim::Current);

                    let coordinator =
                        crate::commands::escape_key_coordinator::get_escape_key_coordinator();
                    let _ = coordinator
                        .unregister_escape_user(app_handle, "agent_transcription")
                        .await;

                    crate::agent_monitor::force_reset_agent_input_state().await;

                    if let Err(e) = crate::state_management::handle_agent_capture_state_transition(
                        app_handle,
                        false,
                        CaptureBar::Follows,
                    )
                    .await
                    {
                        error!("[Agent Mode] Failed to close the agent capture phase after a transcription stop failure: {}", e);
                    }
                }
            }
        }
        None => {
            warn!("[Agent Mode] Voice controller not available - cannot stop transcription");

            // The session we just committed can produce nothing without a
            // controller, so it should not stay registered.
            let _ = app_state.claim_voice_discard(SessionClaim::Current);

            let coordinator = crate::commands::escape_key_coordinator::get_escape_key_coordinator();
            let _ = coordinator
                .unregister_escape_user(app_handle, "agent_transcription")
                .await;

            crate::agent_monitor::force_reset_agent_input_state().await;

            if let Err(e) = crate::state_management::handle_agent_capture_state_transition(
                app_handle,
                false,
                CaptureBar::Follows,
            )
            .await
            {
                error!(
                    "[Agent Mode] Failed to announce the agent capture phase: {}",
                    e
                );
            }
        }
    }
}

/// Handle agent cancel (cancelled before threshold)
async fn handle_agent_cancel(app_handle: &AppHandle) {
    let app_state = app_handle.state::<state::AppState>();

    // The verb stays "discard", the session decides whose audio it discards.
    // This is what the bar's X used to get wrong: it spoke for the agent
    // whatever was open, so a dictation started from the keyboard was silenced
    // at the controller while the dictation state machine, its monitor and its
    // escape registration all still believed a session was running. Hand it to
    // the path that owns it and let that unwind itself.
    if let Some(session) = app_state.current_voice_session() {
        if session.target == VoiceTarget::Dictation {
            info!(
                "[Agent Mode] The open session is {}; routing the cancel there",
                session.describe()
            );
            // Same reason as the stop path: the latch is an agent-side flag
            // and this session is not the agent's.
            crate::agent_monitor::set_bar_voice_active(false);
            if let Err(e) = app_handle.emit(constants::events::dictation::TRANSCRIPTION_CANCEL, ())
            {
                error!("[Agent Mode] Failed to hand the cancel to dictation: {}", e);
            }
            return;
        }
    }

    let claimed = match app_state.claim_voice_discard(SessionClaim::Current) {
        Ok(session) => {
            info!(
                "[Agent Mode] Discarding voice session {}",
                session.describe()
            );
            true
        }
        Err(rejection) => {
            // Deliberately not an early return. The cleanup below is
            // idempotent, and the input-monitor reset in particular must not
            // depend on the registry: a cancel that raced ahead of the start
            // is exactly when a monitor is left mid-hold.
            info!(
                "[Agent Mode] No audio to discard ({}); running cleanup only",
                rejection.reason()
            );
            false
        }
    };

    crate::agent_monitor::set_bar_voice_active(false);
    // Unregister escape key registered during transcription start
    let coordinator = crate::commands::escape_key_coordinator::get_escape_key_coordinator();
    let _ = coordinator
        .unregister_escape_user(app_handle, "agent_transcription")
        .await;

    if !claimed {
        // Nothing of ours is recording, so put the input monitor and the bar
        // back to rest without finalising anything.
        //
        // "Nothing of ours" is not the same as "nothing at all", though. If the
        // engine is still recording with an empty registry, that stream belongs
        // to no session and no later stop or cancel can reach it, so this is
        // the moment to close it. A cancel that merely raced ahead of a start
        // is unaffected: a start registers its session before it opens the
        // microphone, so there is nothing recording yet to close.
        close_unowned_audio_stream(app_handle).await;
        crate::agent_monitor::force_reset_agent_input_state().await;
        crate::commands::ui_commands::handle_dictation_finished(app_handle, None).await;
        if let Err(e) = crate::state_management::handle_agent_capture_state_transition(
            app_handle,
            false,
            CaptureBar::Follows,
        )
        .await
        {
            error!(
                "[Agent Mode] Failed to close the agent capture phase after an unowned cancel: {}",
                e
            );
        }
        return;
    }

    // Cancel means cancel. This used to call stop_dictation, which finalises
    // the audio and emits a final result, which downstream submits the query.
    // Everything named "cancel" in this stack sent what the person said.
    match app_handle.try_state::<Arc<Mutex<VoiceController>>>() {
        Some(controller_state) => {
            match tauri_plugin_voice_transcription::commands::cancel_dictation(
                app_handle.clone(),
                controller_state,
            )
            .await
            {
                Ok(_) => {
                    info!("[Agent Mode] Cancelled agent transcription successfully");
                    // Retract the input monitor's intent, not just the audio.
                    // Cancelling stopped the recording but left AGENT_INPUT_STATE
                    // believing a hold was still in progress, so releasing a
                    // still-held push-to-talk key saw "started and past the
                    // threshold" and emitted a stop, which finalised and
                    // submitted the session that had just been cancelled. The
                    // two failure branches below already reset it; the success
                    // path is the one that needed it most.
                    crate::agent_monitor::force_reset_agent_input_state().await;
                    // Put the bar back to rest; nothing is being processed.
                    // No query: the bar goes back to rest rather than to
                    // a processing state for something that will never arrive.
                    crate::commands::ui_commands::handle_dictation_finished(app_handle, None).await;

                    // Close the capture phase: the microphone is shut, so the bar goes
                    // back to rest and the flag and its event move together.
                    if let Err(e) = crate::state_management::handle_agent_capture_state_transition(
                        app_handle,
                        false,
                        CaptureBar::Follows,
                    )
                    .await
                    {
                        error!(
                            "[Agent Mode] Failed to announce the agent capture phase: {}",
                            e
                        );
                    }
                }
                Err(e) => {
                    error!("[Agent Mode] Failed to cancel agent transcription: {}", e);

                    // Force reset agent input monitor state on failure
                    crate::agent_monitor::force_reset_agent_input_state().await;

                    // Close the capture phase: the microphone is shut, so the bar goes
                    // back to rest and the flag and its event move together.
                    if let Err(e) = crate::state_management::handle_agent_capture_state_transition(
                        app_handle,
                        false,
                        CaptureBar::Follows,
                    )
                    .await
                    {
                        error!("[Agent Mode] Failed to close the agent capture phase after a cancel failure: {}", e);
                    }
                }
            }
        }
        None => {
            warn!(
                "[Agent Mode] Voice controller not available - cannot cancel agent transcription"
            );

            // Reset agent input monitor state
            crate::agent_monitor::force_reset_agent_input_state().await;

            // Close the capture phase: the microphone is shut, so the bar goes
            // back to rest and the flag and its event move together.
            if let Err(e) = crate::state_management::handle_agent_capture_state_transition(
                app_handle,
                false,
                CaptureBar::Follows,
            )
            .await
            {
                error!(
                    "[Agent Mode] Failed to announce the agent capture phase: {}",
                    e
                );
            }
        }
    }
}

/// Handle agent force stop (timeout or stuck)
async fn handle_agent_force_stop(app_handle: &AppHandle) {
    // A force path is for when the rest of the state cannot be trusted, so it
    // is not gated on owning a session. It still claims one when there is one,
    // because this finalises the audio and an unowned transcript is dropped.
    let app_state = app_handle.state::<state::AppState>();
    let owned = match app_state.claim_voice_commit(SessionClaim::Current) {
        Ok(session) => {
            info!(
                "[Agent Mode] Force stop is finalising voice session {}",
                session.describe()
            );
            true
        }
        Err(rejection) => {
            warn!(
                "[Agent Mode] Force stop with no session to claim: {}",
                rejection.reason()
            );
            false
        }
    };

    // Unregister escape key registered during transcription start
    let coordinator = crate::commands::escape_key_coordinator::get_escape_key_coordinator();
    let _ = coordinator
        .unregister_escape_user(app_handle, "agent_transcription")
        .await;

    // Force stop agent mode. A session that was claimed is finalised, because
    // somebody was waiting on what they said. Audio with no session to claim is
    // discarded instead: finalising it produces a transcript whose only route is
    // the agent, and nobody asked for it. The far end drops an unowned
    // transcript too, but not producing one is the better place to stop.
    match app_handle.try_state::<Arc<Mutex<VoiceController>>>() {
        Some(controller_state) => {
            if owned {
                let _ = tauri_plugin_voice_transcription::commands::stop_dictation(
                    app_handle.clone(),
                    controller_state,
                )
                .await;
            } else {
                let _ = tauri_plugin_voice_transcription::commands::cancel_dictation(
                    app_handle.clone(),
                    controller_state,
                )
                .await;
            }
        }
        None => {
            warn!("[Agent Mode] Voice controller not available during force stop");
        }
    }

    // Reset agent input monitor state
    crate::agent_monitor::force_reset_agent_input_state().await;

    // Close the capture phase: the microphone is shut, so the bar goes back to
    // rest and the flag and its event move together.
    if let Err(e) = crate::state_management::handle_agent_capture_state_transition(
        app_handle,
        false,
        CaptureBar::Follows,
    )
    .await
    {
        error!(
            "[Agent Mode] Failed to close the agent capture phase during a force stop: {}",
            e
        );
    }

    info!("[Agent Mode] Force stopped agent mode successfully");
}

/// Handle comprehensive agent stop all (emergency situations)
async fn handle_agent_stop_all(app_handle: &AppHandle) {
    // An emergency stop means nothing that is open should be delivered, so
    // retire the session before the cleanup below runs. `handle_voice_error`
    // further down still calls stop_dictation, which finalises the audio; with
    // no session standing, whatever text that produces has no owner and is not
    // typed or submitted. See the report note on error_handling.rs.
    let app_state = app_handle.state::<state::AppState>();
    if let Ok(session) = app_state.claim_voice_discard(SessionClaim::Current) {
        warn!(
            "[Agent Stop All] Discarding voice session {}",
            session.describe()
        );
    }

    // Use state management module for emergency cleanup
    if let Err(e) = crate::state_management::handle_emergency_state_cleanup(app_handle).await {
        error!("[Agent Stop All] Emergency cleanup failed: {}", e);
    }

    // Force stop voice transcription using centralized error handling
    crate::error_handling::utils::handle_voice_error(app_handle, "Emergency stop requested").await;

    info!("[Agent Stop All] Comprehensive agent shutdown completed");
}

/// Setup development mode integration (debug builds only)
#[cfg(debug_assertions)]
fn setup_development_integration(app_handle: &AppHandle) {
    info!("🛠️ Setting up development mode integration...");

    // Listen for frontend reload events and cleanup resources (development mode)
    let app_handle_for_frontend_reload = app_handle.clone();
    app_handle.listen("frontend-reload", move |_event| {
        info!("🔄 Frontend reload detected - cleaning up resources...");

        let app_handle_clone = app_handle_for_frontend_reload.clone();
        safe_spawn_async_task(move || async move {
            // Cleanup MCP servers to prevent accumulation
            if let Some(state) = app_handle_clone.try_state::<crate::state::AppState>() {
                if let Err(e) = state.cleanup_mcp_resources().await {
                    error!("Failed to cleanup MCP resources: {}", e);
                } else {
                    info!("✅ MCP resources cleaned up successfully");
                }
            }

            info!("✅ Development cleanup completed");
        });
    });

    info!("🛠️ Development mode cleanup handlers installed");
}

// Boot sound function removed - handled by app_setup module

/// Utility functions for component coordination and integration patterns
pub mod utils {
    use super::*;

    /// Synchronize component state changes across multiple systems
    pub async fn synchronize_component_state(
        app_handle: &AppHandle,
        component: &str,
        new_state: bool,
        emit_event: Option<&str>,
    ) -> Result<(), String> {
        info!(
            "🔄 Synchronizing state change for {}: {}",
            component, new_state
        );

        // Update floating bar manager if applicable
        match component {
            "dictation" => {
                commands::ui_commands::handle_dictation_mode_change(app_handle, new_state).await;
            }
            // The agent arm moves only the bar. Both halves of the agent
            // lifecycle now own a flag as well as an event, and a flag written
            // apart from its announcement is what stranded the menu bar the
            // last two times, so go through
            // `handle_agent_capture_state_transition` or
            // `handle_agent_execution_state_transition` instead of asking this
            // function to emit an agent event for you.
            "agent" => {
                if new_state {
                    commands::ui_commands::handle_agent_started(app_handle).await;
                } else {
                    commands::ui_commands::handle_agent_stopped(app_handle).await;
                }
            }
            "always_listening" => {
                commands::ui_commands::handle_always_listening_change(app_handle, new_state).await;
            }
            _ => {
                warn!("Unknown component for state coordination: {}", component);
            }
        }

        // Emit event if specified
        if let Some(event_name) = emit_event {
            if let Err(e) = app_handle.emit(event_name, new_state) {
                error!(
                    "{}",
                    format_error(templates::FAILED_TO_EMIT, event_name, &e)
                );
                return Err(format_error(templates::FAILED_TO_EMIT, "event", e));
            }
        }

        Ok(())
    }

    /// Validate component integration health
    pub fn validate_integration_health(
        app_handle: &AppHandle,
    ) -> Result<serde_json::Value, String> {
        info!("🔍 Validating integration health...");

        let mut health_report = serde_json::Map::new();

        // Check voice controller availability
        let voice_controller_available = app_handle
            .try_state::<Arc<Mutex<VoiceController>>>()
            .is_some();
        health_report.insert(
            "voice_controller".to_string(),
            serde_json::Value::Bool(voice_controller_available),
        );

        // Check app state availability
        let app_state_available = app_handle.try_state::<crate::state::AppState>().is_some();
        health_report.insert(
            "app_state".to_string(),
            serde_json::Value::Bool(app_state_available),
        );

        // Check if main components are responsive
        health_report.insert(
            "integration_status".to_string(),
            serde_json::Value::String("healthy".to_string()),
        );

        Ok(serde_json::Value::Object(health_report))
    }
}

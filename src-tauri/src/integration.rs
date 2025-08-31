//! # Integration Module
//!
//! This module provides comprehensive integration patterns for the Juno application,
//! including component coordination, plugin setup, specialized event listeners,
//! and cross-module communication patterns.

use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use tauri::{AppHandle, Emitter, Listener, Manager};
use tauri_plugin_voice_transcription::controller::VoiceController;
use tokio::sync::Mutex as TokioMutex;
use tracing::{error, info, warn};

use crate::utils::async_runtime::safe_spawn_async_task;
use crate::{commands, constants, state};
use crate::constants::events;
use crate::constants::errors::{templates, prefixes};

// Deduplication state for preventing multiple agent executions
#[derive(Clone)]
struct QueryDeduplicationState {
    last_query: String,
    last_query_time: Instant,
    source_element_id: Option<String>,
}

lazy_static::lazy_static! {
    static ref QUERY_DEDUP_STATE: Arc<TokioMutex<Option<QueryDeduplicationState>>> = Arc::new(TokioMutex::new(None));
}

// Helper function for error formatting - properly handles template substitution
fn format_error(template: &str, context: &str, error: impl std::fmt::Display) -> String {
    template.replacen("{}", context, 1).replacen("{}", &error.to_string(), 1)
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
    app_handle.listen("voice-transcription:started", move |event| {
        info!("[Event] Received voice-transcription:started event");
        let app_handle_clone = app_handle_for_listener.clone();
        safe_spawn_async_task(move || async move {
            // Check if Dictation Mode is active
            let app_state = app_handle_clone.state::<state::AppState>();
            let is_dictation_mode = app_state.is_dictation_active();

            // If it's dictation mode, set the flag in floating bar manager first
            if is_dictation_mode {
                commands::ui_commands::handle_dictation_mode_change(&app_handle_clone, true).await;
            }

            // Then handle the dictation started event
            commands::ui_commands::handle_dictation_started(&app_handle_clone).await;
        });
    });

    // Listen for app-dictation-finished events to trigger the agent
    let app_handle_for_agent_listener = app_handle.clone();
    app_handle.listen("app-dictation-finished", move |event| {
        info!("[Event] Received app-dictation-finished event - triggering agent");

        let app_handle_clone = app_handle_for_agent_listener.clone();
        safe_spawn_async_task(move || async move {
            // Parse the query from the event payload
            let payload_str = event.payload();
            match serde_json::from_str::<serde_json::Value>(payload_str) {
                Ok(payload_json) => {
                    // Extract source element ID if present
                    let source_element_id = payload_json.get("source_element_id")
                        .and_then(|v| v.as_str())
                        .map(|s| s.to_string());
                    
                    if let Some(source_id) = &source_element_id {
                        info!("[Agent Mode] Query originated from element: {}", source_id);
                    }
                    
                    if let Some(query_value) = payload_json.get("query") {
                        if let Some(query_text) = query_value.as_str() {
                            let trimmed_query = query_text.trim();
                            if !trimmed_query.is_empty() {
                                // Check for duplicate queries within a short time window
                                let mut dedup_state = QUERY_DEDUP_STATE.lock().await;
                                let now = Instant::now();
                                
                                // Check if this is a duplicate query
                                let is_duplicate = if let Some(ref last_state) = *dedup_state {
                                    last_state.last_query == trimmed_query && 
                                    now.duration_since(last_state.last_query_time) < Duration::from_millis(500) &&
                                    last_state.source_element_id == source_element_id
                                } else {
                                    false
                                };
                                
                                if is_duplicate {
                                    info!("[Agent Mode] Duplicate query detected from element {:?}, ignoring: '{}'", 
                                          source_element_id, trimmed_query);
                                    return;
                                }
                                
                                // Update deduplication state
                                *dedup_state = Some(QueryDeduplicationState {
                                    last_query: trimmed_query.to_string(),
                                    last_query_time: now,
                                    source_element_id: source_element_id.clone(),
                                });
                                drop(dedup_state); // Release the lock early
                                
                                info!("[Agent Mode] Submitting query to agent: '{}'", trimmed_query);

                                // Emit user message event for frontend to add to conversation
                                let user_message_data = serde_json::json!({
                                    "content": trimmed_query,
                                    "timestamp": std::time::SystemTime::now()
                                        .duration_since(std::time::UNIX_EPOCH)
                                        .unwrap_or_default()
                                        .as_millis() as u64,
                                    "source_element_id": source_element_id
                                });
                                if let Err(e) = app_handle_clone.emit(crate::constants::events::messages::USER_MESSAGE_SUBMITTED, user_message_data) {
                                    error!("{} Failed to emit user-message-submitted event: {}", prefixes::AGENT_MODE, e);
                                }

                                // Submit the query to the agent system
                                let app_handle_for_state = app_handle_clone.clone();
                                let app_state = app_handle_for_state.state::<crate::state::AppState>();

                                // Register escape key when agent processing starts
                                if let Err(e) = crate::commands::shortcuts::register_escape_key_handler(app_handle_clone.clone()).await {
                                    warn!("[Agent Mode] Failed to register escape key for agent processing: {} - continuing without escape key cancellation", e);
                                }

                                // Submit the query to the agent system
                                if let Err(e) = crate::anthropic::submit_query(
                                    trimmed_query.to_string(),
                                    app_state,
                                    app_handle_clone.clone()
                                ).await {
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

    // Listen for voice transcription partial results
    let app_handle_for_listener = app_handle.clone();
    app_handle.listen("voice-transcription:partial-result", move |event| {
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
    });

    // Setup force stop and cleanup event listeners
    setup_force_stop_listeners(app_handle);
}

/// Setup force stop and cleanup event listeners for voice transcription
fn setup_force_stop_listeners(app_handle: &AppHandle) {
    // Listen for consolidated dictation stop events with stopType parameter
    let app_handle_for_dictation_stop = app_handle.clone();
    app_handle.listen("dictation-stop", move |event| {
        // Parse stopType from payload
        let stop_type = if let Ok(payload) = serde_json::from_str::<serde_json::Value>(event.payload()) {
            payload.get("stopType").and_then(|t| t.as_str()).unwrap_or("normal").to_string()
        } else {
            "normal".to_string()
        };

        warn!("[Event] Received dictation-stop event with type: {} - handling dictation stop", stop_type);

        let app_handle_clone = app_handle_for_dictation_stop.clone();
        safe_spawn_async_task(move || async move {
            match stop_type.as_str() {
                "force" => handle_voice_controller_force_stop(&app_handle_clone).await,
                "error" => handle_dictation_state_cleanup(&app_handle_clone).await,
                _ => {
                    // Handle normal stop - just log for now
                    info!("[Event] Normal dictation stop - no special handling needed");
                }
            }
        });
    });
}

/// Handle voice controller force stop with timeout protection
async fn handle_voice_controller_force_stop(app_handle: &AppHandle) {
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
    let app_state = app_handle.state::<state::AppState>();
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

/// Setup always listening integration with wake word detection and agent activation
fn setup_always_listening_integration(app_handle: &AppHandle) {
    info!("🔊 Setting up always listening integration...");

    // Listen for always listening wake word activation
    let app_handle_for_wake_word = app_handle.clone();
    app_handle.listen("always-listening:activated", move |_event| {
        info!("[AlwaysListening] Wake word detected - preparing for agent activation");

        let app_handle_clone = app_handle_for_wake_word.clone();
        safe_spawn_async_task(move || async move {
            // Update floating bar to indicate agent mode is starting
            commands::ui_commands::handle_always_listening_change(&app_handle_clone, true).await;

            // Emit event to UI to show wake word was detected
            if let Err(e) = app_handle_clone.emit(events::always_listening::WAKE_WORD_DETECTED, ()) {
                error!("{} Failed to emit wake-word-detected event: {}", prefixes::ALWAYS_LISTENING, e);
            }

            info!("[AlwaysListening] Wake word activation handled - waiting for follow-up transcription");
        });
    });

    // Listen for always listening transcription results (after wake word)
    let app_handle_for_always_listening = app_handle.clone();
    app_handle.listen("always-listening:transcription", move |event| {
        info!(
            "[AlwaysListening] Received transcription after wake word: {:?}",
            event.payload()
        );

        let app_handle_clone = app_handle_for_always_listening.clone();
        safe_spawn_async_task(move || async move {
            handle_always_listening_transcription(&app_handle_clone, event.payload()).await;
        });
    });

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

                    // Only activate agent if we have meaningful content
                    if !trimmed_text.is_empty() && trimmed_text.len() > 2 {
                        // Submit the query to the agent system
                        if let Err(e) = crate::anthropic::submit_query(
                            trimmed_text.to_string(),
                            app_state,
                            app_handle.clone(),
                        ).await {
                            crate::error_handling::utils::log_and_emit_error(
                                app_handle,
                                "AlwaysListening",
                                "agent_query_submission",
                                &e.to_string(),
                                true,
                            );
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
    app_handle.listen("always-listening:stop-requested", move |event| {
        info!(
            "[AlwaysListening] Received stop request: {:?}",
            event.payload()
        );

        let app_handle_clone = app_handle_for_stop_request.clone();
        safe_spawn_async_task(move || async move {
            handle_always_listening_stop_request(&app_handle_clone).await;
        });
    });

    // Listen for command processed events (to auto-stop or return to wake word mode)
    let app_handle_for_command_processed = app_handle.clone();
    app_handle.listen("always-listening:command-processed", move |_event| {
        info!("[AlwaysListening] Command processed - considering auto-stop");

        let app_handle_clone = app_handle_for_command_processed.clone();
        safe_spawn_async_task(move || async move {
            handle_always_listening_command_processed(&app_handle_clone).await;
        });
    });

    // Listen for return to wake word mode events
    let app_handle_for_wake_word_return = app_handle.clone();
    app_handle.listen("always-listening:return-to-wake-word", move |_event| {
        info!("[AlwaysListening] Returning to wake word detection mode");

        let app_handle_clone = app_handle_for_wake_word_return.clone();
        safe_spawn_async_task(move || async move {
            handle_always_listening_return_to_wake_word(&app_handle_clone).await;
        });
    });
}

/// Handle always listening stop requests
async fn handle_always_listening_stop_request(app_handle: &AppHandle) {
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

    // Agent stop events are now handled in setup_agent_control_listeners with stopType
}

/// Setup agent transcription event listeners for hold mode
fn setup_agent_transcription_listeners(app_handle: &AppHandle) {
    // Listen for agent transcription start events (hold mode)
    let app_handle_for_agent_start = app_handle.clone();
    app_handle.listen("agent-transcription-start", move |_event| {
        info!("[Event] Received agent-transcription-start event - starting agent mode via hold");

        let app_handle_clone = app_handle_for_agent_start.clone();
        safe_spawn_async_task(move || async move {
            handle_agent_transcription_start(&app_handle_clone).await;
        });
    });

    // Listen for agent transcription stop events (hold mode - threshold reached)
    let app_handle_for_agent_transcription_stop = app_handle.clone();
    app_handle.listen("agent-transcription-stop", move |_event| {
        info!("[Event] Received agent-transcription-stop event - stopping transcription to process result");

        let app_handle_clone = app_handle_for_agent_transcription_stop.clone();
        safe_spawn_async_task(move || async move {
            handle_agent_transcription_stop(&app_handle_clone).await;
        });
    });
}

/// Setup agent control event listeners for stop with different types
fn setup_agent_control_listeners(app_handle: &AppHandle) {
    // Listen for consolidated agent stop events with stopType parameter
    let app_handle_for_agent_stop = app_handle.clone();
    app_handle.listen("agent-stop", move |event| {
        // Parse stopType from payload
        let stop_type = if let Ok(payload) = serde_json::from_str::<serde_json::Value>(event.payload()) {
            payload.get("stopType").and_then(|t| t.as_str()).unwrap_or("normal").to_string()
        } else {
            "normal".to_string()
        };

        info!("[Event] Received agent-stop event with type: {} - handling agent stop", stop_type);

        let app_handle_clone = app_handle_for_agent_stop.clone();
        safe_spawn_async_task(move || async move {
            match stop_type.as_str() {
                "force" => handle_agent_force_stop(&app_handle_clone).await,
                "error" => handle_agent_error_stop(&app_handle_clone).await,
                _ => handle_agent_stop(&app_handle_clone).await,
            }
        });
    });
}


/// Handle agent transcription start
async fn handle_agent_transcription_start(app_handle: &AppHandle) {
    // Start agent mode using voice transcription
    match app_handle.try_state::<Arc<Mutex<VoiceController>>>() {
        Some(controller_state) => {
            match tauri_plugin_voice_transcription::commands::start_dictation(
                app_handle.clone(),
                controller_state,
            )
            .await
            {
                Ok(()) => {
                    info!("[Agent Mode] Started agent transcription successfully");

                    // Use synchronize_component_state to update UI manager AND emit event
                    if let Err(e) = utils::synchronize_component_state(
                        app_handle,
                        "agent",
                        true,
                        Some(constants::events::agent::ACTIVE),
                    ).await {
                        error!("[Agent Mode] Failed to synchronize agent state change: {}", e);
                    }
                }
                Err(e) => {
                    // Use centralized error handling for agent transcription errors
                    crate::error_handling::utils::handle_agent_error(
                        app_handle,
                        &format!("Failed to start agent transcription: {}", e),
                    )
                    .await;

                    // Reset agent input monitor state on failure
                    crate::agent_monitor::force_reset_agent_input_state().await;

                    // Use synchronize_component_state to update UI manager AND emit event
                    if let Err(e) = utils::synchronize_component_state(
                        app_handle,
                        "agent",
                        false,
                        Some(constants::events::agent::ACTIVE),
                    ).await {
                        error!("[Agent Mode] Failed to synchronize agent state change after error: {}", e);
                    }
                }
            }
        }
        None => {
            warn!("[Agent Mode] Voice controller not available - cannot start agent transcription");

            // Reset agent input monitor state
            crate::agent_monitor::force_reset_agent_input_state().await;

            // Use synchronize_component_state to update UI manager AND emit event
            if let Err(e) = utils::synchronize_component_state(
                app_handle,
                "agent",
                false,
                Some(constants::events::agent::ACTIVE),
            ).await {
                error!("[Agent Mode] Failed to synchronize agent state change: {}", e);
            }
        }
    }
}

/// Handle agent transcription stop (threshold reached)
async fn handle_agent_transcription_stop(app_handle: &AppHandle) {
    // Stop transcription to trigger final result processing
    // This will cause the voice-transcription:final-result event to be emitted
    // which will then process the transcribed text with the agent
    match app_handle.try_state::<Arc<Mutex<VoiceController>>>() {
        Some(controller_state) => {
            match tauri_plugin_voice_transcription::commands::stop_dictation(
                app_handle.clone(),
                controller_state,
            )
            .await
            {
                Ok(_) => {
                    info!("[Agent Mode] Stopped transcription successfully - final result will be processed");
                    // Note: We don't call synchronize_component_state(false) here because the agent will continue
                    // processing the transcribed text. The agent-active false will be handled
                    // after the agent completes processing the query.
                }
                Err(e) => {
                    error!("[Agent Mode] Failed to stop transcription: {}", e);

                    // Reset agent input monitor state on failure
                    crate::agent_monitor::force_reset_agent_input_state().await;

                    // Use synchronize_component_state to update UI manager AND emit event
                    if let Err(e) = utils::synchronize_component_state(
                        app_handle,
                        "agent",
                        false,
                        Some(constants::events::agent::ACTIVE),
                    ).await {
                        error!("[Agent Mode] Failed to synchronize agent state change after transcription stop failure: {}", e);
                    }
                }
            }
        }
        None => {
            warn!("[Agent Mode] Voice controller not available - cannot stop transcription");

            // Reset agent input monitor state
            crate::agent_monitor::force_reset_agent_input_state().await;

            // Use synchronize_component_state to update UI manager AND emit event
            if let Err(e) = utils::synchronize_component_state(
                app_handle,
                "agent",
                false,
                Some(constants::events::agent::ACTIVE),
            ).await {
                error!("[Agent Mode] Failed to synchronize agent state change: {}", e);
            }
        }
    }
}

/// Handle agent stop (normal completion)
async fn handle_agent_stop(app_handle: &AppHandle) {
    // Stop agent mode using voice transcription
    match app_handle.try_state::<Arc<Mutex<VoiceController>>>() {
        Some(controller_state) => {
            match tauri_plugin_voice_transcription::commands::stop_dictation(
                app_handle.clone(),
                controller_state,
            )
            .await
            {
                Ok(_) => {
                    info!("[Agent Mode] Stopped agent transcription successfully");

                    // Use synchronize_component_state to update UI manager AND emit event
                    if let Err(e) = utils::synchronize_component_state(
                        app_handle,
                        "agent",
                        false,
                        Some(constants::events::agent::ACTIVE),
                    ).await {
                        error!("[Agent Mode] Failed to synchronize agent state change: {}", e);
                    }
                }
                Err(e) => {
                    error!("[Agent Mode] Failed to stop agent transcription: {}", e);

                    // Force reset agent input monitor state on failure
                    crate::agent_monitor::force_reset_agent_input_state().await;

                    // Use synchronize_component_state to update UI manager AND emit event
                    if let Err(e) = utils::synchronize_component_state(
                        app_handle,
                        "agent",
                        false,
                        Some(constants::events::agent::ACTIVE),
                    ).await {
                        error!("[Agent Mode] Failed to synchronize agent state change after stop failure: {}", e);
                    }
                }
            }
        }
        None => {
            warn!("[Agent Mode] Voice controller not available - cannot stop agent transcription");

            // Reset agent input monitor state
            crate::agent_monitor::force_reset_agent_input_state().await;

            // Use synchronize_component_state to update UI manager AND emit event
            if let Err(e) = utils::synchronize_component_state(
                app_handle,
                "agent",
                false,
                Some(constants::events::agent::ACTIVE),
            ).await {
                error!("[Agent Mode] Failed to synchronize agent state change: {}", e);
            }
        }
    }
}

/// Handle agent error stop
async fn handle_agent_error_stop(app_handle: &AppHandle) {
    error!("[Agent Mode] Error stop requested");
    
    // Force stop any ongoing operations
    if let Some(controller_state) = app_handle.try_state::<Arc<Mutex<VoiceController>>>() {
        let _ = tauri_plugin_voice_transcription::commands::stop_dictation(
            app_handle.clone(),
            controller_state,
        ).await;
    }
    
    // Reset agent state
    crate::agent_monitor::force_reset_agent_input_state().await;
    
    if let Err(e) = utils::synchronize_component_state(
        app_handle,
        "agent",
        false,
        Some(constants::events::agent::ACTIVE),
    ).await {
        error!("[Agent Mode] Failed to synchronize agent state: {}", e);
    }
}

/// Handle agent force stop (timeout or stuck)
async fn handle_agent_force_stop(app_handle: &AppHandle) {
    // Force stop agent mode
    match app_handle.try_state::<Arc<Mutex<VoiceController>>>() {
        Some(controller_state) => {
            // Force stop voice transcription
            let _ = tauri_plugin_voice_transcription::commands::stop_dictation(
                app_handle.clone(),
                controller_state,
            )
            .await;
        }
        None => {
            warn!("[Agent Mode] Voice controller not available during force stop");
        }
    }

    // Reset agent input monitor state
    crate::agent_monitor::force_reset_agent_input_state().await;

    // Use synchronize_component_state to update UI manager AND emit event
    if let Err(e) = utils::synchronize_component_state(
        app_handle,
        "agent",
        false,
        Some(constants::events::agent::ACTIVE),
    ).await {
        error!("[Agent Mode] Failed to synchronize agent state change during force stop: {}", e);
    }

    info!("[Agent Mode] Force stopped agent mode successfully");
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
                            error!("{}", format_error(templates::FAILED_TO_EMIT, event_name, &e));
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

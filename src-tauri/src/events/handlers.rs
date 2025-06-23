//! # Event Handlers
//!
//! This module contains all event listener handlers for voice transcription,
//! dictation events, timer events, and other application event management.

use std::sync::{Arc, Mutex};
use tauri::{AppHandle, Emitter, Listener, Manager};
use tauri_plugin_voice_transcription::controller::VoiceController;
use tracing::{error, info, warn};

use crate::agent::tools::timer_tools::TimerTask;
use crate::events::timer_handlers::{TimerEventConfig, TimerEventHandler};
use crate::{constants, state};

/// Setup all event listeners for the application
pub fn setup_event_listeners(app: &AppHandle) {
    setup_voice_transcription_listeners(app);
    setup_dictation_listeners(app);
    setup_timer_event_listeners(app);
}

/// Setup voice transcription event listeners
fn setup_voice_transcription_listeners(app: &AppHandle) {
    // Listen for voice transcription final results
    let app_handle_for_listener = app.clone();
    app.listen("voice-transcription:final-result", move |event| {
        let app_handle = app_handle_for_listener.clone();
        tauri::async_runtime::spawn(async move {
            handle_voice_transcription_final_result(app_handle, event.payload()).await;
        });
    });

    // Listen for dictation stopped events
    let app_handle_for_listener = app.clone();
    app.listen("voice-transcription:dictation-stopped", move |event| {
        let app_handle = app_handle_for_listener.clone();
        tauri::async_runtime::spawn(async move {
            handle_voice_transcription_dictation_stopped(app_handle, event.payload().to_string())
                .await;
        });
    });

    // Listen for voice transcription errors
    let app_handle_for_error_listener = app.clone();
    app.listen("voice-transcription:error", move |_event| {
        let app_handle = app_handle_for_error_listener.clone();
        tauri::async_runtime::spawn(async move {
            handle_voice_transcription_error(app_handle).await;
        });
    });
}

/// Setup dictation event listeners
fn setup_dictation_listeners(app: &AppHandle) {
    // Listen for dictation-transcription-start events
    let app_handle_for_dictation_start = app.clone();
    app.listen("dictation-transcription-start", move |_event| {
        let app_handle = app_handle_for_dictation_start.clone();
        tauri::async_runtime::spawn(async move {
            handle_dictation_transcription_start(app_handle).await;
        });
    });

    // Listen for dictation-cancel events
    let app_handle_for_dictation_cancel = app.clone();
    app.listen("dictation-cancel", move |_event| {
        let app_handle = app_handle_for_dictation_cancel.clone();
        tauri::async_runtime::spawn(async move {
            handle_dictation_cancel(app_handle).await;
        });
    });

    // Listen for dictation-stop events
    let app_handle_for_dictation_stop = app.clone();
    app.listen("dictation-stop", move |_event| {
        let app_handle = app_handle_for_dictation_stop.clone();
        tauri::async_runtime::spawn(async move {
            handle_dictation_stop(app_handle).await;
        });
    });

    // Listen for force stop events
    let app_handle_for_force_stop = app.clone();
    app.listen("force-stop-transcription", move |_event| {
        let app_handle = app_handle_for_force_stop.clone();
        tauri::async_runtime::spawn(async move {
            handle_force_stop_transcription(app_handle).await;
        });
    });
}

/// Setup timer event listeners for processing timer-expired events
fn setup_timer_event_listeners(app: &AppHandle) {
    info!("Setting up timer event listeners");

    // Listen for timer-expired events
    let app_handle_for_timer = app.clone();
    app.listen("timer-expired", move |event| {
        let app_handle = app_handle_for_timer.clone();
        tauri::async_runtime::spawn(async move {
            handle_timer_expired_event(app_handle, event.payload()).await;
        });
    });

    // Listen for timer-queued events (for monitoring)
    let app_handle_for_queued = app.clone();
    app.listen("timer-queued", move |event| {
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

    // Check if Dictation Mode is active to determine processing mode
    let app_state = app_handle.state::<state::AppState>();
    let is_dictation_active = app_state
        .dictation_active
        .lock()
        .map(|active| *active)
        .unwrap_or(false);

    // Extract text from payload
    let extracted_text = match serde_json::from_str::<serde_json::Value>(payload_str) {
        Ok(payload_json) => payload_json
            .get("text")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string()),
        Err(_) => None,
    };

    // Handle the result based on mode
    if is_dictation_active {
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
            let app_state = app_handle.state::<state::AppState>();

            // Store to clipboard if enabled
            let clipboard_enabled = app_state
                .dictation_clipboard_enabled
                .lock()
                .map(|enabled| *enabled)
                .unwrap_or(true);

            if clipboard_enabled {
                match crate::commands::core::dev_set_clipboard(
                    trimmed_text.to_string(),
                    app_state.clone(),
                )
                .await
                {
                    Ok(()) => {
                        info!(
                            "[Dictation Mode] Successfully stored text to clipboard: '{}'",
                            trimmed_text
                        );
                    }
                    Err(e) => {
                        error!("[Dictation Mode] Failed to store text to clipboard: {}", e);
                    }
                }
            }

            // Type the transcribed text
            info!("Executing global_type_text for text: '{}'", trimmed_text);
            match crate::commands::keyboard::global_type_text(
                trimmed_text.to_string(),
                app_handle.clone(),
                app_state.clone(),
                None,
            )
            .await
            {
                Ok(()) => {
                    info!(
                        "[Dictation Mode] Successfully typed text: '{}'",
                        trimmed_text
                    );
                }
                Err(e) => {
                    error!("[Dictation Mode] Failed to type transcribed text: {}", e);
                }
            }
        }
    }

    // Reset Dictation Mode state after processing
    let app_state = app_handle.state::<state::AppState>();
    if let Ok(mut dictation_active) = app_state.dictation_active.lock() {
        *dictation_active = false;
    }

    // Emit state change event for UI
    if let Err(e) = app_handle.emit(constants::events::dictation::ACTIVE, false) {
        error!(
            "[Dictation Mode] Failed to emit dictation-active event after final result: {}",
            e
        );
    }

    // Update floating bar manager for dictation mode completion
    crate::commands::floating_bar::handle_dictation_finished(&app_handle, None).await;
    info!("[Dictation Mode] Completed dictation successfully");
}

async fn handle_agent_mode_result(
    app_handle: AppHandle,
    extracted_text: Option<String>,
    payload_str: String,
) {
    info!("[Event] Processing final result for AI Agent Mode");

    // Update floating bar manager for agent mode query
    if let Some(text) = &extracted_text {
        let query_text = text.clone();
        crate::commands::floating_bar::handle_dictation_finished(&app_handle, Some(query_text))
            .await;
    }

    // Transform the payload format
    match serde_json::from_str::<serde_json::Value>(&payload_str) {
        Ok(payload_json) => {
            if let Some(text_value) = payload_json.get("text") {
                let transformed_payload = serde_json::json!({
                    "query": text_value
                });
                if let Err(e) =
                    app_handle.emit(constants::events::dictation::FINISHED, transformed_payload)
                {
                    error!("[Event] Failed to rebroadcast final-result event: {}", e);
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
            if let Err(e) = app_handle.emit(constants::events::dictation::FINISHED, payload_str) {
                error!(
                    "[Event] Failed to rebroadcast final-result event (fallback): {}",
                    e
                );
            }
        }
    }
}

async fn handle_voice_transcription_dictation_stopped(app_handle: AppHandle, payload: String) {
    // Unregister escape key as dictation is complete
    if let Err(e) =
        crate::commands::shortcuts::unregister_escape_key_handler(app_handle.clone()).await
    {
        warn!(
            "Failed to unregister escape key after dictation: {} - continuing anyway",
            e
        );
    }

    // Play voice end sound automatically when dictation stops
    let state = app_handle.state::<crate::state::AppState>();
    if let Err(e) = crate::commands::sound::play_voice_end_sound(app_handle.clone(), state).await {
        warn!("Failed to play voice end sound: {}", e);
    }

    // Rebroadcast the event as app-dictation-stopped for backward compatibility
    if let Err(e) = app_handle.emit("app-dictation-stopped", payload) {
        error!(
            "[Event] Failed to rebroadcast dictation-stopped event: {}",
            e
        );
    }
}

async fn handle_voice_transcription_error(app_handle: AppHandle) {
    // Play voice error sound automatically when transcription fails
    let state = app_handle.state::<crate::state::AppState>();
    if let Err(e) = crate::commands::sound::play_voice_error_sound(app_handle.clone(), state).await
    {
        warn!("Failed to play voice error sound: {}", e);
    }
}

async fn handle_dictation_transcription_start(app_handle: AppHandle) {
    // Mark this as Dictation Mode in AppState BEFORE starting transcription
    let app_state = app_handle.state::<state::AppState>();
    if let Ok(mut dictation_active) = app_state.dictation_active.lock() {
        *dictation_active = true;
    }

    // Update floating bar manager to set dictation mode
    let app_handle_for_bar = app_handle.clone();
    tauri::async_runtime::spawn(async move {
        crate::commands::floating_bar::handle_dictation_mode_change(&app_handle_for_bar, true)
            .await;
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
                info!("[Dictation Mode] Started immediate transcription successfully");

                if let Err(e) = app_handle.emit(constants::events::dictation::ACTIVE, true) {
                    error!(
                        "[Dictation Mode] Failed to emit dictation-active event: {}",
                        e
                    );
                }

                // Register escape key to cancel dictation
                if let Err(e) =
                    crate::commands::shortcuts::register_escape_key_handler(app_handle.clone())
                        .await
                {
                    warn!(
                        "Failed to register escape key for dictation: {} - continuing anyway",
                        e
                    );
                }

                // Play voice start sound
                let app_state_for_sound = app_handle.state::<state::AppState>();
                if let Err(e) = crate::commands::sound::play_voice_start_sound(
                    app_handle.clone(),
                    app_state_for_sound,
                )
                .await
                {
                    warn!("Failed to play voice start sound: {}", e);
                }
            }
            Err(e) => {
                error!("[Dictation Mode] Failed to start dictation: {}", e);

                // Reset the dictation active flag
                if let Ok(mut dictation_active) = app_state.dictation_active.lock() {
                    *dictation_active = false;
                }

                // Emit state change event for UI
                if let Err(e) = app_handle.emit(constants::events::dictation::ACTIVE, false) {
                    error!(
                        "[Dictation Mode] Failed to emit dictation-active event after error: {}",
                        e
                    );
                }

                // Update floating bar manager to reset dictation mode
                let app_handle_for_bar = app_handle.clone();
                tauri::async_runtime::spawn(async move {
                    crate::commands::floating_bar::handle_dictation_mode_change(
                        &app_handle_for_bar,
                        false,
                    )
                    .await;
                });
            }
        }
    } else {
        error!("[Dictation Mode] Voice controller not found, cannot start dictation");

        // Reset the dictation active flag
        if let Ok(mut dictation_active) = app_state.dictation_active.lock() {
            *dictation_active = false;
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

    // Stop dictation forcefully
    if let Some(controller_state) = app_handle.try_state::<Arc<Mutex<VoiceController>>>() {
        let _ = tauri_plugin_voice_transcription::commands::stop_dictation(
            app_handle.clone(),
            controller_state,
        )
        .await;
    }

    // Reset state
    {
        let app_state = app_handle.state::<state::AppState>();
        if let Ok(mut dictation_active) = app_state.dictation_active.lock() {
            *dictation_active = false;
        };
    }
}

async fn handle_dictation_stop(app_handle: AppHandle) {
    info!("[Event] Stopping dictation normally");

    // Stop dictation using the voice transcription plugin command
    if let Some(controller_state) = app_handle.try_state::<Arc<Mutex<VoiceController>>>() {
        let _ = tauri_plugin_voice_transcription::commands::stop_dictation(
            app_handle.clone(),
            controller_state,
        )
        .await;
    }

    // Reset state
    {
        let app_state = app_handle.state::<state::AppState>();
        if let Ok(mut dictation_active) = app_state.dictation_active.lock() {
            *dictation_active = false;
        };
    }

    // Update floating bar manager
    crate::commands::floating_bar::handle_dictation_mode_change(&app_handle, false).await;

    if let Err(e) = app_handle.emit(constants::events::dictation::ACTIVE, false) {
        error!(
            "[Dictation Mode] Failed to emit dictation-active event: {}",
            e
        );
    }
}

async fn handle_force_stop_transcription(app_handle: AppHandle) {
    info!("[Event] Force stopping transcription");

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
                "timer-processed",
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
                "timer-processed",
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
        "timer-status-update",
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

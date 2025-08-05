//! # Mode Event Handlers
//! 
//! Simplified event handlers that delegate to the mode manager

use tauri::{AppHandle, Listener};
use tracing::{error, info};

use crate::mode_manager::{AppMode, get_mode_manager};
use crate::constants;

/// Setup mode-related event listeners
pub fn setup_mode_listeners(app: &AppHandle) {
    // Listen for voice transcription final results
    let app_handle_clone = app.clone();
    app.listen(
        constants::events::voice_transcription::FINAL_RESULT,
        move |event| {
            let app_handle = app_handle_clone.clone();
            tauri::async_runtime::spawn(async move {
                handle_transcription_result(app_handle, event.payload()).await;
            });
        },
    );

    // Listen for dictation start request
    let app_handle_clone = app.clone();
    app.listen(
        constants::events::dictation::TRANSCRIPTION_START,
        move |_event| {
            let app_handle = app_handle_clone.clone();
            tauri::async_runtime::spawn(async move {
                if let Err(e) = get_mode_manager()
                    .transition_to(AppMode::Dictation, "User request".to_string(), &app_handle)
                    .await
                {
                    error!("Failed to start dictation mode: {}", e);
                }
            });
        },
    );

    // Listen for always listening transcription (wake word detection)
    let app_handle_clone = app.clone();
    app.listen(
        "always-listening:transcription",
        move |event| {
            let app_handle = app_handle_clone.clone();
            tauri::async_runtime::spawn(async move {
                handle_always_listening_transcription(app_handle, event.payload()).await;
            });
        },
    );

    // Listen for mode cancellation requests
    let app_handle_clone = app.clone();
    app.listen(
        constants::events::force_stop::TRANSCRIPTION,
        move |_event| {
            let app_handle = app_handle_clone.clone();
            tauri::async_runtime::spawn(async move {
                if let Err(e) = get_mode_manager()
                    .transition_to(AppMode::Idle, "Force stop".to_string(), &app_handle)
                    .await
                {
                    error!("Failed to stop mode: {}", e);
                }
            });
        },
    );
}

/// Handle transcription results based on current mode
async fn handle_transcription_result(app_handle: AppHandle, payload_str: &str) {
    let mode_manager = get_mode_manager();
    let current_mode = mode_manager.get_mode().await;

    // Extract text from payload
    let text = match extract_text_from_payload(payload_str) {
        Some(t) => t,
        None => {
            error!("Failed to extract text from transcription payload");
            return;
        }
    };

    info!("Transcription result in {:?} mode: '{}'", current_mode, text);

    match current_mode {
        AppMode::Dictation => {
            // Type the text at cursor
            handle_dictation_result(app_handle.clone(), text).await;
            
            // Return to idle after dictation
            if let Err(e) = mode_manager
                .transition_to(AppMode::Idle, "Dictation complete".to_string(), &app_handle)
                .await
            {
                error!("Failed to return to idle after dictation: {}", e);
            }
        }
        AppMode::Agent => {
            // Submit to AI agent
            handle_agent_result(app_handle.clone(), text).await;
            
            // Return to idle after agent processing
            if let Err(e) = mode_manager
                .transition_to(AppMode::Idle, "Agent query submitted".to_string(), &app_handle)
                .await
            {
                error!("Failed to return to idle after agent: {}", e);
            }
        }
        AppMode::Idle => {
            // Shouldn't receive transcriptions in idle mode
            error!("Received transcription in idle mode, ignoring");
        }
    }
}

/// Handle always listening transcription (wake word detection)
async fn handle_always_listening_transcription(app_handle: AppHandle, payload_str: &str) {
    let mode_manager = get_mode_manager();
    let config = mode_manager.get_config().await;

    if !config.always_listening_enabled {
        return;
    }

    // Extract text from payload
    let text = match extract_text_from_payload(payload_str) {
        Some(t) => t,
        None => return,
    };

    info!("Always listening detected: '{}'", text);

    // Check if it contains a wake word
    let text_lower = text.to_lowercase();
    let contains_wake_word = config.wake_words.iter()
        .any(|word| text_lower.contains(&word.to_lowercase()));

    if contains_wake_word {
        info!("Wake word detected, triggering agent mode");
        if let Err(e) = mode_manager.handle_wake_word_detected(&app_handle).await {
            error!("Failed to handle wake word: {}", e);
        }
    }
}

/// Type text at cursor for dictation mode
async fn handle_dictation_result(app_handle: AppHandle, text: String) {
    let app_state = app_handle.state::<crate::state::AppState>();
    
    // Store to clipboard if enabled
    let clipboard_enabled = app_state.get_dictation_clipboard_enabled().unwrap_or(true);
    if clipboard_enabled {
        if let Err(e) = crate::commands::core::set_clipboard(
            text.clone(),
            app_handle.clone(),
            app_state.clone(),
        )
        .await
        {
            error!("Failed to store to clipboard: {}", e);
        }
    }

    // Type the text
    if let Err(e) = crate::commands::keyboard::global_type_text(
        text.clone(),
        app_handle.clone(),
        app_state.clone(),
    )
    .await
    {
        error!("Failed to type text: {}", e);
    } else {
        info!("Successfully typed dictation text");
    }
}

/// Submit query to AI agent
async fn handle_agent_result(app_handle: AppHandle, text: String) {
    let app_state = app_handle.state::<crate::state::AppState>();
    
    // Submit to agent
    if let Err(e) = crate::anthropic::submit_query(
        text.clone(),
        &app_state,
        app_handle.clone(),
    )
    .await
    {
        error!("Failed to submit agent query: {}", e);
        crate::error_handling::utils::log_and_emit_error(
            &app_handle,
            "Agent",
            "query_submission",
            &e.to_string(),
            true,
        );
    } else {
        info!("Successfully submitted query to agent");
    }
}

/// Extract text from transcription payload
fn extract_text_from_payload(payload_str: &str) -> Option<String> {
    match serde_json::from_str::<serde_json::Value>(payload_str) {
        Ok(payload_json) => {
            payload_json
                .get("text")
                .and_then(|v| v.as_str())
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
        }
        Err(_) => None,
    }
}
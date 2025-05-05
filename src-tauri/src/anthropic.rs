use crate::state::AppState;
// use crate::tools::{list_tools, handle_tool_call}; // Removed unused
use crate::tts;
// use reqwest::Client; // Removed unused
use serde::{Deserialize, Serialize};
use serde_json::{Value}; // Keep Value
// use image::{GenericImageView, ImageFormat}; // Removed unused
// use base64::{engine::general_purpose::STANDARD as BASE64_STANDARD, Engine as _}; // Removed unused
// use std::io::Cursor; // Removed unused
use tracing::{error, info, warn};
use tauri::State;
use tauri::{Manager, Emitter}; // Import Manager and Emitter
// use futures::future; // Removed unused
use std::sync::Arc;

// Update agent imports to use consolidated structure
use crate::agent::{
    // Import concrete types from implementations
    implementations::{SimpleMemoryManager, AnthropicBrain, DefaultAgentRunner, LocalToolProvider},
    core::{AgentError, AgentRunnable}, // Import core traits and types
    // Import tool registration functions via agent module re-export
    register_basic_tools,
    // register_desktop_tools, // Try importing directly from sub-module
    // Still need specific browser imports if used
    tools::browser_tools::get_browser_tool_definitions,
    tools::browser_controller::BrowserController,
    // Import register_desktop_tools directly
    tools::desktop_tools::register_desktop_tools,
};
// Correct import for BrainFactory
use crate::agent::providers::factory::BrainFactory;

// --- Agent Integration ---
// Use the new Orchestrator
// use crate::agents::orchestrator::Orchestrator;
// use crate::agent::{
//     implementations::{
//         memory_manager::SimpleMemoryManager,
//         // Remove tool_provider and agent_runner
//         // AnthropicBrain is now selected via the factory
//     },
//     // Remove AgentRunnable
//     tools::{ // Keep tools for now, might be needed by specific agents
//         basic_tools::register_basic_tools, // Moved
//         desktop_tools::register_desktop_tools, // Moved
//         browser_tools::get_browser_tool_definitions, // Moved
//         browser_controller::BrowserController, // Moved
//     },
//      providers::factory::BrainFactory, // Moved
// };

// --- Agent State ---

// Removed unused enum AgentState

// --- Anthropic API Structs ---

// Removed unused struct AnthropicMessage

#[derive(Deserialize, Debug, Clone, Serialize)]
pub(crate) struct AnthropicContentBlock {
    #[serde(rename = "type")]
    pub(crate) type_: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) text: Option<String>,
    // Fields related to tool_use
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) input: Option<Value>,
    // Fields related to tool_result (we create these, don't expect from API)
}

// Removed unused struct ToolResultBlock

// Keep this for payload structure, ensure Clone is derived
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct SubmitQueryResult {
    pub text: String,
    pub audio_base64: Option<String>,
    pub agent_state: String, // Send final state to frontend
    pub screenshot_base64: Option<String>, // Optional screenshot data from the session
}

// Define the payload structure for the event
#[derive(Serialize, Clone)]
struct BackendResponsePayload {
    query: String,
    response: SubmitQueryResult,
}

// Removed AnthropicThinkingBudget as it was commented out

// Removed unused struct AnthropicRequest

#[derive(Deserialize, Debug)]
struct AnthropicUsage {
    #[allow(dead_code)] // Allow dead code for potentially unused fields
    input_tokens: u32,
    #[allow(dead_code)] // Allow dead code for potentially unused fields
    output_tokens: u32,
}

#[derive(Deserialize, Debug)]
struct AnthropicResponse {
    _id: Option<String>,
    #[serde(rename = "type")]
    _type_: Option<String>,
    _role: Option<String>,
    _model: Option<String>,
    _content: Option<Vec<AnthropicContentBlock>>,
    _stop_reason: Option<String>,
    _stop_sequence: Option<String>,
    _usage: Option<AnthropicUsage>,
}

// --- Helper Functions ---

// Removed unused function process_screenshot

// --- Submit Query Function (Refactored with AgentRunner) ---

#[tauri::command]
pub async fn submit_query(
    query: String,
    state: State<'_, AppState>,
    app_handle: tauri::AppHandle,
) -> Result<(), String> {
    info!("Received query: {}", query);

    // --- Cancellation Setup ---
    let cancel_rx = state.cancel_rx.clone();

    // --- Instantiate Agent Components ---
    // Use correct path for SimpleMemoryManager
    let memory_manager = SimpleMemoryManager::new();

    // --- Create Tool Provider ---
    // Use correct path for LocalToolProvider
    let mut tool_provider = LocalToolProvider::with_app_handle(app_handle.clone());
    // Use imported registration functions directly
    register_basic_tools(&mut tool_provider).await;
    register_desktop_tools(&mut tool_provider, app_handle.clone(), state.clone()).await;
    // TODO: Register browser tools if needed
    info!("Tool provider initialized with basic and desktop tools.");

    // --- Create Brain ---
    // Use corrected BrainFactory import
    let agent_brain_result = BrainFactory::create_brain();
    let agent_brain = match agent_brain_result {
        Ok(brain) => brain,
        Err(e) => {
             let err_msg = format!("Failed to initialize agent brain: {}", e);
             error!("{}", err_msg);
             // Error reporting to frontend (keep this logic)
             let result = SubmitQueryResult { text: err_msg.clone(), audio_base64: None, agent_state: "Failed".to_string(), screenshot_base64: None };
             let payload = BackendResponsePayload { query: query.clone(), response: result };
             if let Some(window) = app_handle.get_window("main") {
                 window.emit("backend-response", payload).map_err(|e| format!("Emit failed: {}", e))?;
             } else { error!("Main window not found, cannot emit initial brain error."); }
             return Err(err_msg);
        }
    };
    info!("Agent brain initialized.");

    // --- Create Agent Runner ---
    const MAX_STEPS: u32 = 15;
    // Use imported DefaultAgentRunner directly
    let mut agent_runner = DefaultAgentRunner::with_boxed_brain(
        memory_manager,
        tool_provider,
        agent_brain, // Pass the boxed brain directly
        MAX_STEPS,
    );
    info!("Agent runner initialized.");

    // --- Run Agent ---
    let final_result = agent_runner.run(query.clone(), cancel_rx).await;

    // --- Process Result ---
    let (final_text, final_state_str) = match final_result {
        Ok(text) => {
            info!("Agent run finished successfully.");
            (text, "Finished".to_string())
        }
        Err(e) => {
            error!("Agent run failed: {}", e);
            let state_str = match e {
                AgentError::MaxStepsReached => "MaxStepsReached".to_string(),
                AgentError::Terminated => "Cancelled".to_string(),
                _ => "Failed".to_string(),
            };
            (format!("Agent Error: {}", e), state_str)
        }
    };

    // --- Get Screenshot (Optional) ---
    let screenshot_base64: Option<String> = None; // TODO: Implement screenshot retrieval

    // --- TTS (Optional) ---
    let audio_base64 = match get_tts_audio(final_text.clone(), state.clone()).await {
        Ok(audio) => Some(audio),
        Err(e) => {
            error!("TTS generation failed: {}", e);
            None
        }
    };

    // --- Emit Final Result to Frontend ---
    let result = SubmitQueryResult {
        text: final_text,
        audio_base64,
        agent_state: final_state_str,
        screenshot_base64,
    };

    let payload = BackendResponsePayload { query, response: result };

    info!("Emitting final response to frontend.");
    if let Some(window) = app_handle.get_window("main") {
        window.emit("backend-response", payload).map_err(|e| format!("Emit failed: {}", e))?;
    } else {
        error!("Main window not found, cannot emit final response.");
        return Err("Main window not found".to_string());
    }

    Ok(())
}

// --- Browser Cleanup Function (Keep for now, maybe move later) ---
#[tauri::command]
pub async fn cleanup_browser(_app_handle: tauri::AppHandle) -> Result<(), String> {
    info!("Received request to clean up browser resources.");

    // How to access the BrowserController now? It lives inside BrowserAgent.
    // We might need a way to signal cleanup to the agent or access it via AppState?
    // For now, this command won't work correctly after the refactor.
    // Let's comment out the actual cleanup attempt.

    // let state = app_handle.state::<AppState>();
    // if let Some(controller_arc) = state.browser_controller.lock().await.as_ref() {
    //     let mut controller = controller_arc.lock().await;
    //     if let Err(e) = controller.close().await {
    //         error!("Error cleaning up browser controller: {}", e);
    //         return Err(format!("Cleanup failed: {}", e));
    //     }
    //     info!("Browser resources cleaned up successfully.");
    // } else {
    //     info!("No active browser controller found to clean up.");
    // }
    warn!("Browser cleanup logic needs refactoring after agent changes.");

    Ok(())
}

// Removed unused function get_tool_definitions

// Removed unused function handle_agent_action

// Keep get_tts_audio
#[tauri::command]
pub async fn get_tts_audio(text: String, state: State<'_, AppState>) -> Result<String, String> {
    info!("Received request for TTS audio.");
    tts::invoke_tts(text, state)
        .await
        .map_err(|e| e.to_string())
}

use crate::state::AppState;
// use crate::tools::{list_tools, handle_tool_call}; // Removed unused
// use reqwest::Client; // Removed unused
use serde::{Deserialize, Serialize};
use serde_json::{Value}; // Keep Value
// use image::{GenericImageView, ImageFormat}; // Removed unused
// use base64::{engine::general_purpose::STANDARD as BASE64_STANDARD, Engine as _}; // Removed unused
// use std::io::Cursor; // Removed unused
use tracing::{error, info};
use tauri::State;
use tauri::{Manager, Emitter}; // Import Manager and Emitter
// use futures::future; // Removed unused
use std::sync::Arc;
use crate::agent::structs::{AgentError};

// --- Agent Integration ---
use crate::agent::{
    implementations::{
        // Correct path based on resolved structure
        memory_manager::SimpleMemoryManager,
        tool_provider::LocalToolProvider,
        agent_runner::DefaultAgentRunner,
        // AnthropicBrain is now selected via the factory
        // agent_brain::AnthropicBrain, // Remove direct import
    },
    traits::AgentRunnable, // Import the trait for the run method
    tools::{ // Changed this block
        basic_tools::register_basic_tools,
        desktop_tools::register_desktop_tools,
        browser_tools::get_browser_tool_definitions,
        browser_controller::BrowserController,
    },
     providers::factory::BrainFactory, // Keep BrainFactory import
};

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

// --- Submit Query Function (Refactored with New Agent) ---

#[tauri::command]
pub async fn submit_query(
    query: String,
    state: State<'_, AppState>,
    app_handle: tauri::AppHandle,
) -> Result<(), String> {
    info!("Received query: {}", query);

    let cancel_rx = state.cancel_rx.clone();

    // --- Instantiate Agent Components ---
    let memory_manager = SimpleMemoryManager::new();

    let mut tool_provider = LocalToolProvider::with_app_handle(app_handle.clone());

    // --- Instantiate Browser Controller ---
    let browser_controller = match BrowserController::new().await {
        Ok(controller) => {
            info!("Browser Controller initialized successfully.");
            Some(controller) // Store as Option
        }
        Err(e) => {
            error!("Failed to initialize Browser Controller: {}. Browser tools will not be available.", e);
            // Allow agent to continue without browser tools, just log the error.
            None
        }
    };

    // Register basic file/shell tools
    register_basic_tools(&mut tool_provider).await;
    info!("Registered basic tools for the agent.");

    // Register desktop tools
    register_desktop_tools(&mut tool_provider, app_handle.clone(), state.clone()).await;
    info!("Registered desktop tools for the agent.");

    // --- Register Browser Tools (only if controller initialized) ---
    if let Some(browser_controller) = browser_controller {
        let browser_definitions = get_browser_tool_definitions();
        let shared_browser_controller = Arc::new(tokio::sync::Mutex::new(browser_controller)); // Wrap in Arc<Mutex>

        for definition in browser_definitions {
            let tool_name = definition.name.clone();
            let log_tool_name = tool_name.clone();
            let controller_arc = shared_browser_controller.clone(); // Clone Arc

            let executor = move |input: Value| {
                let controller_lock = controller_arc.clone(); // Clone Arc again for async block
                let name = tool_name.clone();
                async move {
                    let controller = controller_lock.lock().await; // Lock the Mutex
                    let result = match name.as_str() {
                        "browser_navigate" => controller.navigate(&input).await,
                        "browser_extract_content" => controller.extract_content(&input).await,
                        "browser_interact" => controller.interact(&input).await,
                        "browser_get_current_url" => controller.get_current_url(&input).await,
                        "browser_screenshot" => controller.screenshot(&input).await,
                        _ => Err(AgentError::ToolNotFound(name)),
                    };
                    match result {
                        Ok(tool_result) => Ok(tool_result.output),
                        Err(agent_error) => Err(agent_error.to_string()),
                    }
                }
            };
            tool_provider.register_async_tool(definition, executor).await;
            info!("Registered browser tool: {}", log_tool_name);
        }
    } else {
        info!("Skipping browser tool registration as controller failed to initialize.");
    }

    // Use the BrainFactory to create the appropriate AI provider brain
    let agent_brain = match BrainFactory::create_brain() { // Keep using BrainFactory
        Ok(brain) => brain,
        Err(e) => {
             let err_msg = format!("Failed to initialize agent brain: {}", e);
             error!("{}", err_msg);
             let result = SubmitQueryResult { text: err_msg.clone(), audio_base64: None, agent_state: "Failed".to_string(), screenshot_base64: None };
             let payload = BackendResponsePayload { query: query.clone(), response: result };
             if let Some(window) = app_handle.get_window("main") {
                 window.emit("backend-response", payload).map_err(|e| format!("Emit failed: {}", e))?;
             } else { error!("Main window not found, cannot emit initial brain error."); }
             return Err(err_msg);
        }
    };
    info!("Agent brain initialized.");

    const MAX_ITERATIONS: u32 = 15;

    // Create the agent runner using with_boxed_brain because BrainFactory returns a Box
    let mut agent_runner = DefaultAgentRunner::with_boxed_brain(
        memory_manager,
        tool_provider, // This now contains all registered tools
        agent_brain,   // Pass the Box<dyn AgentBrain>
        MAX_ITERATIONS,
    );
    info!("Agent runner created with max {} iterations.", MAX_ITERATIONS);

    info!("Starting agent run...");
    let agent_result = agent_runner.run(query.clone(), cancel_rx).await;

    state.reset_cancel();
    info!("Agent cancellation signal reset.");

    // --- Process Agent Result ---
    let final_response = match agent_result {
        Ok(message) => SubmitQueryResult {
            text: message,
            audio_base64: None, // Add TTS later if needed
            agent_state: "Finished".to_string(),
            screenshot_base64: None, // Capture screenshot if needed
        },
        Err(e) => {
            error!("Agent run failed: {}", e);
            // Map AgentError to a user-friendly state/message
            let (state_str, msg) = match e {
                AgentError::Terminated => ("Cancelled".to_string(), "Agent execution was cancelled.".to_string()),
                AgentError::MaxStepsReached => ("Failed".to_string(), "Agent reached maximum steps.".to_string()),
                _ => ("Failed".to_string(), format!("Agent error: {}", e)),
            };
            SubmitQueryResult {
                text: msg,
                audio_base64: None,
                agent_state: state_str,
                screenshot_base64: None,
            }
        }
    };

    info!("Agent run complete. Final state: {}", final_response.agent_state);

    // --- Emit Final Response --- //
    let payload = BackendResponsePayload { query, response: final_response };
    if let Some(window) = app_handle.get_window("main") {
        window.emit("backend-response", payload)
            .map_err(|e| format!("Emit failed: {}", e))?;
        info!("Final response emitted to frontend.");
    } else {
        error!("Main window not found, cannot emit final response.");
    }

    Ok(())
}

// --- Browser Cleanup Function ---

#[tauri::command]
pub async fn cleanup_browser(app_handle: tauri::AppHandle) -> Result<(), String> {
    log::info!("Cleaning up browser resources...");

    // Simplified cleanup - assuming BrowserController handles its own drop
    // Or rely on OS to clean up processes on app exit.
    // The pkill approach might be too aggressive or fail.

    // If BrowserController needs explicit cleanup, call it here.
    // e.g., if state holds Arc<Mutex<BrowserController>>:
    // let state = app_handle.state::<AppState>();
    // if let Some(controller) = state.browser_controller.lock().await {
    //     controller.close().await; // Assuming a close method exists
    // }

    log::info!("Browser cleanup check completed (manual pkill removed).");
    Ok(())
}

// --- TTS Function ---

#[tauri::command]
pub async fn get_tts_audio(text: String, _state: State<'_, AppState>) -> Result<String, String> {
    let _ = text; // Mark text as used
    error!("TTS function is currently stubbed out.");
    Err("TTS functionality is temporarily disabled.".to_string())
}

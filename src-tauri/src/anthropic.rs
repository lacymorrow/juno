use log::{info, error, warn};

use serde::{Deserialize, Serialize};
use serde_json::Value;
use tauri::{State, Manager, Emitter};

use crate::agent::implementations::{
    agent_runner::DefaultAgentRunner,
    memory_manager::SimpleMemoryManager,
    tool_provider::LocalToolProvider,
};
use crate::agent::tools::{
    basic_tools::register_basic_tools,
    desktop_tools::setup_tools,
    browser_tools::get_browser_tool_definitions,
};
use crate::agent::structs::AgentError;
use crate::agent::traits::AgentRunnable;
use crate::agent::providers::factory::BrainFactory;
use crate::state::AppState;

// use crate::tools::{list_tools, handle_tool_call}; // Removed unused
// use reqwest::Client; // Removed unused
// use image::{GenericImageView, ImageFormat}; // Removed unused
// use base64::{engine::general_purpose::STANDARD as BASE64_STANDARD, Engine as _}; // Removed unused
// use std::io::Cursor; // Removed unused
// use tauri::{Manager, Emitter}; // Import Manager and Emitter
// use futures::future; // Removed unused

// --- Agent Integration ---
// use crate::agent::{
//     implementations::{
//         // Correct path based on resolved structure
//         memory_manager::SimpleMemoryManager,
//         tool_provider::LocalToolProvider,
//         agent_runner::DefaultAgentRunner,
//         // AnthropicBrain is now selected via the factory
//         // agent_brain::AnthropicBrain, // Remove direct import
//     },
//     traits::AgentRunnable, // Import the trait for the run method
//     // tools::{ // Remove this entire block as it's redundant/incorrect
//     //     basic_tools::register_basic_tools,
//     //     desktop_tools::register_desktop_tools,
//     //     browser_tools::get_browser_tool_definitions,
//     //     browser_controller::BrowserController,
//     // },
//      providers::factory::BrainFactory, // Keep BrainFactory import
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

    // --- REMOVE EAGER INITIALIZATION OF BROWSER CONTROLLER ---
    // The BrowserController will be initialized lazily by each tool executor when needed.
    // let browser_controller_option = match state.get_or_init_browser_controller().await {
    //     Ok(controller) => {
    //         info!("Previously, Browser Controller would be initialized here.");
    //         Some(controller)
    //     }
    //     Err(e) => {
    //         error!("Failed to get or initialize Browser Controller: {}. Browser tools will not be available.", e);
    //         None
    //     }
    // };

    // Register basic file/shell tools
    register_basic_tools(&mut tool_provider).await;
    info!("Registered basic tools for the agent.");

    // Setup tools (desktop tools, etc.) - this may include non-browser tools
    setup_tools(&mut tool_provider, state.clone(), app_handle.clone()).await;

    // --- Register Browser Tools (executors will initialize controller lazily) ---
    let browser_definitions = get_browser_tool_definitions();
    for definition in browser_definitions {
        let tool_name = definition.name.clone(); // This will be moved into the closure
        // Clone AppHandle to be moved into the async executor closure
        let app_handle_for_tool_executor = app_handle.clone();

        let executor = move |input: Value| {
            let app_handle_captured = app_handle_for_tool_executor.clone(); // Clone for the async block
            // The `tool_name` from the outer scope is moved here and owned by the closure.
            // We can use it directly or clone it if needed for further nested async blocks.
            let current_tool_name_captured = tool_name.clone(); // Clone the moved tool_name for use in async block
            async move {
                // Get AppState from the AppHandle
                let state_from_handle = app_handle_captured.state::<AppState>();

                // LAZILY get or initialize the browser controller INSIDE the tool execution
                let browser_controller_instance = match state_from_handle.get_or_init_browser_controller().await {
                    Ok(controller) => controller,
                    Err(e) => {
                        let err_msg = format!("Failed to initialize BrowserController for tool {}: {}", current_tool_name_captured, e);
                        error!("{}", err_msg);
                        // Propagate as a string error, consistent with existing tool error handling
                        return Err(err_msg);
                    }
                };

                // Now use the obtained browser_controller_instance
                let result = match current_tool_name_captured.as_str() {
                    "browser_navigate" => browser_controller_instance.navigate(&input).await,
                    "browser_extract_content" => browser_controller_instance.extract_content(&input).await,
                    "browser_interact" => browser_controller_instance.interact(&input).await,
                    "browser_get_current_url" => browser_controller_instance.get_current_url(&input).await,
                    "browser_screenshot" => browser_controller_instance.screenshot(&input).await,
                    _ => Err(AgentError::ToolNotFound(current_tool_name_captured)),
                };

                match result {
                    Ok(tool_result) => Ok(tool_result.output),
                    // Convert AgentError to String to match the executor's expected error type
                    Err(agent_error) => Err(agent_error.to_string()),
                }
            }
        };
        tool_provider.register_async_tool(definition.clone(), executor).await; // Clone definition for register_async_tool
        // Use definition.name directly for logging as tool_name was moved.
        info!("Registered browser tool (with lazy initialization): {}", definition.name);
    }
    // No 'else' block needed as we register definitions regardless;
    // initialization failure is handled within the executor.

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
        app_handle.clone(), // Pass AppHandle
    );
    info!("Agent runner created with max {} iterations.", MAX_ITERATIONS);

    // Register escape key shortcut for agent execution (only when agent is actually running)
    crate::register_escape_key_shortcut(&app_handle);

    info!("Starting agent run...");
    let agent_result = agent_runner.run(query.clone(), cancel_rx).await;

    // Always unregister escape key shortcut when agent finishes (regardless of success/failure)
    crate::unregister_escape_key_shortcut(&app_handle);

    state.reset_cancel();
    info!("Agent cancellation signal reset.");

    // --- Process Agent Result ---
    let mut final_response = match agent_result {
        Ok(message) => SubmitQueryResult {
            text: message.clone(),
            audio_base64: None, // Will be set below if TTS is enabled
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
                text: msg.clone(),
                audio_base64: None, // Will be set below if TTS is enabled
                agent_state: state_str,
                screenshot_base64: None,
            }
        }
    };

    // --- Generate TTS Audio ---
    // Try to generate TTS for the response text if TTS is enabled
    match crate::tts::invoke_tts(final_response.text.clone(), state.clone()).await {
        Ok(audio_result) => {
            if audio_result != "TTS_DISABLED_BY_SETTING" {
                final_response.audio_base64 = Some(audio_result);
                info!("TTS audio generated successfully for response");
            } else {
                info!("TTS is disabled, skipping audio generation");
            }
        }
        Err(e) => {
            warn!("Failed to generate TTS audio: {}. Continuing without audio.", e);
            // Don't fail the whole response, just continue without audio
        }
    }

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

    // Get the app state to access the browser controller
    let state = app_handle.state::<AppState>();

    // Acquire lock on the browser controller
    let mut controller_guard = state.browser_controller.lock().await;

    // If we have a browser controller, clean it up
    if let Some(controller) = controller_guard.take() {
        if let Err(e) = controller.cleanup().await {
            log::error!("Failed to clean up browser controller: {}", e);
            return Err(format!("Failed to clean up browser: {}", e));
        }
        log::info!("Browser controller cleaned up successfully");
    } else {
        log::info!("No browser controller to clean up");
    }

    log::info!("Browser cleanup completed successfully");
    Ok(())
}

// Note: TTS functionality is now handled through the main TTS module
// via crate::tts::invoke_tts() which is called during agent execution

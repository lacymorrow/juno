use crate::state::AppState;
// use crate::tools::{list_tools, handle_tool_call}; // Removed unused
use crate::tts;
// use reqwest::Client; // Removed unused
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use image::{GenericImageView, ImageFormat}; // Keep if process_screenshot is kept/used
use base64::{engine::general_purpose::STANDARD as BASE64_STANDARD, Engine as _}; // Keep if process_screenshot is kept/used
use std::io::Cursor; // Keep if process_screenshot is kept/used
use tracing::{error, info}; // Removed unused debug, warn
use tauri::State;
use tauri::{Manager, Emitter}; // Import Manager and Emitter
// use futures::future; // Removed unused
use std::sync::Arc;

// --- Agent Integration ---
use crate::agent::{
    implementations::{
        agent_brain::AnthropicBrain, // Use the new brain
        memory_manager::SimpleMemoryManager,
        tool_provider::LocalToolProvider,
        agent_runner::DefaultAgentRunner,
    },
    structs::AgentError,
    traits::AgentRunnable, // Import the trait for the run method
    tools::{ // Changed this block
        basic_tools::register_basic_tools,
        desktop_tools::register_desktop_tools,
        browser_tools::get_browser_tool_definitions,
        browser_controller::BrowserController,
    },
};

// --- Agent State ---

#[derive(Debug, Clone, PartialEq, Serialize)] // Added Serialize for potential logging/debugging
enum AgentState {
    Thinking,
    Acting,
    ProcessingActionResult,
    Finished,
    Failed,
}

// --- Anthropic API Structs ---

#[derive(Serialize, Clone)]
pub(crate) struct AnthropicMessage {
    pub(crate) role: String,
    pub(crate) content: Value, // Keep as Value to handle complex content
}

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
    // #[serde(skip_serializing_if = "Option::is_none")]
    // pub(crate) tool_use_id: Option<String>,
    // #[serde(skip_serializing_if = "Option::is_none")]
    // pub(crate) is_error: Option<bool>,
}


#[derive(Serialize, Clone)] // Added Clone
pub(crate) struct ToolResultBlock {
    #[serde(rename = "type")]
    pub(crate) type_: String, // Always "tool_result"
    pub(crate) tool_use_id: String,
    pub(crate) content: Value, // Changed to Value to match Anthropic's structure [ { "type": "text", "text": "..." } ] or [ { "type": "image", ... } ]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) is_error: Option<bool>,
}

// Keep this for payload structure, ensure Clone is derived
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct SubmitQueryResult {
    pub text: String,
    pub audio_base64: Option<String>,
    pub agent_state: String, // Send final state to frontend
    pub screenshot_base64: Option<String>, // Optional screenshot data from the session
    // pub conversation_history: Vec<AnthropicMessage>, // Optionally send history for debugging
}

// Define the payload structure for the event
#[derive(Serialize, Clone)]
struct BackendResponsePayload {
    query: String,
    response: SubmitQueryResult,
}

// Removed AnthropicThinkingBudget as it was commented out
// #[derive(Serialize)]
// struct AnthropicThinkingBudget {
//     #[serde(rename = "type")]
//     type_: String,
//     budget_tokens: u32,
// }

#[derive(Serialize)]
struct AnthropicRequest<'a> {
    model: &'a str,
    max_tokens: u32,
    messages: Vec<AnthropicMessage>,
    tools: Vec<computer_use_ai_sdk::ToolDefinition>, // Use full path
    #[serde(skip_serializing_if = "Option::is_none")]
    system: Option<String>,
    // #[serde(skip_serializing_if = "Option::is_none")] // Removed thinking budget
    // thinking: Option<AnthropicThinkingBudget>,
}

#[derive(Deserialize, Debug)]
struct AnthropicUsage {
    #[allow(dead_code)] // Allow dead code for potentially unused fields
    input_tokens: u32,
    #[allow(dead_code)] // Allow dead code for potentially unused fields
    output_tokens: u32,
}

#[derive(Deserialize, Debug)]
struct AnthropicResponse {
    content: Vec<AnthropicContentBlock>,
    stop_reason: String,
    #[allow(dead_code)] // Allow dead code for potentially unused fields
    usage: AnthropicUsage,
}

// Import the coordinates utility
use crate::utils::coordinates;

// --- Helper Functions ---

async fn process_screenshot(base64_data: &str) -> Result<Value, String> {
     match BASE64_STANDARD.decode(base64_data) {
        Ok(image_bytes) => {
            match image::load_from_memory(&image_bytes) {
                Ok(img) => {
                    let (width, height) = img.dimensions();
                    let max_dim = 1024.0; // Max dimension for resizing
                    let scale = if width > height {
                        max_dim / width as f32
                    } else {
                        max_dim / height as f32
                    };

                    let resized_img = if scale < 1.0 {
                        let new_width = (width as f32 * scale).round() as u32;
                        let new_height = (height as f32 * scale).round() as u32;

                        // Store the scaling information
                        coordinates::update_scaling_info(width, height, new_width, new_height, scale);

                        img.resize_exact(new_width, new_height, image::imageops::FilterType::Lanczos3)
                    } else {
                        // No resizing needed, store 1:1 scaling
                        coordinates::update_scaling_info(width, height, width, height, 1.0);
                        img // No resize needed if already smaller or equal
                    };

                    let mut png_bytes = Vec::new();
                    match resized_img.write_to(&mut Cursor::new(&mut png_bytes), ImageFormat::Png) {
                        Ok(_) => {
                             let resized_base64_data = BASE64_STANDARD.encode(&png_bytes);
                             Ok(json!([{ // Return as array of content blocks
                                "type": "image",
                                "source": {
                                    "type": "base64",
                                    "media_type": "image/png",
                                    "data": resized_base64_data
                                }
                             }]))
                        }
                        Err(e) => {
                            let err_msg = format!("Failed to encode resized image to PNG: {}", e);
                            error!("{}", err_msg);
                            // Return error as text block within the array
                            Ok(json!([{"type": "text", "text": err_msg}]))
                        }
                    }
                }
                Err(e) => {
                    let err_msg = format!("Failed to load image from screenshot bytes: {}", e);
                    error!("{}", err_msg);
                     Ok(json!([{"type": "text", "text": err_msg}]))
                }
            }
        }
        Err(e) => {
            let err_msg = format!("Failed to decode base64 screenshot data: {}", e);
            error!("{}", err_msg);
             Ok(json!([{"type": "text", "text": err_msg}]))
        }
    }
}


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
    // This needs to be Arc<Mutex<>> or similar if shared across async tasks/threads
    // Or clone it into each tool closure if only used there.
    // Let's clone it into closures for now.
    let browser_controller = match BrowserController::new().await {
        Ok(controller) => {
            info!("Browser Controller initialized successfully.");
            controller // Not wrapped in Arc/Mutex yet
        }
        Err(e) => {
            error!("Failed to initialize Browser Controller: {}. Browser tools will not be available.", e);
            // Return error immediately if browser is essential
             let err_msg = format!("Failed to start browser automation: {}", e);
             let result = SubmitQueryResult { text: err_msg.clone(), audio_base64: None, agent_state: "Failed".to_string(), screenshot_base64: None };
             let payload = BackendResponsePayload { query, response: result };
             if let Some(window) = app_handle.get_window("main") {
                 window.emit("backend-response", payload).map_err(|e| format!("Emit failed: {}", e))?;
             } else { error!("Main window not found, cannot emit initial browser error."); }
             return Err(err_msg);
            // Or, allow agent to continue without browser tools:
            // None
        }
    };

    // Register basic file/shell tools
    register_basic_tools(&mut tool_provider).await;
    info!("Registered basic tools for the agent.");

    // Register desktop tools
    register_desktop_tools(&mut tool_provider, app_handle.clone(), state.clone()).await;
    info!("Registered desktop tools for the agent.");

    // --- Register Browser Tools ---
    let browser_definitions = get_browser_tool_definitions();

    // Store the browser controller in an Arc to share it safely
    let shared_browser_controller = Arc::new(browser_controller);

    for definition in browser_definitions {
        let tool_name = definition.name.clone();
        let log_tool_name = tool_name.clone(); // Clone for logging outside the closure

        // Clone the Arc for each iteration, not the controller itself
        let controller_arc = shared_browser_controller.clone();

        let executor = move |input: Value| {
            let controller_ref = controller_arc.clone();
            let name = tool_name.clone();
            async move {
                let result = match name.as_str() {
                    "browser_navigate" => controller_ref.navigate(&input).await,
                    "browser_extract_content" => controller_ref.extract_content(&input).await,
                    "browser_interact" => controller_ref.interact(&input).await,
                    "browser_get_current_url" => controller_ref.get_current_url(&input).await,
                    "browser_screenshot" => controller_ref.screenshot(&input).await,
                    // Add other browser tool cases here
                    _ => Err(AgentError::ToolNotFound(name)),
                };

                // Convert the result to the expected format (Value or String error)
                match result {
                    Ok(tool_result) => Ok(tool_result.output),
                    Err(agent_error) => Err(agent_error.to_string()),
                }
            }
        };

        tool_provider.register_async_tool(definition, executor).await;
        info!("Registered browser tool: {}", log_tool_name);
    }

    // Instantiate the brain
    let agent_brain = match AnthropicBrain::from_env() {
        Ok(brain) => brain,
        Err(e) => {
            // ... (existing error handling) ...
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

    let mut agent_runner = DefaultAgentRunner::new(
        memory_manager,
        tool_provider, // This now contains all registered tools
        agent_brain,
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

    // The state should hold a reference to the browser controller if we want to keep it alive
    // between queries. For now, we just ensure any Playwright processes are terminated.

    // On macOS, attempt to kill any lingering playwright/chromium processes
    if cfg!(target_os = "macos") {
        match std::process::Command::new("pkill")
            .arg("-f")
            .arg("playwright")
            .output() {
                Ok(_) => log::info!("Playwright processes terminated."),
                Err(e) => log::warn!("Failed to terminate Playwright processes: {}", e),
            }

        // Also try to terminate chromium processes spawned by playwright
        match std::process::Command::new("pkill")
            .arg("-f")
            .arg("chromium")
            .output() {
                Ok(_) => log::info!("Chromium processes terminated."),
                Err(e) => log::warn!("Failed to terminate Chromium processes: {}", e),
            }
    }

    log::info!("Browser cleanup completed.");
    Ok(())
}

// --- TTS Function ---

#[tauri::command]
pub async fn get_tts_audio(text: String, _state: State<'_, AppState>) -> Result<String, String> {
    // STUB: Remove dependency on AppState fields for now
    // let client = state.anthropic_client.lock().await;
    // let api_key = state.api_key.lock().await;

    // if let Some(ref key) = *api_key {
    //     match tts::generate_tts(&client, key, &text).await {
    //         Ok(audio_bytes) => Ok(BASE64_STANDARD.encode(audio_bytes)),
    //         Err(e) => Err(format!("TTS generation failed: {}", e)),
    //     }
    // } else {
    //     Err("API key not set for TTS generation".to_string())
    // }
    let _ = text; // Mark text as used
    error!("TTS function is currently stubbed out.");
    Err("TTS functionality is temporarily disabled.".to_string())

}

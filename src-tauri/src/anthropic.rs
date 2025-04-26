use crate::state::AppState;
// use crate::tools::{list_tools, handle_tool_call}; // Removed unused
use crate::tts;
// use reqwest::Client; // Removed unused
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use image::{GenericImageView, ImageFormat, EncodableLayout}; // Keep if process_screenshot is kept/used
use base64::{engine::general_purpose::STANDARD as BASE64_STANDARD, Engine as _}; // Keep if process_screenshot is kept/used
use std::io::Cursor; // Keep if process_screenshot is kept/used
use tracing::{error, info}; // Removed unused debug, warn
use tauri::State;
use tauri::{Manager, Emitter}; // Import Manager and Emitter
// use futures::future; // Removed unused

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
    tools::basic_tools::register_basic_tools, // Import the tool registration helper
    tools::desktop_tools::register_desktop_tools, // Import the new desktop tool registration helper
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

                    // Always update scaling info with 1.0 factor, as we are not resizing
                    coordinates::update_scaling_info(width, height, width, height, 1.0);

                    // Encode as JPEG with quality 75
                    let mut jpg_bytes = Vec::new();
                    let mut encoder = image::codecs::jpeg::JpegEncoder::new_with_quality(&mut jpg_bytes, 75); // Quality 0-100

                    match encoder.encode(img.to_rgb8().as_bytes(), width, height, image::ColorType::Rgb8.into()) {
                        Ok(_) => {
                             let jpeg_base64_data = BASE64_STANDARD.encode(&jpg_bytes);
                             Ok(json!([{ // Return as array of content blocks
                                "type": "image",
                                "source": {
                                    "type": "base64",
                                    "media_type": "image/jpeg", // Use JPEG media type
                                    "data": jpeg_base64_data
                                }
                             }]))
                        }
                        Err(e) => {
                            let err_msg = format!("Failed to encode image to JPEG: {}", e);
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
    app_handle: tauri::AppHandle, // Pass AppHandle
) -> Result<(), String> { // Return Ok(()) or Err(string) for command result
    info!("Received query: {}", query);

    // --- Instantiate Agent Components ---
    let memory_manager = SimpleMemoryManager::new();

    // Create tool provider with app handle for event emission
    let mut tool_provider = LocalToolProvider::with_app_handle(app_handle.clone());

    // Register tools
    // TODO: This currently registers only basic file/shell tools.
    // Adapt `register_basic_tools` or create new registration functions
    // to include the richer set of desktop interaction tools previously defined
    // in `src-tauri/src/tools/definitions.rs` once they are implemented
    // according to the `ToolProvider` trait requirements.
    register_basic_tools(&mut tool_provider).await;
    info!("Registered basic tools for the agent.");

    // Register desktop tools (currently placeholders)
    // Pass cloned app_handle and state for the closures to capture
    register_desktop_tools(&mut tool_provider, app_handle.clone(), state.clone()).await;
    info!("Registered desktop tools for the agent (placeholders).");

    // Instantiate the brain, handling potential env var errors
    let agent_brain = match AnthropicBrain::from_env() {
        Ok(brain) => brain,
        Err(e) => {
             let err_msg = format!("Failed to initialize agent brain: {}", e);
             error!("{}", err_msg);
              // Emit error response immediately
             let result = SubmitQueryResult {
                 text: err_msg.clone(),
                 audio_base64: None,
                 agent_state: "Failed".to_string(),
                 screenshot_base64: None,
             };
             let payload = BackendResponsePayload { query, response: result };
             // Use a blocking send or spawn a task if emit needs to be async within a sync context
             // For now, assume direct emit works or handle potential blocking issues if they arise.
             if let Some(window) = app_handle.get_window("main") {
                 window.emit("backend-response", payload)
                     .map_err(|e| format!("Emit failed: {}", e))?;
             } else {
                 error!("Main window not found, cannot emit initial error.");
             }
             return Err(err_msg); // Return error from the command itself
         }
     };
    info!("Agent brain initialized.");

    // Max steps for the agent loop
    const MAX_ITERATIONS: u32 = 15; // Set a reasonable limit

    // Create the agent runner
    let mut agent_runner = DefaultAgentRunner::new(
        memory_manager,
        tool_provider,
        agent_brain,
        MAX_ITERATIONS,
    );
    info!("Agent runner created with max {} iterations.", MAX_ITERATIONS);

    // --- Run the Agent ---
    info!("Starting agent run...");
    let agent_result = agent_runner.run(query.clone()).await;

    // --- Process Agent Result ---
    let (final_response_text, final_state_str) = match agent_result {
        Ok(final_text) => {
            info!("Agent finished successfully.");
            (final_text, "Finished".to_string())
        }
        Err(agent_error) => {
             error!("Agent failed: {:?}", agent_error);
             // Provide a user-friendly error message based on the AgentError type
             let user_error_message = match agent_error {
                 AgentError::MaxStepsReached => format!("Agent stopped after reaching the maximum {} steps. The task might be too complex or require more iterations.", MAX_ITERATIONS),
                 AgentError::LlmError(s) => format!("An error occurred while communicating with the AI model: {}", s),
                 AgentError::ToolError(s) => format!("An error occurred while executing a required tool: {}", s),
                 AgentError::ToolNotFound(s) => format!("A required tool ('{}') could not be found.", s),
                 AgentError::ConfigurationError(s) => format!("Agent configuration error: {}", s),
                 AgentError::MemoryError(s) => format!("Agent memory error: {}", s),
                 AgentError::StateError(s) => format!("Agent encountered an invalid state: {}", s),
                 AgentError::InputError(s) => format!("Invalid input provided to the agent: {}", s),
                 AgentError::OutputError(s) => format!("Error processing agent output: {}", s),
                 AgentError::LoopError(s) => format!("An internal error occurred in the agent loop: {}", s),
                 AgentError::Terminated => "Agent execution was terminated.".to_string(),
                 AgentError::Unknown(s) => format!("An unknown error occurred: {}", s),
                 // Consider adding more specific handling if needed
             };
            (user_error_message, "Failed".to_string())
        }
    };

    info!("Agent final response text: {}", final_response_text);

    // --- Perform TTS Synthesis ---
    let audio_base64 = match tts::invoke_tts(final_response_text.clone(), state.clone()).await {
        Ok(base64) => Some(base64),
        Err(e) => {
            error!("TTS synthesis failed: {}", e);
            None // Proceed without audio if TTS fails
        }
    };

    // --- Prepare and Emit Final Result ---
    let result = SubmitQueryResult {
        text: final_response_text,
        audio_base64,
        agent_state: final_state_str,
        screenshot_base64: None, // Default to None, could be updated in future to include last screenshot
    };
    let payload = BackendResponsePayload {
        query: query.clone(), // Use the original query
        response: result,
    };

    info!("Emitting final backend-response");
     match app_handle.get_window("main") {
         Some(window) => {
             window.emit("backend-response", payload)
                 .map_err(|e| format!("Failed to emit backend-response event: {}", e))?;
             info!("Successfully emitted backend-response event.");
             Ok(()) // Command succeeded
         }
         None => {
             let err_msg = "Main window not found, cannot emit event.".to_string();
             error!("{}", err_msg);
             Err(err_msg) // Command failed
         }
     }
}

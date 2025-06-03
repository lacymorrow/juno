use log::{info, error, warn};

use serde::{Deserialize, Serialize};
use serde_json::Value;
use tauri::{State, Manager, Emitter};

use crate::agent::implementations::{
    agent_runner::DefaultAgentRunner,
    tool_provider::LocalToolProvider,
};
use crate::agent::tools::{
    basic_tools::register_basic_tools,
    desktop_tools::setup_tools,
    browser_tools::get_browser_tool_definitions,
};
use crate::agent::structs::AgentError;
use crate::agent::traits::{AgentRunnable, MemoryManager};
use crate::agent::providers::factory::BrainFactory;
use crate::agent::providers::config::AgentMode;
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

// --- Submit Query Function (Refactored with Orchestrator-Based Architecture) ---

#[tauri::command]
pub async fn submit_query(
    query: String,
    state: State<'_, AppState>,
    app_handle: tauri::AppHandle,
) -> Result<(), String> {
    info!("Received query: {}", query);

    let cancel_rx = state.cancel_rx.clone();

    // --- Get Persistent Memory Manager (Orchestrator maintains conversation memory) ---
    let memory_manager_arc = state.get_memory_manager().await;
    let memory_manager = memory_manager_arc.lock().await;
    let memory_manager_clone = memory_manager.clone();
    drop(memory_manager); // Release the lock early

    // --- Setup Tool Provider for Specialized Agents ---
    let mut tool_provider = LocalToolProvider::with_app_handle(app_handle.clone());

    // Register basic file/shell tools
    register_basic_tools(&mut tool_provider).await;
    info!("Registered basic tools for specialized agents.");

    // Setup desktop tools for specialized agents
    setup_tools(&mut tool_provider, state.clone(), app_handle.clone()).await;

    // Register browser tools for specialized agents (with lazy initialization)
    let browser_definitions = get_browser_tool_definitions();
    for definition in browser_definitions {
        let tool_name = definition.name.clone();
        let app_handle_for_tool_executor = app_handle.clone();

        let executor = move |input: Value| {
            let app_handle_captured = app_handle_for_tool_executor.clone();
            let current_tool_name_captured = tool_name.clone();
            async move {
                let state_from_handle = app_handle_captured.state::<AppState>();

                let browser_controller_instance = match state_from_handle.get_or_init_browser_controller().await {
                    Ok(controller) => controller,
                    Err(e) => {
                        let err_msg = format!("Failed to initialize BrowserController for tool {}: {}", current_tool_name_captured, e);
                        error!("{}", err_msg);
                        return Err(err_msg);
                    }
                };

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
                    Err(agent_error) => Err(agent_error.to_string()),
                }
            }
        };
        tool_provider.register_async_tool(definition.clone(), executor).await;
        info!("Registered browser tool for specialized agents: {}", definition.name);
    }

    // --- Determine Agent Mode and Create Runtime ---
    let agent_mode = BrainFactory::get_agent_mode();
    info!("Using agent mode: {:?}", agent_mode);

    let agent_result = match agent_mode {
        AgentMode::Single => {
            // Single agent mode - use traditional approach with all tools
            let brain = match BrainFactory::create_brain() {
                Ok(brain) => brain,
                Err(e) => {
                    let err_msg = format!("Failed to initialize single agent brain: {}", e);
                    error!("{}", err_msg);
                    let result = SubmitQueryResult {
                        text: err_msg.clone(),
                        audio_base64: None,
                        agent_state: "Failed".to_string(),
                        screenshot_base64: None
                    };
                    let payload = BackendResponsePayload { query: query.clone(), response: result };
                    if let Some(window) = app_handle.get_window("main") {
                        window.emit("backend-response", payload).map_err(|e| format!("Emit failed: {}", e))?;
                    } else {
                        error!("Main window not found, cannot emit initial brain error.");
                    }
                    return Err(err_msg);
                }
            };
            info!("Single agent brain initialized.");

            const MAX_ITERATIONS: u32 = 15;

            // Create single agent runner with all tools
            let mut single_agent_runner = DefaultAgentRunner::with_boxed_brain(
                memory_manager_clone,
                tool_provider,
                brain,
                MAX_ITERATIONS,
                app_handle.clone(),
            );
            info!("Single agent runner created with all tools.");

            // Register escape key shortcut for agent execution
            crate::register_escape_key_shortcut(&app_handle);

            info!("Starting single agent run...");
            let result = single_agent_runner.run(query.clone(), cancel_rx).await;

            // Always unregister escape key shortcut when agent finishes
            crate::unregister_escape_key_shortcut(&app_handle);

            result
        },
        AgentMode::Multi => {
            // Multi-agent mode - use orchestrator with specialized agents
            let orchestrator_brain = match BrainFactory::create_brain_with_system_prompt(get_orchestrator_personality_prompt()) {
                Ok(brain) => brain,
                Err(e) => {
                    let err_msg = format!("Failed to initialize orchestrator brain: {}", e);
                    error!("{}", err_msg);
                    let result = SubmitQueryResult {
                        text: err_msg.clone(),
                        audio_base64: None,
                        agent_state: "Failed".to_string(),
                        screenshot_base64: None
                    };
                    let payload = BackendResponsePayload { query: query.clone(), response: result };
                    if let Some(window) = app_handle.get_window("main") {
                        window.emit("backend-response", payload).map_err(|e| format!("Emit failed: {}", e))?;
                    } else {
                        error!("Main window not found, cannot emit initial brain error.");
                    }
                    return Err(err_msg);
                }
            };
            info!("Orchestrator brain initialized.");

            // Create orchestrator with delegation tools
            let mut orchestrator_tool_provider = LocalToolProvider::with_app_handle(app_handle.clone());

            // Register delegation tools for the orchestrator
            register_orchestrator_delegation_tools(&mut orchestrator_tool_provider, tool_provider, app_handle.clone()).await;
            info!("Registered delegation tools for orchestrator.");

            const MAX_ITERATIONS: u32 = 15;

            // Create the orchestrator agent runner with personality-focused system prompt
            let mut orchestrator_runner = DefaultAgentRunner::with_boxed_brain(
                memory_manager_clone,
                orchestrator_tool_provider,
                orchestrator_brain,
                MAX_ITERATIONS,
                app_handle.clone(),
            );
            info!("Orchestrator agent runner created with personality and delegation capabilities.");

            // Register escape key shortcut for orchestrator execution
            crate::register_escape_key_shortcut(&app_handle);

            info!("Starting orchestrator run...");
            let result = orchestrator_runner.run(query.clone(), cancel_rx).await;

            // Always unregister escape key shortcut when orchestrator finishes
            crate::unregister_escape_key_shortcut(&app_handle);

            result
        }
    };

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

    // --- Emit Final Response ---
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

/// Get the personality-focused system prompt for the orchestrator
fn get_orchestrator_personality_prompt() -> String {
    r#"You are Juno, an intelligent and capable AI assistant with a warm, helpful personality. You maintain conversation context and memory across interactions.

Your approach:
- Be conversational and engaging while staying helpful and professional
- Remember previous parts of our conversation and refer to them when relevant
- Break down complex requests into manageable tasks
- Delegate specific technical tasks to specialized agents while maintaining the conversational flow
- Always explain what you're doing and why

You have access to specialized agents that can help with specific tasks:
- browser_agent: For web browsing, navigation, and web-based tasks
- desktop_agent: For desktop automation, clicking elements, and system interactions
- file_agent: For file operations, code editing, and terminal commands

When delegating tasks:
1. Use the delegate_to_agent tool to send clear, specific instructions
2. Wait for the agent's response before proceeding
3. Interpret and contextualize the results for the user
4. Handle any errors gracefully and try alternative approaches

Maintain your personality throughout - you're not just routing requests, you're having a conversation and helping solve problems thoughtfully."#.to_string()
}

/// Register delegation tools that allow the orchestrator to communicate with specialized agents
async fn register_orchestrator_delegation_tools(
    orchestrator_provider: &mut LocalToolProvider,
    specialist_provider: LocalToolProvider,
    _app_handle: tauri::AppHandle,
) {
    use serde_json::json;

    // Wrap the specialist provider in Arc for sharing across tool executions
    let specialist_provider_arc = std::sync::Arc::new(specialist_provider);

    // Delegate to Browser Agent
    let browser_delegation_def = crate::agent::structs::ToolDefinition {
        name: "delegate_to_browser_agent".to_string(),
        description: "Delegate web browsing, navigation, and web interaction tasks to the browser specialist agent".to_string(),
        input_schema: json!({
            "type": "object",
            "properties": {
                "task": {
                    "type": "string",
                    "description": "Clear description of the web browsing task to perform"
                },
                "context": {
                    "type": "string",
                    "description": "Additional context or requirements for the task"
                }
            },
            "required": ["task"]
        }),
    };

    let browser_provider = specialist_provider_arc.clone();
    let browser_app_handle = _app_handle.clone();
    let browser_executor = move |input: serde_json::Value| {
        let provider = browser_provider.clone();
        let handle = browser_app_handle.clone();
        async move {
            execute_specialized_agent_task(provider, "browser", input, handle).await
        }
    };
    orchestrator_provider.register_async_tool(browser_delegation_def, browser_executor).await;

    // Delegate to Desktop Agent
    let desktop_delegation_def = crate::agent::structs::ToolDefinition {
        name: "delegate_to_desktop_agent".to_string(),
        description: "Delegate desktop automation, clicking, typing, and system interaction tasks to the desktop specialist agent".to_string(),
        input_schema: json!({
            "type": "object",
            "properties": {
                "task": {
                    "type": "string",
                    "description": "Clear description of the desktop automation task to perform"
                },
                "context": {
                    "type": "string",
                    "description": "Additional context or requirements for the task"
                }
            },
            "required": ["task"]
        }),
    };

    let desktop_provider = specialist_provider_arc.clone();
    let desktop_app_handle = _app_handle.clone();
    let desktop_executor = move |input: serde_json::Value| {
        let provider = desktop_provider.clone();
        let handle = desktop_app_handle.clone();
        async move {
            execute_specialized_agent_task(provider, "desktop", input, handle).await
        }
    };
    orchestrator_provider.register_async_tool(desktop_delegation_def, desktop_executor).await;

    // Delegate to File Agent
    let file_delegation_def = crate::agent::structs::ToolDefinition {
        name: "delegate_to_file_agent".to_string(),
        description: "Delegate file operations, code editing, terminal commands, and development tasks to the file specialist agent".to_string(),
        input_schema: json!({
            "type": "object",
            "properties": {
                "task": {
                    "type": "string",
                    "description": "Clear description of the file/coding task to perform"
                },
                "context": {
                    "type": "string",
                    "description": "Additional context or requirements for the task"
                }
            },
            "required": ["task"]
        }),
    };

    let file_provider = specialist_provider_arc.clone();
    let file_app_handle = _app_handle.clone();
    let file_executor = move |input: serde_json::Value| {
        let provider = file_provider.clone();
        let handle = file_app_handle.clone();
        async move {
            execute_specialized_agent_task(provider, "file", input, handle).await
        }
    };
    orchestrator_provider.register_async_tool(file_delegation_def, file_executor).await;

    info!("Registered all delegation tools for orchestrator");
}

/// Execute a task using a specialized agent and return a formatted response
async fn execute_specialized_agent_task(
    tool_provider: std::sync::Arc<LocalToolProvider>,
    agent_type: &str,
    input: serde_json::Value,
    app_handle: tauri::AppHandle,
) -> Result<serde_json::Value, String> {
    let task = input["task"].as_str()
        .ok_or_else(|| "Missing required 'task' parameter".to_string())?;
    let context = input["context"].as_str().unwrap_or("");

    info!("Executing {} agent task: {}", agent_type, task);

    // Create a simple memory manager for the specialized agent
    let specialist_memory = crate::agent::implementations::memory_manager::SimpleMemoryManager::new();

    // Create appropriate brain for the specialist agent with focused system prompt
    let system_prompt = get_specialist_system_prompt(agent_type);
    let specialist_brain = match BrainFactory::create_brain_with_system_prompt(system_prompt) {
        Ok(brain) => brain,
        Err(e) => return Err(format!("Failed to create specialist brain: {}", e)),
    };

    // Create specialist agent runner with focused system prompt
    let mut specialist_runner = DefaultAgentRunner::with_boxed_brain(
        specialist_memory,
        (*tool_provider).clone(),
        specialist_brain,
        10, // Reduced iterations for focused tasks
        app_handle,
    );

    // Format the query for the specialist agent
    let specialist_query = if context.is_empty() {
        task.to_string()
    } else {
        format!("{}\n\nAdditional context: {}", task, context)
    };

    // Execute the specialist agent
    match specialist_runner.run(specialist_query, tokio::sync::watch::channel(false).1).await {
        Ok(result) => {
            info!("Specialist {} agent completed successfully", agent_type);
            Ok(serde_json::json!({
                "success": true,
                "agent_type": agent_type,
                "result": result,
                "message": format!("{} agent completed the task successfully", agent_type)
            }))
        }
        Err(e) => {
            error!("Specialist {} agent failed: {}", agent_type, e);
            Err(format!("{} agent failed: {}", agent_type, e))
        }
    }
}

/// Get system prompt for specialized agents
fn get_specialist_system_prompt(agent_type: &str) -> String {
    match agent_type {
        "browser" => {
            r#"You are a browser automation specialist. Your job is to handle web browsing tasks efficiently and accurately.

Focus on:
- Navigating to websites
- Interacting with web elements (clicking, typing, scrolling)
- Extracting information from web pages
- Taking screenshots when needed
- Handling forms and web applications

Be direct and task-focused. Execute the requested web task and report back with clear results."#.to_string()
        }
        "desktop" => {
            r#"You are a desktop automation specialist. Your job is to handle desktop interaction tasks efficiently.

Focus on:
- Clicking desktop elements and applications
- Typing text and keyboard shortcuts
- Moving the mouse and performing gestures
- Taking screenshots of the desktop
- Interacting with system UI elements

Be direct and task-focused. Execute the requested desktop task and report back with clear results."#.to_string()
        }
        "file" => {
            r#"You are a file operations and development specialist. Your job is to handle file system and coding tasks efficiently.

Focus on:
- Reading, writing, and editing files
- Running terminal commands
- Code generation and modification
- File system navigation
- Development workflows

Be direct and task-focused. Execute the requested file/coding task and report back with clear results."#.to_string()
        }
        _ => {
            r#"You are a task execution specialist. Execute the given task efficiently and report back with clear results."#.to_string()
        }
    }
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

// --- TTS Function ---

#[tauri::command]
pub async fn get_tts_audio(text: String, state: State<'_, AppState>) -> Result<String, String> {
    // Call the invoke_tts function with the text and state
    crate::tts::invoke_tts(text, state).await
}

// --- Clear Conversation History ---

#[tauri::command]
pub async fn clear_conversation_history(state: State<'_, AppState>) -> Result<(), String> {
    info!("Clearing conversation history...");

    let memory_manager_arc = state.get_memory_manager().await;
    let mut memory_manager = memory_manager_arc.lock().await;

    match memory_manager.clear_memory().await {
        Ok(()) => {
            info!("Conversation history cleared successfully");
            Ok(())
        }
        Err(e) => {
            error!("Failed to clear conversation history: {}", e);
            Err(format!("Failed to clear conversation history: {}", e))
        }
    }
}

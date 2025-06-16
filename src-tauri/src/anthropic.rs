use log::{info, error, warn};
use uuid;

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
use crate::agent::prompts::PromptManager;
use crate::state::AppState;
use crate::utils::{gather_system_context, format_system_context_for_agent};
use crate::constants::{agent_config, tool_names, timeouts};

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


// Keep this for payload structure, ensure Clone is derived
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct SubmitQueryResult {
    pub text: String,
    pub spoken_text: Option<String>, // Optional separate content for TTS
    pub audio_base64: Option<String>,
    pub agent_state: String, // Send final state to frontend
    pub screenshot_base64: Option<String>, // Optional screenshot data from the session
}

// Note: BackendResponsePayload removed as we now use streaming events only

// Removed AnthropicThinkingBudget as it was commented out


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

/// Simple JSX content detection
fn is_jsx_content(content: &str) -> bool {
    // Check for common JSX patterns
    content.contains("<") && content.contains(">") && (
        content.contains("Card") ||
        content.contains("Alert") ||
        content.contains("Button") ||
        content.contains("Badge") ||
        content.contains("Circle") ||
        content.contains("Rectangle") ||
        content.contains("Triangle") ||
        content.contains("StatusCard") ||
        content.contains("ColorShowcase") ||
        content.contains("VisualDemo") ||
        content.contains("className=") ||
        content.contains("jsx") ||
        content.contains("React")
    )
}

// --- Submit Query Function (Refactored with Orchestrator-Based Architecture) ---

#[tauri::command]
pub async fn submit_query(
    query: String,
    state: State<'_, AppState>,
    app_handle: tauri::AppHandle,
) -> Result<(), String> {
    info!("Received query: {}", query);

    // --- Validate query text ---
    let trimmed_query = query.trim();
    if trimmed_query.is_empty() {
        warn!("Received empty or whitespace-only query, ignoring");
        return Ok(());
    }

    // --- Check if an agent is already running and cancel it ---
    if state.is_agent_executing() {
        let current_execution_id = state.get_current_agent_execution_id();
        info!("Agent is currently executing (ID: {:?}), cancelling for new command", current_execution_id);

        // Signal cancellation for the existing agent
        state.signal_cancel();
        info!("Signaled cancellation for existing agent execution");

        // Give existing agent a brief moment to clean up gracefully
        tokio::time::sleep(tokio::time::Duration::from_millis(timeouts::MEDIUM_DELAY_MS)).await;

        // Reset the cancellation signal for the new agent
        state.reset_cancel();
        info!("Reset cancellation signal for new agent");
    } else {
        // No agent currently running, ensure cancellation signal is reset
        state.reset_cancel();
        info!("No existing agent detected, proceeding with new execution");
    }

    // Generate a unique execution ID for this agent run
    let execution_id = uuid::Uuid::new_v4().to_string();

    // Mark agent execution as started with max iterations (both modes use 15)
    state.mark_agent_execution_started_with_steps(execution_id.clone(), agent_config::MAX_ITERATIONS);
    info!("Starting new agent execution with ID: {} (max steps: {})", execution_id, agent_config::MAX_ITERATIONS);

    // Register escape key for cancellation during agent execution
    if let Err(e) = crate::commands::shortcuts::register_escape_key_handler(app_handle.clone()).await {
        warn!("Failed to register escape key for agent execution: {} - continuing without escape key cancellation", e);
    }

    // --- Gather System Context ---
    let system_context = match gather_system_context(Some(&*state)).await {
        Ok(context) => {
            info!("System context gathered successfully");
            Some(context)
        }
        Err(e) => {
            warn!("Failed to gather system context: {}", e);
            None
        }
    };

    let cancel_rx = state.cancel_rx.clone();

    // --- Get Persistent Memory Manager (Orchestrator maintains conversation memory) ---
    let memory_manager_arc = state.get_memory_manager().await;
    let memory_manager_clone = {
        let mut memory_manager = memory_manager_arc.lock().await;

        // Clean up any orphaned tool calls from previous cancelled executions
        // This prevents LLM errors when tool calls don't have corresponding results
        if let Err(e) = memory_manager.clean_orphaned_tool_calls().await {
            warn!("Failed to clean orphaned tool calls: {}", e);
        }

        memory_manager.clone()
    };

    // --- Setup Tool Provider for Specialized Agents ---
    let mut tool_provider = LocalToolProvider::with_app_handle(app_handle.clone());

    // Register basic file/shell tools
    register_basic_tools(&mut tool_provider).await;
    info!("Registered basic tools for specialized agents.");

    // Setup desktop tools for specialized agents and get the shared provider
    let shared_tool_provider = setup_tools(&mut tool_provider, state.clone(), app_handle.clone()).await;

    // Extract the tool provider from Arc<Mutex<>> for agent creation
    let agent_tool_provider = {
        let guard = shared_tool_provider.lock().await;
        guard.clone()
    };

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
        // Register browser tools on the shared provider instance
        {
            let guard = shared_tool_provider.lock().await;
            guard.register_async_tool(definition.clone(), executor).await;
        }
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

                    // Emit error via streaming events instead of backend-response
                    let error_message_id = uuid::Uuid::new_v4().to_string();
                    crate::agent::tool_logger::emit_stream_start(&app_handle, error_message_id.clone());
                    crate::agent::tool_logger::emit_streaming_text_chunk(&app_handle, err_msg.clone(), Some(error_message_id.clone()));
                    crate::agent::tool_logger::emit_stream_end(&app_handle, error_message_id, err_msg.clone());
                    return Err(err_msg);
                }
            };
            info!("Single agent brain initialized.");

            // Create single agent runner with all tools
            let mut single_agent_runner = DefaultAgentRunner::with_boxed_brain(
                memory_manager_clone,
                agent_tool_provider,
                brain,
                agent_config::MAX_ITERATIONS,
                app_handle.clone(),
            );
            info!("Single agent runner created with all tools.");

            info!("Starting single agent run...");

            // Prepare the query with system context
            let contextual_query = if let Some(ref context) = system_context {
                format!("{}\n\nUser Query: {}", format_system_context_for_agent(context), trimmed_query)
            } else {
                trimmed_query.to_string()
            };

            let result = single_agent_runner.run(contextual_query, cancel_rx).await;

            result
        },
        AgentMode::Multi => {
            // Multi-agent mode - use orchestrator with specialized agents
            let orchestrator_brain = match BrainFactory::create_brain_with_system_prompt(get_orchestrator_personality_prompt()) {
                Ok(brain) => brain,
                Err(e) => {
                    let err_msg = format!("Failed to initialize orchestrator brain: {}", e);
                    error!("{}", err_msg);

                    // Emit error via streaming events instead of backend-response
                    let error_message_id = uuid::Uuid::new_v4().to_string();
                    crate::agent::tool_logger::emit_stream_start(&app_handle, error_message_id.clone());
                    crate::agent::tool_logger::emit_streaming_text_chunk(&app_handle, err_msg.clone(), Some(error_message_id.clone()));
                    crate::agent::tool_logger::emit_stream_end(&app_handle, error_message_id, err_msg.clone());
                    return Err(err_msg);
                }
            };
            info!("Orchestrator brain initialized.");

            // Create orchestrator with delegation tools
            let mut orchestrator_tool_provider = LocalToolProvider::with_app_handle(app_handle.clone());

            // Register delegation tools for the orchestrator
            register_orchestrator_delegation_tools(&mut orchestrator_tool_provider, agent_tool_provider, app_handle.clone()).await;
            info!("Registered delegation tools for orchestrator.");

            // Create the orchestrator agent runner with personality-focused system prompt
            let mut orchestrator_runner = DefaultAgentRunner::with_boxed_brain(
                memory_manager_clone,
                orchestrator_tool_provider,
                orchestrator_brain,
                agent_config::MAX_ITERATIONS,
                app_handle.clone(),
            );
            info!("Orchestrator agent runner created with personality and delegation capabilities.");

            info!("Starting orchestrator run...");

            // Prepare the query with system context for orchestrator
            let contextual_query = if let Some(ref context) = system_context {
                format!("{}\n\nUser Query: {}", format_system_context_for_agent(context), trimmed_query)
            } else {
                trimmed_query.to_string()
            };

            let result = orchestrator_runner.run(contextual_query, cancel_rx).await;

            result
        }
    };

    state.reset_cancel();
    info!("Agent cancellation signal reset.");

    // Mark agent execution as finished
    state.mark_agent_execution_finished();
    info!("Agent execution marked as finished for ID: {}", execution_id);

    // Unregister escape key as agent execution is complete
    if let Err(e) = crate::commands::shortcuts::unregister_escape_key_handler(app_handle.clone()).await {
        warn!("Failed to unregister escape key after agent execution: {} - continuing anyway", e);
    }

    // --- Process Agent Result ---
    let mut final_response = match agent_result {
        Ok(message) => {
            // Note: Success sound will be played after TTS completes (or immediately if TTS is disabled)

            // Check if there's stored spoken content from a dual content finish
            let spoken_content = state.get_last_spoken_content();

            SubmitQueryResult {
                text: message.clone(),
                spoken_text: spoken_content, // Use stored spoken content if available
                audio_base64: None, // Will be set below if TTS is enabled
                agent_state: "Finished".to_string(),
                screenshot_base64: None, // Capture screenshot if needed
            }
        },
        Err(e) => {
            error!("Agent run failed: {}", e);
            let (state_str, msg) = match e {
                AgentError::Terminated => {
                    // Play agent attention sound for cancellation (less intrusive than error)
                    if let Err(e) = crate::commands::sound::play_agent_attention_sound(app_handle.clone(), state.clone()).await {
                        warn!("Failed to play cancellation sound: {}", e);
                    }
                    ("Cancelled".to_string(), "Agent execution was cancelled.".to_string())
                },
                AgentError::MaxStepsReached => {
                    // Play agent error sound for failure
                    if let Err(e) = crate::commands::sound::play_agent_error_sound(app_handle.clone(), state.clone()).await {
                        warn!("Failed to play error sound: {}", e);
                    }
                    ("Failed".to_string(), "Agent reached maximum steps.".to_string())
                },
                _ => {
                    // Play agent error sound for other failures
                    if let Err(e) = crate::commands::sound::play_agent_error_sound(app_handle.clone(), state.clone()).await {
                        warn!("Failed to play error sound: {}", e);
                    }
                    ("Failed".to_string(), format!("Agent error: {}", e))
                },
            };
            SubmitQueryResult {
                text: msg.clone(),
                spoken_text: None, // Error messages use same content for speech
                audio_base64: None, // Will be set below if TTS is enabled
                agent_state: state_str,
                screenshot_base64: None,
            }
        }
    };

    // --- Generate TTS Audio ---
    // Use spoken_text if available, otherwise fall back to text
    let tts_content = final_response.spoken_text.as_ref().unwrap_or(&final_response.text).clone();

    // Clear spoken content from app state now that we've used it
    state.clear_last_spoken_content();

    let tts_enabled = match crate::tts::invoke_tts(tts_content, state.clone(), app_handle.clone()).await {
        Ok(audio_result) => {
            if audio_result != "TTS_DISABLED_BY_SETTING" {
                final_response.audio_base64 = Some(audio_result.clone());
                info!("TTS audio generated successfully for response");

                // Emit TTS audio event for frontend to play, regardless of streaming status
                let tts_app_handle = app_handle.clone();
                let tts_audio_data = audio_result.clone();
                tauri::async_runtime::spawn(async move {
                    if let Err(e) = tts_app_handle.emit("tts-audio-ready", serde_json::json!({
                        "audio_base64": tts_audio_data
                    })) {
                        warn!("Failed to emit TTS audio event: {}", e);
                    }
                });

                // Update floating bar manager for TTS start
                let app_handle_for_tts = app_handle.clone();
                let tts_text = final_response.text.clone();
                tauri::async_runtime::spawn(async move {
                    crate::commands::floating_bar::handle_tts_started(&app_handle_for_tts, tts_text).await;
                    // Note: TTS finish event and success sound are now handled by handle_tts_completion
                    // when the frontend notifies us that audio playback has completed
                });
                true // TTS is enabled and audio was generated
            } else {
                info!("TTS is disabled, skipping audio generation");
                false // TTS is disabled
            }
        }
        Err(e) => {
            warn!("Failed to generate TTS audio: {}. Continuing without audio.", e);
            // Don't fail the whole response, just continue without audio
            false // TTS failed, treat as disabled
        }
    };

    // Play agent success sound immediately if TTS is disabled, otherwise it will be played when TTS finishes
    if !tts_enabled && final_response.agent_state == "Finished" {
        if let Err(e) = crate::commands::sound::play_agent_success_sound(app_handle.clone(), state.clone()).await {
            warn!("Failed to play success sound: {}", e);
        }
    }

    info!("Agent run complete. Final state: {}", final_response.agent_state);

    // --- Update Floating Bar Manager ---
    let app_handle_for_bar = app_handle.clone();
    let agent_state_for_bar = final_response.agent_state.clone();
    let text_for_bar = final_response.text.clone();
    tauri::async_runtime::spawn(async move {
        crate::commands::floating_bar::handle_backend_response(
            &app_handle_for_bar,
            &agent_state_for_bar,
            Some(text_for_bar)
        ).await;
    });

    // --- Emit agent error event for main chat interface ---
    if final_response.agent_state == "Failed" || final_response.agent_state == "Cancelled" {
        let error_event_handle = app_handle.clone();
        let error_state = final_response.agent_state.clone();
        let error_text = final_response.text.clone();
        let trimmed_query = query.trim().to_string();
        tauri::async_runtime::spawn(async move {
            let event_data = serde_json::json!({
                "agent_state": error_state,
                "error_message": error_text,
                "original_query": trimmed_query
            });
            if let Err(e) = error_event_handle.emit("agent-error", event_data) {
                warn!("Failed to emit agent-error event: {}", e);
            }
        });
    }

    // Final response is now fully handled by streaming events
    // The frontend will reconstruct the complete response from stream events
    info!("Final response text: \"{}\"", final_response.text);

    Ok(())
}

/// Handle TTS completion and play success sound
#[tauri::command]
pub async fn handle_tts_completion(
    app_handle: tauri::AppHandle,
    state: State<'_, AppState>,
) -> Result<(), String> {
    info!("TTS completion event received from frontend");

    // Update floating bar manager for TTS finish
    crate::commands::floating_bar::handle_tts_finished(&app_handle).await;

    // Play agent success sound now that TTS has finished
    if let Err(e) = crate::commands::sound::play_agent_success_sound(app_handle.clone(), state.clone()).await {
        warn!("Failed to play success sound after TTS completion: {}", e);
    }

    Ok(())
}

/// Get the personality-focused system prompt for the orchestrator
fn get_orchestrator_personality_prompt() -> String {
    // Use prompt manager for orchestrator personality, with fallback to default
    let prompt_manager = PromptManager::load().unwrap_or_default();
    prompt_manager.get_orchestrator_personality_prompt()
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
        name: tool_names::DELEGATE_TO_BROWSER_AGENT.to_string(),
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
            // Get the current cancellation receiver from app state to pass to specialist
            let app_state = handle.state::<crate::state::AppState>();
            let cancel_rx = app_state.cancel_rx.clone();

            // Execute the specialist agent task with proper error handling
            match execute_specialized_agent_task(provider, "browser", input, handle, cancel_rx).await {
                Ok(result) => Ok(result),
                Err(error_msg) => {
                    // Convert any specialist agent error into a proper error response
                    // This ensures that delegation tool failures are handled gracefully
                    log::warn!("Browser agent delegation failed: {}", error_msg);
                    Ok(serde_json::json!({
                        "success": false,
                        "agent_type": "browser",
                        "error": error_msg,
                        "message": format!("Browser agent failed: {}", error_msg)
                    }))
                }
            }
        }
    };
    orchestrator_provider.register_async_tool(browser_delegation_def, browser_executor).await;

    // Delegate to Desktop Agent
    let desktop_delegation_def = crate::agent::structs::ToolDefinition {
        name: tool_names::DELEGATE_TO_DESKTOP_AGENT.to_string(),
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
            // Get the current cancellation receiver from app state to pass to specialist
            let app_state = handle.state::<crate::state::AppState>();
            let cancel_rx = app_state.cancel_rx.clone();

            // Execute the specialist agent task with proper error handling
            match execute_specialized_agent_task(provider, "desktop", input, handle, cancel_rx).await {
                Ok(result) => Ok(result),
                Err(error_msg) => {
                    // Convert any specialist agent error into a proper error response
                    // This ensures that delegation tool failures are handled gracefully
                    log::warn!("Desktop agent delegation failed: {}", error_msg);
                    Ok(serde_json::json!({
                        "success": false,
                        "agent_type": "desktop",
                        "error": error_msg,
                        "message": format!("Desktop agent failed: {}", error_msg)
                    }))
                }
            }
        }
    };
    orchestrator_provider.register_async_tool(desktop_delegation_def, desktop_executor).await;

    // Delegate to File Agent
    let file_delegation_def = crate::agent::structs::ToolDefinition {
        name: tool_names::DELEGATE_TO_FILE_AGENT.to_string(),
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
            // Get the current cancellation receiver from app state to pass to specialist
            let app_state = handle.state::<crate::state::AppState>();
            let cancel_rx = app_state.cancel_rx.clone();

            // Execute the specialist agent task with proper error handling
            match execute_specialized_agent_task(provider, "file", input, handle, cancel_rx).await {
                Ok(result) => Ok(result),
                Err(error_msg) => {
                    // Convert any specialist agent error into a proper error response
                    // This ensures that delegation tool failures are handled gracefully
                    log::warn!("File agent delegation failed: {}", error_msg);
                    Ok(serde_json::json!({
                        "success": false,
                        "agent_type": "file",
                        "error": error_msg,
                        "message": format!("File agent failed: {}", error_msg)
                    }))
                }
            }
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
    cancel_rx: crate::state::CancelReceiver,
) -> Result<serde_json::Value, String> {
    let task = input["task"].as_str()
        .ok_or_else(|| "Missing required 'task' parameter".to_string())?;
    let context = input["context"].as_str().unwrap_or("");

    info!("Executing {} agent task: {}", agent_type, task);

    // Create a simple memory manager for the specialized agent
            let mut specialist_memory = crate::agent::implementations::memory_manager::AdvancedMemoryManager::new();

    // Clean up any orphaned tool calls that might exist from previous failed executions
    // This provides additional safety against conversation state issues
    if let Err(e) = specialist_memory.clean_orphaned_tool_calls().await {
        log::warn!("Failed to clean orphaned tool calls for {} agent: {}", agent_type, e);
    }

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

    // Execute the specialist agent with proper cancellation signal
    match specialist_runner.run(specialist_query, cancel_rx).await {
        Ok(result) => {
            info!("Specialist {} agent completed successfully", agent_type);

            // Check if the result contains JSX content
            let is_jsx = is_jsx_content(&result);

            if is_jsx {
                // If the result contains JSX, return it directly to preserve JSX rendering
                info!("Specialist {} agent returned JSX content, preserving for rendering", agent_type);
                Ok(serde_json::json!({
                    "success": true,
                    "agent_type": agent_type,
                    "result": result,
                    "is_jsx": true,
                    "message": format!("{} agent completed the task successfully with visual components", agent_type)
                }))
            } else {
                // Standard non-JSX response
                Ok(serde_json::json!({
                    "success": true,
                    "agent_type": agent_type,
                    "result": result,
                    "is_jsx": false,
                    "message": format!("{} agent completed the task successfully", agent_type)
                }))
            }
        }
        Err(e) => {
            // Enhanced error handling for different types of failures
            let error_msg = match &e {
                AgentError::Terminated => {
                    format!("{} agent was cancelled or terminated", agent_type)
                }
                AgentError::MaxStepsReached => {
                    format!("{} agent reached maximum steps", agent_type)
                }
                AgentError::LlmError(msg) if msg.contains("Tool calls without results") => {
                    format!("{} agent failed due to timeout - some tool operations did not complete within the time limit", agent_type)
                }
                AgentError::LlmError(msg) if msg.contains("timed out") => {
                    format!("{} agent failed due to timeout - operation exceeded time limit", agent_type)
                }
                AgentError::ToolError(msg) if msg.contains("timed out") => {
                    format!("{} agent failed due to tool timeout: {}", agent_type, msg)
                }
                _ => {
                    format!("{} agent failed: {}", agent_type, e)
                }
            };

            error!("Specialist {} agent failed: {}", agent_type, error_msg);
            Err(error_msg)
        }
    }
}

/// Get system prompt for specialized agents
fn get_specialist_system_prompt(agent_type: &str) -> String {
    // Use prompt manager for specialist prompts, with fallback to defaults
    let prompt_manager = PromptManager::load().unwrap_or_default();
    prompt_manager.get_specialist_prompt(agent_type)
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
pub async fn get_tts_audio(text: String, state: State<'_, AppState>, app_handle: tauri::AppHandle) -> Result<String, String> {
    // Call the invoke_tts function with the text and state
    crate::tts::invoke_tts(text, state, app_handle).await
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

use std::sync::Arc;
use tracing::{error, info, warn};
use uuid;

use serde_json::Value;
use tauri::{Emitter, Manager, State};

use crate::agent::core::AgentError;
use crate::agent::implementations::{
    agent_runner::DefaultAgentRunner, tool_provider::LocalToolProvider,
};
use crate::agent::prompts::PromptManager;
// use crate::agent::providers::anthropic::ToolChoice;
use crate::agent::providers::config::AgentMode;
use crate::agent::providers::factory::BrainFactory;
use crate::agent::tools::{
    basic_tools::register_basic_tools, browser_tools::get_browser_tool_definitions,
    desktop_tools::setup_tools,
};
use crate::agent::traits::{AgentRunnable, MemoryManager, ToolProvider};
use crate::constants::{agent, events};
use crate::state::AppState;
use crate::utils::{format_system_context_for_agent, gather_system_context};
use crate::utils::atomic_state::{AtomicExecutionCoordinator, AtomicQueue};
// TARS Integration: Import event types
// TODO: Implement event system - currently disabled due to incomplete implementation
// use crate::agent::events::JunoAgentEvent;

/// Agent execution queue system to prevent concurrent execution
struct AgentExecutionQueue {
    /// Atomic execution coordinator to prevent race conditions
    coordinator: Arc<AtomicExecutionCoordinator>,
    /// Thread-safe queue of pending queries
    pending_queries: Arc<AtomicQueue<QueuedQuery>>,
}

#[derive(Debug, Clone)]
struct QueuedQuery {
    id: String,
    query: String,
    _queued_at: std::time::Instant,
    app_handle: tauri::AppHandle,
}

impl AgentExecutionQueue {
    fn new() -> Self {
        Self {
            coordinator: Arc::new(AtomicExecutionCoordinator::new()),
            pending_queries: Arc::new(AtomicQueue::new(10)), // Max 10 pending queries
        }
    }

    /// Queue a new query for execution, cancelling any existing execution
    async fn queue_query(
        &self,
        query: String,
        app_handle: tauri::AppHandle,
        state: tauri::State<'_, AppState>,
    ) -> Result<String, String> {
        let query_id = uuid::Uuid::new_v4().to_string();
        let queued_query = QueuedQuery {
            id: query_id.clone(),
            query,
            _queued_at: std::time::Instant::now(),
            app_handle,
        };

        // Cancel current execution if any
        self.cancel_current_execution(state).await;

        // Clear pending queue and add new query atomically
        self.pending_queries.clear().await;
        self.pending_queries.push(queued_query).await?;
        
        info!("Queued new agent query with ID: {}", query_id);
        Ok(query_id)
    }

    /// Cancel the currently executing agent
    async fn cancel_current_execution(&self, state: tauri::State<'_, AppState>) {
        if let Some(execution_id) = self.coordinator.get_current_execution_id().await {
            info!("Cancelling current agent execution: {}", execution_id);
            state.signal_cancel();
            info!("Signalled cancellation for existing agent execution");

            // Wait for the agent to honour the cancel signal. 500ms was too short for
            // slow tools (browser navigation, network requests). 3s covers the common
            // case while still being responsive; tool executors use select! so they
            // should exit well within this window.
            const CANCEL_TIMEOUT_MS: u128 = 3000;
            let start = std::time::Instant::now();
            while self.coordinator.is_executing().await
                && start.elapsed().as_millis() < CANCEL_TIMEOUT_MS
            {
                tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;
            }

            if self.coordinator.is_executing().await {
                warn!(
                    "Agent execution '{}' did not stop within {}ms after cancel signal — proceeding with next execution anyway",
                    execution_id, CANCEL_TIMEOUT_MS
                );
            } else {
                info!(
                    "Agent execution '{}' stopped after {}ms",
                    execution_id,
                    start.elapsed().as_millis()
                );
            }
        }
    }

    /// Execute the next queued query atomically
    async fn execute_next_query(&self, state: tauri::State<'_, AppState>) -> Option<QueuedQuery> {
        // Get next query from queue
        let query = self.pending_queries.pop().await?;
        
        // Try to start execution atomically
        let guard = match self.coordinator.try_start_execution(query.id.clone()).await {
            Ok(guard) => guard,
            Err(e) => {
                warn!("Failed to start execution: {}", e);
                // Re-queue the query
                let _ = self.pending_queries.push(query).await;
                return None;
            }
        };
        info!("Starting atomic agent execution for query ID: {}", query.id);

        // Execute the actual agent logic
        let result = execute_agent_internal(
            query.query.clone(),
            state,
            query.app_handle.clone()
        ).await;

        // Guard automatically cleans up on drop
        drop(guard);

        // Handle execution result
        match result {
            Ok(()) => {
                info!("Agent execution completed successfully for query ID: {}", query.id);
            }
            Err(e) => {
                error!("Agent execution failed for query {}: {}", query.id, e);
            }
        }

        Some(query)
    }

}

/// Global agent execution queue instance
static AGENT_EXECUTION_QUEUE: std::sync::OnceLock<AgentExecutionQueue> = std::sync::OnceLock::new();

/// Get or initialize the global agent execution queue
fn get_agent_execution_queue() -> &'static AgentExecutionQueue {
    AGENT_EXECUTION_QUEUE.get_or_init(AgentExecutionQueue::new)
}

// Remove unused structs AnthropicContentBlock, AnthropicUsage, AnthropicResponse
// They are now handled in agent/providers/anthropic.rs

// Keep this for payload structure, ensure Clone is derived
pub struct SubmitQueryResult {
    pub text: String,
    pub spoken_text: Option<String>, // Optional separate content for TTS
    pub audio_base64: Option<String>,
    pub agent_state: String,               // Send final state to frontend
    pub screenshot_data: Option<serde_json::Value>, // Optional screenshot data from the session
}

// --- Helper Functions ---

/// Optimized JSX content detection using pattern matching
pub fn is_jsx_content(content: &str) -> bool {
    // Early exit for content that's too short to be JSX
    if content.len() < 3 {
        return false;
    }

    // Quick check for basic JSX syntax
    if !content.contains('<') || !content.contains('>') {
        return false;
    }

    // Use static array for better performance than contains() calls
    const JSX_INDICATORS: &[&str] = &[
        "Card", "Alert", "Button", "Badge", "Circle", "Rectangle", "Triangle",
        "StatusCard", "ColorShowcase", "VisualDemo", "className=", "jsx", "React",
        "WeatherCard", "FileListCard", "SystemStatusCard", "ComparisonCard",
        "TimerCard", "LinkCard", "TaskSummaryCard", "ProgressBar",
        "ActionButton", "QueryButton", "OpenButton", "CopyButton",
        "AnimatedCard", "AnimatedList", "AnimatedProgress", "GlowBadge",
        "ShimmerText", "Confetti", "PulseRing", "AnimatedDivider",
        "Stat", "MiniChart", "AnimatedNumber"
    ];

    // Check for JSX patterns efficiently
    JSX_INDICATORS.iter().any(|&pattern| content.contains(pattern))
}

/// Optimized determination of substantial user communication
/// Uses efficient pattern matching and configurable thresholds
fn is_substantial_user_communication(content: &str) -> bool {
    let trimmed = content.trim();

    // Early exit for empty or very short content
    if trimmed.is_empty() || trimmed.len() < crate::constants::text::limits::MIN_SUBSTANTIAL_COMMUNICATION_LENGTH {
        return false;
    }

    let lower_content = trimmed.to_lowercase();

    // Optimized simple status patterns using static array
    const SIMPLE_STATUS_PATTERNS: &[&str] = &[
        "task completed", "operation successful", "done", "finished", "success",
        "failed", "error", "completed successfully", "operation completed",
        "task finished", "file saved", "file created", "file deleted",
        "command executed", "action completed", "processed successfully",
        "unable to", "couldn't", "can't", "not found", "already exists"
    ];

    // Check for simple status messages with early exit
    if trimmed.len() < crate::constants::text::limits::MAX_SHORT_STATUS_MESSAGE_LENGTH {
        for &pattern in SIMPLE_STATUS_PATTERNS {
            if lower_content.contains(pattern) {
                let words: Vec<&str> = trimmed.split_whitespace().collect();
                if words.len() <= crate::constants::text::limits::MAX_SIMPLE_MESSAGE_WORDS {
                    return false;
                }
            }
        }
    }

    // Check for substantial content indicators (optimized order by likelihood)

    // 1. Word count threshold check (fastest)
    let word_count = trimmed.split_whitespace().count();
    if word_count > crate::constants::text::limits::MIN_SUBSTANTIAL_CONTENT_WORDS {
        return true;
    }

    // 2. Multiple sentences check
    let sentence_endings = trimmed.matches(&['.', '?', '!']).count();
    if sentence_endings >= 2 {
        return true;
    }

    // 3. Long single sentence check
    if trimmed.len() > crate::constants::text::limits::MIN_DETAILED_CONTENT_LENGTH && sentence_endings >= 1 {
        return true;
    }

    // 4. Multiple lines check
    if trimmed.lines().count() > crate::constants::text::analysis::MAX_SIMPLE_CONTENT_LINES {
        return true;
    }

    // 5. Content indicators check (most expensive, done last)
    const CONTENT_INDICATORS: &[&str] = &[
        "here's", "i found", "i've", "discovered", "located", "retrieved",
        "extracted", "analysis", "results show", "data indicates", "information",
        "details", "explanation", "because", "since", "therefore", "however",
        "additionally", "furthermore"
    ];

    CONTENT_INDICATORS.iter().any(|&indicator| lower_content.contains(indicator))
}

// --- Submit Query Function (Refactored with Orchestrator-Based Architecture) ---

#[tauri::command]
pub async fn submit_query(
    query: String,
    state: State<'_, AppState>,
    app_handle: tauri::AppHandle,
) -> Result<(), String> {
    info!("Received query for event-driven processing: {}", query);

    // --- Rate limiting check for AI operations ---
    if let Err(e) = state.rate_limiters.ai_operations.check("default_user").await {
        warn!("Rate limit exceeded for AI operations");
        return Err(e.to_user_message());
    }

    // --- Validate query text ---
    let trimmed_query = query.trim();
    if trimmed_query.is_empty() {
        warn!("Received empty or whitespace-only query, ignoring");
        return Ok(());
    }

    // TODO: Event-driven logging disabled - event system not yet implemented
    // let user_message_event = JunoAgentEvent::UserMessage {
    //     content: trimmed_query.to_string(),
    //     timestamp: chrono::Utc::now().timestamp_millis() as u64,
    //     session_id: Some(crate::agent::events::generate_session_id()),
    // };
    // 
    // // Emit event for logging/observability (non-blocking)
    // if let Err(e) = state.emit_event(user_message_event).await {
    //     warn!("Failed to emit user message event for logging: {}", e);
    //     // Don't fail the entire request for logging issues
    // }
    
    // CRITICAL FIX: Execute agent directly instead of relying on incomplete event system
    // The event-driven refactor was incomplete and caused tool calls to not execute
    info!("Executing agent directly for query: {}", trimmed_query);
    
    // Use the queue system to ensure only one agent runs at a time
    let queue = get_agent_execution_queue();
    let _query_id = queue.queue_query(trimmed_query.to_string(), app_handle.clone(), state.clone()).await
        .map_err(|e| format!("Failed to queue query: {}", e))?;
    
    // Execute the next queued query (will be the one we just queued)
    if queue.execute_next_query(state.clone()).await.is_none() {
        warn!("execute_next_query returned None despite having a queued query - execution may have been blocked by a concurrent run");
    }
    
    Ok(())
}

/// Internal agent execution function - handles the actual agent logic
async fn execute_agent_internal(
    query: String,
    state: tauri::State<'_, AppState>,
    app_handle: tauri::AppHandle,
) -> Result<(), String> {
    // Generate a unique execution ID for this agent run
    let execution_id = uuid::Uuid::new_v4().to_string();

    // Mark agent execution as started with max iterations (both modes use 15)
    let _ = state.mark_agent_execution_started_with_steps(
        execution_id.clone(),
        agent::config::MAX_ITERATIONS,
    );
    info!(
        "Starting new agent execution with ID: {} (max steps: {})",
        execution_id,
        agent::config::MAX_ITERATIONS
    );

    // Apply big cursor scaling if enabled — RAII guard restores on all exit paths
    let _cursor_guard = match app_handle.try_state::<crate::settings::manager::SettingsManager>() {
        Some(mgr) => match mgr.get_agent_settings().await {
            Ok(s) if s.big_cursor_enabled => {
                info!("[CursorScale] Big cursor enabled, scaling to {:.1}x", s.big_cursor_scale);
                crate::cursor_scale::CursorScaleGuard::new(s.big_cursor_scale as f64)
            }
            Ok(_) => crate::cursor_scale::CursorScaleGuard::noop(),
            Err(e) => {
                warn!("[CursorScale] Failed to read agent settings, skipping cursor scaling: {}", e);
                crate::cursor_scale::CursorScaleGuard::noop()
            }
        },
        None => {
            warn!("[CursorScale] SettingsManager not found in managed state, skipping cursor scaling");
            crate::cursor_scale::CursorScaleGuard::noop()
        }
    };

    // TODO: TARS Integration disabled - event system not yet implemented
    // let agent_run_start_event = JunoAgentEvent::AgentRunStart {
    //     session_id: execution_id.clone(),
    //     agent_type: "orchestrator".to_string(),
    //     max_iterations: agent::config::MAX_ITERATIONS,
    //     user_query: query.clone(),
    //     timestamp: chrono::Utc::now().timestamp_millis() as u64,
    // };
    // if let Err(e) = state.emit_agent_event(agent_run_start_event).await {
    //     warn!("Failed to emit agent run start event: {}", e);
    // }

    // --- Notify Floating Bar Manager that Agent Started ---
    // Skip in headless mode — no floating bar UI to update
    if !crate::cli::headless::is_headless_mode() {
        let app_handle_for_bar_start = app_handle.clone();
        tauri::async_runtime::spawn(async move {
            crate::commands::ui_commands::handle_agent_started(&app_handle_for_bar_start).await;
        });
    }

    // Reset cancellation signal BEFORE registering escape key
    // so any escape press after registration is honored
    state.reset_cancel();
    info!("Reset cancellation signal for new agent execution");

    // Register escape key for cancellation during agent execution
    // Skip in headless mode — no keyboard to listen to
    if !crate::cli::headless::is_headless_mode() {
        {
            let coordinator = crate::commands::escape_key_coordinator::get_escape_key_coordinator();
            if let Err(e) = coordinator.register_escape_user(&app_handle, "agent_execution").await {
                warn!("Failed to configure escape key for agent execution: {} - continuing without escape key cancellation", e);
            }
        }
    }

    let trimmed_query = query.trim();

    // --- Gather System Context ---
    let system_context = match gather_system_context(Some(&*state)).await {
        Ok(context) => {
            info!("System context gathered successfully");
            Some(context)
        }
        Err(e) => {
            warn!("Failed to retrieve system context: {}", e);
            None
        }
    };

    let cancel_rx = state.cancel_rx.clone();

    // --- Get Persistent Memory Manager (Orchestrator maintains conversation memory) ---
    let memory_manager_arc = state.get_memory_manager().await;

    // Clean up orphaned tool calls before starting
    {
        let mut memory_manager = memory_manager_arc.lock().await;

        // Generate a current execution ID to distinguish between different agent executions
        let current_execution_id = uuid::Uuid::new_v4().to_string();

        // Mark current execution so new tools won't be considered orphaned
        if let Err(e) = memory_manager.set_current_execution_id(&current_execution_id).await {
            warn!("Failed to set current execution ID for orchestrator: {}", e);
        }

        // Use the safe clean method that only removes tool calls from previous executions
        if let Err(e) = memory_manager.clean_orphaned_tool_calls_from_previous_executions().await {
            warn!("Failed to clean orphaned tool calls: {}", e);
        }

        // Also clean up orphaned tool results that have no corresponding tool calls
        match memory_manager.clean_orphaned_tool_results().await {
            Ok(cleaned_count) if cleaned_count > 0 => {
                info!(
                    "Cleaned {} orphaned tool results before agent execution",
                    cleaned_count
                );
            }
            Ok(_) => {} // No orphaned results found
            Err(e) => {
                warn!("Failed to process clean orphaned tool results: {}", e);
            }
        }
    }

    // --- Setup Tool Provider Based on Agent Mode ---
    let agent_mode = BrainFactory::get_agent_mode_with_app_handle(&app_handle).await;
    info!("Using agent mode: {:?}", agent_mode);
    
    // TEMPORARY DEBUG FIX: Force single agent mode for computer use tasks
    // This ensures direct tool access instead of delegation
    let lower_query = trimmed_query.to_lowercase();
    let contains_computer_keywords = lower_query.contains("click")
        || lower_query.contains("drag")
        || lower_query.contains("mouse")
        || lower_query.contains("screenshot")
        || lower_query.contains("computer")
        || lower_query.contains("spiral")
        || lower_query.contains("draw");
        
    let effective_agent_mode = if contains_computer_keywords {
        warn!("FORCING SINGLE AGENT MODE for computer use task: {}", trimmed_query);
        AgentMode::Single
    } else {
        agent_mode
    };
    info!("Effective agent mode: {:?}", effective_agent_mode);

    // Read companion mode once; used in both Single and Multi branches
    let companion_mode = match app_handle.try_state::<crate::settings::manager::SettingsManager>() {
        Some(mgr) => mgr.get_agent_settings().await
            .map(|s| s.companion_mode)
            .unwrap_or(false),
        None => false,
    };
    if companion_mode {
        info!("🔍 Companion mode active — computer use tools will NOT be registered");
    }

    let agent_result = match effective_agent_mode {
        AgentMode::Single => {
            info!("🔧 Setting up SINGLE AGENT mode with direct tools (no delegation)");

            // Create a clean tool provider for single agent with direct tools only
            let mut single_agent_tool_provider = LocalToolProvider::with_app_handle(app_handle.clone());

            // Register basic file/shell tools for single agent
            register_basic_tools(&mut single_agent_tool_provider).await;
            info!("✅ Registered basic tools for single agent");

            // Register desktop tools for single agent
            let _shared_tool_provider = setup_tools(
                &mut single_agent_tool_provider,
                state.clone(),
                app_handle.clone(),
            ).await;
            info!("✅ Registered desktop tools for single agent");

            // Register browser tools for single agent
            let browser_definitions = get_browser_tool_definitions();
            for definition in browser_definitions {
                let tool_name = definition.name.clone();
                let app_handle_for_tool_executor = app_handle.clone();

                let executor = move |input: Value| {
                    let app_handle_captured = app_handle_for_tool_executor.clone();
                    let current_tool_name_captured = tool_name.clone();
                    async move {
                        let state_from_handle = app_handle_captured.state::<AppState>();

                        let browser_controller_instance =
                            match state_from_handle.get_or_init_browser_controller().await {
                                Ok(controller) => controller,
                                Err(e) => {
                                    let err_msg = format!(
                                        "Failed to start {}: {}",
                                        current_tool_name_captured, e
                                    );
                                    error!("{}", err_msg);
                                    return Err(err_msg);
                                }
                            };

                        let result = match current_tool_name_captured.as_str() {
                            "browser_navigate" => browser_controller_instance.navigate(&input).await,
                            "browser_extract_content" => {
                                browser_controller_instance.extract_content(&input).await
                            }
                            "browser_interact" => browser_controller_instance.interact(&input).await,
                            "browser_get_current_url" => {
                                browser_controller_instance.get_current_url(&input).await
                            }
                            "browser_screenshot" => browser_controller_instance.screenshot(&input).await,
                            _ => Err(AgentError::ToolNotFound(current_tool_name_captured)),
                        };

                        match result {
                            Ok(tool_result) => Ok(tool_result.output),
                            Err(agent_error) => Err(agent_error.to_string()),
                        }
                    }
                };
                single_agent_tool_provider
                    .register_async_tool(definition.clone(), executor)
                    .await;
                info!("✅ Registered browser tool for single agent: {}", definition.name);
            }

            // Register the complete Anthropic Computer Use tools (computer, bash, str_replace_based_edit_tool)
            // Skipped in companion mode — agent observes only, no actions.
            if !companion_mode {
                if let Err(e) = BrainFactory::register_computer_use_tools(
                    &mut single_agent_tool_provider,
                    app_handle.clone(),
                )
                .await
                {
                    let err_msg = format!("Failed to register Computer Use tools for single agent: {}", e);
                    error!("{}", err_msg);
                    let coordinator = crate::commands::escape_key_coordinator::get_escape_key_coordinator();
                    let _ = coordinator.unregister_escape_user(&app_handle, "agent_execution").await;
                    state.mark_agent_execution_finished();
                    return Err(err_msg);
                }
                info!("✅ Registered full Computer Use tools for single agent mode");
            }

            // Create single agent brain with enriched system prompt including available MCP tools
            let brain_result = {
                // Load prompt manager to render system prompt with variables
                let settings_manager = match crate::settings::manager::SettingsManager::new(app_handle.clone()) {
                    Ok(m) => Some(m),
                    Err(e) => {
                        warn!("Failed to create settings manager: {}. Using default prompts.", e);
                        None
                    }
                };

                // Collect available tool names including MCP tools (enabled only)
                let mut available_tool_names: Vec<String> = {
                    match single_agent_tool_provider.list_tools().await {
                        Ok(defs) => defs.into_iter().map(|d| d.name).collect(),
                        Err(_) => Vec::new(),
                    }
                };

                // Derive available MCP tools from MCP manager
                let mcp_tools: Vec<String> = {
                    let mcp_manager = state.get_mcp_manager().await;
                    let guard = mcp_manager.lock().await;
                    guard.get_all_tools().await.into_iter().map(|t| t.tool_definition.name).collect()
                };

                // Ensure MCP tools are included in the total list
                available_tool_names.extend(mcp_tools.clone());

                // Build prompt context
                let prompt_manager = if let Some(ref sm) = settings_manager {
                    PromptManager::load_from_centralized_settings(sm).await.unwrap_or_else(|_| PromptManager::new())
                } else {
                    PromptManager::new()
                };

                let prompt_context = crate::agent::prompts::types::PromptContext {
                    user_preferences: None,
                    task_context: None,
                    available_tools: available_tool_names,
                    available_mcp_tools: mcp_tools,
                    provider_constraints: None,
                    custom_variables: std::collections::HashMap::new(),
                };

                let prompt_type = if companion_mode {
                    crate::agent::prompts::types::PromptType::CompanionMode
                } else {
                    crate::agent::prompts::types::PromptType::SystemDefault
                };
                let system_prompt = prompt_manager
                    .get_prompt(prompt_type, Some(prompt_context))
                    .unwrap_or_else(|_| prompt_manager.get_default_system_prompt());

                BrainFactory::create_brain_with_system_prompt(system_prompt, Some(&app_handle))
            };
            let brain = match brain_result {
                Ok(brain) => brain,
                Err(e) => {
                    let err_msg = format!("Failed to initialize single agent brain: {}", e);
                    error!("{}", err_msg);

                    // Emit error via streaming events
                    let error_message_id = uuid::Uuid::new_v4().to_string();
                    crate::agent::tool_logger::emit_stream_start(
                        &app_handle,
                        error_message_id.clone(),
                    );
                    crate::agent::tool_logger::emit_streaming_text_chunk(
                        &app_handle,
                        err_msg.clone(),
                        Some(error_message_id.clone()),
                        None,
                    );
                    crate::agent::tool_logger::emit_stream_end(
                        &app_handle,
                        error_message_id,
                        err_msg.clone(),
                    );
                    let coordinator = crate::commands::escape_key_coordinator::get_escape_key_coordinator();
                    let _ = coordinator.unregister_escape_user(&app_handle, "agent_execution").await;
                    state.mark_agent_execution_finished();
                    return Err(err_msg);
                }
            };
            info!("✅ Single agent brain initialized with enriched system prompt");

            // Create single agent runner with direct tools (no delegation)
            let mut single_agent_runner = DefaultAgentRunner::with_boxed_brain(
                {
                    let memory_guard = memory_manager_arc.lock().await;
                    memory_guard.clone()
                },
                single_agent_tool_provider,
                brain,
                agent::config::MAX_ITERATIONS,
                app_handle.clone(),
            );
            info!("✅ Single agent runner created with direct tools (no delegation capabilities)");

            info!("🚀 Starting single agent run...");

            // Prepare the query with system context
            let contextual_query = if let Some(ref context) = system_context {
                format!(
                    "{}\n\nUser Query: {}",
                    format_system_context_for_agent(context),
                    trimmed_query
                )
            } else {
                trimmed_query.to_string()
            };

            let result = single_agent_runner.run(contextual_query, cancel_rx).await;
            result
        }
        AgentMode::Multi => {
            info!("🔧 Setting up MULTI-AGENT mode with orchestrator delegation");

            // Create tool provider for specialist agents (used by delegation system)
            let mut specialist_tool_provider = LocalToolProvider::with_app_handle(app_handle.clone());

            // Register basic file/shell tools for specialists
            register_basic_tools(&mut specialist_tool_provider).await;
            info!("✅ Registered basic tools for specialist agents");

            // Setup desktop tools for specialists and get the shared provider
            let shared_tool_provider = setup_tools(
                &mut specialist_tool_provider,
                state.clone(),
                app_handle.clone(),
            ).await;

            // Register the complete Anthropic Computer Use tools (computer, bash, str_replace_based_edit_tool)
            // Skipped in companion mode — agent observes only, no actions.
            if !companion_mode {
                if let Err(e) = BrainFactory::register_computer_use_tools(
                    &mut specialist_tool_provider,
                    app_handle.clone(),
                )
                .await
                {
                    let err_msg = format!("Failed to register Computer Use tools for specialist agent: {}", e);
                    error!("{}", err_msg);
                    let coordinator = crate::commands::escape_key_coordinator::get_escape_key_coordinator();
                    let _ = coordinator.unregister_escape_user(&app_handle, "agent_execution").await;
                    state.mark_agent_execution_finished();
                    return Err(err_msg);
                }
                info!("✅ Registered full Computer Use tools for specialist mode");
            }

            // Extract the tool provider from Arc<Mutex<>> for specialist agent creation
            let specialist_agent_tool_provider = {
                let guard = shared_tool_provider.lock().await;
                guard.clone()
            };

            // Register browser tools for specialist agents
            let browser_definitions = get_browser_tool_definitions();
            for definition in browser_definitions {
                let tool_name = definition.name.clone();
                let app_handle_for_tool_executor = app_handle.clone();

                let executor = move |input: Value| {
                    let app_handle_captured = app_handle_for_tool_executor.clone();
                    let current_tool_name_captured = tool_name.clone();
                    async move {
                        let state_from_handle = app_handle_captured.state::<crate::state::AppState>();

                        let browser_controller_instance =
                            match state_from_handle.get_or_init_browser_controller().await {
                                Ok(controller) => controller,
                                Err(e) => {
                                    let err_msg = format!(
                                        "Failed to start {}: {}",
                                        current_tool_name_captured, e
                                    );
                                    error!("{}", err_msg);
                                    return Err(err_msg);
                                }
                            };

                        let result = match current_tool_name_captured.as_str() {
                            "browser_navigate" => browser_controller_instance.navigate(&input).await,
                            "browser_extract_content" => {
                                browser_controller_instance.extract_content(&input).await
                            }
                            "browser_interact" => browser_controller_instance.interact(&input).await,
                            "browser_get_current_url" => {
                                browser_controller_instance.get_current_url(&input).await
                            }
                            "browser_screenshot" => browser_controller_instance.screenshot(&input).await,
                            _ => Err(AgentError::ToolNotFound(current_tool_name_captured)),
                        };

                        match result {
                            Ok(tool_result) => Ok(tool_result.output),
                            Err(agent_error) => Err(agent_error.to_string()),
                        }
                    }
                };
                // Register browser tools on the shared provider instance for specialists
                {
                    let guard = shared_tool_provider.lock().await;
                    guard
                        .register_async_tool(definition.clone(), executor)
                        .await;
                }
                info!("✅ Registered browser tool for specialist agents: {}", definition.name);
            }

            // Create orchestrator provider first and enrich it with MCP tools so it can directly use custom MCP tools
            let mut orchestrator_tool_provider = LocalToolProvider::with_app_handle(app_handle.clone());
            let mcp_manager_for_orchestrator = state.get_mcp_manager().await;
            orchestrator_tool_provider.set_mcp_manager(mcp_manager_for_orchestrator.clone());
            if let Err(e) = orchestrator_tool_provider.refresh_mcp_tools().await {
                warn!("Failed to refresh MCP tools for orchestrator provider: {}", e);
            }

            // Build orchestrator system prompt with available MCP tools listed for visibility
            let orchestrator_brain_result = {
                // Settings → PromptManager
                let settings_manager = match crate::settings::manager::SettingsManager::new(app_handle.clone()) {
                    Ok(m) => Some(m),
                    Err(e) => {
                        warn!("Failed to create settings manager: {}. Using default prompts.", e);
                        None
                    }
                };

                let prompt_manager = if let Some(ref sm) = settings_manager {
                    PromptManager::load_from_centralized_settings(sm).await.unwrap_or_else(|_| PromptManager::new())
                } else {
                    PromptManager::new()
                };

                // Collect tool names the orchestrator can see (delegation + MCP)
                let available_tool_names: Vec<String> = orchestrator_tool_provider
                    .list_tools()
                    .await
                    .unwrap_or_default()
                    .into_iter()
                    .map(|d| d.name)
                    .collect();

                // Collect MCP tool names explicitly
                let mcp_tool_names: Vec<String> = {
                    let guard = mcp_manager_for_orchestrator.lock().await;
                    guard
                        .get_all_tools()
                        .await
                        .into_iter()
                        .map(|t| t.tool_definition.name)
                        .collect()
                };

                // Prepare prompt context
                let prompt_context = crate::agent::prompts::types::PromptContext {
                    user_preferences: None,
                    task_context: None,
                    available_tools: available_tool_names,
                    available_mcp_tools: mcp_tool_names,
                    provider_constraints: None,
                    custom_variables: std::collections::HashMap::new(),
                };

                let system_prompt = prompt_manager
                    .get_prompt(crate::agent::prompts::types::PromptType::OrchestratorPersonality, Some(prompt_context))
                    .unwrap_or_else(|_| prompt_manager.get_orchestrator_personality_prompt());

                BrainFactory::create_brain_with_system_prompt(system_prompt, Some(&app_handle))
            };
            let orchestrator_brain = match orchestrator_brain_result {
                Ok(brain) => brain,
                Err(e) => {
                    let err_msg = format!("Failed to initialize orchestrator brain: {}", e);
                    error!("{}", err_msg);

                    // Emit error via streaming events
                    let error_message_id = uuid::Uuid::new_v4().to_string();
                    crate::agent::tool_logger::emit_stream_start(
                        &app_handle,
                        error_message_id.clone(),
                    );
                    crate::agent::tool_logger::emit_streaming_text_chunk(
                        &app_handle,
                        err_msg.clone(),
                        Some(error_message_id.clone()),
                        None,
                    );
                    crate::agent::tool_logger::emit_stream_end(
                        &app_handle,
                        error_message_id,
                        err_msg.clone(),
                    );
                    let coordinator = crate::commands::escape_key_coordinator::get_escape_key_coordinator();
                    let _ = coordinator.unregister_escape_user(&app_handle, "agent_execution").await;
                    state.mark_agent_execution_finished();
                    return Err(err_msg);
                }
            };
            info!("✅ Orchestrator brain initialized with MCP tools visibility");

            // Register delegation tools for the orchestrator (on the same provider enriched with MCP)
            register_orchestrator_delegation_tools(
                &mut orchestrator_tool_provider,
                specialist_agent_tool_provider,
                app_handle.clone(),
            )
            .await;
            info!("✅ Registered delegation tools for orchestrator (no direct tools)");

            // Create the orchestrator agent runner with delegation-only tools
            let mut orchestrator_runner = DefaultAgentRunner::with_boxed_brain(
                {
                    let memory_guard = memory_manager_arc.lock().await;
                    memory_guard.clone()
                },
                orchestrator_tool_provider,
                orchestrator_brain,
                agent::config::MAX_ITERATIONS,
                app_handle.clone(),
            );
            info!("✅ Orchestrator runner created with delegation tools only");

            info!("🚀 Starting multi-agent orchestrator run...");

            // Prepare the query with system context for orchestrator
            let contextual_query = if let Some(ref context) = system_context {
                format!(
                    "{}\n\nUser Query: {}",
                    format_system_context_for_agent(context),
                    trimmed_query
                )
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
    info!(
        "Agent execution marked as finished for ID: {}",
        execution_id
    );

    // TODO: TARS Integration disabled - event system not yet implemented
    // let agent_run_end_event = JunoAgentEvent::AgentRunEnd {
    //     session_id: execution_id.clone(),
    //     status: match &agent_result {
    //         Ok(_) => "completed".to_string(),
    //         Err(AgentError::Terminated) => "cancelled".to_string(),
    //         Err(AgentError::MaxStepsReached) => "max_steps_reached".to_string(),
    //         Err(_) => "failed".to_string(),
    //     },
    //     iterations: state.get_agent_current_step().unwrap_or(0),
    //     elapsed_ms: 0, // We could track this if needed
    //     timestamp: chrono::Utc::now().timestamp_millis() as u64,
    // };
    // if let Err(e) = state.emit_agent_event(agent_run_end_event).await {
    //     warn!("Failed to emit agent run end event: {}", e);
    // }

    // Unregister escape key as agent execution is complete
    // Skip in headless mode — escape key was never registered
    if !crate::cli::headless::is_headless_mode() {
        let coordinator = crate::commands::escape_key_coordinator::get_escape_key_coordinator();
        if let Err(e) = coordinator.unregister_escape_user(&app_handle, "agent_execution").await {
            warn!("Failed to unregister escape key after agent execution: {} - continuing anyway", e);
        }
    }

    // --- Process Agent Result ---
    let final_response = match agent_result {
        Ok(message) => {
            // Note: Success sound will be played after TTS completes (or immediately if TTS is disabled)

            // TODO: TARS Integration disabled - event system not yet implemented
            // let assistant_message_event = JunoAgentEvent::AssistantMessage {
            //     content: message.clone(),
            //     timestamp: chrono::Utc::now().timestamp_millis() as u64,
            //     session_id: Some(execution_id.clone()),
            // };
            // if let Err(e) = state.emit_agent_event(assistant_message_event).await {
            //     warn!("Failed to emit assistant message event: {}", e);
            // }

            SubmitQueryResult {
                text: message.clone(),
                spoken_text: None, // TTS content is now handled during streaming via XML tags
                audio_base64: None, // Will be set below if TTS is enabled
                agent_state: "Finished".to_string(),
                screenshot_data: None, // Capture screenshot if needed
            }
        }
        Err(e) => {
            error!("Agent run failed: {}", e);

            // TODO: TARS Integration disabled - event system not yet implemented
            // let error_event = JunoAgentEvent::ErrorOccurred {
            //     error_type: match &e {
            //         AgentError::Terminated => "user_cancelled".to_string(),
            //         AgentError::MaxStepsReached => "max_steps_reached".to_string(),
            //         AgentError::LlmError(_) => "llm_error".to_string(),
            //         _ => "unknown".to_string(),
            //     },
            //     message: e.to_string(),
            //     recoverable: !matches!(e, AgentError::Terminated),
            //     timestamp: chrono::Utc::now().timestamp_millis() as u64,
            //     context: Some(serde_json::json!({
            //         "session_id": execution_id,
            //         "agent_type": "orchestrator"
            //     })),
            // };
            // if let Err(e) = state.emit_agent_event(error_event).await {
            //     warn!("Failed to emit error event: {}", e);
            // }

            // Check if this is a network-related error
            let error_message = e.to_string();
            let is_network_error = crate::utils::network::is_network_error(&error_message);

            let (state_str, msg) = match e {
                AgentError::Terminated => {
                    // Play agent attention sound for cancellation (less intrusive than error)
                    if !crate::cli::headless::is_headless_mode() {
                        if let Err(e) = crate::commands::sound::play_agent_attention_sound(
                            app_handle.clone(),
                            state.clone(),
                        )
                        .await
                        {
                            warn!("{}", format!("{}: {}", "Failed to play cancellation sound", e));
                        }
                    }
                    (
                        "Cancelled".to_string(),
                        "Agent execution was cancelled.".to_string(),
                    )
                }
                AgentError::MaxStepsReached => {
                    // Play agent error sound for failure
                    if !crate::cli::headless::is_headless_mode() {
                        if let Err(e) = crate::commands::sound::play_agent_error_sound(
                            app_handle.clone(),
                            state.clone(),
                        )
                        .await
                        {
                            warn!("{}", format!("{}: {}", "Failed to play error sound", e));
                        }
                    }
                    (
                        "Failed".to_string(),
                        "Agent reached maximum steps.".to_string(),
                    )
                }
                AgentError::LlmError(_) if is_network_error => {
                    // Handle network errors gracefully - use friendly message instead of raw error
                    warn!("LLM error appears to be network-related: {}", error_message);

                    // Play different sound for network issues (less alarming)
                    if !crate::cli::headless::is_headless_mode() {
                        if let Err(e) = crate::commands::sound::play_agent_attention_sound(
                            app_handle.clone(),
                            state.clone(),
                        )
                        .await
                        {
                            warn!("{}", format!("{}: {}", "Failed to play network error sound", e));
                        }
                    }

                    (
                        "Offline".to_string(),
                        crate::utils::network::get_offline_message(),
                    )
                }
                _ => {
                    // Play agent error sound for other failures
                    if !crate::cli::headless::is_headless_mode() {
                        if let Err(e) = crate::commands::sound::play_agent_error_sound(
                            app_handle.clone(),
                            state.clone(),
                        )
                        .await
                        {
                            warn!("{}", format!("{}: {}", "Failed to play error sound", e));
                        }
                    }
                    ("Failed".to_string(), format!("Agent error: {}", e))
                }
            };

            // For network errors, temporarily switch to system TTS to ensure the message is heard
            let should_force_system_tts = is_network_error || state_str == "Offline";
            let original_tts_provider = if should_force_system_tts {
                let current_provider = state.get_tts_provider().unwrap_or_default();
                // Switch to system TTS for network errors
                if let Ok(()) = state.set_tts_provider("system".to_string()) {
                    info!("Temporarily switched to system TTS for offline/network error message");
                }
                Some(current_provider)
            } else {
                None
            };

            let result = SubmitQueryResult {
                text: msg.clone(),
                spoken_text: None,  // Error messages use same content for speech
                audio_base64: None, // Will be set below if TTS is enabled
                agent_state: state_str,
                screenshot_data: None,
            };

            // Store the original provider to restore later if needed
            if let Some(original_provider) = original_tts_provider {
                // We'll restore it after TTS processing below
                let state_ref = state.inner().clone();
                tauri::async_runtime::spawn(async move {
                    // Give TTS time to process
                    tokio::time::sleep(std::time::Duration::from_millis(100)).await;
                    if let Ok(()) = state_ref.set_tts_provider(original_provider) {
                        info!("Restored original TTS provider after offline/network error message");
                    }
                });
            }

            result
        }
    };

    // --- Generate TTS Audio ---
    // With XML-based TTS extraction in streaming mode, TTS is already processed immediately
    // Disable fallback TTS processing to prevent duplicates
    let _should_process_final_tts = false; // Always skip final TTS - immediate TTS handles it

    let _tts_enabled = false; // TTS is now handled entirely via immediate processing during streaming

    // Success sound will be played after TTS completion via handle_tts_completion()
    // Skip immediate sound to prevent double-playing
    if final_response.agent_state == "Finished" {
        info!("Agent completed successfully. Success sound will play after TTS completion.");
    }

    info!(
        "Agent run complete. Final state: {}",
        final_response.agent_state
    );

    // --- Notify Floating Bar Manager that Agent Stopped ---
    // Skip in headless mode — no floating bar UI
    if !crate::cli::headless::is_headless_mode() {
        let app_handle_for_bar_stop = app_handle.clone();
        tauri::async_runtime::spawn(async move {
            crate::commands::ui_commands::handle_agent_stopped(&app_handle_for_bar_stop).await;
        });

        // --- Update Floating Bar Manager with Completion Details ---
        let app_handle_for_bar = app_handle.clone();
        let agent_state_for_bar = final_response.agent_state.clone();
        let text_for_bar = final_response.text.clone();
        tauri::async_runtime::spawn(async move {
            crate::commands::ui_commands::handle_backend_response(
                &app_handle_for_bar,
                Some(text_for_bar),
                agent_state_for_bar,
            )
            .await;
        });
    }

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
            if let Err(e) = error_event_handle.emit(events::agent::ERROR, event_data) {
                warn!("{}", format!("{}: {}", "Failed to emit agent-error event", e));
            }
        });
    }

    // NOTE: stream_end event is already emitted during streaming by the Anthropic provider
    // Emitting a second stream_end with a new message_id would cause duplicate messages
    // The frontend reconstructs the complete response from the streaming events
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

    // Update floating bar manager for TTS finish — skip in headless mode
    if !crate::cli::headless::is_headless_mode() {
        crate::commands::ui_commands::handle_tts_finished(&app_handle).await;
    }

    // Play agent success sound now that TTS has finished — skip in headless mode
    if !crate::cli::headless::is_headless_mode() {
        if let Err(e) =
            crate::commands::sound::play_agent_success_sound(app_handle.clone(), state.clone()).await
        {
            warn!("{}", format!("{}: {}", "Failed to play success sound after TTS completion", e));
        }
    }

    Ok(())
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
    let browser_delegation_def = crate::agent::core::ToolDefinition {
        name: agent::tool_names::DELEGATE_TO_BROWSER_AGENT.to_string(),
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
        api_type: None,
        beta_flag: None,
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
            match execute_specialized_agent_task(provider, "browser", input, handle, cancel_rx)
                .await
            {
                Ok(result) => Ok(result),
                Err(error_msg) => {
                    // Convert any specialist agent error into a proper error response
                    // This ensures that delegation tool failures are handled gracefully
                    warn!("Browser agent delegation failed: {}", error_msg);
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
    orchestrator_provider
        .register_async_tool(browser_delegation_def, browser_executor)
        .await;

    // Delegate to Desktop Agent
    let desktop_delegation_def = crate::agent::core::ToolDefinition {
        name: agent::tool_names::DELEGATE_TO_DESKTOP_AGENT.to_string(),
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
        api_type: None,
        beta_flag: None,
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
            match execute_specialized_agent_task(provider, "desktop", input, handle, cancel_rx)
                .await
            {
                Ok(result) => Ok(result),
                Err(error_msg) => {
                    // Convert any specialist agent error into a proper error response
                    // This ensures that delegation tool failures are handled gracefully
                    warn!("Desktop agent delegation failed: {}", error_msg);
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
    orchestrator_provider
        .register_async_tool(desktop_delegation_def, desktop_executor)
        .await;

    // Delegate to File Agent
    let file_delegation_def = crate::agent::core::ToolDefinition {
        name: agent::tool_names::DELEGATE_TO_FILE_AGENT.to_string(),
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
        api_type: None,
        beta_flag: None,
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
    orchestrator_provider
        .register_async_tool(file_delegation_def, file_executor)
        .await;

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
    let task = input["task"]
        .as_str()
        .ok_or_else(|| "Missing required 'task' parameter".to_string())?;
    let context = input["context"].as_str().unwrap_or("");

    info!("Executing {} agent task: {}", agent_type, task);

    // FIXED: Create a completely independent memory manager for specialist agents
    // This prevents ANY tool calls from being tracked in the orchestrator's memory space
    // which would cause the "orphaned tool call" API error
    let specialist_memory = {
        use crate::agent::implementations::memory_manager::AdvancedMemoryManager;
        use crate::agent::core::{Message, Role};

        // Create a completely fresh memory manager for specialist
        let mut fresh_memory = AdvancedMemoryManager::new();

        // Extract only minimal context for the specialist
        // This prevents sharing ANY tool calls between orchestrator and specialist
        let query = if context.is_empty() {
            task.to_string()
        } else {
            format!("{}\n\nAdditional context: {}", task, context)
        };

        // Initialize with just the task as user message
        let user_message = Message {
            role: Role::User,
            content: query,
            tool_calls: None,
            tool_call_id: None,
            name: None,
        };

        if let Err(e) = fresh_memory.add_message(user_message).await {
            warn!("Failed to add task to specialist memory: {}", e);
        }

        // Return the completely isolated memory manager
        fresh_memory
    };

    // Create appropriate brain for the specialist agent with focused system prompt
    let system_prompt = get_specialist_system_prompt(agent_type, &app_handle).await;
    let specialist_brain = match BrainFactory::create_brain_with_system_prompt(system_prompt, Some(&app_handle)) {
        Ok(brain) => brain,
        Err(e) => return Err(format!("Failed to create specialist brain: {}", e)),
    };

    // Create specialist agent runner with the isolated memory
    let mut specialist_runner = DefaultAgentRunner::with_boxed_brain(
        specialist_memory,
        (*tool_provider).clone(), // Clone the LocalToolProvider from the Arc
        specialist_brain,
        agent::config::MAX_ITERATIONS, // Use same max iterations as orchestrator
        app_handle,
    );

    // Execute the specialist agent with proper cancellation signal
    match specialist_runner.run(task.to_string(), cancel_rx).await {
        Ok(result) => {
            info!("Specialist {} agent completed successfully", agent_type);

            // Check if the result contains JSX content
            let is_jsx = is_jsx_content(&result);

            // Determine if the specialist actually handled user communication
            // User communication is considered handled if:
            // 1. The result contains JSX content (visual components for user)
            // 2. The result contains substantial text content (more than just status messages)
            // 3. The result is not just a simple success/failure indicator
            let user_communication_handled = is_jsx || is_substantial_user_communication(&result);

            // Format a rich result for the orchestrator
            Ok(serde_json::json!({
                "success": true,
                "agent_type": agent_type,
                "result": result,
                "jsx_content": is_jsx,
                "user_communication_handled": user_communication_handled,
                "message": format!("{} agent completed task successfully", agent_type)
            }))
        }
        Err(e) => {
            // Format user-friendly error message based on error type
            let error_msg = match e {
                AgentError::MaxStepsReached => {
                    format!("{} agent ran out of iterations - task was too complex to complete in the allowed number of steps", agent_type)
                }
                AgentError::Timeout(_) => {
                    format!("{} agent failed due to timeout - some tool operations did not complete within the time limit", agent_type)
                }
                AgentError::LlmError(msg) if msg.contains("timed out") => {
                    format!(
                        "{} agent failed due to timeout - operation exceeded time limit",
                        agent_type
                    )
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

/// Get specialist system prompt for delegation task execution
async fn get_specialist_system_prompt(agent_type: &str, app_handle: &tauri::AppHandle) -> String {
    // Create settings manager from app handle
    let settings_manager = match crate::settings::manager::SettingsManager::new(app_handle.clone())
    {
        Ok(manager) => manager,
        Err(e) => {
            warn!("Failed to create settings manager: {}. Using defaults.", e);
            return PromptManager::new().get_specialist_prompt(agent_type);
        }
    };

    // FIXED: Use prompt manager with proper centralized settings loading
    let prompt_manager = PromptManager::load_from_centralized_settings(&settings_manager).await.unwrap_or_else(|e| {
        warn!("Failed to load prompt configuration from centralized settings: {}. Using defaults.", e);
        PromptManager::new()
    });
    prompt_manager.get_specialist_prompt(agent_type)
}

// --- Browser Cleanup Function ---

#[tauri::command]
pub async fn cleanup_browser(app_handle: tauri::AppHandle) -> Result<(), String> {
    info!("Cleaning up browser resources...");

    // Get the app state to access the browser controller
    let state = app_handle.state::<AppState>();

    // Acquire lock on the browser controller
    let mut controller_guard = state.browser_controller.lock().await;

    // If we have a browser controller, clean it up
    if let Some(controller) = controller_guard.take() {
        if let Err(e) = controller.cleanup().await {
            error!("Failed to clean up browser controller: {}", e);
            return Err(format!("Failed to clean up browser: {}", e));
        }
        info!("Browser controller cleaned up successfully");
    } else {
        info!("No browser controller to clean up");
    }

    info!("Browser cleanup completed successfully");
    Ok(())
}

// --- TTS Function ---

#[tauri::command]
pub async fn get_tts_audio(
    text: String,
    state: State<'_, AppState>,
    app_handle: tauri::AppHandle,
) -> Result<String, String> {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_substantial_user_communication() {
        // Test cases that should NOT be considered substantial user communication
        assert!(!is_substantial_user_communication(""));
        assert!(!is_substantial_user_communication("   "));
        assert!(!is_substantial_user_communication("Done"));
        assert!(!is_substantial_user_communication("Task completed"));
        assert!(!is_substantial_user_communication("Operation successful"));
        assert!(!is_substantial_user_communication(
            "File saved successfully"
        ));
        assert!(!is_substantial_user_communication("Command executed"));
        assert!(!is_substantial_user_communication("Finished"));
        assert!(!is_substantial_user_communication("Unable to complete"));
        assert!(!is_substantial_user_communication("Error occurred"));
        assert!(!is_substantial_user_communication("Not found"));
        assert!(!is_substantial_user_communication("Short message"));

        // Test cases that SHOULD be considered substantial user communication
        assert!(is_substantial_user_communication("I found several files that match your criteria. Here are the results: file1.txt, file2.txt, and file3.txt."));
        assert!(is_substantial_user_communication("The analysis shows that the system is performing well. CPU usage is at 45% and memory usage is at 60%."));
        assert!(is_substantial_user_communication("Here's what I discovered while searching through the codebase. The main function is located in the src directory."));
        assert!(is_substantial_user_communication("I've successfully completed the task.\n\nThe file has been created with the following content:\n- Line 1\n- Line 2\n- Line 3"));
        assert!(is_substantial_user_communication("Based on my analysis, there are several improvements that can be made to optimize performance."));
        assert!(is_substantial_user_communication("The search results indicate that there are multiple matches for your query across different files."));
        assert!(is_substantial_user_communication("I located the configuration file you requested. It contains important settings for the application."));
        assert!(is_substantial_user_communication(
            "After reviewing the logs, I found several error messages that need attention."
        ));

        // Test edge cases
        assert!(is_substantial_user_communication("This is a longer message that provides detailed information about the task that was completed and what the user should know about the results."));
        assert!(!is_substantial_user_communication("Task completed. Done."));
        assert!(is_substantial_user_communication("Task completed. Here are the detailed results of the operation including all the files that were processed."));
    }
}

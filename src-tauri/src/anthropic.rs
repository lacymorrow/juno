use std::collections::VecDeque;
use std::sync::Arc;
use tokio::sync::{Mutex, Semaphore};
use tracing::{debug, error, info, warn};
use uuid;
use scopeguard::guard;

use serde::{Deserialize, Serialize};
use serde_json::Value;
use tauri::{Emitter, Manager, State};

use crate::agent::core::AgentError;
use crate::agent::providers::factory::BrainFactory;
use crate::agent::structs::{AgentState, AgentStateUpdate};
use crate::agent::traits::MemoryManager;
use crate::constants::events;
use crate::state::AppState;

/// Agent execution queue system to prevent concurrent execution
#[derive(Debug)]
struct AgentExecutionQueue {
    /// Semaphore to ensure only one agent executes at a time
    execution_semaphore: Arc<Semaphore>,
    /// Queue of pending agent queries
    pending_queries: Arc<Mutex<VecDeque<QueuedQuery>>>,
    /// Currently executing query ID
    current_execution_id: Arc<Mutex<Option<String>>>,
}

#[derive(Debug, Clone)]
struct QueuedQuery {
    id: String,
    query: String,
    queued_at: std::time::Instant,
    app_handle: tauri::AppHandle,
}

impl AgentExecutionQueue {
    fn new() -> Self {
        Self {
            execution_semaphore: Arc::new(Semaphore::new(1)), // Only one agent at a time
            pending_queries: Arc::new(Mutex::new(VecDeque::new())),
            current_execution_id: Arc::new(Mutex::new(None)),
        }
    }

    /// Queue a new query for execution, cancelling any existing execution
    async fn queue_query(
        &self,
        query: String,
        app_handle: tauri::AppHandle,
        state: tauri::State<'_, AppState>,
    ) -> String {
        let query_id = uuid::Uuid::new_v4().to_string();
        let queued_query = QueuedQuery {
            id: query_id.clone(),
            query,
            queued_at: std::time::Instant::now(),
            app_handle,
        };

        // Cancel current execution if any
        self.cancel_current_execution(state).await;

        // Clear pending queue and add new query
        {
            let mut pending = self.pending_queries.lock().await;
            pending.clear(); // Only keep the latest query
            pending.push_back(queued_query);
            info!("Queued new agent query with ID: {}", query_id);
        }

        query_id
    }

    /// Cancel the currently executing agent
    async fn cancel_current_execution(&self, state: tauri::State<'_, AppState>) {
        let current_id = {
            let current = self.current_execution_id.lock().await;
            current.clone()
        };

        if let Some(execution_id) = current_id {
            info!("Cancelling current agent execution: {}", execution_id);
            // Signal cancellation through the existing state mechanism
            state.signal_cancel();
            info!("Signalled cancellation for existing agent execution");

            // Give existing agent a brief moment to clean up gracefully
            tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
        }
    }

    /// Execute the next queued query atomically
    async fn execute_next_query(&self, state: tauri::State<'_, AppState>) -> Option<QueuedQuery> {
        // Try to acquire execution semaphore (non-blocking check)
        if let Ok(permit) = self.execution_semaphore.try_acquire() {
            let next_query = {
                let mut pending = self.pending_queries.lock().await;
                pending.pop_front()
            };

            if let Some(query) = next_query.clone() {
                // Set current execution ID
                {
                    let mut current = self.current_execution_id.lock().await;
                    *current = Some(query.id.clone());
                }

                // Execute the query
                info!("Starting atomic agent execution for query ID: {}", query.id);

                // Execute the actual agent logic here
                let result =
                    execute_agent_internal(query.query.clone(), state, query.app_handle.clone(), query.id.clone())
                        .await;

                // Clear current execution
                {
                    let mut current = self.current_execution_id.lock().await;
                    *current = None;
                }

                // Release the semaphore
                drop(permit);

                // Handle execution result - ensure UI cleanup happens on failure
                match result {
                    Ok(()) => {
                        info!("Agent execution completed successfully for query ID: {}", query.id);
                    }
                    Err(e) => {
                        error!("Agent execution failed for query {}: {}", query.id, e);
                        // NOTE: UI cleanup is handled within execute_agent_internal for all scenarios
                        // No additional cleanup needed here to avoid race conditions
                    }
                }

                return Some(query);
            } else {
                // No queries to execute, release semaphore
                drop(permit);
            }
        }

        None
    }

    /// Check if execution is currently in progress
    async fn is_executing(&self) -> bool {
        let current = self.current_execution_id.lock().await;
        current.is_some()
    }
}

/// Global agent execution queue instance
static AGENT_EXECUTION_QUEUE: std::sync::OnceLock<AgentExecutionQueue> = std::sync::OnceLock::new();

/// Get or initialize the global agent execution queue
fn get_agent_execution_queue() -> &'static AgentExecutionQueue {
    AGENT_EXECUTION_QUEUE.get_or_init(|| AgentExecutionQueue::new())
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
}

// Keep this for payload structure, ensure Clone is derived
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct SubmitQueryResult {
    pub text: String,
    pub spoken_text: Option<String>, // Optional separate content for TTS
    pub audio_base64: Option<String>,
    pub agent_state: String,               // Send final state to frontend
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

/// Optimized JSX content detection using pattern matching
fn is_jsx_content(content: &str) -> bool {
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
        "StatusCard", "ColorShowcase", "VisualDemo", "className=", "jsx", "React"
    ];

    // Check for JSX patterns efficiently
    JSX_INDICATORS.iter().any(|&pattern| content.contains(pattern))
}

/// Check if a response from a specialist agent contains substantial user communication
/// This helps the orchestrator decide whether to generate its own TTS response
fn is_substantial_user_communication(content: &str) -> bool {
    let trimmed_content = content.trim();

    // Not substantial if empty or very short
    if trimmed_content.len() < 10 {
        return false;
    }

    // If it looks like a path, it's not communication (e.g., from file operations)
    if trimmed_content.contains('/') || trimmed_content.contains('\\') {
        if !trimmed_content.contains(' ') {
            return false;
        }
    }

    // Check for common non-communicative patterns
    let non_comm_patterns = [
        "Task completed successfully",
        "Operation successful",
        "Done.",
        "OK.",
        "completed",
        "finished",
        "error",
        "failed",
    ];

    for pattern in non_comm_patterns {
        if trimmed_content.to_lowercase() == pattern {
            return false;
        }
    }

    // Check if it's JSON - typically not direct user communication
    if (trimmed_content.starts_with('{') && trimmed_content.ends_with('}'))
        || (trimmed_content.starts_with('[') && trimmed_content.ends_with(']'))
    {
        return false;
    }

    // Looks like substantial communication
    true
}

// Main entry point for submitting a query to the agent system
#[tauri::command]
pub async fn submit_query(
    query: String,
    state: State<'_, AppState>,
    app_handle: tauri::AppHandle,
) -> Result<(), String> {
    info!("Received query submission: '{}'", query);

    let queue = get_agent_execution_queue();

    // Queue the query and get its execution ID
    let execution_id = queue.queue_query(query, app_handle.clone(), state.clone()).await;

    // Immediately emit a "thinking" status to the UI
    if let Err(e) = app_handle.emit(
        events::agent::STATE_CHANGED,
        AgentStateUpdate {
            state: AgentState::Thinking,
            message_id: Some(execution_id),
            ..Default::default()
        },
    ) {
        error!("Failed to emit initial thinking state: {}", e);
    }

    // Spawn a task to process the queue without blocking the main thread
    tokio::spawn(async move {
        queue.execute_next_query(state).await;
    });

    Ok(())
}


async fn execute_agent_internal(
    query: String,
    state: tauri::State<'_, AppState>,
    app_handle: tauri::AppHandle,
    execution_id: String,
) -> Result<(), String> {
    info!("Executing agent with query: '{}'", query);

    let app_handle_clone = app_handle.clone();
    let execution_id_clone = execution_id.clone();

    // Ensure cleanup happens even on panic
    let _guard = guard((), |_| {
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async move {
            let _agent_state = app_handle_clone.state::<AppState>().agent_state.lock().await;

            app_handle_clone
                .emit(
                    events::agent::STATE_CHANGED,
                    AgentStateUpdate {
                        state: AgentState::Idle,
                        ..Default::default()
                    },
                )
                .unwrap();
            app_handle_clone.emit(events::agent::PROCESSING_COMPLETE, ()).unwrap();
        });
    });

    // Reset cancellation flag for new execution
    state.reset_cancel();

    let memory_manager = state.memory_manager.clone();
    let tool_provider = state.tool_provider.clone();

    let mut runner = BrainFactory::create_agent_runtime(memory_manager, tool_provider, Some(app_handle.clone())).await?;

    let cancel_rx = state.get_cancel_receiver();
    let result = runner.run(query, cancel_rx).await;

    // Final state update
    let _agent_state = state.agent_state.lock().await;

    app_handle.emit(
        events::agent::STATE_CHANGED,
        AgentStateUpdate {
            state: AgentState::Idle,
            message_id: Some(execution_id.clone()),
            ..Default::default()
        },
    )?;

    if let Err(e) = result {
        error!("Agent execution failed: {}", e);
        app_handle.emit(
            events::agent::PROCESSING_ERROR,
            format!("Agent execution failed: {}", e),
        )?;
        return Err(e.to_string());
    }

    Ok(())
}

#[tauri::command]
pub async fn cleanup_browser(app_handle: tauri::AppHandle) -> Result<(), String> {
    let state = app_handle.state::<AppState>();
    let browser_manager = state.browser_manager.lock().await;
    browser_manager.cleanup().await;
    Ok(())
}

#[tauri::command]
pub async fn clear_conversation_history(state: State<'_, AppState>) -> Result<(), String> {
    let memory_manager = state.memory_manager.clone();
    let mut memory = memory_manager.lock().await;
    memory.clear_history();
    Ok(())
}

// --- Unit Tests ---
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_jsx_content() {
        assert!(is_jsx_content("<Button>Click me</Button>"));
        assert!(is_jsx_content("<div className='test'>...</div>"));
        assert!(is_jsx_content("Here is a <Card> for you."));
        assert!(!is_jsx_content("Just plain text."));
        assert!(!is_jsx_content("1 < 2 and 3 > 1"));
        assert!(!is_jsx_content(""));
    }

    #[test]
    fn test_is_substantial_user_communication() {
        assert!(is_substantial_user_communication(
            "I have finished processing the file and found 3 errors."
        ));
        assert!(is_substantial_user_communication(
            "Could you please clarify which file you want me to process?"
        ));

        assert!(!is_substantial_user_communication("Done."));
        assert!(!is_substantial_user_communication("Task completed successfully"));
        assert!(!is_substantial_user_communication("ok."));
        assert!(!is_substantial_user_communication(
            "{\"status\": \"complete\", \"files_processed\": 1}"
        ));
        assert!(!is_substantial_user_communication("/path/to/my/file.txt"));
        assert!(!is_substantial_user_communication(""));
        assert!(!is_substantial_user_communication("         "));
    }
}

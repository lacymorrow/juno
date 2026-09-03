use super::core::{AgentAction, AgentError, Message, ToolCall, ToolDefinition, ToolResult};
use crate::state::CancelReceiver;
use async_trait::async_trait;
use tauri::AppHandle;

/// Manages the agent's memory (conversation history).
#[async_trait]
pub trait MemoryManager: Send + Sync {
    /// Adds a message to the memory.
    async fn add_message(&mut self, message: Message) -> Result<(), AgentError>;

    /// Retrieves all messages from memory.
    async fn get_messages(&self) -> Result<Vec<Message>, AgentError>;

    /// Retrieves the last N messages.
    async fn get_last_n_messages(&self, n: usize) -> Result<Vec<Message>, AgentError>;

    /// Clears the agent's memory.
    async fn clear_memory(&mut self) -> Result<(), AgentError>;

    /// Removes orphaned tool calls that don't have corresponding tool results.
    /// This should be implemented by memory managers that track tool call state.
    /// Default implementation does nothing (for backward compatibility).
    async fn clean_orphaned_tool_calls(&mut self) -> Result<(), AgentError> {
        Ok(()) // Default no-op implementation
    }

    /// Removes orphaned tool results that don't have corresponding tool calls.
    /// This should be implemented by memory managers that track tool result consistency.
    /// Default implementation does nothing (for backward compatibility).
    async fn clean_orphaned_tool_results(&mut self) -> Result<usize, AgentError> {
        Ok(0) // Default no-op implementation returns 0 cleaned items
    }

    /// Sets the current execution ID to distinguish between different agent executions.
    /// This helps prevent cleaning up tool calls that belong to the current execution.
    /// Default implementation does nothing (for backward compatibility).
    async fn set_current_execution_id(&mut self, _execution_id: &str) -> Result<(), AgentError> {
        Ok(()) // Default no-op implementation
    }

    /// Removes orphaned tool calls only from previous executions, not from the current one.
    /// This allows safe cleanup without affecting tools currently in progress.
    /// Default implementation falls back to the regular clean_orphaned_tool_calls method.
    async fn clean_orphaned_tool_calls_from_previous_executions(
        &mut self,
    ) -> Result<(), AgentError> {
        self.clean_orphaned_tool_calls().await // Default implementation calls the regular method
    }

    // Potential future additions:
    // async fn summarize_memory(&self) -> Result<String, AgentError>;
    // async fn prune_memory(&mut self, max_tokens: usize) -> Result<(), AgentError>;
}

/// Provides and executes tools available to the agent.
#[async_trait]
pub trait ToolProvider: Send + Sync {
    /// Lists all tools currently available.
    async fn list_tools(&self) -> Result<Vec<ToolDefinition>, AgentError>;

    /// Executes a specific tool call.
    async fn execute_tool(&self, tool_call: ToolCall) -> Result<ToolResult, AgentError>;

    /// Executes multiple tool calls as a batch for improved performance.
    /// Default implementation falls back to sequential execution.
    async fn execute_batch_tools(
        &self,
        tool_calls: Vec<ToolCall>,
    ) -> Result<Vec<ToolResult>, AgentError> {
        let mut results = Vec::new();
        for tool_call in tool_calls {
            results.push(self.execute_tool(tool_call).await?);
        }
        Ok(results)
    }
}

/// Represents the agent's "brain" - responsible for deciding the next action.
#[async_trait]
pub trait AgentBrain: Send + Sync {
    /// Takes the current memory and available tools, returns the next action.
    async fn decide_next_action(
        &self,
        messages: &[Message],
        available_tools: &[ToolDefinition],
    ) -> Result<AgentAction, AgentError>;

    /// Check if this brain supports streaming
    fn supports_streaming(&self) -> bool {
        false // Default implementation - no streaming support
    }

    /// Streaming version of decide_next_action (default implementation delegates to regular method)
    ///
    /// `cancel_rx` is the run's cancellation channel — for session-tracked runs
    /// this is the merged session+global receiver built in `execute_agent_internal`
    /// (LAC-1432), so brains that spawn long-lived work (e.g. the Claude CLI
    /// subprocess) can observe a focused-session cancel that never touches the
    /// global channel (LAC-3697).
    async fn decide_next_action_streaming(
        &self,
        messages: &[Message],
        available_tools: &[ToolDefinition],
        _app_handle: Option<AppHandle>,
        _message_id: Option<String>,
        _cancel_rx: Option<CancelReceiver>,
    ) -> Result<AgentAction, AgentError> {
        // Default implementation ignores streaming parameters and calls regular method
        self.decide_next_action(messages, available_tools).await
    }
}

/// Extended version of AgentBrain that supports streaming responses
#[async_trait]
pub trait StreamingAgentBrain: AgentBrain {
    /// Check if streaming is enabled for this brain
    fn is_streaming_enabled(&self) -> bool;

    /// Enable or disable streaming
    fn set_streaming_enabled(&mut self, enabled: bool);
}

/// Defines the main runnable interface for an agent.
/// This ties together the brain, memory, and tools.
#[async_trait]
pub trait AgentRunnable: Send + Sync {
    /// Runs the agent loop with an initial prompt.
    async fn run(
        &mut self,
        initial_prompt: String,
        cancel_rx: CancelReceiver,
    ) -> Result<String, AgentError>;

    /// Executes a single step of the agent loop.
    async fn step(&mut self, cancel_rx: CancelReceiver) -> Result<AgentAction, AgentError>;

    // Maybe add methods for pausing, resuming, stopping?
    // async fn pause(&mut self) -> Result<(), AgentError>;
    // async fn resume(&mut self) -> Result<(), AgentError>;
    // async fn stop(&mut self) -> Result<(), AgentError>;
}

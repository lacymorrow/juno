use async_trait::async_trait;
use super::structs::{AgentAction, AgentError, Message, ToolCall, ToolDefinition, ToolResult};
use crate::state::CancelReceiver;

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
    async fn step(
        &mut self,
        cancel_rx: CancelReceiver,
    ) -> Result<AgentAction, AgentError>;

    // Maybe add methods for pausing, resuming, stopping?
    // async fn pause(&mut self) -> Result<(), AgentError>;
    // async fn resume(&mut self) -> Result<(), AgentError>;
    // async fn stop(&mut self) -> Result<(), AgentError>;
}

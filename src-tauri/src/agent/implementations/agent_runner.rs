use async_trait::async_trait;
use std::sync::Arc;
use tokio::sync::Mutex; // Using Mutex for mutable access to MemoryManager
use crate::state::CancelReceiver; // Import the type alias

use crate::agent::structs::{
    AgentAction, AgentError, AgentState, Message, Role, // Removed ToolCall, ToolResult
};
use crate::agent::traits::{
    AgentBrain, AgentRunnable, MemoryManager, ToolProvider
};

/// Default implementation of the AgentRunnable trait.
/// Orchestrates the agent's execution flow using the provided components.
pub struct DefaultAgentRunner<M, T>
where
    M: MemoryManager + Send + Sync,
    T: ToolProvider + Send + Sync,
{
    state: AgentState,
    memory: Arc<Mutex<M>>, // Use Mutex for mutable access across async tasks
    tool_provider: Arc<T>,
    brain: Arc<dyn AgentBrain + Send + Sync>, // Use trait object directly
    max_steps: u32,
    current_step: u32,
}

impl<M, T> DefaultAgentRunner<M, T>
where
    M: MemoryManager + Send + Sync + 'static,
    T: ToolProvider + Send + Sync + 'static,
{
    pub fn new(
        memory: M,
        tool_provider: T,
        brain: impl AgentBrain + Send + Sync + 'static,
        max_steps: u32
    ) -> Self {
        DefaultAgentRunner {
            state: AgentState::Idle,
            memory: Arc::new(Mutex::new(memory)),
            tool_provider: Arc::new(tool_provider),
            brain: Arc::new(brain),
            max_steps,
            current_step: 0,
        }
    }

    /// Creates a new DefaultAgentRunner with a boxed brain implementation
    pub fn with_boxed_brain(
        memory: M,
        tool_provider: T,
        brain: Box<dyn AgentBrain + Send + Sync>,
        max_steps: u32,
    ) -> Self {
        DefaultAgentRunner {
            state: AgentState::Idle,
            memory: Arc::new(Mutex::new(memory)),
            tool_provider: Arc::new(tool_provider),
            brain: Arc::from(brain), // Convert Box to Arc
            max_steps,
            current_step: 0,
        }
    }

    async fn transition_state(&mut self, new_state: AgentState) {
        log::debug!("Agent state transition: {:?} -> {:?}", self.state, new_state);
        self.state = new_state;
        // TODO: Emit state change events if needed (e.g., for UI updates)
    }
}

#[async_trait]
impl<M, T> AgentRunnable for DefaultAgentRunner<M, T>
where
    M: MemoryManager + Send + Sync + 'static,
    T: ToolProvider + Send + Sync + 'static,
{
    async fn run(
        &mut self,
        initial_prompt: String,
        cancel_rx: CancelReceiver, // Use watch receiver (no longer needs mut)
    ) -> Result<String, AgentError> {
        if self.state != AgentState::Idle {
            return Err(AgentError::StateError(
                "Agent must be in Idle state to start.".to_string(),
            ));
        }

        // Clone the receiver for the step function
        let step_cancel_rx = cancel_rx.clone();

        self.transition_state(AgentState::Thinking).await;
        self.current_step = 0;

        // Add initial user message to memory
        {
            let mut mem = self.memory.lock().await;
            mem.add_message(Message {
                role: Role::User,
                content: initial_prompt,
                tool_calls: None,
                tool_call_id: None,
                name: None,
            })
            .await?;
        }

        let mut final_response = String::new();

        loop {
            // --- Cancellation Check (Start of Loop) ---
            if *cancel_rx.borrow() {
                log::info!("Agent run cancelled.");
                self.transition_state(AgentState::Failed("Cancelled".to_string())).await;
                return Err(AgentError::Terminated);
            }

            if self.current_step >= self.max_steps {
                log::warn!("Reached maximum steps ({}), terminating agent loop", self.max_steps);
                self.transition_state(AgentState::Failed("Max steps reached".to_string()))
                    .await;
                return Err(AgentError::MaxStepsReached);
            }

            log::info!("Agent step {} of {}", self.current_step + 1, self.max_steps);

            // Execute one step of the agent loop, passing the cloned receiver
            let action = self.step(step_cancel_rx.clone()).await?;

            // Handle agent action
            match action {
                AgentAction::Finish(text) => {
                    log::info!("Agent finished with text response");
                    self.transition_state(AgentState::Finished).await;
                    final_response = text;
                    break; // Exit the loop successfully
                }
                 AgentAction::RespondToUser(text) => {
                    log::info!("Agent intermediate response: {}", text);
                    // Add the assistant's response to memory
                    {
                        let mut mem = self.memory.lock().await;
                        mem.add_message(Message {
                            role: Role::Assistant,
                            content: text.clone(),
                            tool_calls: None,
                            tool_call_id: None,
                            name: None,
                        })
                        .await?;
                    }
                     // Continue loop, keep thinking unless brain explicitly finishes
                     self.transition_state(AgentState::Thinking).await;
                }
                AgentAction::Error(e) => {
                    let error_message = e.to_string();
                    log::error!("Agent encountered error: {}", error_message);
                    self.transition_state(AgentState::Failed(error_message))
                        .await;
                    return Err(e); // Propagate the error
                }
                 AgentAction::Think | AgentAction::ExecuteTool(_) => {
                     // ExecuteTool is handled within step().
                     // Think means continue the loop.
                    log::debug!("AgentAction::Think or handled ExecuteTool, continuing loop.");
                    self.transition_state(AgentState::Thinking).await; // Ensure state is Thinking
                }
            }

            self.current_step += 1;
        }

        Ok(final_response)
    }

    // Modify step to accept the CancelReceiver
    async fn step(
        &mut self,
        cancel_rx: CancelReceiver, // Use watch receiver (no longer needs mut)
    ) -> Result<AgentAction, AgentError> {
        // --- Cancellation Check (Start of Step) ---
        if *cancel_rx.borrow() {
             log::debug!("Cancellation detected at start of step.");
            return Err(AgentError::Terminated);
        }

        self.transition_state(AgentState::Thinking).await;

        let messages = {
            let mem = self.memory.lock().await;
            mem.get_messages().await?
        };
        let tools = self.tool_provider.list_tools().await?;

        // --- Cancellation Check (Before Brain Action) ---
         if *cancel_rx.borrow() {
             log::debug!("Cancellation detected before brain action.");
             return Err(AgentError::Terminated);
         }

        let brain_action = self.brain.decide_next_action(&messages, &tools).await?;
        log::debug!("Brain decided action: {:?}", brain_action);

        match brain_action {
            AgentAction::ExecuteTool(tool_calls) => {
                if tool_calls.is_empty() {
                    log::warn!("ExecuteTool action received with empty tool call list. Switching to Think.");
                    return Ok(AgentAction::Think); // Return Think if no tools to call
                }

                self.transition_state(AgentState::Executing).await;
                log::info!("Executing {} tool call(s)", tool_calls.len());

                // Add assistant message indicating tool call(s)
                {
                    let mut mem = self.memory.lock().await;
                    // Clone the calls for the message
                    let message_tool_calls = tool_calls.clone();
                    mem.add_message(Message {
                        role: Role::Assistant,
                        content: "".to_string(), // Content might be empty or indicate thought process
                        tool_calls: Some(message_tool_calls),
                        tool_call_id: None,
                        name: None,
                    })
                    .await?;
                }

                // Execute tools sequentially for now
                // TODO: Consider parallel execution if tools are independent
                for tool_call in tool_calls {
                    // --- Cancellation Check (Before Tool Execution) ---
                     if *cancel_rx.borrow() {
                         log::debug!("Cancellation detected before tool execution: {}", tool_call.name);
                         return Err(AgentError::Terminated);
                     }

                    log::info!(
                        "Executing tool: {} with ID: {}",
                        tool_call.name,
                        tool_call.id
                    );
                    let tool_result = self.tool_provider.execute_tool(tool_call.clone()).await;

                    // --- Cancellation Check (After Tool Execution) ---
                    // Check even if tool execution failed, to ensure timely termination
                    if *cancel_rx.borrow() {
                         log::debug!("Cancellation detected after tool execution: {}", tool_call.name);
                         return Err(AgentError::Terminated);
                    }

                    // Add tool result message to memory immediately after execution
                    match tool_result {
                        Ok(result) => {
                            let mut mem = self.memory.lock().await;
                            log::debug!("Tool {} finished successfully.", tool_call.name);
                            mem.add_message(Message {
                                role: Role::Tool,
                                // Prefer JSON string if possible, fallback to debug representation
                                content: serde_json::to_string(&result.output)
                                    .unwrap_or_else(|e| {
                                        log::warn!("Failed to serialize tool output to JSON: {}", e);
                                        format!("{:?}", result.output) // Fallback
                                    }),
                                tool_calls: None,
                                tool_call_id: Some(result.call_id),
                                name: Some(tool_call.name.clone()), // Often tool name goes here
                            })
                            .await?;
                        }
                        Err(e) => {
                            // If a tool fails, add an error message to memory
                            // and let the brain decide the next step.
                            log::error!("Tool {} failed: {}", tool_call.name, e);
                            let mut mem = self.memory.lock().await;
                             // Store the error message as the tool's content
                            mem.add_message(Message {
                                role: Role::Tool,
                                content: format!("Error executing tool: {}", e),
                                tool_calls: None,
                                tool_call_id: Some(tool_call.id.clone()), // Use original call ID
                                name: Some(tool_call.name.clone()),
                            })
                            .await?;
                            // Let the LLM handle the tool error, continue thinking
                        }
                    }
                }
                // After executing all tools, transition back to Thinking for the next loop iteration
                self.transition_state(AgentState::Thinking).await;
                Ok(AgentAction::Think) // Indicate thinking should continue
            }
             AgentAction::RespondToUser(response) => {
                 self.transition_state(AgentState::Responding).await;
                 // The run loop will add this response to memory if needed.
                 Ok(AgentAction::RespondToUser(response))
             }
             AgentAction::Finish(response) => {
                 // This action will terminate the loop when returned to run()
                 Ok(AgentAction::Finish(response))
             }
             AgentAction::Error(e) => {
                 // Propagate the error up to the run loop
                 Ok(AgentAction::Error(e))
             }
             AgentAction::Think => {
                 // Brain explicitly requests thinking, keep state as Thinking
                  self.transition_state(AgentState::Thinking).await;
                 Ok(AgentAction::Think)
             }
        }
    }
}


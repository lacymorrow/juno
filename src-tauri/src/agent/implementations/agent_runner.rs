use async_trait::async_trait;
use std::sync::Arc;
use tokio::sync::Mutex; // Using Mutex for mutable access to MemoryManager
use crate::state::CancelReceiver; // Import the type alias

use crate::agent::structs::{
    AgentAction, AgentError, AgentState, Message, Role, ToolCall, ToolResult,
};
use crate::agent::traits::{
    AgentBrain, AgentRunnable, MemoryManager, ToolProvider
};

/// Default implementation of the AgentRunnable trait.
/// Orchestrates the agent's execution flow using the provided components.
pub struct DefaultAgentRunner<M, T, B>
where
    M: MemoryManager + Send + Sync,
    T: ToolProvider + Send + Sync,
    B: AgentBrain + Send + Sync,
{
    state: AgentState,
    memory: Arc<Mutex<M>>, // Use Mutex for mutable access across async tasks
    tool_provider: Arc<T>,
    brain: Arc<B>,
    max_steps: u32,
    current_step: u32,
}

impl<M, T, B> DefaultAgentRunner<M, T, B>
where
    M: MemoryManager + Send + Sync + 'static,
    T: ToolProvider + Send + Sync + 'static,
    B: AgentBrain + Send + Sync + 'static,
{
    pub fn new(memory: M, tool_provider: T, brain: B, max_steps: u32) -> Self {
        DefaultAgentRunner {
            state: AgentState::Idle,
            memory: Arc::new(Mutex::new(memory)),
            tool_provider: Arc::new(tool_provider),
            brain: Arc::new(brain),
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
impl<M, T, B> AgentRunnable for DefaultAgentRunner<M, T, B>
where
    M: MemoryManager + Send + Sync + 'static,
    T: ToolProvider + Send + Sync + 'static,
    B: AgentBrain + Send + Sync + 'static,
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

        loop {
            // --- Cancellation Check (Start of Loop) ---
            // Check if the cancel signal has been received
            if *cancel_rx.borrow() {
                log::info!("Agent run cancelled.");
                self.transition_state(AgentState::Failed("Cancelled".to_string())).await;
                return Err(AgentError::Terminated);
            }

            if self.current_step >= self.max_steps {
                self.transition_state(AgentState::Failed("Max steps reached".to_string()))
                    .await;
                return Err(AgentError::MaxStepsReached);
            }
            self.current_step += 1;
            log::info!("Agent Step {}/{}", self.current_step, self.max_steps);

            // Pass the cloned receiver to the step function
            let action = self.step(step_cancel_rx.clone()).await?;

            match action {
                AgentAction::Finish(final_message) => {
                    self.transition_state(AgentState::Finished).await;
                    // Add final assistant message to memory
                    {
                        let mut mem = self.memory.lock().await;
                        mem.add_message(Message {
                            role: Role::Assistant,
                            content: final_message.clone(),
                            tool_calls: None,
                            tool_call_id: None,
                            name: None,
                        })
                        .await?;
                    }
                    log::info!("Agent finished.");
                    return Ok(final_message);
                }
                AgentAction::Error(e) => {
                    let error_message = e.to_string();
                    self.transition_state(AgentState::Failed(error_message))
                        .await;
                    return Err(e);
                }
                AgentAction::Think => {
                    // Continue loop, state remains Thinking or shifts during step
                    continue;
                }
                AgentAction::RespondToUser(intermediate_response) => {
                    // Log or emit intermediate response, then continue thinking
                    log::info!("Agent intermediate response: {}", intermediate_response);
                    {
                        let mut mem = self.memory.lock().await;
                        mem.add_message(Message {
                            role: Role::Assistant,
                            content: intermediate_response,
                            tool_calls: None,
                            tool_call_id: None,
                            name: None,
                        })
                        .await?;
                    }
                    self.transition_state(AgentState::Thinking).await;
                    continue;
                }
                AgentAction::ExecuteTool(_) => {
                    // This case might indicate an issue if step() isn't fully processing tools.
                    // Assume step() handles execution and adds results, then returns Think or another Action.
                    log::warn!("AgentRunner received ExecuteTool action directly. Assuming step() handled it.");
                    self.transition_state(AgentState::Thinking).await;
                    continue;
                }
            }
        }
    }

    // Modify step to accept the CancelReceiver
    async fn step(
        &mut self,
        cancel_rx: CancelReceiver, // Use watch receiver (no longer needs mut)
    ) -> Result<AgentAction, AgentError> {
        // --- Cancellation Check (Start of Step) ---
        if *cancel_rx.borrow() {
            return Err(AgentError::Terminated);
        }

        self.transition_state(AgentState::Thinking).await;

        let messages = {
            let mem = self.memory.lock().await;
            mem.get_messages().await?
        };
        let tools = self.tool_provider.list_tools().await?;

        // --- Cancellation Check (Before Brain Call) ---
        if *cancel_rx.borrow() {
            return Err(AgentError::Terminated);
        }

        let brain_action = self.brain.decide_next_action(&messages, &tools).await?;
        log::debug!("Brain decided action: {:?}", brain_action);

        // --- Cancellation Check (After Brain Call) ---
        if *cancel_rx.borrow() {
            return Err(AgentError::Terminated);
        }

        match brain_action {
            AgentAction::ExecuteTool(tool_calls) => {
                if tool_calls.is_empty() {
                    log::warn!("ExecuteTool action received with empty tool call list. Switching to Think.");
                    return Ok(AgentAction::Think);
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
                        return Err(AgentError::Terminated);
                    }

                    // Add tool result message to memory immediately after execution
                    match tool_result {
                        Ok(result) => {
                            let mut mem = self.memory.lock().await;
                            log::debug!("Tool {} finished successfully.", tool_call.name);
                            mem.add_message(Message {
                                role: Role::Tool,
                                content: serde_json::to_string(&result.output).unwrap_or_else(
                                    |_| "[Serialization Error]".to_string(),
                                ),
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
                            mem.add_message(Message {
                                role: Role::Tool,
                                content: format!("Error executing tool: {}", e),
                                tool_calls: None,
                                tool_call_id: Some(tool_call.id.clone()), // Use original call ID
                                name: Some(tool_call.name.clone()),
                            })
                            .await?;
                            // Potentially return AgentAction::Error(e) here if we want to stop on any tool error
                            // For now, let the LLM handle the tool error
                        }
                    }
                }
                // After executing tools, transition back to Thinking for the next loop iteration
                Ok(AgentAction::Think)
            }
            // Other actions are returned directly
            other_action => Ok(other_action),
        }
    }
}

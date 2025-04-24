use async_trait::async_trait;
use std::sync::Arc;
use tokio::sync::Mutex; // Using Mutex for mutable access to MemoryManager

use crate::agent::structs::{
    AgentAction, AgentError, AgentState, Message, Role, ToolCall, ToolResult,
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
    M: MemoryManager + Send + Sync + 'static, // 'static needed for Arc<Mutex<>>
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
    async fn run(&mut self, initial_prompt: String) -> Result<String, AgentError> {
        if self.state != AgentState::Idle {
            return Err(AgentError::StateError(
                "Agent must be in Idle state to start.".to_string(),
            ));
        }

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
            }).await?;
        }

        // --- Begin Agent Loop ---
        let mut final_response = String::new();

        while self.state != AgentState::Finished && self.state != AgentState::Failed("".to_string()) {
            log::info!("Agent step {} of {}", self.current_step + 1, self.max_steps);

            if self.current_step >= self.max_steps {
                log::warn!("Reached maximum steps ({}), terminating agent loop", self.max_steps);
                return Err(AgentError::MaxStepsReached);
            }

            // Execute one step of the agent loop
            let action = self.step().await?;

            // Handle agent action
            match action {
                AgentAction::Finish(text) => {
                    log::info!("Agent finished with text response");
                    self.transition_state(AgentState::Finished).await;
                    final_response = text;
                    break;
                }
                AgentAction::RespondToUser(text) => {
                    log::info!("Agent responded to user");
                    // Add the assistant's response to memory
                    {
                        let mut mem = self.memory.lock().await;
                        mem.add_message(Message {
                            role: Role::Assistant,
                            content: text.clone(),
                            tool_calls: None,
                            tool_call_id: None,
                            name: None,
                        }).await?;
                    }
                    // For now, just continue the loop, but we might want to pause
                    // and wait for user input in an interactive scenario.
                }
                AgentAction::ExecuteTool(_) | AgentAction::Think => {
                    // Continue the loop - these actions are handled by step()
                    continue;
                }
                AgentAction::Error(err) => {
                    self.transition_state(AgentState::Failed(format!("{:?}", err))).await;
                    return Err(err);
                }
            }

            self.current_step += 1;
        }

        Ok(final_response)
    }

    async fn step(&mut self) -> Result<AgentAction, AgentError> {
        self.transition_state(AgentState::Thinking).await;

        let messages = {
            let mem = self.memory.lock().await;
            mem.get_messages().await?
        };
        let tools = self.tool_provider.list_tools().await?;

        let brain_action = self.brain.decide_next_action(&messages, &tools).await?;
        log::debug!("Brain decided action: {:?}", brain_action);

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
                    }).await?;
                }

                // Execute tools sequentially for now
                // TODO: Consider parallel execution if tools are independent
                for tool_call in tool_calls {
                    log::info!("Executing tool: {}", tool_call.name);

                    match self.tool_provider.execute_tool(tool_call.clone()).await {
                        Ok(result) => {
                            // Add tool result message to memory
                            log::info!("Tool execution successful: {}", result.output.to_string());
                            {
                                let mut mem = self.memory.lock().await;
                                mem.add_message(Message {
                                    role: Role::Tool,
                                    content: result.output.to_string(),
                                    tool_calls: None,
                                    tool_call_id: Some(result.call_id),
                                    name: Some(tool_call.name.clone()), // Include tool name
                                }).await?;
                            }
                        }
                        Err(e) => {
                            // Add error result message to memory
                            log::error!("Tool execution failed: {}", e);
                            let error_json = serde_json::json!({
                                "error": format!("Tool execution failed: {}", e)
                            });
                            {
                                let mut mem = self.memory.lock().await;
                                mem.add_message(Message {
                                    role: Role::Tool,
                                    content: error_json.to_string(),
                                    tool_calls: None,
                                    tool_call_id: Some(tool_call.id), // Include call ID for linking
                                    name: Some(tool_call.name), // Include tool name
                                }).await?;
                            }
                        }
                    }
                }

                // After tool execution, move back to thinking
                return Ok(AgentAction::Think);
            }
            // Pass through other actions
            other_action => Ok(other_action),
        }
    }
}

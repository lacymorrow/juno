use async_trait::async_trait;
use std::sync::Arc;
use tokio::sync::Mutex; // Using Mutex for mutable access to MemoryManager
use crate::state::CancelReceiver; // Import the type alias
use uuid;

use crate::agent::structs::{
    AgentAction, AgentError, AgentState, Message, Role, // Removed ToolCall, ToolResult
};
use crate::agent::traits::{
    AgentBrain,
    AgentRunnable,
    MemoryManager,
    ToolProvider,
};
use crate::agent::tool_logger; // Added for logging
use tauri::{AppHandle, Manager}; // Added Manager trait for accessing app state

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
    app_handle: Arc<AppHandle>, // Added AppHandle for logging
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
        max_steps: u32,
        app_handle: AppHandle, // Added AppHandle
    ) -> Self {
        log::info!("DefaultAgentRunner::new created. max_steps: {}, current_step: 0 (hardcoded init)", max_steps);
        DefaultAgentRunner {
            state: AgentState::Idle,
            memory: Arc::new(Mutex::new(memory)),
            tool_provider: Arc::new(tool_provider),
            brain: Arc::new(brain),
            max_steps,
            current_step: 0,
            app_handle: Arc::new(app_handle), // Store AppHandle
        }
    }

    /// Creates a new DefaultAgentRunner with a boxed brain implementation
    pub fn with_boxed_brain(
        memory: M,
        tool_provider: T,
        brain: Box<dyn AgentBrain + Send + Sync>,
        max_steps: u32,
        app_handle: AppHandle, // Added AppHandle
    ) -> Self {
        log::info!("DefaultAgentRunner::with_boxed_brain created. max_steps: {}, current_step: 0 (hardcoded init)", max_steps);
        DefaultAgentRunner {
            state: AgentState::Idle,
            memory: Arc::new(Mutex::new(memory)),
            tool_provider: Arc::new(tool_provider),
            brain: Arc::from(brain), // Convert Box to Arc
            max_steps,
            current_step: 0,
            app_handle: Arc::new(app_handle), // Store AppHandle
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
        log::info!(
            "DefaultAgentRunner::run called. Initial state - current_step: {}, max_steps: {}, agent_state: {:?}",
            self.current_step,
            self.max_steps,
            self.state
        );

        if self.state != AgentState::Idle {
            return Err(AgentError::StateError(
                "Agent must be in Idle state to start.".to_string(),
            ));
        }

        // Clone the receiver for the step function
        let step_cancel_rx = cancel_rx.clone();

        self.transition_state(AgentState::Thinking).await;
        self.current_step = 0;
        log::info!(
            "DefaultAgentRunner::run starting loop. current_step reset to: {}, max_steps: {}",
            self.current_step,
            self.max_steps
        );

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

            // Update AppState with current step progress
            let app_state = self.app_handle.state::<crate::state::AppState>();
            app_state.update_agent_current_step(self.current_step);

            // Execute one step of the agent loop, passing the cloned receiver
            let action = self.step(step_cancel_rx.clone()).await?;

            // Handle agent action
            match action {
                AgentAction::Finish(text) => {
                    log::info!("Agent finished with text response: \"{}\"", text);
                    self.transition_state(AgentState::Finished).await;
                    let final_response = text;
                    return Ok(final_response);
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

        // --- Log Thinking Step ---
        tool_logger::log_thinking(&self.app_handle, "Deciding next action based on current messages and available tools...");

        // --- Cancellation Check (Before Brain Action) ---
         if *cancel_rx.borrow() {
             log::debug!("Cancellation detected before brain action.");
             return Err(AgentError::Terminated);
         }

        // Check if brain supports streaming and use appropriate method
        let brain_action = if self.brain.supports_streaming() {
            // Brain supports streaming - generate a message ID and call streaming method
            let message_id = uuid::Uuid::new_v4().to_string();
            log::debug!("Using streaming brain with message ID: {}", message_id);
            self.brain.decide_next_action_streaming(
                &messages,
                &tools,
                Some((*self.app_handle).clone()),
                Some(message_id)
            ).await?
        } else {
            // Fall back to regular brain method
            log::debug!("Using non-streaming brain");
            self.brain.decide_next_action(&messages, &tools).await?
        };

        log::debug!("Brain decided action: {:?}", brain_action);

        match brain_action {
            AgentAction::ExecuteTool(tool_calls) => {
                if tool_calls.is_empty() {
                    log::warn!("ExecuteTool action received with empty tool call list. Switching to Think.");
                    return Ok(AgentAction::Think); // Return Think if no tools to call
                }

                self.transition_state(AgentState::Executing).await;
                log::info!("Executing {} tool call(s)", tool_calls.len());

                // Add assistant message indicating tool call(s) BEFORE starting execution
                // This ensures the conversation is in a consistent state even if interrupted
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

                // Pre-populate cancelled results for all tool calls to maintain conversation consistency
                // These will be overwritten with actual results as tools complete successfully
                let mut tool_results_cache = Vec::new();
                for tool_call in tool_calls.iter() {
                    tool_results_cache.push((tool_call.clone(), None)); // None means not executed yet
                }

                // Execute tools sequentially for now
                // TODO: Consider parallel execution if tools are independent
                for (index, tool_call) in tool_calls.iter().enumerate() {
                    // --- Cancellation Check (Before Tool Execution) ---
                    if *cancel_rx.borrow() {
                        log::info!("Cancellation detected before tool execution: {}", tool_call.name);

                        // Add cancelled tool results for all remaining tool calls (including this one)
                        let mut mem = self.memory.lock().await;
                        for (remaining_tool_call, cached_result) in tool_results_cache.iter().skip(index) {
                            if cached_result.is_none() {
                                log::debug!("Adding cancelled tool result for tool: {}", remaining_tool_call.name);
                                mem.add_message(Message {
                                    role: Role::Tool,
                                    content: "Tool execution was cancelled before completion.".to_string(),
                                    tool_calls: None,
                                    tool_call_id: Some(remaining_tool_call.id.clone()),
                                    name: Some(remaining_tool_call.name.clone()),
                                })
                                .await?;
                            }
                        }

                        return Err(AgentError::Terminated);
                    }

                    // Emit tool call request event
                    tool_logger::log_tool_call_request(
                        &self.app_handle,
                        &tool_call.name,
                        tool_call.input.clone(),
                        Some(format!("Executing tool: {}", tool_call.name))
                    );

                    log::info!(
                        "Executing tool: {} with ID: {}",
                        tool_call.name,
                        tool_call.id
                    );

                    // Execute the tool
                    let tool_result = self.tool_provider.execute_tool(tool_call.clone()).await;

                    // Cache the result for potential cleanup
                    tool_results_cache[index].1 = Some(tool_result.clone());

                    // --- Cancellation Check (After Tool Execution) ---
                    // Check even if tool execution failed, to ensure timely termination
                    if *cancel_rx.borrow() {
                        log::info!("Cancellation detected after tool execution: {}", tool_call.name);

                        // Add tool result for the current tool call (completed or failed)
                        let mut mem = self.memory.lock().await;
                        match tool_result {
                            Ok(result) => {
                                mem.add_message(Message {
                                    role: Role::Tool,
                                    content: serde_json::to_string(&result.output)
                                        .unwrap_or_else(|e| {
                                            log::warn!("Failed to serialize tool output to JSON: {}", e);
                                            format!("{:?}", result.output)
                                        }),
                                    tool_calls: None,
                                    tool_call_id: Some(result.call_id),
                                    name: Some(tool_call.name.clone()),
                                })
                                .await?;
                            }
                            Err(_) => {
                                mem.add_message(Message {
                                    role: Role::Tool,
                                    content: "Tool execution failed and was cancelled.".to_string(),
                                    tool_calls: None,
                                    tool_call_id: Some(tool_call.id.clone()),
                                    name: Some(tool_call.name.clone()),
                                })
                                .await?;
                            }
                        }

                        // Add cancelled tool results for any remaining tool calls
                        for (remaining_tool_call, cached_result) in tool_results_cache.iter().skip(index + 1) {
                            if cached_result.is_none() {
                                log::debug!("Adding cancelled tool result for remaining tool: {}", remaining_tool_call.name);
                                mem.add_message(Message {
                                    role: Role::Tool,
                                    content: "Tool execution was cancelled before completion.".to_string(),
                                    tool_calls: None,
                                    tool_call_id: Some(remaining_tool_call.id.clone()),
                                    name: Some(remaining_tool_call.name.clone()),
                                })
                                .await?;
                            }
                        }

                        return Err(AgentError::Terminated);
                    }

                    // Add tool result message to memory immediately after execution
                    let mut mem = self.memory.lock().await;
                    match tool_result {
                        Ok(result) => {
                            log::debug!("Tool {} finished successfully.", tool_call.name);

                            // Emit tool call result event
                            let screenshot_base64 = if tool_call.name == "capture_screenshot" || tool_call.name == "screenshot" {
                                result.output.as_str().map(|s| s.to_string())
                            } else {
                                None
                            };

                            tool_logger::log_tool_call_result(
                                &self.app_handle,
                                &tool_call.name,
                                result.output.clone(),
                                true, // success
                                Some(format!("Tool {} executed successfully", tool_call.name)),
                                screenshot_base64
                            );

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
                            log::warn!("Tool {} failed with error: {}", tool_call.name, e);

                            // Emit tool call result event for failure
                            tool_logger::log_tool_call_result(
                                &self.app_handle,
                                &tool_call.name,
                                serde_json::json!({"error": e.to_string()}),
                                false, // success = false
                                Some(format!("Tool {} failed: {}", tool_call.name, e)),
                                None
                            );

                            // Create error result message
                            let error_message = format!("Error: {}", e);
                            mem.add_message(Message {
                                role: Role::Tool,
                                content: error_message,
                                tool_calls: None,
                                tool_call_id: Some(tool_call.id.clone()),
                                name: Some(tool_call.name.clone()),
                            })
                            .await?;
                        }
                    }
                }

                // All tools completed successfully without cancellation
                log::info!("All {} tool call(s) completed successfully", tool_calls.len());

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


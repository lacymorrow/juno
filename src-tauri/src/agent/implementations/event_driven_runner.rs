//! Event-Driven Agent Runner
//! 
//! This is a pure event-driven implementation of the agent execution system
//! that replaces direct method calls with event emissions and subscriptions.
//! 
//! TARS Integration Phase 1.6: Event-Driven Agent Runner

use std::sync::Arc;
use async_trait::async_trait;
use tokio::sync::Mutex;
use tracing::{error, info, warn};

use crate::agent::events::{EventHandler, JunoAgentEvent, now};
use crate::agent::{AgentState, AgentStateMachine};
use crate::agent::core::{Message, Role};
use crate::agent::traits::{AgentBrain, MemoryManager};
use crate::agent::providers::factory::BrainFactory;
use crate::agent::EventMemoryManager;

/// Event-driven agent runner that orchestrates agent execution purely through events
pub struct EventDrivenAgentRunner {
    /// AI brain for generating responses
    brain: Arc<dyn AgentBrain + Send + Sync>,
    /// Memory manager for conversation context
    memory_manager: Arc<Mutex<EventMemoryManager>>,
    /// State machine for tracking agent state
    state_machine: Arc<AgentStateMachine>,
    /// App handle for accessing shared state
    app_handle: tauri::AppHandle,
    /// Maximum number of iterations per session
    max_iterations: u32,
}

impl EventDrivenAgentRunner {
    /// Create a new event-driven agent runner
    pub async fn new(
        memory_manager: EventMemoryManager,
        app_handle: tauri::AppHandle,
        max_iterations: u32,
    ) -> Result<Self, String> {
        // Create brain using factory
        let brain = BrainFactory::create_brain().await
            .map_err(|e| format!("Failed to create brain: {}", e))?;
        
        // Create state machine
        let state_machine = Arc::new(AgentStateMachine::new());
        
        Ok(Self {
            brain: Arc::from(brain),
            memory_manager: Arc::new(Mutex::new(memory_manager)),
            state_machine,
            app_handle,
            max_iterations,
        })
    }
    
    /// Start a new agent execution session
    async fn start_agent_run(&self, content: &str, session_id: &str) -> Result<Vec<JunoAgentEvent>, String> {
        info!("Starting event-driven agent run for session: {}", session_id);
        
        // Transition state machine to processing
        let state = AgentState::Processing {
            session_id: session_id.to_string(),
            current_step: 1,
            max_steps: self.max_iterations,
            started_at: now(),
        };
        
        if let Err(e) = self.state_machine.transition_to(state).await {
            error!("Failed to transition to processing state: {}", e);
            return Err(e);
        }
        
        // Add user message to memory
        let user_message = Message {
            role: Role::User,
            content: content.to_string(),
            tool_calls: None,
            tool_call_id: None,
            name: None,
        };
        
        {
            let mut memory = self.memory_manager.lock().await;
            if let Err(e) = memory.add_message(user_message).await {
                error!("Failed to add user message to memory: {}", e);
                return Err(e.to_string());
            }
        }
        
        // Generate initial response from brain
        match self.generate_brain_response(session_id).await {
            Ok(events) => Ok(events),
            Err(e) => {
                error!("Failed to generate brain response: {}", e);
                
                // Transition to error state
                let error_state = AgentState::Error {
                    session_id: session_id.to_string(),
                    error: e.clone(),
                    current_step: 1,
                    recoverable: true,
                };
                
                let _ = self.state_machine.transition_to(error_state).await;
                
                Ok(vec![
                    JunoAgentEvent::ErrorOccurred {
                        error_type: "brain_response_failed".to_string(),
                        message: e,
                        recoverable: true,
                        timestamp: now(),
                        context: Some(serde_json::json!({
                            "session_id": session_id
                        })),
                    }
                ])
            }
        }
    }
    
    /// Process a tool result and continue agent execution
    async fn process_tool_result(&self, tool_call_id: &str, result: &serde_json::Value, session_id: &str) -> Result<Vec<JunoAgentEvent>, String> {
        info!("Processing tool result for call ID: {} in session: {}", tool_call_id, session_id);
        
        // Add tool result to memory
        let tool_result_message = Message {
            role: Role::Tool,
            content: result.to_string(),
            tool_calls: None,
            tool_call_id: Some(tool_call_id.to_string()),
            name: None,
        };
        
        {
            let mut memory = self.memory_manager.lock().await;
            if let Err(e) = memory.add_message(tool_result_message).await {
                error!("Failed to add tool result to memory: {}", e);
                return Err(e.to_string());
            }
        }
        
        // Generate response after tool execution
        self.generate_brain_response(session_id).await
    }
    
    /// Generate response from brain and determine next action
    async fn generate_brain_response(&self, session_id: &str) -> Result<Vec<JunoAgentEvent>, String> {
        // Get messages from memory
        let messages = {
            let memory = self.memory_manager.lock().await;
            memory.get_messages().await
                .map_err(|e| format!("Failed to get messages: {}", e))?
        };
        
        // Generate response from brain using decide_next_action
        let action = self.brain.decide_next_action(&messages, &[]).await
            .map_err(|e| format!("Brain processing failed: {}", e))?;
        
        let mut events = Vec::new();
        
        // Handle the action from the brain
        match action {
            crate::agent::core::AgentAction::ExecuteTool(tool_calls) => {
                // Transition to waiting for tool state
                let waiting_state = AgentState::WaitingForTool {
                    session_id: session_id.to_string(),
                    tool_call_id: tool_calls[0].id.clone(), // Use first tool call ID
                    current_step: 1, // TODO: Track actual step
                    tool_name: tool_calls[0].name.clone(),
                };
                
                if let Err(e) = self.state_machine.transition_to(waiting_state).await {
                    warn!("Failed to transition to waiting for tool state: {}", e);
                }
                
                // Add assistant message with tool calls to memory
                let assistant_message = Message {
                    role: Role::Assistant,
                    content: "".to_string(), // Tool calls don't have content
                    tool_calls: Some(tool_calls.clone()),
                    tool_call_id: None,
                    name: None,
                };
                
                {
                    let mut memory = self.memory_manager.lock().await;
                    if let Err(e) = memory.add_message(assistant_message).await {
                        error!("Failed to add assistant message with tool calls: {}", e);
                    }
                }
                
                // Emit tool call events
                for tool_call in &tool_calls {
                    events.push(JunoAgentEvent::ToolCall {
                        tool_name: tool_call.name.clone(),
                        args: tool_call.input.clone(),
                        id: tool_call.id.clone(),
                        timestamp: now(),
                        session_id: Some(session_id.to_string()),
                    });
                }
            }
            
            crate::agent::core::AgentAction::Finish(content) => {
                // No tool calls - this is a final response
                // Transition to responding state
                let responding_state = AgentState::Responding {
                    session_id: session_id.to_string(),
                    current_step: 1, // TODO: Track actual step
                    partial_response: Some(content.clone()),
                };
                
                if let Err(e) = self.state_machine.transition_to(responding_state).await {
                    warn!("Failed to transition to responding state: {}", e);
                }
                
                // Add final assistant message to memory
                let assistant_message = Message {
                    role: Role::Assistant,
                    content: content.clone(),
                    tool_calls: None,
                    tool_call_id: None,
                    name: None,
                };
                
                {
                    let mut memory = self.memory_manager.lock().await;
                    if let Err(e) = memory.add_message(assistant_message).await {
                        error!("Failed to add final assistant message: {}", e);
                    }
                }
                
                // Transition to completed state
                let completed_state = AgentState::Completed {
                    session_id: session_id.to_string(),
                    final_step: 1, // TODO: Track actual step
                    elapsed_ms: 0, // TODO: Calculate actual elapsed time
                    response: content.clone(),
                };
                
                if let Err(e) = self.state_machine.transition_to(completed_state).await {
                    warn!("Failed to transition to completed state: {}", e);
                }
                
                // Emit assistant message and completion events
                events.push(JunoAgentEvent::AssistantMessage {
                    content: content.clone(),
                    timestamp: now(),
                    session_id: Some(session_id.to_string()),
                });
                
                events.push(JunoAgentEvent::AgentRunEnd {
                    session_id: session_id.to_string(),
                    status: "completed".to_string(),
                    iterations: 1, // TODO: Track actual iterations
                    elapsed_ms: 0, // TODO: Calculate actual elapsed time
                    timestamp: now(),
                });
            }
            
            crate::agent::core::AgentAction::RespondToUser(content) => {
                // Intermediate response - emit message and continue
                events.push(JunoAgentEvent::AssistantMessage {
                    content: content.clone(),
                    timestamp: now(),
                    session_id: Some(session_id.to_string()),
                });
                
                // Continue processing - this is not a final response
                info!("Received intermediate response from agent: {}", content);
            }
            
            crate::agent::core::AgentAction::Error(error) => {
                // Handle error from brain
                error!("Agent brain returned error: {:?}", error);
                
                // Transition to error state
                let error_state = AgentState::Error {
                    session_id: session_id.to_string(),
                    error: error.to_string(),
                    current_step: 1, // TODO: Track actual step
                    recoverable: true,
                };
                
                let _ = self.state_machine.transition_to(error_state).await;
                
                events.push(JunoAgentEvent::ErrorOccurred {
                    error_type: "agent_brain_error".to_string(),
                    message: error.to_string(),
                    recoverable: true,
                    timestamp: now(),
                    context: Some(serde_json::json!({
                        "session_id": session_id
                    })),
                });
            }
            
            crate::agent::core::AgentAction::Think => {
                // Agent needs to think more - continue processing
                info!("Agent is thinking, continuing processing...");
                // This might require additional loop handling in the future
            }
        }
        
        Ok(events)
    }
}

#[async_trait]
impl EventHandler for EventDrivenAgentRunner {
    async fn handle_event(&self, event: &JunoAgentEvent) -> Result<Vec<JunoAgentEvent>, String> {
        match event {
            JunoAgentEvent::AgentRunStart { session_id, user_query, .. } => {
                info!("Event-driven agent runner handling AgentRunStart for session: {}", session_id);
                self.start_agent_run(user_query, session_id).await
            }
            
            JunoAgentEvent::ToolResult { tool_call_id, result, .. } => {
                // We need to determine which session this belongs to
                // For now, get the current session from state machine
                let current_state = self.state_machine.get_state().await;
                if let Some(session_id) = current_state.session_id() {
                    info!("Event-driven agent runner handling ToolResult for session: {}", session_id);
                    self.process_tool_result(tool_call_id, result, session_id).await
                } else {
                    warn!("Received ToolResult but no active session found");
                    Ok(vec![])
                }
            }
            
            _ => {
                // This handler only processes AgentRunStart and ToolResult events
                Ok(vec![])
            }
        }
    }
    
    fn event_types(&self) -> Vec<&'static str> {
        vec!["agent_run_start", "tool_result"]
    }
    
    fn name(&self) -> &'static str {
        "EventDrivenAgentRunner"
    }
    
    fn priority(&self) -> u8 {
        80 // High priority, but after orchestrator
    }
}
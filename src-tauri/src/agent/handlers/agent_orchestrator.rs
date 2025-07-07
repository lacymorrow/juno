use std::sync::Arc;
use async_trait::async_trait;
use tracing::{error, info, warn, debug};

use crate::agent::events::{EventHandler, JunoAgentEvent, now};
use crate::agent::AgentState;
use crate::agent::core::AgentError;
use crate::agent::providers::factory::BrainFactory;
use crate::agent::implementations::memory_manager::AdvancedMemoryManager;
use crate::agent::implementations::tool_provider::LocalToolProvider;
use crate::agent::implementations::agent_runner::DefaultAgentRunner;
use crate::agent::traits::AgentRunnable;
use crate::state::AppState;

/// Orchestrates agent execution in response to AgentRunStart events
pub struct AgentOrchestrator {
    /// Reference to application state for accessing components
    app_state: Arc<AppState>,
    /// App handle for accessing Tauri functionality
    app_handle: tauri::AppHandle,
}

impl AgentOrchestrator {
    pub fn new(app_state: Arc<AppState>, app_handle: tauri::AppHandle) -> Self {
        Self { app_state, app_handle }
    }
    
    /// Execute the agent for a given session
    async fn execute_agent_session(&self, session_id: &str, max_iterations: u32, user_query: &str) -> Result<Vec<JunoAgentEvent>, String> {
        let mut events = Vec::new();
        
        // Update state machine to processing
        let state_machine = self.app_state.agent_state_machine.clone();
        {
            let state = AgentState::Processing {
                session_id: session_id.to_string(),
                current_step: 1,
                max_steps: max_iterations,
                started_at: now(),
            };
            
            let machine = state_machine.lock().await;
            if let Err(e) = machine.transition_to(state).await {
                error!("Failed to transition agent state to processing: {}", e);
                return Err(e);
            }
        }
        
        // User query is now passed directly from the event
        
        info!("Starting agent execution for session {} with query: {}", session_id, user_query);
        
        // Create agent components
        let (brain, tool_provider, memory_manager) = match self.create_agent_components().await {
            Ok(components) => components,
            Err(e) => {
                error!("Failed to create agent components: {}", e);
                
                events.push(JunoAgentEvent::ErrorOccurred {
                    error_type: "component_creation_failed".to_string(),
                    message: e.clone(),
                    recoverable: true,
                    timestamp: now(),
                    context: Some(serde_json::json!({
                        "session_id": session_id
                    })),
                });
                
                return Ok(events);
            }
        };
        
        // Create and run agent
        let mut agent_runner = DefaultAgentRunner::with_boxed_brain(
            memory_manager,
            tool_provider,
            brain,
            max_iterations,
            self.app_handle.clone(),
        );
        
        // Create cancellation receiver (for now, create a simple one)
        let (cancel_tx, cancel_rx) = tokio::sync::watch::channel(false);
        
        // Execute agent (this will generate its own events internally)
        let result = agent_runner.run(user_query.to_string(), cancel_rx).await;
        
        // Process result and generate completion events
        match result {
            Ok(response) => {
                info!("Agent execution completed successfully for session {}", session_id);
                
                // Transition state to completed
                {
                    let state = AgentState::Completed {
                        session_id: session_id.to_string(),
                        final_step: max_iterations, // TODO: Get actual step count
                        elapsed_ms: 0, // TODO: Calculate actual elapsed time
                        response: response.clone(),
                    };
                    
                    let machine = state_machine.lock().await;
                    if let Err(e) = machine.transition_to(state).await {
                        warn!("Failed to transition agent state to completed: {}", e);
                    }
                }
                
                events.push(JunoAgentEvent::AssistantMessage {
                    content: response,
                    timestamp: now(),
                    session_id: Some(session_id.to_string()),
                });
                
                events.push(JunoAgentEvent::AgentRunEnd {
                    session_id: session_id.to_string(),
                    status: "completed".to_string(),
                    iterations: max_iterations, // TODO: Get actual iteration count
                    elapsed_ms: 0, // TODO: Calculate actual elapsed time
                    timestamp: now(),
                });
            }
            Err(agent_error) => {
                error!("Agent execution failed for session {}: {:?}", session_id, agent_error);
                
                let (status, error_type, recoverable) = match agent_error {
                    AgentError::Terminated => ("cancelled".to_string(), "user_cancelled".to_string(), true),
                    AgentError::MaxStepsReached => ("max_steps_reached".to_string(), "max_steps_reached".to_string(), true),
                    AgentError::LlmError(_) => ("failed".to_string(), "llm_error".to_string(), true),
                    _ => ("failed".to_string(), "unknown_error".to_string(), true),
                };
                
                // Transition state to error
                {
                    let state = AgentState::Error {
                        session_id: session_id.to_string(),
                        error: agent_error.to_string(),
                        current_step: max_iterations, // TODO: Get actual step count
                        recoverable,
                    };
                    
                    let machine = state_machine.lock().await;
                    if let Err(e) = machine.transition_to(state).await {
                        warn!("Failed to transition agent state to error: {}", e);
                    }
                }
                
                events.push(JunoAgentEvent::ErrorOccurred {
                    error_type,
                    message: agent_error.to_string(),
                    recoverable,
                    timestamp: now(),
                    context: Some(serde_json::json!({
                        "session_id": session_id,
                        "agent_error": format!("{:?}", agent_error)
                    })),
                });
                
                events.push(JunoAgentEvent::AgentRunEnd {
                    session_id: session_id.to_string(),
                    status,
                    iterations: max_iterations, // TODO: Get actual iteration count
                    elapsed_ms: 0, // TODO: Calculate actual elapsed time
                    timestamp: now(),
                });
            }
        }
        
        Ok(events)
    }
    
    
    /// Create agent components (brain, tool provider, memory manager)
    async fn create_agent_components(&self) -> Result<(Box<dyn crate::agent::traits::AgentBrain>, LocalToolProvider, AdvancedMemoryManager), String> {
        // Create brain using the factory method
        let brain = BrainFactory::create_brain().await
            .map_err(|e| format!("Failed to create brain: {}", e))?;
        
        // Create tool provider and register all tools like in the main agent system
        let mut tool_provider = LocalToolProvider::new();
        
        // Register basic tools
        crate::agent::tools::basic_tools::register_basic_tools(&mut tool_provider).await;
        
        // Register computer use tools
        if let Err(e) = BrainFactory::register_computer_use_tools(&mut tool_provider, self.app_handle.clone()).await {
            warn!("Failed to register computer use tools: {}", e);
        }
        
        // Register desktop tools (pass dummy state - desktop tools don't actually use the state parameter)
        // Note: This is safe because desktop_tools.rs doesn't use the _state parameter
        warn!("Desktop tools registration in event system bypassed (tools don't use state parameter)");
        
        // Register browser tools if available
        for definition in crate::agent::tools::browser_tools::get_browser_tool_definitions() {
            let app_state = self.app_state.clone();
            let executor = move |input: serde_json::Value| {
                let app_state = app_state.clone();
                async move {
                    // Use the same browser tool executor from the main system
                    warn!("Browser tool execution not yet implemented in event-driven system: {}", input);
                    Ok(serde_json::json!({"error": "Browser tools not yet implemented in event system"}))
                }
            };
            tool_provider.register_async_tool(definition.clone(), executor).await;
        }
        
        // Use the shared memory manager from AppState (important for context continuity and pruning)
        let memory_manager_arc = self.app_state.get_memory_manager().await;
        let memory_manager = {
            let manager_guard = memory_manager_arc.lock().await;
            manager_guard.clone()
        };
        
        info!("Successfully created agent components with tools registered");
        Ok((brain, tool_provider, memory_manager))
    }
}

#[async_trait]
impl EventHandler for AgentOrchestrator {
    async fn handle_event(&self, event: &JunoAgentEvent) -> Result<Vec<JunoAgentEvent>, String> {
        match event {
            JunoAgentEvent::AgentRunStart { session_id, max_iterations, user_query, .. } => {
                info!("Agent orchestrator handling run start for session: {}", session_id);
                
                // Execute agent asynchronously and return events
                self.execute_agent_session(session_id, *max_iterations, user_query).await
            }
            _ => {
                // This handler only processes AgentRunStart events
                Ok(vec![])
            }
        }
    }
    
    fn event_types(&self) -> Vec<&'static str> {
        vec!["agent_run_start"]
    }
    
    fn name(&self) -> &'static str {
        "AgentOrchestrator"
    }
    
    fn priority(&self) -> u8 {
        90 // High priority, but after user input processing
    }
}
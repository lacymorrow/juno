// Orchestrator responsible for receiving user commands, planning,
// and delegating tasks to specialized agents.

use crate::agent::structs::{AgentError, Message, Role, AgentAction};
use crate::agent::traits::AgentBrain; // Still need brain for LLM calls
use crate::agent::traits::MemoryManager;
use super::basic_agent::BasicAgent;
use super::browser_agent::BrowserAgent;
use super::desktop_agent::DesktopAgent;
use std::sync::Arc;
use tokio::sync::Mutex;
use chrono;

// Placeholder Task/Output structs - Refine as needed
#[derive(Debug, Clone)]
pub struct Task {
    pub id: String,
    pub description: String,
    pub agent_type: AgentType,
    // Add tool call details if needed
    pub tool_name: Option<String>,
    pub tool_input: Option<serde_json::Value>,
}

#[derive(Debug, Clone)]
pub struct Output {
    pub result: String, // Simplified for now
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AgentType {
    Browser,
    Desktop,
    Basic,
    Orchestrator, // For tasks handled by the orchestrator itself (planning, finishing)
}

pub struct Orchestrator<M: MemoryManager + Send + Sync + 'static> {
    brain: Arc<dyn AgentBrain + Send + Sync>,
    memory: Arc<Mutex<M>>,
    basic_agent: Arc<BasicAgent>,
    browser_agent: Arc<BrowserAgent>,
    desktop_agent: Arc<DesktopAgent>,
    max_steps: u32, // Or manage steps differently?
}

impl<M: MemoryManager + Send + Sync + 'static> Orchestrator<M> {
    pub async fn new(
        brain: Arc<dyn AgentBrain + Send + Sync>,
        memory: M,
        max_steps: u32,
    ) -> Result<Self, AgentError> {
        let browser_agent = BrowserAgent::new().await?;
        let desktop_agent = DesktopAgent::new();
        let basic_agent = BasicAgent::new();

        Ok(Orchestrator {
            brain,
            memory: Arc::new(Mutex::new(memory)),
            basic_agent: Arc::new(basic_agent),
            browser_agent: Arc::new(browser_agent),
            desktop_agent: Arc::new(desktop_agent),
            max_steps,
        })
    }

    // Main entry point to process a user command
    pub async fn process_command(&self, command: String) -> Result<String, AgentError> {
        log::info!("Orchestrator received command: {}", command);

        // 1. Add command to memory
        {
            let mut mem = self.memory.lock().await;
            mem.add_message(Message {
                role: Role::User,
                content: command,
                tool_calls: None,
                tool_call_id: None,
                name: None,
            }).await?;
        }

        let mut current_step = 0;
        let mut final_response: Option<String> = None; // Initialize final_response

        loop {
            if current_step >= self.max_steps {
                return Err(AgentError::MaxStepsReached);
            }
            log::info!("Orchestrator step {}", current_step + 1);

            // 2. Plan next step(s) using the brain
            let planned_tasks = self.plan_next_tasks().await?;

            // 3. Execute tasks
            for task in planned_tasks {
                match task.agent_type {
                    AgentType::Orchestrator => {
                        // Task for orchestrator itself (e.g., finish)
                        log::info!("Orchestrator handling task: {}", task.description);
                        if task.description == "finish" {
                            // How to get final response? Assume it's in memory or last task output
                            let mem_lock = self.memory.lock().await;
                            let messages = mem_lock.get_messages().await?;
                            final_response = messages.last().and_then(|m| {
                                if m.role == Role::Assistant {
                                    Some(m.content.clone())
                                } else {
                                    None // Only consider last Assistant message as final response for now
                                }
                            });
                            drop(mem_lock); // Release lock
                            break; // Break inner loop
                        } else {
                            log::warn!("Unhandled Orchestrator task: {}", task.description);
                        }
                    }
                    _ => {
                        let output = self.delegate_task(task.clone()).await?;
                        // 4. Add result to memory (as Tool response? or Assistant message?)
                        // This needs refinement. How does brain expect tool results?
                        // Let's mimic the old Tool role for now.
                         {
                            let mut mem = self.memory.lock().await;
                            // Need tool_call_id from the planning step ideally
                            let tool_call_id = format!("orch_tool_{:?}_{}", task.agent_type, current_step); // Use Debug format for enum
                            mem.add_message(Message {
                                role: Role::Tool,
                                content: output.result, // Use the output result directly
                                tool_calls: None,
                                tool_call_id: Some(tool_call_id), // Placeholder ID
                                name: task.tool_name, // Pass tool name if available
                            }).await?;
                        }
                    }
                }
            }

            if final_response.is_some() {
                break; // Break outer loop
            }
            current_step += 1;
        }

        // Use the initialized outer final_response
        Ok(final_response.unwrap_or_else(|| "Orchestrator finished without explicit response.".to_string()))
    }

    // Ask the brain to plan the next steps based on memory
    async fn plan_next_tasks(&self) -> Result<Vec<Task>, AgentError> {
        let messages = {
            let mem = self.memory.lock().await;
            mem.get_messages().await?
        };

        // TODO: How to represent available agents/tools to the brain?
        // For now, we don't pass explicit tools, assuming brain infers from prompt/history.
        // Or we pass a generic "delegate" tool description?
        let action = self.brain.decide_next_action(&messages, &[]).await?; // Pass empty tools for now

        match action {
            AgentAction::Finish(response) => {
                // Add final assistant response to memory
                 {
                    let mut mem = self.memory.lock().await;
                    mem.add_message(Message {
                        role: Role::Assistant,
                        content: response.clone(),
                        tool_calls: None,
                        tool_call_id: None,
                        name: None,
                    }).await?;
                }
                Ok(vec![Task {
                    id: format!("task_{}", chrono::Utc::now().timestamp_millis()),
                    description: "finish".to_string(),
                    agent_type: AgentType::Orchestrator,
                    tool_name: None,
                    tool_input: None,
                }])
            }
             AgentAction::RespondToUser(text) => {
                 // If brain just wants to respond, treat it like Finish for now
                 // Or have a specific Orchestrator task?
                 log::warn!("Brain responded directly, treating as Finish: {}", text);
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
                 Ok(vec![Task {
                    id: format!("task_{}", chrono::Utc::now().timestamp_millis()),
                    description: "finish".to_string(),
                    agent_type: AgentType::Orchestrator,
                    tool_name: None,
                    tool_input: None,
                }])
             }
            AgentAction::ExecuteTool(tool_calls) => {
                // Convert LLM tool calls into our Agent Tasks by iterating and cloning
                let tasks: Vec<Task> = tool_calls
                    .iter() // Use iter() to borrow instead of into_iter() to move
                    .map(|tc| {
                        // TODO: How to map LLM tool name (e.g., "browser_navigate")
                        // to our AgentType (Browser) and Task description?
                        // This requires a mapping layer or smarter prompting.
                        // Simple mapping for now based on assumed naming convention:
                        let (agent_type, description) = if tc.name.starts_with("browser_") {
                            (AgentType::Browser, tc.name.clone())
                        } else if tc.name.starts_with("desktop_") {
                            (AgentType::Desktop, tc.name.clone())
                        } else {
                            (AgentType::Basic, tc.name.clone())
                        };
                        Task {
                            id: tc.id.clone(), // Clone necessary fields
                            description,
                            agent_type,
                            tool_name: Some(tc.name.clone()),
                            tool_input: Some(tc.input.clone()),
                        }
                    })
                    .collect();
                 // Add the assistant message containing the original tool calls to memory
                 {
                    let mut mem = self.memory.lock().await;
                    mem.add_message(Message {
                        role: Role::Assistant,
                        content: "".to_string(), // No text content when calling tools
                        tool_calls: Some(tool_calls), // Now OK, tool_calls was only borrowed
                        tool_call_id: None,
                        name: None,
                    }).await?;
                }
                Ok(tasks)
            }
            AgentAction::Think => {
                // Brain wants to think more, maybe loop in plan_next_tasks?
                // For now, just ask again in the next step.
                log::debug!("Brain returned Think action, will retry planning in next step.");
                Ok(vec![]) // No tasks to execute this step
            }
            AgentAction::Error(e) => Err(e),
        }
    }

    // Delegate a task to the appropriate specialized agent
    async fn delegate_task(&self, task: Task) -> Result<Output, AgentError> {
        match task.agent_type {
            AgentType::Browser => self.browser_agent.handle_task(task).await,
            AgentType::Desktop => self.desktop_agent.handle_task(task).await,
            AgentType::Basic => self.basic_agent.handle_task(task).await,
            AgentType::Orchestrator => Err(AgentError::InvalidAction("Cannot delegate Orchestrator task".to_string())),
        }
    }
}

use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

use crate::agent::core::AgentError;
use super::base_agent::{SpecializedAgent, AgentType, Task, TaskResult, AgentStatus};

/// Registry for all specialized agents in the system
pub struct AgentRegistry {
    agents: RwLock<HashMap<AgentType, Arc<dyn SpecializedAgent>>>,
}

impl AgentRegistry {
    /// Create a new agent registry
    pub fn new() -> Self {
        Self {
            agents: RwLock::new(HashMap::new()),
        }
    }

    /// Register a specialized agent
    pub async fn register_agent(&self, agent: Arc<dyn SpecializedAgent>) -> Result<(), AgentError> {
        let agent_type = agent.agent_type();
        let mut agents = self.agents.write().await;

        if agents.contains_key(&agent_type) {
            return Err(AgentError::Other(format!(
                "Agent of type {:?} is already registered",
                agent_type
            )));
        }

        agents.insert(agent_type, agent);
        Ok(())
    }

    /// Get an agent by type
    pub async fn get_agent(&self, agent_type: &AgentType) -> Option<Arc<dyn SpecializedAgent>> {
        let agents = self.agents.read().await;
        agents.get(agent_type).cloned()
    }

    /// Get all registered agents
    pub async fn get_all_agents(&self) -> Vec<Arc<dyn SpecializedAgent>> {
        let agents = self.agents.read().await;
        agents.values().cloned().collect()
    }

    /// Find the best agent for a given task based on capabilities and availability
    pub async fn find_best_agent_for_task(&self, task: &Task) -> Option<Arc<dyn SpecializedAgent>> {
        let agents = self.agents.read().await;

        // First, check if a specific agent type is requested
        if let Some(agent) = agents.get(&task.agent_type) {
            if agent.can_handle_task(task).await && agent.is_available().await {
                return Some(agent.clone());
            }
        }

        // If no specific agent or the specific agent can't handle it,
        // find the best available agent
        let mut best_agent: Option<Arc<dyn SpecializedAgent>> = None;
        let mut best_confidence = 0.0;

        for agent in agents.values() {
            if !agent.can_handle_task(task).await || !agent.is_available().await {
                continue;
            }

            // Calculate average confidence for all tools in the task
            let mut total_confidence = 0.0;
            let mut tool_count = 0;

            for tool_call in &task.tool_calls {
                let confidence = agent.get_confidence_for_tool(&tool_call.name);
                total_confidence += confidence;
                tool_count += 1;
            }

            if tool_count > 0 {
                let avg_confidence = total_confidence / tool_count as f32;
                if avg_confidence > best_confidence {
                    best_confidence = avg_confidence;
                    best_agent = Some(agent.clone());
                }
            }
        }

        best_agent
    }

    /// Get status of all agents
    pub async fn get_all_agent_status(&self) -> Vec<AgentStatus> {
        let agents = self.agents.read().await;
        let mut statuses = Vec::new();

        for agent in agents.values() {
            statuses.push(agent.get_status().await);
        }

        statuses
    }

    /// Initialize all registered agents
    pub async fn initialize_all(&self) -> Result<(), AgentError> {
        let agents = self.agents.read().await;

        for _agent in agents.values() {
            // Note: We can't call mutable methods on Arc<dyn SpecializedAgent>
            // This will need to be addressed in the actual agent implementations
            // by using interior mutability (Mutex/RwLock) for mutable state
        }

        Ok(())
    }

    /// Shutdown all registered agents
    pub async fn shutdown_all(&self) -> Result<(), AgentError> {
        let agents = self.agents.read().await;

        for _agent in agents.values() {
            // Same note as above - will need interior mutability
        }

        Ok(())
    }
}

/// Factory for creating and managing specialized agents
pub struct AgentFactory {
    registry: Arc<AgentRegistry>,
}

impl AgentFactory {
    /// Create a new agent factory
    pub fn new() -> Self {
        Self {
            registry: Arc::new(AgentRegistry::new()),
        }
    }

    /// Get the agent registry
    pub fn get_registry(&self) -> Arc<AgentRegistry> {
        self.registry.clone()
    }

    /// Initialize the factory with default agents
    /// This will be implemented as agents are created
    pub async fn initialize_default_agents(&self) -> Result<(), AgentError> {
        // TODO: Create and register default agents when they're implemented
        //
        // let browser_agent = Arc::new(BrowserAgent::new()?);
        // self.registry.register_agent(browser_agent).await?;
        //
        // let desktop_agent = Arc::new(DesktopAgent::new()?);
        // self.registry.register_agent(desktop_agent).await?;
        //
        // let system_agent = Arc::new(SystemAgent::new()?);
        // self.registry.register_agent(system_agent).await?;

        Ok(())
    }

    /// Execute a task using the best available agent
    pub async fn execute_task(&self, task: Task) -> Result<TaskResult, AgentError> {
        let agent = self.registry.find_best_agent_for_task(&task).await
            .ok_or_else(|| AgentError::Other(format!(
                "No suitable agent found for task: {}", task.description
            )))?;

        agent.handle_task(task).await
    }

    /// Get status of all agents
    pub async fn get_system_status(&self) -> Vec<AgentStatus> {
        self.registry.get_all_agent_status().await
    }
}

impl Default for AgentFactory {
    fn default() -> Self {
        Self::new()
    }
}

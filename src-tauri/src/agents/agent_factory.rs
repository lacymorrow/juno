use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

use super::base_agent::{AgentStatus, AgentType, SpecializedAgent, Task, TaskResult};
use crate::agent::core::AgentError;

/// Registry for all specialized agents in the system
pub struct AgentRegistry {
    agents: RwLock<HashMap<AgentType, Arc<dyn SpecializedAgent>>>,
}

#[allow(clippy::new_without_default)]
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
    app_handle: Option<tauri::AppHandle>,
}

impl AgentFactory {
    /// Create a new agent factory
    pub fn new() -> Self {
        Self {
            registry: Arc::new(AgentRegistry::new()),
            app_handle: None,
        }
    }

    /// Create a new agent factory with app handle
    pub fn with_app_handle(app_handle: tauri::AppHandle) -> Self {
        Self {
            registry: Arc::new(AgentRegistry::new()),
            app_handle: Some(app_handle),
        }
    }

    /// Set the app handle for creating agents that need it
    pub fn set_app_handle(&mut self, app_handle: tauri::AppHandle) {
        self.app_handle = Some(app_handle);
    }

    /// Get the agent registry
    pub fn get_registry(&self) -> Arc<AgentRegistry> {
        self.registry.clone()
    }

    /// Initialize the factory with default agents
    pub async fn initialize_default_agents(&self) -> Result<(), AgentError> {
        // Create and register default agents
        // Browser agent needs app_handle
        if let Some(ref app_handle) = self.app_handle {
            let browser_agent =
                Arc::new(super::browser_agent::BrowserAgent::new(app_handle.clone())?);
            self.registry.register_agent(browser_agent).await?;
        } else {
            tracing::warn!("Cannot initialize BrowserAgent: no app_handle provided");
        }

        // Desktop agent needs app_handle
        if let Some(ref app_handle) = self.app_handle {
            let desktop_agent =
                Arc::new(super::desktop_agent::DesktopAgent::new(app_handle.clone())?);
            self.registry.register_agent(desktop_agent).await?;
        } else {
            tracing::warn!("Cannot initialize DesktopAgent: no app_handle provided");
        }

        // System agent needs app_handle
        if let Some(ref app_handle) = self.app_handle {
            let system_agent = Arc::new(super::system_agent::SystemAgent::new(app_handle.clone())?);
            self.registry.register_agent(system_agent).await?;
        } else {
            tracing::warn!("Cannot initialize SystemAgent: no app_handle provided");
        }

        tracing::info!("Initialized all available specialized agents");
        Ok(())
    }

    /// Execute a task using the best available agent
    pub async fn execute_task(&self, task: Task) -> Result<TaskResult, AgentError> {
        let agent = self
            .registry
            .find_best_agent_for_task(&task)
            .await
            .ok_or_else(|| {
                AgentError::Other(format!(
                    "No suitable agent found for task: {}",
                    task.description
                ))
            })?;

        agent.handle_task(task).await
    }

    /// Get status of all agents
    pub async fn get_system_status(&self) -> Vec<AgentStatus> {
        self.registry.get_all_agent_status().await
    }

    /// Create an orchestrator with this factory's registry
    pub fn create_orchestrator(&self) -> super::orchestrator::Orchestrator {
        super::orchestrator::Orchestrator::new(self.registry.clone())
    }

    /// Create an orchestrator with custom configuration
    pub fn create_orchestrator_with_config(
        &self,
        config: super::orchestrator::OrchestratorConfig,
    ) -> super::orchestrator::Orchestrator {
        super::orchestrator::Orchestrator::with_config(self.registry.clone(), config)
    }
}

impl Default for AgentFactory {
    fn default() -> Self {
        Self::new()
    }
}

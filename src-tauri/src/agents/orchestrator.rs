use async_trait::async_trait;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};
use uuid::Uuid;
use tokio::sync::RwLock;

use crate::agent::core::{AgentError, Message, Role};
use super::base_agent::{
    SpecializedAgent, AgentType, Task, TaskResult, AgentCapability, AgentStatus, TaskPriority
};
use super::agent_factory::AgentRegistry;

/// Configuration for the orchestrator
#[derive(Debug, Clone)]
pub struct OrchestratorConfig {
    pub max_parallel_tasks: usize,
    pub task_timeout: Duration,
    pub enable_task_splitting: bool,
    pub enable_fallback_agents: bool,
    pub min_confidence_threshold: f32,
}

impl Default for OrchestratorConfig {
    fn default() -> Self {
        Self {
            max_parallel_tasks: 5,
            task_timeout: Duration::from_secs(300), // 5 minutes
            enable_task_splitting: true,
            enable_fallback_agents: true,
            min_confidence_threshold: 0.3,
        }
    }
}

/// The main orchestrator that coordinates specialized agents
pub struct Orchestrator {
    registry: Arc<AgentRegistry>,
    config: OrchestratorConfig,
    status: RwLock<OrchestratorStatus>,
    task_history: RwLock<Vec<TaskResult>>,
    active_tasks: RwLock<HashMap<String, Arc<Task>>>,
}

#[derive(Debug, Clone)]
pub struct OrchestratorStatus {
    pub is_available: bool,
    pub current_tasks: usize,
    pub total_tasks_delegated: usize,
    pub successful_delegations: usize,
    pub total_execution_time: Duration,
}

impl Orchestrator {
    /// Create a new orchestrator with the given agent registry
    pub fn new(registry: Arc<AgentRegistry>) -> Self {
        Self {
            registry,
            config: OrchestratorConfig::default(),
            status: RwLock::new(OrchestratorStatus {
                is_available: true,
                current_tasks: 0,
                total_tasks_delegated: 0,
                successful_delegations: 0,
                total_execution_time: Duration::new(0, 0),
            }),
            task_history: RwLock::new(Vec::new()),
            active_tasks: RwLock::new(HashMap::new()),
        }
    }

    /// Create a new orchestrator with custom configuration
    pub fn with_config(registry: Arc<AgentRegistry>, config: OrchestratorConfig) -> Self {
        Self {
            registry,
            config,
            status: RwLock::new(OrchestratorStatus {
                is_available: true,
                current_tasks: 0,
                total_tasks_delegated: 0,
                successful_delegations: 0,
                total_execution_time: Duration::new(0, 0),
            }),
            task_history: RwLock::new(Vec::new()),
            active_tasks: RwLock::new(HashMap::new()),
        }
    }

    /// Process a user command and coordinate agent execution
    pub async fn process_command(&self, user_input: String) -> Result<String, AgentError> {
        // For now, create a simple task from the user input
        // In a full implementation, this would use an LLM to analyze and plan
        let task = Task {
            id: Uuid::new_v4().to_string(),
            description: user_input.clone(),
            tool_calls: vec![], // Would be populated by LLM analysis
            agent_type: AgentType::Desktop, // Default, would be determined by analysis
            priority: TaskPriority::Normal,
            dependencies: vec![],
            timeout: Some(self.config.task_timeout),
            metadata: serde_json::json!({
                "created_at": chrono::Utc::now().to_rfc3339(),
                "user_input": user_input
            }),
        };

        match self.delegate_task(task).await {
            Ok(result) => {
                if result.success {
                    Ok(format!("Task completed successfully: {}",
                        result.output.as_str().unwrap_or("No output")))
                } else {
                    Ok(format!("Task failed: {}",
                        result.error.unwrap_or("Unknown error".to_string())))
                }
            }
            Err(e) => Err(e),
        }
    }

    /// Plan tasks from a conversation history (would use LLM in full implementation)
    pub async fn plan_tasks(&self, messages: &[Message]) -> Result<Vec<Task>, AgentError> {
        // This is a simplified implementation
        // In a full version, this would use an LLM to analyze messages and create tasks
        let mut tasks = Vec::new();

        for (i, message) in messages.iter().enumerate() {
            if message.role == Role::User {
                let task = Task {
                    id: Uuid::new_v4().to_string(),
                    description: message.content.clone(),
                    tool_calls: message.tool_calls.clone().unwrap_or_default(),
                    agent_type: self.determine_agent_type(&message.content).await,
                    priority: TaskPriority::Normal,
                    dependencies: if i > 0 { vec![format!("task_{}", i - 1)] } else { vec![] },
                    timeout: Some(self.config.task_timeout),
                    metadata: serde_json::json!({
                        "message_index": i,
                        "role": message.role
                    }),
                };
                tasks.push(task);
            }
        }

        Ok(tasks)
    }

    /// Delegate a task to the best available agent
    pub async fn delegate_task(&self, task: Task) -> Result<TaskResult, AgentError> {
        let start_time = Instant::now();

        // Update orchestrator status
        {
            let mut status = self.status.write().await;
            status.current_tasks += 1;
            status.total_tasks_delegated += 1;
            status.is_available = status.current_tasks < self.config.max_parallel_tasks;
        }

        // Store active task
        {
            let mut active_tasks = self.active_tasks.write().await;
            active_tasks.insert(task.id.clone(), Arc::new(task.clone()));
        }

        let result = match self.registry.find_best_agent_for_task(&task).await {
            Some(agent) => {
                // Check agent confidence if enabled
                let confidence = agent.get_confidence_for_tool(
                    &task.tool_calls.first()
                        .map(|tc| tc.name.clone())
                        .unwrap_or_default()
                );

                if confidence < self.config.min_confidence_threshold && self.config.enable_fallback_agents {
                    // Try to find a fallback agent
                    if let Some(fallback_agent) = self.find_fallback_agent(&task).await {
                        fallback_agent.handle_task(task.clone()).await
                    } else {
                        Err(AgentError::Other(format!(
                            "No suitable agent found for task: {} (confidence too low: {})",
                            task.description, confidence
                        )))
                    }
                } else {
                    agent.handle_task(task.clone()).await
                }
            }
            None => Err(AgentError::Other(format!(
                "No suitable agent found for task: {}", task.description
            )))
        };

        let execution_time = start_time.elapsed();

        // Update orchestrator status
        {
            let mut status = self.status.write().await;
            status.current_tasks -= 1;
            if result.is_ok() {
                status.successful_delegations += 1;
            }
            status.total_execution_time += execution_time;
            status.is_available = true;
        }

        // Remove from active tasks and add to history
        {
            let mut active_tasks = self.active_tasks.write().await;
            active_tasks.remove(&task.id);
        }

        if let Ok(ref task_result) = result {
            let mut history = self.task_history.write().await;
            history.push(task_result.clone());
            // Keep only last 100 results to prevent memory bloat
            if history.len() > 100 {
                history.remove(0);
            }
        }

        result
    }

    /// Find a fallback agent when the primary agent has low confidence
    async fn find_fallback_agent(&self, task: &Task) -> Option<Arc<dyn SpecializedAgent>> {
        let agents = self.registry.get_all_agents().await;

        for agent in agents {
            if agent.agent_type() != task.agent_type &&
               agent.can_handle_task(task).await &&
               agent.is_available().await {
                return Some(agent);
            }
        }

        None
    }

    /// Determine the best agent type for a given task description
    async fn determine_agent_type(&self, description: &str) -> AgentType {
        let description_lower = description.to_lowercase();

        // Simple keyword-based classification
        // In a full implementation, this would use more sophisticated analysis
        if description_lower.contains("browser") ||
           description_lower.contains("web") ||
           description_lower.contains("navigate") {
            AgentType::Browser
        } else if description_lower.contains("file") ||
                  description_lower.contains("command") ||
                  description_lower.contains("system") {
            AgentType::System
        } else {
            AgentType::Desktop // Default fallback
        }
    }

    /// Get the orchestrator's current status
    pub async fn get_orchestrator_status(&self) -> OrchestratorStatus {
        self.status.read().await.clone()
    }

    /// Get the agent registry
    pub fn get_registry(&self) -> Arc<AgentRegistry> {
        self.registry.clone()
    }

    /// Get task execution history
    pub async fn get_task_history(&self) -> Vec<TaskResult> {
        self.task_history.read().await.clone()
    }

    /// Get currently active tasks
    pub async fn get_active_tasks(&self) -> Vec<Arc<Task>> {
        let active_tasks = self.active_tasks.read().await;
        active_tasks.values().cloned().collect()
    }

    /// Merge results from multiple tasks into a coherent response
    pub fn merge_results(&self, results: Vec<TaskResult>) -> String {
        if results.is_empty() {
            return "No results to merge".to_string();
        }

        let successful_results: Vec<&TaskResult> = results.iter()
            .filter(|r| r.success)
            .collect();

        if successful_results.is_empty() {
            return format!("All {} tasks failed", results.len());
        }

        let mut response = format!("Completed {} out of {} tasks successfully:\n\n",
            successful_results.len(), results.len());

        for (i, result) in successful_results.iter().enumerate() {
            response.push_str(&format!("Task {}: {}\n",
                i + 1,
                result.output.as_str().unwrap_or("No output")
            ));
        }

        if successful_results.len() < results.len() {
            response.push_str(&format!("\n{} tasks failed.",
                results.len() - successful_results.len()));
        }

        response
    }

    /// Execute multiple tasks in parallel with dependency management
    pub async fn execute_parallel_tasks(&self, tasks: Vec<Task>) -> Result<Vec<TaskResult>, AgentError> {
        // Simple parallel execution for now
        // In a full implementation, this would handle dependencies properly
        let mut results = Vec::new();

        for task in tasks {
            let result = self.delegate_task(task).await?;
            results.push(result);
        }

        Ok(results)
    }
}

#[async_trait]
impl SpecializedAgent for Orchestrator {
    fn agent_type(&self) -> AgentType {
        AgentType::Orchestrator
    }

    fn get_capabilities(&self) -> Vec<AgentCapability> {
        vec![
            AgentCapability {
                name: "Task Coordination".to_string(),
                description: "Coordinate and delegate tasks to specialized agents".to_string(),
                tool_patterns: vec!["orchestrate".to_string(), "coordinate".to_string(), "delegate".to_string()],
                confidence: 1.0,
            },
            AgentCapability {
                name: "Multi-Agent Management".to_string(),
                description: "Manage multiple specialized agents and their capabilities".to_string(),
                tool_patterns: vec!["multi".to_string(), "agents".to_string(), "manage".to_string()],
                confidence: 1.0,
            },
            AgentCapability {
                name: "Task Planning".to_string(),
                description: "Analyze complex requests and break them into manageable tasks".to_string(),
                tool_patterns: vec!["plan".to_string(), "analyze".to_string(), "break down".to_string()],
                confidence: 0.9,
            },
        ]
    }

    async fn can_handle_task(&self, _task: &Task) -> bool {
        // The orchestrator can handle any task by delegating to specialized agents
        true
    }

    async fn handle_task(&self, task: Task) -> Result<TaskResult, AgentError> {
        // The orchestrator handles tasks by delegating them
        self.delegate_task(task).await
    }

    async fn get_status(&self) -> AgentStatus {
        let status = self.status.read().await;
        let success_rate = if status.total_tasks_delegated > 0 {
            status.successful_delegations as f32 / status.total_tasks_delegated as f32
        } else {
            0.0
        };

        let average_execution_time = if status.total_tasks_delegated > 0 {
            status.total_execution_time / status.total_tasks_delegated as u32
        } else {
            Duration::new(0, 0)
        };

        AgentStatus {
            agent_type: self.agent_type(),
            is_available: status.is_available,
            current_tasks: status.current_tasks,
            total_completed: status.total_tasks_delegated,
            success_rate,
            average_execution_time,
            capabilities: self.get_capabilities(),
        }
    }

    async fn is_available(&self) -> bool {
        let status = self.status.read().await;
        status.is_available
    }
}

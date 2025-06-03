use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::sync::Mutex;

use crate::agents::{
    AgentFactory, AgentStatus, Orchestrator, OrchestratorConfig, Task, TaskResult
};
use crate::state::AppState;

/// Global orchestrator instance
static ORCHESTRATOR: once_cell::sync::OnceCell<Arc<Mutex<Orchestrator>>> = once_cell::sync::OnceCell::new();

/// Initialize the orchestrator system
pub async fn init_orchestrator_with_app_handle(app_handle: tauri::AppHandle) -> Result<(), String> {
    let factory = AgentFactory::with_app_handle(app_handle);

    // Initialize default agents
    factory.initialize_default_agents().await
        .map_err(|e| format!("Failed to initialize agents: {}", e))?;

    // Create orchestrator
    let orchestrator = factory.create_orchestrator();

    // Store globally
    ORCHESTRATOR.set(Arc::new(Mutex::new(orchestrator)))
        .map_err(|_| "Failed to initialize orchestrator - already initialized")?;

    tracing::info!("Multi-agent orchestrator system initialized successfully");
    Ok(())
}

/// Initialize the orchestrator system (without app handle - deprecated)
pub async fn init_orchestrator() -> Result<(), String> {
    let factory = AgentFactory::new();

    // Initialize default agents (will skip desktop agent without app_handle)
    factory.initialize_default_agents().await
        .map_err(|e| format!("Failed to initialize agents: {}", e))?;

    // Create orchestrator
    let orchestrator = factory.create_orchestrator();

    // Store globally
    ORCHESTRATOR.set(Arc::new(Mutex::new(orchestrator)))
        .map_err(|_| "Failed to initialize orchestrator - already initialized")?;

    tracing::info!("Multi-agent orchestrator system initialized successfully (without app handle)");
    Ok(())
}

/// Get the global orchestrator instance
async fn get_orchestrator() -> Result<Arc<Mutex<Orchestrator>>, String> {
    ORCHESTRATOR.get()
        .ok_or_else(|| "Orchestrator not initialized".to_string())
        .map(|o| o.clone())
}

/// Configuration structure for Tauri
#[derive(Debug, Serialize, Deserialize)]
pub struct OrchestratorConfigDTO {
    pub max_parallel_tasks: usize,
    pub task_timeout_seconds: u64,
    pub enable_task_splitting: bool,
    pub enable_fallback_agents: bool,
    pub min_confidence_threshold: f32,
}

impl From<OrchestratorConfigDTO> for OrchestratorConfig {
    fn from(dto: OrchestratorConfigDTO) -> Self {
        Self {
            max_parallel_tasks: dto.max_parallel_tasks,
            task_timeout: std::time::Duration::from_secs(dto.task_timeout_seconds),
            enable_task_splitting: dto.enable_task_splitting,
            enable_fallback_agents: dto.enable_fallback_agents,
            min_confidence_threshold: dto.min_confidence_threshold,
        }
    }
}

/// Status report for the orchestrator system
#[derive(Debug, Serialize, Deserialize)]
pub struct OrchestratorStatusReport {
    pub orchestrator_available: bool,
    pub current_tasks: usize,
    pub total_tasks_delegated: usize,
    pub success_rate: f32,
    pub agent_statuses: Vec<AgentStatus>,
    pub active_task_count: usize,
}

/// Submit a query to be processed by the orchestrator
#[tauri::command]
pub async fn submit_orchestrated_query(
    query: String,
    use_orchestrator: bool,
    state: tauri::State<'_, AppState>,
    app_handle: tauri::AppHandle,
) -> Result<String, String> {
    if !use_orchestrator {
        // Fall back to the existing single-agent system
        crate::anthropic::submit_query(query.clone(), state, app_handle).await
            .map_err(|e| e)?;
        return Ok(format!("Query processed: {}", query));
    }

    // Register escape key shortcut for orchestrator execution
    crate::register_escape_key_shortcut(&app_handle);

    let orchestrator = get_orchestrator().await?;
    let orchestrator_guard = orchestrator.lock().await;

    let result = orchestrator_guard.process_command(query).await
        .map_err(|e| format!("Orchestrator error: {}", e));

    // Unregister escape key shortcut when orchestrator finishes
    crate::unregister_escape_key_shortcut(&app_handle);

    result
}

/// Get the status of the orchestrator and all agents
#[tauri::command]
pub async fn get_orchestrator_status() -> Result<OrchestratorStatusReport, String> {
    let orchestrator = get_orchestrator().await?;
    let orchestrator_guard = orchestrator.lock().await;

    let orch_status = orchestrator_guard.get_orchestrator_status().await;
    let agent_statuses = orchestrator_guard.get_registry().get_all_agent_status().await;
    let active_tasks = orchestrator_guard.get_active_tasks().await;

    Ok(OrchestratorStatusReport {
        orchestrator_available: orch_status.is_available,
        current_tasks: orch_status.current_tasks,
        total_tasks_delegated: orch_status.total_tasks_delegated,
        success_rate: if orch_status.total_tasks_delegated > 0 {
            orch_status.successful_delegations as f32 / orch_status.total_tasks_delegated as f32
        } else {
            0.0
        },
        agent_statuses,
        active_task_count: active_tasks.len(),
    })
}

/// Configure the orchestrator settings
#[tauri::command]
pub async fn configure_orchestrator(config: OrchestratorConfigDTO) -> Result<(), String> {
    // For now, we'll recreate the orchestrator with new config
    // In a production system, this would be more sophisticated
    let factory = AgentFactory::new();
    factory.initialize_default_agents().await
        .map_err(|e| format!("Failed to initialize agents: {}", e))?;

    let orchestrator_config: OrchestratorConfig = config.into();
    let orchestrator = factory.create_orchestrator_with_config(orchestrator_config);

    // Replace the global orchestrator
    if let Some(orch_cell) = ORCHESTRATOR.get() {
        let mut orch_guard = orch_cell.lock().await;
        *orch_guard = orchestrator;
    }

    Ok(())
}

/// Get the task execution history
#[tauri::command]
pub async fn get_task_history() -> Result<Vec<TaskResult>, String> {
    let orchestrator = get_orchestrator().await?;
    let orchestrator_guard = orchestrator.lock().await;

    Ok(orchestrator_guard.get_task_history().await)
}

/// Get currently active tasks
#[tauri::command]
pub async fn get_active_tasks() -> Result<Vec<Task>, String> {
    let orchestrator = get_orchestrator().await?;
    let orchestrator_guard = orchestrator.lock().await;

    let active_tasks = orchestrator_guard.get_active_tasks().await;
    Ok(active_tasks.into_iter().map(|t| (*t).clone()).collect())
}

/// Get all agent capabilities
#[tauri::command]
pub async fn get_agent_capabilities() -> Result<std::collections::HashMap<String, Vec<crate::agents::AgentCapability>>, String> {
    let orchestrator = get_orchestrator().await?;
    let orchestrator_guard = orchestrator.lock().await;

    let agent_statuses = orchestrator_guard.get_registry().get_all_agent_status().await;
    let mut capabilities_map = std::collections::HashMap::new();

    for status in agent_statuses {
        capabilities_map.insert(
            format!("{:?}", status.agent_type),
            status.capabilities
        );
    }

    Ok(capabilities_map)
}

/// Initialize the orchestrator on app startup
pub async fn initialize_orchestrator_system() -> Result<(), String> {
    init_orchestrator().await
}

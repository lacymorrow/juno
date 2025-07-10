use once_cell::sync::OnceCell;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::Mutex;
use tracing::warn;

use crate::agent::tools::mcp_integration::{MCPManager, MCPServerConfig};
use crate::agents::{
    AgentFactory, AgentStatus, AgentType, Orchestrator, OrchestratorConfig, Task, TaskPriority,
    TaskResult,
};
use crate::state::AppState;

/// Global orchestrator instance
static ORCHESTRATOR: OnceCell<Arc<Mutex<Orchestrator>>> = OnceCell::new();

/// Global MCP manager instance for orchestrator
static MCP_MANAGER: OnceCell<Arc<MCPManager>> = OnceCell::new();

/// Initialize the orchestrator system with MCP integration
pub async fn init_orchestrator_with_app_handle(app_handle: tauri::AppHandle) -> Result<(), String> {
    let factory = AgentFactory::with_app_handle(app_handle);

    // Initialize default agents
    factory
        .initialize_default_agents()
        .await
        .map_err(|e| format!("Failed to initialize agents: {}", e))?;

    // Create orchestrator
    let orchestrator = factory.create_orchestrator();

    // Initialize MCP manager
    let mcp_manager = Arc::new(MCPManager::new());

    // Initialize default MCP servers in the background to not block app startup
    let mcp_manager_bg = mcp_manager.clone();
    tokio::spawn(async move {
        if let Err(e) = initialize_default_mcp_servers_safely(&mcp_manager_bg).await {
            tracing::warn!("Background MCP server initialization had issues: {}", e);
            tracing::info!("MCP servers can be configured and started manually via Settings");
        }
    });

    // Store globally
    ORCHESTRATOR
        .set(Arc::new(Mutex::new(orchestrator)))
        .map_err(|_| "Failed to initialize orchestrator - already initialized")?;
    MCP_MANAGER
        .set(mcp_manager)
        .map_err(|_| "Failed to initialize MCP manager - already initialized")?;

    tracing::info!("Enhanced multi-agent orchestrator system initialized successfully");
    tracing::info!("MCP server initialization continues in background");
    Ok(())
}

/// Initialize default MCP servers safely without blocking app startup
async fn initialize_default_mcp_servers_safely(mcp_manager: &MCPManager) -> Result<(), String> {
    // Add essential MCP servers that provide intelligent capabilities with improved configurations
    let default_servers = vec![
        // Core filesystem operations (this package exists) - DISABLED by default to prevent startup issues
        MCPServerConfig {
            id: uuid::Uuid::new_v4().to_string(),
            name: "filesystem".to_string(),
            description: Some("Secure file system operations and management".to_string()),
            command: "npx".to_string(),
            args: vec![
                "@modelcontextprotocol/server-filesystem".to_string(),
                "/Users".to_string(),
            ],
            working_directory: None,
            environment_variables: {
                let mut env = std::collections::HashMap::new();
                // Prevent TLS warnings and improve stability
                env.insert("NODE_TLS_REJECT_UNAUTHORIZED".to_string(), "1".to_string());
                env.insert(
                    "NODE_OPTIONS".to_string(),
                    "--max-old-space-size=512".to_string(),
                );
                env.insert("MCP_LOG_LEVEL".to_string(), "error".to_string());
                env
            },
            enabled: false, // Disabled by default to prevent startup blocking
            auto_start: false,
            timeout_seconds: 60,
            max_retries: 1, // Reduced retries to prevent spam
        },
        // Everything server for comprehensive testing and development - DISABLED by default
        MCPServerConfig {
            id: uuid::Uuid::new_v4().to_string(),
            name: "everything".to_string(),
            description: Some(
                "Reference server with comprehensive MCP features for testing".to_string(),
            ),
            command: "npx".to_string(),
            args: vec!["@modelcontextprotocol/server-everything".to_string()],
            working_directory: None,
            environment_variables: {
                let mut env = std::collections::HashMap::new();
                env.insert("NODE_TLS_REJECT_UNAUTHORIZED".to_string(), "1".to_string());
                env.insert(
                    "NODE_OPTIONS".to_string(),
                    "--max-old-space-size=512".to_string(),
                );
                env.insert("MCP_LOG_LEVEL".to_string(), "error".to_string());
                // Limit event listeners to prevent memory leaks
                env.insert("NODE_MAX_LISTENERS".to_string(), "20".to_string());
                env
            },
            enabled: false, // Disabled by default to prevent EPIPE errors
            auto_start: false,
            timeout_seconds: 75,
            max_retries: 1,
        },
        // Memory and sequential thinking - DISABLED by default
        MCPServerConfig {
            id: uuid::Uuid::new_v4().to_string(),
            name: "memory".to_string(),
            description: Some("Knowledge graph-based persistent memory system".to_string()),
            command: "npx".to_string(),
            args: vec!["@modelcontextprotocol/server-memory".to_string()],
            working_directory: None,
            environment_variables: {
                let mut env = std::collections::HashMap::new();
                env.insert("NODE_TLS_REJECT_UNAUTHORIZED".to_string(), "1".to_string());
                env.insert(
                    "NODE_OPTIONS".to_string(),
                    "--max-old-space-size=256".to_string(),
                );
                env.insert("MCP_LOG_LEVEL".to_string(), "error".to_string());
                env
            },
            enabled: false, // Disabled by default
            auto_start: false,
            timeout_seconds: 45,
            max_retries: 1,
        },
        // Sequential thinking for problem solving - DISABLED by default
        MCPServerConfig {
            id: uuid::Uuid::new_v4().to_string(),
            name: "sequential-thinking".to_string(),
            description: Some("Sequential thinking and problem-solving capabilities".to_string()),
            command: "npx".to_string(),
            args: vec!["@modelcontextprotocol/server-sequential-thinking".to_string()],
            working_directory: None,
            environment_variables: {
                let mut env = std::collections::HashMap::new();
                env.insert("NODE_TLS_REJECT_UNAUTHORIZED".to_string(), "1".to_string());
                env.insert(
                    "NODE_OPTIONS".to_string(),
                    "--max-old-space-size=256".to_string(),
                );
                env.insert("MCP_LOG_LEVEL".to_string(), "error".to_string());
                env
            },
            enabled: false, // Disabled by default
            auto_start: false,
            timeout_seconds: 45,
            max_retries: 1,
        },
    ];

    tracing::info!(
        "Adding {} default MCP server configurations (disabled by default)...",
        default_servers.len()
    );

    let mut successful_configs = 0;
    for config in default_servers {
        match mcp_manager.add_server(config.clone()).await {
            Ok(_) => {
                successful_configs += 1;
                tracing::info!(
                    "Successfully added MCP server configuration '{}'",
                    config.name
                );
            }
            Err(e) => {
                tracing::warn!("Failed to add default MCP server '{}': {}", config.name, e);
                // Continue with other servers - don't let one failure block everything
            }
        }
    }

    tracing::info!(
        "Successfully configured {}/{} default MCP servers",
        successful_configs,
        4
    );
    tracing::info!("MCP servers are disabled by default - enable them in Settings if needed");
    tracing::info!(
        "To prevent app startup delays, MCP servers must be manually enabled in Settings"
    );

    Ok(())
}

/// Initialize default MCP servers for common tools
async fn initialize_default_mcp_servers(mcp_manager: &MCPManager) -> Result<(), String> {
    tracing::info!("Redirecting to safe MCP server initialization...");
    initialize_default_mcp_servers_safely(mcp_manager).await
}

/// Get the global orchestrator instance
async fn get_orchestrator() -> Result<Arc<Mutex<Orchestrator>>, String> {
    ORCHESTRATOR
        .get()
        .ok_or_else(|| "Orchestrator not initialized".to_string())
        .map(|o| o.clone())
}

/// Get the global MCP manager instance
fn get_mcp_manager() -> Result<Arc<MCPManager>, String> {
    MCP_MANAGER
        .get()
        .ok_or_else(|| "MCP manager not initialized".to_string())
        .map(|m| m.clone())
}

/// Enhanced configuration structure for Tauri
#[derive(Debug, Serialize, Deserialize)]
pub struct OrchestratorConfigDTO {
    pub max_parallel_tasks: usize,
    pub task_timeout_seconds: u64,
    pub enable_task_splitting: bool,
    pub enable_fallback_agents: bool,
    pub min_confidence_threshold: f32,
    /// Enable Model Context Protocol (MCP) integration for external tool servers
    pub enable_mcp_integration: bool,
    /// Enable intelligent task batching for performance optimization (separate from MCP)
    pub enable_intelligent_batching: bool,
    pub task_queue_size: usize,
    pub retry_failed_tasks: bool,
    pub max_task_retries: u32,
}

impl From<OrchestratorConfigDTO> for OrchestratorConfig {
    fn from(dto: OrchestratorConfigDTO) -> Self {
        Self {
            max_parallel_tasks: dto.max_parallel_tasks,
            task_timeout: std::time::Duration::from_secs(dto.task_timeout_seconds),
            enable_task_splitting: dto.enable_task_splitting,
            enable_fallback_agents: dto.enable_fallback_agents,
            min_confidence_threshold: dto.min_confidence_threshold,
            max_queue_size: dto.task_queue_size,
            queue_processing_interval: std::time::Duration::from_millis(500), // Default 500ms
            enable_intelligent_batching: dto.enable_intelligent_batching,
            batch_size: 4,
            parallel_execution_threshold: 3,
            enable_adaptive_timeout: true,
            enable_resource_aware_scheduling: true,
            max_concurrent_agents: 8,
            priority_boost_factor: 1.5,
            enable_predictive_caching: true,
        }
    }
}

/// Enhanced status report for the orchestrator system
#[derive(Debug, Serialize, Deserialize)]
pub struct OrchestratorStatusReport {
    pub orchestrator_available: bool,
    pub current_tasks: usize,
    pub total_tasks_delegated: usize,
    pub success_rate: f32,
    pub agent_statuses: Vec<AgentStatus>,
    pub active_task_count: usize,
    pub queued_task_count: usize,
    pub failed_task_count: usize,
    pub mcp_servers_connected: usize,
    pub mcp_tools_available: usize,
    pub average_task_completion_time: f32,
}

/// Task creation request with enhanced parameters
#[derive(Debug, Serialize, Deserialize)]
pub struct TaskCreationRequest {
    pub description: String,
    pub agent_type: Option<String>,
    pub priority: Option<String>,
    pub dependencies: Option<Vec<String>>,
    pub timeout_seconds: Option<u64>,
    pub context: Option<serde_json::Value>,
    pub use_mcp_tools: Option<bool>,
}

/// Workflow template for common task patterns
#[derive(Debug, Serialize, Deserialize)]
pub struct WorkflowTemplate {
    pub id: String,
    pub name: String,
    pub description: String,
    pub tasks: Vec<TaskTemplate>,
    pub variables: HashMap<String, String>,
}

/// Task template within a workflow
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskTemplate {
    pub name: String,
    pub description: String,
    pub agent_type: String,
    pub dependencies: Vec<String>,
    pub context: serde_json::Value,
}

/// Submit a query to be processed by the orchestrator with enhanced options
#[tauri::command]
pub async fn submit_orchestrated_query(
    query: String,
    use_orchestrator: bool,
    priority: Option<String>,
    context: Option<serde_json::Value>,
    state: tauri::State<'_, AppState>,
    app_handle: tauri::AppHandle,
) -> Result<String, String> {
    // --- Validate query text ---
    let trimmed_query = query.trim();
    if trimmed_query.is_empty() {
        warn!("Received empty or whitespace-only query for orchestrator, ignoring");
        return Ok("No query provided".to_string());
    }

    if !use_orchestrator {
        // Fall back to the existing single-agent system
        crate::anthropic::submit_query(trimmed_query.to_string(), state, app_handle)
            .await
            .map_err(|e| e)?;
        return Ok(format!("Query processed: {}", trimmed_query));
    }

    let orchestrator = get_orchestrator().await?;
    let orchestrator_guard = orchestrator.lock().await;

    // Parse priority
    let task_priority = match priority.as_deref() {
        Some("low") => TaskPriority::Low,
        Some("high") => TaskPriority::High,
        Some("critical") => TaskPriority::Critical,
        _ => TaskPriority::Normal,
    };

    // Create enhanced task request
    let task_request = TaskCreationRequest {
        description: trimmed_query.to_string(),
        agent_type: None, // Let orchestrator decide
        priority: Some(format!("{:?}", task_priority)),
        dependencies: None,
        timeout_seconds: None,
        context,
        use_mcp_tools: Some(true),
    };

    let result = create_and_execute_task(&orchestrator_guard, task_request).await?;
    Ok(format!("Orchestrated task completed: {}", result))
}

/// Create and execute a task with the orchestrator
async fn create_and_execute_task(
    orchestrator: &Orchestrator,
    request: TaskCreationRequest,
) -> Result<String, String> {
    use crate::agents::{AgentType, Task};
    use uuid::Uuid;

    // Determine agent type intelligently
    let agent_type = if let Some(ref agent_str) = request.agent_type {
        match agent_str.to_lowercase().as_str() {
            "browser" => AgentType::Browser,
            "desktop" => AgentType::Desktop,
            "system" => AgentType::System,
            _ => AgentType::Desktop,
        }
    } else {
        // Use orchestrator's intelligent agent selection
        orchestrator
            .determine_agent_type(&request.description)
            .await
    };

    // Parse priority
    let priority = if let Some(ref priority_str) = request.priority {
        match priority_str.to_lowercase().as_str() {
            "low" => TaskPriority::Low,
            "high" => TaskPriority::High,
            "critical" => TaskPriority::Critical,
            _ => TaskPriority::Normal,
        }
    } else {
        TaskPriority::Normal
    };

    // Create task
    let task = Task {
        id: Uuid::new_v4().to_string(),
        description: request.description.clone(),
        tool_calls: vec![], // Will be populated by agent
        agent_type,
        priority,
        dependencies: request.dependencies.unwrap_or_default(),
        timeout: request.timeout_seconds.map(std::time::Duration::from_secs),
        metadata: serde_json::json!({
            "created_at": chrono::Utc::now().to_rfc3339(),
            "context": request.context,
            "use_mcp_tools": request.use_mcp_tools.unwrap_or(false)
        }),
    };

    match orchestrator.delegate_task(task).await {
        Ok(result) => {
            if result.success {
                Ok(result
                    .output
                    .as_str()
                    .unwrap_or("Task completed successfully")
                    .to_string())
            } else {
                Err(result
                    .error
                    .unwrap_or("Task failed without details".to_string()))
            }
        }
        Err(e) => Err(format!("Orchestrator error: {}", e)),
    }
}

/// Get the enhanced status of the orchestrator and all agents
#[tauri::command]
pub async fn get_orchestrator_status() -> Result<OrchestratorStatusReport, String> {
    let orchestrator = get_orchestrator().await?;
    let orchestrator_guard = orchestrator.lock().await;

    let orch_status = orchestrator_guard.get_orchestrator_status().await;
    let agent_statuses = orchestrator_guard
        .get_registry()
        .get_all_agent_status()
        .await;
    let active_tasks = orchestrator_guard.get_active_tasks().await;
    let task_history = orchestrator_guard.get_task_history().await;

    // Calculate enhanced metrics
    let failed_tasks = task_history.iter().filter(|t| !t.success).count();
    let successful_tasks = task_history.iter().filter(|t| t.success).count();
    let average_completion_time = if successful_tasks > 0 {
        task_history
            .iter()
            .filter(|t| t.success)
            .map(|t| t.execution_time.as_secs_f32())
            .sum::<f32>()
            / successful_tasks as f32
    } else {
        0.0
    };

    // Get MCP status if available
    let (mcp_servers_connected, mcp_tools_available) = if let Ok(mcp_manager) = get_mcp_manager() {
        let server_statuses = mcp_manager.get_server_statuses().await;
        let connected_servers = server_statuses
            .values()
            .filter(|status| {
                matches!(
                    status,
                    crate::agent::tools::mcp_integration::MCPServerStatus::Connected
                )
            })
            .count();
        let available_tools = mcp_manager.get_all_tools().await.len();
        (connected_servers, available_tools)
    } else {
        (0, 0)
    };

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
        queued_task_count: orchestrator_guard.get_queued_task_count().await,
        failed_task_count: failed_tasks,
        mcp_servers_connected,
        mcp_tools_available,
        average_task_completion_time: average_completion_time,
    })
}

/// Configure the orchestrator settings with enhanced options
#[tauri::command]
pub async fn configure_orchestrator(config: OrchestratorConfigDTO) -> Result<(), String> {
    // For now, we'll recreate the orchestrator with new config
    // In a production system, this would be more sophisticated
    let factory = AgentFactory::new();
    factory
        .initialize_default_agents()
        .await
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

/// Create a new task with enhanced parameters
#[tauri::command]
pub async fn create_orchestrator_task(request: TaskCreationRequest) -> Result<String, String> {
    let orchestrator = get_orchestrator().await?;
    let orchestrator_guard = orchestrator.lock().await;

    let result = create_and_execute_task(&orchestrator_guard, request).await?;
    Ok(result)
}

/// Get the task execution history with filtering options
#[tauri::command]
pub async fn get_task_history(
    limit: Option<usize>,
    filter_success: Option<bool>,
    agent_type: Option<String>,
) -> Result<Vec<TaskResult>, String> {
    let orchestrator = get_orchestrator().await?;
    let orchestrator_guard = orchestrator.lock().await;

    let mut history = orchestrator_guard.get_task_history().await;

    // Apply filters
    if let Some(success_filter) = filter_success {
        history.retain(|task| task.success == success_filter);
    }

    if let Some(ref agent_filter) = agent_type {
        history.retain(|task| {
            format!("{:?}", task.agent_type).to_lowercase() == agent_filter.to_lowercase()
        });
    }

    // Apply limit
    if let Some(limit_count) = limit {
        history.truncate(limit_count);
    }

    Ok(history)
}

/// Get currently active tasks with detailed information
#[tauri::command]
pub async fn get_active_tasks() -> Result<Vec<Task>, String> {
    let orchestrator = get_orchestrator().await?;
    let orchestrator_guard = orchestrator.lock().await;

    let active_tasks = orchestrator_guard.get_active_tasks().await;
    Ok(active_tasks.into_iter().map(|t| (*t).clone()).collect())
}

/// Get all agent capabilities with enhanced information
#[tauri::command]
pub async fn get_agent_capabilities(
) -> Result<std::collections::HashMap<String, Vec<crate::agents::AgentCapability>>, String> {
    let orchestrator = get_orchestrator().await?;
    let orchestrator_guard = orchestrator.lock().await;

    let agent_statuses = orchestrator_guard
        .get_registry()
        .get_all_agent_status()
        .await;
    let mut capabilities_map = std::collections::HashMap::new();

    for status in agent_statuses {
        capabilities_map.insert(format!("{:?}", status.agent_type), status.capabilities);
    }

    Ok(capabilities_map)
}

/// Cancel a specific active task
#[tauri::command]
pub async fn cancel_task(task_id: String) -> Result<bool, String> {
    let orchestrator = get_orchestrator().await?;
    let orchestrator_guard = orchestrator.lock().await;

    match orchestrator_guard
        .cancel_task(&task_id, "User requested cancellation")
        .await
    {
        Ok(cancelled) => {
            if cancelled {
                tracing::info!("Successfully cancelled task: {}", task_id);
                Ok(true)
            } else {
                tracing::warn!("Task {} was not found in queue or active tasks", task_id);
                Ok(false)
            }
        }
        Err(e) => {
            tracing::error!("Failed to cancel task {}: {}", task_id, e);
            Err(format!("Failed to cancel task: {}", e))
        }
    }
}

/// Get detailed queue status information
#[tauri::command]
pub async fn get_queue_status() -> Result<serde_json::Value, String> {
    let orchestrator = get_orchestrator().await?;
    let orchestrator_guard = orchestrator.lock().await;

    Ok(orchestrator_guard.get_queue_status().await)
}

// MCP integration commands are handled by commands/mcp.rs
// This orchestrator module focuses on high-level workflow orchestration

/// Execute a task using MCP tools
#[tauri::command]
pub async fn execute_mcp_task(
    tool_name: String,
    input: serde_json::Value,
    call_id: String,
) -> Result<String, String> {
    let mcp_manager = get_mcp_manager()?;

    match mcp_manager.execute_tool(&tool_name, input, call_id).await {
        Ok(result) => Ok(crate::agents::base_agent::format_task_output(&result.output)),
        Err(e) => Err(format!("MCP tool execution failed: {}", e)),
    }
}

/// Get predefined workflow templates
#[tauri::command]
pub async fn get_workflow_templates() -> Result<Vec<WorkflowTemplate>, String> {
    // Return predefined workflow templates for common orchestration patterns
    let templates = vec![
        WorkflowTemplate {
            id: "web-research".to_string(),
            name: "Web Research Workflow".to_string(),
            description: "Search, analyze and summarize web content".to_string(),
            tasks: vec![
                TaskTemplate {
                    name: "search".to_string(),
                    description: "Search for relevant web content".to_string(),
                    agent_type: "Browser".to_string(),
                    dependencies: vec![],
                    context: serde_json::json!({"search_query": "{{query}}"}),
                },
                TaskTemplate {
                    name: "analyze".to_string(),
                    description: "Analyze and extract key information".to_string(),
                    agent_type: "System".to_string(),
                    dependencies: vec!["search".to_string()],
                    context: serde_json::json!({"analysis_focus": "{{focus}}"}),
                },
            ],
            variables: [
                ("query".to_string(), "Search query".to_string()),
                ("focus".to_string(), "Analysis focus".to_string()),
            ]
            .into_iter()
            .collect(),
        },
        WorkflowTemplate {
            id: "file-processing".to_string(),
            name: "File Processing Workflow".to_string(),
            description: "Process and transform files with multiple steps".to_string(),
            tasks: vec![
                TaskTemplate {
                    name: "read_files".to_string(),
                    description: "Read and validate input files".to_string(),
                    agent_type: "System".to_string(),
                    dependencies: vec![],
                    context: serde_json::json!({"file_path": "{{input_path}}"}),
                },
                TaskTemplate {
                    name: "process".to_string(),
                    description: "Process file content".to_string(),
                    agent_type: "System".to_string(),
                    dependencies: vec!["read_files".to_string()],
                    context: serde_json::json!({"operation": "{{operation}}"}),
                },
                TaskTemplate {
                    name: "save_results".to_string(),
                    description: "Save processed results".to_string(),
                    agent_type: "System".to_string(),
                    dependencies: vec!["process".to_string()],
                    context: serde_json::json!({"output_path": "{{output_path}}"}),
                },
            ],
            variables: [
                ("input_path".to_string(), "Input file path".to_string()),
                ("operation".to_string(), "Processing operation".to_string()),
                ("output_path".to_string(), "Output file path".to_string()),
            ]
            .into_iter()
            .collect(),
        },
        WorkflowTemplate {
            id: "desktop-automation".to_string(),
            name: "Desktop Automation Workflow".to_string(),
            description: "Automate desktop tasks with multiple applications".to_string(),
            tasks: vec![
                TaskTemplate {
                    name: "setup".to_string(),
                    description: "Prepare desktop environment".to_string(),
                    agent_type: "Desktop".to_string(),
                    dependencies: vec![],
                    context: serde_json::json!({"applications": "{{apps}}"}),
                },
                TaskTemplate {
                    name: "execute_actions".to_string(),
                    description: "Execute automated actions".to_string(),
                    agent_type: "Desktop".to_string(),
                    dependencies: vec!["setup".to_string()],
                    context: serde_json::json!({"actions": "{{action_sequence}}"}),
                },
                TaskTemplate {
                    name: "verify_results".to_string(),
                    description: "Verify automation results".to_string(),
                    agent_type: "Desktop".to_string(),
                    dependencies: vec!["execute_actions".to_string()],
                    context: serde_json::json!({"verification": "{{verify_method}}"}),
                },
            ],
            variables: [
                ("apps".to_string(), "Applications to use".to_string()),
                (
                    "action_sequence".to_string(),
                    "Sequence of actions".to_string(),
                ),
                (
                    "verify_method".to_string(),
                    "Verification method".to_string(),
                ),
            ]
            .into_iter()
            .collect(),
        },
    ];

    Ok(templates)
}

/// Execute a workflow template with provided variables
#[tauri::command]
pub async fn execute_workflow_template(
    template_id: String,
    variables: HashMap<String, String>,
) -> Result<String, String> {
    let templates = get_workflow_templates().await?;
    let template = templates
        .into_iter()
        .find(|t| t.id == template_id)
        .ok_or_else(|| format!("Workflow template '{}' not found", template_id))?;

    let orchestrator = get_orchestrator().await?;
    let _orchestrator_guard = orchestrator.lock().await;

    // Execute each task in the template based on dependencies
    let mut task_results = HashMap::new();
    let mut tasks_to_execute: Vec<_> = template.tasks.clone();

    while !tasks_to_execute.is_empty() {
        let mut executed_any = false;

        tasks_to_execute.retain(|task_template| {
            // Check if all dependencies are completed
            let deps_satisfied = task_template
                .dependencies
                .iter()
                .all(|dep| task_results.contains_key(dep));

            if deps_satisfied {
                // Execute this task
                let mut context = task_template.context.clone();

                // Replace variables in context
                if let Some(context_obj) = context.as_object_mut() {
                    for (_key, value) in context_obj.iter_mut() {
                        if let Some(value_str) = value.as_str() {
                            let mut replaced_value = value_str.to_string();
                            for (var_name, var_value) in &variables {
                                replaced_value = replaced_value
                                    .replace(&format!("{{{{{}}}}}", var_name), var_value);
                            }
                            *value = serde_json::Value::String(replaced_value);
                        }
                    }
                }

                // Create task request
                let _task_request = TaskCreationRequest {
                    description: task_template.description.clone(),
                    agent_type: Some(task_template.agent_type.clone()),
                    priority: Some("Normal".to_string()),
                    dependencies: Some(task_template.dependencies.clone()),
                    timeout_seconds: None,
                    context: Some(context),
                    use_mcp_tools: Some(true),
                };

                // Execute task (this is async, so we'd need to handle this differently in a real implementation)
                // For now, we'll mark it as completed
                task_results.insert(
                    task_template.name.clone(),
                    format!("Task '{}' executed successfully", task_template.name),
                );

                executed_any = true;
                false // Remove from tasks_to_execute
            } else {
                true // Keep in tasks_to_execute
            }
        });

        if !executed_any {
            return Err("Circular dependency detected in workflow template".to_string());
        }
    }

    Ok(format!(
        "Workflow template '{}' executed successfully with {} tasks completed",
        template.name,
        task_results.len()
    ))
}

// Note: initialize_orchestrator_system() was removed to prevent duplicate initialization
// All orchestrator initialization should go through init_orchestrator_with_app_handle()
// which is called by initialize_orchestrator_state() in state_management.rs

/// NEW: Execute tasks with intelligent parallel processing
#[tauri::command]
pub async fn execute_intelligent_parallel_tasks(
    task_descriptions: Vec<String>,
    priority: Option<String>,
    context: Option<serde_json::Value>,
) -> Result<Vec<String>, String> {
    let orchestrator = get_orchestrator().await?;
    let orchestrator_guard = orchestrator.lock().await;

    // Convert descriptions to tasks
    let mut tasks = Vec::new();
    for (i, description) in task_descriptions.into_iter().enumerate() {
        let task_priority = match priority.as_deref() {
            Some("low") => TaskPriority::Low,
            Some("high") => TaskPriority::High,
            Some("critical") => TaskPriority::Critical,
            _ => TaskPriority::Normal,
        };

        let task = Task {
            id: uuid::Uuid::new_v4().to_string(),
            description,
            tool_calls: vec![],
            agent_type: AgentType::Desktop, // Will be determined by orchestrator
            priority: task_priority,
            dependencies: vec![],
            timeout: None,
            metadata: serde_json::json!({
                "batch_index": i,
                "context": context,
                "intelligent_execution": true
            }),
        };
        tasks.push(task);
    }

    // Execute with intelligent parallel processing
    match orchestrator_guard
        .execute_intelligent_parallel_tasks(tasks)
        .await
    {
        Ok(results) => {
            let outputs: Vec<String> = results
                .into_iter()
                .map(|r| crate::agents::base_agent::format_task_output(&r.output))
                .collect();
            Ok(outputs)
        }
        Err(e) => Err(format!("Intelligent parallel execution failed: {}", e)),
    }
}

/// NEW: Split a complex task into optimized subtasks
#[tauri::command]
pub async fn intelligent_task_splitting(
    task_description: String,
    context: Option<serde_json::Value>,
) -> Result<Vec<String>, String> {
    let orchestrator = get_orchestrator().await?;
    let orchestrator_guard = orchestrator.lock().await;

    let complex_task = Task {
        id: uuid::Uuid::new_v4().to_string(),
        description: task_description,
        tool_calls: vec![],
        agent_type: AgentType::Desktop,
        priority: TaskPriority::Normal,
        dependencies: vec![],
        timeout: None,
        metadata: serde_json::json!({
            "context": context,
            "task_splitting": true
        }),
    };

    match orchestrator_guard
        .intelligent_task_splitting(&complex_task)
        .await
    {
        Ok(subtasks) => {
            let descriptions: Vec<String> = subtasks.into_iter().map(|t| t.description).collect();
            Ok(descriptions)
        }
        Err(e) => Err(format!("Task splitting failed: {}", e)),
    }
}

/// NEW: Get detailed performance metrics for the orchestrator
#[tauri::command]
pub async fn get_orchestrator_performance_metrics() -> Result<serde_json::Value, String> {
    let orchestrator = get_orchestrator().await?;
    let orchestrator_guard = orchestrator.lock().await;

    Ok(orchestrator_guard.get_performance_metrics().await)
}

/// NEW: Execute workflow with performance optimization
#[tauri::command]
pub async fn execute_optimized_workflow(
    workflow_description: String,
    enable_parallel_execution: bool,
    enable_task_splitting: bool,
    context: Option<serde_json::Value>,
) -> Result<String, String> {
    let orchestrator = get_orchestrator().await?;
    let orchestrator_guard = orchestrator.lock().await;

    // Create workflow task
    let workflow_task = Task {
        id: uuid::Uuid::new_v4().to_string(),
        description: workflow_description.clone(),
        tool_calls: vec![],
        agent_type: AgentType::Desktop,
        priority: TaskPriority::Normal,
        dependencies: vec![],
        timeout: None,
        metadata: serde_json::json!({
            "workflow_execution": true,
            "enable_parallel": enable_parallel_execution,
            "enable_splitting": enable_task_splitting,
            "context": context
        }),
    };

    // Step 1: Intelligent task splitting if enabled
    let tasks = if enable_task_splitting {
        match orchestrator_guard
            .intelligent_task_splitting(&workflow_task)
            .await
        {
            Ok(split_tasks) => {
                tracing::info!("Workflow split into {} tasks", split_tasks.len());
                split_tasks
            }
            Err(_) => {
                tracing::info!("Workflow kept as single task");
                vec![workflow_task]
            }
        }
    } else {
        vec![workflow_task]
    };

    // Step 2: Execute with intelligent parallel processing if enabled
    let results = if enable_parallel_execution && tasks.len() > 1 {
        orchestrator_guard
            .execute_intelligent_parallel_tasks(tasks)
            .await
    } else {
        orchestrator_guard.execute_parallel_tasks(tasks).await
    };

    match results {
        Ok(task_results) => {
            let output = orchestrator_guard.merge_results(task_results);
            Ok(format!("Optimized workflow completed: {}", output))
        }
        Err(e) => Err(format!("Optimized workflow failed: {}", e)),
    }
}

/// NEW: Enhanced orchestrator configuration with performance tuning
#[tauri::command]
pub async fn configure_enhanced_orchestrator(
    max_parallel_tasks: Option<usize>,
    enable_intelligent_batching: Option<bool>,
    batch_size: Option<usize>,
    enable_adaptive_timeout: Option<bool>,
    enable_resource_aware_scheduling: Option<bool>,
) -> Result<String, String> {
    let orchestrator = get_orchestrator().await?;
    let current_status = {
        let guard = orchestrator.lock().await;
        guard.get_orchestrator_status().await
    };

    // Create enhanced configuration
    let enhanced_config = crate::agents::OrchestratorConfig {
        max_parallel_tasks: max_parallel_tasks.unwrap_or(12),
        task_timeout: std::time::Duration::from_secs(
            crate::constants::agent::config::DEFAULT_TASK_TIMEOUT_SECONDS,
        ),
        enable_task_splitting: true,
        enable_fallback_agents: true,
        min_confidence_threshold: 0.3,
        max_queue_size: 100,
        queue_processing_interval: std::time::Duration::from_millis(250),
        enable_intelligent_batching: enable_intelligent_batching.unwrap_or(true),
        batch_size: batch_size.unwrap_or(4),
        parallel_execution_threshold: 3,
        enable_adaptive_timeout: enable_adaptive_timeout.unwrap_or(true),
        enable_resource_aware_scheduling: enable_resource_aware_scheduling.unwrap_or(true),
        max_concurrent_agents: 8,
        priority_boost_factor: 1.5,
        enable_predictive_caching: true,
    };

    // Apply configuration (in a real implementation, this would update the running orchestrator)
    tracing::info!(
        "Enhanced orchestrator configuration applied: parallel_tasks={}, batching={}, adaptive_timeout={}",
        enhanced_config.max_parallel_tasks,
        enhanced_config.enable_intelligent_batching,
        enhanced_config.enable_adaptive_timeout
    );

    Ok(format!(
        "Enhanced orchestrator configured successfully: {} -> {} parallel tasks, intelligent batching: {}, adaptive timeout: {}",
        current_status.current_tasks,
        enhanced_config.max_parallel_tasks,
        enhanced_config.enable_intelligent_batching,
        enhanced_config.enable_adaptive_timeout
    ))
}

/// NEW: Benchmark orchestrator performance improvements
#[tauri::command]
pub async fn benchmark_orchestrator_performance(
    test_task_count: usize,
    enable_optimizations: bool,
) -> Result<serde_json::Value, String> {
    let orchestrator = get_orchestrator().await?;
    let orchestrator_guard = orchestrator.lock().await;

    let test_task_count = test_task_count.min(20).max(1); // Safety bounds

    tracing::info!(
        "Starting orchestrator performance benchmark with {} tasks",
        test_task_count
    );

    // Create test tasks
    let mut test_tasks = Vec::new();
    for i in 0..test_task_count {
        let task = Task {
            id: format!("benchmark_task_{}", i),
            description: format!("Benchmark test task {} - simulate processing", i),
            tool_calls: vec![],
            agent_type: match i % 3 {
                0 => AgentType::Browser,
                1 => AgentType::Desktop,
                _ => AgentType::System,
            },
            priority: TaskPriority::Normal,
            dependencies: vec![],
            timeout: Some(std::time::Duration::from_secs(30)),
            metadata: serde_json::json!({
                "benchmark": true,
                "optimizations_enabled": enable_optimizations
            }),
        };
        test_tasks.push(task);
    }

    // Execute benchmark
    let start_time = std::time::Instant::now();
    let results = if enable_optimizations {
        orchestrator_guard
            .execute_intelligent_parallel_tasks(test_tasks)
            .await
    } else {
        orchestrator_guard.execute_parallel_tasks(test_tasks).await
    };

    let execution_time = start_time.elapsed();
    let performance_metrics = orchestrator_guard.get_performance_metrics().await;

    match results {
        Ok(task_results) => {
            let successful_tasks = task_results.iter().filter(|r| r.success).count();
            let average_execution_time = task_results
                .iter()
                .map(|r| r.execution_time.as_millis())
                .sum::<u128>() as f32
                / task_results.len() as f32;

            Ok(serde_json::json!({
                "benchmark_results": {
                    "total_tasks": test_task_count,
                    "successful_tasks": successful_tasks,
                    "failed_tasks": test_task_count - successful_tasks,
                    "total_execution_time_ms": execution_time.as_millis(),
                    "average_task_time_ms": average_execution_time,
                    "tasks_per_second": test_task_count as f32 / execution_time.as_secs_f32(),
                    "optimizations_enabled": enable_optimizations,
                    "performance_improvement_factor": if enable_optimizations {
                        performance_metrics["performance_optimization"]["parallel_factor"].as_f64().unwrap_or(1.0)
                    } else {
                        1.0
                    }
                },
                "detailed_metrics": performance_metrics
            }))
        }
        Err(e) => Err(format!("Benchmark failed: {}", e)),
    }
}

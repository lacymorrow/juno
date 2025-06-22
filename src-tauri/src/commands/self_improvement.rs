//! Self-Improvement Commands for Juno AI
//!
//! Provides frontend interface for autonomous code generation and system enhancement capabilities.

use serde::{Deserialize, Serialize};
use tauri::{command, State};
use std::sync::Arc;
use tokio::sync::Mutex;

use crate::agent::core::AgentError;
use crate::agent::tools::self_improvement::{
    SelfImprovementEngine, SelfImprovementConfig, ImprovementIteration,
    SafetyConstraints, BenchmarkResults, IterationStatus
};

/// Global self-improvement engine state
type SelfImprovementState = Arc<Mutex<Option<SelfImprovementEngine>>>;

#[derive(Debug, Serialize, Deserialize)]
pub struct SelfImprovementStatus {
    pub enabled: bool,
    pub current_iteration: Option<String>,
    pub total_iterations: u32,
    pub last_improvement_score: f64,
    pub archive_size: u32,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct InitializeSelfImprovementRequest {
    pub config: SelfImprovementConfig,
}

/// Initialize the self-improvement system
#[command]
pub async fn initialize_self_improvement(
    request: InitializeSelfImprovementRequest,
    state: State<'_, SelfImprovementState>,
) -> Result<String, String> {
    let engine = SelfImprovementEngine::new(request.config)
        .map_err(|e| format!("Failed to initialize self-improvement engine: {}", e))?;

    let mut state_guard = state.lock().await;
    *state_guard = Some(engine);

    log::info!("Self-improvement system initialized successfully");
    Ok("Self-improvement system initialized".to_string())
}

/// Start a self-improvement cycle
#[command]
pub async fn start_improvement_cycle(
    state: State<'_, SelfImprovementState>,
) -> Result<ImprovementIteration, String> {
    let mut state_guard = state.lock().await;

    if let Some(engine) = state_guard.as_mut() {
        let iteration = engine.execute_improvement_cycle().await
            .map_err(|e| format!("Failed to execute improvement cycle: {}", e))?;

        log::info!("Improvement cycle completed: iteration {}", iteration.id);
        Ok(iteration)
    } else {
        Err("Self-improvement system not initialized".to_string())
    }
}

/// Get the current status of the self-improvement system
#[command]
pub async fn get_self_improvement_status(
    state: State<'_, SelfImprovementState>,
) -> Result<SelfImprovementStatus, String> {
    let state_guard = state.lock().await;

    if let Some(engine) = state_guard.as_ref() {
        let status = SelfImprovementStatus {
            enabled: true,
            current_iteration: engine.current_iteration.as_ref().map(|i| i.id.clone()),
            total_iterations: engine.archive.len() as u32,
            last_improvement_score: engine.archive.last()
                .map(|i| i.utility_score)
                .unwrap_or(0.0),
            archive_size: engine.archive.len() as u32,
        };

        Ok(status)
    } else {
        Ok(SelfImprovementStatus {
            enabled: false,
            current_iteration: None,
            total_iterations: 0,
            last_improvement_score: 0.0,
            archive_size: 0,
        })
    }
}

/// Get the improvement archive (history of all iterations)
#[command]
pub async fn get_improvement_archive(
    state: State<'_, SelfImprovementState>,
) -> Result<Vec<ImprovementIteration>, String> {
    let state_guard = state.lock().await;

    if let Some(engine) = state_guard.as_ref() {
        Ok(engine.archive.clone())
    } else {
        Err("Self-improvement system not initialized".to_string())
    }
}

/// Get detailed results for a specific iteration
#[command]
pub async fn get_iteration_details(
    iteration_id: String,
    state: State<'_, SelfImprovementState>,
) -> Result<Option<ImprovementIteration>, String> {
    let state_guard = state.lock().await;

    if let Some(engine) = state_guard.as_ref() {
        let iteration = engine.archive.iter()
            .find(|i| i.id == iteration_id)
            .cloned();

        Ok(iteration)
    } else {
        Err("Self-improvement system not initialized".to_string())
    }
}

/// Run benchmarks manually to evaluate current performance
#[command]
pub async fn run_performance_benchmarks(
    state: State<'_, SelfImprovementState>,
) -> Result<BenchmarkResults, String> {
    let state_guard = state.lock().await;

    if let Some(engine) = state_guard.as_ref() {
        // Create a dummy iteration for benchmark evaluation
        let dummy_iteration = ImprovementIteration {
            id: "benchmark-run".to_string(),
            timestamp: chrono::Utc::now(),
            changes: Vec::new(),
            benchmark_results: BenchmarkResults::default(),
            utility_score: 0.0,
            status: IterationStatus::Evaluating,
        };

        let results = engine.evaluate_improvements(&dummy_iteration).await
            .map_err(|e| format!("Failed to run benchmarks: {}", e))?;

        Ok(results)
    } else {
        Err("Self-improvement system not initialized".to_string())
    }
}

/// Update self-improvement configuration
#[command]
pub async fn update_self_improvement_config(
    config: SelfImprovementConfig,
    state: State<'_, SelfImprovementState>,
) -> Result<String, String> {
    let mut state_guard = state.lock().await;

    if let Some(engine) = state_guard.as_mut() {
        engine.config = config;
        Ok("Configuration updated successfully".to_string())
    } else {
        Err("Self-improvement system not initialized".to_string())
    }
}

/// Emergency stop for self-improvement process
#[command]
pub async fn emergency_stop_improvement(
    state: State<'_, SelfImprovementState>,
) -> Result<String, String> {
    let mut state_guard = state.lock().await;

    if let Some(engine) = state_guard.as_mut() {
        if let Some(current) = &engine.current_iteration {
            log::warn!("Emergency stop triggered for iteration: {}", current.id);

            // Rollback current iteration if in progress
            if current.status == IterationStatus::Implementing ||
               current.status == IterationStatus::Testing {
                let mut iteration = current.clone();
                engine.rollback_iteration(iteration).await
                    .map_err(|e| format!("Failed to rollback iteration: {}", e))?;
            }
        }

        engine.current_iteration = None;
        Ok("Self-improvement stopped and rolled back".to_string())
    } else {
        Err("Self-improvement system not initialized".to_string())
    }
}

/// Analyze current system performance without making changes
#[command]
pub async fn analyze_system_performance(
    state: State<'_, SelfImprovementState>,
) -> Result<serde_json::Value, String> {
    let state_guard = state.lock().await;

    if let Some(engine) = state_guard.as_ref() {
        let analysis = engine.analyze_current_performance().await
            .map_err(|e| format!("Failed to analyze performance: {}", e))?;

        // Convert analysis to JSON for frontend consumption
        let analysis_json = serde_json::json!({
            "tool_metrics": {
                "usage_frequency": analysis.tool_metrics.usage_frequency,
                "success_rate": analysis.tool_metrics.success_rate,
                "average_execution_time": analysis.tool_metrics.average_execution_time
            },
            "prompt_metrics": {
                "effectiveness_score": analysis.prompt_metrics.effectiveness_score,
                "token_efficiency": analysis.prompt_metrics.token_efficiency,
                "failure_rate": analysis.prompt_metrics.failure_rate
            },
            "improvement_opportunities": analysis.improvement_opportunities.len(),
            "system_bottlenecks": analysis.bottlenecks.len(),
            "failure_patterns": analysis.failure_patterns.len()
        });

        Ok(analysis_json)
    } else {
        Err("Self-improvement system not initialized".to_string())
    }
}

/// Generate an improvement proposal without implementing it
#[command]
pub async fn generate_improvement_proposal(
    state: State<'_, SelfImprovementState>,
) -> Result<serde_json::Value, String> {
    let state_guard = state.lock().await;

    if let Some(engine) = state_guard.as_ref() {
        let analysis = engine.analyze_current_performance().await
            .map_err(|e| format!("Failed to analyze performance: {}", e))?;

        let meta_agent = engine.select_meta_agent()
            .map_err(|e| format!("Failed to select meta-agent: {}", e))?;

        let proposal = engine.generate_improvement_proposal(&analysis, meta_agent).await
            .map_err(|e| format!("Failed to generate proposal: {}", e))?;

        let proposal_json = serde_json::json!({
            "rationale": proposal.rationale,
            "expected_impact": proposal.expected_impact,
            "changes": proposal.changes.iter().map(|change| {
                serde_json::json!({
                    "change_type": format!("{:?}", change.change_type),
                    "target_file": change.target_file,
                    "description": change.description,
                    "implementation_details": change.implementation_details
                })
            }).collect::<Vec<_>>()
        });

        Ok(proposal_json)
    } else {
        Err("Self-improvement system not initialized".to_string())
    }
}

/// Get system health metrics for self-improvement decision making
#[command]
pub async fn get_system_health_metrics() -> Result<serde_json::Value, String> {
    // Collect system health metrics
    let metrics = serde_json::json!({
        "cpu_usage": get_cpu_usage().await.unwrap_or(0.0),
        "memory_usage": get_memory_usage().await.unwrap_or(0.0),
        "disk_usage": get_disk_usage().await.unwrap_or(0.0),
        "agent_success_rate": get_agent_success_rate().await.unwrap_or(0.0),
        "tool_reliability": get_tool_reliability().await.unwrap_or(0.0),
        "timestamp": chrono::Utc::now()
    });

    Ok(metrics)
}

// Helper functions for system metrics
async fn get_cpu_usage() -> Result<f64, AgentError> {
    // Implementation for CPU usage
    Ok(0.0)
}

async fn get_memory_usage() -> Result<f64, AgentError> {
    // Implementation for memory usage
    Ok(0.0)
}

async fn get_disk_usage() -> Result<f64, AgentError> {
    // Implementation for disk usage
    Ok(0.0)
}

async fn get_agent_success_rate() -> Result<f64, AgentError> {
    // Implementation for agent success rate
    Ok(0.0)
}

async fn get_tool_reliability() -> Result<f64, AgentError> {
    // Implementation for tool reliability
    Ok(0.0)
}

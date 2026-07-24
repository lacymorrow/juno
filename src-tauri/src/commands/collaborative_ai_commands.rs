use tauri::State;
use std::sync::Arc;
use tokio::sync::RwLock;
use serde::{Deserialize, Serialize};

use crate::agent::tools::collaborative_ai::{
    CollaborativeAIDesigner, CollaborativeAIConfig, SystemRequirements, ComplexityLevel,
    WorkflowDesignResult, WorkflowExecutionResult, DesignCapabilities, DesignStatistics,
    PerformanceRequirements, ResourceRequirements
};


/// Global state for the Collaborative AI Designer
pub struct CollaborativeAIState {
    pub designer: Arc<RwLock<CollaborativeAIDesigner>>,
}

#[allow(clippy::new_without_default)]
impl CollaborativeAIState {
    pub fn new() -> Self {
        let config = CollaborativeAIConfig::default();
        let designer = CollaborativeAIDesigner::new(config);

        Self {
            designer: Arc::new(RwLock::new(designer)),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CollaborativeAIRequest {
    pub description: String,
    pub goals: Vec<String>,
    pub constraints: Vec<String>,
    pub preferred_technologies: Vec<String>,
    pub complexity_level: String, // "simple", "moderate", "complex", "expert"
    pub timeline_hours: u64,
    pub performance_requirements: PerformanceRequirementsInput,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PerformanceRequirementsInput {
    pub max_response_time_ms: u64,
    pub min_throughput: f32,
    pub availability_percent: f32,
    pub max_cpu_cores: u32,
    pub max_memory_mb: u64,
    pub max_disk_space_mb: u64,
    pub max_network_bandwidth: u64,
    pub max_concurrent_agents: u32,
}

impl From<CollaborativeAIRequest> for SystemRequirements {
    fn from(request: CollaborativeAIRequest) -> Self {
        let complexity_level = match request.complexity_level.to_lowercase().as_str() {
            "simple" => ComplexityLevel::Simple,
            "moderate" => ComplexityLevel::Moderate,
            "complex" => ComplexityLevel::Complex,
            "expert" => ComplexityLevel::Expert,
            _ => ComplexityLevel::Moderate,
        };

        SystemRequirements {
            description: request.description,
            goals: request.goals,
            constraints: request.constraints,
            preferred_technologies: request.preferred_technologies,
            complexity_level,
            timeline: std::time::Duration::from_secs(request.timeline_hours * 3600),
            performance_requirements: PerformanceRequirements {
                max_response_time_ms: request.performance_requirements.max_response_time_ms,
                min_throughput: request.performance_requirements.min_throughput,
                availability_percent: request.performance_requirements.availability_percent,
                max_resource_usage: ResourceRequirements {
                    cpu_cores: request.performance_requirements.max_cpu_cores,
                    memory_mb: request.performance_requirements.max_memory_mb,
                    disk_space_mb: request.performance_requirements.max_disk_space_mb,
                    network_bandwidth: request.performance_requirements.max_network_bandwidth,
                    concurrent_agents: request.performance_requirements.max_concurrent_agents,
                },
            },
        }
    }
}

/// Design a collaborative AI system
#[tauri::command]
pub async fn design_collaborative_ai_system(
    request: CollaborativeAIRequest,
    state: State<'_, CollaborativeAIState>,
) -> Result<WorkflowDesignResult, String> {
    tracing::info!("Designing collaborative AI system: {}", request.description);

    let designer = state.designer.read().await;
    let requirements = SystemRequirements::from(request);

    match designer.design_collaborative_system(&requirements).await {
        Ok(result) => {
            tracing::info!("Collaborative AI system designed successfully with {:.1}% estimated success rate",
                          result.success_rate * crate::constants::text::ratios::PERCENTAGE_MULTIPLIER as f32);
            Ok(result)
        }
        Err(e) => {
            tracing::error!("Failed to design collaborative AI system: {}", e);
            Err(format!("Design failed: {}", e))
        }
    }
}

/// Execute a designed workflow
#[tauri::command]
pub async fn execute_collaborative_workflow(
    workflow: WorkflowDesignResult,
    state: State<'_, CollaborativeAIState>,
) -> Result<WorkflowExecutionResult, String> {
    tracing::info!("Executing collaborative AI workflow");

    let designer = state.designer.read().await;

    match designer.execute_workflow(&workflow).await {
        Ok(result) => {
            tracing::info!("Workflow execution completed with {:.1}% success rate",
                          result.success_rate * crate::constants::text::ratios::PERCENTAGE_MULTIPLIER as f32);
            Ok(result)
        }
        Err(e) => {
            tracing::error!("Failed to execute workflow: {}", e);
            Err(format!("Execution failed: {}", e))
        }
    }
}

/// Get design capabilities of the system
#[tauri::command]
pub async fn get_collaborative_ai_capabilities(
    state: State<'_, CollaborativeAIState>,
) -> Result<DesignCapabilities, String> {
    tracing::debug!("Getting collaborative AI design capabilities");

    let designer = state.designer.read().await;
    Ok(designer.get_design_capabilities().await)
}

/// Get design statistics
#[tauri::command]
pub async fn get_collaborative_ai_statistics(
    state: State<'_, CollaborativeAIState>,
) -> Result<DesignStatistics, String> {
    tracing::debug!("Getting collaborative AI design statistics");

    let designer = state.designer.read().await;
    Ok(designer.get_design_statistics().await)
}

/// Create a sample collaborative AI request for testing
#[tauri::command]
pub async fn create_sample_collaborative_ai_request() -> Result<CollaborativeAIRequest, String> {
    Ok(CollaborativeAIRequest {
        description: "Design an intelligent document processing system that can analyze, categorize, and extract key information from various document types".to_string(),
        goals: vec![
            "Automatically classify documents by type".to_string(),
            "Extract key information from documents".to_string(),
            "Generate summaries for complex documents".to_string(),
            "Provide quality assurance and validation".to_string(),
        ],
        constraints: vec![
            "Must handle PDF, Word, and image documents".to_string(),
            "Processing time should be under 30 seconds per document".to_string(),
            "Must maintain 95% accuracy for key information extraction".to_string(),
        ],
        preferred_technologies: vec![
            "Computer Vision".to_string(),
            "Natural Language Processing".to_string(),
            "Machine Learning".to_string(),
        ],
        complexity_level: "complex".to_string(),
        timeline_hours: 72, // 3 days
        performance_requirements: PerformanceRequirementsInput {
            max_response_time_ms: 30000, // 30 seconds
            min_throughput: 2.0, // documents per minute
            availability_percent: 99.5,
            max_cpu_cores: 4,
            max_memory_mb: 8192, // 8GB
            max_disk_space_mb: 2048, // 2GB
            max_network_bandwidth: 1000, // 1Mbps
            max_concurrent_agents: 6,
        },
    })
}

/// Validate a collaborative AI request
#[tauri::command]
pub async fn validate_collaborative_ai_request(
    request: CollaborativeAIRequest,
) -> Result<ValidationResult, String> {
    let mut errors = Vec::new();
    let mut warnings = Vec::new();

    // Validate description
    if request.description.trim().is_empty() {
        errors.push("Description cannot be empty".to_string());
    } else if request.description.len() < crate::constants::text::validation::MIN_REQUEST_DESCRIPTION_LENGTH {
        warnings.push("Description is very short, consider adding more details".to_string());
    }

    // Validate goals
    if request.goals.is_empty() {
        errors.push("At least one goal must be specified".to_string());
    } else if request.goals.len() > crate::constants::text::validation::MAX_COLLABORATIVE_AI_GOALS {
        warnings.push("Large number of goals may increase complexity".to_string());
    }

    // Validate timeline
    if request.timeline_hours == 0 {
        errors.push("Timeline must be greater than 0 hours".to_string());
    } else if request.timeline_hours > 8760 { // 1 year
        warnings.push("Very long timeline specified".to_string());
    }

    // Validate performance requirements
    if request.performance_requirements.max_response_time_ms == 0 {
        errors.push("Max response time must be greater than 0".to_string());
    }

    if request.performance_requirements.availability_percent < crate::constants::text::ratios::MIN_PERCENTAGE as f32 ||
       request.performance_requirements.availability_percent > crate::constants::text::ratios::MAX_PERCENTAGE as f32 {
        errors.push(format!("Availability percent must be between {} and {}",
                           crate::constants::text::ratios::MIN_PERCENTAGE,
                           crate::constants::text::ratios::MAX_PERCENTAGE));
    }

    // Validate complexity level
    let valid_complexity_levels = ["simple", "moderate", "complex", "expert"];
    if !valid_complexity_levels.contains(&request.complexity_level.to_lowercase().as_str()) {
        errors.push("Invalid complexity level. Must be one of: simple, moderate, complex, expert".to_string());
    }

    let is_valid = errors.is_empty();
    let estimated_complexity = estimate_request_complexity(&request);

    Ok(ValidationResult {
        is_valid,
        errors,
        warnings,
        estimated_complexity,
        estimated_design_time_hours: estimate_design_time(&request),
    })
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidationResult {
    pub is_valid: bool,
    pub errors: Vec<String>,
    pub warnings: Vec<String>,
    pub estimated_complexity: f32,
    pub estimated_design_time_hours: f32,
}

fn estimate_request_complexity(request: &CollaborativeAIRequest) -> f32 {
    let mut complexity = 1.0;

    // Add complexity based on number of goals
    complexity += request.goals.len() as f32 * 0.5;

    // Add complexity based on constraints
    complexity += request.constraints.len() as f32 * 0.3;

    // Add complexity based on technologies
    complexity += request.preferred_technologies.len() as f32 * 0.2;

    // Add complexity based on performance requirements
    if request.performance_requirements.availability_percent > 99.0 {
        complexity += 1.0;
    }

    if request.performance_requirements.max_response_time_ms < 5000 {
        complexity += 0.5;
    }

    // Factor in complexity level
    match request.complexity_level.to_lowercase().as_str() {
        "simple" => complexity *= 0.8,
        "moderate" => complexity *= 1.0,
        "complex" => complexity *= 1.5,
        "expert" => complexity *= 2.0,
        _ => complexity *= 1.0,
    }

    complexity.min(crate::constants::text::validation::MAX_COLLABORATIVE_AI_GOALS as f32) // Cap at 10.0
}

fn estimate_design_time(request: &CollaborativeAIRequest) -> f32 {
    let complexity = estimate_request_complexity(request);

    // Base time estimation in hours
    let base_time = match request.complexity_level.to_lowercase().as_str() {
        "simple" => 2.0,
        "moderate" => 8.0,
        "complex" => 24.0,
        "expert" => 72.0,
        _ => 8.0,
    };

    // Adjust based on calculated complexity
    let adjusted_time = base_time * (complexity / 5.0);

    adjusted_time.clamp(0.5, 168.0) // Between 30 minutes and 1 week
}

/// Get available complexity levels with descriptions
#[tauri::command]
pub async fn get_complexity_levels() -> Result<Vec<ComplexityLevelInfo>, String> {
    Ok(vec![
        ComplexityLevelInfo {
            level: "simple".to_string(),
            name: "Simple".to_string(),
            description: "Basic automation with 1-3 components, minimal coordination".to_string(),
            estimated_time_hours: "2-6".to_string(),
            max_agents: 3,
        },
        ComplexityLevelInfo {
            level: "moderate".to_string(),
            name: "Moderate".to_string(),
            description: "Multi-component system with some coordination and decision-making".to_string(),
            estimated_time_hours: "8-24".to_string(),
            max_agents: 5,
        },
        ComplexityLevelInfo {
            level: "complex".to_string(),
            name: "Complex".to_string(),
            description: "Advanced system with multiple specialized agents and complex workflows".to_string(),
            estimated_time_hours: "24-72".to_string(),
            max_agents: 8,
        },
        ComplexityLevelInfo {
            level: "expert".to_string(),
            name: "Expert".to_string(),
            description: "Highly sophisticated system with advanced coordination and adaptive behavior".to_string(),
            estimated_time_hours: "72-168".to_string(),
            max_agents: 12,
        },
    ])
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComplexityLevelInfo {
    pub level: String,
    pub name: String,
    pub description: String,
    pub estimated_time_hours: String,
    pub max_agents: usize,
}

/// Initialize collaborative AI state for the application
pub fn initialize_collaborative_ai_state() -> CollaborativeAIState {
    tracing::info!("Initializing Collaborative AI System based on ComfyBench research");
    CollaborativeAIState::new()
}

/// List of all collaborative AI command names for reference
pub const COLLABORATIVE_AI_COMMANDS: &[&str] = &[
    "design_collaborative_ai_system",
    "execute_collaborative_workflow",
    "get_collaborative_ai_capabilities",
    "get_collaborative_ai_statistics",
    "create_sample_collaborative_ai_request",
    "validate_collaborative_ai_request",
    "get_complexity_levels",
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_complexity_estimation() {
        let simple_request = CollaborativeAIRequest {
            description: "Simple task".to_string(),
            goals: vec!["Goal 1".to_string()],
            constraints: vec![],
            preferred_technologies: vec![],
            complexity_level: "simple".to_string(),
            timeline_hours: 4,
            performance_requirements: PerformanceRequirementsInput {
                max_response_time_ms: 10000,
                min_throughput: 1.0,
                availability_percent: 95.0,
                max_cpu_cores: 2,
                max_memory_mb: 2048,
                max_disk_space_mb: 1024,
                max_network_bandwidth: 100,
                max_concurrent_agents: 2,
            },
        };

        let complexity = estimate_request_complexity(&simple_request);
        assert!(complexity < 3.0, "Simple request should have low complexity");

        let complex_request = CollaborativeAIRequest {
            description: "Complex multi-agent system".to_string(),
            goals: vec!["Goal 1".to_string(), "Goal 2".to_string(), "Goal 3".to_string(), "Goal 4".to_string()],
            constraints: vec!["Constraint 1".to_string(), "Constraint 2".to_string()],
            preferred_technologies: vec!["AI".to_string(), "ML".to_string(), "NLP".to_string()],
            complexity_level: "expert".to_string(),
            timeline_hours: 72,
            performance_requirements: PerformanceRequirementsInput {
                max_response_time_ms: 1000,
                min_throughput: 10.0,
                availability_percent: 99.9,
                max_cpu_cores: 8,
                max_memory_mb: 16384,
                max_disk_space_mb: 8192,
                max_network_bandwidth: 10000,
                max_concurrent_agents: 12,
            },
        };

        let complex_complexity = estimate_request_complexity(&complex_request);
        assert!(complex_complexity > complexity, "Complex request should have higher complexity");
    }

    #[test]
    fn test_time_estimation() {
        let simple_request = CollaborativeAIRequest {
            description: "Simple task".to_string(),
            goals: vec!["Goal 1".to_string()],
            constraints: vec![],
            preferred_technologies: vec![],
            complexity_level: "simple".to_string(),
            timeline_hours: 4,
            performance_requirements: PerformanceRequirementsInput {
                max_response_time_ms: 10000,
                min_throughput: 1.0,
                availability_percent: 95.0,
                max_cpu_cores: 2,
                max_memory_mb: 2048,
                max_disk_space_mb: 1024,
                max_network_bandwidth: 100,
                max_concurrent_agents: 2,
            },
        };

        let time = estimate_design_time(&simple_request);
        assert!((0.5..=168.0).contains(&time), "Design time should be within reasonable bounds");
    }
}

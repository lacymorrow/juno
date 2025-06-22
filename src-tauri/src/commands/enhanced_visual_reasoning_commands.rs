use tauri::State;
use std::sync::Arc;
use tokio::sync::RwLock;
use serde::{Deserialize, Serialize};

use crate::agent::tools::enhanced_visual_reasoning::{
    VisualReasoningEngine, VisualReasoningConfig, VisualReasoningResult,
    ReasoningContext, ReasoningCapabilities, ReasoningStatistics,
    SceneUnderstanding, SceneType
};
use crate::agent::core::AgentError;

/// Global state for the Enhanced Visual Reasoning Engine
pub struct VisualReasoningState {
    pub engine: Arc<RwLock<VisualReasoningEngine>>,
}

impl VisualReasoningState {
    pub fn new() -> Self {
        let config = VisualReasoningConfig::default();
        let engine = VisualReasoningEngine::new(config);

        Self {
            engine: Arc::new(RwLock::new(engine)),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VisualAnalysisRequest {
    pub screenshot_base64: String,
    pub task_description: String,
    pub user_intent: String,
    pub interaction_context: String,
    pub application_context: String,
    pub platform_info: String,
    pub enable_multimodal_processing: bool,
    pub enable_spatial_reasoning: bool,
    pub enable_temporal_modeling: bool,
    pub enable_cross_modal_grounding: bool,
    pub enable_hierarchical_analysis: bool,
}

impl From<VisualAnalysisRequest> for ReasoningContext {
    fn from(request: VisualAnalysisRequest) -> Self {
        ReasoningContext {
            task_description: request.task_description,
            user_intent: request.user_intent,
            interaction_context: request.interaction_context,
            application_context: request.application_context,
            platform_info: request.platform_info,
        }
    }
}

/// Analyze GUI scene with enhanced visual reasoning
#[tauri::command]
pub async fn analyze_gui_scene_with_visual_reasoning(
    request: VisualAnalysisRequest,
    state: State<'_, VisualReasoningState>,
) -> Result<VisualReasoningResult, String> {
    tracing::debug!("Analyzing GUI scene with enhanced visual reasoning");

    // Decode base64 screenshot
    use base64::{Engine, engine::general_purpose};
    let screenshot_data = general_purpose::STANDARD.decode(&request.screenshot_base64)
        .map_err(|e| format!("Failed to decode screenshot: {}", e))?;

    let context = ReasoningContext::from(request.clone());

    // Update engine configuration based on request
    {
        let mut engine = state.engine.write().await;
        let mut config = VisualReasoningConfig::default();
        config.enable_multimodal_processing = request.enable_multimodal_processing;
        config.enable_spatial_reasoning = request.enable_spatial_reasoning;
        config.enable_temporal_modeling = request.enable_temporal_modeling;
        config.enable_cross_modal_grounding = request.enable_cross_modal_grounding;
        config.enable_hierarchical_analysis = request.enable_hierarchical_analysis;

        *engine = VisualReasoningEngine::new(config);
    }

    let engine = state.engine.read().await;

    engine.analyze_gui_scene(&screenshot_data, &context)
        .await
        .map_err(|e| format!("Visual reasoning analysis failed: {}", e))
}

/// Get enhanced visual reasoning capabilities
#[tauri::command]
pub async fn get_visual_reasoning_capabilities(
    state: State<'_, VisualReasoningState>,
) -> Result<ReasoningCapabilities, String> {
    tracing::debug!("Getting enhanced visual reasoning capabilities");

    let engine = state.engine.read().await;
    Ok(engine.get_reasoning_capabilities().await)
}

/// Get visual reasoning statistics
#[tauri::command]
pub async fn get_visual_reasoning_statistics(
    state: State<'_, VisualReasoningState>,
) -> Result<ReasoningStatistics, String> {
    tracing::debug!("Getting visual reasoning statistics");

    let engine = state.engine.read().await;
    Ok(engine.get_reasoning_statistics().await)
}

/// Create a sample visual analysis request for testing
#[tauri::command]
pub async fn create_sample_visual_analysis_request() -> Result<VisualAnalysisRequest, String> {
    Ok(VisualAnalysisRequest {
        screenshot_base64: "".to_string(), // Would be populated with actual screenshot
        task_description: "Analyze this form interface and identify all interactive elements".to_string(),
        user_intent: "Fill out and submit a contact form".to_string(),
        interaction_context: "User is on a website contact page".to_string(),
        application_context: "Web browser - contact form page".to_string(),
        platform_info: "macOS Safari 17.x".to_string(),
        enable_multimodal_processing: true,
        enable_spatial_reasoning: true,
        enable_temporal_modeling: true,
        enable_cross_modal_grounding: true,
        enable_hierarchical_analysis: true,
    })
}

/// Validate visual analysis request
#[tauri::command]
pub async fn validate_visual_analysis_request(
    request: VisualAnalysisRequest,
) -> Result<ValidationResult, String> {
    let mut errors = Vec::new();
    let mut warnings = Vec::new();

    // Validate screenshot data
    if request.screenshot_base64.is_empty() {
        errors.push("Screenshot data is required".to_string());
    } else {
        use base64::{Engine, engine::general_purpose};
        if let Err(e) = general_purpose::STANDARD.decode(&request.screenshot_base64) {
            errors.push(format!("Invalid screenshot base64 encoding: {}", e));
        }
    }

    // Validate task description
    if request.task_description.trim().is_empty() {
        errors.push("Task description is required".to_string());
    } else if request.task_description.len() > 1000 {
        warnings.push("Task description is very long, consider shortening".to_string());
    }

    // Validate user intent
    if request.user_intent.trim().is_empty() {
        warnings.push("User intent helps improve analysis accuracy".to_string());
    }

    // Validate analysis configuration
    if !request.enable_multimodal_processing &&
       !request.enable_spatial_reasoning &&
       !request.enable_temporal_modeling &&
       !request.enable_cross_modal_grounding &&
       !request.enable_hierarchical_analysis {
        warnings.push("At least one analysis type should be enabled for meaningful results".to_string());
    }

    let is_valid = errors.is_empty();
    let complexity_score = calculate_request_complexity(&request);
    let estimated_time_ms = estimate_processing_time(&request);

    Ok(ValidationResult {
        is_valid,
        errors,
        warnings,
        complexity_score,
        estimated_processing_time_ms: estimated_time_ms,
        recommendations: generate_optimization_recommendations(&request),
    })
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidationResult {
    pub is_valid: bool,
    pub errors: Vec<String>,
    pub warnings: Vec<String>,
    pub complexity_score: f32,
    pub estimated_processing_time_ms: u64,
    pub recommendations: Vec<String>,
}

fn calculate_request_complexity(request: &VisualAnalysisRequest) -> f32 {
    let mut complexity = 1.0;

    // Base complexity factors
    if request.enable_multimodal_processing { complexity += 1.0; }
    if request.enable_spatial_reasoning { complexity += 0.8; }
    if request.enable_temporal_modeling { complexity += 1.2; }
    if request.enable_cross_modal_grounding { complexity += 1.0; }
    if request.enable_hierarchical_analysis { complexity += 0.6; }

    // Context complexity factors
    complexity += request.task_description.len() as f32 / 1000.0;
    complexity += request.interaction_context.len() as f32 / 500.0;

    complexity.min(10.0) // Cap at 10.0
}

fn estimate_processing_time(request: &VisualAnalysisRequest) -> u64 {
    let base_time_ms = 1000; // 1 second base
    let complexity = calculate_request_complexity(request);

    (base_time_ms as f32 * complexity) as u64
}

fn generate_optimization_recommendations(request: &VisualAnalysisRequest) -> Vec<String> {
    let mut recommendations = Vec::new();

    if request.enable_multimodal_processing &&
       request.enable_spatial_reasoning &&
       request.enable_temporal_modeling &&
       request.enable_cross_modal_grounding &&
       request.enable_hierarchical_analysis {
        recommendations.push("All analysis types are enabled. Consider disabling some for faster processing if not all are needed.".to_string());
    }

    if request.task_description.len() > 500 {
        recommendations.push("Consider shortening the task description for faster processing.".to_string());
    }

    if request.interaction_context.is_empty() {
        recommendations.push("Adding interaction context can improve analysis accuracy.".to_string());
    }

    if request.application_context.is_empty() {
        recommendations.push("Adding application context helps with scene understanding.".to_string());
    }

    if recommendations.is_empty() {
        recommendations.push("Request configuration looks optimal.".to_string());
    }

    recommendations
}

/// Get available scene types with descriptions
#[tauri::command]
pub async fn get_scene_types() -> Result<Vec<SceneTypeInfo>, String> {
    Ok(vec![
        SceneTypeInfo {
            scene_type: "Desktop".to_string(),
            name: "Desktop Environment".to_string(),
            description: "Desktop interface with windows, icons, and system elements".to_string(),
            typical_elements: vec!["Windows".to_string(), "Icons".to_string(), "Taskbar".to_string(), "Menus".to_string()],
        },
        SceneTypeInfo {
            scene_type: "WebPage".to_string(),
            name: "Web Page".to_string(),
            description: "Web browser page with HTML elements and content".to_string(),
            typical_elements: vec!["Links".to_string(), "Buttons".to_string(), "Forms".to_string(), "Images".to_string()],
        },
        SceneTypeInfo {
            scene_type: "Form".to_string(),
            name: "Input Form".to_string(),
            description: "Data entry form with input fields and controls".to_string(),
            typical_elements: vec!["Text Fields".to_string(), "Checkboxes".to_string(), "Dropdowns".to_string(), "Submit Button".to_string()],
        },
    ])
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SceneTypeInfo {
    pub scene_type: String,
    pub name: String,
    pub description: String,
    pub typical_elements: Vec<String>,
}

/// Test visual reasoning engine with sample data
#[tauri::command]
pub async fn test_visual_reasoning_engine(
    state: State<'_, VisualReasoningState>,
) -> Result<TestResult, String> {
    tracing::debug!("Testing visual reasoning engine");

    let sample_request = create_sample_visual_analysis_request().await?;

    // Create minimal test screenshot data (1x1 pixel PNG)
    let test_screenshot = vec![
        0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A, 0x00, 0x00, 0x00, 0x0D,
        0x49, 0x48, 0x44, 0x52, 0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x01,
        0x08, 0x06, 0x00, 0x00, 0x00, 0x1F, 0x15, 0xC4, 0x89, 0x00, 0x00, 0x00,
        0x0A, 0x49, 0x44, 0x41, 0x54, 0x78, 0x9C, 0x63, 0x00, 0x01, 0x00, 0x00,
        0x05, 0x00, 0x01, 0x0D, 0x0A, 0x2D, 0xB4, 0x00, 0x00, 0x00, 0x00, 0x49,
        0x45, 0x4E, 0x44, 0xAE, 0x42, 0x60, 0x82
    ];

    let context = ReasoningContext::from(sample_request);

    let start_time = std::time::Instant::now();

    let engine = state.engine.read().await;
    let result = engine.analyze_gui_scene(&test_screenshot, &context).await;

    let processing_time = start_time.elapsed();

    match result {
        Ok(analysis_result) => Ok(TestResult {
            success: true,
            processing_time_ms: processing_time.as_millis() as u64,
            confidence_score: analysis_result.reasoning_confidence,
            elements_detected: analysis_result.scene_understanding.primary_elements.len(),
            spatial_relationships: analysis_result.spatial_relationships.len(),
            error_message: None,
        }),
        Err(e) => Ok(TestResult {
            success: false,
            processing_time_ms: processing_time.as_millis() as u64,
            confidence_score: 0.0,
            elements_detected: 0,
            spatial_relationships: 0,
            error_message: Some(format!("Test failed: {}", e)),
        }),
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TestResult {
    pub success: bool,
    pub processing_time_ms: u64,
    pub confidence_score: f32,
    pub elements_detected: usize,
    pub spatial_relationships: usize,
    pub error_message: Option<String>,
}

/// Initialize enhanced visual reasoning state for the application
pub fn initialize_visual_reasoning_state() -> VisualReasoningState {
    tracing::info!("Initializing Enhanced Visual Reasoning System based on CVPR 2025 research");
    VisualReasoningState::new()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_complexity_calculation() {
        let simple_request = VisualAnalysisRequest {
            screenshot_base64: "".to_string(),
            task_description: "Simple task".to_string(),
            user_intent: "Click button".to_string(),
            interaction_context: "Web page".to_string(),
            application_context: "Browser".to_string(),
            platform_info: "macOS".to_string(),
            enable_multimodal_processing: true,
            enable_spatial_reasoning: false,
            enable_temporal_modeling: false,
            enable_cross_modal_grounding: false,
            enable_hierarchical_analysis: false,
        };

        let complexity = calculate_request_complexity(&simple_request);
        assert!(complexity >= 1.0 && complexity <= 10.0, "Complexity should be within valid range");
    }

    #[test]
    fn test_time_estimation() {
        let request = VisualAnalysisRequest {
            screenshot_base64: "".to_string(),
            task_description: "Analyze interface".to_string(),
            user_intent: "Navigate".to_string(),
            interaction_context: "App".to_string(),
            application_context: "Desktop".to_string(),
            platform_info: "macOS".to_string(),
            enable_multimodal_processing: true,
            enable_spatial_reasoning: true,
            enable_temporal_modeling: false,
            enable_cross_modal_grounding: false,
            enable_hierarchical_analysis: false,
        };

        let time = estimate_processing_time(&request);
        assert!(time >= 1000, "Processing time should be at least 1 second");
    }

    #[tokio::test]
    async fn test_sample_request_creation() {
        let sample = create_sample_visual_analysis_request().await.unwrap();
        assert!(!sample.task_description.is_empty());
        assert!(sample.enable_multimodal_processing);
    }
}

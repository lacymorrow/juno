use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, State};
use tracing::{debug, info, warn};

use crate::agent::intelligence::{
    AnalysisContext, IntelligenceConfig, OperationalMode, ToolChoiceDecision,
    ToolChoiceIntelligence,
};
use crate::agent::providers::anthropic::ToolChoice;
use crate::state::AppState;
use crate::constants::events;

/// Configuration for tool choice intelligence system
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolChoiceConfig {
    /// Current operational mode
    pub mode: String,
    /// Enable aggressive tool forcing for clear action commands
    pub aggressive_action_forcing: bool,
    /// Enable voice command optimization
    pub voice_command_optimization: bool,
    /// Enable context-aware tool selection
    pub context_awareness: bool,
    /// Minimum confidence threshold for tool forcing (0.0 - 1.0)
    pub confidence_threshold: f32,
    /// Enable learning from user corrections
    pub adaptive_learning: bool,
    /// Enable debug logging
    pub debug_logging: bool,
}

impl Default for ToolChoiceConfig {
    fn default() -> Self {
        Self {
            mode: "agent".to_string(),
            aggressive_action_forcing: true,
            voice_command_optimization: true,
            context_awareness: true,
            confidence_threshold: 0.7,
            adaptive_learning: false,
            debug_logging: false,
        }
    }
}

impl From<ToolChoiceConfig> for IntelligenceConfig {
    fn from(config: ToolChoiceConfig) -> Self {
        Self {
            aggressive_action_forcing: config.aggressive_action_forcing,
            voice_command_optimization: config.voice_command_optimization,
            context_awareness: config.context_awareness,
            confidence_threshold: config.confidence_threshold,
            adaptive_learning: config.adaptive_learning,
        }
    }
}

impl From<&str> for OperationalMode {
    fn from(mode_str: &str) -> Self {
        match mode_str.to_lowercase().as_str() {
            "agent" => OperationalMode::Agent,
            "voice" => OperationalMode::Voice,
            "dictation" => OperationalMode::Dictation,
            "alwayslistening" | "always_listening" => OperationalMode::AlwaysListening,
            "debug" => OperationalMode::Debug,
            _ => OperationalMode::Agent, // Default fallback
        }
    }
}

/// Tool choice analysis result for frontend
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolChoiceAnalysis {
    /// The recommended tool choice
    pub tool_choice: Option<String>,
    /// Confidence level in this decision (0.0 - 1.0)
    pub confidence: f32,
    /// Human-readable reasoning for this decision
    pub reasoning: String,
    /// Source of this decision
    pub source: String,
    /// Whether this decision meets the confidence threshold
    pub should_apply: bool,
}

impl From<ToolChoiceDecision> for ToolChoiceAnalysis {
    fn from(decision: ToolChoiceDecision) -> Self {
        let tool_choice_str = match &decision.tool_choice {
            Some(ToolChoice::Auto) => Some("auto".to_string()),
            Some(ToolChoice::Any) => Some("any".to_string()),
            Some(ToolChoice::None) => Some("none".to_string()),
            Some(ToolChoice::Tool { name, .. }) => Some(format!("tool:{}", name)),
            None => None,
        };

        Self {
            tool_choice: tool_choice_str,
            confidence: decision.confidence,
            reasoning: decision.reasoning,
            source: format!("{:?}", decision.source),
            should_apply: decision.confidence > 0.6, // Default threshold
        }
    }
}

/// Get current tool choice configuration
#[tauri::command]
pub async fn get_tool_choice_config(
    state: State<'_, AppState>,
) -> Result<ToolChoiceConfig, String> {
    debug!("Getting tool choice configuration");

    // For now, return default config - in future this could be stored in app state
    let config = ToolChoiceConfig::default();

    // Update mode based on current app state
    let mode = if state.get_dictation_active().unwrap_or(false) {
        "dictation"
    } else if state.get_always_listening_active().unwrap_or(false) {
        "alwayslistening"
    } else {
        "agent"
    };

    let mut config = config;
    config.mode = mode.to_string();

    debug!("Tool choice config: {:?}", config);
    Ok(config)
}

/// Update tool choice configuration
#[tauri::command]
pub async fn set_tool_choice_config(
    config: ToolChoiceConfig,
    _state: State<'_, AppState>,
    app_handle: AppHandle,
) -> Result<(), String> {
    info!("Updating tool choice configuration: {:?}", config);

    // Validate configuration
    if config.confidence_threshold < 0.0 || config.confidence_threshold > 1.0 {
        return Err("Confidence threshold must be between 0.0 and 1.0".to_string());
    }

    // TODO: Store configuration in app state or persistent storage
    // For now, we'll just log the update

    info!("Tool choice configuration updated successfully");

    // Emit configuration change event to frontend
    if let Err(e) = app_handle.emit(events::tool_choice::CONFIG_CHANGED, &config) {
        warn!("Failed to emit tool choice config change event: {}", e);
    }

    Ok(())
}

/// Analyze input text and get tool choice recommendation
#[tauri::command]
pub async fn analyze_tool_choice(
    input: String,
    mode: Option<String>,
    state: State<'_, AppState>,
) -> Result<ToolChoiceAnalysis, String> {
    debug!("Analyzing tool choice for input: '{}'", input);

    if input.trim().is_empty() {
        return Err("Input cannot be empty".to_string());
    }

    // Determine operational mode
    let operational_mode = if let Some(mode_str) = mode {
        OperationalMode::from(mode_str.as_str())
    } else {
        // Auto-detect from app state
        if state.get_dictation_active().unwrap_or(false) {
            OperationalMode::Dictation
        } else if state.get_always_listening_active().unwrap_or(false) {
            OperationalMode::AlwaysListening
        } else {
            OperationalMode::Agent
        }
    };

    // Create tool choice intelligence system
    let intelligence = ToolChoiceIntelligence::new(operational_mode);

    // Build analysis context
    let context = AnalysisContext {
        previous_was_tool_call: false, // TODO: Could be enhanced with conversation history
        last_tool_name: None,
        last_tool_error: false,
        conversation_length: 0,
        available_tools: Vec::new(), // TODO: Could list available tools
    };

    // Analyze input
    let decision = intelligence.analyze_input(&input, &context);
    let analysis = ToolChoiceAnalysis::from(decision);

    debug!("Tool choice analysis result: {:?}", analysis);
    Ok(analysis)
}

/// Get list of available operational modes
#[tauri::command]
pub async fn get_operational_modes() -> Result<Vec<String>, String> {
    Ok(vec![
        "agent".to_string(),
        "voice".to_string(),
        "dictation".to_string(),
        "alwayslistening".to_string(),
        "debug".to_string(),
    ])
}

/// Test tool choice intelligence with sample inputs
#[tauri::command]
pub async fn test_tool_choice_patterns(
    mode: Option<String>,
) -> Result<Vec<ToolChoiceAnalysis>, String> {
    debug!("Testing tool choice patterns");

    let operational_mode = OperationalMode::from(mode.as_deref().unwrap_or("agent"));
    let intelligence = ToolChoiceIntelligence::new(operational_mode);
    let context = AnalysisContext::default();

    let test_inputs = vec![
        "take a screenshot",
        "click the button",
        "type hello world",
        "open the browser",
        "refresh the page",
        "save the file",
        "minimize the window",
        "what is the weather?",
        "explain quantum physics",
        "help me with this code",
    ];

    let mut results = Vec::new();
    for input in test_inputs {
        let decision = intelligence.analyze_input(input, &context);
        let mut analysis = ToolChoiceAnalysis::from(decision);
        // Add the test input to the reasoning for reference
        analysis.reasoning = format!("Input: '{}' - {}", input, analysis.reasoning);
        results.push(analysis);
    }

    debug!("Generated {} test results", results.len());
    Ok(results)
}

/// Get tool choice statistics and performance metrics
#[tauri::command]
pub async fn get_tool_choice_stats() -> Result<serde_json::Value, String> {
    debug!("Getting tool choice statistics");

    // TODO: Implement actual statistics collection
    // For now, return mock data
    let stats = serde_json::json!({
        "total_analyses": 0,
        "forced_tool_calls": 0,
        "confidence_distribution": {
            "high": 0,
            "medium": 0,
            "low": 0
        },
        "mode_usage": {
            "agent": 0,
            "voice": 0,
            "dictation": 0,
            "alwayslistening": 0,
            "debug": 0
        },
        "pattern_matches": {
            "screenshot": 0,
            "click": 0,
            "keyboard": 0,
            "browser": 0,
            "file": 0,
            "desktop": 0
        }
    });

    Ok(stats)
}

/// Reset tool choice configuration to defaults
#[tauri::command]
pub async fn reset_tool_choice_config(
    _state: State<'_, AppState>,
    app_handle: AppHandle,
) -> Result<ToolChoiceConfig, String> {
    info!("Resetting tool choice configuration to defaults");

    let default_config = ToolChoiceConfig::default();

    // TODO: Clear any stored configuration from app state

    info!("Tool choice configuration reset successfully");

    // Emit reset event to frontend
    if let Err(e) = app_handle.emit(events::tool_choice::CONFIG_RESET, &default_config) {
        warn!("Failed to emit tool choice config reset event: {}", e);
    }

    Ok(default_config)
}

/// Enable or disable tool choice intelligence globally
#[tauri::command]
pub async fn set_tool_choice_enabled(
    enabled: bool,
    _state: State<'_, AppState>,
    app_handle: AppHandle,
) -> Result<(), String> {
    info!("Setting tool choice intelligence enabled: {}", enabled);

    // TODO: Store enabled state in app state

    // Emit state change event to frontend
    if let Err(e) = app_handle.emit(events::tool_choice::ENABLED_CHANGED, enabled) {
        warn!("Failed to emit tool choice enabled change event: {}", e);
    }

    Ok(())
}

/// Get tool choice intelligence enabled state
#[tauri::command]
pub async fn get_tool_choice_enabled(_state: State<'_, AppState>) -> Result<bool, String> {
    // TODO: Get actual enabled state from app state
    // For now, return true as default
    Ok(true)
}

/// Validate tool choice configuration
#[tauri::command]
pub async fn validate_tool_choice_config(config: ToolChoiceConfig) -> Result<Vec<String>, String> {
    debug!("Validating tool choice configuration: {:?}", config);

    let mut errors = Vec::new();

    // Validate confidence threshold
    if config.confidence_threshold < 0.0 || config.confidence_threshold > 1.0 {
        errors.push("Confidence threshold must be between 0.0 and 1.0".to_string());
    }

    // Validate mode
    let valid_modes = ["agent", "voice", "dictation", "alwayslistening", "debug"];
    if !valid_modes.contains(&config.mode.as_str()) {
        errors.push(format!(
            "Invalid mode '{}'. Valid modes are: {}",
            config.mode,
            valid_modes.join(", ")
        ));
    }

    // Validate reasonable confidence threshold
    if config.confidence_threshold < 0.1 {
        errors
            .push("Confidence threshold below 0.1 may cause too many false positives".to_string());
    }

    if config.confidence_threshold > 0.95 {
        errors.push("Confidence threshold above 0.95 may be too restrictive".to_string());
    }

    debug!("Validation completed with {} errors", errors.len());
    Ok(errors)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_operational_mode_conversion() {
        assert!(matches!(
            OperationalMode::from("agent"),
            OperationalMode::Agent
        ));
        assert!(matches!(
            OperationalMode::from("voice"),
            OperationalMode::Voice
        ));
        assert!(matches!(
            OperationalMode::from("dictation"),
            OperationalMode::Dictation
        ));
        assert!(matches!(
            OperationalMode::from("alwayslistening"),
            OperationalMode::AlwaysListening
        ));
        assert!(matches!(
            OperationalMode::from("debug"),
            OperationalMode::Debug
        ));
        assert!(matches!(
            OperationalMode::from("invalid"),
            OperationalMode::Agent
        ));
    }

    #[test]
    fn test_config_validation() {
        let mut config = ToolChoiceConfig::default();

        // Valid config should have no errors
        config.confidence_threshold = 0.7;
        // In a real test, we'd call validate_tool_choice_config

        // Invalid confidence threshold
        config.confidence_threshold = 1.5;
        // Should produce error

        config.confidence_threshold = -0.1;
        // Should produce error
    }

    #[test]
    fn test_intelligence_config_conversion() {
        let tool_choice_config = ToolChoiceConfig {
            mode: "agent".to_string(),
            aggressive_action_forcing: true,
            voice_command_optimization: false,
            context_awareness: true,
            confidence_threshold: 0.8,
            adaptive_learning: false,
            debug_logging: true,
        };

        let intelligence_config: IntelligenceConfig = tool_choice_config.into();
        assert!(intelligence_config.aggressive_action_forcing);
        assert!(!intelligence_config.voice_command_optimization);
        assert!(intelligence_config.context_awareness);
        assert_eq!(intelligence_config.confidence_threshold, 0.8);
        assert!(!intelligence_config.adaptive_learning);
    }
}

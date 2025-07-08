//! Tool call engine abstraction for supporting different LLM providers
//! 
//! This module implements TARS's strategy pattern for tool execution while
//! maintaining Juno's reliability and error recovery patterns.

use async_trait::async_trait;
use serde_json::Value;

use crate::agent::core::{ToolDefinition, ToolCall, ToolResult};

pub mod native;
pub mod prompt_engineering;
pub mod structured_outputs;

pub use native::NativeToolCallEngine;
pub use prompt_engineering::PromptEngineeringEngine;
pub use structured_outputs::StructuredOutputsEngine;

/// Tool call execution strategies for different LLM providers
#[derive(Debug, Clone, PartialEq)]
pub enum ToolCallEngineType {
    /// OpenAI-style function calling with native tool support
    Native,
    /// JSON-based tool execution via engineered prompts
    PromptEngineering,
    /// Anthropic-style structured outputs
    StructuredOutputs,
}

/// Request context for tool call preparation
#[derive(Debug)]
pub struct ToolCallContext {
    pub model: String,
    pub provider: String,
    pub messages: Vec<ChatMessage>,
    pub system_prompt: Option<String>,
    pub max_tokens: Option<u32>,
    pub temperature: Option<f32>,
}

/// Chat message format for tool call preparation (compatible with Juno's Message)
#[derive(Debug, Clone)]
pub struct ChatMessage {
    pub role: String,
    pub content: String,
    pub tool_calls: Option<Vec<ToolCall>>,
    pub tool_call_id: Option<String>,
}

/// Tool call engine trait defining the interface for different strategies
#[async_trait]
pub trait ToolCallEngine: Send + Sync {
    /// Prepare tools for LLM request based on provider requirements
    async fn prepare_tools_for_llm(
        &self,
        tools: &[ToolDefinition],
        context: &ToolCallContext,
    ) -> Result<Value, String>;
    
    /// Extract tool calls from LLM response
    async fn extract_tool_calls(
        &self,
        response: &str,
        context: &ToolCallContext,
    ) -> Result<Vec<ToolCall>, String>;
    
    /// Format tool results for next LLM request
    async fn format_tool_results(
        &self,
        tool_results: &[ToolResult],
        context: &ToolCallContext,
    ) -> Result<Vec<ChatMessage>, String>;
    
    /// Get the engine type identifier
    fn get_engine_type(&self) -> ToolCallEngineType;
    
    /// Check if this engine supports the given provider
    fn supports_provider(&self, provider: &str) -> bool;
    
    /// Get provider-specific configuration
    fn get_provider_config(&self, _provider: &str) -> Option<ProviderConfig> {
        None
    }
}

/// Provider-specific configuration for tool engines
#[derive(Debug, Clone)]
pub struct ProviderConfig {
    pub max_tools_per_request: Option<usize>,
    pub supports_parallel_calls: bool,
    pub requires_system_prompt: bool,
    pub tool_choice_support: bool,
}

/// Engine selection logic based on provider
pub fn get_engine_for_provider(provider: &str) -> Box<dyn ToolCallEngine> {
    match provider.to_lowercase().as_str() {
        "anthropic" | "claude" => Box::new(StructuredOutputsEngine::new()),
        "openai" | "azure-openai" | "gpt" => Box::new(NativeToolCallEngine::new()),
        "deepseek" | "qwen" | "llama" => Box::new(PromptEngineeringEngine::new()),
        _ => {
            tracing::warn!("Unknown provider '{}', using prompt engineering engine", provider);
            Box::new(PromptEngineeringEngine::new())
        }
    }
}

/// Get all available engines for testing or selection
pub fn get_all_engines() -> Vec<Box<dyn ToolCallEngine>> {
    vec![
        Box::new(NativeToolCallEngine::new()),
        Box::new(PromptEngineeringEngine::new()),
        Box::new(StructuredOutputsEngine::new()),
    ]
}

/// Engine capability testing
pub struct EngineCapabilities;

impl EngineCapabilities {
    pub fn test_engine_compatibility(
        engine: &dyn ToolCallEngine,
        provider: &str,
        tools: &[ToolDefinition],
    ) -> Result<CompatibilityReport, String> {
        let supports_provider = engine.supports_provider(provider);
        let config = engine.get_provider_config(provider);
        
        let tool_count_ok = if let Some(config) = &config {
            if let Some(max_tools) = config.max_tools_per_request {
                tools.len() <= max_tools
            } else {
                true
            }
        } else {
            true
        };
        
        Ok(CompatibilityReport {
            engine_type: engine.get_engine_type(),
            provider: provider.to_string(),
            supports_provider,
            tool_count_compatible: tool_count_ok,
            parallel_execution_supported: config
                .as_ref()
                .map(|c| c.supports_parallel_calls)
                .unwrap_or(false),
            recommendations: vec![],
        })
    }
}

#[derive(Debug)]
pub struct CompatibilityReport {
    pub engine_type: ToolCallEngineType,
    pub provider: String,
    pub supports_provider: bool,
    pub tool_count_compatible: bool,
    pub parallel_execution_supported: bool,
    pub recommendations: Vec<String>,
}

/// Utility functions for tool call processing
pub mod utils {
    use super::*;
    
    /// Generate a unique tool call ID
    pub fn generate_tool_call_id() -> String {
        format!("call_{}", uuid::Uuid::new_v4().to_string()[..8].to_string())
    }
    
    /// Validate tool call format
    pub fn validate_tool_call(tool_call: &ToolCall, available_tools: &[ToolDefinition]) -> Result<(), String> {
        // Check if tool exists
        if !available_tools.iter().any(|t| t.name == tool_call.name) {
            return Err(format!("Tool '{}' not found in available tools", tool_call.name));
        }
        
        // Validate ID format
        if tool_call.id.is_empty() {
            return Err("Tool call ID cannot be empty".to_string());
        }
        
        // Validate arguments are valid JSON (using 'input' field from Juno's structure)
        serde_json::from_value::<Value>(tool_call.input.clone())
            .map_err(|e| format!("Invalid tool call input: {}", e))?;
        
        Ok(())
    }
    
    /// Sanitize tool call for logging
    pub fn sanitize_tool_call_for_logging(tool_call: &ToolCall) -> Value {
        serde_json::json!({
            "id": tool_call.id,
            "name": tool_call.name,
            "input_type": tool_call.input.get("type").unwrap_or(&Value::String("unknown".to_string())),
            "input_size": tool_call.input.to_string().len()
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_engine_selection() {
        let anthropic_engine = get_engine_for_provider("anthropic");
        assert_eq!(anthropic_engine.get_engine_type(), ToolCallEngineType::StructuredOutputs);
        
        let openai_engine = get_engine_for_provider("openai");
        assert_eq!(openai_engine.get_engine_type(), ToolCallEngineType::Native);
        
        let unknown_engine = get_engine_for_provider("unknown_provider");
        assert_eq!(unknown_engine.get_engine_type(), ToolCallEngineType::PromptEngineering);
    }
    
    #[test]
    fn test_tool_call_validation() {
        let tools = vec![
            ToolDefinition {
                name: "test_tool".to_string(),
                description: "Test tool".to_string(),
                input_schema: serde_json::json!({
                    "type": "object",
                    "properties": {
                        "param": {"type": "string"}
                    }
                }),
                api_type: None,
                beta_flag: None,
            }
        ];
        
        let valid_call = ToolCall {
            id: "call_123".to_string(),
            name: "test_tool".to_string(),
            input: serde_json::json!({"param": "value"}),
        };
        
        assert!(utils::validate_tool_call(&valid_call, &tools).is_ok());
        
        let invalid_call = ToolCall {
            id: "call_456".to_string(),
            name: "nonexistent_tool".to_string(),
            input: serde_json::json!({}),
        };
        
        assert!(utils::validate_tool_call(&invalid_call, &tools).is_err());
    }
}
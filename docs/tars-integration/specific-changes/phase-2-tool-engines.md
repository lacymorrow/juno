# Phase 2: Tool Engine Implementation

## Overview

This document provides exact code changes for implementing multiple tool call strategies (engines) in Juno, based on TARS's approach while maintaining Juno's production-ready reliability patterns.

## Files to Create

### 1. Tool Engine Abstraction

**File**: `src-tauri/src/agent/tools/engines/mod.rs`

```rust
//! Tool call engine abstraction for supporting different LLM providers
//! 
//! This module implements TARS's strategy pattern for tool execution while
//! maintaining Juno's reliability and error recovery patterns.

use async_trait::async_trait;
use serde_json::Value;
use std::collections::HashMap;

use crate::agent::core::{ToolDefinition, ToolCall, ToolResult, AgentError};

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

/// Chat message format for tool call preparation
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
    fn get_provider_config(&self, provider: &str) -> Option<ProviderConfig> {
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
        
        // Validate arguments are valid JSON
        serde_json::from_value::<Value>(tool_call.arguments.clone())
            .map_err(|e| format!("Invalid tool call arguments: {}", e))?;
        
        Ok(())
    }
    
    /// Sanitize tool call for logging
    pub fn sanitize_tool_call_for_logging(tool_call: &ToolCall) -> Value {
        serde_json::json!({
            "id": tool_call.id,
            "name": tool_call.name,
            "args_type": tool_call.arguments.get("type").unwrap_or(&Value::String("unknown".to_string())),
            "args_size": tool_call.arguments.to_string().len()
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
            arguments: serde_json::json!({"param": "value"}),
        };
        
        assert!(utils::validate_tool_call(&valid_call, &tools).is_ok());
        
        let invalid_call = ToolCall {
            id: "call_456".to_string(),
            name: "nonexistent_tool".to_string(),
            arguments: serde_json::json!({}),
        };
        
        assert!(utils::validate_tool_call(&invalid_call, &tools).is_err());
    }
}
```

### 2. Native Tool Call Engine (OpenAI-style)

**File**: `src-tauri/src/agent/tools/engines/native.rs`

```rust
//! Native tool call engine for OpenAI-style function calling
//! 
//! This engine handles providers that support native function calling
//! like OpenAI, Azure OpenAI, and compatible APIs.

use async_trait::async_trait;
use serde_json::{json, Value};
use tracing::{debug, error, warn};

use super::{ToolCallEngine, ToolCallEngineType, ToolCallContext, ChatMessage, ProviderConfig};
use crate::agent::core::{ToolDefinition, ToolCall, ToolResult};

/// Native tool call engine for OpenAI-compatible APIs
pub struct NativeToolCallEngine {
    config: NativeEngineConfig,
}

#[derive(Debug, Clone)]
pub struct NativeEngineConfig {
    pub max_tools_per_request: usize,
    pub supports_parallel_calls: bool,
    pub strict_schema_validation: bool,
}

impl Default for NativeEngineConfig {
    fn default() -> Self {
        Self {
            max_tools_per_request: 100, // OpenAI supports many tools
            supports_parallel_calls: true,
            strict_schema_validation: true,
        }
    }
}

impl NativeToolCallEngine {
    pub fn new() -> Self {
        Self {
            config: NativeEngineConfig::default(),
        }
    }
    
    pub fn with_config(config: NativeEngineConfig) -> Self {
        Self { config }
    }
    
    /// Convert Juno tool definition to OpenAI function format
    fn tool_definition_to_openai_function(&self, tool: &ToolDefinition) -> Result<Value, String> {
        // Validate the input schema
        if !tool.input_schema.is_object() {
            return Err(format!("Tool '{}' has invalid schema - must be an object", tool.name));
        }
        
        let function = json!({
            "name": tool.name,
            "description": tool.description,
            "parameters": tool.input_schema,
            "strict": self.config.strict_schema_validation
        });
        
        Ok(json!({
            "type": "function",
            "function": function
        }))
    }
    
    /// Parse OpenAI tool call format to Juno ToolCall
    fn parse_openai_tool_call(&self, tool_call_json: &Value) -> Result<ToolCall, String> {
        let function = tool_call_json.get("function")
            .ok_or("Missing 'function' field in tool call")?;
            
        let id = tool_call_json.get("id")
            .and_then(|v| v.as_str())
            .ok_or("Missing or invalid 'id' field in tool call")?;
            
        let name = function.get("name")
            .and_then(|v| v.as_str())
            .ok_or("Missing or invalid 'name' field in function")?;
            
        let arguments_str = function.get("arguments")
            .and_then(|v| v.as_str())
            .ok_or("Missing or invalid 'arguments' field in function")?;
            
        // Parse arguments JSON string
        let arguments: Value = serde_json::from_str(arguments_str)
            .map_err(|e| format!("Invalid JSON in tool call arguments: {}", e))?;
            
        Ok(ToolCall {
            id: id.to_string(),
            name: name.to_string(),
            arguments,
        })
    }
}

#[async_trait]
impl ToolCallEngine for NativeToolCallEngine {
    async fn prepare_tools_for_llm(
        &self,
        tools: &[ToolDefinition],
        context: &ToolCallContext,
    ) -> Result<Value, String> {
        debug!("Preparing {} tools for native engine (provider: {})", tools.len(), context.provider);
        
        if tools.len() > self.config.max_tools_per_request {
            warn!("Tool count ({}) exceeds maximum ({}), truncating", 
                  tools.len(), self.config.max_tools_per_request);
        }
        
        let tools_to_use = &tools[..tools.len().min(self.config.max_tools_per_request)];
        
        let mut openai_tools = Vec::new();
        for tool in tools_to_use {
            let openai_function = self.tool_definition_to_openai_function(tool)?;
            openai_tools.push(openai_function);
        }
        
        let result = json!({
            "tools": openai_tools,
            "tool_choice": "auto", // Let the model decide when to use tools
            "parallel_tool_calls": self.config.supports_parallel_calls
        });
        
        debug!("Prepared {} tools for OpenAI-style API", openai_tools.len());
        Ok(result)
    }
    
    async fn extract_tool_calls(
        &self,
        response: &str,
        _context: &ToolCallContext,
    ) -> Result<Vec<ToolCall>, String> {
        debug!("Extracting tool calls from native API response");
        
        // Parse the response JSON
        let response_json: Value = serde_json::from_str(response)
            .map_err(|e| format!("Failed to parse API response: {}", e))?;
            
        // Extract tool calls from the response
        let tool_calls = response_json
            .get("choices")
            .and_then(|choices| choices.get(0))
            .and_then(|choice| choice.get("message"))
            .and_then(|message| message.get("tool_calls"))
            .and_then(|tc| tc.as_array())
            .unwrap_or(&vec![]);
            
        let mut parsed_calls = Vec::new();
        for tool_call_json in tool_calls {
            match self.parse_openai_tool_call(tool_call_json) {
                Ok(tool_call) => {
                    debug!("Parsed tool call: {} ({})", tool_call.name, tool_call.id);
                    parsed_calls.push(tool_call);
                }
                Err(e) => {
                    error!("Failed to parse tool call: {}", e);
                    return Err(format!("Tool call parsing error: {}", e));
                }
            }
        }
        
        debug!("Extracted {} tool calls from response", parsed_calls.len());
        Ok(parsed_calls)
    }
    
    async fn format_tool_results(
        &self,
        tool_results: &[ToolResult],
        _context: &ToolCallContext,
    ) -> Result<Vec<ChatMessage>, String> {
        debug!("Formatting {} tool results for native engine", tool_results.len());
        
        let mut messages = Vec::new();
        
        for result in tool_results {
            let content = if result.success {
                serde_json::to_string(&result.output)
                    .map_err(|e| format!("Failed to serialize tool result: {}", e))?
            } else {
                format!("Error: {}", result.error_message.as_deref().unwrap_or("Unknown error"))
            };
            
            let message = ChatMessage {
                role: "tool".to_string(),
                content,
                tool_calls: None,
                tool_call_id: Some(result.tool_call_id.clone()),
            };
            
            messages.push(message);
        }
        
        debug!("Formatted {} tool result messages", messages.len());
        Ok(messages)
    }
    
    fn get_engine_type(&self) -> ToolCallEngineType {
        ToolCallEngineType::Native
    }
    
    fn supports_provider(&self, provider: &str) -> bool {
        matches!(provider.to_lowercase().as_str(), 
                 "openai" | "azure-openai" | "gpt" | "azure" | "openai-compatible")
    }
    
    fn get_provider_config(&self, provider: &str) -> Option<ProviderConfig> {
        if self.supports_provider(provider) {
            Some(ProviderConfig {
                max_tools_per_request: Some(self.config.max_tools_per_request),
                supports_parallel_calls: self.config.supports_parallel_calls,
                requires_system_prompt: false,
                tool_choice_support: true,
            })
        } else {
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    
    #[tokio::test]
    async fn test_tool_preparation() {
        let engine = NativeToolCallEngine::new();
        
        let tool = ToolDefinition {
            name: "test_function".to_string(),
            description: "A test function".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "param1": {"type": "string"},
                    "param2": {"type": "number"}
                },
                "required": ["param1"]
            }),
            api_type: None,
            beta_flag: None,
        };
        
        let context = ToolCallContext {
            model: "gpt-4".to_string(),
            provider: "openai".to_string(),
            messages: vec![],
            system_prompt: None,
            max_tokens: None,
            temperature: None,
        };
        
        let result = engine.prepare_tools_for_llm(&[tool], &context).await;
        assert!(result.is_ok());
        
        let prepared = result.unwrap();
        assert!(prepared.get("tools").is_some());
        assert!(prepared.get("tools").unwrap().as_array().unwrap().len() == 1);
    }
    
    #[tokio::test]
    async fn test_tool_call_extraction() {
        let engine = NativeToolCallEngine::new();
        
        let response = json!({
            "choices": [{
                "message": {
                    "tool_calls": [{
                        "id": "call_123",
                        "type": "function",
                        "function": {
                            "name": "test_function",
                            "arguments": "{\"param1\": \"value1\", \"param2\": 42}"
                        }
                    }]
                }
            }]
        });
        
        let context = ToolCallContext {
            model: "gpt-4".to_string(),
            provider: "openai".to_string(),
            messages: vec![],
            system_prompt: None,
            max_tokens: None,
            temperature: None,
        };
        
        let result = engine.extract_tool_calls(&response.to_string(), &context).await;
        assert!(result.is_ok());
        
        let tool_calls = result.unwrap();
        assert_eq!(tool_calls.len(), 1);
        assert_eq!(tool_calls[0].name, "test_function");
        assert_eq!(tool_calls[0].id, "call_123");
    }
}
```

### 3. Prompt Engineering Engine

**File**: `src-tauri/src/agent/tools/engines/prompt_engineering.rs`

```rust
//! Prompt engineering tool call engine for models without native tool support
//! 
//! This engine uses carefully crafted prompts to enable tool calling
//! for models that don't have built-in function calling capabilities.

use async_trait::async_trait;
use serde_json::{json, Value};
use tracing::{debug, error, warn};
use regex::Regex;

use super::{ToolCallEngine, ToolCallEngineType, ToolCallContext, ChatMessage, ProviderConfig};
use crate::agent::core::{ToolDefinition, ToolCall, ToolResult};

/// Prompt engineering tool call engine
pub struct PromptEngineeringEngine {
    config: PromptEngineConfig,
}

#[derive(Debug, Clone)]
pub struct PromptEngineConfig {
    pub max_tools_per_request: usize,
    pub use_xml_format: bool,
    pub include_examples: bool,
    pub strict_json_parsing: bool,
}

impl Default for PromptEngineConfig {
    fn default() -> Self {
        Self {
            max_tools_per_request: 20, // More conservative for prompt-based
            use_xml_format: true,      // XML is often more reliable
            include_examples: true,    // Examples help with consistency
            strict_json_parsing: false, // Be lenient with prompt-generated JSON
        }
    }
}

impl PromptEngineeringEngine {
    pub fn new() -> Self {
        Self {
            config: PromptEngineConfig::default(),
        }
    }
    
    pub fn with_config(config: PromptEngineConfig) -> Self {
        Self { config }
    }
    
    /// Generate tool calling instructions for the system prompt
    fn generate_tool_instructions(&self, tools: &[ToolDefinition]) -> String {
        let format_description = if self.config.use_xml_format {
            "Use XML format for tool calls"
        } else {
            "Use JSON format for tool calls"
        };
        
        let mut instructions = format!(
            "You have access to the following tools. {}:\n\n",
            format_description
        );
        
        // List available tools
        for tool in tools {
            instructions.push_str(&format!(
                "## {}\n{}\n\nParameters:\n```json\n{}\n```\n\n",
                tool.name,
                tool.description,
                serde_json::to_string_pretty(&tool.input_schema).unwrap_or_default()
            ));
        }
        
        // Add calling format instructions
        if self.config.use_xml_format {
            instructions.push_str(&self.get_xml_format_instructions());
        } else {
            instructions.push_str(&self.get_json_format_instructions());
        }
        
        // Add examples if configured
        if self.config.include_examples {
            instructions.push_str(&self.get_examples(tools));
        }
        
        instructions
    }
    
    fn get_xml_format_instructions(&self) -> String {
        r#"
## Tool Calling Format

To call a tool, use the following XML format:

<tool_call>
<tool_name>function_name</tool_name>
<tool_id>call_12345</tool_id>
<arguments>
{
  "parameter1": "value1",
  "parameter2": "value2"
}
</arguments>
</tool_call>

IMPORTANT:
- Always generate a unique tool_id for each call
- Provide valid JSON in the arguments section
- You can make multiple tool calls in sequence
- After making tool calls, wait for the results before responding to the user
"#.to_string()
    }
    
    fn get_json_format_instructions(&self) -> String {
        r#"
## Tool Calling Format

To call a tool, use the following JSON format:

```tool_call
{
  "tool_name": "function_name",
  "tool_id": "call_12345",
  "arguments": {
    "parameter1": "value1",
    "parameter2": "value2"
  }
}
```

IMPORTANT:
- Always generate a unique tool_id for each call
- Provide valid JSON for all fields
- You can make multiple tool calls by using multiple ```tool_call blocks
- After making tool calls, wait for the results before responding to the user
"#.to_string()
    }
    
    fn get_examples(&self, tools: &[ToolDefinition]) -> String {
        if tools.is_empty() {
            return String::new();
        }
        
        let example_tool = &tools[0];
        let format = if self.config.use_xml_format {
            format!(
                r#"
## Example

User: Can you help me with {}?
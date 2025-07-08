//! Structured outputs engine for Anthropic-style tool calling
//! 
//! This engine handles Anthropic's approach to tool calling which uses
//! structured responses and explicit tool definitions.

use async_trait::async_trait;
use serde_json::{json, Value};
use tracing::{debug, error, warn};

use super::{ToolCallEngine, ToolCallEngineType, ToolCallContext, ChatMessage, ProviderConfig};
use crate::agent::core::{ToolDefinition, ToolCall, ToolResult};

/// Structured outputs engine for Anthropic-compatible APIs
pub struct StructuredOutputsEngine {
    config: StructuredEngineConfig,
}

#[derive(Debug, Clone)]
pub struct StructuredEngineConfig {
    pub max_tools_per_request: usize,
    pub supports_parallel_calls: bool,
    pub use_beta_tools: bool,
    pub enforce_schema_validation: bool,
}

impl Default for StructuredEngineConfig {
    fn default() -> Self {
        Self {
            max_tools_per_request: 50, // Anthropic supports moderate tool counts
            supports_parallel_calls: true,
            use_beta_tools: true,      // Anthropic often uses beta flags
            enforce_schema_validation: true,
        }
    }
}

impl StructuredOutputsEngine {
    pub fn new() -> Self {
        Self {
            config: StructuredEngineConfig::default(),
        }
    }
    
    pub fn with_config(config: StructuredEngineConfig) -> Self {
        Self { config }
    }
    
    /// Convert Juno tool definition to Anthropic tool format
    fn tool_definition_to_anthropic_tool(&self, tool: &ToolDefinition) -> Result<Value, String> {
        // Validate the input schema
        if !tool.input_schema.is_object() {
            return Err(format!("Tool '{}' has invalid schema - must be an object", tool.name));
        }
        
        let mut anthropic_tool = json!({
            "name": tool.name,
            "description": tool.description,
            "input_schema": tool.input_schema
        });
        
        // Add API type if specified (for versioned APIs)
        if let Some(api_type) = &tool.api_type {
            anthropic_tool["type"] = json!(api_type);
        }
        
        // Add beta flag if specified and config allows
        if self.config.use_beta_tools {
            if let Some(beta_flag) = &tool.beta_flag {
                anthropic_tool["cache_control"] = json!({"type": "ephemeral"});
                anthropic_tool["beta"] = json!(beta_flag);
            }
        }
        
        Ok(anthropic_tool)
    }
    
    /// Parse Anthropic tool use format to Juno ToolCall
    fn parse_anthropic_tool_use(&self, content_block: &Value) -> Result<ToolCall, String> {
        // Anthropic tool use has the format:
        // {
        //   "type": "tool_use",
        //   "id": "toolu_...",
        //   "name": "tool_name",
        //   "input": {...}
        // }
        
        let block_type = content_block.get("type")
            .and_then(|v| v.as_str())
            .ok_or("Missing or invalid 'type' field in content block")?;
            
        if block_type != "tool_use" {
            return Err(format!("Expected 'tool_use' type, got '{}'", block_type));
        }
        
        let id = content_block.get("id")
            .and_then(|v| v.as_str())
            .ok_or("Missing or invalid 'id' field in tool use")?;
            
        let name = content_block.get("name")
            .and_then(|v| v.as_str())
            .ok_or("Missing or invalid 'name' field in tool use")?;
            
        let input = content_block.get("input")
            .cloned()
            .unwrap_or(json!({}));
            
        Ok(ToolCall {
            id: id.to_string(),
            name: name.to_string(),
            input,
        })
    }
    
    /// Parse text content that might contain thinking or explanations
    fn extract_text_content(&self, content_blocks: &[Value]) -> String {
        let mut text_parts = Vec::new();
        
        for block in content_blocks {
            if let Some(block_type) = block.get("type").and_then(|v| v.as_str()) {
                if block_type == "text" {
                    if let Some(text) = block.get("text").and_then(|v| v.as_str()) {
                        text_parts.push(text.to_string());
                    }
                }
            }
        }
        
        text_parts.join("\\n\\n")
    }
}

#[async_trait]
impl ToolCallEngine for StructuredOutputsEngine {
    async fn prepare_tools_for_llm(
        &self,
        tools: &[ToolDefinition],
        context: &ToolCallContext,
    ) -> Result<Value, String> {
        debug!("Preparing {} tools for structured outputs engine (provider: {})", tools.len(), context.provider);
        
        if tools.len() > self.config.max_tools_per_request {
            warn!("Tool count ({}) exceeds maximum ({}), truncating", 
                  tools.len(), self.config.max_tools_per_request);
        }
        
        let tools_to_use = &tools[..tools.len().min(self.config.max_tools_per_request)];
        
        let mut anthropic_tools = Vec::new();
        for tool in tools_to_use {
            let anthropic_tool = self.tool_definition_to_anthropic_tool(tool)?;
            anthropic_tools.push(anthropic_tool);
        }
        
        let mut result = json!({
            "tools": anthropic_tools,
            "tool_choice": {"type": "auto"} // Let Anthropic decide when to use tools
        });
        
        // Add additional Anthropic-specific parameters
        if self.config.use_beta_tools {
            result["betas"] = json!(["computer-use-2024-10-22", "prompt-caching-2024-07-31"]);
        }
        
        debug!("Prepared {} tools for Anthropic-style API", anthropic_tools.len());
        Ok(result)
    }
    
    async fn extract_tool_calls(
        &self,
        response: &str,
        _context: &ToolCallContext,
    ) -> Result<Vec<ToolCall>, String> {
        debug!("Extracting tool calls from structured outputs response");
        
        // Parse the response JSON
        let response_json: Value = serde_json::from_str(response)
            .map_err(|e| format!("Failed to parse API response: {}", e))?;
        
        // Extract content from Anthropic response format
        let content = response_json
            .get("content")
            .and_then(|c| c.as_array())
            .ok_or("Missing or invalid 'content' array in response")?;
        
        let mut tool_calls = Vec::new();
        let mut has_text_content = false;
        
        for content_block in content {
            if let Some(block_type) = content_block.get("type").and_then(|v| v.as_str()) {
                match block_type {
                    "tool_use" => {
                        match self.parse_anthropic_tool_use(content_block) {
                            Ok(tool_call) => {
                                debug!("Parsed tool call: {} ({})", tool_call.name, tool_call.id);
                                tool_calls.push(tool_call);
                            }
                            Err(e) => {
                                error!("Failed to parse tool use: {}", e);
                                return Err(format!("Tool use parsing error: {}", e));
                            }
                        }
                    }
                    "text" => {
                        has_text_content = true;
                        // Text content is normal - Anthropic often includes thinking
                    }
                    _ => {
                        debug!("Unknown content block type: {}", block_type);
                    }
                }
            }
        }
        
        // Log if we have mixed content (text + tool calls)
        if has_text_content && !tool_calls.is_empty() {
            debug!("Response contains both text content and tool calls (Anthropic thinking pattern)");
        }
        
        debug!("Extracted {} tool calls from response", tool_calls.len());
        Ok(tool_calls)
    }
    
    async fn format_tool_results(
        &self,
        tool_results: &[ToolResult],
        _context: &ToolCallContext,
    ) -> Result<Vec<ChatMessage>, String> {
        debug!("Formatting {} tool results for structured outputs engine", tool_results.len());
        
        let mut messages = Vec::new();
        
        // Create content blocks for each tool result
        let mut content_blocks = Vec::new();
        
        for result in tool_results {
            let tool_result_block = json!({
                "type": "tool_result",
                "tool_use_id": result.call_id,
                "content": serde_json::to_string(&result.output)
                    .map_err(|e| format!("Failed to serialize tool result: {}", e))?
            });
            
            content_blocks.push(tool_result_block);
        }
        
        if !content_blocks.is_empty() {
            let message = ChatMessage {
                role: "user".to_string(),
                content: serde_json::to_string(&content_blocks)
                    .map_err(|e| format!("Failed to serialize content blocks: {}", e))?,
                tool_calls: None,
                tool_call_id: None,
            };
            
            messages.push(message);
        }
        
        debug!("Formatted {} tool result messages", messages.len());
        Ok(messages)
    }
    
    fn get_engine_type(&self) -> ToolCallEngineType {
        ToolCallEngineType::StructuredOutputs
    }
    
    fn supports_provider(&self, provider: &str) -> bool {
        matches!(provider.to_lowercase().as_str(), 
                 "anthropic" | "claude" | "claude-3" | "claude-3.5" | "bedrock")
    }
    
    fn get_provider_config(&self, provider: &str) -> Option<ProviderConfig> {
        if self.supports_provider(provider) {
            Some(ProviderConfig {
                max_tools_per_request: Some(self.config.max_tools_per_request),
                supports_parallel_calls: self.config.supports_parallel_calls,
                requires_system_prompt: false, // Can work with or without system prompts
                tool_choice_support: true,     // Supports explicit tool choice control
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
        let engine = StructuredOutputsEngine::new();
        
        let tool = ToolDefinition {
            name: "computer_20241022".to_string(),
            description: "Use computer to interact with the screen".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "action": {"type": "string", "enum": ["screenshot", "click", "type"]},
                    "coordinate": {"type": "array", "items": {"type": "number"}}
                },
                "required": ["action"]
            }),
            api_type: Some("computer_20241022".to_string()),
            beta_flag: Some("computer-use-2024-10-22".to_string()),
        };
        
        let context = ToolCallContext {
            model: "claude-3-5-sonnet-20241022".to_string(),
            provider: "anthropic".to_string(),
            messages: vec![],
            system_prompt: None,
            max_tokens: None,
            temperature: None,
        };
        
        let result = engine.prepare_tools_for_llm(&[tool], &context).await;
        assert!(result.is_ok());
        
        let prepared = result.unwrap();
        assert!(prepared.get("tools").is_some());
        assert!(prepared.get("betas").is_some());
        assert!(prepared.get("tools").unwrap().as_array().unwrap().len() == 1);
    }
    
    #[tokio::test]
    async fn test_tool_call_extraction() {
        let engine = StructuredOutputsEngine::new();
        
        let response = json!({
            "content": [
                {
                    "type": "text",
                    "text": "I'll help you take a screenshot of the screen."
                },
                {
                    "type": "tool_use",
                    "id": "toolu_123abc",
                    "name": "computer_20241022",
                    "input": {
                        "action": "screenshot"
                    }
                }
            ]
        });
        
        let context = ToolCallContext {
            model: "claude-3-5-sonnet-20241022".to_string(),
            provider: "anthropic".to_string(),
            messages: vec![],
            system_prompt: None,
            max_tokens: None,
            temperature: None,
        };
        
        let result = engine.extract_tool_calls(&response.to_string(), &context).await;
        assert!(result.is_ok());
        
        let tool_calls = result.unwrap();
        assert_eq!(tool_calls.len(), 1);
        assert_eq!(tool_calls[0].name, "computer_20241022");
        assert_eq!(tool_calls[0].id, "toolu_123abc");
        assert_eq!(tool_calls[0].input["action"], "screenshot");
    }
    
    #[tokio::test]
    async fn test_mixed_content_extraction() {
        let engine = StructuredOutputsEngine::new();
        
        let response = json!({
            "content": [
                {
                    "type": "text",
                    "text": "I need to take a screenshot first to see the current state of the screen."
                },
                {
                    "type": "tool_use",
                    "id": "toolu_456def",
                    "name": "computer_20241022",
                    "input": {
                        "action": "screenshot"
                    }
                },
                {
                    "type": "text",
                    "text": "Now I'll analyze what I can see and provide guidance."
                }
            ]
        });
        
        let context = ToolCallContext {
            model: "claude-3-5-sonnet-20241022".to_string(),
            provider: "anthropic".to_string(),
            messages: vec![],
            system_prompt: None,
            max_tokens: None,
            temperature: None,
        };
        
        let result = engine.extract_tool_calls(&response.to_string(), &context).await;
        assert!(result.is_ok());
        
        let tool_calls = result.unwrap();
        assert_eq!(tool_calls.len(), 1);
        assert_eq!(tool_calls[0].name, "computer_20241022");
    }
}
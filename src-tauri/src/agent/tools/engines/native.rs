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
        let input: Value = serde_json::from_str(arguments_str)
            .map_err(|e| format!("Invalid JSON in tool call arguments: {}", e))?;
            
        Ok(ToolCall {
            id: id.to_string(),
            name: name.to_string(),
            input, // Using Juno's 'input' field instead of 'arguments'
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
        let empty_vec = vec![];
        let tool_calls = response_json
            .get("choices")
            .and_then(|choices| choices.get(0))
            .and_then(|choice| choice.get("message"))
            .and_then(|message| message.get("tool_calls"))
            .and_then(|tc| tc.as_array())
            .unwrap_or(&empty_vec);
            
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
            let content = serde_json::to_string(&result.output)
                .map_err(|e| format!("Failed to serialize tool result: {}", e))?;
            
            let message = ChatMessage {
                role: "tool".to_string(),
                content,
                tool_calls: None,
                tool_call_id: Some(result.call_id.clone()), // Using Juno's 'call_id' field
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
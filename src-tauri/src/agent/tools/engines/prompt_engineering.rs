//! Prompt engineering tool call engine for models without native tool support
//! 
//! This engine uses carefully crafted prompts to enable tool calling
//! for models that don't have built-in function calling capabilities.

use async_trait::async_trait;
use serde_json::{json, Value};
use tracing::{debug, warn};
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
        let _format_name = if self.config.use_xml_format { "XML" } else { "JSON" };
        
        let example_args = json!({"example": "value"});
        
        if self.config.use_xml_format {
            format!(
                r#"
## Example

User: Can you help me with {}?

Assistant: I'll help you with that. Let me use the {} tool.

<tool_call>
<tool_name>{}</tool_name>
<tool_id>call_example_001</tool_id>
<arguments>
{}
</arguments>
</tool_call>
"#,
                example_tool.description,
                example_tool.name,
                example_tool.name,
                serde_json::to_string_pretty(&example_args).unwrap_or_default()
            )
        } else {
            format!(
                r#"
## Example

User: Can you help me with {}?

Assistant: I'll help you with that. Let me use the {} tool.

```tool_call
{{
  "tool_name": "{}",
  "tool_id": "call_example_001",
  "arguments": {}
}}
```
"#,
                example_tool.description,
                example_tool.name,
                example_tool.name,
                serde_json::to_string_pretty(&example_args).unwrap_or_default()
            )
        }
    }
    
    /// Extract tool calls from XML format
    fn extract_xml_tool_calls(&self, text: &str) -> Result<Vec<ToolCall>, String> {
        let mut tool_calls = Vec::new();
        
        // Regex to match XML tool call blocks
        let tool_call_regex = Regex::new(
            r"(?s)<tool_call>.*?<tool_name>(.*?)</tool_name>.*?<tool_id>(.*?)</tool_id>.*?<arguments>(.*?)</arguments>.*?</tool_call>"
        ).map_err(|e| format!("Regex compilation error: {}", e))?;
        
        for captures in tool_call_regex.captures_iter(text) {
            let name = captures.get(1).map(|m| m.as_str().trim()).unwrap_or("");
            let id = captures.get(2).map(|m| m.as_str().trim()).unwrap_or("");
            let args_text = captures.get(3).map(|m| m.as_str().trim()).unwrap_or("");
            
            if name.is_empty() || id.is_empty() {
                warn!("Skipping malformed XML tool call: name='{}', id='{}'", name, id);
                continue;
            }
            
            // Parse arguments JSON
            let input = if args_text.is_empty() {
                json!({})
            } else {
                match serde_json::from_str(args_text) {
                    Ok(json) => json,
                    Err(e) => {
                        if self.config.strict_json_parsing {
                            return Err(format!("Invalid JSON in tool call arguments: {}", e));
                        } else {
                            warn!("Using empty object for malformed JSON: {}", e);
                            json!({})
                        }
                    }
                }
            };
            
            tool_calls.push(ToolCall {
                id: id.to_string(),
                name: name.to_string(),
                input,
            });
        }
        
        Ok(tool_calls)
    }
    
    /// Extract tool calls from JSON format
    fn extract_json_tool_calls(&self, text: &str) -> Result<Vec<ToolCall>, String> {
        let mut tool_calls = Vec::new();
        
        // Regex to match ```tool_call JSON blocks
        let tool_call_regex = Regex::new(
            r"(?s)```tool_call\s*(.+?)\s*```"
        ).map_err(|e| format!("Regex compilation error: {}", e))?;
        
        for captures in tool_call_regex.captures_iter(text) {
            let json_text = captures.get(1).map(|m| m.as_str().trim()).unwrap_or("");
            
            if json_text.is_empty() {
                warn!("Skipping empty JSON tool call block");
                continue;
            }
            
            // Parse the JSON
            let parsed: Value = match serde_json::from_str(json_text) {
                Ok(json) => json,
                Err(e) => {
                    if self.config.strict_json_parsing {
                        return Err(format!("Invalid JSON in tool call: {}", e));
                    } else {
                        warn!("Skipping malformed JSON tool call: {}", e);
                        continue;
                    }
                }
            };
            
            // Extract fields
            let name = parsed.get("tool_name")
                .and_then(|v| v.as_str())
                .unwrap_or("");
            let id = parsed.get("tool_id")
                .and_then(|v| v.as_str())
                .unwrap_or("");
            let input = parsed.get("arguments")
                .cloned()
                .unwrap_or(json!({}));
            
            if name.is_empty() || id.is_empty() {
                warn!("Skipping malformed JSON tool call: name='{}', id='{}'", name, id);
                continue;
            }
            
            tool_calls.push(ToolCall {
                id: id.to_string(),
                name: name.to_string(),
                input,
            });
        }
        
        Ok(tool_calls)
    }
}

#[async_trait]
impl ToolCallEngine for PromptEngineeringEngine {
    async fn prepare_tools_for_llm(
        &self,
        tools: &[ToolDefinition],
        context: &ToolCallContext,
    ) -> Result<Value, String> {
        debug!("Preparing {} tools for prompt engineering (provider: {})", tools.len(), context.provider);
        
        if tools.len() > self.config.max_tools_per_request {
            warn!("Tool count ({}) exceeds maximum ({}), truncating", 
                  tools.len(), self.config.max_tools_per_request);
        }
        
        let tools_to_use = &tools[..tools.len().min(self.config.max_tools_per_request)];
        
        // Generate tool instructions to be added to system prompt
        let tool_instructions = self.generate_tool_instructions(tools_to_use);
        
        // Return the instructions as a structured format
        let result = json!({
            "system_prompt_addition": tool_instructions,
            "format": if self.config.use_xml_format { "xml" } else { "json" },
            "tool_count": tools_to_use.len(),
            "max_tools": self.config.max_tools_per_request
        });
        
        debug!("Prepared prompt engineering instructions for {} tools", tools_to_use.len());
        Ok(result)
    }
    
    async fn extract_tool_calls(
        &self,
        response: &str,
        _context: &ToolCallContext,
    ) -> Result<Vec<ToolCall>, String> {
        debug!("Extracting tool calls from prompt engineering response (format: {})", 
               if self.config.use_xml_format { "XML" } else { "JSON" });
        
        let tool_calls = if self.config.use_xml_format {
            self.extract_xml_tool_calls(response)?
        } else {
            self.extract_json_tool_calls(response)?
        };
        
        debug!("Extracted {} tool calls from response", tool_calls.len());
        Ok(tool_calls)
    }
    
    async fn format_tool_results(
        &self,
        tool_results: &[ToolResult],
        _context: &ToolCallContext,
    ) -> Result<Vec<ChatMessage>, String> {
        debug!("Formatting {} tool results for prompt engineering", tool_results.len());
        
        let mut messages = Vec::new();
        
        // Create a single message with all tool results
        let mut content_parts = Vec::new();
        
        for result in tool_results {
            let result_text = if self.config.use_xml_format {
                format!(
                    "<tool_result>\n<tool_call_id>{}</tool_call_id>\n<result>\n{}\n</result>\n</tool_result>",
                    result.call_id,
                    serde_json::to_string_pretty(&result.output).unwrap_or_default()
                )
            } else {
                format!(
                    "```tool_result\n{{\n  \"tool_call_id\": \"{}\",\n  \"result\": {}\n}}\n```",
                    result.call_id,
                    serde_json::to_string(&result.output).unwrap_or_default()
                )
            };
            
            content_parts.push(result_text);
        }
        
        if !content_parts.is_empty() {
            let combined_content = if self.config.use_xml_format {
                format!("Tool execution results:\n\n{}", content_parts.join("\n\n"))
            } else {
                format!("Tool execution results:\n\n{}", content_parts.join("\n\n"))
            };
            
            let message = ChatMessage {
                role: "user".to_string(), // Results come back as user messages in prompt engineering
                content: combined_content,
                tool_calls: None,
                tool_call_id: None,
            };
            
            messages.push(message);
        }
        
        debug!("Formatted {} tool result messages", messages.len());
        Ok(messages)
    }
    
    fn get_engine_type(&self) -> ToolCallEngineType {
        ToolCallEngineType::PromptEngineering
    }
    
    fn supports_provider(&self, provider: &str) -> bool {
        // Prompt engineering works with any provider
        match provider.to_lowercase().as_str() {
            "deepseek" | "qwen" | "llama" | "mistral" | "gemma" => true,
            _ => true // Fallback support for any provider
        }
    }
    
    fn get_provider_config(&self, provider: &str) -> Option<ProviderConfig> {
        if self.supports_provider(provider) {
            Some(ProviderConfig {
                max_tools_per_request: Some(self.config.max_tools_per_request),
                supports_parallel_calls: false, // Sequential execution for prompt-based
                requires_system_prompt: true,   // Need system prompt for instructions
                tool_choice_support: false,     // No explicit tool choice control
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
    async fn test_xml_tool_call_extraction() {
        let engine = PromptEngineeringEngine::new();
        
        let response = r#"
I'll help you with that.

<tool_call>
<tool_name>test_function</tool_name>
<tool_id>call_123</tool_id>
<arguments>
{"param1": "value1", "param2": 42}
</arguments>
</tool_call>

Here's the result.
"#;
        
        let context = ToolCallContext {
            model: "llama".to_string(),
            provider: "llama".to_string(),
            messages: vec![],
            system_prompt: None,
            max_tokens: None,
            temperature: None,
        };
        
        let result = engine.extract_tool_calls(response, &context).await;
        assert!(result.is_ok());
        
        let tool_calls = result.unwrap();
        assert_eq!(tool_calls.len(), 1);
        assert_eq!(tool_calls[0].name, "test_function");
        assert_eq!(tool_calls[0].id, "call_123");
    }
    
    #[tokio::test]
    async fn test_json_tool_call_extraction() {
        let mut config = PromptEngineConfig::default();
        config.use_xml_format = false;
        let engine = PromptEngineeringEngine::with_config(config);
        
        let response = r#"
I'll help you with that.

```tool_call
{
  "tool_name": "test_function",
  "tool_id": "call_123",
  "arguments": {"param1": "value1", "param2": 42}
}
```

Here's the result.
"#;
        
        let context = ToolCallContext {
            model: "deepseek".to_string(),
            provider: "deepseek".to_string(),
            messages: vec![],
            system_prompt: None,
            max_tokens: None,
            temperature: None,
        };
        
        let result = engine.extract_tool_calls(response, &context).await;
        assert!(result.is_ok());
        
        let tool_calls = result.unwrap();
        assert_eq!(tool_calls.len(), 1);
        assert_eq!(tool_calls[0].name, "test_function");
        assert_eq!(tool_calls[0].id, "call_123");
    }
    
    #[tokio::test]
    async fn test_tool_preparation() {
        let engine = PromptEngineeringEngine::new();
        
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
            model: "llama".to_string(),
            provider: "llama".to_string(),
            messages: vec![],
            system_prompt: None,
            max_tokens: None,
            temperature: None,
        };
        
        let result = engine.prepare_tools_for_llm(&[tool], &context).await;
        assert!(result.is_ok());
        
        let prepared = result.unwrap();
        assert!(prepared.get("system_prompt_addition").is_some());
        assert_eq!(prepared.get("format").unwrap().as_str().unwrap(), "xml");
    }
}
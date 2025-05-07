use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use thiserror::Error;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ToolResult {
    pub output: Option<String>,
    pub error: Option<String>,
    pub image_base64: Option<String>, // Base64 encoded PNG
    pub system_message: Option<String>, // For messages not intended for the LLM but for system logging/UI
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ToolDescription {
    pub name: String,
    pub description: String,
    pub parameters_schema: serde_json::Value, // JSON schema for tool parameters
}

#[derive(Error, Debug)]
pub enum ToolError {
    #[error("Tool execution error: {0}")]
    ExecutionError(String),
    #[error("Invalid parameters: {0}")]
    InvalidParameters(String),
    #[error("Tool not found: {0}")]
    NotFound(String),
    #[error(transparent)]
    SerdeJsonError(#[from] serde_json::Error),
    #[error("Internal tool error: {0}")]
    InternalError(String),
}

// Using a dynamic dispatch for tools
#[async_trait::async_trait]
pub trait Tool: Send + Sync {
    fn get_description(&self) -> ToolDescription;
    async fn execute(&self, params: serde_json::Value) -> Result<ToolResult, ToolError>;
}

#[derive(Default)]
pub struct ToolCollection {
    tools: HashMap<String, Box<dyn Tool>>,
}

impl ToolCollection {
    pub fn new() -> Self {
        ToolCollection {
            tools: HashMap::new(),
        }
    }

    pub fn add_tool(&mut self, tool: Box<dyn Tool>) -> Result<(), String> {
        let name = tool.get_description().name;
        if self.tools.contains_key(&name) {
            return Err(format!("Tool with name '{}' already exists.", name));
        }
        self.tools.insert(name, tool);
        Ok(())
    }

    pub fn get_tool(&self, name: &str) -> Option<&Box<dyn Tool>> {
        self.tools.get(name)
    }

    pub fn get_all_descriptions(&self) -> Vec<ToolDescription> {
        self.tools.values().map(|tool| tool.get_description()).collect()
    }

    pub async fn call_tool(&self, name: &str, params: serde_json::Value) -> Result<ToolResult, ToolError> {
        match self.get_tool(name) {
            Some(tool) => tool.execute(params).await,
            None => Err(ToolError::NotFound(name.to_string())),
        }
    }
}

// Example of how a tool might be implemented:
/*
#[derive(Debug)]
struct ExampleTool;

#[async_trait::async_trait]
impl Tool for ExampleTool {
    fn get_description(&self) -> ToolDescription {
        ToolDescription {
            name: "example_tool".to_string(),
            description: "An example tool that does something.".to_string(),
            parameters_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "message": {
                        "type": "string",
                        "description": "A message to print."
                    }
                },
                "required": ["message"]
            }),
        }
    }

    async fn execute(&self, params: serde_json::Value) -> Result<ToolResult, ToolError> {
        let message = params
            .get("message")
            .and_then(|v| v.as_str())
            .ok_or_else(|| ToolError::InvalidParameters("Missing or invalid 'message' parameter".to_string()))?;

        println!("ExampleTool executed with message: {}", message);

        Ok(ToolResult {
            output: Some(format!("Successfully processed message: {}", message)),
            error: None,
            image_base64: None,
            system_message: None,
        })
    }
}
*/

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[derive(Debug)]
    struct MockTool {
        name: String,
        description: String,
        params_schema: serde_json::Value,
        should_succeed: bool,
        output_message: String,
    }

    #[async_trait::async_trait]
    impl Tool for MockTool {
        fn get_description(&self) -> ToolDescription {
            ToolDescription {
                name: self.name.clone(),
                description: self.description.clone(),
                parameters_schema: self.params_schema.clone(),
            }
        }

        async fn execute(&self, params: serde_json::Value) -> Result<ToolResult, ToolError> {
            println!("MockTool {} executed with params: {:?}", self.name, params);
            if self.should_succeed {
                Ok(ToolResult {
                    output: Some(self.output_message.clone()),
                    error: None,
                    image_base64: None,
                    system_message: None,
                })
            } else {
                Err(ToolError::ExecutionError("Mock tool failed as instructed".to_string()))
            }
        }
    }

    #[test]
    fn tool_result_serialization() {
        let result = ToolResult {
            output: Some("Success".to_string()),
            error: None,
            image_base64: Some("base64string".to_string()),
            system_message: Some("System info".to_string()),
        };
        let serialized = serde_json::to_string(&result).unwrap();
        let deserialized: ToolResult = serde_json::from_str(&serialized).unwrap();
        assert_eq!(result.output, deserialized.output);
        assert_eq!(result.image_base64, deserialized.image_base64);
    }

    #[test]
    fn tool_description_serialization() {
        let desc = ToolDescription {
            name: "test_tool".to_string(),
            description: "A tool for testing".to_string(),
            parameters_schema: json!({"type": "object"}),
        };
        let serialized = serde_json::to_string(&desc).unwrap();
        let deserialized: ToolDescription = serde_json::from_str(&serialized).unwrap();
        assert_eq!(desc.name, deserialized.name);
        assert_eq!(desc.parameters_schema, deserialized.parameters_schema);
    }

    #[tokio::test]
    async fn tool_collection_add_and_get_tool() {
        let mut collection = ToolCollection::new();
        let tool1 = Box::new(MockTool {
            name: "tool1".to_string(),
            description: "First mock tool".to_string(),
            params_schema: json!({}),
            should_succeed: true,
            output_message: "Tool1 success".to_string(),
        });
        let tool1_name = tool1.get_description().name;
        collection.add_tool(tool1).unwrap();

        assert!(collection.get_tool(&tool1_name).is_some());
        assert!(collection.get_tool("non_existent_tool").is_none());

        let descriptions = collection.get_all_descriptions();
        assert_eq!(descriptions.len(), 1);
        assert_eq!(descriptions[0].name, tool1_name);
    }

    #[tokio::test]
    async fn tool_collection_call_tool_success() {
        let mut collection = ToolCollection::new();
        let tool_name = "successful_tool";
        let output_msg = "Success!";
        collection.add_tool(Box::new(MockTool {
            name: tool_name.to_string(),
            description: "A tool that succeeds".to_string(),
            params_schema: json!({}),
            should_succeed: true,
            output_message: output_msg.to_string(),
        })).unwrap();

        let result = collection.call_tool(tool_name, json!({})).await;
        assert!(result.is_ok());
        let tool_result = result.unwrap();
        assert_eq!(tool_result.output, Some(output_msg.to_string()));
        assert!(tool_result.error.is_none());
    }

    #[tokio::test]
    async fn tool_collection_call_tool_failure() {
        let mut collection = ToolCollection::new();
        let tool_name = "failing_tool";
        collection.add_tool(Box::new(MockTool {
            name: tool_name.to_string(),
            description: "A tool that fails".to_string(),
            params_schema: json!({}),
            should_succeed: false,
            output_message: "".to_string(),
        })).unwrap();

        let result = collection.call_tool(tool_name, json!({})).await;
        assert!(result.is_err());
        match result.err().unwrap() {
            ToolError::ExecutionError(msg) => assert_eq!(msg, "Mock tool failed as instructed"),
            _ => panic!("Expected ExecutionError"),
        }
    }

    #[tokio::test]
    async fn tool_collection_call_non_existent_tool() {
        let collection = ToolCollection::new();
        let result = collection.call_tool("ghost_tool", json!({})).await;
        assert!(result.is_err());
        match result.err().unwrap() {
            ToolError::NotFound(name) => assert_eq!(name, "ghost_tool"),
            _ => panic!("Expected NotFound error"),
        }
    }

     #[test]
    fn tool_collection_add_duplicate_tool() {
        let mut collection = ToolCollection::new();
        let tool = Box::new(MockTool {
            name: "duplicate_tool".to_string(),
            description: "A tool".to_string(),
            params_schema: json!({}),
            should_succeed: true,
            output_message: "output".to_string(),
        });
        collection.add_tool(tool).unwrap();

        let duplicate_tool = Box::new(MockTool {
            name: "duplicate_tool".to_string(), // Same name
            description: "Another tool".to_string(),
            params_schema: json!({}),
            should_succeed: true,
            output_message: "output2".to_string(),
        });

        let result = collection.add_tool(duplicate_tool);
        assert!(result.is_err());
        assert_eq!(result.err().unwrap(), "Tool with name 'duplicate_tool' already exists.");
    }
}

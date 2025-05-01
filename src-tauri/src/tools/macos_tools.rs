// Placeholder for macOS specific tools implementation

// Implementation for macOS specific tools
use crate::agent::structs::AgentError;
// Use ToolDefinition and ToolInputSchema from the SDK, not agent::structs
use computer_use_ai_sdk::{
    platforms::macos::engine::MacOSEngine,
    platforms::AccessibilityEngine, // Import the trait here
    ToolDefinition,
    ToolInputSchema,
};
use crate::tools::Tool;
use async_trait::async_trait;
use serde_json::{json, Value};
use std::collections::HashMap;

// --- GetRunningApplicationsTool ---

pub struct GetRunningApplicationsTool;

#[async_trait]
impl Tool for GetRunningApplicationsTool {
    // Return the SDK's ToolDefinition
    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "get_running_applications".to_string(),
            description: "Lists currently running applications on macOS.".to_string(),
            // Use the SDK's ToolInputSchema
            input_schema: ToolInputSchema {
                type_: "object".to_string(),
                properties: HashMap::new(), // No input properties
                required: Vec::new(),
            },
        }
    }

    async fn execute(&self, _args: Value) -> Result<Value, AgentError> {
        log::debug!("Executing get_running_applications tool");
        // Instantiate the engine. use_background_apps=true is less intrusive.
        let engine = MacOSEngine::new(true, false)
            .map_err(|e| AgentError::ToolError(format!("Failed to create MacOSEngine: {}", e)))?;

        // Get application elements (get_applications is now in scope via AccessibilityEngine trait)
        let app_elements = engine
            .get_applications()
            .map_err(|e| AgentError::ToolError(format!("Failed to get applications: {}", e)))?;

        // Extract names (labels) from the elements
        let app_names: Vec<String> = app_elements
            .into_iter()
            .filter_map(|app| app.attributes().label) // Get label (name) if available
            .collect();

        log::info!("Found running applications: {:?}", app_names);

        // Return as JSON array of strings
        Ok(json!(app_names))
    }
}

// --- Add other macOS tools below ---

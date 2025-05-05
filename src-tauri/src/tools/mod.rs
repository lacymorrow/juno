//! Module for handling low-level interactions with desktop UI elements and controls.

use computer_use_ai_sdk::{Desktop, ToolDefinition, ToolInputSchema, ToolParameter};
use std::collections::HashMap;
use std::sync::Arc;
use crate::agent::core::{AgentError, ToolCall};
use async_trait::async_trait;
use serde_json::Value;
use tokio::sync::Mutex;
use tracing::info;

pub mod macos_tools;

// --- Tool Definitions ---

#[allow(unused_variables)] // desktop parameter is not used currently
pub fn list_tools(desktop: &Arc<Desktop>) -> Vec<ToolDefinition> {
    // Keep existing tools and add new ones
    let tools = vec![
        // --- Existing Tools (Corrected Construction) ---
        ToolDefinition {
            name: "get_focused_element_info".to_string(),
            description: "Get information about the currently focused UI element.".to_string(),
            input_schema: ToolInputSchema {
                type_: "object".to_string(),
                properties: HashMap::new(), // No properties
                required: Vec::new(),       // No required fields
            },
        },
        ToolDefinition {
            name: "click_focused_element".to_string(),
            description: "Clicks the center of the currently focused UI element.".to_string(),
            input_schema: ToolInputSchema {
                type_: "object".to_string(),
                properties: HashMap::new(),
                required: Vec::new(),
            },
        },
        ToolDefinition {
            name: "type_text".to_string(),
            description: "Types the given text into the currently focused element.".to_string(),
            input_schema: ToolInputSchema {
                type_: "object".to_string(),
                properties: {
                    let mut props = HashMap::new();
                    props.insert("text".to_string(), ToolParameter { type_: "string".to_string(), description: "The text to type.".to_string() });
                    props
                },
                required: vec!["text".to_string()],
            },
        },
        ToolDefinition {
            name: "press_key".to_string(),
            description: "Presses a single key, optionally with a modifier.".to_string(),
            input_schema: ToolInputSchema {
                type_: "object".to_string(),
                properties: {
                    let mut props = HashMap::new();
                    props.insert("key".to_string(), ToolParameter { type_: "string".to_string(), description: "The key to press (e.g., 'a', 'Enter').".to_string() });
                    props.insert("modifier".to_string(), ToolParameter { type_: "string".to_string(), description: "Optional modifier key (e.g., 'cmd', 'ctrl').".to_string() }); // Add enum validation if needed
                    props
                },
                required: vec!["key".to_string()],
            },
        },
        ToolDefinition {
            name: "open_application".to_string(),
            description: "Opens an application by its name.".to_string(),
            input_schema: ToolInputSchema {
                type_: "object".to_string(),
                properties: {
                    let mut props = HashMap::new();
                    props.insert("app_name".to_string(), ToolParameter { type_: "string".to_string(), description: "The name of the application to open.".to_string() });
                    props
                },
                required: vec!["app_name".to_string()],
            },
        },
        ToolDefinition {
            name: "open_url".to_string(),
            description: "Opens a URL in the default web browser.".to_string(),
            input_schema: ToolInputSchema {
                type_: "object".to_string(),
                properties: {
                    let mut props = HashMap::new();
                    props.insert("url".to_string(), ToolParameter { type_: "string".to_string(), description: "The URL to open.".to_string() });
                    props
                },
                required: vec!["url".to_string()],
            },
        },
        ToolDefinition {
            name: "scroll_window".to_string(),
            description: "Scrolls the currently active window or element, optionally holding modifier keys.".to_string(),
            input_schema: ToolInputSchema {
                type_: "object".to_string(),
                properties: {
                    let mut props = HashMap::new();
                    props.insert("direction".to_string(), ToolParameter { type_: "string".to_string(), description: "Direction (up, down, left, right).".to_string() });
                    props.insert("amount".to_string(), ToolParameter { type_: "number".to_string(), description: "Amount to scroll.".to_string() });
                    props.insert("modifier_keys".to_string(), ToolParameter {
                        type_: "array".to_string(),
                        description: "Optional array of modifier keys (e.g., ['shift', 'cmd']) to hold during the scroll.".to_string()
                    });
                    props
                },
                required: vec!["direction".to_string(), "amount".to_string()],
            },
        },
        ToolDefinition {
            name: "capture_screenshot".to_string(),
            description: "Captures a screenshot of the entire screen.".to_string(),
            input_schema: ToolInputSchema {
                type_: "object".to_string(),
                properties: HashMap::new(),
                required: Vec::new(),
            },
        },
        ToolDefinition {
            name: "capture_element_screenshot".to_string(),
            description: "Captures a screenshot of the currently focused UI element.".to_string(),
            input_schema: ToolInputSchema {
                type_: "object".to_string(),
                properties: HashMap::new(),
                required: Vec::new(),
            },
        },
        // --- Added Tools (Corrected Construction) ---
        ToolDefinition {
            name: "wait".to_string(),
            description: "Pauses execution for a specified duration.".to_string(),
            input_schema: ToolInputSchema {
                type_: "object".to_string(),
                properties: {
                    let mut props = HashMap::new();
                    props.insert("duration_ms".to_string(), ToolParameter { type_: "integer".to_string(), description: "Wait duration in milliseconds.".to_string() });
                    props
                },
                required: vec!["duration_ms".to_string()],
            },
        },
        ToolDefinition {
            name: "cursor_position".to_string(),
            description: "Gets the current mouse cursor position.".to_string(),
            input_schema: ToolInputSchema {
                type_: "object".to_string(),
                properties: HashMap::new(),
                required: Vec::new(),
            },
        },
        ToolDefinition {
            name: "mouse_move".to_string(),
            description: "Moves the mouse cursor to specified coordinates.".to_string(),
            input_schema: ToolInputSchema {
                type_: "object".to_string(),
                properties: {
                    let mut props = HashMap::new();
                    props.insert("x".to_string(), ToolParameter { type_: "number".to_string(), description: "Target X coordinate.".to_string() });
                    props.insert("y".to_string(), ToolParameter { type_: "number".to_string(), description: "Target Y coordinate.".to_string() });
                    props
                },
                required: vec!["x".to_string(), "y".to_string()],
            },
        },
        ToolDefinition {
            name: "left_mouse_down".to_string(),
            description: "Presses and holds the left mouse button at coordinates.".to_string(),
            input_schema: ToolInputSchema {
                type_: "object".to_string(),
                properties: {
                    let mut props = HashMap::new();
                    props.insert("x".to_string(), ToolParameter { type_: "number".to_string(), description: "X coordinate.".to_string() });
                    props.insert("y".to_string(), ToolParameter { type_: "number".to_string(), description: "Y coordinate.".to_string() });
                    props
                },
                required: vec!["x".to_string(), "y".to_string()],
            },
        },
        ToolDefinition {
            name: "left_mouse_up".to_string(),
            description: "Releases the left mouse button at coordinates.".to_string(),
            input_schema: ToolInputSchema {
                type_: "object".to_string(),
                properties: {
                    let mut props = HashMap::new();
                    props.insert("x".to_string(), ToolParameter { type_: "number".to_string(), description: "X coordinate.".to_string() });
                    props.insert("y".to_string(), ToolParameter { type_: "number".to_string(), description: "Y coordinate.".to_string() });
                    props
                },
                required: vec!["x".to_string(), "y".to_string()],
            },
        },
        ToolDefinition {
            name: "left_click".to_string(),
            description: "Performs a left mouse click at coordinates, optionally holding modifier keys.".to_string(),
            input_schema: ToolInputSchema {
                type_: "object".to_string(),
                properties: {
                    let mut props = HashMap::new();
                    props.insert("x".to_string(), ToolParameter { type_: "number".to_string(), description: "X coordinate.".to_string() });
                    props.insert("y".to_string(), ToolParameter { type_: "number".to_string(), description: "Y coordinate.".to_string() });
                    // Add optional modifier keys parameter
                    props.insert("modifier_keys".to_string(), ToolParameter {
                        type_: "array".to_string(),
                        description: "Optional array of modifier keys (e.g., ['shift', 'cmd']) to hold during the click.".to_string(),
                        // items: Some(Box::new(ToolParameter { type_: "string".to_string(), description: "A modifier key (e.g., 'shift', 'cmd', 'ctrl', 'alt')".to_string() })) // Define item type if schema supports it
                    });
                    props
                },
                required: vec!["x".to_string(), "y".to_string()], // Modifiers are optional
            },
        },
        ToolDefinition {
            name: "right_click".to_string(),
            description: "Performs a right mouse click at coordinates.".to_string(),
            input_schema: ToolInputSchema {
                type_: "object".to_string(),
                properties: {
                    let mut props = HashMap::new();
                    props.insert("x".to_string(), ToolParameter { type_: "number".to_string(), description: "X coordinate.".to_string() });
                    props.insert("y".to_string(), ToolParameter { type_: "number".to_string(), description: "Y coordinate.".to_string() });
                    props
                },
                required: vec!["x".to_string(), "y".to_string()],
            },
        },
        ToolDefinition {
            name: "middle_click".to_string(),
            description: "Performs a middle mouse click at coordinates.".to_string(),
            input_schema: ToolInputSchema {
                type_: "object".to_string(),
                properties: {
                    let mut props = HashMap::new();
                    props.insert("x".to_string(), ToolParameter { type_: "number".to_string(), description: "X coordinate.".to_string() });
                    props.insert("y".to_string(), ToolParameter { type_: "number".to_string(), description: "Y coordinate.".to_string() });
                    props
                },
                required: vec!["x".to_string(), "y".to_string()],
            },
        },
        ToolDefinition {
            name: "double_click".to_string(),
            description: "Double-clicks the left mouse button at the specified (x, y) coordinates.".to_string(),
            input_schema: ToolInputSchema {
                type_: "object".to_string(),
                properties: {
                    let mut props = HashMap::new();
                    props.insert("x".to_string(), ToolParameter { type_: "number".to_string(), description: "X coordinate".to_string() });
                    props.insert("y".to_string(), ToolParameter { type_: "number".to_string(), description: "Y coordinate".to_string() });
                    props
                },
                required: vec!["x".to_string(), "y".to_string()],
            },
        },
        ToolDefinition {
            name: "triple_click".to_string(),
            description: "Triple-clicks the left mouse button at the specified (x, y) coordinates.".to_string(),
            input_schema: ToolInputSchema {
                type_: "object".to_string(),
                properties: {
                    let mut props = HashMap::new();
                    props.insert("x".to_string(), ToolParameter { type_: "number".to_string(), description: "X coordinate".to_string() });
                    props.insert("y".to_string(), ToolParameter { type_: "number".to_string(), description: "Y coordinate".to_string() });
                    props
                },
                required: vec!["x".to_string(), "y".to_string()],
            },
        },
        ToolDefinition {
            name: "left_click_drag".to_string(),
            description: "Drags the mouse with the left button held down.".to_string(),
            input_schema: ToolInputSchema {
                type_: "object".to_string(),
                properties: {
                    let mut props = HashMap::new();
                    props.insert("start_x".to_string(), ToolParameter { type_: "number".to_string(), description: "Starting X coordinate.".to_string() });
                    props.insert("start_y".to_string(), ToolParameter { type_: "number".to_string(), description: "Starting Y coordinate.".to_string() });
                    props.insert("end_x".to_string(), ToolParameter { type_: "number".to_string(), description: "Ending X coordinate.".to_string() });
                    props.insert("end_y".to_string(), ToolParameter { type_: "number".to_string(), description: "Ending Y coordinate.".to_string() });
                    props
                },
                required: vec!["start_x".to_string(), "start_y".to_string(), "end_x".to_string(), "end_y".to_string()],
            },
        },
        ToolDefinition {
            name: "scroll_at_position".to_string(),
            description: "Scrolls the view at a specific coordinate, optionally holding modifier keys.".to_string(),
            input_schema: ToolInputSchema {
                type_: "object".to_string(),
                properties: {
                    let mut props = HashMap::new();
                    props.insert("x".to_string(), ToolParameter { type_: "number".to_string(), description: "X coordinate to scroll at.".to_string() });
                    props.insert("y".to_string(), ToolParameter { type_: "number".to_string(), description: "Y coordinate to scroll at.".to_string() });
                    props.insert("direction".to_string(), ToolParameter { type_: "string".to_string(), description: "Direction (up, down, left, right).".to_string() });
                    props.insert("amount".to_string(), ToolParameter { type_: "number".to_string(), description: "Amount to scroll.".to_string() });
                    props.insert("modifier_keys".to_string(), ToolParameter {
                        type_: "array".to_string(),
                        description: "Optional array of modifier keys (e.g., ['shift', 'cmd']) to hold during the scroll.".to_string()
                    });
                    props
                },
                required: vec!["x".to_string(), "y".to_string(), "direction".to_string(), "amount".to_string()],
            },
        },
        ToolDefinition {
            name: "hold_key".to_string(),
            description: "Presses and holds a modifier key.".to_string(),
            input_schema: ToolInputSchema {
                type_: "object".to_string(),
                properties: {
                    let mut props = HashMap::new();
                    props.insert("key".to_string(), ToolParameter { type_: "string".to_string(), description: "Modifier key to hold (cmd, ctrl, alt, shift).".to_string() });
                    props
                },
                required: vec!["key".to_string()],
            },
        },
        ToolDefinition {
            name: "release_key".to_string(),
            description: "Releases a previously held modifier key.".to_string(),
            input_schema: ToolInputSchema {
                type_: "object".to_string(),
                properties: {
                    let mut props = HashMap::new();
                    props.insert("key".to_string(), ToolParameter { type_: "string".to_string(), description: "Modifier key to release.".to_string() });
                    props
                },
                required: vec!["key".to_string()],
            },
        },
        // --- Text Editor Tools ---
        ToolDefinition {
            name: "text_editor_insert".to_string(),
            description: "Inserts text into a file at a specific line number.".to_string(),
            input_schema: ToolInputSchema {
                type_: "object".to_string(),
                properties: {
                    let mut props = HashMap::new();
                    props.insert("file_path".to_string(), ToolParameter { type_: "string".to_string(), description: "Absolute path to the file.".to_string() });
                    props.insert("line_number".to_string(), ToolParameter { type_: "integer".to_string(), description: "1-based line number to insert at.".to_string() });
                    props.insert("text_to_insert".to_string(), ToolParameter { type_: "string".to_string(), description: "Text to insert.".to_string() });
                    props
                },
                required: vec!["file_path".to_string(), "line_number".to_string(), "text_to_insert".to_string()],
            },
        },
        ToolDefinition {
            name: "text_editor_str_replace".to_string(),
            description: "Replaces all occurrences of a string in a file.".to_string(),
            input_schema: ToolInputSchema {
                type_: "object".to_string(),
                properties: {
                    let mut props = HashMap::new();
                    props.insert("file_path".to_string(), ToolParameter { type_: "string".to_string(), description: "Absolute path to the file.".to_string() });
                    props.insert("find_text".to_string(), ToolParameter { type_: "string".to_string(), description: "Text to find.".to_string() });
                    props.insert("replace_text".to_string(), ToolParameter { type_: "string".to_string(), description: "Replacement text.".to_string() });
                    props
                },
                required: vec!["file_path".to_string(), "find_text".to_string(), "replace_text".to_string()],
            },
        },
        // text_editor_undo_edit (Corrected definition)
        ToolDefinition {
            name: "text_editor_undo_edit".to_string(),
            description: "Undoes the last text editing operation (create, insert, replace).".to_string(),
            input_schema: ToolInputSchema {
                type_: "object".to_string(),
                properties: {
                    let mut props = HashMap::new();
                    props.insert("file_path".to_string(), ToolParameter { type_: "string".to_string(), description: "The path to the file for which the last edit should be undone (used for confirmation).".to_string() });
                    props
                },
                required: vec!["file_path".to_string()],
            },
        },
        // --- Bash Tool ---
        ToolDefinition {
            name: "bash".to_string(),
            description: "Executes a shell command.".to_string(),
            input_schema: ToolInputSchema {
                type_: "object".to_string(),
                properties: {
                    let mut props = HashMap::new();
                    props.insert("command".to_string(), ToolParameter { type_: "string".to_string(), description: "Command line to execute.".to_string() });
                    props.insert("timeout_seconds".to_string(), ToolParameter { type_: "integer".to_string(), description: "Optional timeout.".to_string() });
                    props.insert("restart".to_string(), ToolParameter { type_: "boolean".to_string(), description: "Optional: Specify true to restart the shell state before running the command.".to_string() });
                    props
                },
                required: vec!["command".to_string()],
            },
        },
        // --- Standard Tools (Potentially Missing or Custom Implemented) ---
        ToolDefinition {
            name: "read_file".to_string(),
            description: "Reads the content of a file at the specified path.".to_string(),
            input_schema: ToolInputSchema {
                type_: "object".to_string(),
                properties: {
                    let mut props = HashMap::new();
                    props.insert("path".to_string(), ToolParameter { type_: "string".to_string(), description: "The path to the file.".to_string() });
                    props
                },
                required: vec!["path".to_string()],
            },
        },
        ToolDefinition {
            name: "write_file".to_string(),
            description: "Writes content to a file at the specified path.".to_string(),
            input_schema: ToolInputSchema {
                type_: "object".to_string(),
                properties: {
                    let mut props = HashMap::new();
                    props.insert("path".to_string(), ToolParameter { type_: "string".to_string(), description: "The path to the file.".to_string() });
                    props.insert("content".to_string(), ToolParameter { type_: "string".to_string(), description: "The content to write.".to_string() });
                    props
                },
                required: vec!["path".to_string(), "content".to_string()],
            },
        },
        ToolDefinition {
            name: "get_element_by_description".to_string(),
            description: "Finds a UI element based on a natural language description.".to_string(),
            input_schema: ToolInputSchema {
                type_: "object".to_string(),
                properties: {
                    let mut props = HashMap::new();
                    props.insert("description".to_string(), ToolParameter { type_: "string".to_string(), description: "Natural language description of the element.".to_string() });
                    props
                },
                required: vec!["description".to_string()],
            },
        },
        ToolDefinition {
            name: "get_element_tree".to_string(),
            description: "Gets the UI element tree structure.".to_string(),
            input_schema: ToolInputSchema {
                type_: "object".to_string(),
                properties: HashMap::new(),
                required: Vec::new(),
            },
        },
        ToolDefinition {
            name: "get_clipboard_content".to_string(),
            description: "Gets the current content of the clipboard.".to_string(),
            input_schema: ToolInputSchema {
                type_: "object".to_string(),
                properties: HashMap::new(),
                required: Vec::new(), // Corrected: No required fields for 'get'
            },
        },
        ToolDefinition {
            name: "set_clipboard_content".to_string(),
            description: "Sets the content of the clipboard.".to_string(),
            input_schema: ToolInputSchema {
                type_: "object".to_string(),
                properties: {
                    let mut props = HashMap::new();
                    props.insert("content".to_string(), ToolParameter { type_: "string".to_string(), description: "The content to set.".to_string() });
                    props
                },
                required: vec!["content".to_string()],
            },
        },
        // --- Newly Added Definitions from Anthropic Docs ---
        ToolDefinition {
            name: "get_browser_info".to_string(),
            description: "Gets information about the active browser tab (URL, title). Requires browser extension.".to_string(),
            input_schema: ToolInputSchema {
                type_: "object".to_string(),
                properties: HashMap::new(),
                required: Vec::new(),
            },
        },
        ToolDefinition {
            name: "run_applescript".to_string(),
            description: "Executes the given AppleScript code (macOS only).".to_string(),
            input_schema: ToolInputSchema {
                type_: "object".to_string(),
                properties: {
                    let mut props = HashMap::new();
                    props.insert("script".to_string(), ToolParameter { type_: "string".to_string(), description: "The AppleScript code to execute.".to_string() });
                    props
                },
                required: vec!["script".to_string()],
            },
        },
        ToolDefinition {
            name: "get_screen_text".to_string(),
            description: "Extracts text content from the screen using OCR.".to_string(),
            input_schema: ToolInputSchema {
                type_: "object".to_string(),
                properties: HashMap::new(), // Optionally add region parameters later
                required: Vec::new(),
            },
        },
        // --- Custom Tools ---
        // ToolDefinition {
        //     name: "read_file_contents".to_string(), // Keep custom for now
        //     description: "Reads the content of a file at the specified path.".to_string(),
        //     input_schema: ToolInputSchema {
        //         type_: "object".to_string(),
        //         properties: {
        //             let mut props = HashMap::new();
        //             props.insert("path".to_string(), ToolParameter { type_: "string".to_string(), description: "The path to the file.".to_string() });
        //             props
        //         },
        //         required: vec!["path".to_string()],
        //     },
        // },
        // --- Standard Tools (To Be Implemented) ---
        ToolDefinition {
            name: "find_files".to_string(),
            description: "Finds files based on name patterns and optional search path.".to_string(),
            input_schema: ToolInputSchema {
                type_: "object".to_string(),
                properties: {
                    let mut props = HashMap::new();
                    props.insert("pattern".to_string(), ToolParameter { type_: "string".to_string(), description: "Filename pattern (e.g., '*.txt', 'document?.doc').".to_string() });
                    props.insert("path".to_string(), ToolParameter { type_: "string".to_string(), description: "Optional directory path to search within (defaults to home directory).".to_string() });
                    props
                },
                required: vec!["pattern".to_string()],
            },
        },
    ];

    // Add platform-specific tools or modify existing ones if needed
    #[cfg(target_os = "macos")]
    {
        // Example: Add macOS specific tool if any
    }

    tools
}

/// Defines the interface for a tool that the agent can execute.
#[async_trait]
pub trait Tool: Send + Sync {
    /// Returns the definition of the tool (name, description, schema).
    fn definition(&self) -> ToolDefinition;

    /// Executes the tool with the given arguments.
    /// `args` should be a JSON Value that conforms to the tool's input schema.
    async fn execute(&self, desktop: &Desktop, args: Value) -> Result<Value, AgentError>;
}

/// Manages a collection of available tools.
pub struct ToolRegistry {
    tools: HashMap<String, Arc<dyn Tool>>,
    desktop: Mutex<Arc<Desktop>>,
}

impl ToolRegistry {
    pub fn new(tools: Vec<Arc<dyn Tool>>, desktop: Arc<Desktop>) -> Self {
        let tools_map = tools
            .into_iter()
            .map(|tool| (tool.definition().name.clone(), tool))
            .collect();
        ToolRegistry { tools: tools_map, desktop: Mutex::new(desktop) }
    }

    /// Returns definitions of all registered tools.
    pub fn get_tool_definitions(&self) -> Vec<ToolDefinition> {
        self.tools.values().map(|tool| tool.definition()).collect()
    }

    /// Executes a specific tool call.
    pub async fn execute_tool(&self, tool_call: &ToolCall) -> Result<Value, AgentError> {
        match self.tools.get(&tool_call.name) {
            Some(tool) => {
                // Lock the desktop mutex to get a reference
                let desktop_guard = self.desktop.lock().await;
                // Pass the locked desktop reference and the args
                tool.execute(&*desktop_guard, tool_call.input.clone()).await
            },
            None => Err(AgentError::ToolNotFound(tool_call.name.clone())),
        }
    }
}

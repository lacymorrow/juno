use crate::state::AppState;
use computer_use_ai_sdk::{
    Desktop, ToolDefinition, ToolInputSchema, ToolParameter,
};
use serde_json::{json, Value};
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use std::process::Command;
use std::sync::Arc;
use std::time::Duration;
use tauri::{AppHandle, State};
use tauri_plugin_notification::NotificationExt;
use tracing::{error, info, warn};
use wait_timeout::ChildExt;

#[cfg(target_os = "macos")]
use computer_use_ai_sdk::platforms::macos::utils as macos_utils;
#[cfg(target_os = "macos")]
use computer_use_ai_sdk::platforms::macos::element::MacOSUIElement;

// --- Tool Definitions ---

#[allow(unused_variables)] // desktop parameter is not used currently
pub(crate) fn list_tools(desktop: &Arc<Desktop>) -> Vec<ToolDefinition> {
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
            name: "text_editor_view".to_string(),
            description: "Reads and returns the content of a text file, optionally within a specific line range.".to_string(),
            input_schema: ToolInputSchema {
                type_: "object".to_string(),
                properties: {
                     let mut props = HashMap::new();
                    props.insert("file_path".to_string(), ToolParameter { type_: "string".to_string(), description: "Absolute path to the file.".to_string() });
                    props.insert("start_line".to_string(), ToolParameter { type_: "integer".to_string(), description: "Optional 1-based start line number.".to_string() });
                    props.insert("end_line".to_string(), ToolParameter { type_: "integer".to_string(), description: "Optional 1-based end line number.".to_string() });
                    props
                },
                required: vec!["file_path".to_string()],
            },
        },
        ToolDefinition {
            name: "text_editor_create".to_string(),
            description: "Creates/overwrites a text file with given content.".to_string(),
            input_schema: ToolInputSchema {
                type_: "object".to_string(),
                properties: {
                     let mut props = HashMap::new();
                    props.insert("file_path".to_string(), ToolParameter { type_: "string".to_string(), description: "Absolute path for the file.".to_string() });
                    props.insert("content".to_string(), ToolParameter { type_: "string".to_string(), description: "Initial content.".to_string() });
                    props
                },
                required: vec!["file_path".to_string(), "content".to_string()],
            },
        },
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
                required: vec!["content".to_string()],
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
            description: "Executes the given AppleScript code (macOS only)." .to_string(),
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
        ToolDefinition {
            name: "read_file_contents".to_string(), // Keep custom for now
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
    ];

    // Add platform-specific tools or modify existing ones if needed
    #[cfg(target_os = "macos")]
    {
        // Example: Add macOS specific tool if any
    }

    tools
}

// --- Parameter Helper Functions ---

// Helper to extract string param or return error JSON
#[allow(dead_code)] // Allow dead code for helper potentially used by call_tool
fn get_string_param(input: &Value, key: &str) -> Result<String, Value> {
    input[key]
        .as_str()
        .map(String::from)
        .ok_or_else(|| json!({"error": format!("Missing or invalid string parameter: {}", key)}))
}

// Helper to extract optional string param (Corrected)
#[allow(dead_code)] // Allow dead code for helper potentially used by call_tool
fn get_optional_string_param(input: &Value, key: &str) -> Result<Option<String>, Value> {
    match input.get(key) {
        Some(value) => {
            if value.is_null() {
                Ok(None) // Treat null as None
            } else {
                value.as_str()
                     .map(|s| Ok(Some(s.to_string())))
                     .unwrap_or_else(|| Err(json!({"error": format!("Invalid optional string parameter type: {}", key)})))
            }
        }
        None => Ok(None), // Key not present is Ok(None)
    }
}


// Helper to extract f64 param or return error JSON
#[allow(dead_code)] // Allow dead code for helper potentially used by call_tool
fn get_f64_param(input: &Value, key: &str) -> Result<f64, Value> {
    input[key]
        .as_f64()
        .ok_or_else(|| json!({"error": format!("Missing or invalid number parameter: {}", key)}))
}

// Helper to extract u64 param or return error JSON
#[allow(dead_code)] // Allow dead code for helper potentially used by call_tool
fn get_u64_param(input: &Value, key: &str) -> Result<u64, Value> {
    input[key]
        .as_u64()
        .ok_or_else(|| json!({"error": format!("Missing or invalid integer parameter: {}", key)}))
}

// Helper to extract i64 param or return error JSON
#[allow(dead_code)] // Allow dead code for helper potentially used by call_tool
fn get_i64_param(input: &Value, key: &str) -> Result<i64, Value> {
    input[key]
        .as_i64()
        .ok_or_else(|| json!({"error": format!("Missing or invalid integer parameter: {}", key)}))
}

// Helper function to get an optional u64 parameter from JSON
#[allow(dead_code)] // Allow dead code for helper potentially used by call_tool
fn get_optional_u64_param(input: &Value, key: &str) -> Result<Option<u64>, Value> {
    match input.get(key) {
        Some(value) => {
            if value.is_null() {
                Ok(None) // Treat null as None
            } else if let Some(num) = value.as_u64() {
                Ok(Some(num))
            } else {
                // Use value.to_string() or describe the type in the error message
                Err(json!({ "error": format!("Invalid type for parameter '{}': expected u64 or null, got type {}", key, value.to_string()) }))
            }
        }
        None => Ok(None), // Key not present
    }
}

// Helper to extract boolean param or return error JSON
#[allow(dead_code)] // Allow dead code for helper potentially used by call_tool
fn get_optional_bool_param(input: &Value, key: &str) -> Result<Option<bool>, Value> {
    match input.get(key) {
        Some(value) => {
            if value.is_null() {
                Ok(None) // Treat null as None
            } else if let Some(bool_value) = value.as_bool() {
                Ok(Some(bool_value))
            } else {
                // Use value.to_string() or describe the type in the error message
                Err(json!({ "error": format!("Invalid type for parameter '{}': expected bool or null, got type {}", key, value.to_string()) }))
            }
        }
        None => Ok(None), // Key not present
    }
}

// --- Tool Implementations (Specific) ---

// Tool function for find and replace in a file
fn str_replace_editor(file_path: String, find_text: String, replace_text: String) -> Result<String, String> {
    info!(file_path = %file_path, find = %find_text, "Attempting str_replace_editor");

    // Read the file content
    let content = match fs::read_to_string(&file_path) {
        Ok(c) => c,
        Err(e) => {
            let err_msg = format!("Failed to read file '{}': {}", file_path, e);
            error!(error = %err_msg, "str_replace_editor failed");
            return Err(err_msg);
        }
    };

    // Perform the replacement
    let new_content = content.replace(&find_text, &replace_text);

    // Write the new content back to the file
    match fs::write(&file_path, new_content) {
        Ok(_) => {
            let success_msg = format!("Successfully updated file '{}'", file_path);
            info!(success_msg);
            Ok(success_msg)
        }
        Err(e) => {
            let err_msg = format!("Failed to write file '{}': {}", file_path, e);
            error!(error = %err_msg, "str_replace_editor failed");
            Err(err_msg)
        }
    }
}


// --- Tool Call Dispatcher ---

// Tool call dispatcher (Corrected Error Handling and Return Type)
#[allow(dead_code)] // Allow dead code for helper potentially used by submit_query
pub(crate) async fn call_tool(
    desktop: &Arc<Desktop>,
    app_handle: &AppHandle,
    tool_name: &str,
    input: &Value,
    state: &State<'_, AppState>, // Correctly include state here in the definition
) -> Result<Value, Value> { // Returns Result<SuccessJson, ErrorJson>
    // Use debug formatting for potentially complex input Value
    info!(tool_name = %tool_name, input = ?input, "Calling tool");

    // Wrap the core logic in an async block
    let result = async {
        match tool_name {
            "get_focused_element_info" => {
                match desktop.focused_element() { // Changed from get_focused_element
                    Ok(element) => {
                        let attrs = element.attributes();
                        serde_json::to_value(&attrs).map_err(|e| json!({"error": format!("Failed to serialize element info: {}", e)}))
                    },
                    Err(e) => Err(json!({"error": format!("Failed to get focused element: {}", e)})),
                }
            }
            "click_focused_element" => {
                match desktop.focused_element() { // Changed from get_focused_element
                    Ok(element) => {
                        match element.click() {
                             Ok(_) => Ok(json!({"success": true, "message": "Clicked focused element."})),
                             Err(e) => Err(json!({"error": format!("Failed to click focused element: {}", e)})),
                        }
                    },
                    Err(e) => Err(json!({"error": format!("Failed to get focused element for clicking: {}", e)})),
                }
            }
            "type_text" => {
                match get_string_param(input, "text") {
                    Ok(text) => match desktop.type_text(&text) {
                        Ok(_) => Ok(json!({"success": true, "message": "Text typed."})),
                        Err(e) => Err(json!({"error": format!("Failed to type text: {}", e)})),
                    },
                    Err(e) => Err(e), // Propagate param parsing error
                }
            }
            "press_key" => {
                 match (get_string_param(input, "key"), get_optional_string_param(input, "modifier")) {
                    (Ok(key), Ok(modifier)) => {
                         match desktop.press_key(&key, modifier.as_deref()) {
                             Ok(_) => Ok(json!({"success": true, "message": format!("Key '{}' pressed.", key)})),
                             Err(e) => Err(json!({"error": format!("Failed to press key: {}", e)})),
                         }
                     }
                     (Err(e), _) | (_, Err(e)) => Err(e), // Propagate param parsing error
                 }
            }
            "open_application" => {
                 match get_string_param(input, "app_name") {
                     Ok(app_name) => match desktop.open_application(&app_name) {
                         Ok(_) => Ok(json!({"success": true, "message": format!("Application '{}' opened.", app_name)})),
                         Err(e) => Err(json!({"error": format!("Failed to open application: {}", e)})),
                     },
                     Err(e) => Err(e),
                 }
            }
            "open_url" => {
                 match get_string_param(input, "url") {
                     Ok(url) => match desktop.open_url(&url, None) {
                         Ok(_) => Ok(json!({"success": true, "message": format!("URL '{}' opened.", url)})),
                         Err(e) => Err(json!({"error": format!("Failed to open URL: {}", e)})),
                     },
                     Err(e) => Err(e),
                 }
            }
            "scroll_window" => { // Maps to scroll_at_current_position
                 let direction = get_string_param(input, "direction")?;
                 let amount = get_f64_param(input, "amount")?;
                 let modifier_keys = get_optional_modifier_keys(input)?;

                 if let Some(keys) = modifier_keys {
                     if !keys.is_empty() {
                         info!(%direction, %amount, ?keys, "Executing scroll_window with modifiers");
                         hold_keys_and_run(desktop, &keys, || {
                             desktop.scroll_at_current_position(&direction, amount)
                         })
                         .map(|_| json!({ "success": true, "message": format!("Scrolled {} by {} with modifiers {:?}.", direction, amount, keys) }))
                     } else {
                         // Empty modifier list, normal scroll
                         info!(%direction, %amount, "Executing scroll_window (empty modifiers list)");
                         match desktop.scroll_at_current_position(&direction, amount) {
                             Ok(_) => Ok(json!({"success": true, "message": format!("Scrolled {} by {}.", direction, amount)})),
                             Err(e) => Err(json!({"error": format!("Failed to scroll window: {}", e)})),
                         }
                     }
                 } else {
                     // No modifiers, normal scroll
                     info!(%direction, %amount, "Executing scroll_window");
                     match desktop.scroll_at_current_position(&direction, amount) {
                         Ok(_) => Ok(json!({"success": true, "message": format!("Scrolled {} by {}.", direction, amount)})),
                         Err(e) => Err(json!({"error": format!("Failed to scroll window: {}", e)})),
                     }
                 }
            }
            "capture_screenshot" => {
                #[cfg(target_os = "macos")]
                {
                    match macos_utils::capture_and_encode_screenshot() {
                        Ok(base64_string) => {
                            app_handle.notification().builder().title("Screenshot").body("Screenshot captured.").show().ok();
                            // Return the raw base64 string for processing in submit_query
                            Ok(json!({"success": true, "screenshot_base64": base64_string}))
                        },
                        Err(e) => Err(json!({"error": format!("Failed to capture screenshot: {}", e)})),
                    }
                }
                #[cfg(not(target_os = "macos"))]
                {
                     Err(json!({"error": "Screenshot capture is only supported on macOS currently."}))
                }
            }
             "capture_element_screenshot" => {
                #[cfg(target_os = "macos")]
                {
                     match desktop.focused_element() { // Changed from get_focused_element
                         Ok(focused_element) => {
                            if let Some(macos_element) = focused_element.as_any().downcast_ref::<MacOSUIElement>() {
                                 match macos_utils::capture_element_screenshot(macos_element) {
                                    Ok(base64_string) => {
                                         app_handle.notification().builder().title("Element Screenshot").body("Focused element screenshot captured.").show().ok();
                                         // Return the raw base64 string for processing in submit_query
                                         Ok(json!({"success": true, "screenshot_base64": base64_string}))
                                    },
                                    Err(e) => Err(json!({"error": format!("Failed to capture element screenshot: {}", e)})),
                                 }
                            } else {
                                Err(json!({"error": "Focused element is not a MacOSUIElement"}))
                            }
                        },
                        Err(e) => Err(json!({"error": format!("Failed to get focused element for screenshot: {}", e)})),
                    }
                }
                 #[cfg(not(target_os = "macos"))]
                {
                     Err(json!({"error": "Element screenshot capture is only supported on macOS currently."}))
                }
            }
            // --- Added Tool Handlers ---
            "wait" => {
                 match get_u64_param(input, "duration_ms") {
                     Ok(duration_ms) => match desktop.wait(duration_ms) {
                         Ok(_) => Ok(json!({"success": true, "message": format!("Waited for {} ms.", duration_ms)})),
                         Err(e) => Err(json!({"error": format!("Wait failed: {}", e)})),
                     },
                     Err(e) => Err(e),
                }
            }
            "cursor_position" => {
                match desktop.cursor_position() {
                    Ok((x, y)) => Ok(json!({"success": true, "x": x, "y": y})),
                    Err(e) => Err(json!({"error": format!("Failed to get cursor position: {}", e)})),
                }
            }
            "mouse_move" => {
                match (get_f64_param(input, "x"), get_f64_param(input, "y")) {
                    (Ok(x), Ok(y)) => match desktop.mouse_move(x, y) {
                        Ok(_) => Ok(json!({"success": true, "message": format!("Mouse moved to ({}, {}).", x, y)})),
                        Err(e) => Err(json!({"error": format!("Failed to move mouse: {}", e)})),
                    },
                    (Err(e), _) | (_, Err(e)) => Err(e),
                }
            }
             "left_mouse_down" => {
                 match (get_f64_param(input, "x"), get_f64_param(input, "y")) {
                    (Ok(x), Ok(y)) => match desktop.left_mouse_down(x, y) {
                        Ok(_) => Ok(json!({"success": true, "message": "Left mouse button pressed down."})),
                        Err(e) => Err(json!({"error": format!("Failed to press left mouse button down: {}", e)})),
                    },
                    (Err(e), _) | (_, Err(e)) => Err(e),
                }
            }
            "left_mouse_up" => {
                match (get_f64_param(input, "x"), get_f64_param(input, "y")) {
                    (Ok(x), Ok(y)) => match desktop.left_mouse_up(x, y) {
                         Ok(_) => Ok(json!({"success": true, "message": "Left mouse button released."})),
                         Err(e) => Err(json!({"error": format!("Failed to release left mouse button: {}", e)})),
                     },
                     (Err(e), _) | (_, Err(e)) => Err(e),
                }
            }
            "left_click" => {
                 let x = get_f64_param(input, "x")?;
                 let y = get_f64_param(input, "y")?;
                 // Extract optional modifier keys
                 let modifier_keys_val = input.get("modifier_keys");
                 let modifier_keys: Option<Vec<String>> = modifier_keys_val
                    .and_then(|v| v.as_array())
                    .map(|arr| arr.iter().filter_map(|v| v.as_str().map(String::from)).collect());

                 if let Some(keys) = modifier_keys {
                     if !keys.is_empty() {
                         info!(x = %x, y = %y, ?keys, "Executing left click with modifiers");
                         // Hold keys
                         for key in &keys {
                             if let Err(e) = desktop.hold_key(key) {
                                 // Attempt to release any already held keys before returning error
                                 for held_key in keys.iter().take_while(|&k| k != key) {
                                     desktop.release_key(held_key).ok(); // Ignore release error during cleanup
                                 }
                                 return Err(json!({"error": format!("Failed to hold modifier key '{}': {}", key, e)}));
                             }
                         }

                         // Perform click
                         let click_result = desktop.left_click(x, y);

                         // Release keys (attempt regardless of click result)
                         let mut release_errors = Vec::new();
                         for key in keys.iter().rev() { // Release in reverse order
                             if let Err(e) = desktop.release_key(key) {
                                 release_errors.push(format!("Failed to release key '{}': {}", key, e));
                             }
                         }

                         // Handle results
                         match click_result {
                             Ok(_) if release_errors.is_empty() => Ok(json!({"success": true, "message": format!("Left clicked at ({}, {}) with modifiers {:?}.", x, y, keys)})),
                             Ok(_) => Err(json!({"error": format!("Click succeeded, but failed to release modifiers: {}", release_errors.join(", "))})),
                             Err(e) if release_errors.is_empty() => Err(json!({"error": format!("Click failed: {}. Modifiers released.", e)})),
                             Err(e) => Err(json!({"error": format!("Click failed: {}. Also failed to release modifiers: {}", e, release_errors.join(", "))})),
                         }
                     } else {
                         // Modifiers array provided but empty, treat as normal click
                         info!(x = %x, y = %y, "Executing left click (empty modifiers list)");
                         match desktop.left_click(x, y) {
                             Ok(_) => Ok(json!({"success": true, "message": format!("Left clicked at ({}, {}).", x, y)})),
                             Err(e) => Err(json!({"error": format!("Failed to perform left click: {}", e)})),
                         }
                     }
                 } else {
                     // No modifiers provided, perform normal click
                     info!(x = %x, y = %y, "Executing left click");
                     match desktop.left_click(x, y) {
                         Ok(_) => Ok(json!({"success": true, "message": format!("Left clicked at ({}, {}).", x, y)})),
                         Err(e) => Err(json!({"error": format!("Failed to perform left click: {}", e)})),
                     }
                 }
            }
            "right_click" => {
                let x = get_f64_param(input, "x")?;
                let y = get_f64_param(input, "y")?;
                info!(x = %x, y = %y, "Executing right click");
                match desktop.right_click(x, y) {
                    Ok(_) => Ok(json!({"success": true, "message": format!("Right clicked at ({}, {}).", x, y)})),
                    Err(e) => Err(json!({"error": format!("Failed to perform right click: {}", e)})),
                }
            }
            "middle_click" => {
                 match (get_f64_param(input, "x"), get_f64_param(input, "y")) {
                     (Ok(x), Ok(y)) => match desktop.middle_click(x, y) {
                         Ok(_) => Ok(json!({"success": true, "message": format!("Middle clicked at ({}, {}).", x, y)})),
                         Err(e) => Err(json!({"error": format!("Failed to perform middle click: {}", e)})),
                     },
                     (Err(e), _) | (_, Err(e)) => Err(e),
                 }
            }
            "double_click" => {
                let x = get_f64_param(input, "x")?;
                let y = get_f64_param(input, "y")?;
                match desktop.double_click(x, y) {
                    Ok(_) => Ok(json!({"success": true, "message": format!("Double clicked at ({}, {})", x, y)})),
                    Err(e) => Err(json!({"error": e.to_string()})),
                }
            }
            "triple_click" => {
                let x = get_f64_param(input, "x")?;
                let y = get_f64_param(input, "y")?;
                // Use the correct 'triple_click' method
                match desktop.triple_click(x, y) {
                    Ok(_) => Ok(json!({"success": true, "message": format!("Triple clicked at ({}, {})", x, y)})),
                    Err(e) => Err(json!({"error": e.to_string()})),
                }
            },
            "scroll_at_position" => { // Assuming Desktop has this method wrapping the engine call
                 let x = get_f64_param(input, "x")?;
                 let y = get_f64_param(input, "y")?;
                 let direction = get_string_param(input, "direction")?;
                 let amount = get_f64_param(input, "amount")?;
                 let modifier_keys = get_optional_modifier_keys(input)?;

                 if let Some(keys) = modifier_keys {
                     if !keys.is_empty() {
                         info!(%x, %y, %direction, %amount, ?keys, "Executing scroll_at_position with modifiers");
                         hold_keys_and_run(desktop, &keys, || {
                             desktop.scroll_at_position(x, y, &direction, amount)
                         })
                         .map(|_| json!({ "success": true, "message": format!("Scrolled {} by {} at ({}, {}) with modifiers {:?}.", direction, amount, x, y, keys) }))
                     } else {
                         // Empty modifier list, normal scroll
                         info!(%x, %y, %direction, %amount, "Executing scroll_at_position (empty modifiers list)");
                         match desktop.scroll_at_position(x, y, &direction, amount) {
                             Ok(_) => Ok(json!({"success": true, "message": format!("Scrolled {} by {} at ({}, {}).", direction, amount, x, y)})),
                             Err(e) => Err(json!({"error": format!("Failed to scroll at position: {}", e)})),
                         }
                     }
                 } else {
                     // No modifiers, normal scroll
                     info!(%x, %y, %direction, %amount, "Executing scroll_at_position");
                     match desktop.scroll_at_position(x, y, &direction, amount) {
                         Ok(_) => Ok(json!({"success": true, "message": format!("Scrolled {} by {} at ({}, {}).", direction, amount, x, y)})),
                         Err(e) => Err(json!({"error": format!("Failed to scroll at position: {}", e)})),
                     }
                 }
            }
             "hold_key" => {
                match get_string_param(input, "key") {
                     Ok(key) => match desktop.hold_key(&key) {
                        Ok(_) => Ok(json!({"success": true, "message": format!("Holding key '{}'.", key)})),
                        Err(e) => Err(json!({"error": format!("Failed to hold key: {}", e)})),
                    },
                    Err(e) => Err(e),
                }
            }
            "release_key" => {
                 match get_string_param(input, "key") {
                     Ok(key) => match desktop.release_key(&key) {
                        Ok(_) => Ok(json!({"success": true, "message": format!("Released key '{}'.", key)})),
                        Err(e) => Err(json!({"error": format!("Failed to release key: {}", e)})),
                    },
                    Err(e) => Err(e),
                }
            }
            // --- Text Editor Handlers ---
            "text_editor_view" => {
                 let file_path = get_string_param(input, "file_path")?;
                 // Get optional line numbers (use our helper for optional u64)
                 let start_line_opt = get_optional_u64_param(input, "start_line")?;
                 let end_line_opt = get_optional_u64_param(input, "end_line")?;

                 match (start_line_opt, end_line_opt) {
                     (Some(start), Some(end)) => {
                         // Both start and end provided: Read range
                         info!(path = %file_path, start=start, end=end, "Reading file range");
                         if start == 0 || end == 0 || start > end {
                             return Err(json!({"error": format!("Invalid line range: start ({}) must be >= 1 and <= end ({}).", start, end)}));
                         }
                         match fs::read_to_string(&file_path) {
                             Ok(content) => {
                                 let lines: Vec<&str> = content.lines().collect();
                                 let total_lines = lines.len();
                                 // Adjust to 0-based index, clamp to bounds
                                 let start_idx = (start - 1).clamp(0, total_lines as u64) as usize;
                                 let end_idx = (end).clamp(0, total_lines as u64) as usize; // end is exclusive for slice

                                 if start_idx >= end_idx {
                                     // Handle case where start is beyond end after clamping (e.g., start > total_lines)
                                     Ok(json!({ "success": true, "content": "", "message": "Specified range is empty or invalid for this file." }))
                                 } else {
                                     let range_content = lines[start_idx..end_idx].join("\n");
                                     Ok(json!({"success": true, "content": range_content}))
                                 }
                             }
                             Err(e) => Err(json!({"error": format!("Failed to read file '{}': {}", file_path, e)})),
                         }
                     }
                     (None, None) => {
                         // Neither provided: Read whole file
                         info!(path = %file_path, "Reading whole file");
                         match fs::read_to_string(&file_path) {
                             Ok(content) => Ok(json!({"success": true, "content": content})),
                             Err(e) => Err(json!({"error": format!("Failed to read file '{}': {}", file_path, e)})),
                         }
                     }
                     _ => {
                         // Only one provided: Invalid combination
                         Err(json!({"error": "Both start_line and end_line must be provided together, or neither."}))
                     }
                 }
            }
            "text_editor_create" => {
                 match (get_string_param(input, "file_path"), get_string_param(input, "content")) {
                     (Ok(file_path), Ok(content)) => {
                        // --- Undo State Update ---
                        let path = PathBuf::from(file_path.clone());
                        crate::state::update_undo_state(state, path, None); // Use crate::state::
                        // --- End Undo State Update ---
                        match fs::write(&file_path, content) {
                            Ok(_) => Ok(json!({"success": true, "message": format!("File '{}' created/overwritten.", file_path)})),
                            Err(e) => Err(json!({"error": format!("Failed to write file '{}': {}", file_path, e)})),
                        }
                     },
                     (Err(e), _) | (_, Err(e)) => Err(e),
                 }
            }
            "text_editor_insert" => {
                  match (
                     get_string_param(input, "file_path"),
                     get_string_param(input, "text_to_insert"),
                     get_i64_param(input, "line_number")
                  ) {
                     (Ok(file_path), Ok(text_to_insert), Ok(line_number)) => {
                        let line_usize = line_number as usize;
                        // --- Undo State Update ---
                        let path = PathBuf::from(file_path.clone());
                        // Read current content *before* modification
                        let current_content = match fs::read_to_string(&path) {
                             Ok(content) => Some(content),
                             Err(e) => {
                                  // If the file doesn't exist, it's an error for insert, but technically the previous state is "doesn't exist"
                                  warn!(error = %e, file_path = %file_path, "File not found for insert, proceeding but undo will delete.");
                                  None
                             }
                        };
                        crate::state::update_undo_state(state, path.clone(), current_content); // Use crate::state::
                         // --- End Undo State Update ---
                        match fs::read_to_string(&file_path) {
                            Ok(content) => {
                                let mut lines: Vec<String> = content.lines().map(String::from).collect();
                                 if line_usize == 0 || line_usize > lines.len() + 1 { // Allow inserting at the end (len + 1)
                                    lines.push(text_to_insert);
                                } else {
                                    lines.insert(line_usize - 1, text_to_insert);
                                }
                                let new_content = lines.join("\n");
                                match fs::write(&file_path, new_content) {
                                    Ok(_) => Ok(json!({"success": true, "message": format!("Inserted text into '{}' at line {}.", file_path, line_usize)})),
                                    Err(e) => Err(json!({"error": format!("Failed to write updated file '{}': {}", file_path, e)})),
                                }
                            }
                            Err(e) if e.kind() == std::io::ErrorKind::NotFound => { // File doesn't exist, create it
                                 match fs::write(&file_path, text_to_insert) {
                                    Ok(_) => Ok(json!({"success": true, "message": format!("Created file '{}' with inserted text.", file_path)})),
                                    Err(write_err) => Err(json!({"error": format!("Failed to create file '{}' for insert: {}", file_path, write_err)})),
                                }
                            },
                            Err(e) => Err(json!({"error": format!("Failed to read file '{}' for insert: {}", file_path, e)})),
                        }
                     }
                     (Err(e), _, _) | (_, Err(e), _) | (_, _, Err(e)) => Err(e),
                  }
            }
             "text_editor_str_replace" => {
                 match (
                    get_string_param(input, "file_path"),
                    get_string_param(input, "find_text"),
                    get_string_param(input, "replace_text")
                 ) {
                     (Ok(file_path), Ok(find_text), Ok(replace_text)) => {
                        // --- Undo State Update ---
                        let path = PathBuf::from(file_path.clone());
                        let current_content = match fs::read_to_string(&path) {
                            Ok(content) => Some(content),
                            Err(e) if e.kind() == std::io::ErrorKind::NotFound => None, // File doesn't exist yet, treat as create
                            Err(e) => return Err(json!({ "status": "error", "message": format!("Failed to read file '{}' before replace: {}", file_path, e) })),
                        };
                        crate::state::update_undo_state(state, path.clone(), current_content); // Use crate::state::
                        // --- End Undo State Update ---

                        match str_replace_editor(file_path.clone(), find_text, replace_text) {
                             Ok(msg) => Ok(json!({"success": true, "message": msg})),
                             Err(e) => Err(json!({"error": format!("Failed to replace text in file '{}': {}", file_path, e)})),
                        }
                     }
                      (Err(e), _, _) | (_, Err(e), _) | (_, _, Err(e)) => Err(e),
                 }
            }
            "text_editor_undo_edit" => {
                let file_path_param = get_string_param(input, "file_path")?; // Get param for logging/confirmation if needed

                let mut last_edited_path_guard = state.last_edited_file.lock().unwrap();
                let mut previous_content_guard = state.previous_content.lock().unwrap();

                if let Some(path_to_undo) = last_edited_path_guard.take() {
                     // Verify param matches state if desired, though state is source of truth
                    if PathBuf::from(&file_path_param) != path_to_undo {
                        warn!(param_path=%file_path_param, state_path=?path_to_undo, "Undo called with path mismatch, using state path.");
                     }

                    if let Some(maybe_content) = previous_content_guard.take() {
                        match maybe_content {
                            Some(content) => {
                                // Last action was an edit, restore content
                                match fs::write(&path_to_undo, content) {
                                    Ok(_) => Ok(json!({ "status": "success", "message": format!("Undo successful for '{}'.", path_to_undo.display()) })),
                                    Err(e) => Err(json!({ "status": "error", "message": format!("Failed to write previous content during undo for '{}': {}", path_to_undo.display(), e) })),
                                }
                            }
                            None => {
                                // Last action was create, delete the file
                                match fs::remove_file(&path_to_undo) {
                                    Ok(_) => Ok(json!({ "status": "success", "message": format!("Undo successful for '{}' (file deleted).", path_to_undo.display()) })),
                                    // If it already doesn't exist, that's okay for undoing a create
                                    Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                                         Ok(json!({ "status": "success", "message": format!("Undo successful for '{}' (file was already deleted).", path_to_undo.display()) }))
                                    }
                                    Err(e) => Err(json!({ "status": "error", "message": format!("Failed to delete file during undo for '{}': {}", path_to_undo.display(), e) })),
                                }
                            }
                        }
                    } else {
                         // Should not happen if last_edited_path was Some, indicates state inconsistency
                         error!("Undo state inconsistency: last_edited_file was Some, but previous_content was None.");
                         Err(json!({ "status": "error", "message": "Internal error: Undo state inconsistent." }))
                     }

                } else {
                    Err(json!({ "status": "error", "message": "Nothing to undo." }))
                }
            }
            // --- Bash Handler ---
            "bash" => {
                match (get_string_param(input, "command"), get_optional_u64_param(input, "timeout_seconds"), get_optional_bool_param(input, "restart")) {
                    (Ok(command), Ok(timeout_seconds), Ok(restart_opt)) => {
                        let restart = restart_opt.unwrap_or(false);
                        info!(command = %command, timeout = ?timeout_seconds, restart = restart, "Executing bash command");

                        // TODO: Implement proper shell state management for 'restart' if needed.
                        if restart {
                            warn!("Bash 'restart' parameter is noted but full shell state reset is not yet implemented.");
                            // Potentially clear environment variables or reset CWD if state is managed here
                        }

                        let mut cmd = Command::new("sh"); // Using sh -c for broader compatibility
                        cmd.arg("-c");
                        cmd.arg(&command);

                        // Basic timeout handling with std::process, more robust handling might need wait-timeout crate
                        let timeout_duration = timeout_seconds.map(Duration::from_secs);

                        // Spawn the child process
                        match cmd.spawn() {
                             Ok(mut child) => {
                                let status_result = if let Some(duration) = timeout_duration {
                                    match child.wait_timeout(duration) {
                                        Ok(Some(status)) => Ok(status),
                                        Ok(None) => { // Timeout occurred
                                            warn!(command = %command, "Command timed out, killing process...");
                                            child.kill().map_err(|e| format!("Failed to kill timed out process: {}", e))?;
                                            child.wait().map_err(|e| format!("Failed to wait on killed process: {}", e))
                                            // Return a specific status or error indicating timeout
                                            // For now, just return the wait status after kill
                                        }
                                        Err(e) => Err(format!("Failed to wait for child process with timeout: {}", e)),
                                    }
                                } else {
                                     child.wait().map_err(|e| format!("Failed to wait for child process: {}", e))
                                };

                                match status_result {
                                    Ok(status) => {
                                        // Attempt to read stdout/stderr after process exits
                                        // Note: Reading from pipes of a finished process might be tricky/platform-dependent
                                        // Consider using libraries like `duct` or managing pipes explicitly if needed.
                                        // For now, we return exit code and timeout status.
                                        let timed_out = timeout_duration.is_some() && !status.success() && status.code().is_none(); // Heuristic for timeout

                                        Ok(json!({
                                            "success": status.success(),
                                            "stdout": "(stdout not captured in this implementation)", // Placeholder
                                            "stderr": "(stderr not captured in this implementation)", // Placeholder
                                            "exit_code": status.code(),
                                            "timed_out": timed_out
                                        }))
                                    },
                                    Err(e) => Err(json!({ "error": e }))
                                }
                            }
                            Err(e) => Err(json!({"error": format!("Failed to spawn command '{}': {}", command, e)}))
                        }
                    }
                    (Err(e), _, _) | (_, Err(e), _) | (_, _, Err(e)) => Err(json!({ "error": format!("Failed to parse command arguments: {}", e) })), // Corrected structure
                }
            }
            // --- Standard Tool Implementations ---
             "read_file" => { // Reverted to use std::fs
                 match get_string_param(input, "path") {
                     Ok(path) => {
                         info!(path = %path, "Reading file");
                         match fs::read_to_string(&path) {
                             Ok(content) => Ok(json!({"success": true, "content": content})),
                             Err(e) => {
                                 error!(path = %path, error = %e, "Failed to read file");
                                 Err(json!({"error": format!("Failed to read file '{}': {}", path, e)}))
                             }
                         }
                     }
                     Err(e) => Err(e),
                 }
            }
            "write_file" => { // Reverted to use std::fs
                 match (get_string_param(input, "path"), get_string_param(input, "content")) {
                     (Ok(path), Ok(content)) => {
                         info!(path = %path, content_length = content.len(), "Writing file");
                         match fs::write(&path, &content) {
                            Ok(_) => Ok(json!({"success": true, "message": "File written."})),
                            Err(e) => {
                                error!(path = %path, error = %e, "Failed to write file");
                                Err(json!({"error": format!("Failed to write file '{}': {}", path, e)}))
                            }
                         }
                     }
                     (Err(e), _) | (_, Err(e)) => Err(e),
                 }
            }
            "get_element_by_description" => {
                let description = get_string_param(input, "description")?;
                info!(description = %description, "Executing get_element_by_description");
                // Reverted: Method not found on desktop object
                // match desktop.find_element_by_description(&description) {
                //     Ok(element_info) => Ok(json!({"success": true, "element": element_info})),
                //     Err(e) => Err(json!({"error": format!("Failed to find element by description: {}", e)})),
                // }
                Err(json!({ "error": "Tool 'get_element_by_description' not implemented yet." }))
            }
            "get_element_tree" => {
                info!("Executing get_element_tree");
                // Reverted: Method not found on desktop object
                //  match desktop.get_element_tree() {
                //     Ok(tree_info) => Ok(json!({"success": true, "tree": tree_info})),
                //     Err(e) => Err(json!({"error": format!("Failed to get element tree: {}", e)})),
                // }
                Err(json!({ "error": "Tool 'get_element_tree' not implemented yet." }))
            }
            "get_clipboard_content" => {
                 match desktop.get_clipboard_content() {
                    Ok(content) => Ok(json!({"success": true, "content": content})),
                    Err(e) => Err(json!({"error": format!("Failed to get clipboard content: {}", e)})),
                }
            }
            "set_clipboard_content" => {
                 match get_string_param(input, "content") {
                    Ok(content) => match desktop.set_clipboard_content(&content) {
                        Ok(_) => Ok(json!({"success": true, "message": "Clipboard content set."})),
                        Err(e) => Err(json!({"error": format!("Failed to set clipboard content: {}", e)})),
                    },
                    Err(e) => Err(e),
                }
            }
            // --- Newly Added Tool Handlers (Placeholders/Implementations) ---
            "get_browser_info" => {
                #[cfg(target_os = "macos")]
                {
                    info!("Executing get_browser_info for Safari");
                    // AppleScript to get URL and Title from the front Safari window's current tab
                    let script = r#"
                        tell application "Safari"
                            if it is running then
                                try
                                    set currentURL to URL of current tab of front window
                                    set currentTitle to name of current tab of front window
                                    return "{\"url\":\"" & currentURL & "\", \"title\":\"" & currentTitle & "\"}"
                                on error errMsg number errorNumber
                                    # Return error details as JSON string
                                    return "{\"error\": \"Failed to get Safari info: " & errMsg & " (" & (errorNumber as string) & ")\"}"
                                end try
                            else
                                return "{\"error\": \"Safari is not running\"}"
                            end if
                        end tell
                    "#;

                    match Command::new("osascript").arg("-e").arg(script).output() {
                        Ok(output) => {
                            let result_str = String::from_utf8_lossy(&output.stdout).trim().to_string();
                            if output.status.success() {
                                // Attempt to parse the JSON string returned by AppleScript
                                match serde_json::from_str::<Value>(&result_str) {
                                    Ok(json_value) => {
                                        // Check if the parsed JSON contains an error key (from the script itself)
                                        if json_value.get("error").is_some() {
                                            warn!(script_error = %result_str, "get_browser_info AppleScript reported an error");
                                            Err(json_value) // Return the error JSON from the script
                                        } else {
                                            Ok(json!({"success": true, "browser_info": json_value}))
                                        }
                                    },
                                    Err(parse_err) => {
                                         error!(output = %result_str, error = %parse_err, "Failed to parse AppleScript output as JSON");
                                         Err(json!({ "error": format!("Failed to parse browser info from script: {}", parse_err), "raw_output": result_str }))
                                    }
                                }
                            } else {
                                let stderr = String::from_utf8_lossy(&output.stderr).to_string();
                                warn!(stdout = %result_str, stderr = %stderr, "osascript execution failed for get_browser_info");
                                // Attempt to parse stdout as potential error JSON from script, otherwise use stderr
                                 match serde_json::from_str::<Value>(&result_str) {
                                     Ok(json_value) if json_value.get("error").is_some() => Err(json_value),
                                     _ => Err(json!({ "error": format!("osascript failed: {}", stderr), "stdout": result_str }))
                                }
                            }
                        }
                        Err(e) => {
                             error!(error = %e, "Failed to run osascript command for get_browser_info");
                             Err(json!({"error": format!("Failed to execute osascript for browser info: {}", e)}))
                        }
                    }
                 }
                 #[cfg(not(target_os = "macos"))]
                 {
                     Err(json!({ "error": "get_browser_info is only implemented for macOS (Safari) currently." }))
                 }
            }
            "run_applescript" => {
                #[cfg(target_os = "macos")]
                {
                     match get_string_param(input, "script") {
                        Ok(script) => {
                            info!(script = %script, "Executing AppleScript");
                            match Command::new("osascript").arg("-e").arg(&script).output() {
                                Ok(output) => {
                                    if output.status.success() {
                                        let stdout = String::from_utf8_lossy(&output.stdout).to_string();
                                        Ok(json!({"success": true, "result": stdout}))
                                    } else {
                                        let stderr = String::from_utf8_lossy(&output.stderr).to_string();
                                        warn!(stderr = %stderr, "AppleScript execution failed");
                                        Err(json!({"error": format!("AppleScript execution failed: {}", stderr)}))
                                    }
                                }
                                Err(e) => {
                                     error!(error = %e, "Failed to run osascript command");
                                     Err(json!({"error": format!("Failed to execute osascript: {}", e)}))
                                }
                             }
                         }
                         Err(e) => Err(e),
                     }
                 }
                 #[cfg(not(target_os = "macos"))]
                 {
                     Err(json!({"error": "run_applescript is only available on macOS."}))
                 }
            }
            "get_screen_text" => {
                 // This requires an OCR engine integration.
                Err(json!({"error": "Tool 'get_screen_text' not implemented yet."}))
            }
             // --- Unknown Tool ---
            _ => Err(json!({"error": format!("Unknown tool name: {}", tool_name)})),
        }
    };

    result.await
}

// --- Tool Call Wrapper ---

// Wrapper function to integrate call_tool result into Anthropic flow
#[allow(dead_code)] // Allow dead code for helper potentially used by submit_query
pub(crate) async fn handle_tool_call(
    desktop: &Arc<Desktop>,
    app_handle: &AppHandle,
    tool_name: &str,
    input: &Value,
    state: &State<'_, AppState>, // Added state parameter
) -> Value { // Returns the JSON expected by Anthropic (either success or error content)
    match call_tool(desktop, app_handle, tool_name, input, state).await { // Pass state
        Ok(success_json) => {
            info!(tool_name = %tool_name, output = ?success_json, "Tool call succeeded");
            success_json
        }
        Err(error_json) => {
            error!(tool_name = %tool_name, error = ?error_json, "Tool call failed");
            // Ensure the error JSON has an "error" field for consistency
            if error_json.get("error").is_some() {
                error_json
            } else {
                json!({"error": "An unexpected error occurred", "details": error_json})
            }
        }
    }
}

// --- Helper Function for Hold/Release Simulation ---
fn get_optional_modifier_keys(input: &Value) -> Result<Option<Vec<String>>, Value> {
    input.get("modifier_keys")
       .map_or(Ok(None), |v| {
           if v.is_null() {
               Ok(None)
           } else if let Some(arr) = v.as_array() {
               let keys = arr.iter()
                             .filter_map(|val| val.as_str().map(String::from))
                             .collect::<Vec<String>>();
                // Check if all elements were strings
                if keys.len() == arr.len() {
                     Ok(Some(keys))
                 } else {
                     Err(json!({ "error": "Invalid non-string value found in modifier_keys array" }))
                 }
           } else {
               Err(json!({ "error": "Invalid type for modifier_keys: expected an array of strings or null" }))
           }
       })
}

/// Holds specified keys, runs an action, then releases keys.
/// Returns the result of the action, or an error if holding/releasing fails.
fn hold_keys_and_run<F, T>(
    desktop: &Arc<Desktop>,
    keys: &[String],
    action: F,
) -> Result<T, Value>
where
    F: FnOnce() -> Result<T, computer_use_ai_sdk::AutomationError>,
{
    // Hold keys
    for key in keys {
        if let Err(e) = desktop.hold_key(key) {
            // Attempt to release any already held keys before returning error
            for held_key in keys.iter().take_while(|&k| k != key) {
                desktop.release_key(held_key).ok(); // Ignore release error during cleanup
            }
            return Err(json!({"error": format!("Failed to hold modifier key '{}': {}", key, e)}));
        }
    }

    // Perform action
    let action_result = action();

    // Release keys (attempt regardless of action result)
    let mut release_errors = Vec::new();
    for key in keys.iter().rev() { // Release in reverse order
        if let Err(e) = desktop.release_key(key) {
            release_errors.push(format!("Failed to release key '{}': {}", key, e));
        }
    }

    // Handle results
    match action_result {
        Ok(res) if release_errors.is_empty() => Ok(res),
        Ok(_) => Err(json!({"error": format!("Action succeeded, but failed to release modifiers: {}", release_errors.join(", "))})),
        Err(e) if release_errors.is_empty() => Err(json!({"error": format!("Action failed: {}. Modifiers released.", e)})),
        Err(e) => Err(json!({"error": format!("Action failed: {}. Also failed to release modifiers: {}", e, release_errors.join(", "))})),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_get_string_param_success() {
        let input = json!({ "key": "value" });
        assert_eq!(get_string_param(&input, "key").unwrap(), "value");
    }

    #[test]
    fn test_get_string_param_missing() {
        let input = json!({ "other_key": "value" });
        assert!(get_string_param(&input, "key").is_err());
    }

    #[test]
    fn test_get_string_param_wrong_type() {
        let input = json!({ "key": 123 });
        assert!(get_string_param(&input, "key").is_err());
    }

    #[test]
    fn test_get_string_param_null() {
        let input = json!({ "key": null });
        assert!(get_string_param(&input, "key").is_err());
    }
}

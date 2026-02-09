//! Desktop UI automation through accessibility APIs
//!
//! This module provides a cross-platform API for automating desktop applications
//! through accessibility APIs, inspired by Playwright's web automation model.

use anyhow::Result;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;
use std::sync::Arc;
use std::str::FromStr;
use tracing::{error, info};
use serde_json::{self, json, from_value};
use std::fs;
use std::process::Command;
use crate::platforms::AccessibilityEngine;

// Make element module public
pub mod element;
mod errors;
mod locator;
pub mod platforms;
mod selector;
#[cfg(test)]
mod tests;

// Now UIElement is publicly accessible via computer_use_ai_sdk::element::UIElement
// We still re-export it for convenience
pub use element::{ElementTreeNode, UIElement, UIElementAttributes};
pub use errors::AutomationError;
pub use locator::Locator;
pub use selector::Selector;

// Log Entry Struct
#[derive(Serialize, Deserialize, Clone)]
pub struct LogEntry {
    timestamp: u64,
    pub level: String,
    pub message: String,
}

// --- Tool Definition Structures (for Anthropic) ---

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ToolInputSchema {
    #[serde(rename = "type")]
    pub type_: String, // Typically "object"
    pub properties: HashMap<String, ToolParameter>,
    pub required: Vec<String>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ToolParameter {
    #[serde(rename = "type")]
    pub type_: String, // e.g., "string", "number", "boolean"
    pub description: String,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ToolDefinition {
    pub name: String,
    pub description: String,
    pub input_schema: ToolInputSchema,
}

// --- End Tool Definition Structures ---

// Define a new struct to hold click result information - move to module level
#[derive(Debug)]
pub struct ClickResult {
    pub method: String,
    pub coordinates: Option<(f64, f64)>,
    pub details: String,
}

/// The main entry point for UI automation
#[derive(Clone)]
pub struct Desktop {
    engine: Arc<dyn platforms::AccessibilityEngine + Send + Sync>,
    #[allow(dead_code)] // Keep field, might be used for platform differences or config later
    use_background_apps: bool,
    #[allow(dead_code)] // Keep field, might be used for platform differences or config later
    activate_app: bool,
}

impl Desktop {
    /// Initializes the Desktop environment.
    pub fn new(use_background_apps: bool, activate_app: bool) -> Result<Self, AutomationError> {
        let engine_result = platforms::create_engine(use_background_apps, activate_app);

        match engine_result {
            Ok(engine) => {
                info!("Desktop engine initialized successfully.");
                Ok(Self {
                    engine: Arc::from(engine as Box<dyn AccessibilityEngine + Send + Sync>),
                    use_background_apps,
                    activate_app,
                })
            }
            Err(e) => {
                error!("Failed to initialize desktop engine: {}", e);
                Err(e)
            }
        }
    }

    /// Initializes the Desktop environment with auto-redirect permission handling.
    /// When permissions are denied, automatically opens System Settings for the user.
    pub fn new_with_auto_redirect(use_background_apps: bool, activate_app: bool, auto_open_settings: bool) -> Result<Self, AutomationError> {
        let engine_result = platforms::create_engine_with_auto_redirect(use_background_apps, activate_app, auto_open_settings);

        match engine_result {
            Ok(engine) => {
                info!("Desktop engine with auto-redirect initialized successfully.");
                Ok(Self {
                    engine: Arc::from(engine as Box<dyn AccessibilityEngine + Send + Sync>),
                    use_background_apps,
                    activate_app,
                })
            }
            Err(e) => {
                error!("Failed to initialize desktop engine with auto-redirect: {}", e);
                Err(e)
            }
        }
    }

    /// Returns a reference to the underlying accessibility engine.
    pub fn engine(&self) -> Arc<dyn platforms::AccessibilityEngine + Send + Sync> {
        self.engine.clone()
    }

    /// Get the root UI element representing the entire desktop
    pub fn root(&self) -> UIElement {
        self.engine.get_root_element()
    }

    /// Create a locator to find elements matching the given selector
    pub fn locator(&self, selector: impl Into<Selector>) -> Locator {
        Locator::new(Arc::clone(&self.engine), selector.into())
    }

    /// Get the currently focused element
    pub fn focused_element(&self) -> Result<UIElement, AutomationError> {
        self.engine.get_focused_element()
    }

    /// List all running applications
    pub fn applications(&self) -> Result<Vec<UIElement>, AutomationError> {
        self.engine.get_applications()
    }

    /// Find an application by name
    pub fn application(&self, name: &str) -> Result<UIElement, AutomationError> {
        self.engine.get_application_by_name(name)
    }

    /// Open an application by name
    pub fn open_application(&self, app_name: &str) -> Result<UIElement, AutomationError> {
        self.engine.open_application(app_name)
    }

    /// Open a URL in a specified browser (or default browser if None)
    pub fn open_url(&self, url: &str, browser: Option<&str>) -> Result<UIElement, AutomationError> {
        self.engine.open_url(url, browser)
    }

    /// Type text globally using keyboard simulation.
    pub fn type_text(&self, text: &str) -> Result<(), AutomationError> {
        self.engine.type_text(text)
    }

    /// Get the current clipboard content
    pub fn get_clipboard_content(&self) -> Result<String, AutomationError> {
        self.engine.get_clipboard_content()
    }

    /// Set the clipboard content
    pub fn set_clipboard_content(&self, content: &str) -> Result<(), AutomationError> {
        self.engine.set_clipboard_content(content)
    }

    /// Hold down a modifier key.
    pub fn hold_key(&self, key: &str, duration_ms: Option<u64>) -> Result<(), AutomationError> {
        self.engine.hold_key(key, duration_ms)
    }

    /// Release a modifier key.
    pub fn release_key(&self, key: &str) -> Result<(), AutomationError> {
        self.engine.release_key(key)
    }

    /// Wait for a specified duration.
    pub fn wait(&self, duration_ms: u64) -> Result<(), AutomationError> {
        self.engine.wait(duration_ms)
    }

    /// Get the current mouse cursor position.
    pub fn cursor_position(&self) -> Result<(f64, f64), AutomationError> {
        self.engine.cursor_position()
    }

    /// Move the mouse cursor to the specified coordinates.
    pub fn mouse_move(&self, x: f64, y: f64) -> Result<(), AutomationError> {
        self.engine.mouse_move(x, y)
    }

    /// Simulate pressing the left mouse button down at the specified coordinates.
    pub fn left_mouse_down(&self, x: f64, y: f64) -> Result<(), AutomationError> {
        self.engine.left_mouse_down(x, y)
    }

    /// Simulate releasing the left mouse button at the specified coordinates.
    pub fn left_mouse_up(&self, x: f64, y: f64) -> Result<(), AutomationError> {
        self.engine.left_mouse_up(x, y)
    }

    /// Simulate a standard left click (down + up) at specified coordinates.
    pub fn left_click(&self, x: f64, y: f64, modifiers: Option<&str>) -> Result<(), AutomationError> {
        self.engine.left_click(x, y, modifiers)
    }

    /// Simulate a right click (down + up) at specified coordinates.
    pub fn right_click(&self, x: f64, y: f64, modifiers: Option<&str>) -> Result<(), AutomationError> {
        self.engine.right_click(x, y, modifiers)
    }

    /// Simulate a middle click (down + up) at specified coordinates.
    pub fn middle_click(&self, x: f64, y: f64, modifiers: Option<&str>) -> Result<(), AutomationError> {
        self.engine.middle_click(x, y, modifiers)
    }

    /// Simulate a double left click at the specified coordinates.
    pub fn double_click(&self, x: f64, y: f64, modifiers: Option<&str>) -> Result<(), AutomationError> {
        self.engine.double_click(x, y, modifiers)
    }

    /// Simulate a triple left click at the specified coordinates.
    pub fn triple_click(&self, x: f64, y: f64, modifiers: Option<&str>) -> Result<(), AutomationError> {
        self.engine.triple_click(x, y, modifiers)
    }

    /// Simulate dragging with the left mouse button from a start point to an end point.
    pub fn left_click_drag(
        &self,
        start_x: f64,
        start_y: f64,
        end_x: f64,
        end_y: f64,
    ) -> Result<(), AutomationError> {
        self.engine.left_click_drag(start_x, start_y, end_x, end_y)
    }

    /// Scroll at a specific position on screen
    pub fn scroll_at_position(&self, x: f64, y: f64, direction: &str, amount: f64) -> Result<(), AutomationError> {
        self.engine.scroll_at_position(x, y, direction, amount)
    }

    /// Scroll at the current mouse position
    pub fn scroll_at_current_position(&self, direction: &str, amount: f64) -> Result<(), AutomationError> {
        self.engine.scroll_at_current_position(direction, amount)
    }

    /// List all windows
    pub fn list_windows(&self) -> Result<Vec<UIElement>, AutomationError> {
        self.engine.list_windows()
    }

    /// Press a single key with an optional modifier
    pub fn press_key(&self, key_name: &str, modifier: Option<&str>) -> Result<(), AutomationError> {
        self.engine.press_key(key_name, modifier)
    }

    // --- New Methods for Agent Loop ---

    /// Returns the list of available tools for this Desktop instance.
    /// Used by: MCP server initialization and tool discovery.
    pub fn list_tools(&self) -> Vec<ToolDefinition> {
        let tools = vec![
            // --- OFFICIAL ANTHROPIC COMPUTER USE TOOLS ---
            // Following the official specification: https://docs.anthropic.com/en/docs/agents-and-tools/tool-use/computer-use-tool

            // Computer Tool (computer_20250124) - Single tool for all computer operations
            ToolDefinition {
                name: "computer".to_string(),
                description: "Use a mouse and keyboard to interact with a computer, and take screenshots. This is the official Anthropic Computer Use tool that handles all desktop interaction through action parameters.".to_string(),
                input_schema: ToolInputSchema {
                    type_: "object".to_string(),
                    properties: {
                        let mut props = HashMap::new();
                        props.insert(
                            "action".to_string(),
                            ToolParameter {
                                type_: "string".to_string(),
                                description: "The action to perform: screenshot, left_click, right_click, middle_click, double_click, triple_click, left_click_drag, mouse_move, left_mouse_down, left_mouse_up, type, key, hold_key, scroll, wait, cursor_position".to_string(),
                            },
                        );
                        props.insert(
                            "coordinate".to_string(),
                            ToolParameter {
                                type_: "array".to_string(),
                                description: "Array of [x, y] coordinates for click, mouse actions, and end position of drag actions. Required for mouse actions.".to_string(),
                            },
                        );
                        // Note: Removed legacy start_coordinate and end_coordinate parameters
                        // Following official Anthropic Computer Use specification:
                        // Drag operations use only 'coordinate' (end position) - start is current cursor position
                        props.insert(
                            "text".to_string(),
                            ToolParameter {
                                type_: "string".to_string(),
                                description: "Text to type or key combination to press (e.g., 'ctrl+s', 'Return').".to_string(),
                            },
                        );
                        props.insert(
                            "scroll_direction".to_string(),
                            ToolParameter {
                                type_: "string".to_string(),
                                description: "Direction to scroll: up, down, left, right.".to_string(),
                            },
                        );
                        props.insert(
                            "scroll_amount".to_string(),
                            ToolParameter {
                                type_: "number".to_string(),
                                description: "Amount to scroll (default: 3).".to_string(),
                            },
                        );
                        props.insert(
                            "duration".to_string(),
                            ToolParameter {
                                type_: "number".to_string(),
                                description: "Duration in milliseconds for wait or hold_key actions.".to_string(),
                            },
                        );
                        props
                    },
                    // Note: Only action is universally required. Other parameters are conditionally required based on action.
                    // This matches the official Anthropic Computer Use specification.
                    required: vec!["action".to_string()],
                },
            },

            // Text Editor Tool (str_replace_based_edit_tool) - Official Anthropic tool
            ToolDefinition {
                name: "str_replace_based_edit_tool".to_string(),
                description: "Custom editing tool for viewing, creating and editing files".to_string(),
                input_schema: ToolInputSchema {
                    type_: "object".to_string(),
                    properties: {
                        let mut props = HashMap::new();
                        props.insert(
                            "command".to_string(),
                            ToolParameter {
                                type_: "string".to_string(),
                                description: "The command to run: view, create, str_replace, insert".to_string(),
                            },
                        );
                        props.insert(
                            "path".to_string(),
                            ToolParameter {
                                type_: "string".to_string(),
                                description: "Absolute path to file or directory".to_string(),
                            },
                        );
                        props.insert(
                            "file_text".to_string(),
                            ToolParameter {
                                type_: "string".to_string(),
                                description: "Content for create command".to_string(),
                            },
                        );
                        props.insert(
                            "old_str".to_string(),
                            ToolParameter {
                                type_: "string".to_string(),
                                description: "String to replace in str_replace command".to_string(),
                            },
                        );
                        props.insert(
                            "new_str".to_string(),
                            ToolParameter {
                                type_: "string".to_string(),
                                description: "Replacement string for str_replace or text for insert command".to_string(),
                            },
                        );
                        props.insert(
                            "insert_line".to_string(),
                            ToolParameter {
                                type_: "integer".to_string(),
                                description: "Line number for insert command. Use 0 to insert at beginning, 1 to insert after line 1, 2 to insert after line 2, etc.".to_string(),
                            },
                        );
                        props
                    },
                    required: vec!["command".to_string(), "path".to_string()],
                },
            },

            // Bash Tool - Official Anthropic tool
            ToolDefinition {
                name: "bash".to_string(),
                description: "Run commands in a bash shell".to_string(),
                input_schema: ToolInputSchema {
                    type_: "object".to_string(),
                    properties: {
                        let mut props = HashMap::new();
                        props.insert(
                            "command".to_string(),
                            ToolParameter {
                                type_: "string".to_string(),
                                description: "The bash command to run".to_string(),
                            },
                        );
                        props.insert(
                            "restart".to_string(),
                            ToolParameter {
                                type_: "boolean".to_string(),
                                description: "Set to true to restart the bash environment".to_string(),
                            },
                        );
                        props
                    },
                    required: vec!["command".to_string()],
                },
            },

            // --- DESKTOP ACCESSIBILITY TOOLS ---
            // These are MCP-specific tools for accessibility interface, not covered by Anthropic Computer Use

            ToolDefinition {
                name: "getUiTree".to_string(),
                description: "Gets the UI tree structure for accessibility analysis. This provides structured access to UI elements beyond what screenshot analysis can provide.".to_string(),
                input_schema: ToolInputSchema {
                    type_: "object".to_string(),
                    properties: {
                        let mut props = HashMap::new();
                        props.insert(
                            "application_name".to_string(),
                            ToolParameter {
                                type_: "string".to_string(),
                                description: "Optional name of the application to get the tree for. If omitted, uses the focused application.".to_string(),
                            },
                        );
                        props
                    },
                    required: vec![], // application_name is optional
                },
            },
            ToolDefinition {
                name: "captureScreenshot".to_string(),
                description: "Captures a screenshot of the entire screen and returns it as a base64 encoded PNG.".to_string(),
                input_schema: ToolInputSchema {
                    type_: "object".to_string(),
                    properties: HashMap::new(),
                    required: Vec::new(),
                },
            },
            ToolDefinition {
                name: "getClipboard".to_string(),
                description: "Gets the current content of the system clipboard.".to_string(),
                input_schema: ToolInputSchema {
                    type_: "object".to_string(),
                    properties: HashMap::new(), // No input parameters
                    required: Vec::new(),
                },
            },
            ToolDefinition {
                name: "setClipboard".to_string(),
                description: "Sets the system clipboard to the specified text content.".to_string(),
                input_schema: ToolInputSchema {
                    type_: "object".to_string(),
                    properties: [(
                        "content".to_string(),
                        ToolParameter {
                            type_: "string".to_string(),
                            description: "The text content to set the clipboard to.".to_string(),
                        },
                    )]
                    .iter()
                    .cloned()
                    .collect(),
                    required: vec!["content".to_string()],
                },
            },
            ToolDefinition {
                name: "findElementsBySelector".to_string(),
                description: "Finds UI elements matching a specified selector (e.g., role, title, description). Returns a list of element attributes.".to_string(),
                input_schema: ToolInputSchema {
                    type_: "object".to_string(),
                    properties: {
                        let mut props = HashMap::new();
                        props.insert(
                            "selector".to_string(),
                            ToolParameter {
                                type_: "string".to_string(),
                                description: "The selector string (e.g., 'button[title=\"OK\"]').".to_string(),
                            },
                        );
                        // Optional root element ID might be added here later
                        props
                    },
                    required: vec!["selector".to_string()],
                },
            },
            ToolDefinition {
                name: "getElementAttributes".to_string(),
                description: "Gets the accessibility attributes of a UI element specified by a selector.".to_string(),
                input_schema: ToolInputSchema {
                    type_: "object".to_string(),
                    properties: {
                        let mut props = HashMap::new();
                        props.insert(
                            "selector".to_string(),
                            ToolParameter {
                                type_: "string".to_string(),
                                description: "The selector string for the element (e.g., 'button[title=\"OK\"]').".to_string(),
                            },
                        );
                        // Optional root element ID might be added here later
                        props
                    },
                    required: vec!["selector".to_string()],
                },
            },

            // --- APPLICATION MANAGEMENT TOOLS ---
            // These are system-level tools not covered by Anthropic Computer Use

            ToolDefinition {
                name: "open_application".to_string(),
                description: "Opens an application specified by its name.".to_string(),
                input_schema: ToolInputSchema {
                    type_: "object".to_string(),
                    properties: [
                        ("app_name".to_string(), ToolParameter {
                            type_: "string".to_string(),
                            description: "The name of the application to open (e.g., 'Safari', 'Terminal').".to_string(),
                        })
                    ].iter().cloned().collect(),
                    required: vec!["app_name".to_string()],
                },
            },
            ToolDefinition {
                name: "open_url".to_string(),
                description: "Opens a URL in the default web browser or a specified one.".to_string(),
                input_schema: ToolInputSchema {
                    type_: "object".to_string(),
                    properties: [
                        ("url".to_string(), ToolParameter {
                            type_: "string".to_string(),
                            description: "The full URL to open (e.g., 'https://www.google.com').".to_string(),
                        }),
                        ("browser".to_string(), ToolParameter {
                            type_: "string".to_string(),
                            description: "Optional. The name of the browser to use (e.g., 'Safari', 'Chrome'). Defaults to the system default browser if not specified.".to_string(),
                        })
                    ].iter().cloned().collect(),
                    required: vec!["url".to_string()],
                },
            },
            ToolDefinition {
                name: "get_focused_element_info".to_string(),
                description: "Gets information about the currently focused UI element on the screen.".to_string(),
                input_schema: ToolInputSchema {
                    type_: "object".to_string(),
                    properties: HashMap::new(), // No parameters needed
                    required: Vec::new(),
                },
            },

            // --- UI ELEMENT INTERACTION TOOLS ---
            // These provide selector-based interaction, complementing the coordinate-based computer tool

            ToolDefinition {
                name: "find_element".to_string(),
                description: "Finds a UI element based on a CSS-like selector.".to_string(),
                input_schema: ToolInputSchema {
                    type_: "object".to_string(),
                    properties: [
                        ("selector".to_string(), ToolParameter {
                            type_: "string".to_string(),
                            description: "The CSS-like selector string (e.g., 'window[title=\"Calculator\"] button[label=\"1\"]').".to_string(),
                        })
                    ].iter().cloned().collect(),
                    required: vec!["selector".to_string()],
                },
            },
            ToolDefinition {
                name: "click".to_string(),
                description: "Clicks on a UI element specified by a selector. Note: For coordinate-based clicking, use the 'computer' tool with action 'left_click'.".to_string(),
                input_schema: ToolInputSchema {
                    type_: "object".to_string(),
                    properties: [
                        ("selector".to_string(), ToolParameter {
                            type_: "string".to_string(),
                            description: "The selector for the element to click.".to_string(),
                        })
                    ].iter().cloned().collect(),
                    required: vec!["selector".to_string()],
                },
            },
            ToolDefinition {
                name: "type_text".to_string(),
                description: "Types text into a UI element specified by a selector. Note: For general text typing, use the 'computer' tool with action 'type'.".to_string(),
                input_schema: ToolInputSchema {
                    type_: "object".to_string(),
                    properties: [
                        ("selector".to_string(), ToolParameter {
                            type_: "string".to_string(),
                            description: "The selector for the element to type into.".to_string(),
                        }),
                        ("text".to_string(), ToolParameter {
                            type_: "string".to_string(),
                            description: "The text to type.".to_string(),
                        }),
                    ].iter().cloned().collect(),
                    required: vec!["selector".to_string(), "text".to_string()],
                },
            },
             ToolDefinition {
                name: "scroll_element".to_string(),
                description: "Scrolls a UI element specified by a selector. Note: For general scrolling, use the 'computer' tool with action 'scroll'.".to_string(),
                input_schema: ToolInputSchema {
                    type_: "object".to_string(),
                    properties: [
                        ("selector".to_string(), ToolParameter {
                            type_: "string".to_string(),
                            description: "The selector for the element to scroll.".to_string(),
                        }),
                        ("direction".to_string(), ToolParameter {
                            type_: "string".to_string(),
                            description: "The direction to scroll ('up', 'down', 'left', 'right').".to_string(),
                        }),
                        ("amount".to_string(), ToolParameter {
                            type_: "number".to_string(),
                            description: "The amount to scroll (default: 3).".to_string(),
                        }),
                    ].iter().cloned().collect(),
                    required: vec!["selector".to_string(), "direction".to_string()],
                },
            },
            ToolDefinition {
                name: "typeText".to_string(),
                description: "Types text at the current cursor position or into the focused element. Note: This is an alias for compatibility - prefer using the 'computer' tool with action 'type'.".to_string(),
                input_schema: ToolInputSchema {
                    type_: "object".to_string(),
                    properties: HashMap::from([
                        ("text".to_string(), ToolParameter { type_: "string".to_string(), description: "The text to type.".to_string() }),
                    ]),
                    required: vec!["text".to_string()],
                },
            },
        ];

        // REMOVED: All individual mouse and keyboard tools that don't follow Anthropic Computer Use spec
        // The following tools have been consolidated into the unified 'computer' tool:
        // - holdKey, releaseKey, pressKey -> computer tool with action: "hold_key", "key"
        // - mouseMove -> computer tool with action: "mouse_move"
        // - leftMouseDown, leftMouseUp -> computer tool with action: "left_mouse_down", "left_mouse_up"
        // - leftClick, rightClick, middleClick -> computer tool with action: "left_click", "right_click", "middle_click"
        // - doubleClick, tripleClick -> computer tool with action: "double_click", "triple_click"
        // - leftClickDrag -> computer tool with action: "left_click_drag"
        // - cursorPosition -> computer tool with action: "cursor_position"
        // - wait -> computer tool with action: "wait"
        //
        // This consolidation ensures 100% compliance with the official Anthropic Computer Use specification
        // while maintaining all functionality through the unified computer tool interface.

        tools
    }

    /// Call a specific tool by name with given arguments
    pub fn call_tool(&self, name: &str, args: Value) -> Result<Value, AutomationError> {
        info!("Calling tool: {} with args: {}", name, args);

        match name {
            "open_application" => {
                let app_name = args
                    .get("app_name")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| {
                        AutomationError::InvalidArgument(
                            "Missing or invalid 'app_name' argument".to_string(),
                        )
                    })?;
                self.open_application(app_name)?;
                Ok(
                    serde_json::json!({"status": "success", "message": format!("Application '{}' opened.", app_name)}),
                )
            }
            "open_url" => {
                let url = args.get("url").and_then(|v| v.as_str()).ok_or_else(|| {
                    AutomationError::InvalidArgument(
                        "Missing or invalid 'url' argument".to_string(),
                    )
                })?;
                let browser = args.get("browser").and_then(|v| v.as_str()); // Optional
                self.open_url(url, browser)?;
                Ok(
                    serde_json::json!({"status": "success", "message": format!("URL '{}' opened.", url)}),
                )
            }
            "typeText" => {
                let text = args["text"].as_str().ok_or_else(|| {
                    AutomationError::InvalidArgument("Missing or invalid 'text' argument".to_string())
                })?;
                self.type_text(text)?;
                Ok(Value::Null)
            }
            "get_focused_element_info" => {
                let element = self.focused_element()?;
                let attributes = element.attributes();
                // Convert attributes to JSON value
                let result_json = serde_json::to_value(attributes).map_err(|e| {
                    AutomationError::Internal(format!(
                        "Failed to serialize element attributes: {}",
                        e
                    ))
                })?;
                Ok(result_json)
            }
            "find_element" => {
                let selector_str =
                    args.get("selector")
                        .and_then(|v| v.as_str())
                        .ok_or_else(|| {
                            AutomationError::InvalidArgument(
                                "Missing or invalid 'selector' argument".to_string(),
                            )
                        })?;
                let selector: Selector = selector_str.into();
                let element_option = self.locator(selector).first()?;
                if let Some(element) = element_option {
                    let attributes = element.attributes();
                    let result_json = serde_json::to_value(attributes).map_err(|e| {
                        AutomationError::Internal(format!(
                            "Failed to serialize element attributes: {}",
                            e
                        ))
                    })?;
                    Ok(serde_json::json!({
                       "status": "success",
                       "element_found": true,
                       "attributes": result_json
                    }))
                } else {
                    Err(AutomationError::ElementNotFound(format!(
                        "Element not found for selector: {}",
                        selector_str
                    )))
                }
            }
            "click" => {
                let selector_str =
                    args.get("selector")
                        .and_then(|v| v.as_str())
                        .ok_or_else(|| {
                            AutomationError::InvalidArgument(
                                "Missing or invalid 'selector' argument".to_string(),
                            )
                        })?;
                let selector: Selector = selector_str.into();
                let element_option = self.locator(selector).first()?;
                if let Some(element) = element_option {
                    let click_result = element.click()?;
                    Ok(serde_json::json!({
                        "status": "success",
                        "message": format!("Clicked element matching selector '{}'. Method: {}, Details: {}", selector_str, click_result.method, click_result.details),
                        "coordinates": click_result.coordinates
                    }))
                } else {
                    Err(AutomationError::ElementNotFound(format!(
                        "Element not found for selector: {}",
                        selector_str
                    )))
                }
            }
            "type_text" => {
                let selector_str =
                    args.get("selector")
                        .and_then(|v| v.as_str())
                        .ok_or_else(|| {
                            AutomationError::InvalidArgument(
                                "Missing or invalid 'selector' argument".to_string(),
                            )
                        })?;
                let text_to_type = args.get("text").and_then(|v| v.as_str()).ok_or_else(|| {
                    AutomationError::InvalidArgument(
                        "Missing or invalid 'text' argument".to_string(),
                    )
                })?;
                let selector: Selector = selector_str.into();
                let element_option = self.locator(selector).first()?;
                if let Some(element) = element_option {
                    element.type_text(text_to_type)?;
                    Ok(serde_json::json!({
                        "status": "success",
                        "message": format!("Typed text into element matching selector '{}'", selector_str)
                    }))
                } else {
                    Err(AutomationError::ElementNotFound(format!(
                        "Element not found for selector: {}",
                        selector_str
                    )))
                }
            }
            "get_element_attributes" => {
                let selector_str =
                    args.get("selector")
                        .and_then(|v| v.as_str())
                        .ok_or_else(|| {
                            AutomationError::InvalidArgument(
                                "Missing or invalid 'selector' argument".to_string(),
                            )
                        })?;
                let selector: Selector = selector_str.into();
                let element_option = self.locator(selector).first()?;
                if let Some(element) = element_option {
                    let attributes = element.attributes();
                    let result_json = serde_json::to_value(attributes).map_err(|e| {
                        AutomationError::Internal(format!(
                            "Failed to serialize element attributes: {}",
                            e
                        ))
                    })?;
                    Ok(result_json)
                } else {
                    Err(AutomationError::ElementNotFound(format!(
                        "Element not found for selector: {}",
                        selector_str
                    )))
                }
            }
            "scroll_element" => {
                let selector_str =
                    args.get("selector")
                        .and_then(|v| v.as_str())
                        .ok_or_else(|| {
                            AutomationError::InvalidArgument(
                                "Missing or invalid 'selector' argument".to_string(),
                            )
                        })?;
                let direction =
                    args.get("direction")
                        .and_then(|v| v.as_str())
                        .ok_or_else(|| {
                            AutomationError::InvalidArgument(
                                "Missing or invalid 'direction' argument".to_string(),
                            )
                        })?;
                let amount = args.get("amount").and_then(|v| v.as_f64()).ok_or_else(|| {
                    AutomationError::InvalidArgument(
                        "Missing or invalid 'amount' argument".to_string(),
                    )
                })?;
                let selector: Selector = selector_str.into();
                let element_option = self.locator(selector).first()?;
                if let Some(element) = element_option {
                    element.scroll(direction, amount)?;
                    Ok(serde_json::json!({
                        "status": "success",
                        "message": format!("Scrolled element matching selector '{}'", selector_str)
                    }))
                } else {
                    Err(AutomationError::ElementNotFound(format!(
                        "Element not found for selector: {}",
                        selector_str
                    )))
                }
            }
            "getUiTree" => {
                #[derive(Deserialize)]
                struct GetUiTreeArgs {
                    application_name: Option<String>,
                }
                let parsed_args: GetUiTreeArgs = from_value(args).map_err(|e| AutomationError::InvalidArgument(format!("Failed to parse getUiTree args: {}", e)))?;
                self.engine.get_ui_tree(parsed_args.application_name.as_deref())
            }
            "findElementsBySelector" => {
                #[derive(Deserialize)]
                struct FindArgs {
                    selector: String,
                    // root_element_id: Option<String>, // TODO: Add support for root element ID
                }
                let parsed_args: FindArgs = from_value(args).map_err(|e| AutomationError::InvalidArgument(format!("Failed to parse findElementsBySelector args: {}", e)))?;
                let selector = Selector::from_str(&parsed_args.selector)?;
                // TODO: Implement finding root element by ID if provided
                let elements = self.engine.find_elements(&selector, None)?;
                let element_attributes: Vec<_> = elements.iter().map(|el| el.attributes()).collect();
                Ok(json!(element_attributes))
            }
            "getElementAttributes" => {
                #[derive(Deserialize)]
                struct GetAttributesArgs {
                    selector: String,
                    // root_element_id: Option<String>, // TODO: Add support for root element ID
                }
                let parsed_args: GetAttributesArgs = from_value(args).map_err(|e| AutomationError::InvalidArgument(format!("Failed to parse getElementAttributes args: {}", e)))?;
                let selector = Selector::from_str(&parsed_args.selector)?;
                // TODO: Implement finding root element by ID if provided
                let element = self.engine.find_element(&selector, None)?;
                Ok(json!(element.attributes()))
            }
            "captureScreenshot" => {
                // No arguments expected for captureScreenshot
                self.capture_screenshot_base64().map(|base64_str| json!({ "screenshot_base64": base64_str }))
            }
            "getClipboard" => {
                let content = self.get_clipboard_content()?;
                Ok(json!({ "content": content }))
            }
            "setClipboard" => {
                #[derive(Deserialize)]
                struct SetClipboardArgs {
                    content: String,
                }
                let parsed_args: SetClipboardArgs = from_value(args).map_err(|e| AutomationError::InvalidArgument(format!("Failed to parse setClipboard args: {}", e)))?;
                self.set_clipboard_content(&parsed_args.content)?;
                Ok(json!({ "status": "success" }))
            }
            "pressKey" => {
                #[derive(Deserialize)]
                struct PressKeyArgs {
                    key: String,
                    modifier: Option<String>,
                }
                let parsed_args: PressKeyArgs = from_value(args).map_err(|e| AutomationError::InvalidArgument(format!("Failed to parse pressKey args: {}", e)))?;
                self.press_key(&parsed_args.key, parsed_args.modifier.as_deref())?;
                Ok(json!({
                    "status": "success",
                    "details": format!("Pressed key '{}' with modifier '{}'", parsed_args.key, parsed_args.modifier.as_deref().unwrap_or("none"))
                }))
            }
            "holdKey" => {
                let key = args["key"].as_str().ok_or_else(|| {
                    AutomationError::InvalidArgument("Missing or invalid 'key' argument for holdKey".to_string())
                })?;
                let duration_ms = args.get("duration_ms").and_then(|v| v.as_u64());
                self.hold_key(key, duration_ms)?;
                Ok(Value::String(format!("Key '{}' held successfully.", key)))
            }
            "releaseKey" => {
                let key = args["key"].as_str().ok_or_else(|| {
                    AutomationError::InvalidArgument("Missing or invalid 'key' argument for releaseKey".to_string())
                })?;
                self.release_key(key)?;
                Ok(Value::String(format!("Key '{}' released successfully.", key)))
            }
            "wait" => {
                #[derive(Deserialize)]
                struct WaitArgs {
                    duration_ms: u64,
                }
                let args: WaitArgs = from_value(args).map_err(|e| AutomationError::InvalidArgument(format!("Error parsing wait args: {}", e)))?;
                self.wait(args.duration_ms)?;
                Ok(json!(null))
            }
            "cursorPosition" => {
                let (x, y) = self.cursor_position()?;
                Ok(json!({ "x": x, "y": y }))
            }
            "mouseMove" => {
                #[derive(Deserialize)]
                struct MouseMoveArgs { x: f64, y: f64 }
                let args: MouseMoveArgs = from_value(args).map_err(|e| AutomationError::InvalidArgument(format!("Error parsing mouseMove args: {}", e)))?;
                self.mouse_move(args.x, args.y)?;
                Ok(json!(null))
            }
            "leftMouseDown" => {
                #[derive(Deserialize)]
                struct MouseDownArgs { x: f64, y: f64 }
                let args: MouseDownArgs = from_value(args).map_err(|e| AutomationError::InvalidArgument(format!("Error parsing leftMouseDown args: {}", e)))?;
                self.left_mouse_down(args.x, args.y)?;
                Ok(json!(null))
            }
            "leftMouseUp" => {
                #[derive(Deserialize)]
                struct MouseUpArgs { x: f64, y: f64 }
                let args: MouseUpArgs = from_value(args).map_err(|e| AutomationError::InvalidArgument(format!("Error parsing leftMouseUp args: {}", e)))?;
                self.left_mouse_up(args.x, args.y)?;
                Ok(json!(null))
            }
            "leftClick" => {
                #[derive(Deserialize)]
                struct ClickArgs { x: f64, y: f64 }
                let args: ClickArgs = from_value(args).map_err(|e| AutomationError::InvalidArgument(format!("Error parsing leftClick args: {}", e)))?;
                self.left_click(args.x, args.y, None)?;
                Ok(json!(null))
            }
            "rightClick" => {
                #[derive(Deserialize)]
                struct ClickArgs { x: f64, y: f64 }
                let args: ClickArgs = from_value(args).map_err(|e| AutomationError::InvalidArgument(format!("Error parsing rightClick args: {}", e)))?;
                self.right_click(args.x, args.y, None)?;
                Ok(json!(null))
            }
            "middleClick" => {
                #[derive(Deserialize)]
                struct ClickArgs { x: f64, y: f64 }
                let args: ClickArgs = from_value(args).map_err(|e| AutomationError::InvalidArgument(format!("Error parsing middleClick args: {}", e)))?;
                self.middle_click(args.x, args.y, None)?;
                Ok(json!(null))
            }
            "doubleClick" => {
                #[derive(Deserialize)]
                struct ClickArgs { x: f64, y: f64 }
                let args: ClickArgs = from_value(args).map_err(|e| AutomationError::InvalidArgument(format!("Error parsing doubleClick args: {}", e)))?;
                self.double_click(args.x, args.y, None)?;
                Ok(json!(null))
            }
            "tripleClick" => {
                #[derive(Deserialize)]
                struct ClickArgs { x: f64, y: f64 }
                let args: ClickArgs = from_value(args).map_err(|e| AutomationError::InvalidArgument(format!("Error parsing tripleClick args: {}", e)))?;
                self.triple_click(args.x, args.y, None)?;
                Ok(json!(null))
            }

            // --- Text Editor Handlers ---
            "text_editor_view" => {
                #[derive(Deserialize)]
                struct TextViewArgs { file_path: String }
                let parsed_args: TextViewArgs = from_value(args).map_err(|e| AutomationError::InvalidArgument(format!("Error parsing text_editor_view args: {}", e)))?;
                match fs::read_to_string(&parsed_args.file_path) {
                    Ok(content) => Ok(json!({ "content": content })),
                    Err(e) => Err(AutomationError::Internal(format!("Failed to read file '{}': {}", parsed_args.file_path, e))),
                }
            }
            "text_editor_create" => {
                #[derive(Deserialize)]
                struct TextCreateArgs { file_path: String, content: Option<String> }
                let parsed_args: TextCreateArgs = from_value(args).map_err(|e| AutomationError::InvalidArgument(format!("Error parsing text_editor_create args: {}", e)))?;
                match fs::OpenOptions::new().write(true).create_new(true).open(&parsed_args.file_path) {
                    Ok(mut file) => {
                        use std::io::Write; // Import Write trait here
                        let content_to_write = parsed_args.content.unwrap_or_default();
                        match file.write_all(content_to_write.as_bytes()) {
                            Ok(_) => Ok(json!({ "status": format!("File '{}' created successfully.", parsed_args.file_path) })),
                            Err(e) => Err(AutomationError::Internal(format!("Failed to write initial content to file '{}': {}", parsed_args.file_path, e))),
                        }
                    }
                    Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {
                        Err(AutomationError::Internal(format!("File '{}' already exists. Cannot create.", parsed_args.file_path)))
                    }
                    Err(e) => {
                        Err(AutomationError::Internal(format!("Failed to create file '{}': {}", parsed_args.file_path, e)))
                    }
                }
            }
            "text_editor_str_replace" => {
                #[derive(Deserialize)]
                struct TextReplaceArgs { file_path: String, find: String, replace: String }
                let parsed_args: TextReplaceArgs = from_value(args).map_err(|e| AutomationError::InvalidArgument(format!("Error parsing text_editor_str_replace args: {}", e)))?;
                let content = match fs::read_to_string(&parsed_args.file_path) {
                    Ok(c) => c,
                    Err(e) => return Err(AutomationError::Internal(format!("Failed to read file '{}' for replacement: {}", parsed_args.file_path, e))),
                };
                let new_content = content.replace(&parsed_args.find, &parsed_args.replace);
                match fs::write(&parsed_args.file_path, new_content) {
                    Ok(_) => Ok(json!({ "status": format!("File '{}' updated successfully with replacements.", parsed_args.file_path) })),
                    Err(e) => Err(AutomationError::Internal(format!("Failed to write updated content to file '{}': {}", parsed_args.file_path, e))),
                }
            }
            "text_editor_insert" => {
                #[derive(Deserialize)]
                struct TextInsertArgs { file_path: String, text: String, line: Option<usize> }
                let parsed_args: TextInsertArgs = from_value(args).map_err(|e| AutomationError::InvalidArgument(format!("Error parsing text_editor_insert args: {}", e)))?;
                let content = match fs::read_to_string(&parsed_args.file_path) {
                    Ok(c) => c,
                    Err(e) => return Err(AutomationError::Internal(format!("Failed to read file '{}' for insertion: {}", parsed_args.file_path, e))),
                };

                let mut lines: Vec<String> = content.lines().map(|s| s.to_string()).collect();
                let insertion_point = match parsed_args.line {
                    // Convert 1-based line to 0-based index, clamp to valid range (0 to lines.len())
                    Some(line_num_1_based) => line_num_1_based.saturating_sub(1).min(lines.len()),
                    None => lines.len(), // Append if line is null
                };

                lines.insert(insertion_point, parsed_args.text);

                // Join lines, ensuring newline at the end if original content had one or if inserting into empty file
                let new_content = lines.join("\n");
                // A simple heuristic: add newline if original content ended with one or if it was empty
                let final_content = if content.ends_with('\n') || content.is_empty() {
                    format!("{}\n", new_content)
                } else {
                    new_content
                };

                match fs::write(&parsed_args.file_path, final_content) {
                    Ok(_) => Ok(json!({ "status": format!("Text inserted successfully into file '{}' at line {}.", parsed_args.file_path, insertion_point + 1) })),
                    Err(e) => Err(AutomationError::Internal(format!("Failed to write updated content to file '{}': {}", parsed_args.file_path, e))),
                }
            }
            // --- End Text Editor Handlers ---

            // --- Bash Handler ---
            "bash" => {
                #[derive(Deserialize)]
                struct BashArgs { command: String, timeout: Option<u64> } // Timeout in seconds
                let parsed_args: BashArgs = from_value(args).map_err(|e| AutomationError::InvalidArgument(format!("Error parsing bash args: {}", e)))?;

                // Basic implementation without timeout handling for now
                if parsed_args.timeout.is_some() {
                    info!("Bash tool timeout parameter specified but not yet implemented.");
                }

                // Determine shell based on OS
                let shell_cmd = if cfg!(target_os = "windows") {
                    ("cmd", vec!["/C".to_string(), parsed_args.command])
                } else {
                    // Assume Unix-like shell (sh)
                    ("sh", vec!["-c".to_string(), parsed_args.command])
                };

                match Command::new(shell_cmd.0).args(&shell_cmd.1).output() {
                    Ok(output) => {
                        let stdout = String::from_utf8_lossy(&output.stdout).to_string();
                        let stderr = String::from_utf8_lossy(&output.stderr).to_string();
                        let status_code = output.status.code();
                        Ok(json!({
                            "stdout": stdout,
                            "stderr": stderr,
                            "exit_code": status_code,
                            "success": output.status.success()
                        }))
                    }
                    Err(e) => {
                        Err(AutomationError::Internal(format!("Failed to execute bash command '{}': {}", shell_cmd.1.join(" "), e)))
                    }
                }
            }
            // --- End Bash Handler ---

            // --- Computer Tool Handler (Official Anthropic Computer Use) ---
            "computer" => {
                #[derive(Deserialize)]
                struct ComputerArgs {
                    action: String,
                    coordinate: Option<Vec<f64>>,
                    // Note: For drag operations, coordinate represents the end position
                    // Drag starts from current cursor position as per Anthropic Computer Use specification
                    text: Option<String>,
                    scroll_direction: Option<String>,
                    scroll_amount: Option<f64>,
                    duration: Option<u64>,
                }
                let parsed_args: ComputerArgs = from_value(args).map_err(|e| {
                    AutomationError::InvalidArgument(format!("Error parsing computer args: {}", e))
                })?;

                match parsed_args.action.as_str() {
                    "screenshot" => {
                        let base64_screenshot = self.capture_screenshot_base64()?;
                        Ok(json!({ "screenshot": base64_screenshot }))
                    }
                    "left_click" => {
                        let coords = parsed_args.coordinate.ok_or_else(|| {
                            AutomationError::InvalidArgument("coordinate required for left_click action".to_string())
                        })?;
                        if coords.len() != 2 {
                            return Err(AutomationError::InvalidArgument("coordinate must be [x, y] array".to_string()));
                        }
                        self.left_click(coords[0], coords[1], None)?;
                        Ok(json!({"status": "success"}))
                    }
                    "right_click" => {
                        let coords = parsed_args.coordinate.ok_or_else(|| {
                            AutomationError::InvalidArgument("coordinate required for right_click action".to_string())
                        })?;
                        if coords.len() != 2 {
                            return Err(AutomationError::InvalidArgument("coordinate must be [x, y] array".to_string()));
                        }
                        self.right_click(coords[0], coords[1], None)?;
                        Ok(json!({"status": "success"}))
                    }
                    "middle_click" => {
                        let coords = parsed_args.coordinate.ok_or_else(|| {
                            AutomationError::InvalidArgument("coordinate required for middle_click action".to_string())
                        })?;
                        if coords.len() != 2 {
                            return Err(AutomationError::InvalidArgument("coordinate must be [x, y] array".to_string()));
                        }
                        self.middle_click(coords[0], coords[1], None)?;
                        Ok(json!({"status": "success"}))
                    }
                    "double_click" => {
                        let coords = parsed_args.coordinate.ok_or_else(|| {
                            AutomationError::InvalidArgument("coordinate required for double_click action".to_string())
                        })?;
                        if coords.len() != 2 {
                            return Err(AutomationError::InvalidArgument("coordinate must be [x, y] array".to_string()));
                        }
                        self.double_click(coords[0], coords[1], None)?;
                        Ok(json!({"status": "success"}))
                    }
                    "triple_click" => {
                        let coords = parsed_args.coordinate.ok_or_else(|| {
                            AutomationError::InvalidArgument("coordinate required for triple_click action".to_string())
                        })?;
                        if coords.len() != 2 {
                            return Err(AutomationError::InvalidArgument("coordinate must be [x, y] array".to_string()));
                        }
                        self.triple_click(coords[0], coords[1], None)?;
                        Ok(json!({"status": "success"}))
                    }
                    "left_click_drag" => {
                        // Get current cursor position as start point (following Anthropic Computer Use specification)
                        let (start_x, start_y) = self.cursor_position()?;

                        // Get end coordinates from coordinate parameter
                        let end_coords = parsed_args.coordinate.ok_or_else(|| {
                            AutomationError::InvalidArgument("coordinate required for left_click_drag action".to_string())
                        })?;

                        if end_coords.len() != 2 {
                            return Err(AutomationError::InvalidArgument("coordinate must be [x, y] array".to_string()));
                        }

                        // Perform drag operation from current cursor position to specified coordinate
                        self.left_click_drag(start_x, start_y, end_coords[0], end_coords[1])?;
                        Ok(json!({
                            "status": "success",
                            "start": [start_x, start_y],
                            "end": end_coords
                        }))
                    }
                    "mouse_move" => {
                        let coords = parsed_args.coordinate.ok_or_else(|| {
                            AutomationError::InvalidArgument("coordinate required for mouse_move action".to_string())
                        })?;
                        if coords.len() != 2 {
                            return Err(AutomationError::InvalidArgument("coordinate must be [x, y] array".to_string()));
                        }
                        self.mouse_move(coords[0], coords[1])?;
                        Ok(json!({"status": "success"}))
                    }
                    "left_mouse_down" => {
                        let coords = parsed_args.coordinate.ok_or_else(|| {
                            AutomationError::InvalidArgument("coordinate required for left_mouse_down action".to_string())
                        })?;
                        if coords.len() != 2 {
                            return Err(AutomationError::InvalidArgument("coordinate must be [x, y] array".to_string()));
                        }
                        self.left_mouse_down(coords[0], coords[1])?;
                        Ok(json!({"status": "success"}))
                    }
                    "left_mouse_up" => {
                        let coords = parsed_args.coordinate.ok_or_else(|| {
                            AutomationError::InvalidArgument("coordinate required for left_mouse_up action".to_string())
                        })?;
                        if coords.len() != 2 {
                            return Err(AutomationError::InvalidArgument("coordinate must be [x, y] array".to_string()));
                        }
                        self.left_mouse_up(coords[0], coords[1])?;
                        Ok(json!({"status": "success"}))
                    }
                    "type" => {
                        let text = parsed_args.text.ok_or_else(|| {
                            AutomationError::InvalidArgument("text required for type action".to_string())
                        })?;
                        self.type_text(&text)?;
                        Ok(json!({"status": "success"}))
                    }
                    "key" => {
                        let key = parsed_args.text.ok_or_else(|| {
                            AutomationError::InvalidArgument("text (key combination) required for key action".to_string())
                        })?;
                        self.press_key(&key, None)?;
                        Ok(json!({"status": "success"}))
                    }
                    "hold_key" => {
                        let key = parsed_args.text.ok_or_else(|| {
                            AutomationError::InvalidArgument("text (key name) required for hold_key action".to_string())
                        })?;
                        self.hold_key(&key, parsed_args.duration)?;
                        Ok(json!({"status": "success"}))
                    }
                    "scroll" => {
                        let direction = parsed_args.scroll_direction.ok_or_else(|| {
                            AutomationError::InvalidArgument("scroll_direction required for scroll action".to_string())
                        })?;
                        let amount = parsed_args.scroll_amount.unwrap_or(3.0);
                        if let Some(coords) = parsed_args.coordinate {
                            if coords.len() != 2 {
                                return Err(AutomationError::InvalidArgument("coordinate must be [x, y] array".to_string()));
                            }
                            self.scroll_at_position(coords[0], coords[1], &direction, amount)?;
                        } else {
                            self.scroll_at_current_position(&direction, amount)?;
                        }
                        Ok(json!({"status": "success"}))
                    }
                    "wait" => {
                        let duration = parsed_args.duration.ok_or_else(|| {
                            AutomationError::InvalidArgument("duration required for wait action".to_string())
                        })?;
                        self.wait(duration)?;
                        Ok(json!({"status": "success"}))
                    }
                    "cursor_position" => {
                        let (x, y) = self.cursor_position()?;
                        Ok(json!({"x": x, "y": y}))
                    }
                    _ => {
                        Err(AutomationError::InvalidArgument(format!("Unknown computer action: {}", parsed_args.action)))
                    }
                }
            }
            // --- End Computer Tool Handler ---

            // --- str_replace_based_edit_tool Handler (Official Anthropic Tool) ---
            "str_replace_based_edit_tool" => {
                #[derive(Deserialize)]
                struct EditToolArgs {
                    command: String,
                    path: String,
                    file_text: Option<String>,
                    old_str: Option<String>,
                    new_str: Option<String>,
                    insert_line: Option<usize>,
                }
                let parsed_args: EditToolArgs = from_value(args).map_err(|e| {
                    AutomationError::InvalidArgument(format!("Error parsing str_replace_based_edit_tool args: {}", e))
                })?;

                match parsed_args.command.as_str() {
                    "view" => {
                        match fs::read_to_string(&parsed_args.path) {
                            Ok(content) => Ok(json!({ "content": content })),
                            Err(e) => Err(AutomationError::Internal(format!("Failed to read file '{}': {}", parsed_args.path, e))),
                        }
                    }
                    "create" => {
                        let content = parsed_args.file_text.ok_or_else(|| {
                            AutomationError::InvalidArgument("file_text required for create command".to_string())
                        })?;
                        match fs::OpenOptions::new().write(true).create_new(true).open(&parsed_args.path) {
                            Ok(mut file) => {
                                use std::io::Write;
                                match file.write_all(content.as_bytes()) {
                                    Ok(_) => Ok(json!({ "status": format!("File '{}' created successfully.", parsed_args.path) })),
                                    Err(e) => Err(AutomationError::Internal(format!("Failed to write content to file '{}': {}", parsed_args.path, e))),
                                }
                            }
                            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {
                                Err(AutomationError::Internal(format!("File '{}' already exists. Cannot create.", parsed_args.path)))
                            }
                            Err(e) => {
                                Err(AutomationError::Internal(format!("Failed to create file '{}': {}", parsed_args.path, e)))
                            }
                        }
                    }
                    "str_replace" => {
                        let old_str = parsed_args.old_str.ok_or_else(|| {
                            AutomationError::InvalidArgument("old_str required for str_replace command".to_string())
                        })?;
                        // new_str defaults to empty string if not provided (enables deletion)
                        let new_str = parsed_args.new_str.unwrap_or_default();
                        let content = match fs::read_to_string(&parsed_args.path) {
                            Ok(c) => c,
                            Err(e) => return Err(AutomationError::Internal(format!("Failed to read file '{}' for replacement: {}", parsed_args.path, e))),
                        };

                        // Count matches before replacement to provide feedback
                        let match_count = content.matches(&old_str).count();

                        if match_count == 0 {
                            return Err(AutomationError::Internal(format!("No matches found for '{}' in file '{}'", old_str, parsed_args.path)));
                        }

                        let new_content = content.replace(&old_str, &new_str);
                        match fs::write(&parsed_args.path, new_content) {
                            Ok(_) => Ok(json!({
                                "status": format!("File '{}' updated successfully. {} occurrence(s) of '{}' replaced with '{}'.",
                                    parsed_args.path, match_count, old_str, new_str),
                                "matches_replaced": match_count
                            })),
                            Err(e) => Err(AutomationError::Internal(format!("Failed to write updated content to file '{}': {}", parsed_args.path, e))),
                        }
                    }
                    "insert" => {
                        let new_str = parsed_args.new_str.ok_or_else(|| {
                            AutomationError::InvalidArgument("new_str required for insert command".to_string())
                        })?;
                        let insert_line = parsed_args.insert_line.ok_or_else(|| {
                            AutomationError::InvalidArgument("insert_line required for insert command".to_string())
                        })?;
                        let content = match fs::read_to_string(&parsed_args.path) {
                            Ok(c) => c,
                            Err(e) => return Err(AutomationError::Internal(format!("Failed to read file '{}' for insertion: {}", parsed_args.path, e))),
                        };

                        let mut lines: Vec<String> = content.lines().map(|s| s.to_string()).collect();

                        // Fix: insert_line should mean "insert after line N" for consistency with text editor expectations
                        // insert_line=1 means insert after line 1 (at index 1)
                        // insert_line=0 means insert at beginning (at index 0)
                        let insertion_point = if insert_line == 0 {
                            0
                        } else {
                            insert_line.min(lines.len())
                        };

                        lines.insert(insertion_point, new_str);

                        let new_content = lines.join("\n");
                        let final_content = if content.ends_with('\n') || content.is_empty() {
                            format!("{}\n", new_content)
                        } else {
                            new_content
                        };

                        match fs::write(&parsed_args.path, final_content) {
                            Ok(_) => Ok(json!({ "status": format!("Text inserted successfully into file '{}' after line {}.", parsed_args.path, if insert_line == 0 { "beginning".to_string() } else { insert_line.to_string() }) })),
                            Err(e) => Err(AutomationError::Internal(format!("Failed to write updated content to file '{}': {}", parsed_args.path, e))),
                        }
                    }
                    _ => {
                        Err(AutomationError::InvalidArgument(format!("Unknown str_replace_based_edit_tool command: {}", parsed_args.command)))
                    }
                }
            }
            // --- End str_replace_based_edit_tool Handler ---

            _ => {
                error!("Unknown tool called: {}", name);
                Err(AutomationError::ToolNotFound(name.to_string()))
            }
        }
    }

    // --- Screenshot Functionality ---
    #[cfg(target_os = "macos")]
    pub fn capture_screenshot_base64(&self) -> Result<String, AutomationError> {
        // Call the platform-specific function that now handles encoding
        platforms::macos::utils::capture_and_encode_screenshot()
    }

    #[cfg(not(target_os = "macos"))]
    pub fn capture_screenshot_base64(&self) -> Result<String, AutomationError> {
        Err(AutomationError::UnsupportedOperation(
            "Screenshot capture is only supported on macOS currently.".to_string(),
        ))
    }
    // --- End Screenshot Functionality ---

    // --- End New Methods ---
}


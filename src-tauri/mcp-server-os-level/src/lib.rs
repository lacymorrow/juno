//! Desktop UI automation through accessibility APIs
//!
//! This module provides a cross-platform API for automating desktop applications
//! through accessibility APIs, inspired by Playwright's web automation model.

use anyhow::Result;
use serde::{Deserialize, Serialize};
use serde_json;
use serde_json::Value;
use std::collections::HashMap;
use std::sync::{Arc};
use std::str::FromStr;
use tracing::{error, info};
use serde_json::{json, from_value};
use std::fs;
use std::process::Command;

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
pub use element::{UIElement, UIElementAttributes};
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
        let engine_result = if cfg!(target_os = "macos") {
            info!("Initializing macOS engine...");
            platforms::macos::MacOSEngine::new(use_background_apps, activate_app)
                .map(|e| Arc::new(e) as Arc<dyn platforms::AccessibilityEngine + Send + Sync>)
        } else if cfg!(target_os = "windows") {
            info!("Initializing Windows engine...");
            #[cfg(target_os = "windows")]
            {
                platforms::windows::WindowsEngine::new()
                    .map(|e| Arc::new(e) as Arc<dyn platforms::AccessibilityEngine + Send + Sync>)
            }
            #[cfg(not(target_os = "windows"))]
            {
                 // Ensure the Err type matches the other branches
                 Err(AutomationError::UnsupportedPlatform("Windows engine not supported on this platform".to_string()))
            }
        } else {
            Err(AutomationError::UnsupportedPlatform("Platform not supported".to_string()))
        };

        match engine_result {
            Ok(engine) => {
                info!("Desktop engine initialized successfully.");
                Ok(Self {
                    engine,
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
    pub fn hold_key(&self, key: &str) -> Result<(), AutomationError> {
        self.engine.hold_key(key)
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
    pub fn left_click(&self, x: f64, y: f64) -> Result<(), AutomationError> {
        self.engine.left_click(x, y)
    }

    /// Simulate a right click (down + up) at specified coordinates.
    pub fn right_click(&self, x: f64, y: f64) -> Result<(), AutomationError> {
        self.engine.right_click(x, y)
    }

    /// Simulate a middle click (down + up) at specified coordinates.
    pub fn middle_click(&self, x: f64, y: f64) -> Result<(), AutomationError> {
        self.engine.middle_click(x, y)
    }

    /// Simulate a double left click at the specified coordinates.
    pub fn double_click(&self, x: f64, y: f64) -> Result<(), AutomationError> {
        self.engine.double_click(x, y)
    }

    /// Simulate a triple left click at the specified coordinates.
    pub fn triple_click(&self, x: f64, y: f64) -> Result<(), AutomationError> {
        self.engine.triple_click(x, y)
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

    // /// Scroll at a specific position on screen
    // pub fn scroll_at_position(&self, x: f64, y: f64, direction: &str, amount: f64) -> Result<(), AutomationError> {
    //     self.engine.scroll_at_position(x, y, direction, amount)
    // }

    // /// Scroll at the current mouse position
    // pub fn scroll_at_current_position(&self, direction: &str, amount: f64) -> Result<(), AutomationError> {
    //     self.engine.scroll_at_current_position(direction, amount)
    // }

    // --- New Methods for Agent Loop ---

    /// List available tools for the LLM
    pub fn list_tools(&self) -> Vec<ToolDefinition> {
        let mut tools = vec![
            // --- Standard Tools ---
            ToolDefinition {
                name: "getUiTree".to_string(),
                description: "Get the UI element tree for the specified application or the currently focused one.".to_string(),
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
                name: "holdKey".to_string(),
                description: "Holds down a specified modifier key (Shift, Command/Cmd/Meta, Control/Ctrl, Option/Alt). The key remains held until 'releaseKey' is called.".to_string(),
                input_schema: ToolInputSchema {
                    type_: "object".to_string(),
                    properties: [(
                        "key".to_string(),
                        ToolParameter {
                            type_: "string".to_string(),
                            description: "The modifier key to hold (e.g., 'shift', 'cmd', 'ctrl', 'alt').".to_string(),
                        },
                    )]
                    .iter()
                    .cloned()
                    .collect(),
                    required: vec!["key".to_string()],
                },
            },
            ToolDefinition {
                name: "releaseKey".to_string(),
                description: "Releases a previously held modifier key.".to_string(),
                input_schema: ToolInputSchema {
                    type_: "object".to_string(),
                    properties: [(
                        "key".to_string(),
                        ToolParameter {
                            type_: "string".to_string(),
                            description: "The modifier key to release (e.g., 'shift', 'cmd', 'ctrl', 'alt').".to_string(),
                        },
                    )]
                    .iter()
                    .cloned()
                    .collect(),
                    required: vec!["key".to_string()],
                },
            },
            ToolDefinition {
                name: "wait".to_string(),
                description: "Wait for a specified duration in milliseconds before proceeding.".to_string(),
                input_schema: ToolInputSchema {
                    type_: "object".to_string(),
                    properties: [(
                        "duration_ms".to_string(),
                        ToolParameter {
                            type_: "number".to_string(),
                            description: "The number of milliseconds to wait.".to_string(),
                        },
                    )]
                    .iter()
                    .cloned()
                    .collect(),
                    required: vec!["duration_ms".to_string()],
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
            ToolDefinition {
                name: "pressKey".to_string(),
                description: "Presses a single key with an optional modifier key (e.g., Command, Shift, Option, Control). Common key names include 'return', 'enter', 'tab', 'space', 'delete', 'backspace', 'escape', 'left', 'right', 'up', 'down'. For letters/numbers, use the character itself (e.g., 'a', '1').".to_string(),
                input_schema: ToolInputSchema {
                    type_: "object".to_string(),
                    properties: {
                        let mut props = HashMap::new();
                        props.insert(
                            "key".to_string(),
                            ToolParameter {
                                type_: "string".to_string(),
                                description: "The name of the key to press (e.g., 'enter', 'a', 'tab').".to_string(),
                            },
                        );
                        props.insert(
                            "modifier".to_string(),
                            ToolParameter {
                                type_: "string".to_string(),
                                description: "Optional modifier key (e.g., 'command', 'shift', 'option', 'control').".to_string(),
                            },
                        );
                        props
                    },
                    required: vec!["key".to_string()], // key is required, modifier is optional
                },
            },
            ToolDefinition {
                name: "cursorPosition".to_string(),
                description: "Gets the current position (x, y coordinates) of the mouse cursor on the screen.".to_string(),
                input_schema: ToolInputSchema {
                    type_: "object".to_string(),
                    properties: HashMap::new(), // No input parameters
                    required: Vec::new(),
                },
            },
            ToolDefinition {
                name: "mouseMove".to_string(),
                description: "Moves the mouse cursor to the specified (x, y) coordinates on the screen.".to_string(),
                input_schema: ToolInputSchema {
                    type_: "object".to_string(),
                    properties: {
                        let mut props = HashMap::new();
                        props.insert("x".to_string(), ToolParameter { type_: "number".to_string(), description: "The target x-coordinate.".to_string() });
                        props.insert("y".to_string(), ToolParameter { type_: "number".to_string(), description: "The target y-coordinate.".to_string() });
                        props
                    },
                    required: vec!["x".to_string(), "y".to_string()],
                },
            },
            ToolDefinition {
                name: "leftMouseDown".to_string(),
                description: "Simulates pressing the left mouse button down at the specified (x, y) coordinates.".to_string(),
                input_schema: ToolInputSchema {
                    type_: "object".to_string(),
                    properties: {
                        let mut props = HashMap::new();
                        props.insert("x".to_string(), ToolParameter { type_: "number".to_string(), description: "The x-coordinate for the mouse down event.".to_string() });
                        props.insert("y".to_string(), ToolParameter { type_: "number".to_string(), description: "The y-coordinate for the mouse down event.".to_string() });
                        props
                    },
                    required: vec!["x".to_string(), "y".to_string()],
                },
            },
            ToolDefinition {
                name: "leftMouseUp".to_string(),
                description: "Simulates releasing the left mouse button at the specified (x, y) coordinates.".to_string(),
                input_schema: ToolInputSchema {
                    type_: "object".to_string(),
                    properties: {
                        let mut props = HashMap::new();
                        props.insert("x".to_string(), ToolParameter { type_: "number".to_string(), description: "The x-coordinate for the mouse up event.".to_string() });
                        props.insert("y".to_string(), ToolParameter { type_: "number".to_string(), description: "The y-coordinate for the mouse up event.".to_string() });
                        props
                    },
                    required: vec!["x".to_string(), "y".to_string()],
                },
            },
            ToolDefinition {
                name: "leftClick".to_string(),
                description: "Simulates a standard left mouse click (down then up) at the specified (x, y) coordinates.".to_string(),
                input_schema: ToolInputSchema {
                    type_: "object".to_string(),
                    properties: {
                        let mut props = HashMap::new();
                        props.insert("x".to_string(), ToolParameter { type_: "number".to_string(), description: "The x-coordinate to click at.".to_string() });
                        props.insert("y".to_string(), ToolParameter { type_: "number".to_string(), description: "The y-coordinate to click at.".to_string() });
                        props
                    },
                    required: vec!["x".to_string(), "y".to_string()],
                },
            },
            ToolDefinition {
                name: "rightClick".to_string(),
                description: "Simulates a standard right mouse click (down then up) at the specified (x, y) coordinates.".to_string(),
                input_schema: ToolInputSchema {
                    type_: "object".to_string(),
                    properties: {
                        let mut props = HashMap::new();
                        props.insert("x".to_string(), ToolParameter { type_: "number".to_string(), description: "The x-coordinate to click at.".to_string() });
                        props.insert("y".to_string(), ToolParameter { type_: "number".to_string(), description: "The y-coordinate to click at.".to_string() });
                        props
                    },
                    required: vec!["x".to_string(), "y".to_string()],
                },
            },
            ToolDefinition {
                name: "middleClick".to_string(),
                description: "Simulates a standard middle mouse click (down then up) at the specified (x, y) coordinates.".to_string(),
                input_schema: ToolInputSchema {
                    type_: "object".to_string(),
                    properties: {
                        let mut props = HashMap::new();
                        props.insert("x".to_string(), ToolParameter { type_: "number".to_string(), description: "The x-coordinate to click at.".to_string() });
                        props.insert("y".to_string(), ToolParameter { type_: "number".to_string(), description: "The y-coordinate to click at.".to_string() });
                        props
                    },
                    required: vec!["x".to_string(), "y".to_string()],
                },
            },
            ToolDefinition {
                name: "doubleClick".to_string(),
                description: "Simulates a double left mouse click at the specified (x, y) coordinates.".to_string(),
                input_schema: ToolInputSchema {
                    type_: "object".to_string(),
                    properties: {
                        let mut props = HashMap::new();
                        props.insert("x".to_string(), ToolParameter { type_: "number".to_string(), description: "The x-coordinate to double click at.".to_string() });
                        props.insert("y".to_string(), ToolParameter { type_: "number".to_string(), description: "The y-coordinate to double click at.".to_string() });
                        props
                    },
                    required: vec!["x".to_string(), "y".to_string()],
                },
            },
            ToolDefinition {
                name: "tripleClick".to_string(),
                description: "Simulates a triple left mouse click at the specified (x, y) coordinates.".to_string(),
                input_schema: ToolInputSchema {
                    type_: "object".to_string(),
                    properties: {
                        let mut props = HashMap::new();
                        props.insert("x".to_string(), ToolParameter { type_: "number".to_string(), description: "The x-coordinate to triple click at.".to_string() });
                        props.insert("y".to_string(), ToolParameter { type_: "number".to_string(), description: "The y-coordinate to triple click at.".to_string() });
                        props
                    },
                    required: vec!["x".to_string(), "y".to_string()],
                },
            },
            ToolDefinition {
                name: "leftClickDrag".to_string(),
                description: "Simulates dragging the mouse with the left button held down, from start (x, y) to end (x, y) coordinates.".to_string(),
                input_schema: ToolInputSchema {
                    type_: "object".to_string(),
                    properties: {
                        let mut props = HashMap::new();
                        props.insert("start_x".to_string(), ToolParameter { type_: "number".to_string(), description: "The starting x-coordinate of the drag.".to_string() });
                        props.insert("start_y".to_string(), ToolParameter { type_: "number".to_string(), description: "The starting y-coordinate of the drag.".to_string() });
                        props.insert("end_x".to_string(), ToolParameter { type_: "number".to_string(), description: "The ending x-coordinate of the drag.".to_string() });
                        props.insert("end_y".to_string(), ToolParameter { type_: "number".to_string(), description: "The ending y-coordinate of the drag.".to_string() });
                        props
                    },
                    required: vec!["start_x".to_string(), "start_y".to_string(), "end_x".to_string(), "end_y".to_string()],
                },
            },
            // --- Text Editor Tools ---
            ToolDefinition {
                name: "text_editor_view".to_string(),
                description: "View the content of a specified file.".to_string(),
                input_schema: ToolInputSchema {
                    type_: "object".to_string(),
                    properties: HashMap::from([
                        ("file_path".to_string(), ToolParameter { type_: "string".to_string(), description: "The path to the file to view.".to_string() }),
                    ]),
                    required: vec!["file_path".to_string()],
                },
            },
            ToolDefinition {
                name: "text_editor_create".to_string(),
                description: "Create a new file, optionally with initial content. Fails if the file already exists.".to_string(),
                input_schema: ToolInputSchema {
                    type_: "object".to_string(),
                    properties: HashMap::from([
                        ("file_path".to_string(), ToolParameter { type_: "string".to_string(), description: "The path where the new file should be created.".to_string() }),
                        ("content".to_string(), ToolParameter { type_: "string".to_string(), description: "Optional initial text content for the file.".to_string() }),
                    ]),
                    required: vec!["file_path".to_string()], // Content is optional
                },
            },
            ToolDefinition {
                name: "text_editor_str_replace".to_string(),
                description: "Find and replace all occurrences of a string within a specified file.".to_string(),
                input_schema: ToolInputSchema {
                    type_: "object".to_string(),
                    properties: HashMap::from([
                        ("file_path".to_string(), ToolParameter { type_: "string".to_string(), description: "The path to the file to modify.".to_string() }),
                        ("find".to_string(), ToolParameter { type_: "string".to_string(), description: "The exact string to find.".to_string() }),
                        ("replace".to_string(), ToolParameter { type_: "string".to_string(), description: "The string to replace occurrences with.".to_string() }),
                    ]),
                    required: vec!["file_path".to_string(), "find".to_string(), "replace".to_string()],
                },
            },
            ToolDefinition {
                name: "text_editor_insert".to_string(),
                description: "Insert text into a file at a specific 1-based line number. If line is null or out of bounds, appends to the end.".to_string(),
                input_schema: ToolInputSchema {
                    type_: "object".to_string(),
                    properties: HashMap::from([
                        ("file_path".to_string(), ToolParameter { type_: "string".to_string(), description: "The path to the file to modify.".to_string() }),
                        ("text".to_string(), ToolParameter { type_: "string".to_string(), description: "The text to insert.".to_string() }),
                        ("line".to_string(), ToolParameter { type_: "integer".to_string(), description: "The 1-based line number to insert before. Null or out of bounds appends.".to_string() }),
                    ]),
                    required: vec!["file_path".to_string(), "text".to_string()], // line is optional implicitly by nullability
                },
            },
            // --- End Text Editor Tools ---

            // --- Bash Tool ---
            ToolDefinition {
                name: "bash".to_string(),
                description: "Executes a shell command and returns its stdout and stderr.".to_string(),
                input_schema: ToolInputSchema {
                    type_: "object".to_string(),
                    properties: HashMap::from([
                        ("command".to_string(), ToolParameter { type_: "string".to_string(), description: "The shell command to execute.".to_string() }),
                        // Timeout parameter included in schema but not yet implemented in handler
                        ("timeout".to_string(), ToolParameter { type_: "integer".to_string(), description: "Optional timeout in seconds for the command execution.".to_string() }),
                    ]),
                    required: vec!["command".to_string()],
                },
            },
            // --- End Bash Tool ---
        ];

        // Manually define the tools available from this Desktop instance
        // This should reflect the public methods of Desktop and UIElement potentially
        tools.extend(vec![
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
            // TODO: Add more tools corresponding to Desktop/UIElement methods
            // e.g., find_element (using locator), click, type_text, get_attributes, etc.
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
                description: "Clicks on a UI element specified by a selector.".to_string(),
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
                description: "Types text into a UI element specified by a selector.".to_string(),
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
                name: "get_element_attributes".to_string(),
                description: "Gets the attributes of a UI element specified by a selector.".to_string(),
                input_schema: ToolInputSchema {
                    type_: "object".to_string(),
                    properties: [
                        ("selector".to_string(), ToolParameter {
                            type_: "string".to_string(),
                            description: "The selector for the element.".to_string(),
                        })
                    ].iter().cloned().collect(),
                    required: vec!["selector".to_string()],
                },
            },
             ToolDefinition {
                name: "scroll_element".to_string(),
                description: "Scrolls a UI element specified by a selector.".to_string(),
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
                            description: "The amount/distance to scroll (platform interpretation varies).".to_string(),
                        }),
                    ].iter().cloned().collect(),
                    required: vec!["selector".to_string(), "direction".to_string(), "amount".to_string()],
                },
            },
            ToolDefinition {
                name: "typeText".to_string(),
                description: "Types the given text into the currently focused element, or globally if no element is focused.".to_string(),
                input_schema: ToolInputSchema {
                    type_: "object".to_string(),
                    properties: [(
                        "text".to_string(),
                        ToolParameter {
                            type_: "string".to_string(),
                            description: "The text to type.".to_string(),
                        },
                    )]
                    .iter()
                    .cloned()
                    .collect(),
                    required: vec!["text".to_string()],
                },
            },
        ]);

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
                self.engine.press_key(&parsed_args.key, parsed_args.modifier.as_deref())?;
                Ok(json!({
                    "status": "success",
                    "details": format!("Pressed key '{}' with modifier '{}'", parsed_args.key, parsed_args.modifier.as_deref().unwrap_or("none"))
                }))
            }
            "holdKey" => {
                let key = args["key"].as_str().ok_or_else(|| {
                    AutomationError::InvalidArgument("Missing or invalid 'key' argument for holdKey".to_string())
                })?;
                self.hold_key(key)?;
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
                self.left_click(args.x, args.y)?;
                Ok(json!(null))
            }
            "rightClick" => {
                #[derive(Deserialize)]
                struct ClickArgs { x: f64, y: f64 }
                let args: ClickArgs = from_value(args).map_err(|e| AutomationError::InvalidArgument(format!("Error parsing rightClick args: {}", e)))?;
                self.right_click(args.x, args.y)?;
                Ok(json!(null))
            }
            "middleClick" => {
                #[derive(Deserialize)]
                struct ClickArgs { x: f64, y: f64 }
                let args: ClickArgs = from_value(args).map_err(|e| AutomationError::InvalidArgument(format!("Error parsing middleClick args: {}", e)))?;
                self.middle_click(args.x, args.y)?;
                Ok(json!(null))
            }
            "doubleClick" => {
                #[derive(Deserialize)]
                struct ClickArgs { x: f64, y: f64 }
                let args: ClickArgs = from_value(args).map_err(|e| AutomationError::InvalidArgument(format!("Error parsing doubleClick args: {}", e)))?;
                self.double_click(args.x, args.y)?;
                Ok(json!(null))
            }
            "tripleClick" => {
                #[derive(Deserialize)]
                struct ClickArgs { x: f64, y: f64 }
                let args: ClickArgs = from_value(args).map_err(|e| AutomationError::InvalidArgument(format!("Error parsing tripleClick args: {}", e)))?;
                self.triple_click(args.x, args.y)?;
                Ok(json!(null))
            }
            "leftClickDrag" => {
                #[derive(Deserialize)]
                struct DragArgs { start_x: f64, start_y: f64, end_x: f64, end_y: f64 }
                let args: DragArgs = from_value(args).map_err(|e| AutomationError::InvalidArgument(format!("Error parsing leftClickDrag args: {}", e)))?;
                self.left_click_drag(args.start_x, args.start_y, args.end_x, args.end_y)?;
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


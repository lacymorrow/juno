//! Desktop UI automation through accessibility APIs
//!
//! This module provides a cross-platform API for automating desktop applications
//! through accessibility APIs, inspired by Playwright's web automation model.

use anyhow::Result;
use serde::{Deserialize, Serialize};
use serde_json;
use serde_json::Value;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};
use tracing::{debug, error, info, warn};

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
    use_background_apps: bool,
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
        // Manually define the tools available from this Desktop instance
        // This should reflect the public methods of Desktop and UIElement potentially
        vec![
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
            ToolDefinition {
                name: "captureScreenshot".to_string(),
                description: "Captures a screenshot of the main display and returns it as a base64 encoded PNG string.".to_string(),
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
                description: "Pauses execution for a specified number of milliseconds.".to_string(),
                input_schema: ToolInputSchema {
                    type_: "object".to_string(),
                    properties: [(
                        "duration_ms".to_string(),
                        ToolParameter {
                            type_: "number".to_string(), // Use number for duration
                            description: "The duration to wait in milliseconds.".to_string(),
                        },
                    )]
                    .iter()
                    .cloned()
                    .collect(),
                    required: vec!["duration_ms".to_string()],
                },
            },
        ]
    }

    /// Call a specific tool by name with given arguments
    pub fn call_tool(&self, name: &str, args: Value) -> Result<Value, AutomationError> {
        info!("Calling tool: {} with args: {:?}", name, args);
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
            "captureScreenshot" => {
                if !args.is_null() && !args.as_object().map_or(true, |m| m.is_empty()) {
                    return Err(AutomationError::InvalidArgument(
                        "captureScreenshot tool does not accept any arguments.".to_string(),
                    ));
                }
                let base64_image = self.capture_screenshot_base64()?;
                Ok(serde_json::json!({
                    "status": "success",
                    "screenshot_base64": base64_image,
                    "format": "png"
                }))
            }
            "getClipboard" => {
                let content = self.get_clipboard_content()?;
                Ok(Value::String(content))
            }
            "setClipboard" => {
                let content = args["content"].as_str().ok_or_else(|| {
                    AutomationError::InvalidArgument("Missing or invalid 'content' argument".to_string())
                })?;
                self.set_clipboard_content(content)?;
                Ok(Value::String("Clipboard content set successfully.".to_string()))
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
                let duration_ms = args["duration_ms"].as_u64().ok_or_else(|| {
                    AutomationError::InvalidArgument("Missing or invalid 'duration_ms' argument for wait (must be a non-negative integer)".to_string())
                })?;
                self.wait(duration_ms)?;
                Ok(Value::String(format!("Waited for {} ms.", duration_ms)))
            }
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

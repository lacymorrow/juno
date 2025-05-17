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
use tracing::{error, info, warn};

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
#[derive(Debug, Serialize, Deserialize)]
pub struct ClickResult {
    pub method: String,
    pub coordinates: Option<(f64, f64)>,
    pub details: String,
}

/// The main entry point for UI automation
#[derive(Clone)] // Added from HEAD
pub struct Desktop {
    engine: Arc<dyn platforms::AccessibilityEngine + Send + Sync>,
    #[allow(dead_code)] // Added from HEAD
    use_background_apps: bool,
    #[allow(dead_code)] // Added from HEAD
    activate_app: bool,
}

impl Desktop {
    /// Create a new instance with the default platform-specific implementation
    pub fn new(use_background_apps: bool, activate_app: bool) -> Result<Self, AutomationError> {
        info!("Attempting to initialize desktop engine...");
        let engine_result = if cfg!(target_os = "macos") {
            info!("Initializing macOS engine with use_background_apps: {}, activate_app: {}", use_background_apps, activate_app);
            platforms::macos::MacOSEngine::new(use_background_apps, activate_app)
                .map(|e| Arc::new(e) as Arc<dyn platforms::AccessibilityEngine + Send + Sync>)
        } else if cfg!(target_os = "windows") {
            warn!("Windows platform is not fully supported yet, attempting initialization.");
            // This would be the place for windows::WindowsEngine::new if available
            Err(AutomationError::UnsupportedPlatform("Windows desktop automation is not yet fully implemented.".to_string()))
        } else if cfg!(target_os = "linux") {
            warn!("Linux platform is not fully supported yet, attempting initialization.");
            Err(AutomationError::UnsupportedPlatform("Linux desktop automation is not yet fully implemented.".to_string()))
        } else {
            error!("Unsupported platform for desktop engine initialization.");
            Err(AutomationError::UnsupportedPlatform(
                "Desktop engine initialization failed: Unsupported operating system".to_string(),
            ))
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

    /// Wait for a specified duration
    pub fn wait(&self, duration_ms: u64) -> Result<(), AutomationError> {
        self.engine.wait(duration_ms)
    }

    /// Get clipboard content
    pub fn get_clipboard_content(&self) -> Result<String, AutomationError> {
        self.engine.get_clipboard_content()
    }

    /// Set clipboard content
    pub fn set_clipboard_content(&self, content: &str) -> Result<(), AutomationError> {
        self.engine.set_clipboard_content(content)
    }

    /// Type text into a UI element
    pub fn type_text(&self, text: &str) -> Result<(), AutomationError> {
        self.engine.type_text(text)
    }

    /// Press a key
    pub fn press_key(&self, key_name: &str, modifier: Option<&str>) -> Result<(), AutomationError> {
        self.engine.press_key(key_name, modifier)
    }

    /// Hold a key
    pub fn hold_key(&self, key: &str, duration_ms: Option<u64>) -> Result<(), AutomationError> {
        self.engine.hold_key(key, duration_ms)
    }

    /// Release a key
    pub fn release_key(&self, key: &str) -> Result<(), AutomationError> {
        self.engine.release_key(key)
    }

    /// Left click
    pub fn left_click(&self, x: f64, y: f64, modifiers: Option<&str>) -> Result<(), AutomationError> {
        self.engine.left_click(x, y, modifiers)
    }

    /// Right click
    pub fn right_click(&self, x: f64, y: f64, modifiers: Option<&str>) -> Result<(), AutomationError> {
        self.engine.right_click(x, y, modifiers)
    }

    /// Middle click
    pub fn middle_click(&self, x: f64, y: f64, modifiers: Option<&str>) -> Result<(), AutomationError> {
        self.engine.middle_click(x, y, modifiers)
    }

    /// Double click
    pub fn double_click(&self, x: f64, y: f64, modifiers: Option<&str>) -> Result<(), AutomationError> {
        self.engine.double_click(x, y, modifiers)
    }

    /// Triple click
    pub fn triple_click(&self, x: f64, y: f64, modifiers: Option<&str>) -> Result<(), AutomationError> {
        self.engine.triple_click(x, y, modifiers)
    }

    /// Mouse move
    pub fn mouse_move(&self, x: f64, y: f64) -> Result<(), AutomationError> {
        self.engine.mouse_move(x, y)
    }

    /// Left mouse down
    pub fn left_mouse_down(&self, x: f64, y: f64) -> Result<(), AutomationError> {
        self.engine.left_mouse_down(x, y)
    }

    /// Left mouse up
    pub fn left_mouse_up(&self, x: f64, y: f64) -> Result<(), AutomationError> {
        self.engine.left_mouse_up(x, y)
    }

    /// Left click drag
    pub fn left_click_drag(&self, start_x: f64, start_y: f64, end_x: f64, end_y: f64) -> Result<(), AutomationError> {
        self.engine.left_click_drag(start_x, start_y, end_x, end_y)
    }

    /// Cursor position
    pub fn cursor_position(&self) -> Result<(f64, f64), AutomationError> {
        self.engine.cursor_position()
    }

    /// Scroll at position
    pub fn scroll_at_position(&self, x: f64, y: f64, direction: &str, amount: f64) -> Result<(), AutomationError> {
        self.engine.scroll_at_position(x, y, direction, amount)
    }

    /// Scroll at current position
    pub fn scroll_at_current_position(&self, direction: &str, amount: f64) -> Result<(), AutomationError> {
        self.engine.scroll_at_current_position(direction, amount)
    }

    /// List windows
    pub fn list_windows(&self) -> Result<Vec<UIElement>, AutomationError> {
        self.engine.list_windows()
    }

    /// List available tools for the LLM
    pub fn list_tools(&self) -> Vec<ToolDefinition> {
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
        ]
    }

    /// Call a specific tool by name with JSON arguments
    pub fn call_tool(&self, name: &str, args: Value) -> Result<Value, AutomationError> {
        match name {
            "open_application" => {
                let app_name = args.get("app_name")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| AutomationError::InvalidArgument("Missing or invalid 'app_name' argument".to_string()))?;
                self.open_application(app_name)?;
                Ok(serde_json::json!({"status": "success", "message": format!("Application '{}' opened.", app_name)}))
            }
            "open_url" => {
                let url = args.get("url")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| AutomationError::InvalidArgument("Missing or invalid 'url' argument".to_string()))?;
                let browser = args.get("browser").and_then(|v| v.as_str()); // Optional
                self.open_url(url, browser)?;
                Ok(serde_json::json!({"status": "success", "message": format!("URL '{}' opened.", url)}))
            }
             "get_focused_element_info" => {
                let element = self.focused_element()?;
                let attributes = element.attributes();
                // Convert attributes to JSON value
                let result_json = serde_json::to_value(attributes)
                    .map_err(|e| AutomationError::Internal(format!("Failed to serialize element attributes: {}", e)))?;
                Ok(result_json)
            }
            "find_element" => {
                let selector_str = args.get("selector")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| AutomationError::InvalidArgument("Missing or invalid 'selector' argument".to_string()))?;
                let selector: Selector = selector_str.into();
                let element_option = self.locator(selector).first()?;
                if let Some(element) = element_option {
                    let attributes = element.attributes();
                    let result_json = serde_json::to_value(attributes)
                         .map_err(|e| AutomationError::Internal(format!("Failed to serialize element attributes: {}", e)))?;
                     Ok(serde_json::json!({
                        "status": "success",
                        "element_found": true,
                        "attributes": result_json
                     }))
                } else {
                     Err(AutomationError::ElementNotFound(format!("Element not found for selector: {}", selector_str)))
                }
            }
            "click" => {
                let selector_str = args.get("selector")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| AutomationError::InvalidArgument("Missing or invalid 'selector' argument".to_string()))?;
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
                     Err(AutomationError::ElementNotFound(format!("Element not found for selector: {}", selector_str)))
                }
            }
             "type_text" => {
                 let selector_str = args.get("selector")
                     .and_then(|v| v.as_str())
                     .ok_or_else(|| AutomationError::InvalidArgument("Missing or invalid 'selector' argument".to_string()))?;
                 let text_to_type = args.get("text")
                     .and_then(|v| v.as_str())
                     .ok_or_else(|| AutomationError::InvalidArgument("Missing or invalid 'text' argument".to_string()))?;
                let selector: Selector = selector_str.into();
                 let element_option = self.locator(selector).first()?;
                 if let Some(element) = element_option {
                     element.type_text(text_to_type)?;
                     Ok(serde_json::json!({
                         "status": "success",
                         "message": format!("Typed text into element matching selector '{}'", selector_str)
                     }))
                 } else {
                     Err(AutomationError::ElementNotFound(format!("Element not found for selector: {}", selector_str)))
                 }
             }
             "get_element_attributes" => {
                  let selector_str = args.get("selector")
                      .and_then(|v| v.as_str())
                      .ok_or_else(|| AutomationError::InvalidArgument("Missing or invalid 'selector' argument".to_string()))?;
                  let selector: Selector = selector_str.into();
                  let element_option = self.locator(selector).first()?;
                   if let Some(element) = element_option {
                      let attributes = element.attributes();
                      let result_json = serde_json::to_value(attributes)
                          .map_err(|e| AutomationError::Internal(format!("Failed to serialize element attributes: {}", e)))?;
                      Ok(result_json)
                  } else {
                     Err(AutomationError::ElementNotFound(format!("Element not found for selector: {}", selector_str)))
                  }
              }
             "scroll_element" => {
                let selector_str = args.get("selector")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| AutomationError::InvalidArgument("Missing or invalid 'selector' argument".to_string()))?;
                let direction = args.get("direction")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| AutomationError::InvalidArgument("Missing or invalid 'direction' argument".to_string()))?;
                let amount = args.get("amount")
                    .and_then(|v| v.as_f64())
                    .ok_or_else(|| AutomationError::InvalidArgument("Missing or invalid 'amount' argument".to_string()))?;
                let selector: Selector = selector_str.into();
                let element_option = self.locator(selector).first()?;
                if let Some(element) = element_option {
                    element.scroll(direction, amount)?;
                    Ok(serde_json::json!({
                        "status": "success",
                        "message": format!("Scrolled element matching selector '{}'", selector_str)
                    }))
                } else {
                    Err(AutomationError::ElementNotFound(format!("Element not found for selector: {}", selector_str)))
            }
            }
            _ => Err(AutomationError::UnsupportedOperation(format!("Tool '{}' not recognized.", name))),
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
}

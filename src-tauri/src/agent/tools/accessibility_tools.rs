//! # Accessibility Tools
//!
//! Native macOS accessibility tools that provide element-level interaction
//! as an alternative to coordinate-based clicking. These tools use the existing
//! computer-use-ai-sdk infrastructure to provide reliable UI element discovery
//! and interaction capabilities.

use computer_use_ai_sdk::{Desktop, UIElement};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use tauri::AppHandle;
use tracing::{debug, info, warn};

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct AccessibilityElement {
    pub id: u32,
    pub role: String,
    pub title: String,
    pub description: String,
    pub position: Option<(f64, f64)>,
    pub size: Option<(f64, f64)>,
    pub is_clickable: bool,
    pub app_name: Option<String>,
}

#[derive(Clone)]
pub struct AccessibilityTools {
    desktop: Arc<Mutex<Option<Desktop>>>,
    element_cache: Arc<Mutex<HashMap<u32, UIElement>>>,
    next_id: Arc<Mutex<u32>>,
}

impl AccessibilityTools {
    pub fn new() -> Self {
        Self {
            desktop: Arc::new(Mutex::new(None)),
            element_cache: Arc::new(Mutex::new(HashMap::new())),
            next_id: Arc::new(Mutex::new(1)),
        }
    }

        /// Initialize the desktop accessibility system if not already done
    pub fn ensure_engine_initialized(&self) -> Result<(), String> {
        let mut desktop_guard = self.desktop.lock().map_err(|e| format!("Lock error: {}", e))?;

        if desktop_guard.is_none() {
            debug!("Initializing Desktop accessibility system");
            match Desktop::new_with_auto_redirect(true, true, false) {
                Ok(desktop) => {
                    *desktop_guard = Some(desktop);
                    info!("Desktop accessibility system initialized successfully");
                }
                Err(e) => {
                    return Err(format!("Failed to initialize desktop accessibility: {}", e));
                }
            }
        }

        Ok(())
    }

        /// Scan the frontmost application for clickable UI elements
    pub fn scan_frontmost_application(&self) -> Result<Vec<AccessibilityElement>, String> {
        self.ensure_engine_initialized()?;

        let desktop_guard = self.desktop.lock().map_err(|e| format!("Lock error: {}", e))?;
        let desktop = desktop_guard.as_ref().ok_or("Desktop not initialized")?;

        debug!("Scanning frontmost application for accessibility elements");

        // Get all applications and find the frontmost one
        let applications = desktop.applications()
            .map_err(|e| format!("Failed to get applications: {}", e))?;

        if applications.is_empty() {
            return Ok(Vec::new());
        }

        // For now, use the first application (could be enhanced to find frontmost)
        let app = &applications[0];

        // Get all child elements from the application using locator
        let locator = app.locator("*").map_err(|e| format!("Failed to create locator: {}", e))?;
        let all_elements = locator.all()
            .map_err(|e| format!("Failed to find elements: {}", e))?;

        debug!("Found {} total elements", all_elements.len());

        let mut accessibility_elements = Vec::new();
        let mut element_cache = self.element_cache.lock().map_err(|e| format!("Lock error: {}", e))?;
        let mut next_id = self.next_id.lock().map_err(|e| format!("Lock error: {}", e))?;

        // Clear previous cache
        element_cache.clear();

        for element in all_elements {
            let attributes = element.attributes();

            // Only include clickable elements
            if !Self::is_clickable_role(&attributes.role) {
                continue;
            }

            // Filter out elements that are too small or don't have useful information
            let (x, y, width, height) = match element.bounds() {
                Ok(bounds) => bounds,
                Err(_) => continue, // Skip elements without bounds
            };

            // Skip tiny elements
            if width < 5.0 || height < 5.0 {
                continue;
            }

            // Get element information
            let role = attributes.role;
            let title = attributes.label.unwrap_or_default();
            let description = attributes.description.unwrap_or_default();

            // Skip elements without any useful text
            if title.is_empty() && description.is_empty() {
                continue;
            }

            let id = *next_id;
            *next_id += 1;

            // Cache the element for clicking
            element_cache.insert(id, element);

            let accessibility_element = AccessibilityElement {
                id,
                role: role.clone(),
                title: if title.is_empty() { description.clone() } else { title.clone() },
                description: format!("{}: {}", role, if title.is_empty() { &description } else { &title }),
                position: Some((x, y)),
                size: Some((width, height)),
                is_clickable: true,
                app_name: None, // Could be enhanced to get app name
            };

            accessibility_elements.push(accessibility_element);
        }

        info!("Processed {} clickable accessibility elements", accessibility_elements.len());
        Ok(accessibility_elements)
    }

        /// Click an element by its ID
    pub fn click_element_by_id(&self, element_id: u32) -> Result<bool, String> {
        let element_cache = self.element_cache.lock().map_err(|e| format!("Lock error: {}", e))?;

        if let Some(element) = element_cache.get(&element_id) {
            debug!("Clicking accessibility element with ID: {}", element_id);

            match element.click() {
                Ok(_click_result) => {
                    info!("Successfully clicked element with ID: {}", element_id);
                    Ok(true)
                }
                Err(e) => {
                    warn!("Failed to click element with ID {}: {}", element_id, e);
                    Err(format!("Failed to click element: {}", e))
                }
            }
        } else {
            Err(format!("Element with ID {} not found. Run accessibility_scan first.", element_id))
        }
    }

    /// Check if a role is typically clickable
    fn is_clickable_role(role: &str) -> bool {
        matches!(role.to_lowercase().as_str(),
            "button" | "link" | "textfield" | "textarea" |
            "checkbox" | "radiobutton" | "popupbutton" |
            "combobox" | "tab" | "menuitem" | "image" |
            "cell" | "searchfield" | "statictext"
        )
    }

    /// Get tool definitions for the agent
    pub fn get_tool_definitions() -> Vec<serde_json::Value> {
        vec![
            json!({
                "name": "accessibility_scan",
                "description": "Scan the frontmost application for clickable UI elements using native macOS accessibility APIs. This provides element-level interaction as an alternative to coordinate-based clicking. Returns a list of elements with IDs that can be clicked.",
                "input_schema": {
                    "type": "object",
                    "properties": {},
                    "required": []
                }
            }),
            json!({
                "name": "accessibility_click",
                "description": "Click a UI element by its accessibility ID. Use this after scanning elements to interact with specific UI components reliably. This is often more reliable than coordinate-based clicking for UI elements.",
                "input_schema": {
                    "type": "object",
                    "properties": {
                        "element_id": {
                            "type": "integer",
                            "description": "The unique ID of the element to click (from accessibility_scan results)"
                        }
                    },
                    "required": ["element_id"]
                }
            })
        ]
    }

    /// Execute a tool by name
    pub async fn execute_tool(&self, tool_name: &str, parameters: &Value, _app_handle: &AppHandle) -> Result<Value, String> {
        match tool_name {
            "accessibility_scan" => {
                let elements = self.scan_frontmost_application()?;
                Ok(json!({
                    "success": true,
                    "elements": elements,
                    "count": elements.len(),
                    "message": format!("Found {} clickable elements in frontmost application", elements.len())
                }))
            },
            "accessibility_click" => {
                let element_id = parameters.get("element_id")
                    .and_then(|v| v.as_u64())
                    .ok_or("Missing or invalid element_id parameter")? as u32;

                let success = self.click_element_by_id(element_id)?;
                Ok(json!({
                    "success": success,
                    "element_id": element_id,
                    "message": if success {
                        "Element clicked successfully using accessibility API"
                    } else {
                        "Failed to click element"
                    }
                }))
            },
            _ => Err(format!("Unknown accessibility tool: {}", tool_name)),
        }
    }
}

impl Default for AccessibilityTools {
    fn default() -> Self {
        Self::new()
    }
}

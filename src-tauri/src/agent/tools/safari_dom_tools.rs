//! Safari DOM Tools Module
//!
//! Safari-specific DOM analysis and interaction tools for enhanced browser automation.
//! Provides native Safari JavaScript injection capabilities for faster browser operations
//! compared to Playwright-based automation.
//!
//! ## Core Capabilities:
//! - Direct Safari JavaScript injection via AppleScript
//! - Structured DOM extraction and serialization
//! - Element discovery with ID-based interaction
//! - Fast Safari-specific automation without browser overhead
//!
//! ## Integration:
//! - Complements existing browser controller tools
//! - Optimized for Safari-specific workflows
//! - Thread-safe implementation with element caching
//!
//! ## Usage
//! Used by: Safari-specific automation, DOM analysis, fast web interaction
//! Registration: Tool definitions returned by `get_safari_dom_tool_definitions()`

use crate::agent::core::{AgentError, ToolDefinition, ToolResult};
use serde_json::{json, Value};
use std::collections::HashMap;
use std::process::Command;
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};

/// Safari DOM Tools - Provides Safari-specific JavaScript injection and DOM analysis
#[derive(Debug)]
pub struct SafariDomTools {
    /// Cache for DOM elements with generated IDs
    element_cache: Arc<Mutex<HashMap<u32, DomElement>>>,
    /// Counter for generating unique element IDs
    element_id_counter: Arc<Mutex<u32>>,
}

/// Represents a DOM element with Safari-specific properties
#[derive(Debug, Clone)]
struct DomElement {
    id: u32,
    tag: String,
    element_id: Option<String>,
    class: Option<String>,
    role: Option<String>,
    text: Option<String>,
    clickable: bool,
    selector: String, // CSS selector for the element
    timestamp: u64,   // For cache expiration
}

impl SafariDomTools {
    /// Creates a new SafariDomTools instance
    pub fn new() -> Self {
        Self {
            element_cache: Arc::new(Mutex::new(HashMap::new())),
            element_id_counter: Arc::new(Mutex::new(1)),
        }
    }

    /// Checks if Safari is the currently active application
    pub fn is_safari_active(&self) -> Result<bool, AgentError> {
        let output = Command::new("osascript")
            .arg("-e")
            .arg("tell application \"System Events\" to get name of first application process whose frontmost is true")
            .output()
            .map_err(|e| AgentError::ToolError(format!("Failed to check active application: {}", e)))?;

        let active_app = String::from_utf8_lossy(&output.stdout).trim().to_string();
        Ok(active_app == "Safari")
    }

    /// Extracts structured DOM from the current Safari tab
    pub fn extract_safari_dom(&self) -> Result<ToolResult, AgentError> {
        if !self.is_safari_active()? {
            return Ok(ToolResult::error("Safari is not the active application"));
        }

        // JavaScript function to serialize DOM structure (based on opus implementation)
        let js_to_inject = r#"
function serializeDOMWithIds(node, idCounter = { value: 0 }) {
    if (!node || node.nodeType !== 1) return null;

    // Generate unique ID for this element
    const elementId = ++idCounter.value;

    // Add data attribute for identification
    node.setAttribute('data-juno-id', elementId);

    const children = [...node.children]
        .map(child => serializeDOMWithIds(child, idCounter))
        .filter(Boolean);

    // Determine if element is clickable
    const isClickable = typeof node.onclick === 'function' ||
        ['A', 'BUTTON', 'INPUT', 'SELECT', 'TEXTAREA'].includes(node.tagName) ||
        node.hasAttribute('onclick') ||
        node.getAttribute('role') === 'button' ||
        node.getAttribute('tabindex') !== null;

    // Generate CSS selector for this element
    let selector = node.tagName.toLowerCase();
    if (node.id) selector += '#' + node.id;
    if (node.className) selector += '.' + node.className.split(' ').join('.');

    return {
        id: elementId,
        tag: node.tagName,
        elementId: node.id || null,
        class: node.className || null,
        role: node.getAttribute('role') || null,
        text: (node.innerText || node.textContent || '').trim().slice(0, 100) || null,
        clickable: isClickable,
        selector: selector,
        children: children.length ? children : null
    };
}

JSON.stringify(serializeDOMWithIds(document.body));
        "#;

        // Execute JavaScript in Safari
        let escaped_js = js_to_inject.replace("\"", "\\\"");
        let applescript = format!(
            "tell application \"Safari\" to do JavaScript \"{}\" in current tab of first window",
            escaped_js
        );

        let output = Command::new("osascript")
            .arg("-e")
            .arg(&applescript)
            .output()
            .map_err(|e| AgentError::ToolError(format!("Failed to execute Safari JavaScript: {}", e)))?;

        if !output.status.success() {
            let error = String::from_utf8_lossy(&output.stderr);
            return Err(AgentError::ToolError(format!("AppleScript execution failed: {}", error)));
        }

        let dom_json = String::from_utf8_lossy(&output.stdout);

        // Parse and cache the DOM elements
        match serde_json::from_str::<Value>(&dom_json) {
            Ok(dom_data) => {
                self.cache_dom_elements(&dom_data)?;
                Ok(ToolResult::success(json!({
                    "dom_structure": dom_data,
                    "extraction_method": "Safari JavaScript injection",
                    "timestamp": SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs()
                })))
            }
            Err(e) => {
                log::warn!("Failed to parse DOM JSON: {}", e);
                Err(AgentError::ToolError(format!("Failed to parse DOM structure: {}", e)))
            }
        }
    }

    /// Caches DOM elements from the extracted structure
    fn cache_dom_elements(&self, dom_data: &Value) -> Result<(), AgentError> {
        let mut cache = self.element_cache.lock().map_err(|e| {
            AgentError::ToolError(format!("Failed to acquire element cache lock: {}", e))
        })?;

        cache.clear(); // Clear old cache
        let timestamp = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs();

        self.cache_elements_recursive(dom_data, &mut cache, timestamp);

        log::info!("Cached {} DOM elements from Safari", cache.len());
        Ok(())
    }

    /// Recursively caches elements from DOM structure
    fn cache_elements_recursive(&self, node: &Value, cache: &mut HashMap<u32, DomElement>, timestamp: u64) {
        if let Some(obj) = node.as_object() {
            if let (
                Some(id_val),
                Some(tag_val),
                Some(clickable_val),
                Some(selector_val)
            ) = (
                obj.get("id"),
                obj.get("tag"),
                obj.get("clickable"),
                obj.get("selector")
            ) {
                if let (
                    Some(id),
                    Some(tag),
                    Some(clickable),
                    Some(selector)
                ) = (
                    id_val.as_u64().map(|i| i as u32),
                    tag_val.as_str(),
                    clickable_val.as_bool(),
                    selector_val.as_str()
                ) {
                    let element = DomElement {
                        id,
                        tag: tag.to_string(),
                        element_id: obj.get("elementId").and_then(|v| v.as_str()).map(String::from),
                        class: obj.get("class").and_then(|v| v.as_str()).map(String::from),
                        role: obj.get("role").and_then(|v| v.as_str()).map(String::from),
                        text: obj.get("text").and_then(|v| v.as_str()).map(String::from),
                        clickable,
                        selector: selector.to_string(),
                        timestamp,
                    };

                    cache.insert(id, element);
                }
            }

            // Process children
            if let Some(children) = obj.get("children").and_then(|c| c.as_array()) {
                for child in children {
                    self.cache_elements_recursive(child, cache, timestamp);
                }
            }
        }
    }

    /// Clicks an element by its cached ID using Safari JavaScript injection
    pub fn click_element_by_id(&self, element_id: u32) -> Result<ToolResult, AgentError> {
        if !self.is_safari_active()? {
            return Ok(ToolResult::error("Safari is not the active application"));
        }

        let cache = self.element_cache.lock().map_err(|e| {
            AgentError::ToolError(format!("Failed to acquire element cache lock: {}", e))
        })?;

        let element = cache.get(&element_id).ok_or_else(|| {
            AgentError::ToolError(format!("Element with ID {} not found in cache", element_id))
        })?;

        if !element.clickable {
            return Ok(ToolResult::error(&format!("Element {} is not clickable", element_id)));
        }

        // Generate JavaScript to click the element
        let click_js = format!(
            r#"
var element = document.querySelector('[data-juno-id="{}"]');
if (element) {{
    element.click();
    'clicked';
}} else {{
    'element not found';
}}
            "#,
            element_id
        );

        let escaped_js = click_js.replace("\"", "\\\"");
        let applescript = format!(
            "tell application \"Safari\" to do JavaScript \"{}\" in current tab of first window",
            escaped_js
        );

        let output = Command::new("osascript")
            .arg("-e")
            .arg(&applescript)
            .output()
            .map_err(|e| AgentError::ToolError(format!("Failed to execute click JavaScript: {}", e)))?;

        if !output.status.success() {
            let error = String::from_utf8_lossy(&output.stderr);
            return Err(AgentError::ToolError(format!("Click execution failed: {}", error)));
        }

        let result = String::from_utf8_lossy(&output.stdout).trim().to_string();

        if result.contains("clicked") {
            Ok(ToolResult::success(json!({
                "clicked_element": {
                    "id": element_id,
                    "tag": element.tag,
                    "text": element.text,
                    "selector": element.selector
                },
                "method": "Safari JavaScript injection"
            })))
        } else {
            Ok(ToolResult::error(&format!("Failed to click element {}: {}", element_id, result)))
        }
    }

    /// Types text into an element by its cached ID
    pub fn type_in_element(&self, element_id: u32, text: &str) -> Result<ToolResult, AgentError> {
        if !self.is_safari_active()? {
            return Ok(ToolResult::error("Safari is not the active application"));
        }

        let cache = self.element_cache.lock().map_err(|e| {
            AgentError::ToolError(format!("Failed to acquire element cache lock: {}", e))
        })?;

        let element = cache.get(&element_id).ok_or_else(|| {
            AgentError::ToolError(format!("Element with ID {} not found in cache", element_id))
        })?;

        // Generate JavaScript to type in the element
        let type_js = format!(
            r#"
var element = document.querySelector('[data-juno-id="{}"]');
if (element) {{
    if (element.tagName === 'INPUT' || element.tagName === 'TEXTAREA') {{
        element.focus();
        element.value = '{}';
        element.dispatchEvent(new Event('input', {{ bubbles: true }}));
        element.dispatchEvent(new Event('change', {{ bubbles: true }}));
        'typed';
    }} else {{
        element.focus();
        element.innerText = '{}';
        'typed';
    }}
}} else {{
    'element not found';
}}
            "#,
            element_id,
            text.replace("'", "\\'").replace("\n", "\\n"),
            text.replace("'", "\\'").replace("\n", "\\n")
        );

        let escaped_js = type_js.replace("\"", "\\\"");
        let applescript = format!(
            "tell application \"Safari\" to do JavaScript \"{}\" in current tab of first window",
            escaped_js
        );

        let output = Command::new("osascript")
            .arg("-e")
            .arg(&applescript)
            .output()
            .map_err(|e| AgentError::ToolError(format!("Failed to execute type JavaScript: {}", e)))?;

        if !output.status.success() {
            let error = String::from_utf8_lossy(&output.stderr);
            return Err(AgentError::ToolError(format!("Type execution failed: {}", error)));
        }

        let result = String::from_utf8_lossy(&output.stdout).trim().to_string();

        if result.contains("typed") {
            Ok(ToolResult::success(json!({
                "typed_in_element": {
                    "id": element_id,
                    "tag": element.tag,
                    "text": text,
                    "selector": element.selector
                },
                "method": "Safari JavaScript injection"
            })))
        } else {
            Ok(ToolResult::error(&format!("Failed to type in element {}: {}", element_id, result)))
        }
    }

    /// Gets the current Safari tab URL
    pub fn get_current_url(&self) -> Result<ToolResult, AgentError> {
        if !self.is_safari_active()? {
            return Ok(ToolResult::error("Safari is not the active application"));
        }

        let applescript = "tell application \"Safari\" to get URL of current tab of first window";

        let output = Command::new("osascript")
            .arg("-e")
            .arg(applescript)
            .output()
            .map_err(|e| AgentError::ToolError(format!("Failed to get Safari URL: {}", e)))?;

        if !output.status.success() {
            let error = String::from_utf8_lossy(&output.stderr);
            return Err(AgentError::ToolError(format!("URL retrieval failed: {}", error)));
        }

        let url = String::from_utf8_lossy(&output.stdout).trim().to_string();

        Ok(ToolResult::success(json!({
            "current_url": url,
            "method": "Safari AppleScript"
        })))
    }

    /// Navigates Safari to a URL
    pub fn navigate_to_url(&self, url: &str) -> Result<ToolResult, AgentError> {
        let applescript = format!(
            "tell application \"Safari\" to set URL of current tab of first window to \"{}\"",
            url.replace("\"", "\\\"")
        );

        let output = Command::new("osascript")
            .arg("-e")
            .arg(&applescript)
            .output()
            .map_err(|e| AgentError::ToolError(format!("Failed to navigate Safari: {}", e)))?;

        if !output.status.success() {
            let error = String::from_utf8_lossy(&output.stderr);
            return Err(AgentError::ToolError(format!("Navigation failed: {}", error)));
        }

        // Wait a moment for navigation
        std::thread::sleep(std::time::Duration::from_millis(1000));

        Ok(ToolResult::success(json!({
            "navigated_to": url,
            "method": "Safari AppleScript"
        })))
    }

    /// Lists all cached clickable elements
    pub fn list_clickable_elements(&self) -> Result<ToolResult, AgentError> {
        let cache = self.element_cache.lock().map_err(|e| {
            AgentError::ToolError(format!("Failed to acquire element cache lock: {}", e))
        })?;

        let clickable_elements: Vec<_> = cache
            .values()
            .filter(|element| element.clickable)
            .map(|element| json!({
                "id": element.id,
                "tag": element.tag,
                "text": element.text,
                "role": element.role,
                "class": element.class,
                "selector": element.selector
            }))
            .collect();

        Ok(ToolResult::success(json!({
            "clickable_elements": clickable_elements,
            "total_count": clickable_elements.len(),
            "extraction_method": "Safari DOM cache"
        })))
    }
}

/// Global Safari DOM tools instance (thread-safe)
lazy_static::lazy_static! {
    static ref SAFARI_DOM_TOOLS: SafariDomTools = SafariDomTools::new();
}

/// Returns the global Safari DOM tools instance
pub fn get_safari_dom_tools() -> &'static SafariDomTools {
    &SAFARI_DOM_TOOLS
}

/// Returns the complete set of Safari DOM tool definitions
pub fn get_safari_dom_tool_definitions() -> Vec<ToolDefinition> {
    vec![
        ToolDefinition {
            name: "safari_extract_dom".to_string(),
            description: "Extracts structured DOM from the current Safari tab using JavaScript injection. Much faster than Playwright for Safari-specific automation.".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {},
                "required": []
            }),
        },
        ToolDefinition {
            name: "safari_click_element".to_string(),
            description: "Clicks a DOM element in Safari by its ID using JavaScript injection. Requires prior DOM extraction to get element IDs.".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "element_id": {
                        "type": "number",
                        "description": "The ID of the element to click (obtained from safari_extract_dom or safari_list_clickable_elements)"
                    }
                },
                "required": ["element_id"]
            }),
        },
        ToolDefinition {
            name: "safari_type_text".to_string(),
            description: "Types text into a DOM element in Safari by its ID using JavaScript injection.".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "element_id": {
                        "type": "number",
                        "description": "The ID of the element to type into (obtained from DOM extraction)"
                    },
                    "text": {
                        "type": "string",
                        "description": "The text to type into the element"
                    }
                },
                "required": ["element_id", "text"]
            }),
        },
        ToolDefinition {
            name: "safari_get_url".to_string(),
            description: "Gets the current URL of the active Safari tab using AppleScript.".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {},
                "required": []
            }),
        },
        ToolDefinition {
            name: "safari_navigate".to_string(),
            description: "Navigates Safari to a specific URL using AppleScript. Faster than browser controller for simple navigation.".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "url": {
                        "type": "string",
                        "description": "The URL to navigate to"
                    }
                },
                "required": ["url"]
            }),
        },
        ToolDefinition {
            name: "safari_list_clickable_elements".to_string(),
            description: "Lists all clickable elements from the cached DOM structure. Requires prior DOM extraction.".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {},
                "required": []
            }),
        },
    ]
}

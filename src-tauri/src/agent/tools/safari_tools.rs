//! Safari Tools Module
//!
//! Safari-specific browser automation tools using AppleScript and JavaScript injection.
//! Provides fast Safari DOM analysis and interaction capabilities as an alternative to
//! Playwright-based browser automation for Safari-specific workflows.
//!
//! ## Core Capabilities:
//! - Direct Safari JavaScript injection via AppleScript
//! - Structured DOM extraction and element caching
//! - Fast element clicking and text input
//! - Safari tab navigation and URL management
//! - Clickable element discovery and interaction
//!
//! ## Integration:
//! - Complements existing browser automation tools
//! - Optimized for Safari-specific workflows
//! - Thread-safe implementation with element caching
//! - Based on Opus Safari DOM injection patterns
//!
//! ## Usage:
//! Used by: Safari-specific automation, fast web interaction, DOM analysis
//! Registration: Tool definitions returned by `get_safari_tool_definitions()`

use crate::agent::core::{AgentError, ToolDefinition};
use serde_json::{json, Value};
use std::collections::HashMap;
use std::process::Command;
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};

/// Safari Tools - Provides Safari-specific JavaScript injection and DOM automation
#[derive(Debug)]
pub struct SafariTools {
    /// Cache for DOM elements with generated IDs
    element_cache: Arc<Mutex<HashMap<u32, SafariElement>>>,
    /// Counter for generating unique element IDs
    element_id_counter: Arc<Mutex<u32>>,
}

/// Represents a Safari DOM element with automation properties
#[derive(Debug, Clone)]
struct SafariElement {
    id: u32,
    tag: String,
    element_id: Option<String>,
    class: Option<String>,
    role: Option<String>,
    text: Option<String>,
    clickable: bool,
    selector: String,
    timestamp: u64,
}

impl SafariTools {
    /// Creates a new SafariTools instance
    pub fn new() -> Self {
        Self {
            element_cache: Arc::new(Mutex::new(HashMap::new())),
            element_id_counter: Arc::new(Mutex::new(1)),
        }
    }

    /// Checks if Safari is the currently active application
    pub fn is_safari_active(&self) -> Result<bool, AgentError> {
        log::debug!("Checking if Safari is the active application");

        let output = Command::new("osascript")
            .arg("-e")
            .arg("tell application \"System Events\" to get name of first application process whose frontmost is true")
            .output()
            .map_err(|e| AgentError::ToolError(format!("Failed to check active application: {}", e)))?;

        if !output.status.success() {
            let error = String::from_utf8_lossy(&output.stderr);
            return Err(AgentError::ToolError(format!("Failed to get active application: {}", error)));
        }

        let active_app = String::from_utf8_lossy(&output.stdout).trim().to_string();
        let is_safari = active_app == "Safari";

        log::debug!("Active application: {}, is Safari: {}", active_app, is_safari);
        Ok(is_safari)
    }

    /// Extracts structured DOM from the current Safari tab (based on Opus implementation)
    pub fn extract_dom(&self) -> Result<Value, AgentError> {
        log::info!("Extracting Safari DOM structure");

        if !self.is_safari_active()? {
            return Err(AgentError::ToolError("Safari is not the active application".to_string()));
        }

        // JavaScript function to serialize DOM structure (enhanced from Opus)
        let js_to_inject = r#"
function serializeDOMWithIds(node, idCounter = { value: 0 }) {
    if (!node || node.nodeType !== 1) return null;

    // Generate unique ID for this element
    const elementId = ++idCounter.value;

    // Add data attribute for identification
    node.setAttribute('data-juno-safari-id', elementId);

    const children = [...node.children]
        .map(child => serializeDOMWithIds(child, idCounter))
        .filter(Boolean);

    // Enhanced clickable detection
    const isClickable = typeof node.onclick === 'function' ||
        ['A', 'BUTTON', 'INPUT', 'SELECT', 'TEXTAREA', 'LABEL'].includes(node.tagName) ||
        node.hasAttribute('onclick') ||
        node.hasAttribute('href') ||
        node.getAttribute('role') === 'button' ||
        node.getAttribute('role') === 'link' ||
        node.getAttribute('role') === 'tab' ||
        node.getAttribute('tabindex') !== null ||
        node.style.cursor === 'pointer' ||
        getComputedStyle(node).cursor === 'pointer';

    // Generate improved CSS selector
    let selector = node.tagName.toLowerCase();
    if (node.id) {
        selector += '#' + node.id;
    } else if (node.className) {
        const classes = node.className.split(' ').filter(c => c.trim());
        if (classes.length > 0) {
            selector += '.' + classes.join('.');
        }
    }

    // Get text content with better extraction
    let textContent = null;
    if (node.innerText) {
        textContent = node.innerText.trim().slice(0, 100);
    } else if (node.textContent) {
        textContent = node.textContent.trim().slice(0, 100);
    } else if (node.value) {
        textContent = node.value.trim().slice(0, 100);
    } else if (node.alt) {
        textContent = node.alt.trim().slice(0, 100);
    } else if (node.title) {
        textContent = node.title.trim().slice(0, 100);
    }

    return {
        id: elementId,
        tag: node.tagName,
        elementId: node.id || null,
        class: node.className || null,
        role: node.getAttribute('role') || null,
        text: textContent || null,
        clickable: isClickable,
        selector: selector,
        children: children.length ? children : null
    };
}

JSON.stringify(serializeDOMWithIds(document.body));
        "#;

        // Execute JavaScript in Safari with proper escaping
        let escaped_js = js_to_inject.replace('"', r#"\""#);
        let applescript = format!(
            r#"tell application "Safari" to do JavaScript "{}" in current tab of first window"#,
            escaped_js
        );

        log::debug!("Executing Safari DOM extraction JavaScript");
        let output = Command::new("osascript")
            .arg("-e")
            .arg(&applescript)
            .output()
            .map_err(|e| AgentError::ToolError(format!("Failed to execute Safari JavaScript: {}", e)))?;

        if !output.status.success() {
            let error = String::from_utf8_lossy(&output.stderr);
            return Err(AgentError::ToolError(format!("Safari JavaScript execution failed: {}", error)));
        }

        let dom_json = String::from_utf8_lossy(&output.stdout);
        log::debug!("Safari DOM extraction completed, JSON length: {}", dom_json.len());

        // Parse and cache the DOM elements
        match serde_json::from_str::<Value>(&dom_json) {
            Ok(dom_data) => {
                self.cache_dom_elements(&dom_data)?;
                Ok(json!({
                    "dom_structure": dom_data,
                    "extraction_method": "Safari JavaScript injection",
                    "timestamp": SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs()
                }))
            }
            Err(e) => {
                log::warn!("Failed to parse Safari DOM JSON: {}", e);
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

        log::info!("Cached {} Safari DOM elements", cache.len());
        Ok(())
    }

    /// Recursively caches elements from DOM structure
    fn cache_elements_recursive(&self, node: &Value, cache: &mut HashMap<u32, SafariElement>, timestamp: u64) {
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
                    let element = SafariElement {
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
    pub fn click_element(&self, element_id: u32) -> Result<Value, AgentError> {
        log::info!("Clicking Safari element ID: {}", element_id);

        if !self.is_safari_active()? {
            return Err(AgentError::ToolError("Safari is not the active application".to_string()));
        }

        let cache = self.element_cache.lock().map_err(|e| {
            AgentError::ToolError(format!("Failed to acquire element cache lock: {}", e))
        })?;

        let element = cache.get(&element_id).ok_or_else(|| {
            AgentError::ToolError(format!("Element with ID {} not found in cache", element_id))
        })?;

        if !element.clickable {
            return Err(AgentError::ToolError(format!("Element {} is not clickable", element_id)));
        }

        // Generate JavaScript to click the element
        let click_js = format!(
            r#"
var element = document.querySelector('[data-juno-safari-id="{}"]');
if (element) {{
    element.focus();
    element.click();
    'SUCCESS: Element clicked';
}} else {{
    'ERROR: Element not found';
}}
            "#,
            element_id
        );

        let escaped_js = click_js.replace('"', r#"\""#);
        let applescript = format!(
            r#"tell application "Safari" to do JavaScript "{}" in current tab of first window"#,
            escaped_js
        );

        log::debug!("Executing Safari click JavaScript for element {}", element_id);
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
        log::debug!("Safari click result: {}", result);

        if result.contains("SUCCESS") {
            Ok(json!({
                "clicked_element": {
                    "id": element_id,
                    "tag": element.tag,
                    "text": element.text,
                    "selector": element.selector
                },
                "method": "Safari JavaScript injection"
            }))
        } else {
            Err(AgentError::ToolError(format!("Failed to click element {}: {}", element_id, result)))
        }
    }

    /// Types text into an element by its cached ID
    pub fn type_text(&self, element_id: u32, text: &str) -> Result<Value, AgentError> {
        log::info!("Typing text into Safari element ID: {}", element_id);

        if !self.is_safari_active()? {
            return Err(AgentError::ToolError("Safari is not the active application".to_string()));
        }

        let cache = self.element_cache.lock().map_err(|e| {
            AgentError::ToolError(format!("Failed to acquire element cache lock: {}", e))
        })?;

        let element = cache.get(&element_id).ok_or_else(|| {
            AgentError::ToolError(format!("Element with ID {} not found in cache", element_id))
        })?;

        // Generate JavaScript to type in the element
        let escaped_text = text.replace('"', r#"\""#).replace('\n', r#"\n"#).replace('\r', r#"\r"#).replace('\t', r#"\t"#);
        let type_js = format!(
            r#"
var element = document.querySelector('[data-juno-safari-id="{}"]');
if (element) {{
    element.focus();
    if (element.tagName === 'INPUT' || element.tagName === 'TEXTAREA') {{
        element.value = '{}';
        element.dispatchEvent(new Event('input', {{ bubbles: true }}));
        element.dispatchEvent(new Event('change', {{ bubbles: true }}));
    }} else if (element.contentEditable === 'true') {{
        element.innerText = '{}';
        element.dispatchEvent(new Event('input', {{ bubbles: true }}));
    }} else {{
        element.innerText = '{}';
    }}
    'SUCCESS: Text typed';
}} else {{
    'ERROR: Element not found';
}}
            "#,
            element_id, escaped_text, escaped_text, escaped_text
        );

        let escaped_js = type_js.replace('"', r#"\""#);
        let applescript = format!(
            r#"tell application "Safari" to do JavaScript "{}" in current tab of first window"#,
            escaped_js
        );

        log::debug!("Executing Safari type JavaScript for element {}", element_id);
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
        log::debug!("Safari type result: {}", result);

        if result.contains("SUCCESS") {
            Ok(json!({
                "typed_in_element": {
                    "id": element_id,
                    "tag": element.tag,
                    "text": text,
                    "selector": element.selector
                },
                "method": "Safari JavaScript injection"
            }))
        } else {
            Err(AgentError::ToolError(format!("Failed to type in element {}: {}", element_id, result)))
        }
    }

    /// Gets the current Safari tab URL
    pub fn get_current_url(&self) -> Result<Value, AgentError> {
        log::debug!("Getting current Safari tab URL");

        let applescript = r#"tell application "Safari" to get URL of current tab of first window"#;

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
        log::debug!("Current Safari URL: {}", url);

        Ok(json!({
            "current_url": url,
            "method": "Safari AppleScript"
        }))
    }

    /// Navigates Safari to a URL
    pub fn navigate_to_url(&self, url: &str) -> Result<Value, AgentError> {
        log::info!("Navigating Safari to URL: {}", url);

        let escaped_url = url.replace('"', r#"\""#);
        let applescript = format!(
            r#"tell application "Safari" to set URL of current tab of first window to "{}""#,
            escaped_url
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

        // Wait for navigation to complete
        std::thread::sleep(std::time::Duration::from_millis(2000));

        Ok(json!({
            "navigated_to": url,
            "method": "Safari AppleScript"
        }))
    }

    /// Lists all cached clickable elements
    pub fn list_clickable_elements(&self) -> Result<Value, AgentError> {
        log::debug!("Listing cached clickable Safari elements");

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

        log::debug!("Found {} clickable Safari elements", clickable_elements.len());

        Ok(json!({
            "clickable_elements": clickable_elements,
            "total_count": clickable_elements.len(),
            "extraction_method": "Safari DOM cache"
        }))
    }

    /// Executes custom JavaScript in the current Safari tab
    pub fn execute_javascript(&self, javascript: &str) -> Result<Value, AgentError> {
        log::info!("Executing custom JavaScript in Safari");

        if !self.is_safari_active()? {
            return Err(AgentError::ToolError("Safari is not the active application".to_string()));
        }

        let escaped_js = javascript.replace('"', r#"\""#);
        let applescript = format!(
            r#"tell application "Safari" to do JavaScript "{}" in current tab of first window"#,
            escaped_js
        );

        log::debug!("Executing custom Safari JavaScript");
        let output = Command::new("osascript")
            .arg("-e")
            .arg(&applescript)
            .output()
            .map_err(|e| AgentError::ToolError(format!("Failed to execute JavaScript: {}", e)))?;

        if !output.status.success() {
            let error = String::from_utf8_lossy(&output.stderr);
            return Err(AgentError::ToolError(format!("JavaScript execution failed: {}", error)));
        }

        let result = String::from_utf8_lossy(&output.stdout).trim().to_string();
        log::debug!("Safari JavaScript result: {}", result);

        Ok(json!({
            "javascript_result": result,
            "method": "Safari JavaScript injection"
        }))
    }

    /// Clears the element cache
    pub fn clear_cache(&self) -> Result<Value, AgentError> {
        log::debug!("Clearing Safari element cache");

        let mut cache = self.element_cache.lock().map_err(|e| {
            AgentError::ToolError(format!("Failed to acquire element cache lock: {}", e))
        })?;

        let cleared_count = cache.len();
        cache.clear();

        log::info!("Cleared {} Safari elements from cache", cleared_count);

        Ok(json!({
            "cleared_elements": cleared_count,
            "cache_status": "cleared"
        }))
    }
}

/// Global Safari tools instance (thread-safe)
lazy_static::lazy_static! {
    static ref SAFARI_TOOLS: SafariTools = SafariTools::new();
}

/// Returns the global Safari tools instance
pub fn get_safari_tools() -> &'static SafariTools {
    &SAFARI_TOOLS
}

/// Returns the complete set of Safari tool definitions for AI agent integration
pub fn get_safari_tool_definitions() -> Vec<ToolDefinition> {
    vec![
        ToolDefinition {
            name: "safari_extract_dom".to_string(),
            description: "Extracts structured DOM from the current Safari tab using JavaScript injection. Much faster than Playwright for Safari-specific automation. Caches elements with IDs for subsequent interaction.".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {},
                "required": []
            }),
        },
        ToolDefinition {
            name: "safari_click_element".to_string(),
            description: "Clicks a DOM element in Safari by its ID using JavaScript injection. Requires prior DOM extraction to get element IDs. Only works with clickable elements.".to_string(),
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
            description: "Types text into a DOM element in Safari by its ID using JavaScript injection. Works with input fields, textareas, and contenteditable elements.".to_string(),
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
            description: "Gets the current URL of the active Safari tab using AppleScript. Fast and reliable for Safari URL detection.".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {},
                "required": []
            }),
        },
        ToolDefinition {
            name: "safari_navigate".to_string(),
            description: "Navigates Safari to a specific URL using AppleScript. Faster than browser controller for simple navigation in Safari.".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "url": {
                        "type": "string",
                        "description": "The URL to navigate to (must include protocol like https://)"
                    }
                },
                "required": ["url"]
            }),
        },
        ToolDefinition {
            name: "safari_list_clickable_elements".to_string(),
            description: "Lists all clickable elements from the cached DOM structure. Requires prior DOM extraction. Useful for discovering interactive elements on the page.".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {},
                "required": []
            }),
        },
        ToolDefinition {
            name: "safari_execute_javascript".to_string(),
            description: "Executes custom JavaScript in the current Safari tab and returns the result. Powerful tool for custom DOM manipulation and data extraction.".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "javascript": {
                        "type": "string",
                        "description": "The JavaScript code to execute in Safari"
                    }
                },
                "required": ["javascript"]
            }),
        },
        ToolDefinition {
            name: "safari_clear_cache".to_string(),
            description: "Clears the Safari element cache. Useful when DOM structure has changed and fresh extraction is needed.".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {},
                "required": []
            }),
        },
    ]
}

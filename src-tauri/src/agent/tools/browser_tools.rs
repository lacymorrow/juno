use crate::agent::core::ToolDefinition;
use crate::agent::tools::browser_controller::BrowserController;
use serde_json::{json, Value};

pub fn get_browser_tool_definitions() -> Vec<ToolDefinition> {
    vec![
        ToolDefinition {
            name: "browser_navigate".to_string(),
            description: "Navigates the browser to a specified URL and returns the page content or title.".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "url": {
                        "type": "string",
                        "description": "The URL to navigate to (e.g., 'https://example.com')."
                    },
                    "wait_until": {
                        "type": "string",
                        "enum": ["load", "domcontentloaded", "networkidle", "commit"],
                        "description": "Optional event to wait for before considering navigation successful. 'networkidle' is often good for SPAs.",
                        "default": "load"
                    },
                    "timeout": {
                        "type": "number",
                        "description": "Optional navigation timeout in milliseconds.",
                        "default": 30000
                    }
                    // Add more options like returning content vs title later if needed
                },
                "required": ["url"]
            }),
        },
        ToolDefinition {
            name: "browser_extract_content".to_string(),
            description: "Extracts content (text or attributes) from the current browser page using CSS selectors.".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "selector": {
                        "type": "string",
                        "description": "CSS selector for the target element(s) (e.g., 'h1', 'div.product > span')."
                    },
                    "attribute": {
                        "type": "string",
                        "description": "Optional attribute to extract (e.g., 'href', 'src'). If omitted, extracts text content."
                    },
                    "multiple": {
                        "type": "boolean",
                        "description": "Set to true to return all matching elements, false for the first one.",
                        "default": false
                    },
                     "timeout": {
                        "type": "number",
                        "description": "Optional timeout in milliseconds to wait for the selector.",
                        "default": 5000
                    }
                },
                "required": ["selector"]
            }),
        },
        ToolDefinition {
            name: "browser_interact".to_string(),
            description: "Performs an interaction (click, type, select) on an element on the current browser page.".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "action": {
                        "type": "string",
                        "enum": ["click", "type", "select", "scroll"],
                        "description": "The type of interaction to perform."
                    },
                    "selector": {
                        "type": "string",
                        "description": "CSS selector for the target element."
                    },
                    "value": {
                        "type": "string",
                        "description": "Value for 'type' (text to type) or 'select' (option value to select)."
                    },
                    "scroll_direction": {
                         "type": "string",
                         "enum": ["up", "down"],
                         "description": "Direction for 'scroll' action ('up' or 'down'). Scrolls the page window."
                    },
                    "scroll_amount_pixels": {
                        "type": "number",
                        "description": "Amount to scroll in pixels for 'scroll' action.",
                        "default": 500
                    },
                    "wait_for_navigation": {
                        "type": "boolean",
                        "description": "Set to true if the interaction is expected to trigger navigation.",
                        "default": false
                    },
                    "timeout": {
                        "type": "number",
                        "description": "Optional timeout in milliseconds for the interaction/selector.",
                        "default": 5000
                    }
                    // 'select' action might need more specific options later
                },
                "required": ["action"] // Selector required for most, value for type/select
                // TODO: Add conditional requirements (e.g., selector required if action is not scroll)
            }),
        },
         ToolDefinition {
            name: "browser_get_current_url".to_string(),
            description: "Returns the current URL of the browser page.".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {}, // No input needed
            }),
        },
        ToolDefinition {
            name: "browser_screenshot".to_string(),
            description: "Takes a screenshot of the current browser page or a specific element.".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "selector": {
                        "type": "string",
                        "description": "Optional CSS selector to capture only a specific element. If omitted, captures the entire page."
                    },
                    "full_page": {
                        "type": "boolean",
                        "description": "Whether to capture the full scrollable page (true) or just the visible viewport (false).",
                        "default": false
                    },
                    "format": {
                        "type": "string",
                        "enum": ["png", "jpeg"],
                        "description": "Image format for the screenshot.",
                        "default": "png"
                    },
                    "quality": {
                        "type": "number",
                        "description": "Image quality from 0-100 (only applies to jpeg format).",
                        "default": 80
                    }
                }
            }),
        },
        // Add more tools like execute_script later if needed
    ]
}

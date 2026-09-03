//! Safari Tools Commands
//!
//! Tauri commands for Safari-specific DOM extraction and interaction.
//! Provides fast Safari automation using JavaScript injection via AppleScript.
//! Complements existing browser automation with Safari-optimized performance.

use crate::agent::core::ToolResult;
use crate::agent::tools::safari_tools::get_safari_tools;
use serde_json::Value;
use tauri::command;

/// Checks if Safari is the currently active application
#[command]
pub async fn safari_is_active() -> Result<bool, String> {
    get_safari_tools()
        .is_safari_active()
        .map_err(|e| e.to_string())
}

/// Extracts structured DOM from the current Safari tab
#[command]
pub async fn safari_extract_dom() -> Result<ToolResult, String> {
    match get_safari_tools().extract_dom() {
        Ok(output) => Ok(ToolResult {
            call_id: "safari_extract_dom".to_string(),
            output,
        }),
        Err(e) => Err(e.to_string()),
    }
}

/// Clicks a DOM element in Safari by its ID
#[command]
pub async fn safari_click_element(element_id: u32) -> Result<ToolResult, String> {
    match get_safari_tools().click_element(element_id) {
        Ok(output) => Ok(ToolResult {
            call_id: format!("safari_click_element_{}", element_id),
            output,
        }),
        Err(e) => Err(e.to_string()),
    }
}

/// Types text into a DOM element in Safari by its ID
#[command]
pub async fn safari_type_text(element_id: u32, text: String) -> Result<ToolResult, String> {
    match get_safari_tools().type_text(element_id, &text) {
        Ok(output) => Ok(ToolResult {
            call_id: format!("safari_type_text_{}_{}", element_id, text.len()),
            output,
        }),
        Err(e) => Err(e.to_string()),
    }
}

/// Gets the current URL of the active Safari tab
#[command]
pub async fn safari_get_url() -> Result<ToolResult, String> {
    match get_safari_tools().get_current_url() {
        Ok(output) => Ok(ToolResult {
            call_id: "safari_get_url".to_string(),
            output,
        }),
        Err(e) => Err(e.to_string()),
    }
}

/// Navigates Safari to a specific URL
#[command]
pub async fn safari_navigate(url: String) -> Result<ToolResult, String> {
    match get_safari_tools().navigate_to_url(&url) {
        Ok(output) => Ok(ToolResult {
            call_id: format!("safari_navigate_{}", url.len()),
            output,
        }),
        Err(e) => Err(e.to_string()),
    }
}

/// Lists all cached clickable elements from Safari DOM
#[command]
pub async fn safari_list_clickable_elements() -> Result<ToolResult, String> {
    match get_safari_tools().list_clickable_elements() {
        Ok(output) => Ok(ToolResult {
            call_id: "safari_list_clickable_elements".to_string(),
            output,
        }),
        Err(e) => Err(e.to_string()),
    }
}

/// Executes custom JavaScript in the current Safari tab
#[command]
pub async fn safari_execute_javascript(javascript: String) -> Result<ToolResult, String> {
    match get_safari_tools().execute_javascript(&javascript) {
        Ok(output) => Ok(ToolResult {
            call_id: format!("safari_execute_javascript_{}", javascript.len()),
            output,
        }),
        Err(e) => Err(e.to_string()),
    }
}

/// Clears the Safari element cache
#[command]
pub async fn safari_clear_cache() -> Result<ToolResult, String> {
    match get_safari_tools().clear_cache() {
        Ok(output) => Ok(ToolResult {
            call_id: "safari_clear_cache".to_string(),
            output,
        }),
        Err(e) => Err(e.to_string()),
    }
}

/// Execute Safari tool with parameters (for agent integration)
#[command]
pub async fn execute_safari_tool(
    tool_name: String,
    parameters: Value,
) -> Result<ToolResult, String> {
    let result = match tool_name.as_str() {
        "safari_extract_dom" => match get_safari_tools().extract_dom() {
            Ok(output) => Ok(output),
            Err(e) => Err(e.to_string()),
        },
        "safari_click_element" => {
            let element_id = parameters["element_id"]
                .as_u64()
                .ok_or("Missing or invalid element_id parameter")?
                as u32;
            match get_safari_tools().click_element(element_id) {
                Ok(output) => Ok(output),
                Err(e) => Err(e.to_string()),
            }
        }
        "safari_type_text" => {
            let element_id = parameters["element_id"]
                .as_u64()
                .ok_or("Missing or invalid element_id parameter")?
                as u32;
            let text = parameters["text"]
                .as_str()
                .ok_or("Missing or invalid text parameter")?;
            match get_safari_tools().type_text(element_id, text) {
                Ok(output) => Ok(output),
                Err(e) => Err(e.to_string()),
            }
        }
        "safari_get_url" => match get_safari_tools().get_current_url() {
            Ok(output) => Ok(output),
            Err(e) => Err(e.to_string()),
        },
        "safari_navigate" => {
            let url = parameters["url"]
                .as_str()
                .ok_or("Missing or invalid url parameter")?;
            match get_safari_tools().navigate_to_url(url) {
                Ok(output) => Ok(output),
                Err(e) => Err(e.to_string()),
            }
        }
        "safari_list_clickable_elements" => match get_safari_tools().list_clickable_elements() {
            Ok(output) => Ok(output),
            Err(e) => Err(e.to_string()),
        },
        "safari_execute_javascript" => {
            let js = parameters["javascript"]
                .as_str()
                .ok_or("Missing or invalid javascript parameter")?;
            match get_safari_tools().execute_javascript(js) {
                Ok(output) => Ok(output),
                Err(e) => Err(e.to_string()),
            }
        }
        "safari_clear_cache" => match get_safari_tools().clear_cache() {
            Ok(output) => Ok(output),
            Err(e) => Err(e.to_string()),
        },
        _ => Err(format!("Unknown Safari tool: {}", tool_name)),
    };

    match result {
        Ok(output) => Ok(ToolResult {
            call_id: tool_name,
            output,
        }),
        Err(e) => Err(e),
    }
}

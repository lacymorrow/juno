//! Safari DOM Commands
//!
//! Tauri commands for Safari-specific DOM extraction and interaction.
//! Provides fast Safari automation using JavaScript injection via AppleScript.

use crate::agent::tools::safari_dom_tools::get_safari_dom_tools;
use crate::agent::core::ToolResult;
use serde_json::Value;
use tauri::command;

/// Extracts structured DOM from the current Safari tab
#[command]
pub async fn safari_extract_dom() -> Result<ToolResult, String> {
    get_safari_dom_tools()
        .extract_safari_dom()
        .map_err(|e| e.to_string())
}

/// Clicks a DOM element in Safari by its ID
#[command]
pub async fn safari_click_element(element_id: u32) -> Result<ToolResult, String> {
    get_safari_dom_tools()
        .click_element_by_id(element_id)
        .map_err(|e| e.to_string())
}

/// Types text into a DOM element in Safari by its ID
#[command]
pub async fn safari_type_text(element_id: u32, text: String) -> Result<ToolResult, String> {
    get_safari_dom_tools()
        .type_in_element(element_id, &text)
        .map_err(|e| e.to_string())
}

/// Gets the current URL of the active Safari tab
#[command]
pub async fn safari_get_url() -> Result<ToolResult, String> {
    get_safari_dom_tools()
        .get_current_url()
        .map_err(|e| e.to_string())
}

/// Navigates Safari to a specific URL
#[command]
pub async fn safari_navigate(url: String) -> Result<ToolResult, String> {
    get_safari_dom_tools()
        .navigate_to_url(&url)
        .map_err(|e| e.to_string())
}

/// Lists all cached clickable elements from Safari DOM
#[command]
pub async fn safari_list_clickable_elements() -> Result<ToolResult, String> {
    get_safari_dom_tools()
        .list_clickable_elements()
        .map_err(|e| e.to_string())
}

/// Checks if Safari is the currently active application
#[command]
pub async fn safari_is_active() -> Result<bool, String> {
    get_safari_dom_tools()
        .is_safari_active()
        .map_err(|e| e.to_string())
}

/// Execute Safari DOM tool with parameters (for agent integration)
#[command]
pub async fn execute_safari_dom_tool(tool_name: String, parameters: Value) -> Result<ToolResult, String> {
    match tool_name.as_str() {
        "safari_extract_dom" => safari_extract_dom().await,
        "safari_click_element" => {
            let element_id = parameters["element_id"]
                .as_u64()
                .ok_or("Missing or invalid element_id parameter")?;
            safari_click_element(element_id as u32).await
        }
        "safari_type_text" => {
            let element_id = parameters["element_id"]
                .as_u64()
                .ok_or("Missing or invalid element_id parameter")?;
            let text = parameters["text"]
                .as_str()
                .ok_or("Missing or invalid text parameter")?;
            safari_type_text(element_id as u32, text.to_string()).await
        }
        "safari_get_url" => safari_get_url().await,
        "safari_navigate" => {
            let url = parameters["url"]
                .as_str()
                .ok_or("Missing or invalid url parameter")?;
            safari_navigate(url.to_string()).await
        }
        "safari_list_clickable_elements" => safari_list_clickable_elements().await,
        _ => Err(format!("Unknown Safari DOM tool: {}", tool_name)),
    }
}

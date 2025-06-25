//! # Accessibility Commands
//!
//! Tauri commands for native macOS accessibility functionality.
//! These commands provide element-level interaction capabilities
//! as an alternative to coordinate-based automation.

use crate::agent::tools::accessibility_tools::{AccessibilityElement, AccessibilityTools};
use serde_json::Value;
use std::sync::Mutex;
use tracing::{debug, info};

// Global accessibility tools instance
lazy_static::lazy_static! {
    static ref ACCESSIBILITY_TOOLS: Mutex<AccessibilityTools> = Mutex::new(AccessibilityTools::new());
}

/// Scan the frontmost application for clickable UI elements
#[tauri::command]
pub async fn accessibility_scan() -> Result<Vec<AccessibilityElement>, String> {
    debug!("accessibility_scan command called");

    let tools = ACCESSIBILITY_TOOLS.lock().map_err(|e| format!("Lock error: {}", e))?;
    let elements = tools.scan_frontmost_application()?;

    info!("accessibility_scan completed: found {} elements", elements.len());
    Ok(elements)
}

/// Click a UI element by its accessibility ID
#[tauri::command]
pub async fn accessibility_click(element_id: u32) -> Result<bool, String> {
    debug!("accessibility_click command called with element_id: {}", element_id);

    let tools = ACCESSIBILITY_TOOLS.lock().map_err(|e| format!("Lock error: {}", e))?;
    let success = tools.click_element_by_id(element_id)?;

    info!("accessibility_click completed: success = {}", success);
    Ok(success)
}

/// Test if accessibility permissions are granted
#[tauri::command]
pub async fn test_accessibility_permissions() -> Result<bool, String> {
    debug!("test_accessibility_permissions command called");

    // Try to initialize the accessibility tools to test permissions
    let tools = ACCESSIBILITY_TOOLS.lock().map_err(|e| format!("Lock error: {}", e))?;
    match tools.ensure_engine_initialized() {
        Ok(()) => {
            info!("Accessibility permissions are granted");
            Ok(true)
        }
        Err(e) if e.contains("permission") => {
            debug!("Accessibility permissions not granted: {}", e);
            Ok(false)
        }
        Err(e) => {
            debug!("Error testing accessibility permissions: {}", e);
            Err(e)
        }
    }
}

/// Get accessibility tool definitions for the agent
#[tauri::command]
pub async fn get_accessibility_tool_definitions() -> Result<Vec<Value>, String> {
    debug!("get_accessibility_tool_definitions command called");

    let definitions = AccessibilityTools::get_tool_definitions();

    info!("Returning {} accessibility tool definitions", definitions.len());
    Ok(definitions)
}

/// Execute an accessibility tool (for agent use)
#[tauri::command]
pub async fn execute_accessibility_tool(
    tool_name: String,
    parameters: Value,
    app_handle: tauri::AppHandle,
) -> Result<Value, String> {
    debug!("execute_accessibility_tool command called: {}", tool_name);

    // Clone the tools to avoid holding the lock across await
    let tools = {
        let tools_guard = ACCESSIBILITY_TOOLS.lock().map_err(|e| format!("Lock error: {}", e))?;
        tools_guard.clone()
    };

    let result = tools.execute_tool(&tool_name, &parameters, &app_handle).await?;

    info!("execute_accessibility_tool completed: {}", tool_name);
    Ok(result)
}

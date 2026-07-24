//! # Display Information Tools
//!
//! Provides screen resolution and display information without requiring screenshots.
//! This eliminates the need for the agent to take screenshots just to get screen dimensions.
//!
//! ## Core Capabilities:
//! - Get screen resolution for main display
//! - Get information about all active displays
//! - Calculate screen center point
//! - Get display bounds and positioning
//! - Multi-monitor support
//!
//! ## Integration:
//! - Uses existing display utilities from mcp-server-os-level
//! - Provides standard resolution coordinates for API compliance
//! - Compatible with coordinate transformation system
//!
//! ## Usage
//! Used by: Computer use agents, coordinate calculations, display-aware workflows
//! Registration: Called via `register_display_info_tools()`

use crate::agent::core::ToolDefinition;
use crate::agent::implementations::tool_provider::LocalToolProvider;
use crate::utils::coordinates::get_current_standard_resolution;
use serde_json::{json, Value};
use tracing::{error, info};

#[cfg(target_os = "macos")]
use computer_use_ai_sdk::platforms::macos::display::{get_active_displays, get_main_display};

/// Registers display information tools with the tool provider.
///
/// This provides the agent with direct access to screen resolution and display information
/// without requiring screenshots, significantly improving performance and reducing overhead.
///
/// # Arguments
/// * `provider` - Mutable reference to LocalToolProvider for tool registration
/// * `app_handle` - Tauri app handle for state access
///
/// # Tools Registered
/// - `get_screen_info`: Get comprehensive screen and display information
/// - `get_screen_center`: Calculate center point of main display
/// - `get_display_list`: Get information about all active displays
///
/// # Returns
/// `Result<(), String>` - Success or error message
pub async fn register_display_info_tools(
    provider: &mut LocalToolProvider,
    _app_handle: tauri::AppHandle,
) -> Result<(), String> {
    info!("Registering display information tools...");

    // Tool for getting comprehensive screen information
    let get_screen_info_def = ToolDefinition {
        name: "get_screen_info".to_string(),
        description: "Get comprehensive screen and display information including resolution, center point, and bounds without taking a screenshot.".to_string(),
        input_schema: json!({
            "type": "object",
            "properties": {},
            "required": []
        }),
        api_type: None,
        beta_flag: None,
    };

    let get_screen_info_exec = move |_input: Value| async move { get_screen_info_impl().await };
    provider
        .register_async_tool(get_screen_info_def, get_screen_info_exec)
        .await;
    info!("Registered tool: get_screen_info");

    // Tool for getting screen center point
    let get_screen_center_def = ToolDefinition {
        name: "get_screen_center".to_string(),
        description: "Get the center point coordinates of the main display in standard resolution coordinates.".to_string(),
        input_schema: json!({
            "type": "object",
            "properties": {},
            "required": []
        }),
        api_type: None,
        beta_flag: None,
    };

    let get_screen_center_exec = move |_input: Value| async move { get_screen_center_impl().await };
    provider
        .register_async_tool(get_screen_center_def, get_screen_center_exec)
        .await;
    info!("Registered tool: get_screen_center");

    // Tool for getting information about all displays
    let get_display_list_def = ToolDefinition {
        name: "get_display_list".to_string(),
        description: "Get detailed information about all active displays including bounds, resolution, and which is the main display.".to_string(),
        input_schema: json!({
            "type": "object",
            "properties": {},
            "required": []
        }),
        api_type: None,
        beta_flag: None,
    };

    let get_display_list_exec = move |_input: Value| async move { get_display_list_impl().await };
    provider
        .register_async_tool(get_display_list_def, get_display_list_exec)
        .await;
    info!("Registered tool: get_display_list");

    Ok(())
}

/// Implementation for getting comprehensive screen information
async fn get_screen_info_impl() -> Result<Value, String> {
    #[cfg(target_os = "macos")]
    {
        match get_main_display() {
            Ok(display) => {
                let bounds = display.bounds;
                let actual_width = bounds.size.width as u32;
                let actual_height = bounds.size.height as u32;

                // Get the standard resolution coordinates that the agent should use
                let (standard_width, standard_height) =
                    get_current_standard_resolution().unwrap_or((actual_width, actual_height));

                // Calculate center point in standard resolution coordinates
                let center_x = standard_width / 2;
                let center_y = standard_height / 2;

                info!(
                    "Screen info: actual {}x{}, standard {}x{}, center ({}, {})",
                    actual_width,
                    actual_height,
                    standard_width,
                    standard_height,
                    center_x,
                    center_y
                );

                Ok(json!({
                    "resolution": {
                        "width": standard_width,
                        "height": standard_height,
                        "actual_width": actual_width,
                        "actual_height": actual_height
                    },
                    "center": {
                        "x": center_x,
                        "y": center_y
                    },
                    "bounds": {
                        "origin": {
                            "x": bounds.origin.x,
                            "y": bounds.origin.y
                        },
                        "size": {
                            "width": bounds.size.width,
                            "height": bounds.size.height
                        }
                    },
                    "display_id": display.id,
                    "is_main": display.is_main,
                    "coordinate_system": "standard_resolution"
                }))
            }
            Err(e) => {
                error!("Failed to get main display info: {}", e);
                Err(format!("Failed to get display information: {}", e))
            }
        }
    }

    #[cfg(not(target_os = "macos"))]
    {
        // Fallback for non-macOS platforms
        let (width, height) = get_current_standard_resolution().unwrap_or((1366, 768)); // Common fallback resolution

        let center_x = width / 2;
        let center_y = height / 2;

        info!(
            "Screen info (fallback): {}x{}, center ({}, {})",
            width, height, center_x, center_y
        );

        Ok(json!({
            "resolution": {
                "width": width,
                "height": height,
                "actual_width": width,
                "actual_height": height
            },
            "center": {
                "x": center_x,
                "y": center_y
            },
            "bounds": {
                "origin": { "x": 0, "y": 0 },
                "size": { "width": width, "height": height }
            },
            "display_id": 0,
            "is_main": true,
            "coordinate_system": "standard_resolution"
        }))
    }
}

/// Implementation for getting screen center point
async fn get_screen_center_impl() -> Result<Value, String> {
    #[cfg(target_os = "macos")]
    {
        match get_main_display() {
            Ok(display) => {
                let bounds = display.bounds;
                let actual_width = bounds.size.width as u32;
                let actual_height = bounds.size.height as u32;

                // Get the standard resolution coordinates
                let (standard_width, standard_height) =
                    get_current_standard_resolution().unwrap_or((actual_width, actual_height));

                let center_x = standard_width / 2;
                let center_y = standard_height / 2;

                info!(
                    "Screen center: ({}, {}) in standard resolution {}x{}",
                    center_x, center_y, standard_width, standard_height
                );

                Ok(json!({
                    "x": center_x,
                    "y": center_y,
                    "resolution": {
                        "width": standard_width,
                        "height": standard_height
                    },
                    "coordinate_system": "standard_resolution"
                }))
            }
            Err(e) => {
                error!("Failed to get main display for center calculation: {}", e);
                Err(format!("Failed to calculate screen center: {}", e))
            }
        }
    }

    #[cfg(not(target_os = "macos"))]
    {
        let (width, height) = get_current_standard_resolution().unwrap_or((1366, 768));

        let center_x = width / 2;
        let center_y = height / 2;

        info!(
            "Screen center (fallback): ({}, {}) in {}x{}",
            center_x, center_y, width, height
        );

        Ok(json!({
            "x": center_x,
            "y": center_y,
            "resolution": {
                "width": width,
                "height": height
            },
            "coordinate_system": "standard_resolution"
        }))
    }
}

/// Implementation for getting list of all displays
async fn get_display_list_impl() -> Result<Value, String> {
    #[cfg(target_os = "macos")]
    {
        match get_active_displays() {
            Ok(displays) => {
                let mut display_list = Vec::new();

                for display in displays {
                    let bounds = display.bounds;
                    let actual_width = bounds.size.width as u32;
                    let actual_height = bounds.size.height as u32;

                    // For multi-monitor setups, we might want to handle standard resolution per display
                    // For now, use the coordinate system's standard resolution
                    let (standard_width, standard_height) =
                        get_current_standard_resolution().unwrap_or((actual_width, actual_height));

                    display_list.push(json!({
                        "id": display.id,
                        "is_main": display.is_main,
                        "bounds": {
                            "origin": {
                                "x": bounds.origin.x,
                                "y": bounds.origin.y
                            },
                            "size": {
                                "width": bounds.size.width,
                                "height": bounds.size.height
                            }
                        },
                        "resolution": {
                            "actual_width": actual_width,
                            "actual_height": actual_height,
                            "standard_width": standard_width,
                            "standard_height": standard_height
                        },
                        "center": {
                            "x": standard_width / 2,
                            "y": standard_height / 2
                        }
                    }));
                }

                info!("Found {} active displays", display_list.len());

                Ok(json!({
                    "displays": display_list,
                    "count": display_list.len(),
                    "coordinate_system": "standard_resolution"
                }))
            }
            Err(e) => {
                error!("Failed to get active displays: {}", e);
                Err(format!("Failed to get display list: {}", e))
            }
        }
    }

    #[cfg(not(target_os = "macos"))]
    {
        let (width, height) = get_current_standard_resolution().unwrap_or((1366, 768));

        let display_list = vec![json!({
            "id": 0,
            "is_main": true,
            "bounds": {
                "origin": { "x": 0, "y": 0 },
                "size": { "width": width, "height": height }
            },
            "resolution": {
                "actual_width": width,
                "actual_height": height,
                "standard_width": width,
                "standard_height": height
            },
            "center": {
                "x": width / 2,
                "y": height / 2
            }
        })];

        info!(
            "Display list (fallback): single display {}x{}",
            width, height
        );

        Ok(json!({
            "displays": display_list,
            "count": 1,
            "coordinate_system": "standard_resolution"
        }))
    }
}

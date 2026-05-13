//! # Visible Windows Tool
//!
//! Provides `list_visible_windows` — a lightweight tool that returns all
//! on-screen application windows without taking a screenshot.
//!
//! Uses CGWindowListCopyWindowInfo (~1 ms) and requires only Screen Recording
//! permission (not Accessibility).
//!
//! ## Usage
//! Used by: computer-use agents that need spatial context before deciding
//! which app to interact with.
//! Registration: `register_visible_windows_tools()`

use crate::agent::core::ToolDefinition;
use crate::agent::implementations::tool_provider::LocalToolProvider;
use serde_json::{json, Value};
use tracing::{debug, warn};

#[cfg(target_os = "macos")]
use computer_use_ai_sdk::platforms::macos::display::list_visible_windows;

/// Registers the `list_visible_windows` tool with the given provider.
pub async fn register_visible_windows_tools(
    provider: &mut LocalToolProvider,
    _app_handle: tauri::AppHandle,
) -> Result<(), String> {
    debug!("Registering visible_windows tool...");

    let def = ToolDefinition {
        name: "list_visible_windows".to_string(),
        description: "Returns all currently visible application windows on screen with their \
            titles, positions, and sizes. Use this to understand what the user is looking at \
            before deciding which app to interact with. Much lighter than taking a screenshot."
            .to_string(),
        input_schema: json!({
            "type": "object",
            "properties": {},
            "required": []
        }),
        api_type: None,
        beta_flag: None,
    };

    let exec = move |_input: Value| async move { list_visible_windows_impl().await };
    provider.register_async_tool(def, exec).await;
    debug!("Registered tool: list_visible_windows");

    Ok(())
}

async fn list_visible_windows_impl() -> Result<Value, String> {
    #[cfg(target_os = "macos")]
    {
        tokio::task::spawn_blocking(|| list_visible_windows())
            .await
            .map_err(|e| format!("list_visible_windows task panicked: {}", e))?
            .map(|windows| {
                let count = windows.len();
                let items: Vec<Value> = windows
                    .iter()
                    .map(|w| {
                        json!({
                            "app_name": w.app_name,
                            "window_title": w.window_title,
                            "position": { "x": w.position.0, "y": w.position.1 },
                            "size": { "w": w.size.0, "h": w.size.1 },
                            "is_frontmost": w.is_frontmost,
                            "layer": w.layer,
                        })
                    })
                    .collect();
                json!({
                    "windows": items,
                    "count": count
                })
            })
            .map_err(|e| {
                warn!("list_visible_windows failed: {}", e);
                format!("Failed to list visible windows: {}", e)
            })
    }

    #[cfg(not(target_os = "macos"))]
    {
        Ok(json!({ "windows": [], "count": 0, "note": "only supported on macOS" }))
    }
}

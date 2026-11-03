//! # Visible Windows Tool
//!
//! Provides `list_visible_windows` — a lightweight tool that returns all
//! on-screen application windows without taking a screenshot.
//!
//! Uses CGWindowListCopyWindowInfo (~1 ms) on macOS, which requires only Screen
//! Recording permission (not Accessibility), and EnumWindows on Windows.
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

/// Registers both `list_visible_windows` and `get_app_windows` tools.
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

    // ── get_app_windows ──────────────────────────────────────────────────────

    let get_app_windows_def = ToolDefinition {
        name: "get_app_windows".to_string(),
        description: "Returns the visible window titles for a specific running application. \
            Use this to check what is open in an app before deciding whether to navigate \
            or open a new window (e.g. 'is Gmail already open in Chrome?'). \
            Returns an empty list if the app has no visible windows or is not running."
            .to_string(),
        input_schema: json!({
            "type": "object",
            "properties": {
                "app_name": {
                    "type": "string",
                    "description": "The application name to query (e.g. 'Google Chrome', 'Safari', 'Finder')"
                }
            },
            "required": ["app_name"]
        }),
        api_type: None,
        beta_flag: None,
    };

    let exec_gaw = move |input: Value| async move { get_app_windows_impl(input).await };
    provider
        .register_async_tool(get_app_windows_def, exec_gaw)
        .await;
    debug!("Registered tool: get_app_windows");

    Ok(())
}

/// Returns visible window titles for a given app name (case-insensitive).
/// Shared by both the agent tool and the Tauri command.
pub async fn window_titles_for_app(app_name: &str) -> Result<Vec<String>, String> {
    #[cfg(target_os = "macos")]
    {
        let target = app_name.to_lowercase();
        let windows = tokio::task::spawn_blocking(list_visible_windows)
            .await
            .map_err(|e| format!("get_app_windows task panicked: {}", e))?
            .map_err(|e| {
                warn!("get_app_windows failed: {}", e);
                format!("Failed to list windows: {}", e)
            })?;

        let titles: Vec<String> = windows
            .into_iter()
            .filter(|w| w.app_name.to_lowercase() == target)
            .filter_map(|w| w.window_title)
            .collect();

        Ok(titles)
    }

    #[cfg(not(target_os = "macos"))]
    {
        // Matches a process (`msedge`) or part of a window title ("Edge"), since
        // a person's name for an app is rarely its executable's.
        let target = app_name.to_lowercase();
        let records = tokio::task::spawn_blocking(windows_of_apps)
            .await
            .map_err(|e| format!("get_app_windows task panicked: {}", e))?;
        Ok(records
            .into_iter()
            .filter(|w| {
                w.owner.to_lowercase().contains(&target)
                    || w.title
                        .as_deref()
                        .is_some_and(|t| t.to_lowercase().contains(&target))
            })
            .filter_map(|w| w.title)
            .collect())
    }
}

async fn get_app_windows_impl(input: Value) -> Result<Value, String> {
    let app_name = input["app_name"]
        .as_str()
        .ok_or_else(|| "Missing required parameter 'app_name'".to_string())?
        .to_string();

    if app_name.is_empty() {
        return Err("app_name cannot be empty".to_string());
    }

    let titles = window_titles_for_app(&app_name).await?;
    let count = titles.len();

    Ok(json!({
        "app_name": app_name,
        "window_titles": titles,
        "count": count
    }))
}

async fn list_visible_windows_impl() -> Result<Value, String> {
    #[cfg(target_os = "macos")]
    {
        tokio::task::spawn_blocking(list_visible_windows)
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
        let frontmost = computer_use_ai_sdk::window_target::native::frontmost().map(|w| w.id);
        let windows = tokio::task::spawn_blocking(windows_of_apps)
            .await
            .map_err(|e| format!("list_visible_windows task panicked: {}", e))?;
        let items: Vec<Value> = windows
            .iter()
            .map(|w| {
                json!({
                    "app_name": w.owner,
                    "window_title": w.title,
                    "window_id": w.id,
                    "position": { "x": w.bounds.0, "y": w.bounds.1 },
                    "size": { "w": w.bounds.2, "h": w.bounds.3 },
                    "is_frontmost": frontmost == Some(w.id),
                    "layer": w.layer,
                })
            })
            .collect();
        Ok(json!({ "windows": items, "count": items.len() }))
    }
}

/// Other apps' ordinary windows, front to back: not Juno, not the shell.
#[cfg(not(target_os = "macos"))]
fn windows_of_apps() -> Vec<computer_use_ai_sdk::window_target::WindowRecord> {
    let own_pid = std::process::id() as i32;
    computer_use_ai_sdk::window_target::native::window_records()
        .into_iter()
        .filter(|w| w.pid != own_pid && w.is_user_layer())
        .collect()
}

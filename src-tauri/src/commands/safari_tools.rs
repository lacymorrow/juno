//! Safari Tools Commands
//!
//! Tauri commands for Safari-specific DOM extraction and interaction.
//! Provides fast Safari automation using JavaScript injection via AppleScript.
//! Complements existing browser automation with Safari-optimized performance.

use crate::agent::core::ToolResult;
use crate::agent::tools::safari_tools::{get_safari_tools, validate_navigation_url};
use serde_json::Value;
use tauri::{command, AppHandle};

/// Checks if Safari is the currently active application
///
/// Takes the handle because the AppleScript behind it needs Automation
/// consent, and the gate puts the ask on screen (see `send_applescript`).
#[command]
pub async fn safari_is_active(app_handle: AppHandle) -> Result<bool, String> {
    get_safari_tools()
        .is_safari_active(&app_handle)
        .await
        .map_err(|e| e.to_string())
}

/// Extracts structured DOM from the current Safari tab
#[command]
pub async fn safari_extract_dom(app_handle: AppHandle) -> Result<ToolResult, String> {
    match get_safari_tools().extract_dom(&app_handle).await {
        Ok(output) => Ok(ToolResult {
            call_id: "safari_extract_dom".to_string(),
            output,
        }),
        Err(e) => Err(e.to_string()),
    }
}

/// Clicks a DOM element in Safari by its ID
#[command]
pub async fn safari_click_element(
    app_handle: AppHandle,
    element_id: u32,
) -> Result<ToolResult, String> {
    match get_safari_tools()
        .click_element(&app_handle, element_id)
        .await
    {
        Ok(output) => Ok(ToolResult {
            call_id: format!("safari_click_element_{}", element_id),
            output,
        }),
        Err(e) => Err(e.to_string()),
    }
}

/// Types text into a DOM element in Safari by its ID
#[command]
pub async fn safari_type_text(
    app_handle: AppHandle,
    element_id: u32,
    text: String,
) -> Result<ToolResult, String> {
    match get_safari_tools()
        .type_text(&app_handle, element_id, &text)
        .await
    {
        Ok(output) => Ok(ToolResult {
            call_id: format!("safari_type_text_{}_{}", element_id, text.len()),
            output,
        }),
        Err(e) => Err(e.to_string()),
    }
}

/// Gets the current URL of the active Safari tab
#[command]
pub async fn safari_get_url(app_handle: AppHandle) -> Result<ToolResult, String> {
    match get_safari_tools().get_current_url(&app_handle).await {
        Ok(output) => Ok(ToolResult {
            call_id: "safari_get_url".to_string(),
            output,
        }),
        Err(e) => Err(e.to_string()),
    }
}

/// Navigates Safari to a specific URL
///
/// Only http/https URLs are accepted (validated here and again in the tool
/// layer, which is the choke point for all navigation paths).
#[command]
pub async fn safari_navigate(app_handle: AppHandle, url: String) -> Result<ToolResult, String> {
    let url = validate_navigation_url(&url).map_err(|e| e.to_string())?;
    match get_safari_tools().navigate_to_url(&app_handle, &url).await {
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

/// Error returned when arbitrary Safari JavaScript is requested through the
/// webview-invokable command surface.
///
/// Any JavaScript running in the webview can call Tauri commands, so an open
/// `safari_execute_javascript` command would let compromised or injected
/// frontend code run arbitrary JS in the user's real Safari session while
/// completely bypassing the agent runner's approval flow. The command layer
/// has no way to prompt the user (the approval loop lives in
/// `AgentRunner::check_batch_approval`), and no frontend code path uses this
/// command, so the honest gate is to refuse it here outright. The agent
/// pipeline reaches JS execution only through
/// [`execute_safari_tool_for_agent`], after the risk classifier marks the
/// call High and the user approves it (security audit 2026-02-08, #15/#20).
const ARBITRARY_JS_COMMAND_BLOCKED: &str = "safari_execute_javascript is not available as a \
    direct command: arbitrary JavaScript execution in Safari runs only through the agent \
    pipeline, where it is classified High risk and requires user approval \
    (security audit 2026-02-08, items #15/#20)";

/// The webview surface's answer for a tool name: `Some(refusal)` when the name
/// is arbitrary JavaScript, `None` when the call may be dispatched.
///
/// Its own function so it can be asserted on its own. The commands around it
/// now take an `AppHandle` (the Automation gate asks through it) and a unit
/// test cannot build a Tauri app — the EventLoop has to be created on the main
/// thread, which `cargo test` never hands a test. The refusal is decided from
/// the name alone and needs neither.
fn refuse_arbitrary_js(tool_name: &str) -> Option<&'static str> {
    if tool_name == "safari_execute_javascript" {
        Some(ARBITRARY_JS_COMMAND_BLOCKED)
    } else {
        None
    }
}

/// The error for a tool name the agent dispatcher does not know. Named for the
/// same reason as [`refuse_arbitrary_js`].
fn unknown_tool_error(tool_name: &str) -> String {
    format!("Unknown Safari tool: {}", tool_name)
}

/// Executes custom JavaScript in the current Safari tab — BLOCKED at the
/// command layer.
///
/// This command is intentionally a stub that always errors; see
/// [`ARBITRARY_JS_COMMAND_BLOCKED`] for the rationale. It stays registered so
/// existing invokers get a clear error instead of a missing-command failure.
#[command]
pub async fn safari_execute_javascript(javascript: String) -> Result<ToolResult, String> {
    log::warn!(
        "Blocked direct safari_execute_javascript command invocation ({} bytes of JavaScript)",
        javascript.len()
    );
    Err(ARBITRARY_JS_COMMAND_BLOCKED.to_string())
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

/// Execute Safari tool with parameters — webview-invokable command surface.
///
/// Delegates to [`execute_safari_tool_for_agent`] for every tool EXCEPT
/// arbitrary JavaScript execution, which is refused here because the command
/// layer bypasses the agent runner's approval flow (see
/// [`ARBITRARY_JS_COMMAND_BLOCKED`]).
#[command]
pub async fn execute_safari_tool(
    app_handle: AppHandle,
    tool_name: String,
    parameters: Value,
) -> Result<ToolResult, String> {
    if let Some(refusal) = refuse_arbitrary_js(&tool_name) {
        log::warn!(
            "Blocked direct execute_safari_tool command invocation for arbitrary JavaScript"
        );
        return Err(refusal.to_string());
    }
    execute_safari_tool_for_agent(&app_handle, tool_name, parameters).await
}

/// Dispatches a Safari tool invocation coming from the AGENT pipeline.
///
/// This is a plain function, not a Tauri command, so it is not reachable from
/// webview JavaScript. By the time the agent's tool executor calls it,
/// `AgentRunner::check_batch_approval` has already classified the call
/// (`safari_execute_javascript` is High risk in `risk_classifier.rs`) and
/// obtained human approval where required — that gate, not the advisory
/// substring filter in the tool layer, is the control for arbitrary Safari JS
/// (security audit 2026-02-08, items #15/#20).
///
/// `app_handle` is what lets the Automation gate put its card on screen. Every
/// tool below that sends AppleScript needs it; the two cache-only tools do not.
pub async fn execute_safari_tool_for_agent(
    app_handle: &AppHandle,
    tool_name: String,
    parameters: Value,
) -> Result<ToolResult, String> {
    let result = match tool_name.as_str() {
        "safari_extract_dom" => match get_safari_tools().extract_dom(app_handle).await {
            Ok(output) => Ok(output),
            Err(e) => Err(e.to_string()),
        },
        "safari_click_element" => {
            let element_id = parameters["element_id"]
                .as_u64()
                .ok_or("Missing or invalid element_id parameter")?
                as u32;
            match get_safari_tools()
                .click_element(app_handle, element_id)
                .await
            {
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
            match get_safari_tools()
                .type_text(app_handle, element_id, text)
                .await
            {
                Ok(output) => Ok(output),
                Err(e) => Err(e.to_string()),
            }
        }
        "safari_get_url" => match get_safari_tools().get_current_url(app_handle).await {
            Ok(output) => Ok(output),
            Err(e) => Err(e.to_string()),
        },
        "safari_navigate" => {
            let url = parameters["url"]
                .as_str()
                .ok_or("Missing or invalid url parameter")?;
            // Scheme validation happens in navigate_to_url; validate here too so
            // the command layer rejects bad URLs even if the tool layer changes.
            let url = validate_navigation_url(url).map_err(|e| e.to_string())?;
            match get_safari_tools().navigate_to_url(app_handle, &url).await {
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
            match get_safari_tools().execute_javascript(app_handle, js).await {
                Ok(output) => Ok(output),
                Err(e) => Err(e.to_string()),
            }
        }
        "safari_clear_cache" => match get_safari_tools().clear_cache() {
            Ok(output) => Ok(output),
            Err(e) => Err(e.to_string()),
        },
        _ => Err(unknown_tool_error(&tool_name)),
    };

    match result {
        Ok(output) => Ok(ToolResult {
            call_id: tool_name,
            output,
        }),
        Err(e) => Err(e),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn direct_safari_execute_javascript_command_is_blocked() {
        // The webview-invokable command must refuse arbitrary JS regardless
        // of payload — even trivially harmless code — because approval
        // happens in the agent runner, which this path bypasses.
        let result = safari_execute_javascript("1 + 1".to_string()).await;
        let err = match result {
            Err(e) => e,
            Ok(_) => panic!("safari_execute_javascript command must always error"),
        };
        assert!(err.contains("agent pipeline"), "unexpected error: {}", err);
        assert!(err.contains("#15/#20"), "unexpected error: {}", err);
    }

    #[test]
    fn execute_safari_tool_command_blocks_arbitrary_js_branch() {
        // Both directions, which the previous version of this test did not
        // check: the JS tool is refused, and an ordinary tool is not.
        let refusal = refuse_arbitrary_js("safari_execute_javascript")
            .expect("the command surface must refuse arbitrary JavaScript");
        assert!(
            refusal.contains("agent pipeline"),
            "unexpected refusal: {}",
            refusal
        );
        assert!(
            refusal.contains("#15/#20"),
            "unexpected refusal: {}",
            refusal
        );

        assert!(
            refuse_arbitrary_js("safari_extract_dom").is_none(),
            "an ordinary Safari tool must not be refused as arbitrary JavaScript"
        );
    }

    #[test]
    fn agent_dispatch_rejects_unknown_tools() {
        let err = unknown_tool_error("safari_totally_unknown");
        assert!(
            err.contains("Unknown Safari tool"),
            "unexpected error: {}",
            err
        );
        assert!(
            err.contains("safari_totally_unknown"),
            "the error must name the tool that was asked for: {}",
            err
        );
    }
}

//! Safari Tools Commands
//!
//! Tauri commands for Safari-specific DOM extraction and interaction.
//! Provides fast Safari automation using JavaScript injection via AppleScript.
//! Complements existing browser automation with Safari-optimized performance.

use crate::agent::core::ToolResult;
use crate::agent::tools::safari_tools::{get_safari_tools, validate_navigation_url};
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
///
/// Only http/https URLs are accepted (validated here and again in the tool
/// layer, which is the choke point for all navigation paths).
#[command]
pub async fn safari_navigate(url: String) -> Result<ToolResult, String> {
    let url = validate_navigation_url(&url).map_err(|e| e.to_string())?;
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
    tool_name: String,
    parameters: Value,
) -> Result<ToolResult, String> {
    if tool_name == "safari_execute_javascript" {
        log::warn!(
            "Blocked direct execute_safari_tool command invocation for arbitrary JavaScript"
        );
        return Err(ARBITRARY_JS_COMMAND_BLOCKED.to_string());
    }
    execute_safari_tool_for_agent(tool_name, parameters).await
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
pub async fn execute_safari_tool_for_agent(
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
            // Scheme validation happens in navigate_to_url; validate here too so
            // the command layer rejects bad URLs even if the tool layer changes.
            let url = validate_navigation_url(url).map_err(|e| e.to_string())?;
            match get_safari_tools().navigate_to_url(&url) {
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

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

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

    #[tokio::test]
    async fn execute_safari_tool_command_blocks_arbitrary_js_branch() {
        let result = execute_safari_tool(
            "safari_execute_javascript".to_string(),
            json!({"javascript": "document.title"}),
        )
        .await;
        let err = match result {
            Err(e) => e,
            Ok(_) => panic!("execute_safari_tool must refuse the arbitrary-JS branch"),
        };
        assert!(err.contains("agent pipeline"), "unexpected error: {}", err);
    }

    #[tokio::test]
    async fn agent_dispatch_rejects_unknown_tools() {
        let result =
            execute_safari_tool_for_agent("safari_totally_unknown".to_string(), json!({})).await;
        let err = match result {
            Err(e) => e,
            Ok(_) => panic!("unknown tool must error"),
        };
        assert!(
            err.contains("Unknown Safari tool"),
            "unexpected error: {}",
            err
        );
    }
}

//! # Juno's own computer tool, offered to the Claude CLI over MCP
//!
//! The Claude CLI runs its own agent loop in a subprocess, so anything it does
//! to the desktop happens outside Juno unless Juno hands it a tool. It used to
//! be handed `juno-cua`, a separate binary spawned as a stdio MCP server. That
//! worked, in the sense that the mouse moved. It also meant every mouse action
//! took a path through another process that knows nothing about Juno: the
//! smooth-movement setting was never read, the cursor overlay was never told,
//! and the person watching their pointer fly around had no indication that
//! Juno was the one doing it. Both features existed the whole time. Neither was
//! on the road the agent actually travelled.
//!
//! So Juno serves the tool itself. This is an MCP server inside the running
//! app, bound to loopback, whose `computer` tool is the same
//! [`run_computer_action`] the in-process provider calls. One implementation,
//! one cursor, one setting that means something.
//!
//! ## Shape
//!
//! Streamable HTTP, which the CLI accepts as `"type": "http"` in `--mcp-config`.
//! JSON-RPC 2.0 in, a single JSON response out; no SSE stream, because every
//! call here is one request and one answer.
//!
//! ## Who may call it
//!
//! Loopback only, plus a bearer token minted per app run and handed to the CLI
//! through the config file. Loopback alone is not a boundary: any process on
//! the machine can reach 127.0.0.1, and this tool moves the mouse and presses
//! keys. The token is what makes "only the child we spawned" true.

use std::net::SocketAddr;
use std::sync::OnceLock;

use axum::{
    extract::State,
    http::{HeaderMap, StatusCode},
    response::IntoResponse,
    routing::post,
    Json, Router,
};
use serde_json::{json, Value};
use tracing::{debug, error, info, warn};

use crate::agent::core::AgentError;
use crate::agent::providers::cli_approval;
use crate::agent::tools::anthropic_computer_use::{create_versioned_tools, run_computer_action};
use crate::agent::tools::settings_tool;
use crate::agent::tools::tool_versioning::{ApiVersion, ToolVersionConfig};
use crate::constants::agent::tool_names;

/// The MCP protocol revision this server speaks.
const PROTOCOL_VERSION: &str = "2025-06-18";

/// The overlay cursor identity for work the Claude CLI drives.
///
/// One identity, because the CLI is one agent however many turns it takes.
const CLI_CURSOR_ID: &str = "claude-cli";

/// The primary cursor slot: the CLI is one agent, so its cursor wears the
/// color chosen in Settings, Appearance, like any single run (see
/// `cursor_overlay::cursor_color`). This used to be a fixed systemPink; pink
/// is now one of the choices.
const CLI_CURSOR_COLOR: &str = crate::constants::ui::agent_session_colors::SLOT_0;

/// Where the server is listening, and the token that gets you in.
#[derive(Clone, Debug)]
pub struct Endpoint {
    pub url: String,
    pub token: String,
}

static ENDPOINT: OnceLock<Endpoint> = OnceLock::new();

#[derive(Clone)]
struct ServerState {
    app: tauri::AppHandle,
    token: String,
}

/// Start the server if it is not already up, and describe how to reach it.
///
/// Idempotent: the first query of the app's life pays for the bind, and every
/// query after it gets the same address back.
pub async fn ensure_running(app: &tauri::AppHandle) -> Result<Endpoint, AgentError> {
    if let Some(endpoint) = ENDPOINT.get() {
        return Ok(endpoint.clone());
    }

    let token = uuid::Uuid::new_v4().to_string();
    let state = ServerState {
        app: app.clone(),
        token: token.clone(),
    };

    let router = Router::new()
        .route("/mcp", post(handle_rpc))
        .with_state(state);

    // Port 0: the OS picks a free one. A fixed port would collide with a second
    // Juno, and with whatever else happens to want it.
    let listener = tokio::net::TcpListener::bind::<SocketAddr>(([127, 0, 0, 1], 0).into())
        .await
        .map_err(|e| {
            AgentError::ConfigurationError(format!("Could not start Juno's tool server: {e}"))
        })?;

    let addr = listener.local_addr().map_err(|e| {
        AgentError::ConfigurationError(format!("Could not read the tool server address: {e}"))
    })?;

    tauri::async_runtime::spawn(async move {
        if let Err(e) = axum::serve(listener, router).await {
            error!("[JunoMCP] Tool server stopped: {}", e);
        }
    });

    let endpoint = Endpoint {
        url: format!("http://{addr}/mcp"),
        token,
    };
    info!("[JunoMCP] Serving Juno's computer tool at {}", endpoint.url);

    // Another task may have won the race; either way one endpoint wins and
    // both callers get the same one.
    Ok(ENDPOINT.get_or_init(|| endpoint).clone())
}

/// The `--mcp-config` contents pointing the CLI at this server.
pub fn mcp_config(endpoint: &Endpoint) -> Value {
    json!({
        "mcpServers": {
            "juno": {
                "type": "http",
                "url": endpoint.url,
                "headers": {
                    "Authorization": format!("Bearer {}", endpoint.token)
                }
            }
        }
    })
}

async fn handle_rpc(
    State(state): State<ServerState>,
    headers: HeaderMap,
    Json(request): Json<Value>,
) -> impl IntoResponse {
    if !authorized(&headers, &state.token) {
        warn!("[JunoMCP] Rejected a request with no valid token");
        return (StatusCode::UNAUTHORIZED, Json(json!({}))).into_response();
    }

    let method = request.get("method").and_then(Value::as_str).unwrap_or("");
    let id = request.get("id").cloned();

    // A notification has no id and expects no answer.
    let Some(id) = id else {
        debug!("[JunoMCP] Notification: {}", method);
        return StatusCode::ACCEPTED.into_response();
    };

    let outcome = match method {
        "initialize" => Ok(json!({
            "protocolVersion": PROTOCOL_VERSION,
            "capabilities": { "tools": {} },
            "serverInfo": { "name": "juno", "version": env!("CARGO_PKG_VERSION") }
        })),
        "ping" => Ok(json!({})),
        "tools/list" => Ok(json!({ "tools": tool_list() })),
        "tools/call" => call_tool(&state.app, &request).await,
        other => Err(RpcError::method_not_found(other)),
    };

    let body = match outcome {
        Ok(result) => json!({ "jsonrpc": "2.0", "id": id, "result": result }),
        Err(e) => json!({ "jsonrpc": "2.0", "id": id, "error": e.to_json() }),
    };
    (StatusCode::OK, Json(body)).into_response()
}

fn authorized(headers: &HeaderMap, token: &str) -> bool {
    headers
        .get(axum::http::header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer "))
        .is_some_and(|presented| presented == token)
}

/// The tools this server offers, in MCP's shape.
///
/// `computer`, plus `approve` (LAC-4058). Bash and file editing are the
/// CLI's own, and better there: it already has them, they need no desktop,
/// and routing them through Juno would add a hop for nothing. What Juno has
/// that the CLI does not is a pointer people can see — and a person to ask
/// before something goes out: `approve` is the CLI's
/// `--permission-prompt-tool`, routed into Juno's approval sheet.
fn tool_list() -> Vec<Value> {
    // Only the schema is served here. The CLI picks its own tool types, so the
    // Anthropic tool version on these definitions is never sent anywhere.
    let mut tools: Vec<Value> =
        create_versioned_tools(ToolVersionConfig::new(ApiVersion::Computer20251124))
            .into_iter()
            .filter(|tool| tool.name == "computer")
            .map(|tool| {
                json!({
                    "name": tool.name,
                    "description": tool.description,
                    "inputSchema": tool.input_schema,
                })
            })
            .collect();
    // Juno's own settings: find, show and change them, with the guardrail
    // settings refused (`settings::registry`).
    let settings = settings_tool::definition();
    tools.push(json!({
        "name": settings.name,
        "description": settings.description,
        "inputSchema": settings.input_schema,
    }));
    tools.push(json!({
        "name": cli_approval::APPROVE_TOOL_NAME,
        "description": "Internal: Juno's permission prompt. The Claude CLI calls this \
            automatically when a tool needs the person's approval. Never call it directly.",
        "inputSchema": {
            "type": "object",
            "properties": {
                "tool_name": { "type": "string" },
                "input": { "type": "object" },
                "tool_use_id": { "type": "string" }
            },
            "required": ["tool_name", "input"]
        }
    }));
    tools
}

async fn call_tool(app: &tauri::AppHandle, request: &Value) -> Result<Value, RpcError> {
    let params = request.get("params").cloned().unwrap_or_else(|| json!({}));
    let name = params.get("name").and_then(Value::as_str).unwrap_or("");

    let arguments = params
        .get("arguments")
        .cloned()
        .unwrap_or_else(|| json!({}));

    if name == cli_approval::APPROVE_TOOL_NAME {
        return Ok(cli_approval::handle_approve(app, &arguments).await);
    }
    if name == tool_names::SETTINGS {
        return Ok(match settings_tool::run(app, arguments).await {
            Ok(value) => to_mcp_content(value),
            Err(e) => json!({
                "content": [{ "type": "text", "text": e }],
                "isError": true
            }),
        });
    }
    if name != "computer" {
        return Err(RpcError::method_not_found(name));
    }

    match run_computer_action(app, arguments, None, CLI_CURSOR_ID, CLI_CURSOR_COLOR).await {
        Ok(value) => Ok(to_mcp_content(value)),
        // A tool that failed is reported through `isError`, not as a protocol
        // error: the model is supposed to read it and try something else, and a
        // JSON-RPC error would end the turn instead.
        Err(e) => Ok(json!({
            "content": [{ "type": "text", "text": e }],
            "isError": true
        })),
    }
}

/// Turn a computer-tool result into MCP content blocks.
///
/// A screenshot comes back as a struct carrying base64 PNG data, and MCP has a
/// block type for exactly that, so it goes across as an image the model can
/// look at rather than as a wall of base64 in a text block.
fn to_mcp_content(value: Value) -> Value {
    if let Some(image) = value.get("base64_image").and_then(Value::as_str) {
        return json!({
            "content": [{
                "type": "image",
                "data": image,
                "mimeType": "image/png"
            }],
            "isError": false
        });
    }

    let text = match value.as_str() {
        Some(s) => s.to_string(),
        None => serde_json::to_string(&value).unwrap_or_else(|_| value.to_string()),
    };
    json!({
        "content": [{ "type": "text", "text": text }],
        "isError": false
    })
}

struct RpcError {
    code: i64,
    message: String,
}

impl RpcError {
    fn method_not_found(what: &str) -> Self {
        Self {
            code: -32601,
            message: format!("Unknown method or tool: {what}"),
        }
    }

    fn to_json(&self) -> Value {
        json!({ "code": self.code, "message": self.message })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_computer_settings_and_approve_tools_are_offered() {
        let tools = tool_list();
        assert_eq!(tools.len(), 3);
        assert_eq!(tools[0]["name"], "computer");
        assert!(
            tools[0]["inputSchema"]["properties"]["action"].is_object(),
            "the CLI needs the schema to call it"
        );
        // The permission prompt tool (LAC-4058) must exist on the server the
        // CLI is pointed at, or every gated call would hang and die.
        // Juno's own settings, so "change my voice" works on the CLI path.
        assert_eq!(tools[1]["name"], "settings");
        assert!(tools[1]["inputSchema"]["properties"]["action"].is_object());
        assert_eq!(tools[2]["name"], "approve");
        assert!(tools[2]["inputSchema"]["properties"]["tool_name"].is_object());
    }

    #[test]
    fn the_config_points_at_the_server_and_carries_the_token() {
        let endpoint = Endpoint {
            url: "http://127.0.0.1:51234/mcp".to_string(),
            token: "secret".to_string(),
        };
        let config = mcp_config(&endpoint);
        let server = &config["mcpServers"]["juno"];
        assert_eq!(server["type"], "http");
        assert_eq!(server["url"], "http://127.0.0.1:51234/mcp");
        assert_eq!(server["headers"]["Authorization"], "Bearer secret");
    }

    #[test]
    fn a_request_without_the_token_is_refused() {
        let mut headers = HeaderMap::new();
        assert!(!authorized(&headers, "secret"), "no header at all");
        headers.insert(
            axum::http::header::AUTHORIZATION,
            "Bearer wrong".parse().expect("valid header"),
        );
        assert!(!authorized(&headers, "secret"), "wrong token");
        headers.insert(
            axum::http::header::AUTHORIZATION,
            "Bearer secret".parse().expect("valid header"),
        );
        assert!(authorized(&headers, "secret"));
    }

    #[test]
    fn a_screenshot_crosses_as_an_image_not_as_base64_text() {
        let result = to_mcp_content(json!({
            "base64_image": "iVBORw0KGgo=",
            "original_width": 100,
            "original_height": 50
        }));
        assert_eq!(result["content"][0]["type"], "image");
        assert_eq!(result["content"][0]["mimeType"], "image/png");
        assert_eq!(result["isError"], false);
    }

    #[test]
    fn anything_else_crosses_as_text() {
        let result = to_mcp_content(json!({ "output": "done" }));
        assert_eq!(result["content"][0]["type"], "text");
        assert_eq!(result["isError"], false);
    }
}

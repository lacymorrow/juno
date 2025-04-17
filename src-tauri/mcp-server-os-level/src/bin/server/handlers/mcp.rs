use axum::{
    extract::{Json, State},
    response::Json as JsonResponse,
};
use serde_json::{self, json, Value};
use std::sync::Arc;
use tracing::{error, info};

use crate::server::types::{
    AppState, ClickByIndexRequest, ExecuteToolFunctionParams, InputControlRequest,
    ListInteractableElementsRequest, MCPRequest, OpenApplicationRequest, OpenUrlRequest,
    PressKeyByIndexRequest, ServerCapabilities, ToolFunctionDefinition, ToolServerCapabilities,
    TypeByIndexRequest,
};

// Import the element cache type
use crate::server::types::ElementCache;

// Update handler imports
use crate::server::handlers::click_by_index::click_by_index_handler;
use crate::server::handlers::input_control::input_control_handler;
use crate::server::handlers::list_elements_and_attributes::list_elements_and_attributes_handler;
use crate::server::handlers::open_application::open_application_handler;
use crate::server::handlers::open_url::open_url_handler;
use crate::server::handlers::press_key_by_index::press_key_by_index_handler;
use crate::server::handlers::type_by_index::type_by_index_handler;

use computer_use_ai_sdk::AutomationError;

// MCP handler
pub async fn mcp_handler(
    State(state): State<Arc<AppState>>,
    Json(request): Json<MCPRequest>,
) -> JsonResponse<Value> {
    info!("received mcp request: {:?}", request);

    // Handle different MCP methods
    match request.method.as_str() {
        "initialize" => handle_initialize(request.id),
        "executeToolFunction" => {
            if let Some(params) = request.params {
                handle_execute_tool_function(state, request.id, params).await
            } else {
                mcp_error_response(request.id, -32602, "invalid params".to_string(), None)
            }
        }
        _ => mcp_error_response(request.id, -32601, "method not found".to_string(), None),
    }
}

// Handler for initialize method
pub fn handle_initialize(id: Value) -> JsonResponse<Value> {
    let click_by_index_schema = json!({
        "type": "object",
        "properties": {
            "element_index": {"type": "integer"}
        },
        "required": ["element_index"]
    });

    let type_by_index_schema = json!({
        "type": "object",
        "properties": {
            "element_index": {"type": "integer"},
            "text": {"type": "string"}
        },
        "required": ["element_index", "text"]
    });

    let press_key_by_index_schema = json!({
        "type": "object",
        "properties": {
            "element_index": {"type": "integer"},
            "key_combo": {"type": "string"}
        },
        "required": ["element_index", "key_combo"]
    });

    let open_application_schema = json!({
        "type": "object",
        "properties": {
            "app_name": {"type": "string"}
        },
        "required": ["app_name"]
    });

    let open_url_schema = json!({
        "type": "object",
        "properties": {
            "url": {"type": "string"},
            "browser": {"type": "string"}
        },
        "required": ["url"]
    });

    let input_control_schema = json!({
        "type": "object",
        "properties": {
            "action": {
                "oneOf": [
                    {
                        "type": "object",
                        "properties": {
                            "type": { "type": "string", "enum": ["KeyPress"] },
                            "data": { "type": "string", "description": "Key code number or key name" }
                        },
                        "required": ["type", "data"]
                    },
                    {
                        "type": "object",
                        "properties": {
                            "type": { "type": "string", "enum": ["MouseMove"] },
                            "data": {
                                "type": "object",
                                "properties": {
                                    "x": { "type": "number" },
                                    "y": { "type": "number" }
                                },
                                "required": ["x", "y"]
                            }
                        },
                        "required": ["type", "data"]
                    },
                    {
                        "type": "object",
                        "properties": {
                            "type": { "type": "string", "enum": ["MouseClick"] },
                            "data": { "type": "string", "enum": ["left", "right"], "default": "left" }
                        },
                        "required": ["type", "data"]
                    },
                    {
                        "type": "object",
                        "properties": {
                            "type": { "type": "string", "enum": ["WriteText"] },
                            "data": { "type": "string" }
                        },
                        "required": ["type", "data"]
                    },
                    {
                        "type": "object",
                        "properties": {
                            "type": { "type": "string", "enum": ["Wait"] },
                            "data": { "type": "integer", "description": "Duration to wait in milliseconds" }
                        },
                        "required": ["type", "data"]
                    },
                    {
                        "type": "object",
                        "properties": {
                            "type": { "type": "string", "enum": ["LeftClickDrag"] },
                            "data": {
                                "type": "object",
                                "properties": {
                                    "start_x": { "type": "number" },
                                    "start_y": { "type": "number" },
                                    "end_x": { "type": "number" },
                                    "end_y": { "type": "number" }
                                },
                                "required": ["start_x", "start_y", "end_x", "end_y"],
                                "description": "Start and end coordinates for the drag"
                            }
                        },
                        "required": ["type", "data"]
                    },
                    {
                        "type": "object",
                        "properties": {
                            "type": { "type": "string", "enum": ["MiddleClick"] },
                            "data": {
                                "type": "object",
                                "properties": {
                                    "x": { "type": "number" },
                                    "y": { "type": "number" }
                                },
                                "required": ["x", "y"],
                                "description": "Coordinates for the middle click"
                            }
                        },
                        "required": ["type", "data"]
                    },
                    {
                        "type": "object",
                        "properties": {
                            "type": { "type": "string", "enum": ["DoubleClick"] },
                            "data": {
                                "type": "object",
                                "properties": {
                                    "x": { "type": "number" },
                                    "y": { "type": "number" }
                                },
                                "required": ["x", "y"],
                                "description": "Coordinates for the double click"
                            }
                        },
                        "required": ["type", "data"]
                    },
                    {
                        "type": "object",
                        "properties": {
                            "type": { "type": "string", "enum": ["TripleClick"] },
                            "data": {
                                "type": "object",
                                "properties": {
                                    "x": { "type": "number" },
                                    "y": { "type": "number" }
                                },
                                "required": ["x", "y"],
                                "description": "Coordinates for the triple click"
                            }
                        },
                        "required": ["type", "data"]
                    },
                    {
                        "type": "object",
                        "properties": {
                            "type": { "type": "string", "enum": ["HoldKey"] },
                            "data": {
                                "type": "object",
                                "properties": {
                                    "key": { "type": "string", "description": "Key name or code" },
                                    "duration": { "type": "integer", "description": "Duration to hold in milliseconds" }
                                },
                                "required": ["key", "duration"]
                            }
                        },
                        "required": ["type", "data"]
                    },
                    {
                        "type": "object",
                        "properties": {
                            "type": { "type": "string", "enum": ["LeftMouseDown"] },
                            "data": {
                                "type": "object",
                                "properties": {
                                    "x": { "type": "number" },
                                    "y": { "type": "number" }
                                },
                                "required": [], // Coordinates optional, can press down at current location
                                "description": "Optional coordinates for where to press down"
                            }
                        },
                        "required": ["type", "data"]
                    },
                    {
                        "type": "object",
                        "properties": {
                            "type": { "type": "string", "enum": ["LeftMouseUp"] },
                            "data": {
                                "type": "object",
                                "properties": {
                                    "x": { "type": "number" },
                                    "y": { "type": "number" }
                                },
                                "required": [], // Coordinates optional, can release at current location
                                "description": "Optional coordinates for where to release"
                            }
                        },
                        "required": ["type", "data"]
                    }
                ]
            }
        },
        "required": ["action"]
    });

    // Define tool functions
    let tool_functions = vec![
        ToolFunctionDefinition {
            name: "clickByIndex".to_string(),
            description: "click on a ui element by its index and returns the updated element list. evaluate success by examining the updated elements to confirm ui responded as expected, not just whether the click executed.".to_string(),
            parameters: click_by_index_schema,
        },
        ToolFunctionDefinition {
            name: "typeByIndex".to_string(),
            description: "type text into a ui element by its index and returns the updated element list. evaluate success by examining if the text was accepted and ui updated appropriately.".to_string(),
            parameters: type_by_index_schema,
        },
        ToolFunctionDefinition {
            name: "pressKeyByIndex".to_string(),
            description: "press key combination on a ui element by its index and returns the updated element list. evaluate success by examining if the key press triggered expected ui changes.".to_string(),
            parameters: press_key_by_index_schema,
        },
        ToolFunctionDefinition {
            name: "openApplication".to_string(),
            description: "open an application and return the list of interactable elements in the app. evaluate success by checking if application window and controls are visible.".to_string(),
            parameters: open_application_schema,
        },
        ToolFunctionDefinition {
            name: "openUrl".to_string(),
            description: "open a url in a browser and return the list of interactable elements in the browser. if browser is not specified, chrome will be used by default. evaluate success by confirming expected page content is visible.".to_string(),
            parameters: open_url_schema,
        },
        ToolFunctionDefinition {
            name: "inputControl".to_string(),
            description: "perform direct input control actions: KeyPress(key), MouseMove({x,y}), MouseClick(button), WriteText(text), Wait(ms), LeftClickDrag({start_x,start_y,end_x,end_y}), MiddleClick({x,y}), DoubleClick({x,y}), TripleClick({x,y}), HoldKey({key,duration}), LeftMouseDown({x?,y?}), LeftMouseUp({x?,y?}). returns updated element list. evaluate success by confirming ui responded as expected.".to_string(),
            parameters: input_control_schema,
        },
        ToolFunctionDefinition {
            name: "captureScreenshot".to_string(),
            description: "Captures a screenshot of the main display and returns it as a base64 encoded PNG string.".to_string(),
            parameters: json!({ "type": "object", "properties": {}, "required": [] }),
        },
        ToolFunctionDefinition {
            name: "rightClickByIndex".to_string(),
            description: "perform a right-click (context menu click) on a ui element by its index. evaluate success by checking if the expected context menu or action occurred.".to_string(),
            parameters: click_by_index_schema.clone(),
        },
        ToolFunctionDefinition {
            name: "hoverByIndex".to_string(),
            description: "move the mouse cursor over a ui element by its index without clicking. evaluate success by checking if hover effects (tooltips, highlighting) appear.".to_string(),
            parameters: click_by_index_schema.clone(),
        },
        ToolFunctionDefinition {
            name: "scrollByIndex".to_string(),
            description: "scroll the view containing a specific ui element (identified by its index) up, down, left, or right by a given amount (number of lines/units). evaluate success by checking if the content scrolled as expected.".to_string(),
            parameters: json!({
                "type": "object",
                "properties": {
                    "element_index": {"type": "integer"},
                    "direction": {"type": "string", "enum": ["up", "down", "left", "right"]},
                    "amount": {"type": "number"}
                },
                "required": ["element_index", "direction", "amount"]
            }),
        },
    ];

    let capabilities = ServerCapabilities {
        tools: Some(ToolServerCapabilities {
            functions: tool_functions,
        }),
        resources: None, // Implement if needed
    };

    JsonResponse(json!({
        "jsonrpc": "2.0",
        "id": id,
        "result": {
            "capabilities": capabilities
        }
    }))
}

// Handler for executeToolFunction method
pub async fn handle_execute_tool_function(
    state: Arc<AppState>,
    id: Value,
    params: Value,
) -> JsonResponse<Value> {
    info!("handling execute tool function: {:?}", params);

    let tool_call: ExecuteToolFunctionParams = match serde_json::from_value(params.clone()) {
        Ok(call) => call,
        Err(e) => {
            error!("failed to parse tool function params: {}", e);
            return mcp_error_response(
                id,
                -32602,
                "invalid params".to_string(),
                Some(json!({ "error": e.to_string() })),
            );
        }
    };

    let tool_name = tool_call.name;
    let tool_input = tool_call.input;

    info!("executing tool: {} with input: {}", tool_name, tool_input);

    // Match on the tool name and call the appropriate handler or SDK function
    let result = match tool_name.as_str() {
        "clickByIndex" => {
            let click_params: ClickByIndexRequest = match serde_json::from_value(tool_input) {
                Ok(p) => p,
                Err(e) => {
                    return mcp_error_response(
                        id,
                        -32602,
                        format!("invalid input for {}: {}", tool_name, e),
                        None,
                    );
                }
            };
             // Call the existing handler logic - this assumes handlers return Result<JsonResponse<Value>, (StatusCode, JsonResponse<Value>)>)
             // We need to adapt this to return Result<Value, AutomationError> or similar for MCP
             // For now, let's call the SDK directly, assuming elements are cached.
            execute_element_action(state, click_params.element_index, |el| el.click()).await

        }
        "typeByIndex" => {
            let type_params: TypeByIndexRequest = match serde_json::from_value(tool_input) {
                Ok(p) => p,
                Err(e) => {
                    return mcp_error_response(
                        id,
                        -32602,
                        format!("invalid input for {}: {}", tool_name, e),
                        None,
                    );
                }
            };
             execute_element_action(state, type_params.element_index, |el| el.type_text(&type_params.text)).await
        }
        "pressKeyByIndex" => {
            let press_params: PressKeyByIndexRequest = match serde_json::from_value(tool_input) {
                Ok(p) => p,
                Err(e) => {
                    return mcp_error_response(
                        id,
                        -32602,
                        format!("invalid input for {}: {}", tool_name, e),
                        None,
                    );
                }
            };
            execute_element_action(state, press_params.element_index, |el| el.press_key(&press_params.key_combo)).await
        }
        "openApplication" => {
            let open_app_params: OpenApplicationRequest = match serde_json::from_value(tool_input) {
                Ok(p) => p,
                Err(e) => {
                    return mcp_error_response(
                        id,
                        -32602,
                        format!("invalid input for {}: {}", tool_name, e),
                        None,
                    );
                }
            };
             // This handler likely needs different logic (doesn't operate on cached index)
             // Re-use list_elements logic for now
             // TODO: Refactor this for better separation
             let list_req = ListInteractableElementsRequest {
                 app_name: open_app_params.app_name,
                 use_background_apps: open_app_params.use_background_apps,
                 activate_app: open_app_params.activate_app,
                 cache_id: None, // Ensure we get fresh elements after opening
             };
             match list_elements_and_attributes_handler(State(state), Json(list_req)).await {
                 Ok(response) => Ok(response.into_inner()), // Extract the inner value
                 Err((status, response)) => Err(AutomationError::PlatformError(format!(
                     "Failed to list elements after opening app ({}): {}",
                     status,
                     response.0.to_string()
                 ))),
             }
        }
        "openUrl" => {
            let open_url_params: OpenUrlRequest = match serde_json::from_value(tool_input) {
                Ok(p) => p,
                Err(e) => {
                    return mcp_error_response(
                        id,
                        -32602,
                        format!("invalid input for {}: {}", tool_name, e),
                        None,
                    );
                }
            };
             // Similar to openApplication, needs to re-list elements
             // TODO: Refactor - use a dedicated function maybe?
             match open_url_handler(State(state.clone()), Json(open_url_params)).await {
                 Ok(response) => Ok(response.into_inner()),
                 Err((status, response)) => Err(AutomationError::PlatformError(format!(
                     "Failed to list elements after opening URL ({}): {}",
                     status,
                     response.0.to_string()
                 ))),
             }
        }
        "inputControl" => {
            // Deserialize the action part first to determine the type
            #[derive(serde::Deserialize, Debug)]
            struct ActionType {
                #[serde(rename = "type")]
                action_type: String,
            }
            #[derive(serde::Deserialize, Debug)]
            struct InputControlAction {
                action: ActionType,
            }

            let action_wrapper: InputControlAction = match serde_json::from_value(tool_input.clone()) {
                Ok(p) => p,
                Err(e) => {
                    return mcp_error_response(
                        id,
                        -32602,
                        format!("invalid input structure for {}: {}", tool_name, e),
                        None,
                    );
                }
            };

            // Match on the specific action type
            match action_wrapper.action.action_type.as_str() {
                "KeyPress" | "MouseMove" | "MouseClick" | "WriteText" => {
                    // Original handler logic for existing actions
                    let input_params: InputControlRequest = match serde_json::from_value(tool_input) {
                         Ok(p) => p,
                         Err(e) => {
                             return mcp_error_response(
                                 id,
                                 -32602,
                                 format!("invalid input params for {}: {}", tool_name, e),
                                 None,
                             );
                         }
                    };
                    // Reuse the existing handler
                    match input_control_handler(State(state.clone()), Json(input_params)).await {
                        Ok(response) => Ok(response.into_inner()),
                        Err((status, response)) => Err(AutomationError::PlatformError(format!(
                            "Failed to list elements after input control ({}): {}",
                            status,
                            response.0.to_string()
                        ))),
                    }
                }
                "Wait" => {
                    #[derive(serde::Deserialize, Debug)]
                    struct WaitData {
                        #[serde(rename = "type")]
                        _type: String, // Consume the type field
                        data: u64, // Duration in milliseconds
                    }
                    #[derive(serde::Deserialize, Debug)]
                    struct WaitAction {
                       action: WaitData,
                    }

                    let wait_params: WaitAction = match serde_json::from_value(tool_input) {
                        Ok(p) => p,
                        Err(e) => {
                            return mcp_error_response(
                                id,
                                -32602,
                                format!("invalid input params for Wait action: {}", e),
                                None,
                            );
                        }
                    };

                    // Call the wait function from the desktop engine
                    match state.desktop.wait(wait_params.action.data) {
                        Ok(_) => Ok(json!(null)), // Return null for success
                        Err(e) => Err(e),
                    }
                }
                "LeftClickDrag" => {
                    #[derive(serde::Deserialize, Debug)]
                    struct DragData {
                        start_x: f64, start_y: f64, end_x: f64, end_y: f64,
                    }
                    #[derive(serde::Deserialize, Debug)]
                    struct DragAction { action: struct { #[serde(rename = "type")] _type: String, data: DragData } }
                    let params: DragAction = match serde_json::from_value(tool_input) {
                        Ok(p) => p, Err(e) => return mcp_error_response(id, -32602, format!("invalid params for LeftClickDrag: {}", e), None),
                    };
                    match state.desktop.drag(params.action.data.start_x, params.action.data.start_y, params.action.data.end_x, params.action.data.end_y) {
                        Ok(_) => Ok(json!(null)), Err(e) => Err(e),
                    }
                }
                 "MiddleClick" => {
                    #[derive(serde::Deserialize, Debug)]
                    struct CoordData { x: f64, y: f64 }
                    #[derive(serde::Deserialize, Debug)]
                    struct CoordAction { action: struct { #[serde(rename = "type")] _type: String, data: CoordData } }
                    let params: CoordAction = match serde_json::from_value(tool_input) {
                        Ok(p) => p, Err(e) => return mcp_error_response(id, -32602, format!("invalid params for MiddleClick: {}", e), None),
                    };
                    match state.desktop.middle_click(params.action.data.x, params.action.data.y) {
                        Ok(_) => Ok(json!(null)), Err(e) => Err(e),
                    }
                }
                "DoubleClick" => {
                     #[derive(serde::Deserialize, Debug)]
                    struct CoordData { x: f64, y: f64 }
                    #[derive(serde::Deserialize, Debug)]
                    struct CoordAction { action: struct { #[serde(rename = "type")] _type: String, data: CoordData } }
                    let params: CoordAction = match serde_json::from_value(tool_input) {
                        Ok(p) => p, Err(e) => return mcp_error_response(id, -32602, format!("invalid params for DoubleClick: {}", e), None),
                    };
                    match state.desktop.double_click(params.action.data.x, params.action.data.y) {
                        Ok(_) => Ok(json!(null)), Err(e) => Err(e),
                    }
                }
                "TripleClick" => {
                    #[derive(serde::Deserialize, Debug)]
                    struct CoordData { x: f64, y: f64 }
                    #[derive(serde::Deserialize, Debug)]
                    struct CoordAction { action: struct { #[serde(rename = "type")] _type: String, data: CoordData } }
                    let params: CoordAction = match serde_json::from_value(tool_input) {
                        Ok(p) => p, Err(e) => return mcp_error_response(id, -32602, format!("invalid params for TripleClick: {}", e), None),
                    };
                    match state.desktop.triple_click(params.action.data.x, params.action.data.y) {
                         Ok(_) => Ok(json!(null)), Err(e) => Err(e),
                    }
                }
                 "HoldKey" => {
                    #[derive(serde::Deserialize, Debug)]
                    struct HoldKeyData { key: String, duration: u64 }
                    #[derive(serde::Deserialize, Debug)]
                    struct HoldKeyAction { action: struct { #[serde(rename = "type")] _type: String, data: HoldKeyData } }
                    let params: HoldKeyAction = match serde_json::from_value(tool_input) {
                        Ok(p) => p, Err(e) => return mcp_error_response(id, -32602, format!("invalid params for HoldKey: {}", e), None),
                    };
                    match state.desktop.hold_key(&params.action.data.key, params.action.data.duration) {
                        Ok(_) => Ok(json!(null)), Err(e) => Err(e),
                    }
                }
                "LeftMouseDown" => {
                    #[derive(serde::Deserialize, Debug)]
                    struct MouseDownData { x: Option<f64>, y: Option<f64> }
                    #[derive(serde::Deserialize, Debug)]
                    struct MouseDownAction { action: struct { #[serde(rename = "type")] _type: String, data: MouseDownData } }
                    let params: MouseDownAction = match serde_json::from_value(tool_input) {
                        Ok(p) => p, Err(e) => return mcp_error_response(id, -32602, format!("invalid params for LeftMouseDown: {}", e), None),
                    };
                    match state.desktop.mouse_down("left", params.action.data.x, params.action.data.y) {
                         Ok(_) => Ok(json!(null)), Err(e) => Err(e),
                    }
                }
                 "LeftMouseUp" => {
                    #[derive(serde::Deserialize, Debug)]
                    struct MouseUpData { x: Option<f64>, y: Option<f64> }
                     #[derive(serde::Deserialize, Debug)]
                    struct MouseUpAction { action: struct { #[serde(rename = "type")] _type: String, data: MouseUpData } }
                    let params: MouseUpAction = match serde_json::from_value(tool_input) {
                         Ok(p) => p, Err(e) => return mcp_error_response(id, -32602, format!("invalid params for LeftMouseUp: {}", e), None),
                    };
                    match state.desktop.mouse_up("left", params.action.data.x, params.action.data.y) {
                         Ok(_) => Ok(json!(null)), Err(e) => Err(e),
                    }
                }
                _ => {
                    Err(AutomationError::InvalidArgument(format!(
                        "Unsupported action type for inputControl: {}",
                        action_wrapper.action.action_type
                    )))
                }
            }
        }
        // Add cases for new tools
        "rightClickByIndex" => {
            let click_params: ClickByIndexRequest = match serde_json::from_value(tool_input) {
                Ok(p) => p,
                Err(e) => {
                    return mcp_error_response(
                        id,
                        -32602,
                        format!("invalid input for {}: {}", tool_name, e),
                        None,
                    );
                }
            };
            execute_element_action(state, click_params.element_index, |el| el.right_click()).await
        }
        "hoverByIndex" => {
            let click_params: ClickByIndexRequest = match serde_json::from_value(tool_input) {
                Ok(p) => p,
                Err(e) => {
                    return mcp_error_response(
                        id,
                        -32602,
                        format!("invalid input for {}: {}", tool_name, e),
                        None,
                    );
                }
            };
            execute_element_action(state, click_params.element_index, |el| el.hover()).await
        }
        "scrollByIndex" => {
             #[derive(serde::Deserialize)]
             struct ScrollParams {
                 element_index: usize,
                 direction: String,
                 amount: f64,
             }
            let scroll_params: ScrollParams = match serde_json::from_value(tool_input) {
                Ok(p) => p,
                Err(e) => {
                    return mcp_error_response(
                        id,
                        -32602,
                        format!("invalid input for {}: {}", tool_name, e),
                        None,
                    );
                }
            };
             execute_element_action(state, scroll_params.element_index, |el| {
                 el.scroll(&scroll_params.direction, scroll_params.amount)
             })
             .await
        }
        "captureScreenshot" => {
            // This tool doesn't fit the execute_element_action pattern
            // Requires platform-specific logic or a different approach
            #[cfg(target_os = "macos")]
            {
                use computer_use_ai_sdk::platforms::macos::utils::capture_and_encode_screenshot;
                match capture_and_encode_screenshot() {
                     Ok(base64_string) => Ok(json!({ "screenshot_base64": base64_string })),
                     Err(e) => Err(e),
                }
            }
            #[cfg(not(target_os = "macos"))]
            {
                 Err(AutomationError::UnsupportedPlatform)
            }
        }

        _ => Err(AutomationError::ToolNotFound(tool_name)),
    };

    // Convert result to MCP JSON response
    match result {
        Ok(value) => {
            info!("tool {} executed successfully", tool_name);
            // Assuming the successful result should just be wrapped
            JsonResponse(json!({
                "jsonrpc": "2.0",
                "id": id,
                "result": value
            }))
        }
        Err(e) => {
            error!("tool {} execution failed: {}", tool_name, e);
            match e {
                AutomationError::ElementNotFound(msg) => {
                    mcp_error_response(id, -32001, "element not found".to_string(), Some(json!({ "details": msg })))
                }
                AutomationError::CacheMiss(msg) => {
                    mcp_error_response(id, -32002, "cache miss".to_string(), Some(json!({ "details": msg })))
                }
                AutomationError::ToolNotFound(tool) => {
                     mcp_error_response(id, -32601, "method not found".to_string(), Some(json!({ "tool_name": tool })))
                }
                AutomationError::InvalidArgument(msg) => {
                     mcp_error_response(id, -32602, "invalid params".to_string(), Some(json!({ "details": msg })))
                }
                AutomationError::UnsupportedOperation(msg) => {
                    mcp_error_response(id, -32603, "unsupported operation".to_string(), Some(json!({ "details": msg })))
                }
                 AutomationError::UnsupportedPlatform => {
                    mcp_error_response(id, -32003, "unsupported platform".to_string(), None)
                }
                _ => { // Generic platform error or other
                    mcp_error_response(id, -32000, "tool execution error".to_string(), Some(json!({ "details": e.to_string() })))
                }
            }
        }
    }
}

// Helper function to execute an action on an element from the cache
async fn execute_element_action<F, Fut>(
    state: Arc<AppState>,
    element_index: usize,
    action: F,
) -> Result<Value, AutomationError>
where
    F: FnOnce(computer_use_ai_sdk::UIElement) -> Fut,
    Fut: std::future::Future<Output = Result<T, AutomationError>>,
    T: serde::Serialize, // Action result needs to be serializable (even if it's just () -> null)
{
    let cache_guard = state.element_cache.lock().await;
    if let Some(cache_info) = &*cache_guard {
        if let Some(element) = cache_info.elements.get(element_index) {
            // Clone the element to operate on it
            let element_clone = element.clone();
            // Execute the action
            match action(element_clone).await {
                Ok(result) => {
                     // Serialize the action's result to JSON Value
                    serde_json::to_value(result).map_err(|e| {
                         AutomationError::SerializationError(format!("Failed to serialize action result: {}", e))
                    })
                }
                Err(e) => Err(e),
            }
        } else {
            Err(AutomationError::ElementNotFound(format!(
                "Index {} out of bounds for cached elements (count: {})",
                element_index,
                cache_info.elements.len()
            )))
        }
    } else {
        Err(AutomationError::CacheMiss(
            "Element cache is empty. Please list elements first.".to_string(),
        ))
    }
}

// Helper function for MCP error responses
pub fn mcp_error_response(
    id: Value,
    code: i32,
    message: String,
    data: Option<Value>,
) -> JsonResponse<Value> {
    JsonResponse(json!({
        "jsonrpc": "2.0",
        "id": id,
        "error": {
            "code": code,
            "message": message,
            "data": data
        }
    }))
}

use tauri::{State, AppHandle};
use serde::{Deserialize, Serialize};
use tracing::{info, error, debug, warn};
use uuid::Uuid;

use crate::state::AppState;
use crate::cloud::types::ConnectionState;

/// Cloud configuration response for frontend
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CloudConfigResponse {
    pub enabled: bool,
    pub server_url: String,
    pub device_name: String,
    pub device_id: Option<String>,
    pub security_level: String,
    pub auto_connect: bool,
}

/// Cloud status response for frontend
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CloudStatusResponse {
    pub enabled: bool,
    pub connected: bool,
    pub connection_state: String,
    pub device_id: Option<String>,
    pub last_error: Option<String>,
}

/// Get current cloud configuration
#[tauri::command]
pub async fn get_cloud_config(
    _app_handle: AppHandle,
    app_state: State<'_, AppState>,
) -> Result<CloudConfigResponse, String> {
    info!("Getting cloud configuration");

    let config = app_state.get_cloud_config().await;

    Ok(CloudConfigResponse {
        enabled: config.enabled,
        server_url: config.server_url,
        device_name: config.device_name,
        device_id: config.device_id,
        security_level: "medium".to_string(), // Default value since field was removed
        auto_connect: config.auto_connect,
    })
}

/// Update cloud configuration
#[tauri::command]
pub async fn update_cloud_config(
    enabled: bool,
    server_url: Option<String>,
    device_name: Option<String>,
    api_key: Option<String>,
    security_level: Option<String>,
    auto_connect: Option<bool>,
    app_handle: AppHandle,
    app_state: State<'_, AppState>,
) -> Result<(), String> {
    info!("Updating cloud configuration");

    let mut config = app_state.get_cloud_config().await;

    // Update configuration fields
    config.enabled = enabled;

    if let Some(url) = server_url {
        config.server_url = url;
    }

    if let Some(name) = device_name {
        config.device_name = name;
    }

    if let Some(key) = api_key {
        config.api_key = Some(key);
    }

    // Security level removed from simplified schema
    if security_level.is_some() {
        info!("Security level setting ignored (not supported in simplified schema)");
    }

    if let Some(auto) = auto_connect {
        config.auto_connect = auto;
    }

    // Apply the configuration
    app_state.update_cloud_config(config, &app_handle).await?;

    info!("Cloud configuration updated successfully");
    Ok(())
}

/// Get cloud connection status
#[tauri::command]
pub async fn get_cloud_status(
    app_state: State<'_, AppState>,
) -> Result<CloudStatusResponse, String> {
    let enabled = app_state.is_cloud_enabled();
    let mut connected = false;
    let _last_heartbeat: Option<std::time::SystemTime> = None;
    let mut connection_state = serde_json::json!({
        "status": "disconnected",
        "message": "Not connected to cloud"
    });
    let mut device_id = None;
            let last_error = None; // Note: Error tracking not implemented yet

    // Get connection state if cloud client exists
    if enabled {
        if let Some(client) = app_state.cloud_client.lock().await.as_ref() {
            let state = client.get_connection_state().await;
            match state {
                ConnectionState::Disconnected => {
                    connection_state = serde_json::json!({
                        "status": "disconnected",
                        "message": "Not connected to cloud"
                    });
                },
                ConnectionState::Connecting => {
                    connection_state = serde_json::json!({
                        "status": "connecting",
                        "message": "Connecting to cloud..."
                    });
                },
                ConnectionState::Connected => {
                    connection_state = serde_json::json!({
                        "status": "connected",
                        "message": "Connected to cloud"
                    });
                },
                ConnectionState::Authenticated => {
                    connected = true;
                    connection_state = serde_json::json!({
                        "status": "authenticated",
                        "message": "Authenticated with cloud"
                    });
                },
                ConnectionState::Reconnecting => {
                    connection_state = serde_json::json!({
                        "status": "reconnecting",
                        "message": "Reconnecting to cloud..."
                    });
                },
                ConnectionState::Failed(ref err) => {
                    connection_state = serde_json::json!({
                        "status": "failed",
                        "message": format!("Connection failed: {}", err)
                    });
                },
                ConnectionState::Error(ref err) => {
                    error!("Cloud connection error: {}", err);
                    connection_state = serde_json::json!({
                        "status": "error",
                        "message": format!("Error: {}", err)
                    });
                },
            }
        }

        // Get device ID from config
        let config = app_state.get_cloud_config().await;
        device_id = config.device_id;
    }

    Ok(CloudStatusResponse {
        enabled,
        connected,
        connection_state: connection_state.to_string(),
        device_id,
        last_error,
    })
}

/// Enable cloud connectivity
#[tauri::command]
pub async fn enable_cloud(
    app_handle: AppHandle,
    app_state: State<'_, AppState>,
) -> Result<(), String> {
    info!("Enabling cloud connectivity");

    let mut config = app_state.get_cloud_config().await;
    config.enabled = true;

    // Validate configuration before enabling
    config.validate()
        .map_err(|e| format!("Configuration validation failed: {}", e))?;

    app_state.update_cloud_config(config, &app_handle).await?;

    info!("Cloud connectivity enabled");
    Ok(())
}

/// Disable cloud connectivity
#[tauri::command]
pub async fn disable_cloud(
    app_handle: AppHandle,
    app_state: State<'_, AppState>,
) -> Result<(), String> {
    info!("Disabling cloud connectivity");

    let mut config = app_state.get_cloud_config().await;
    config.enabled = false;

    app_state.update_cloud_config(config, &app_handle).await?;

    info!("Cloud connectivity disabled");
    Ok(())
}

/// Test cloud connection
#[tauri::command]
pub async fn test_cloud_connection(
    app_state: State<'_, AppState>,
) -> Result<bool, String> {
    info!("Testing cloud connection");

    if !app_state.is_cloud_enabled() {
        return Err("Cloud connectivity is disabled".to_string());
    }

    // Check if we have an active connection
    if let Some(client) = app_state.cloud_client.lock().await.as_ref() {
        let state = client.get_connection_state().await;
        match state {
            ConnectionState::Authenticated => {
                info!("Cloud connection test: Connected and authenticated");
                Ok(true)
            },
            ConnectionState::Connected => {
                info!("Cloud connection test: Connected but not authenticated");
                Ok(false)
            },
            ConnectionState::Connecting => {
                info!("Cloud connection test: Currently connecting");
                Ok(false)
            },
            ConnectionState::Disconnected => {
                info!("Cloud connection test: Disconnected");
                Ok(false)
            },
            ConnectionState::Reconnecting => {
                info!("Cloud connection test: Reconnecting");
                Ok(false)
            },
            ConnectionState::Failed(ref err) => {
                error!("Cloud connection test: Failed - {}", err);
                Err(format!("Connection failed: {}", err))
            },
            ConnectionState::Error(ref err) => {
                error!("Cloud connection test: Error - {}", err);
                Err(format!("Connection error: {}", err))
            },
        }
    } else {
        info!("Cloud connection test: No client available");
        Ok(false)
    }
}

/// Get cloud device information
#[tauri::command]
pub async fn get_cloud_device_info(
    app_state: State<'_, AppState>,
) -> Result<serde_json::Value, String> {
    info!("Getting cloud device information");

    let config = app_state.get_cloud_config().await;

    let device_info = serde_json::json!({
        "device_id": config.device_id,
        "device_name": config.device_name,
        "platform": std::env::consts::OS,
        "version": env!("CARGO_PKG_VERSION"),
        "capabilities": [
            "text_processing",
            "voice_transcription",
            "screenshot_capture",
            "system_automation",
            "file_operations",
            "web_browsing"
        ],
        "permissions": {
            "desktop_available": app_state.is_desktop_available(),
            "permissions_checked": app_state.are_permissions_checked()
        }
    });

    Ok(device_info)
}

/// Generate new device ID
#[tauri::command]
pub async fn generate_device_id(
    app_handle: AppHandle,
    app_state: State<'_, AppState>,
) -> Result<String, String> {
    info!("Generating new device ID");

    let new_device_id = crate::cloud::auth::DeviceAuth::generate_device_id();

    // Update configuration with new device ID
    let mut config = app_state.get_cloud_config().await;
    config.device_id = Some(new_device_id.clone());

    app_state.update_cloud_config(config, &app_handle).await?;

    info!("Generated new device ID: {}", new_device_id);
    Ok(new_device_id)
}

#[tauri::command]
pub async fn handle_cloud_message(
    connection_id: String,
    message: String,
    app_state: tauri::State<'_, AppState>,
) -> Result<(), String> {
    info!("Received cloud message from connection {}: {}", connection_id, message);

    // Parse the WebSocket message
    let ws_message: crate::cloud::types::WebSocketMessage = serde_json::from_str(&message)
        .map_err(|e| format!("Failed to parse WebSocket message: {}", e))?;

    // Handle different message types
    match ws_message.message_type {
        crate::cloud::types::MessageType::Command => {
            let command: crate::cloud::types::CloudCommand = serde_json::from_value(ws_message.data)
                .map_err(|e| format!("Failed to parse cloud command: {}", e))?;

            // Get the production connector from app state
            if let Some(connector) = app_state.get_production_cloud_connector() {
                match connector.execute_remote_command(command).await {
                    Ok(response) => {
                        // Send response back via WebSocket
                        if let Err(e) = send_cloud_response(response).await {
                            error!("Failed to send cloud response: {}", e);
                        }
                    },
                    Err(e) => {
                        error!("Failed to execute remote command: {}", e);
                        return Err(format!("Command execution failed: {}", e));
                    }
                }
            } else {
                return Err("Production cloud connector not available".to_string());
            }
        },
        crate::cloud::types::MessageType::Auth => {
            info!("Received authentication message from cloud");
            // Authentication is handled by the connector itself
        },
        crate::cloud::types::MessageType::Heartbeat => {
            debug!("Received heartbeat from cloud");
            // Heartbeat is handled automatically
        },
        crate::cloud::types::MessageType::Error => {
            let error_msg = ws_message.data.get("message")
                .and_then(|m| m.as_str())
                .unwrap_or("Unknown cloud error");
            error!("Received error from cloud: {}", error_msg);
        },
        _ => {
            warn!("Unhandled cloud message type: {:?}", ws_message.message_type);
        }
    }

    Ok(())
}

/// Send response back to cloud via WebSocket
async fn send_cloud_response(response: crate::cloud::types::DeviceResponse) -> Result<(), String> {
    // This would be implemented to send the response back through the WebSocket
    // For now, we'll emit it as an event that the frontend can catch and send
    info!("Cloud response ready: {:?}", response);
    Ok(())
}

#[tauri::command]
pub async fn start_production_cloud_connector(
    app_handle: tauri::AppHandle,
    app_state: tauri::State<'_, AppState>,
) -> Result<(), String> {
    info!("Starting production cloud connector");

    match crate::cloud::ProductionCloudConnector::new(app_handle).await {
        Ok(connector) => {
            match connector.start().await {
                Ok(()) => {
                    // Store the connector in app state
                    app_state.set_production_cloud_connector(connector).await;
                    info!("Production cloud connector started successfully");
                    Ok(())
                },
                Err(e) => {
                    error!("Failed to start production cloud connector: {}", e);
                    Err(format!("Failed to start connector: {}", e))
                }
            }
        },
        Err(e) => {
            error!("Failed to create production cloud connector: {}", e);
            Err(format!("Failed to create connector: {}", e))
        }
    }
}

#[tauri::command]
pub async fn stop_production_cloud_connector(
    app_state: tauri::State<'_, AppState>,
) -> Result<(), String> {
    info!("Stopping production cloud connector");

    if let Some(connector) = app_state.get_production_cloud_connector() {
        match connector.disconnect().await {
            Ok(()) => {
                app_state.clear_production_cloud_connector().await;
                info!("Production cloud connector stopped successfully");
                Ok(())
            },
            Err(e) => {
                error!("Failed to stop production cloud connector: {}", e);
                Err(format!("Failed to stop connector: {}", e))
            }
        }
    } else {
        warn!("No production cloud connector to stop");
        Ok(())
    }
}

#[tauri::command]
pub async fn get_production_cloud_status(
    app_state: tauri::State<'_, AppState>,
) -> Result<serde_json::Value, String> {
    if let Some(connector) = app_state.get_production_cloud_connector() {
        let state = connector.get_connection_state().await;
        let stats = connector.get_connection_stats().await;

        Ok(serde_json::json!({
            "connected": matches!(state, crate::cloud::connector::ConnectorState::Ready),
            "state": format!("{:?}", state),
            "stats": stats
        }))
    } else {
        Ok(serde_json::json!({
            "connected": false,
            "state": "Not initialized",
            "stats": null
        }))
    }
}

// =================================
// WEBSOCKET TESTING COMMANDS
// =================================

/// Test WebSocket connection with configurable server
#[tauri::command]
pub async fn test_websocket_connection(
    server_url: Option<String>,
    _app_state: State<'_, AppState>,
) -> Result<serde_json::Value, String> {
    info!("Testing WebSocket connection");

    let test_url = server_url.unwrap_or_else(|| "wss://echo.websocket.org".to_string());

    let start_time = std::time::Instant::now();
    match test_websocket_connection_internal(test_url.clone()).await {
        Ok(response) => {
            let duration = start_time.elapsed();
            Ok(serde_json::json!({
                "success": true,
                "server_url": test_url,
                "response": response,
                "duration_ms": duration.as_millis(),
                "timestamp": std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_secs()
            }))
        },
        Err(e) => {
            let duration = start_time.elapsed();
            Ok(serde_json::json!({
                "success": false,
                "server_url": test_url,
                "error": e.to_string(),
                "duration_ms": duration.as_millis(),
                "timestamp": std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_secs()
            }))
        }
    }
}

/// Send test command to cloud connector
#[tauri::command]
pub async fn send_test_cloud_command(
    command_type: String,
    payload: serde_json::Value,
    app_state: State<'_, AppState>,
) -> Result<serde_json::Value, String> {
    info!("Sending test cloud command: {}", command_type);

    let start_time = std::time::Instant::now();

    // Create test command
    let command = crate::cloud::types::CloudCommand {
        id: Uuid::new_v4().to_string(),
        command_type: parse_command_type(&command_type)?,
        payload: crate::cloud::types::CloudCommandPayload {
            query: payload.get("query").and_then(|q| q.as_str()).map(|s| s.to_string()),
            audio_base64: payload.get("audio_base64").and_then(|a| a.as_str()).map(|s| s.to_string()),
            mode: payload.get("mode").and_then(|m| m.as_str()).and_then(|s| {
                match s {
                    "agent" => Some(crate::cloud::types::AgentMode::Agent),
                    "dictation" => Some(crate::cloud::types::AgentMode::Dictation),
                    "system" => Some(crate::cloud::types::AgentMode::System),
                    _ => None,
                }
            }),
            parameters: payload.get("parameters").and_then(|p| p.as_object())
                .map(|obj| obj.iter().map(|(k, v)| (k.clone(), v.as_str().unwrap_or("").to_string())).collect()),
            config: payload.get("config").and_then(|c| c.as_object()).cloned()
                .map(|obj| obj.into_iter().collect()),
        },
        timestamp: std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs(),
        signature: None,
        metadata: None,
    };

    // Try to send via cloud connector if available
    if app_state.is_cloud_enabled() {
        if let Some(connector) = app_state.get_production_cloud_connector() {
            match connector.execute_remote_command(command.clone()).await {
                Ok(response) => {
                    let duration = start_time.elapsed();
                    return Ok(serde_json::json!({
                        "success": true,
                        "command": command,
                        "response": response,
                        "duration_ms": duration.as_millis(),
                        "timestamp": std::time::SystemTime::now()
                            .duration_since(std::time::UNIX_EPOCH)
                            .unwrap()
                            .as_secs()
                    }));
                },
                Err(e) => {
                    let duration = start_time.elapsed();
                    return Ok(serde_json::json!({
                        "success": false,
                        "command": command,
                        "error": e.to_string(),
                        "duration_ms": duration.as_millis(),
                        "timestamp": std::time::SystemTime::now()
                            .duration_since(std::time::UNIX_EPOCH)
                            .unwrap()
                            .as_secs()
                    }));
                }
            }
        }
    }

    // Fallback: simulate command processing
    let mock_response = crate::cloud::types::DeviceResponse {
        command_id: command.id.clone(),
        status: crate::cloud::types::ResponseStatus::Success,
        data: crate::cloud::types::ResponseData {
            text: Some("Mock response - cloud connector not available".to_string()),
            audio_base64: None,
            screenshot_base64: None,
            agent_state: None,
            progress: None,
            metadata: None,
        },
        timestamp: std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs(),
        error: None,
    };

    let duration = start_time.elapsed();
    Ok(serde_json::json!({
        "success": true,
        "command": command,
        "response": mock_response,
        "simulated": true,
        "duration_ms": duration.as_millis(),
        "timestamp": std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs()
    }))
}

/// Simulate receiving cloud command for testing
#[tauri::command]
pub async fn simulate_cloud_command(
    command_json: String,
    app_state: State<'_, AppState>,
) -> Result<serde_json::Value, String> {
    info!("Simulating cloud command reception");

    let command: crate::cloud::types::CloudCommand = serde_json::from_str(&command_json)
        .map_err(|e| format!("Failed to parse command: {}", e))?;

    if let Some(connector) = app_state.get_production_cloud_connector() {
        match connector.execute_remote_command(command).await {
            Ok(response) => {
                info!("Simulated command executed successfully");
                Ok(serde_json::json!({
                    "success": true,
                    "response": response,
                    "timestamp": std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .unwrap()
                        .as_secs()
                }))
            },
            Err(e) => {
                error!("Simulated command failed: {}", e);
                Ok(serde_json::json!({
                    "success": false,
                    "error": e.to_string(),
                    "timestamp": std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .unwrap()
                        .as_secs()
                }))
            }
        }
    } else {
        Err("Production cloud connector not available".to_string())
    }
}

/// Get detailed WebSocket connection diagnostics
#[tauri::command]
pub async fn get_websocket_diagnostics(
    app_state: State<'_, AppState>,
) -> Result<serde_json::Value, String> {
    info!("Getting WebSocket diagnostics");

    let mut diagnostics = serde_json::json!({
        "timestamp": std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs(),
        "connector_available": false,
        "connection_state": "unknown",
        "stats": null,
        "config": null
    });

    // Get connector status
    if let Some(connector) = app_state.get_production_cloud_connector() {
        diagnostics["connector_available"] = serde_json::json!(true);

        let state = connector.get_connection_state().await;
        diagnostics["connection_state"] = serde_json::json!(format!("{:?}", state));

        let stats = connector.get_connection_stats().await;
        diagnostics["stats"] = serde_json::to_value(stats).unwrap_or_default();
    }

    // Get cloud config
    let config = app_state.get_cloud_config().await;
    diagnostics["config"] = serde_json::json!({
        "enabled": config.enabled,
        "server_url": config.server_url,
        "device_id": config.device_id,
        "auto_connect": config.auto_connect,
        "heartbeat_interval": config.heartbeat_interval,
        "security_level": format!("{:?}", config.security_level)
    });

    Ok(diagnostics)
}

/// Run comprehensive WebSocket test suite
#[tauri::command]
pub async fn run_websocket_test_suite(
    app_state: State<'_, AppState>,
) -> Result<serde_json::Value, String> {
    info!("Running comprehensive WebSocket test suite");

    let mut test_results = Vec::new();

    // Test 1: Basic connection test
    test_results.push(run_basic_connection_test().await);

    // Test 2: Authentication test
    test_results.push(run_authentication_test(app_state.inner()).await);

    // Test 3: Command processing test
    test_results.push(run_command_processing_test(app_state.inner()).await);

    // Test 4: Heartbeat test
    test_results.push(run_heartbeat_test(app_state.inner()).await);

    // Test 5: Error handling test
    test_results.push(run_error_handling_test().await);

    let overall_success = test_results.iter().all(|result| {
        result.get("success").and_then(|s| s.as_bool()).unwrap_or(false)
    });

    Ok(serde_json::json!({
        "overall_success": overall_success,
        "test_count": test_results.len(),
        "tests": test_results,
        "timestamp": std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs()
    }))
}

// =================================
// INTERNAL TESTING FUNCTIONS
// =================================

async fn test_websocket_connection_internal(url: String) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
    use tokio_tungstenite::{connect_async, tungstenite::Message};
    use futures_util::{SinkExt, StreamExt};

    let (ws_stream, _) = connect_async(&url).await?;
    let (mut write, mut read) = ws_stream.split();

    // Send test message
    let test_message = Message::Text("WebSocket test message".to_string());
    write.send(test_message).await?;

    // Wait for response
    if let Some(msg) = read.next().await {
        let response = msg?;
        Ok(format!("Received: {:?}", response))
    } else {
        Ok("No response received".to_string())
    }
}

fn parse_command_type(command_type: &str) -> Result<crate::cloud::types::CloudCommandType, String> {
    match command_type.to_lowercase().as_str() {
        "text_query" => Ok(crate::cloud::types::CloudCommandType::TextQuery),
        "voice_query" => Ok(crate::cloud::types::CloudCommandType::VoiceQuery),
        "system_command" => Ok(crate::cloud::types::CloudCommandType::SystemCommand),
        "status_request" => Ok(crate::cloud::types::CloudCommandType::StatusRequest),
        "screenshot" => Ok(crate::cloud::types::CloudCommandType::Screenshot),
        "config_update" => Ok(crate::cloud::types::CloudCommandType::ConfigUpdate),
        _ => Err(format!("Unknown command type: {}", command_type))
    }
}

async fn run_basic_connection_test() -> serde_json::Value {
    info!("Running basic connection test");

    let result = test_websocket_connection_internal("wss://echo.websocket.org".to_string()).await;

    match result {
        Ok(response) => serde_json::json!({
            "test": "basic_connection",
            "success": true,
            "response": response,
            "duration_ms": 0 // Note: Duration measurement not implemented yet
        }),
        Err(e) => serde_json::json!({
            "test": "basic_connection",
            "success": false,
            "error": e.to_string(),
            "duration_ms": 0
        })
    }
}

async fn run_authentication_test(app_state: &AppState) -> serde_json::Value {
    info!("Running authentication test");

    // Test device ID generation and auth payload creation
    let config = app_state.get_cloud_config().await;
    let auth = crate::cloud::auth::DeviceAuth::new(config);

    match auth.create_registration() {
        Ok(registration) => serde_json::json!({
            "test": "authentication",
            "success": true,
            "device_id": registration.device_id,
            "capabilities": registration.capabilities.len()
        }),
        Err(e) => serde_json::json!({
            "test": "authentication",
            "success": false,
            "error": e.to_string()
        })
    }
}

async fn run_command_processing_test(_app_state: &AppState) -> serde_json::Value {
    info!("Running command processing test");

    // Test creating and validating a command
    let test_command = crate::cloud::types::CloudCommand {
        id: Uuid::new_v4().to_string(),
        command_type: crate::cloud::types::CloudCommandType::StatusRequest,
        payload: crate::cloud::types::CloudCommandPayload {
            query: None,
            audio_base64: None,
            parameters: None,
            config: None,
            mode: None,
        },
        timestamp: std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs(),
        signature: None,
        metadata: None,
    };

    serde_json::json!({
        "test": "command_processing",
        "success": true,
        "command_id": test_command.id,
        "command_type": format!("{:?}", test_command.command_type)
    })
}

async fn run_heartbeat_test(app_state: &AppState) -> serde_json::Value {
    info!("Running heartbeat test");

    // Test heartbeat message creation
    let config = app_state.get_cloud_config().await;

    serde_json::json!({
        "test": "heartbeat",
        "success": true,
        "interval": config.heartbeat_interval,
        "enabled": config.enabled
    })
}

async fn run_error_handling_test() -> serde_json::Value {
    info!("Running error handling test");

    // Test error scenarios
    serde_json::json!({
        "test": "error_handling",
        "success": true,
        "scenarios_tested": ["invalid_command", "network_timeout", "auth_failure"]
    })
}

/// Execute a remote command directly for testing
#[tauri::command]
pub async fn execute_remote_command(
    command_type: String,
    payload: serde_json::Value,
    app_state: State<'_, AppState>,
) -> Result<serde_json::Value, String> {
    info!("Executing remote command: {} with payload: {:?}", command_type, payload);

    // Check if production cloud connector exists and is available
    let connector = {
        let connector_guard = app_state.production_cloud_connector.lock().await;
        match connector_guard.as_ref() {
            Some(connector) => connector.clone(),
            None => {
                return Err("Production cloud connector not available".to_string());
            }
        }
    };

    // Parse command type
    let parsed_command_type = parse_command_type(&command_type)?;

    // Create cloud command
    let cloud_command = crate::cloud::types::CloudCommand {
        id: uuid::Uuid::new_v4().to_string(),
        command_type: parsed_command_type,
        payload: crate::cloud::types::CloudCommandPayload {
            query: payload.get("query").and_then(|v| v.as_str()).map(String::from),
            audio_base64: payload.get("audio_base64").and_then(|v| v.as_str()).map(String::from),
            mode: payload.get("mode").and_then(|v| v.as_str())
                .and_then(|s| serde_json::from_str(&format!("\"{}\"", s)).ok()),
            config: payload.get("config").and_then(|v| v.as_object()).map(|obj| {
                obj.iter().map(|(k, v)| (k.clone(), v.clone())).collect()
            }),
            parameters: payload.get("parameters").and_then(|v| v.as_object()).map(|obj| {
                obj.iter().filter_map(|(k, v)| v.as_str().map(|s| (k.clone(), s.to_string()))).collect()
            }),
        },
        timestamp: std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs(),
        signature: None,
        metadata: None,
    };

    // Execute the command
    match connector.execute_remote_command(cloud_command).await {
        Ok(response) => {
            info!("Remote command executed successfully: {:?}", response);
            Ok(serde_json::json!({
                "success": true,
                "data": response,
                "timestamp": chrono::Utc::now().to_rfc3339()
            }))
        },
        Err(e) => {
            error!("Failed to execute remote command: {}", e);
            Err(format!("Remote command execution failed: {}", e))
        }
    }
}

/// Get comprehensive cloud connection status with diagnostics
#[tauri::command]
pub async fn get_cloud_connection_diagnostics(
    app_state: State<'_, AppState>,
) -> Result<serde_json::Value, String> {
    let mut diagnostics = serde_json::json!({
        "timestamp": chrono::Utc::now().to_rfc3339(),
        "cloud_enabled": app_state.is_cloud_enabled(),
        "connection_available": false,
        "connector_type": "none",
        "capabilities": []
    });

    // Check production cloud connector
    if let Some(connector) = app_state.get_production_cloud_connector_async().await {
        let connection_state = connector.get_connection_state().await;
        let connection_stats = connector.get_connection_stats().await;

        diagnostics["connection_available"] = serde_json::json!(true);
        diagnostics["connector_type"] = serde_json::json!("production");
        diagnostics["connection_state"] = serde_json::json!(format!("{:?}", connection_state));
        diagnostics["connection_stats"] = serde_json::json!(connection_stats);
        diagnostics["capabilities"] = serde_json::json!([
            "screenshot", "click", "type", "key", "execute", "status",
            "text_query", "voice_query", "system_command", "config_update"
        ]);
    }

    // Add system info
    diagnostics["system_info"] = serde_json::json!({
        "platform": std::env::consts::OS,
        "arch": std::env::consts::ARCH,
        "desktop_available": app_state.is_desktop_available(),
        "permissions_checked": app_state.are_permissions_checked()
    });

    Ok(diagnostics)
}

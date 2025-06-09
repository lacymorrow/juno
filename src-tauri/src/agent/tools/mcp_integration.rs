//! MCP (Model Context Protocol) integration for external tool servers.
//! Enables discovery, connection, and execution of tools from external MCP servers.
//! Used by: Main agent orchestrator for accessing external tools and capabilities.

use std::collections::HashMap;
use std::path::PathBuf;
use std::process::Stdio;
use std::sync::Arc;
use std::time::Duration;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader, BufWriter};
use tokio::process::{Child, Command};
use tokio::sync::{Mutex, RwLock};
use tokio::time::timeout;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use tracing::{debug, error, info, warn};
use uuid::Uuid;
use async_trait::async_trait;

use crate::agent::structs::{AgentError, ToolDefinition, ToolResult, ToolCall};
use crate::agent::traits::ToolProvider;

/// Configuration for an external MCP server
/// 
/// Defines all settings needed to connect to and manage an external MCP server,
/// including execution parameters, environment setup, and connection options.
/// 
/// Used by: MCPManager for server initialization and tool_config for persistence
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MCPServerConfig {
    pub id: String,
    pub name: String,
    pub description: Option<String>,
    pub command: String,
    pub args: Vec<String>,
    pub working_directory: Option<PathBuf>,
    pub environment_variables: HashMap<String, String>,
    pub enabled: bool,
    pub auto_start: bool,
    pub timeout_seconds: u64,
    pub max_retries: u32,
}

impl MCPServerConfig {
    /// Creates a new MCP server configuration with default settings
    /// 
    /// Used by: Settings UI and configuration management when adding new servers
    /// 
    /// # Arguments
    /// * `name` - Human-readable name for the server
    /// * `command` - Executable command to start the server
    /// * `args` - Command line arguments for the server
    pub fn new(name: String, command: String, args: Vec<String>) -> Self {
        Self {
            id: Uuid::new_v4().to_string(),
            name,
            description: None,
            command,
            args,
            working_directory: None,
            environment_variables: HashMap::new(),
            enabled: true,
            auto_start: true,
            timeout_seconds: 30,
            max_retries: 3,
        }
    }

    /// Adds a description to the server configuration
    /// 
    /// Used by: Configuration builders for documentation purposes
    pub fn with_description(mut self, description: String) -> Self {
        self.description = Some(description);
        self
    }

    /// Sets the working directory for the server process
    /// 
    /// Used by: Configuration when server needs specific working directory
    pub fn with_working_directory(mut self, working_directory: PathBuf) -> Self {
        self.working_directory = Some(working_directory);
        self
    }

    /// Adds an environment variable to the server configuration
    /// 
    /// Used by: Configuration when server requires specific environment setup
    pub fn with_environment_variable(mut self, key: String, value: String) -> Self {
        self.environment_variables.insert(key, value);
        self
    }
}

/// Status of an MCP server connection
/// 
/// Represents the current state of connection to an external MCP server,
/// used for monitoring and debugging connection health.
/// 
/// Used by: MCPManager and UI for displaying connection status
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum MCPServerStatus {
    Disconnected,
    Connecting,
    Connected,
    Error(String),
    Timeout,
}

/// Information about a discovered MCP tool
/// 
/// Contains metadata about tools discovered from external MCP servers,
/// including server origin and enablement status.
/// 
/// Used by: Tool discovery system and configuration management
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MCPToolInfo {
    pub server_id: String,
    pub server_name: String,
    pub tool_definition: ToolDefinition,
    pub enabled: bool,
}

/// Manages connections to MCP servers and external tool providers.
/// Handles server lifecycle, tool discovery, and protocol communication.
/// Used by: Tool configuration system for external tool integration.
pub struct MCPServer {
    pub id: String,
    pub name: String,
    pub command: String,
    pub args: Vec<String>,
    pub tools: Vec<ToolDefinition>,
    pub status: MCPServerStatus,
    process: Option<Arc<Mutex<Child>>>,
    stdin: Option<Arc<Mutex<ChildStdin>>>,
    stdout: Option<Arc<Mutex<BufReader<ChildStdout>>>>,
    request_id: Arc<Mutex<u64>>,
}

/// Main MCP integration provider for external tool management.
/// Coordinates multiple MCP servers and provides unified tool access.
/// Used by: Agent tool system for accessing external capabilities.
pub struct MCPIntegrationProvider {
    servers: Arc<RwLock<HashMap<String, Arc<Mutex<MCPServer>>>>>,
}

impl MCPIntegrationProvider {
    /// Creates a new MCP integration provider.
    /// Used by: Tool registration system during agent initialization.
    pub fn new() -> Self {
        Self {
            servers: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Adds a new MCP server configuration to the provider.
    /// Used by: Tool configuration loading when setting up external servers.
    pub async fn add_server(&self, server: MCPServer) -> Result<(), AgentError> {
        let server_id = server.id.clone();

        // Store the configuration
        {
            let mut servers = self.servers.write().await;
            servers.insert(server_id.clone(), Arc::new(Mutex::new(server)));
        }

        if server.auto_start && server.enabled {
            self.start_server(&server_id).await?;
        }

        info!("Added MCP server configuration: {}", server.name);
        Ok(())
    }

    /// Starts an MCP server and establishes JSON-RPC communication.
    /// Used by: Server management when activating external tool servers.
    pub async fn start_server(&self, server_id: &str) -> Result<(), AgentError> {
        let mut servers = self.servers.write().await;
        if let Some(server) = servers.get_mut(server_id) {
            server.lock().await.connect().await?;
            Ok(())
        } else {
            Err(AgentError::ServerNotFound(server_id.to_string()))
        }
    }

    /// Stops an MCP server and cleans up resources.
    /// Used by: Server management when deactivating external tool servers.
    pub async fn stop_server(&self, server_id: &str) -> Result<(), AgentError> {
        let mut servers = self.servers.write().await;
        if let Some(server) = servers.get_mut(server_id) {
            server.lock().await.disconnect().await;
            Ok(())
        } else {
            Err(AgentError::ServerNotFound(server_id.to_string()))
        }
    }

    /// Lists all available tools from connected MCP servers.
    /// Used by: Tool discovery system for building available tool catalog.
    pub async fn list_all_tools(&self) -> Result<Vec<ToolDefinition>, AgentError> {
        let servers = self.servers.read().await;
        let mut all_tools = Vec::new();

        for (server_id, server) in servers.iter() {
            if matches!(server.lock().await.status, MCPServerStatus::Connected) {
                all_tools.extend(server.lock().await.tools.iter().cloned());
            }
        }

        Ok(all_tools)
    }

    /// Executes a tool on the appropriate MCP server.
    /// Used by: Agent tool execution when invoking external tools.
    pub async fn execute_tool(&self, tool_call: ToolCall) -> Result<ToolResult, AgentError> {
        let mut servers = self.servers.write().await;

        // Find the server that has this tool
        for (server_id, server) in servers.iter_mut() {
            if server.lock().await.tools.iter().any(|t| t.name == tool_call.name) {
                match server.lock().await.execute_tool(tool_call.name, tool_call.input, tool_call.id.clone()).await {
                    Ok(result) => return Ok(result),
                    Err(e) => return Err(AgentError::ToolError(e)),
                }
            }
        }

        Err(AgentError::ToolNotFound(tool_call.name))
    }

    /// Sends a JSON-RPC request to the specified MCP server.
    /// Used by: Tool execution and server communication for protocol handling.
    async fn send_request(&self, server_id: &str, method: &str, params: Value) -> Result<Value, AgentError> {
        let mut servers = self.servers.write().await;
        if let Some(server) = servers.get_mut(server_id) {
            let server = server.lock().await;
            let request = json!({
                "jsonrpc": "2.0",
                "id": server.next_request_id(),
                "method": method,
                "params": params
            });
            server.send_request(request).await
        } else {
            Err(AgentError::ServerNotFound(server_id.to_string()))
        }
    }

    /// Reads response from MCP server stdout with timeout handling.
    /// Used by: JSON-RPC communication for receiving server responses.
    async fn read_response(&self, server_id: &str) -> Result<Value, AgentError> {
        let mut servers = self.servers.write().await;
        if let Some(server) = servers.get_mut(server_id) {
            let server = server.lock().await;
            server.read_response().await
        } else {
            Err(AgentError::ServerNotFound(server_id.to_string()))
        }
    }

    /// Discovers available tools from an MCP server after connection.
    /// Used by: Server startup to populate tool catalog from external server.
    async fn discover_tools(&self, server_id: &str) -> Result<Vec<ToolDefinition>, AgentError> {
        let mut servers = self.servers.write().await;
        if let Some(server) = servers.get_mut(server_id) {
            server.lock().await.discover_tools().await
        } else {
            Err(AgentError::ServerNotFound(server_id.to_string()))
        }
    }
}

impl MCPServer {
    /// Creates a new MCP server configuration.
    /// Used by: Server configuration when setting up external tool providers.
    pub fn new(id: String, name: String, command: String, args: Vec<String>) -> Self {
        Self {
            id,
            name,
            command,
            args,
            tools: Vec::new(),
            status: MCPServerStatus::Disconnected,
            process: None,
            stdin: None,
            stdout: None,
            request_id: Arc::new(Mutex::new(0)),
        }
    }

    /// Checks if the MCP server is currently connected and operational.
    /// Used by: Server management for connection status verification.
    pub fn is_connected(&self) -> bool {
        matches!(self.status, MCPServerStatus::Connected)
    }
}

#[async_trait]
impl ToolProvider for MCPIntegrationProvider {
    /// Executes tools on external MCP servers.
    /// Used by: Agent tool execution system for external tool invocation.
    async fn execute_tool(&self, tool_call: ToolCall) -> Result<ToolResult, AgentError> {
        self.execute_tool(tool_call).await
    }

    /// Lists all tools available from connected MCP servers.
    /// Used by: Tool discovery and agent initialization systems.
    async fn list_tools(&self) -> Result<Vec<ToolDefinition>, AgentError> {
        self.list_all_tools().await
    }
}

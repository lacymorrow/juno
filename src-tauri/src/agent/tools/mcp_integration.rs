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

use crate::agent::structs::{AgentError, ToolDefinition, ToolResult};

/// Configuration for an external MCP server
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

    pub fn with_description(mut self, description: String) -> Self {
        self.description = Some(description);
        self
    }

    pub fn with_working_directory(mut self, working_directory: PathBuf) -> Self {
        self.working_directory = Some(working_directory);
        self
    }

    pub fn with_environment_variable(mut self, key: String, value: String) -> Self {
        self.environment_variables.insert(key, value);
        self
    }
}

/// Status of an MCP server connection
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum MCPServerStatus {
    Disconnected,
    Connecting,
    Connected,
    Error(String),
    Timeout,
}

/// Information about a discovered MCP tool
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MCPToolInfo {
    pub server_id: String,
    pub server_name: String,
    pub tool_definition: ToolDefinition,
    pub enabled: bool,
}

/// An active MCP server connection
pub struct MCPServerConnection {
    config: MCPServerConfig,
    process: Option<Child>,
    status: MCPServerStatus,
    tools: Vec<ToolDefinition>,
    request_id_counter: u64,
    stdin_writer: Option<BufWriter<tokio::process::ChildStdin>>,
    stdout_reader: Option<BufReader<tokio::process::ChildStdout>>,
}

impl MCPServerConnection {
    pub fn new(config: MCPServerConfig) -> Self {
        Self {
            config,
            process: None,
            status: MCPServerStatus::Disconnected,
            tools: Vec::new(),
            request_id_counter: 0,
            stdin_writer: None,
            stdout_reader: None,
        }
    }

    /// Start the MCP server process and establish connection
    pub async fn connect(&mut self) -> Result<(), String> {
        if matches!(self.status, MCPServerStatus::Connected) {
            return Ok(());
        }

        self.status = MCPServerStatus::Connecting;
        info!("Starting MCP server: {}", self.config.name);

        // Start the server process
        let mut command = Command::new(&self.config.command);
        command.args(&self.config.args);
        command.stdin(Stdio::piped());
        command.stdout(Stdio::piped());
        command.stderr(Stdio::piped());

        if let Some(working_dir) = &self.config.working_directory {
            command.current_dir(working_dir);
        }

        for (key, value) in &self.config.environment_variables {
            command.env(key, value);
        }

        let mut child = command.spawn()
            .map_err(|e| {
                let err = format!("Failed to start MCP server '{}': {}", self.config.name, e);
                self.status = MCPServerStatus::Error(err.clone());
                err
            })?;

        // Setup STDIO communication
        let stdin = child.stdin.take().ok_or_else(|| {
            let err = "Failed to get stdin for MCP server".to_string();
            self.status = MCPServerStatus::Error(err.clone());
            err
        })?;

        let stdout = child.stdout.take().ok_or_else(|| {
            let err = "Failed to get stdout for MCP server".to_string();
            self.status = MCPServerStatus::Error(err.clone());
            err
        })?;

        self.stdin_writer = Some(BufWriter::new(stdin));
        self.stdout_reader = Some(BufReader::new(stdout));
        self.process = Some(child);

        // Initialize the MCP connection
        self.initialize().await?;

        // Discover available tools
        self.discover_tools().await?;

        self.status = MCPServerStatus::Connected;
        info!("Successfully connected to MCP server: {}", self.config.name);
        Ok(())
    }

    /// Send the MCP initialize request
    async fn initialize(&mut self) -> Result<(), String> {
        let request = json!({
            "jsonrpc": "2.0",
            "id": self.next_request_id(),
            "method": "initialize",
            "params": {
                "capabilities": {
                    "tools": {
                        "execution": true
                    }
                }
            }
        });

        let response = self.send_request(request).await?;
        
        if response.get("error").is_some() {
            return Err(format!("MCP server initialization failed: {}", response));
        }

        debug!("MCP server '{}' initialized successfully", self.config.name);
        Ok(())
    }

    /// Discover available tools from the MCP server
    async fn discover_tools(&mut self) -> Result<(), String> {
        let request = json!({
            "jsonrpc": "2.0",
            "id": self.next_request_id(),
            "method": "listTools",
            "params": {}
        });

        let response = self.send_request(request).await?;

        if let Some(error) = response.get("error") {
            return Err(format!("Failed to list tools from MCP server: {}", error));
        }

        let tools_array = response
            .get("result")
            .and_then(|r| r.get("tools"))
            .and_then(|t| t.as_array())
            .ok_or_else(|| "Invalid tools response format".to_string())?;

        self.tools.clear();
        for tool_json in tools_array {
            match self.parse_tool_definition(tool_json) {
                Ok(tool_def) => {
                    debug!("Discovered tool '{}' from server '{}'", tool_def.name, self.config.name);
                    self.tools.push(tool_def);
                }
                Err(e) => {
                    warn!("Failed to parse tool definition: {} - {}", e, tool_json);
                }
            }
        }

        info!("Discovered {} tools from MCP server '{}'", self.tools.len(), self.config.name);
        Ok(())
    }

    /// Parse a tool definition from MCP server response
    fn parse_tool_definition(&self, tool_json: &Value) -> Result<ToolDefinition, String> {
        let name = tool_json.get("name")
            .and_then(|n| n.as_str())
            .ok_or_else(|| "Tool missing name".to_string())?
            .to_string();

        let description = tool_json.get("description")
            .and_then(|d| d.as_str())
            .unwrap_or("")
            .to_string();

        let input_schema = tool_json.get("inputSchema")
            .unwrap_or(&json!({"type": "object", "properties": {}}))
            .clone();

        // Prefix tool name with server name to avoid conflicts
        let prefixed_name = format!("{}_{}", self.config.name, name);

        Ok(ToolDefinition {
            name: prefixed_name,
            description: format!("[{}] {}", self.config.name, description),
            input_schema,
        })
    }

    /// Execute a tool on the MCP server
    pub async fn execute_tool(&mut self, tool_name: &str, input: Value, call_id: String) -> Result<ToolResult, String> {
        // Remove the server prefix from the tool name
        let original_tool_name = tool_name.strip_prefix(&format!("{}_", self.config.name))
            .unwrap_or(tool_name);

        let request = json!({
            "jsonrpc": "2.0",
            "id": self.next_request_id(),
            "method": "callTool",
            "params": {
                "name": original_tool_name,
                "arguments": input
            }
        });

        let response = self.send_request(request).await?;

        if let Some(error) = response.get("error") {
            return Err(format!("Tool execution failed: {}", error));
        }

        let result = response.get("result")
            .unwrap_or(&json!({}))
            .clone();

        Ok(ToolResult {
            call_id,
            output: result,
        })
    }

    /// Send a JSON-RPC request and wait for response
    async fn send_request(&mut self, request: Value) -> Result<Value, String> {
        let request_str = serde_json::to_string(&request)
            .map_err(|e| format!("Failed to serialize request: {}", e))?;

        // Send request
        if let Some(ref mut writer) = self.stdin_writer {
            writer.write_all(request_str.as_bytes()).await
                .map_err(|e| format!("Failed to write request: {}", e))?;
            writer.write_all(b"\n").await
                .map_err(|e| format!("Failed to write newline: {}", e))?;
            writer.flush().await
                .map_err(|e| format!("Failed to flush request: {}", e))?;
        } else {
            return Err("No stdin writer available".to_string());
        }

        // Read response with timeout
        let response_future = async {
            if let Some(ref mut reader) = self.stdout_reader {
                let mut line = String::new();
                reader.read_line(&mut line).await
                    .map_err(|e| format!("Failed to read response: {}", e))?;
                
                serde_json::from_str::<Value>(&line)
                    .map_err(|e| format!("Failed to parse response JSON: {}", e))
            } else {
                Err("No stdout reader available".to_string())
            }
        };

        timeout(Duration::from_secs(self.config.timeout_seconds), response_future)
            .await
            .map_err(|_| {
                self.status = MCPServerStatus::Timeout;
                "Request timeout".to_string()
            })?
    }

    /// Disconnect from the MCP server
    pub async fn disconnect(&mut self) {
        if let Some(mut process) = self.process.take() {
            let _ = process.kill().await;
        }
        self.stdin_writer = None;
        self.stdout_reader = None;
        self.status = MCPServerStatus::Disconnected;
        info!("Disconnected from MCP server: {}", self.config.name);
    }

    fn next_request_id(&mut self) -> u64 {
        self.request_id_counter += 1;
        self.request_id_counter
    }

    pub fn get_status(&self) -> &MCPServerStatus {
        &self.status
    }

    pub fn get_tools(&self) -> &[ToolDefinition] {
        &self.tools
    }

    pub fn get_config(&self) -> &MCPServerConfig {
        &self.config
    }
}

/// Manager for all MCP server connections
pub struct MCPManager {
    servers: Arc<RwLock<HashMap<String, MCPServerConnection>>>,
    configs: Arc<RwLock<HashMap<String, MCPServerConfig>>>,
}

impl MCPManager {
    pub fn new() -> Self {
        Self {
            servers: Arc::new(RwLock::new(HashMap::new())),
            configs: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Add a new MCP server configuration
    pub async fn add_server(&self, config: MCPServerConfig) -> Result<(), String> {
        let server_id = config.id.clone();
        
        // Store the configuration
        {
            let mut configs = self.configs.write().await;
            configs.insert(server_id.clone(), config.clone());
        }

        // Create and optionally start the connection
        let connection = MCPServerConnection::new(config.clone());
        {
            let mut servers = self.servers.write().await;
            servers.insert(server_id.clone(), connection);
        }

        if config.auto_start && config.enabled {
            self.start_server(&server_id).await?;
        }

        info!("Added MCP server configuration: {}", config.name);
        Ok(())
    }

    /// Start a specific MCP server
    pub async fn start_server(&self, server_id: &str) -> Result<(), String> {
        let mut servers = self.servers.write().await;
        if let Some(connection) = servers.get_mut(server_id) {
            connection.connect().await
        } else {
            Err(format!("MCP server not found: {}", server_id))
        }
    }

    /// Stop a specific MCP server
    pub async fn stop_server(&self, server_id: &str) -> Result<(), String> {
        let mut servers = self.servers.write().await;
        if let Some(connection) = servers.get_mut(server_id) {
            connection.disconnect().await;
            Ok(())
        } else {
            Err(format!("MCP server not found: {}", server_id))
        }
    }

    /// Get all available tools from all connected servers
    pub async fn get_all_tools(&self) -> Vec<MCPToolInfo> {
        let servers = self.servers.read().await;
        let mut all_tools = Vec::new();

        for (server_id, connection) in servers.iter() {
            if matches!(connection.get_status(), MCPServerStatus::Connected) {
                for tool_def in connection.get_tools() {
                    all_tools.push(MCPToolInfo {
                        server_id: server_id.clone(),
                        server_name: connection.get_config().name.clone(),
                        tool_definition: tool_def.clone(),
                        enabled: true, // TODO: Get from configuration
                    });
                }
            }
        }

        all_tools
    }

    /// Execute a tool on the appropriate MCP server
    pub async fn execute_tool(&self, tool_name: &str, input: Value, call_id: String) -> Result<ToolResult, AgentError> {
        let mut servers = self.servers.write().await;

        // Find the server that has this tool
        for (_, connection) in servers.iter_mut() {
            if connection.get_tools().iter().any(|t| t.name == tool_name) {
                match connection.execute_tool(tool_name, input, call_id).await {
                    Ok(result) => return Ok(result),
                    Err(e) => return Err(AgentError::ToolError(e)),
                }
            }
        }

        Err(AgentError::ToolNotFound(tool_name.to_string()))
    }

    /// Get status of all servers
    pub async fn get_server_statuses(&self) -> HashMap<String, MCPServerStatus> {
        let servers = self.servers.read().await;
        let mut statuses = HashMap::new();

        for (server_id, connection) in servers.iter() {
            statuses.insert(server_id.clone(), connection.get_status().clone());
        }

        statuses
    }

    /// Remove an MCP server
    pub async fn remove_server(&self, server_id: &str) -> Result<(), String> {
        // Stop the server first
        self.stop_server(server_id).await?;

        // Remove from both configs and servers
        {
            let mut configs = self.configs.write().await;
            configs.remove(server_id);
        }
        {
            let mut servers = self.servers.write().await;
            servers.remove(server_id);
        }

        info!("Removed MCP server: {}", server_id);
        Ok(())
    }

    /// Start all enabled servers
    pub async fn start_all_enabled_servers(&self) -> Result<(), String> {
        let configs = self.configs.read().await;
        let server_ids: Vec<String> = configs
            .values()
            .filter(|config| config.enabled && config.auto_start)
            .map(|config| config.id.clone())
            .collect();

        drop(configs); // Release the read lock

        for server_id in server_ids {
            if let Err(e) = self.start_server(&server_id).await {
                error!("Failed to start MCP server {}: {}", server_id, e);
            }
        }

        Ok(())
    }

    /// Get all server configurations
    pub async fn get_server_configs(&self) -> Vec<MCPServerConfig> {
        let configs = self.configs.read().await;
        configs.values().cloned().collect()
    }
}

impl Default for MCPManager {
    fn default() -> Self {
        Self::new()
    }
}
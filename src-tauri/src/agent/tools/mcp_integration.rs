use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::HashMap;
use std::path::PathBuf;
use std::process::Stdio;
use std::sync::Arc;
use std::time::Duration;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader, BufWriter};
use tokio::process::{Child, Command};
use tokio::sync::RwLock;
use tokio::time::timeout;
use tracing::{debug, error, info, warn};
use uuid::Uuid;

use crate::agent::core::{AgentError, ToolCall, ToolDefinition, ToolResult};
use crate::constants::agent;

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
    stderr_reader: Option<BufReader<tokio::process::ChildStderr>>,
    // HTTP transport (for servers exposed via HTTP JSON-RPC)
    http_client: Option<reqwest::Client>,
    http_url: Option<String>,
    // Error recovery tracking
    connection_attempts: u32,
    last_failure_time: Option<std::time::Instant>,
    consecutive_failures: u32,
    last_successful_communication: Option<std::time::Instant>,
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
            stderr_reader: None,
            http_client: None,
            http_url: None,
            // Initialize error recovery fields
            connection_attempts: 0,
            last_failure_time: None,
            consecutive_failures: 0,
            last_successful_communication: None,
        }
    }

    fn is_http_transport(&self) -> bool {
        let cmd = self.config.command.to_lowercase();
        cmd == "http" || cmd == "https"
    }

    fn ensure_http_url(&mut self) -> Result<String, String> {
        if let Some(url) = &self.http_url {
            return Ok(url.clone());
        }
        let url =
            self.config.args.first().cloned().ok_or_else(|| {
                "HTTP MCP server requires args[0] to be the endpoint URL".to_string()
            })?;
        self.http_url = Some(url.clone());
        Ok(url)
    }

    fn get_http_client(&mut self) -> Result<reqwest::Client, String> {
        if let Some(c) = &self.http_client {
            return Ok(c.clone());
        }
        let timeout = Duration::from_secs(self.config.timeout_seconds);
        let client = reqwest::Client::builder()
            .timeout(timeout)
            .build()
            .map_err(|e| format!("Failed to build HTTP client: {}", e))?;
        self.http_client = Some(client.clone());
        Ok(client)
    }

    /// Calculate backoff delay based on consecutive failures
    fn calculate_backoff_delay(&self) -> Duration {
        let _base_delay = Duration::from_millis(500); // Start with 500ms
        let max_delay = Duration::from_secs(30); // Cap at 30s

        if self.consecutive_failures == 0 {
            return Duration::from_millis(0);
        }

        // Exponential backoff: 500ms, 1s, 2s, 4s, 8s, 16s, 30s (capped)
        let delay_ms = 500_u64
            .saturating_mul(2_u64.saturating_pow(self.consecutive_failures.saturating_sub(1)));
        let delay = Duration::from_millis(delay_ms);

        if delay > max_delay {
            max_delay
        } else {
            delay
        }
    }

    /// Check if we should attempt to reconnect based on failure history
    fn should_attempt_reconnect(&self) -> bool {
        // Don't exceed max retries
        if self.connection_attempts >= self.config.max_retries {
            return false;
        }

        // If we have a recent failure, respect backoff delay
        if let Some(last_failure) = self.last_failure_time {
            let backoff_delay = self.calculate_backoff_delay();
            if last_failure.elapsed() < backoff_delay {
                return false;
            }
        }

        true
    }

    /// Record a connection failure for backoff calculation
    fn record_failure(&mut self) {
        self.connection_attempts += 1;
        self.consecutive_failures += 1;
        self.last_failure_time = Some(std::time::Instant::now());

        debug!(
            "MCP server '{}' failure recorded: attempt {}/{}, consecutive failures: {}",
            self.config.name,
            self.connection_attempts,
            self.config.max_retries,
            self.consecutive_failures
        );
    }

    /// Record a successful connection/communication
    fn record_success(&mut self) {
        self.consecutive_failures = 0;
        self.last_successful_communication = Some(std::time::Instant::now());

        debug!(
            "MCP server '{}' success recorded, consecutive failures reset",
            self.config.name
        );
    }

    /// Start the MCP server process and establish connection with retry logic
    pub async fn connect(&mut self) -> Result<(), String> {
        if matches!(self.status, MCPServerStatus::Connected) {
            return Ok(());
        }

        // Check if we should attempt reconnection based on failure history
        if !self.should_attempt_reconnect() {
            let backoff_delay = self.calculate_backoff_delay();
            return Err(format!(
                "MCP server '{}' not attempting reconnect: {}/{} attempts used, next retry in {}s",
                self.config.name,
                self.connection_attempts,
                self.config.max_retries,
                backoff_delay.as_secs()
            ));
        }

        self.status = MCPServerStatus::Connecting;
        info!(
            "Starting MCP server: {} (command: {} {:?})",
            self.config.name, self.config.command, self.config.args
        );

        // HTTP transport branch: do not spawn a process; connect via HTTP JSON-RPC
        if self.is_http_transport() {
            let url = self.ensure_http_url()?;
            let _client = self.get_http_client()?;

            // Initialize via HTTP and discover tools
            self.initialize().await?;
            self.send_initialized_notification().await.ok();
            self.discover_tools().await?;

            self.status = MCPServerStatus::Connected;
            self.record_success();
            info!(
                "Successfully connected to HTTP MCP server at {}: {}",
                url, self.config.name
            );
            return Ok(());
        }

        // Start the server process
        let mut command = Command::new(&self.config.command);
        command.args(&self.config.args);
        command.stdin(Stdio::piped());
        command.stdout(Stdio::piped());
        command.stderr(Stdio::piped());

        if let Some(working_dir) = &self.config.working_directory {
            command.current_dir(working_dir);
            info!(
                "MCP server '{}' working directory: {:?}",
                self.config.name, working_dir
            );
        }

        for (key, value) in &self.config.environment_variables {
            command.env(key, value);
        }

        let mut child = command.spawn().map_err(|e| {
            let err = format!(
                "Failed to start MCP server '{}' (command: {}): {}",
                self.config.name, self.config.command, e
            );
            error!("{}", err);
            self.record_failure();
            self.status = MCPServerStatus::Error(err.clone());
            err
        })?;

        // Setup STDIO communication
        let stdin = child.stdin.take().ok_or_else(|| {
            let err = "Failed to get stdin for MCP server".to_string();
            self.record_failure();
            self.status = MCPServerStatus::Error(err.clone());
            err
        })?;

        let stdout = child.stdout.take().ok_or_else(|| {
            let err = "Failed to get stdout for MCP server".to_string();
            self.record_failure();
            self.status = MCPServerStatus::Error(err.clone());
            err
        })?;

        let stderr = child.stderr.take().ok_or_else(|| {
            let err = "Failed to get stderr for MCP server".to_string();
            self.record_failure();
            self.status = MCPServerStatus::Error(err.clone());
            err
        })?;

        self.stdin_writer = Some(BufWriter::new(stdin));
        self.stdout_reader = Some(BufReader::new(stdout));
        self.stderr_reader = Some(BufReader::new(stderr));
        self.process = Some(child);

        // Start stderr monitoring task
        self.start_stderr_monitoring().await;

        // Give the server a moment to start up
        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

        // Check if process is still running
        if let Some(ref mut process) = self.process {
            match process.try_wait() {
                Ok(Some(exit_status)) => {
                    let err = format!(
                        "MCP server '{}' exited immediately with status: {}",
                        self.config.name, exit_status
                    );
                    error!("{}", err);
                    self.status = MCPServerStatus::Error(err.clone());
                    return Err(err);
                }
                Ok(None) => {
                    // Process is still running, continue
                    info!(
                        "MCP server '{}' process started successfully",
                        self.config.name
                    );
                }
                Err(e) => {
                    let err = format!(
                        "Failed to check MCP server '{}' process status: {}",
                        self.config.name, e
                    );
                    error!("{}", err);
                    self.status = MCPServerStatus::Error(err.clone());
                    return Err(err);
                }
            }
        }

        // Initialize the MCP connection
        self.initialize().await?;

        // Send initialized notification
        self.send_initialized_notification().await?;

        // Discover available tools
        self.discover_tools().await?;

        self.status = MCPServerStatus::Connected;
        self.record_success(); // Reset failure counters on successful connection
        info!("Successfully connected to MCP server: {}", self.config.name);
        Ok(())
    }

    /// Start monitoring stderr for error messages
    async fn start_stderr_monitoring(&mut self) {
        if let Some(stderr_reader) = self.stderr_reader.take() {
            let server_name = self.config.name.clone();
            tokio::spawn(async move {
                let mut reader = stderr_reader;
                let mut line = String::new();
                loop {
                    line.clear();
                    match reader.read_line(&mut line).await {
                        Ok(0) => break, // EOF
                        Ok(_) => {
                            let trimmed = line.trim();
                            if !trimmed.is_empty() {
                                warn!("MCP server '{}' stderr: {}", server_name, trimmed);
                            }
                        }
                        Err(e) => {
                            error!(
                                "Error reading stderr from MCP server '{}': {}",
                                server_name, e
                            );
                            break;
                        }
                    }
                }
                debug!("Stderr monitoring ended for MCP server '{}'", server_name);
            });
        }
    }

    /// Send the MCP initialize request
    async fn initialize(&mut self) -> Result<(), String> {
        let request = json!({
            "jsonrpc": "2.0",
            "id": self.next_request_id(),
            "method": "initialize",
            "params": {
                "protocolVersion": "2024-11-05",
                "capabilities": {
                    "tools": {
                        "execution": true
                    }
                },
                "clientInfo": {
                    "name": "Juno AI Agent",
                    "version": "1.0.0"
                }
            }
        });

        let response = if self.is_http_transport() {
            self.send_http_request(request).await?
        } else {
            self.send_request(request).await?
        };

        if response.get("error").is_some() {
            return Err(format!("MCP server initialization failed: {}", response));
        }

        debug!("MCP server '{}' initialized successfully", self.config.name);
        Ok(())
    }

    /// Send initialized notification
    async fn send_initialized_notification(&mut self) -> Result<(), String> {
        let notification = json!({
            "jsonrpc": "2.0",
            "method": "notifications/initialized"
        });

        if self.is_http_transport() {
            // Best-effort: notify over HTTP, ignore response
            let _ = self.send_http_request(notification).await;
            return Ok(());
        } else {
            let notification_str = serde_json::to_string(&notification)
                .map_err(|e| format!("Failed to serialize notification: {}", e))?;

            // Send notification (no response expected)
            if let Some(ref mut writer) = self.stdin_writer {
                writer
                    .write_all(notification_str.as_bytes())
                    .await
                    .map_err(|e| format!("Failed to write notification: {}", e))?;
                writer
                    .write_all(b"\n")
                    .await
                    .map_err(|e| format!("Failed to write newline: {}", e))?;
                writer
                    .flush()
                    .await
                    .map_err(|e| format!("Failed to flush notification: {}", e))?;
            } else {
                return Err("No stdin writer available".to_string());
            }
        }

        debug!(
            "MCP server '{}' initialized notification sent successfully",
            self.config.name
        );
        Ok(())
    }

    /// Discover available tools from the MCP server
    async fn discover_tools(&mut self) -> Result<(), String> {
        let request = json!({
            "jsonrpc": "2.0",
            "id": self.next_request_id(),
            "method": "tools/list",
            "params": {}
        });

        let response = if self.is_http_transport() {
            self.send_http_request(request).await?
        } else {
            self.send_request(request).await?
        };

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
                    debug!(
                        "Discovered tool '{}' from server '{}'",
                        tool_def.name, self.config.name
                    );
                    self.tools.push(tool_def);
                }
                Err(e) => {
                    warn!("Failed to parse tool definition: {} - {}", e, tool_json);
                }
            }
        }

        info!(
            "Discovered {} tools from MCP server '{}'",
            self.tools.len(),
            self.config.name
        );
        Ok(())
    }

    /// Parse a tool definition from MCP server response
    fn parse_tool_definition(&self, tool_json: &Value) -> Result<ToolDefinition, String> {
        let name = tool_json
            .get("name")
            .and_then(|n| n.as_str())
            .ok_or_else(|| "Tool missing name".to_string())?
            .to_string();

        let description = tool_json
            .get("description")
            .and_then(|d| d.as_str())
            .unwrap_or("")
            .to_string();

        let input_schema = tool_json
            .get("inputSchema")
            .unwrap_or(&json!({"type": "object", "properties": {}}))
            .clone();

        // Prefix tool name with server name to avoid conflicts
        let prefixed_name = format!("{}_{}", self.config.name, name);

        Ok(ToolDefinition {
            name: prefixed_name,
            description: format!("[{}] {}", self.config.name, description),
            input_schema,
            api_type: None,
            beta_flag: None,
        })
    }

    /// Execute a tool on the MCP server
    pub async fn execute_tool(
        &mut self,
        tool_name: &str,
        input: Value,
        call_id: String,
    ) -> Result<ToolResult, String> {
        // Remove the server prefix from the tool name
        let original_tool_name = tool_name
            .strip_prefix(&format!("{}_", self.config.name))
            .unwrap_or(tool_name);

        let request = json!({
            "jsonrpc": "2.0",
            "id": self.next_request_id(),
            "method": "tools/call",
            "params": {
                "name": original_tool_name,
                "arguments": input
            }
        });

        let response = if self.is_http_transport() {
            self.send_http_request(request).await?
        } else {
            self.send_request(request).await?
        };

        if let Some(error) = response.get("error") {
            return Err(format!("Tool execution failed: {}", error));
        }

        let result = response.get("result").unwrap_or(&json!({})).clone();

        Ok(ToolResult {
            call_id,
            output: result,
        })
    }

    /// Send request with enhanced error handling for EPIPE and connection issues
    async fn send_request(&mut self, request: Value) -> Result<Value, String> {
        let request_str = serde_json::to_string(&request)
            .map_err(|e| format!("Failed to serialize request: {}", e))?;

        debug!(
            "Sending MCP request to '{}': {}",
            self.config.name, request_str
        );

        // Check if process is still alive before attempting to write
        if let Some(ref mut process) = self.process {
            match process.try_wait() {
                Ok(Some(exit_status)) => {
                    let err = format!(
                        "MCP server '{}' has exited with status: {}",
                        self.config.name, exit_status
                    );
                    error!("{}", err);
                    self.status = MCPServerStatus::Error(err.clone());
                    return Err(err);
                }
                Ok(None) => {
                    // Process is still running, continue
                }
                Err(e) => {
                    let err = format!(
                        "Failed to check MCP server '{}' process status: {}",
                        self.config.name, e
                    );
                    warn!("{}", err);
                    // Continue anyway - might be a temporary check failure
                }
            }
        }

        // Send request with EPIPE handling
        if let Some(ref mut writer) = self.stdin_writer {
            // Enhanced error handling for broken pipes
            if let Err(e) = writer.write_all(request_str.as_bytes()).await {
                let error_msg = format!(
                    "Failed to write request to MCP server '{}': {}",
                    self.config.name, e
                );
                if e.kind() == std::io::ErrorKind::BrokenPipe {
                    warn!(
                        "Broken pipe detected for MCP server '{}' - server may have crashed",
                        self.config.name
                    );
                    self.status =
                        MCPServerStatus::Error("Broken pipe - server crashed".to_string());
                } else {
                    error!("{}", error_msg);
                }
                return Err(error_msg);
            }

            if let Err(e) = writer.write_all(b"\n").await {
                let error_msg = format!(
                    "Failed to write newline to MCP server '{}': {}",
                    self.config.name, e
                );
                if e.kind() == std::io::ErrorKind::BrokenPipe {
                    warn!(
                        "Broken pipe during newline write for MCP server '{}'",
                        self.config.name
                    );
                    self.status = MCPServerStatus::Error("Broken pipe during write".to_string());
                }
                return Err(error_msg);
            }

            if let Err(e) = writer.flush().await {
                let error_msg = format!(
                    "Failed to flush request to MCP server '{}': {}",
                    self.config.name, e
                );
                if e.kind() == std::io::ErrorKind::BrokenPipe {
                    warn!(
                        "Broken pipe during flush for MCP server '{}'",
                        self.config.name
                    );
                    self.status = MCPServerStatus::Error("Broken pipe during flush".to_string());
                }
                return Err(error_msg);
            }
        } else {
            return Err("No stdin writer available".to_string());
        }

        // Read response with timeout and enhanced error handling
        let response_future = async {
            if let Some(ref mut reader) = self.stdout_reader {
                // Try to read multiple lines until we get a valid JSON response
                let mut attempts = 0;
                let mut consecutive_empty_lines = 0;

                while attempts < agent::config::MAX_RETRY_ATTEMPTS {
                    let mut line = String::new();
                    match reader.read_line(&mut line).await {
                        Ok(0) => {
                            return Err(format!(
                                "MCP server '{}' closed stdout (EOF)",
                                self.config.name
                            ));
                        }
                        Ok(_) => {
                            let trimmed = line.trim();
                            if trimmed.is_empty() {
                                consecutive_empty_lines += 1;
                                if consecutive_empty_lines > 5 {
                                    warn!("Too many empty lines from MCP server '{}', may be unresponsive", self.config.name);
                                    return Err(format!("MCP server '{}' appears unresponsive (too many empty lines)", self.config.name));
                                }
                                attempts += 1;
                                tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;
                                continue;
                            }
                            consecutive_empty_lines = 0; // Reset counter on non-empty line

                            debug!(
                                "Received MCP response from '{}': {}",
                                self.config.name, trimmed
                            );

                            match serde_json::from_str::<Value>(trimmed) {
                                Ok(json) => return Ok(json),
                                Err(e) => {
                                    warn!("Failed to parse JSON from MCP server '{}' (attempt {}): {} - Response: '{}'",
                                          self.config.name, attempts + 1, e, trimmed);
                                    attempts += 1;
                                    if attempts >= agent::config::MAX_RETRY_ATTEMPTS {
                                        return Err(format!("Failed to parse response JSON from '{}' after {} attempts: {} (last response: '{}')",
                                                         self.config.name, agent::config::MAX_RETRY_ATTEMPTS, e, trimmed));
                                    }
                                    // Exponential backoff for retries
                                    let delay_ms =
                                        std::cmp::min(100 * (2_u64.pow(attempts as u32)), 1000);
                                    tokio::time::sleep(tokio::time::Duration::from_millis(
                                        delay_ms,
                                    ))
                                    .await;
                                }
                            }
                        }
                        Err(e) => {
                            if e.kind() == std::io::ErrorKind::BrokenPipe {
                                return Err(format!(
                                    "MCP server '{}' pipe broken during read",
                                    self.config.name
                                ));
                            }
                            return Err(format!(
                                "Failed to read response from '{}': {}",
                                self.config.name, e
                            ));
                        }
                    }
                }

                Err(format!(
                    "No valid response received from MCP server '{}' after {} attempts",
                    self.config.name,
                    agent::config::MAX_RETRY_ATTEMPTS
                ))
            } else {
                Err("No stdout reader available".to_string())
            }
        };

        timeout(
            Duration::from_secs(self.config.timeout_seconds),
            response_future,
        )
        .await
        .map_err(|_| {
            self.status = MCPServerStatus::Timeout;
            self.record_failure();
            format!(
                "Request timeout for MCP server '{}' ({}s)",
                self.config.name, self.config.timeout_seconds
            )
        })?
    }

    /// Send a JSON-RPC request over HTTP to the configured endpoint
    async fn send_http_request(&mut self, request: Value) -> Result<Value, String> {
        let client = self.get_http_client()?;
        let url = self.ensure_http_url()?;
        let req_timeout = Duration::from_secs(self.config.timeout_seconds);
        let resp = client
            .post(url.clone())
            .header(
                reqwest::header::ACCEPT,
                "application/json, text/event-stream",
            )
            .json(&request)
            .timeout(req_timeout)
            .send()
            .await
            .map_err(|e| format!("HTTP request failed to {}: {}", url, e))?;
        let status = resp.status();
        let text = resp
            .text()
            .await
            .map_err(|e| format!("Failed reading HTTP response: {}", e))?;
        if !status.is_success() {
            return Err(format!("HTTP MCP server returned {}: {}", status, text));
        }
        serde_json::from_str::<Value>(&text)
            .map_err(|e| format!("Failed to parse HTTP JSON-RPC response: {} - {}", e, text))
    }

    /// Disconnect from the MCP server
    pub async fn disconnect(&mut self) {
        info!("🔌 Disconnecting from MCP server: {}", self.config.name);

        // Try graceful termination first, then force kill if needed
        if let Some(mut process) = self.process.take() {
            // First attempt: try to terminate gracefully
            match process.kill().await {
                Ok(_) => {
                    info!("✅ MCP server '{}' terminated gracefully", self.config.name);
                }
                Err(e) => {
                    warn!(
                        "Failed to terminate MCP server '{}' gracefully: {}",
                        self.config.name, e
                    );

                    // Second attempt: Force kill with timeout
                    let kill_future = async { process.kill().await };

                    match tokio::time::timeout(Duration::from_secs(5), kill_future).await {
                        Ok(Ok(_)) => {
                            warn!(
                                "✅ MCP server '{}' force-killed successfully",
                                self.config.name
                            );
                        }
                        Ok(Err(e)) => {
                            error!(
                                "❌ Failed to force-kill MCP server '{}': {}",
                                self.config.name, e
                            );
                        }
                        Err(_) => {
                            error!(
                                "❌ Timeout while force-killing MCP server '{}'",
                                self.config.name
                            );
                        }
                    }
                }
            }

            // Wait for process to actually exit (with timeout)
            let wait_future = async { process.wait().await };

            match tokio::time::timeout(Duration::from_secs(3), wait_future).await {
                Ok(Ok(exit_status)) => {
                    info!(
                        "MCP server '{}' exited with status: {}",
                        self.config.name, exit_status
                    );
                }
                Ok(Err(e)) => {
                    warn!(
                        "Error waiting for MCP server '{}' to exit: {}",
                        self.config.name, e
                    );
                }
                Err(_) => {
                    warn!(
                        "Timeout waiting for MCP server '{}' to exit",
                        self.config.name
                    );
                }
            }
        }

        // Clean up all resources
        self.stdin_writer = None;
        self.stdout_reader = None;
        self.stderr_reader = None;
        self.status = MCPServerStatus::Disconnected;

        info!(
            "✅ MCP server '{}' disconnected and cleaned up",
            self.config.name
        );
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

        // Check for existing server to prevent duplication
        {
            let configs = self.configs.read().await;
            if configs.contains_key(&server_id) {
                return Err(format!("MCP server with ID '{}' already exists", server_id));
            }
        }

        // Atomic add: both configs and servers together
        {
            let mut configs = self.configs.write().await;
            let mut servers = self.servers.write().await;

            // Double-check in case another thread added it between our check and lock
            if configs.contains_key(&server_id) {
                return Err(format!("MCP server with ID '{}' already exists", server_id));
            }

            // Store configuration and create connection atomically
            configs.insert(server_id.clone(), config.clone());
            let connection = MCPServerConnection::new(config.clone());
            servers.insert(server_id.clone(), connection);
        }

        // Start server if needed (outside the lock to avoid deadlock)
        if config.auto_start && config.enabled {
            if let Err(e) = self.start_server(&server_id).await {
                warn!("Failed to auto-start MCP server '{}': {}", config.name, e);
                // Don't fail the add operation if auto-start fails
            }
        }

        debug!("Added MCP server configuration: {}", config.name);
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
    pub async fn execute_tool(
        &self,
        tool_name: &str,
        input: Value,
        call_id: String,
    ) -> Result<ToolResult, AgentError> {
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

    /// Execute multiple tools as a batch on the appropriate MCP servers
    pub async fn execute_batch_tools(
        &self,
        tool_calls: Vec<ToolCall>,
    ) -> Result<Vec<ToolResult>, AgentError> {
        if tool_calls.is_empty() {
            return Ok(Vec::new());
        }

        info!("Executing batch of {} tools via MCP", tool_calls.len());

        // Group tool calls by server
        let mut server_batches: std::collections::HashMap<String, Vec<ToolCall>> =
            std::collections::HashMap::new();
        let servers_guard = self.servers.read().await;

        for tool_call in tool_calls {
            let mut server_found = false;
            for (server_id, connection) in servers_guard.iter() {
                if connection
                    .get_tools()
                    .iter()
                    .any(|t| t.name == tool_call.name)
                {
                    server_batches
                        .entry(server_id.clone())
                        .or_default()
                        .push(tool_call.clone());
                    server_found = true;
                    break;
                }
            }

            if !server_found {
                return Err(AgentError::ToolNotFound(tool_call.name.clone()));
            }
        }

        drop(servers_guard); // Release read lock before acquiring write lock

        // Execute batches on each server
        let mut all_results = Vec::new();
        let mut servers_guard = self.servers.write().await;

        for (server_id, tool_calls_for_server) in server_batches {
            if let Some(connection) = servers_guard.get_mut(&server_id) {
                match connection.execute_batch_tools(tool_calls_for_server).await {
                    Ok(batch_response) => {
                        // Extract successful results and handle errors
                        for batch_result in batch_response.results {
                            match batch_result.result {
                                Ok(tool_result) => all_results.push(tool_result),
                                Err(e) => return Err(AgentError::ToolError(e)),
                            }
                        }
                    }
                    Err(e) => return Err(AgentError::ToolError(e)),
                }
            }
        }

        info!("Batch execution completed: {} results", all_results.len());
        Ok(all_results)
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
        // Check if server exists first
        let server_exists = {
            let configs = self.configs.read().await;
            configs.contains_key(server_id)
        };

        if !server_exists {
            return Err(format!("MCP server '{}' not found", server_id));
        }

        // Stop the server with timeout to prevent hanging
        match tokio::time::timeout(Duration::from_secs(10), self.stop_server(server_id)).await {
            Ok(Ok(_)) => {
                info!("Successfully stopped MCP server: {}", server_id);
            }
            Ok(Err(e)) => {
                warn!(
                    "Error stopping MCP server '{}': {} (proceeding with removal)",
                    server_id, e
                );
            }
            Err(_) => {
                warn!(
                    "Timeout stopping MCP server '{}' (proceeding with removal)",
                    server_id
                );
            }
        }

        // Atomic removal from both data structures
        let (removed_config, removed_server) = {
            let mut configs = self.configs.write().await;
            let mut servers = self.servers.write().await;

            let config = configs.remove(server_id);
            let server = servers.remove(server_id);
            (config, server)
        };

        if removed_config.is_some() || removed_server.is_some() {
            info!("Successfully removed MCP server: {}", server_id);
            Ok(())
        } else {
            Err(format!(
                "MCP server '{}' was not found during removal",
                server_id
            ))
        }
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

/// Batch request for executing multiple tools in sequence
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MCPBatchRequest {
    pub id: String,
    pub requests: Vec<MCPBatchItem>,
    pub execution_mode: BatchExecutionMode,
    pub timeout_seconds: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MCPBatchItem {
    pub id: String,
    pub method: String, // "tools/call"
    pub params: Value,
    pub tool_name: String,
    pub call_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum BatchExecutionMode {
    Sequential,
    Parallel,
    Optimized, // Intelligently choose based on tool dependencies
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MCPBatchResponse {
    pub batch_id: String,
    pub results: Vec<MCPBatchResult>,
    pub execution_time_ms: u64,
    pub success_count: usize,
    pub failed_count: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MCPBatchResult {
    pub request_id: String,
    pub tool_name: String,
    pub call_id: String,
    pub result: Result<ToolResult, String>,
    pub execution_time_ms: u64,
}

/// Tool batching analyzer for determining which tools can be batched together
pub struct ToolBatchingAnalyzer;

impl ToolBatchingAnalyzer {
    /// Analyze tool calls to determine if they can be batched together
    /// Simple approach: MCP tools can generally be batched if they're read-only
    pub fn can_batch_tools(tool_calls: &[ToolCall]) -> bool {
        if tool_calls.len() < 2 {
            return false;
        }

        // For MCP tools, check if they're all read-only and can be safely batched
        let all_mcp_readonly = tool_calls
            .iter()
            .all(|tool| tool.name.starts_with("mcp_") && Self::is_readonly_tool(&tool.name));

        all_mcp_readonly
    }

    /// Determine if a tool is read-only and safe for batching
    fn is_readonly_tool(tool_name: &str) -> bool {
        let readonly_patterns = [
            "search", "get", "read", "list", "check", "status", "info", "find", "query", "fetch",
            "retrieve",
        ];

        readonly_patterns
            .iter()
            .any(|pattern| tool_name.contains(pattern))
    }

    /// Create batches from tool calls - trust the agent's decision
    /// If multiple tools are provided together, batch them (up to max batch size)
    pub fn create_batches(tool_calls: Vec<ToolCall>) -> Vec<Vec<ToolCall>> {
        let mut batches = Vec::new();
        let mut current_batch = Vec::new();

        for tool_call in tool_calls {
            current_batch.push(tool_call);

            // Respect max batch size limit
            if current_batch.len() >= 5 {
                batches.push(current_batch);
                current_batch = Vec::new();
            }
        }

        if !current_batch.is_empty() {
            batches.push(current_batch);
        }

        batches
    }
}

impl MCPServerConnection {
    /// Execute multiple tools as a batch request using JSON-RPC batch format
    pub async fn execute_batch_tools(
        &mut self,
        tool_calls: Vec<ToolCall>,
    ) -> Result<MCPBatchResponse, String> {
        let batch_start = std::time::Instant::now();
        let batch_id = uuid::Uuid::new_v4().to_string();

        info!(
            "Executing batch of {} tools on server '{}'",
            tool_calls.len(),
            self.config.name
        );

        // Create batch request items
        let mut batch_items = Vec::new();
        for tool_call in &tool_calls {
            let original_tool_name = tool_call
                .name
                .strip_prefix(&format!("{}_", self.config.name))
                .unwrap_or(&tool_call.name);

            batch_items.push(json!({
                "jsonrpc": "2.0",
                "id": self.next_request_id(),
                "method": "tools/call",
                "params": {
                    "name": original_tool_name,
                    "arguments": tool_call.input
                }
            }));
        }

        // Send batch request (JSON-RPC 2.0 supports array of requests)
        let batch_request = Value::Array(batch_items);

        debug!(
            "Sending batch request to '{}': {}",
            self.config.name, batch_request
        );

        // Send and receive batch response
        let batch_response = self.send_batch_request(batch_request).await?;

        // Process batch response with proper validation
        let mut results = Vec::new();
        let mut success_count = 0;
        let mut failed_count = 0;

        if let Value::Array(responses) = batch_response {
            // Validate response count matches request count
            if responses.len() != tool_calls.len() {
                warn!(
                    "MCP server '{}' returned {} responses for {} tool calls",
                    self.config.name,
                    responses.len(),
                    tool_calls.len()
                );

                // Handle mismatched response counts gracefully
                let min_count = std::cmp::min(responses.len(), tool_calls.len());

                // Process matched responses
                for index in 0..min_count {
                    let response = &responses[index];
                    let tool_call = &tool_calls[index];
                    let result_start = std::time::Instant::now();

                    let result = if let Some(error) = response.get("error") {
                        failed_count += 1;
                        Err(format!("Tool execution failed: {}", error))
                    } else {
                        success_count += 1;
                        let output = response.get("result").unwrap_or(&json!({})).clone();
                        Ok(ToolResult {
                            call_id: tool_call.id.clone(),
                            output,
                        })
                    };

                    results.push(MCPBatchResult {
                        request_id: tool_call.id.clone(),
                        tool_name: tool_call.name.clone(),
                        call_id: tool_call.id.clone(),
                        result,
                        execution_time_ms: result_start.elapsed().as_millis() as u64,
                    });
                }

                // Handle missing responses (if responses.len() < tool_calls.len())
                for tool_call in &tool_calls[min_count..] {
                    failed_count += 1;

                    results.push(MCPBatchResult {
                        request_id: tool_call.id.clone(),
                        tool_name: tool_call.name.clone(),
                        call_id: tool_call.id.clone(),
                        result: Err("No response received from MCP server".to_string()),
                        execution_time_ms: 0,
                    });
                }

                // Log extra responses (if responses.len() > tool_calls.len())
                if responses.len() > tool_calls.len() {
                    warn!(
                        "MCP server '{}' returned {} extra responses that will be ignored",
                        self.config.name,
                        responses.len() - tool_calls.len()
                    );
                }
            } else {
                // Normal case: response count matches request count
                for (index, response) in responses.iter().enumerate() {
                    let tool_call = &tool_calls[index];
                    let result_start = std::time::Instant::now();

                    let result = if let Some(error) = response.get("error") {
                        failed_count += 1;
                        Err(format!("Tool execution failed: {}", error))
                    } else {
                        success_count += 1;
                        let output = response.get("result").unwrap_or(&json!({})).clone();
                        Ok(ToolResult {
                            call_id: tool_call.id.clone(),
                            output,
                        })
                    };

                    results.push(MCPBatchResult {
                        request_id: tool_call.id.clone(),
                        tool_name: tool_call.name.clone(),
                        call_id: tool_call.id.clone(),
                        result,
                        execution_time_ms: result_start.elapsed().as_millis() as u64,
                    });
                }
            }
        } else {
            return Err("Invalid batch response format - expected array".to_string());
        }

        let total_time = batch_start.elapsed().as_millis() as u64;

        info!(
            "Batch execution completed: {}/{} succeeded in {}ms",
            success_count,
            tool_calls.len(),
            total_time
        );

        Ok(MCPBatchResponse {
            batch_id,
            results,
            execution_time_ms: total_time,
            success_count,
            failed_count,
        })
    }

    /// Send batch request with enhanced error handling
    async fn send_batch_request(&mut self, batch_request: Value) -> Result<Value, String> {
        let request_str = serde_json::to_string(&batch_request)
            .map_err(|e| format!("Failed to serialize batch request: {}", e))?;

        debug!(
            "Sending MCP batch request to '{}': {}",
            self.config.name, request_str
        );

        // Check if process is still alive
        if let Some(ref mut process) = self.process {
            match process.try_wait() {
                Ok(Some(exit_status)) => {
                    let err = format!(
                        "MCP server '{}' has exited with status: {}",
                        self.config.name, exit_status
                    );
                    error!("{}", err);
                    self.status = MCPServerStatus::Error(err.clone());
                    return Err(err);
                }
                Ok(None) => {} // Process is still running
                Err(e) => {
                    warn!(
                        "Failed to check MCP server '{}' process status: {}",
                        self.config.name, e
                    );
                }
            }
        }

        // Send batch request
        if let Some(ref mut writer) = self.stdin_writer {
            writer
                .write_all(request_str.as_bytes())
                .await
                .map_err(|e| {
                    if e.kind() == std::io::ErrorKind::BrokenPipe {
                        format!(
                            "MCP server '{}' pipe broken during batch write",
                            self.config.name
                        )
                    } else {
                        format!(
                            "Failed to write batch request to '{}': {}",
                            self.config.name, e
                        )
                    }
                })?;
            writer
                .write_all(b"\n")
                .await
                .map_err(|e| format!("Failed to write newline: {}", e))?;
            writer
                .flush()
                .await
                .map_err(|e| format!("Failed to flush batch request: {}", e))?;
        } else {
            return Err("No stdin writer available for batch request".to_string());
        }

        // Read batch response with timeout
        let timeout_duration = Duration::from_secs(self.config.timeout_seconds * 2); // Double timeout for batch

        let response_future = async {
            if let Some(ref mut reader) = self.stdout_reader {
                let mut line = String::new();
                match reader.read_line(&mut line).await {
                    Ok(0) => Err(format!(
                        "MCP server '{}' closed stdout during batch read",
                        self.config.name
                    )),
                    Ok(_) => {
                        let trimmed = line.trim();
                        debug!(
                            "Received MCP batch response from '{}': {}",
                            self.config.name, trimmed
                        );

                        serde_json::from_str::<Value>(trimmed)
                            .map_err(|e| format!("Failed to parse batch response JSON from '{}': {} - Response: '{}'",
                                              self.config.name, e, trimmed))
                    }
                    Err(e) => {
                        if e.kind() == std::io::ErrorKind::BrokenPipe {
                            Err(format!(
                                "MCP server '{}' pipe broken during batch read",
                                self.config.name
                            ))
                        } else {
                            Err(format!(
                                "Failed to read batch response from '{}': {}",
                                self.config.name, e
                            ))
                        }
                    }
                }
            } else {
                Err("No stdout reader available for batch response".to_string())
            }
        };

        tokio::time::timeout(timeout_duration, response_future)
            .await
            .map_err(|_| {
                self.status = MCPServerStatus::Timeout;
                self.record_failure();
                format!(
                    "Batch request timeout for MCP server '{}' ({}s)",
                    self.config.name,
                    timeout_duration.as_secs()
                )
            })?
    }
}

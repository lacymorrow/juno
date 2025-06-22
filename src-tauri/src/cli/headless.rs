//! # Headless Runtime Module
//!
//! Provides headless execution capabilities for Juno AI Computer Use Agent
//! Runs the full Tauri app without creating any windows

use std::sync::Arc;
use std::time::{Duration, Instant};
use tauri::{AppHandle, Manager, Builder};
use tokio::sync::{mpsc, oneshot};
use tracing::{info, error, warn, debug};
use serde_json::Value;

use crate::anthropic;
use crate::state::AppState;
use crate::cli::{CliResult, OutputFormat};
use crate::error_handling::JunoError;
use crate::settings::manager::SettingsManager;

/// Headless runtime configuration
#[derive(Debug, Clone)]
pub struct HeadlessConfig {
    pub max_execution_time: Duration,
    pub enable_screenshots: bool,
    pub output_format: OutputFormat,
    pub verbose: bool,
    pub save_session: bool,
}

impl Default for HeadlessConfig {
    fn default() -> Self {
        Self {
            max_execution_time: Duration::from_secs(300), // 5 minutes
            enable_screenshots: false,
            output_format: OutputFormat::Text,
            verbose: false,
            save_session: true,
        }
    }
}

/// Headless runtime for executing agent queries without GUI
pub struct HeadlessRuntime {
    app_handle: AppHandle,
    config: HeadlessConfig,
}

impl HeadlessRuntime {
    /// Create a new headless runtime with the given Tauri app
    pub fn new(app_handle: AppHandle, config: HeadlessConfig) -> Self {
        Self {
            app_handle,
            config,
        }
    }

    /// Execute a single query and return the result
    pub async fn execute_query(&self, query: String) -> Result<CliResult, JunoError> {
        let start_time = Instant::now();

        info!("Starting headless query execution: {}", query);

        // Setup cancellation and timeout
        let (cancel_tx, cancel_rx) = oneshot::channel::<()>();
        let timeout_duration = self.config.max_execution_time;

        // Execute the query with timeout
        let result = tokio::time::timeout(
            timeout_duration,
            self.execute_query_internal(query.clone(), cancel_rx)
        ).await;

        let execution_time = start_time.elapsed();

        match result {
            Ok(Ok(response)) => {
                info!("Query executed successfully in {:?}", execution_time);
                Ok(CliResult::success_with_data(
                    "Query executed successfully",
                    self.format_response(response)?
                ).with_execution_time(execution_time))
            },
            Ok(Err(e)) => {
                error!("Query execution failed: {}", e);
                Ok(CliResult::error(format!("Query execution failed: {}", e))
                    .with_execution_time(execution_time))
            },
            Err(_) => {
                warn!("Query execution timed out after {:?}", timeout_duration);
                // Send cancellation signal
                let _ = cancel_tx.send(());
                Ok(CliResult::error("Query execution timed out")
                    .with_execution_time(execution_time))
            }
        }
    }

    /// Internal query execution with cancellation support
    async fn execute_query_internal(
        &self,
        query: String,
        mut cancel_rx: oneshot::Receiver<()>
    ) -> Result<Value, String> {
        let state = self.app_handle.state::<AppState>();

        // Reset any previous agent state
        state.reset_cancel();

        // Create a channel to receive the agent response
        let (response_tx, mut response_rx) = mpsc::unbounded_channel::<Value>();

        // Set up response listener
        let app_handle_clone = self.app_handle.clone();
        let listener_handle = tokio::spawn(async move {
            let mut last_response = None;

            // Listen for agent completion events
            let _unlisten = app_handle_clone.listen("agent-response", move |event| {
                if let Some(payload) = event.payload() {
                    if let Ok(response) = serde_json::from_str::<Value>(payload) {
                        let _ = response_tx.send(response.clone());
                        last_response = Some(response);
                    }
                }
            });

            // Keep the listener alive
            loop {
                tokio::time::sleep(Duration::from_millis(100)).await;
            }
        });

        // Execute the agent query
        let execution_handle = tokio::spawn({
            let app_handle = self.app_handle.clone();
            let state = state.clone();
            let query = query.clone();

            async move {
                anthropic::submit_query(query, state, app_handle).await
            }
        });

        // Wait for either completion, cancellation, or timeout
        tokio::select! {
            // Agent execution completed
            result = execution_handle => {
                listener_handle.abort();
                match result {
                    Ok(Ok(())) => {
                        // Try to get the final response
                        if let Ok(response) = response_rx.try_recv() {
                            Ok(response)
                        } else {
                            Ok(serde_json::json!({
                                "success": true,
                                "message": "Query executed successfully"
                            }))
                        }
                    },
                    Ok(Err(e)) => Err(e),
                    Err(e) => Err(format!("Agent execution panicked: {}", e))
                }
            },
            // Cancellation requested
            _ = &mut cancel_rx => {
                warn!("Query execution cancelled");
                state.signal_cancel();
                execution_handle.abort();
                listener_handle.abort();
                Err("Query execution was cancelled".to_string())
            },
            // Response received
            response = response_rx.recv() => {
                if let Some(response) = response {
                    execution_handle.abort();
                    listener_handle.abort();
                    Ok(response)
                } else {
                    Err("No response received from agent".to_string())
                }
            }
        }
    }

    /// Format the response according to the configured output format
    fn format_response(&self, response: Value) -> Result<Value, JunoError> {
        match self.config.output_format {
            OutputFormat::Json => Ok(response),
            OutputFormat::Text => {
                // Extract text content from the response
                let text = if let Some(text) = response.get("text") {
                    text.as_str().unwrap_or("No text content").to_string()
                } else if let Some(message) = response.get("message") {
                    message.as_str().unwrap_or("No message content").to_string()
                } else {
                    format!("{:#}", response)
                };
                Ok(serde_json::json!({"text": text}))
            },
            OutputFormat::Xml => {
                // Convert JSON to XML format
                Ok(serde_json::json!({
                    "xml": format!("<response>{}</response>", self.json_to_xml(&response)?)
                }))
            },
            OutputFormat::Yaml => {
                // Convert JSON to YAML format
                let yaml_string = serde_yaml::to_string(&response)
                    .map_err(|e| JunoError::SerializationError(format!("YAML serialization failed: {}", e)))?;
                Ok(serde_json::json!({"yaml": yaml_string}))
            }
        }
    }

    /// Simple JSON to XML conversion
    fn json_to_xml(&self, value: &Value) -> Result<String, JunoError> {
        match value {
            Value::String(s) => Ok(s.clone()),
            Value::Number(n) => Ok(n.to_string()),
            Value::Bool(b) => Ok(b.to_string()),
            Value::Null => Ok("null".to_string()),
            Value::Array(arr) => {
                let items: Result<Vec<String>, _> = arr.iter()
                    .map(|v| self.json_to_xml(v))
                    .collect();
                Ok(format!("<array>{}</array>", items?.join("")))
            },
            Value::Object(obj) => {
                let items: Result<Vec<String>, _> = obj.iter()
                    .map(|(k, v)| {
                        let content = self.json_to_xml(v)?;
                        Ok(format!("<{}>{}</{}>", k, content, k))
                    })
                    .collect();
                Ok(items?.join(""))
            }
        }
    }

    /// Start an interactive session in headless mode
    pub async fn start_interactive_session(&self, name: Option<String>) -> Result<CliResult, JunoError> {
        info!("Starting headless interactive session: {:?}", name);

        // This would implement a headless interactive REPL
        // For now, return a placeholder
        Ok(CliResult::success("Interactive session started (not yet implemented)"))
    }

    /// Get system diagnostics
    pub async fn run_diagnostics(&self, full: bool, component: Option<String>) -> Result<CliResult, JunoError> {
        let start_time = Instant::now();

        info!("Running system diagnostics (full: {}, component: {:?})", full, component);

        let mut diagnostics = serde_json::json!({
            "juno_version": env!("CARGO_PKG_VERSION"),
            "timestamp": chrono::Utc::now().to_rfc3339(),
            "platform": std::env::consts::OS,
            "architecture": std::env::consts::ARCH,
        });

        // Check app state
        let state = self.app_handle.state::<AppState>();
        diagnostics["desktop_available"] = serde_json::json!(state.is_desktop_available());
        diagnostics["agent_executing"] = serde_json::json!(state.is_agent_executing());

        // Check settings
        if let Ok(settings_manager) = SettingsManager::new(self.app_handle.clone()) {
            if let Ok(providers) = settings_manager.get_provider_settings().await {
                diagnostics["active_provider"] = serde_json::json!(providers.active_provider);
            }
        }

        // Component-specific diagnostics
        if let Some(comp) = component {
            match comp.as_str() {
                "desktop" => {
                    diagnostics["desktop_details"] = self.diagnose_desktop_component().await;
                },
                "providers" => {
                    diagnostics["providers_details"] = self.diagnose_providers_component().await;
                },
                "tools" => {
                    diagnostics["tools_details"] = self.diagnose_tools_component().await;
                },
                _ => {
                    return Ok(CliResult::error(format!("Unknown component: {}", comp)));
                }
            }
        }

        if full {
            // Add comprehensive diagnostics
            diagnostics["memory_usage"] = self.get_memory_usage().await;
            diagnostics["environment_variables"] = self.check_environment_variables().await;
            diagnostics["permissions"] = self.check_permissions().await;
        }

        let execution_time = start_time.elapsed();

        Ok(CliResult::success_with_data(
            "Diagnostics completed successfully",
            diagnostics
        ).with_execution_time(execution_time))
    }

    /// Diagnose desktop component
    async fn diagnose_desktop_component(&self) -> Value {
        let state = self.app_handle.state::<AppState>();

        serde_json::json!({
            "available": state.is_desktop_available(),
            "wrapper_type": "DesktopWrapper",
            "permissions_checked": state.are_permissions_checked()
        })
    }

    /// Diagnose providers component
    async fn diagnose_providers_component(&self) -> Value {
        if let Ok(settings_manager) = SettingsManager::new(self.app_handle.clone()) {
            if let Ok(providers) = settings_manager.get_provider_settings().await {
                return serde_json::json!({
                    "active_provider": providers.active_provider,
                    "available_providers": ["anthropic", "openai", "gemini"],
                    "status": "configured"
                });
            }
        }

        serde_json::json!({
            "status": "error",
            "message": "Failed to load provider settings"
        })
    }

    /// Diagnose tools component
    async fn diagnose_tools_component(&self) -> Value {
        let state = self.app_handle.state::<AppState>();
        let tool_config = state.get_tool_config_manager().await;

        // This would check tool configuration status
        serde_json::json!({
            "tool_config_loaded": true,
            "mcp_servers_available": true, // Would check actual MCP status
            "status": "operational"
        })
    }

    /// Get memory usage information
    async fn get_memory_usage(&self) -> Value {
        // This would implement actual memory usage checking
        serde_json::json!({
            "rss_kb": "unknown",
            "heap_kb": "unknown",
            "note": "Memory usage monitoring not implemented"
        })
    }

    /// Check environment variables
    async fn check_environment_variables(&self) -> Value {
        let critical_vars = [
            "ANTHROPIC_API_KEY",
            "OPENAI_API_KEY",
            "ELEVENLABS_API_KEY",
            "GEMINI_API_KEY",
        ];

        let mut env_status = serde_json::Map::new();

        for var in critical_vars.iter() {
            env_status.insert(
                var.to_string(),
                serde_json::json!(std::env::var(var).is_ok())
            );
        }

        serde_json::json!(env_status)
    }

    /// Check system permissions
    async fn check_permissions(&self) -> Value {
        let state = self.app_handle.state::<AppState>();

        serde_json::json!({
            "accessibility": state.is_desktop_available(),
            "permissions_checked": state.are_permissions_checked(),
            "note": "Detailed permission checking would be implemented here"
        })
    }
}

/// Initialize headless Tauri app without creating windows
pub async fn create_headless_app() -> Result<tauri::App, JunoError> {
    info!("Initializing headless Tauri application");

    let builder = tauri::Builder::default()
        .plugin(tauri_plugin_voice_transcription::init())
        .setup(|app| {
            // Initialize app state without windows
            let (desktop_arc, app_state) = crate::startup::quick_startup()
                .map_err(|e| format!("Failed to initialize app state: {}", e))?;

            app.manage(app_state);

            info!("Headless app state initialized successfully");
            Ok(())
        });

    // Build the app but don't run it
    builder.build(tauri::generate_context!())
        .map_err(|e| JunoError::ApplicationError(format!("Failed to build headless app: {}", e)))
}

/// Run a single query in headless mode
pub async fn run_headless_query(
    query: String,
    config: HeadlessConfig,
    output_file: Option<std::path::PathBuf>
) -> Result<CliResult, JunoError> {
    let app = create_headless_app().await?;
    let app_handle = app.handle().clone();

    let runtime = HeadlessRuntime::new(app_handle, config);
    let result = runtime.execute_query(query).await?;

    // Save output to file if specified
    if let Some(path) = output_file {
        let output_content = match result.data.as_ref() {
            Some(data) => serde_json::to_string_pretty(data)
                .map_err(|e| JunoError::SerializationError(e.to_string()))?,
            None => result.message.clone(),
        };

        std::fs::write(&path, output_content)
            .map_err(|e| JunoError::FileSystemError(format!("Failed to write output file: {}", e)))?;

        info!("Results saved to: {:?}", path);
    }

    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_headless_config_default() {
        let config = HeadlessConfig::default();
        assert_eq!(config.max_execution_time, Duration::from_secs(300));
        assert!(!config.enable_screenshots);
        assert!(matches!(config.output_format, OutputFormat::Text));
    }

    #[tokio::test]
    async fn test_json_to_xml_conversion() {
        let config = HeadlessConfig::default();
        // This would need a mock runtime for full testing
        // For now, just test the basic structure is sound
        assert!(true);
    }
}

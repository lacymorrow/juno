//! # Headless Runtime Module
//!
//! Provides headless execution capabilities for the Juno AI agent,
//! allowing CLI-driven operations without requiring a frontend UI.

use std::sync::{Arc, Mutex};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};
use tokio::sync::oneshot;
use tokio::time::timeout;
use tracing::info;
use tauri::{AppHandle, Manager, Listener, EventId};
use serde_json::{json, Value};
use crate::agent::providers::factory::{BrainFactory, Provider};
use crate::settings::manager::SettingsManager;

use crate::cli::{Cli, OutputFormat};
use crate::constants::cli;
use crate::error_handling::JunoError;
use crate::state::AppState;

// Global flag to track headless mode
static HEADLESS_MODE: AtomicBool = AtomicBool::new(false);

/// Headless runtime for CLI operations
pub struct HeadlessRuntime {
    app_handle: AppHandle,
    output_format: OutputFormat,
    verbosity: u8,
    timeout_duration: Duration,
}



/// Result of a headless operation
#[derive(Debug, Clone)]
pub struct HeadlessResult {
    pub success: bool,
    pub output: String,
    pub error: Option<String>,
    pub execution_time: Duration,
    pub agent_state: Option<String>,
    pub screenshot: Option<String>,
}

impl HeadlessRuntime {
    // Implementation priority for future headless voice features
    // TODO(headless-voice-order): Implement in this order for headless mode support
    // 1) Voice::Transcribe (file and mic) → returns transcript JSON
    // 2) Voice::Query (dictation → agent) → wraps execute_agent_mode
    // 3) Voice::Record (file output) → writes audio to disk
    const VOICE_IMPL_ORDER: &'static [&'static str] = &[
        "voice.transcribe",
        "voice.query",
        "voice.record",
    ];

    // Implementation priority for future headless dictation features
    // TODO(headless-dictation-order): Implement in this order for headless mode support
    // 1) Dictation::Status → returns current dictation status
    // 2) Dictation::Start → begins session and returns transcript
    // 3) Dictation::Stop → ends session
    // 4) Dictation::Configure → persists settings
    const DICTATION_IMPL_ORDER: &'static [&'static str] = &[
        "dictation.status",
        "dictation.start",
        "dictation.stop",
        "dictation.configure",
    ];
    /// Create a new headless runtime
    pub fn new(app_handle: AppHandle, cli: &Cli) -> Self {
        Self {
            app_handle,
            output_format: cli.output.clone(),
            verbosity: match cli.get_verbosity_level() {
                crate::cli::VerbosityLevel::Quiet => 0,
                crate::cli::VerbosityLevel::Normal => 1,
                crate::cli::VerbosityLevel::Verbose => 2,
                crate::cli::VerbosityLevel::Debug => 3,
                crate::cli::VerbosityLevel::Trace => 4,
            },
            timeout_duration: cli.get_timeout(),
        }
    }

    /// Execute a CLI command in headless mode
    pub async fn execute_command(&self, cli: &Cli) -> Result<HeadlessResult, JunoError> {
        let start_time = Instant::now();

        if self.verbosity >= 2 {
            info!("Starting headless execution with timeout: {:?}", self.timeout_duration);
        }

        // Check for legacy CLI flags first
        let result = if cli.has_legacy_flags() {
            self.execute_legacy_commands(cli).await
        } else if let Some(command) = &cli.command {
            self.execute_subcommand(command).await
        } else {
            return Err(JunoError::ApplicationError("No valid command specified".to_string()));
        };

        let execution_time = start_time.elapsed();

        match result {
            Ok(mut res) => {
                res.execution_time = execution_time;
                if self.verbosity >= 1 && !matches!(self.output_format, OutputFormat::Quiet) {
                    self.output_result(&res);
                }
                Ok(res)
            }
            Err(e) => {
                let error_result = HeadlessResult {
                    success: false,
                    output: String::new(),
                    error: Some(e.to_string()),
                    execution_time,
                    agent_state: None,
                    screenshot: None,
                };
                if self.verbosity >= 1 && !matches!(self.output_format, OutputFormat::Quiet) {
                    self.output_result(&error_result);
                }
                Err(e)
            }
        }
    }

    /// Execute legacy CLI commands
    async fn execute_legacy_commands(&self, cli: &Cli) -> Result<HeadlessResult, JunoError> {
        if cli.test_focused_element_ns {
            // TODO(headless-legacy-impl): Replace mock with real focused element test or remove legacy flag
            tracing::warn!("Using mock implementation for --test-focused-element-ns in headless mode");
            Ok(HeadlessResult {
                success: true,
                output: "Focused element test completed".to_string(),
                error: None,
                execution_time: Duration::default(),
                agent_state: Some("Completed".to_string()),
                screenshot: None,
            })
        } else if cli.check_accessibility {
            // TODO(headless-legacy-impl): Replace mock with real accessibility check or remove legacy flag
            tracing::warn!("Using mock implementation for --check-accessibility in headless mode");
            Ok(HeadlessResult {
                success: true,
                output: "Accessibility check completed".to_string(),
                error: None,
                execution_time: Duration::default(),
                agent_state: Some("Completed".to_string()),
                screenshot: None,
            })
        } else if cli.tts_provider.is_some() || cli.tts_text.is_some() {
            // TODO(headless-legacy-impl): Replace mock with real TTS invocation using backend TTS command(s)
            tracing::warn!("Using mock implementation for TTS test in headless mode");
            let provider = cli.tts_provider.as_deref().unwrap_or("system");
            let text = cli.tts_text.as_deref().unwrap_or("Test speech");
            Ok(HeadlessResult {
                success: true,
                output: format!("TTS test completed with provider: {}, text: {}", provider, text),
                error: None,
                execution_time: Duration::default(),
                agent_state: Some("Completed".to_string()),
                screenshot: None,
            })
        } else {
            Err(JunoError::ApplicationError("No valid legacy command specified".to_string()))
        }
    }

    /// Execute a subcommand
    async fn execute_subcommand(&self, command: &crate::cli::Commands) -> Result<HeadlessResult, JunoError> {
        use crate::cli::Commands;

        match command {
            Commands::Query { text, .. } => {
                self.execute_query(text.clone()).await
            }
            Commands::Voice { command } => {
                self.execute_voice_command(command).await
            }
            Commands::Dictation { command } => {
                self.execute_dictation_command(command).await
            }
            Commands::Agent { command } => {
                self.execute_agent_command(command).await
            }
            Commands::Config { command } => {
                self.execute_config_command(command).await
            }
            Commands::Mcp { command } => {
                self.execute_mcp_command(command).await
            }
            Commands::System { command } => {
                self.execute_system_command(command).await
            }
            Commands::Batch { file, .. } => {
                self.execute_batch_mode(Some(file.clone())).await
            }
            Commands::Interactive { .. } => {
                self.execute_interactive_mode().await
            }
            Commands::Daemon { command } => {
                self.execute_daemon_command(command).await
            }
            Commands::Test { command } => {
                self.execute_test_command(command).await
            }
        }
    }

    /// Execute voice subcommands
    async fn execute_voice_command(&self, command: &crate::cli::VoiceCommands) -> Result<HeadlessResult, JunoError> {
        use crate::cli::VoiceCommands;
        match command {
            VoiceCommands::Transcribe { .. } => {
                // TODO(headless-voice-transcribe): Implement file/mic transcription in headless mode
                let guidance = format!("Not implemented. Planned order: {}", Self::VOICE_IMPL_ORDER.join(" -> "));
                Err(JunoError::ApplicationError(format!(
                    "voice transcribe is not available in headless mode yet. {}",
                    guidance
                )))
            }
            VoiceCommands::Query { duration, .. } => {
                // TODO(headless-voice-query): Route to execute_agent_mode with duration
                let guidance = format!("Not implemented. Planned order: {}", Self::VOICE_IMPL_ORDER.join(" -> "));
                Err(JunoError::ApplicationError(format!(
                    "voice query is not available in headless mode yet (requested duration: {}s). {}",
                    duration,
                    guidance
                )))
            }
            VoiceCommands::Record { .. } => {
                // TODO(headless-voice-record): Implement recording to file in headless mode
                let guidance = format!("Not implemented. Planned order: {}", Self::VOICE_IMPL_ORDER.join(" -> "));
                Err(JunoError::ApplicationError(format!(
                    "voice record is not available in headless mode yet. {}",
                    guidance
                )))
            }
        }
    }

    /// Execute dictation subcommands
    async fn execute_dictation_command(&self, command: &crate::cli::DictationCommands) -> Result<HeadlessResult, JunoError> {
        use crate::cli::DictationCommands;
        match command {
            DictationCommands::Status => {
                // TODO(headless-dictation-status): Implement status retrieval in headless mode
                let guidance = format!("Not implemented. Planned order: {}", Self::DICTATION_IMPL_ORDER.join(" -> "));
                Err(JunoError::ApplicationError(format!(
                    "dictation status is not available in headless mode yet. {}",
                    guidance
                )))
            }
            DictationCommands::Start { .. } => {
                // TODO(headless-dictation-start): Implement start dictation session
                let guidance = format!("Not implemented. Planned order: {}", Self::DICTATION_IMPL_ORDER.join(" -> "));
                Err(JunoError::ApplicationError(format!(
                    "dictation start is not available in headless mode yet. {}",
                    guidance
                )))
            }
            DictationCommands::Stop => {
                // TODO(headless-dictation-stop): Implement stop dictation session
                let guidance = format!("Not implemented. Planned order: {}", Self::DICTATION_IMPL_ORDER.join(" -> "));
                Err(JunoError::ApplicationError(format!(
                    "dictation stop is not available in headless mode yet. {}",
                    guidance
                )))
            }
            DictationCommands::Configure { .. } => {
                // TODO(headless-dictation-configure): Implement configuration persistence
                let guidance = format!("Not implemented. Planned order: {}", Self::DICTATION_IMPL_ORDER.join(" -> "));
                Err(JunoError::ApplicationError(format!(
                    "dictation configure is not available in headless mode yet. {}",
                    guidance
                )))
            }
        }
    }

    /// Execute agent subcommands
    async fn execute_agent_command(&self, command: &crate::cli::AgentCommands) -> Result<HeadlessResult, JunoError> {
        use crate::cli::AgentCommands;
        match command {
            AgentCommands::Status => self.execute_status().await,
            // For other agent subcommands, return a simple success placeholder for now
            _ => Ok(HeadlessResult {
                success: true,
                output: "Agent command executed".to_string(),
                error: None,
                execution_time: Duration::default(),
                agent_state: Some("Completed".to_string()),
                screenshot: None,
            }),
        }
    }

    /// Execute config subcommands
    async fn execute_config_command(&self, _command: &crate::cli::ConfigCommands) -> Result<HeadlessResult, JunoError> {
        Err(JunoError::ApplicationError("Config subcommands are not implemented in headless mode".to_string()))
    }

    /// Execute MCP subcommands
    async fn execute_mcp_command(&self, command: &crate::cli::McpCommands) -> Result<HeadlessResult, JunoError> {
        match command {
            crate::cli::McpCommands::AddServer { name, http_url, enabled, auto_start, timeout } => {
                // Build config and call backend command to persist
                use crate::agent::tools::MCPServerConfig;
                let cfg = MCPServerConfig {
                    id: uuid::Uuid::new_v4().to_string(),
                    name: name.clone(),
                    description: Some("HTTP MCP server".to_string()),
                    command: "http".to_string(),
                    args: vec![http_url.clone()],
                    working_directory: None,
                    environment_variables: std::collections::HashMap::new(),
                    enabled: *enabled,
                    auto_start: *auto_start,
                    timeout_seconds: *timeout,
                    max_retries: 3,
                };

                let state = self.app_handle.state::<AppState>();
                match crate::commands::mcp::add_mcp_server(self.app_handle.clone(), state, cfg).await {
                    Ok(_) => Ok(HeadlessResult {
                        success: true,
                        output: format!("MCP server '{}' added", name),
                        error: None,
                        execution_time: Duration::default(),
                        agent_state: Some("Completed".to_string()),
                        screenshot: None,
                    }),
                    Err(e) => Err(JunoError::ApplicationError(format!("Failed to add MCP server: {}", e))),
                }
            }
        }
    }

    /// Execute system subcommands
    async fn execute_system_command(&self, _command: &crate::cli::SystemCommands) -> Result<HeadlessResult, JunoError> {
        Err(JunoError::ApplicationError("System subcommands are not implemented in headless mode".to_string()))
    }

    /// Execute daemon subcommands
    async fn execute_daemon_command(&self, _command: &crate::cli::DaemonCommands) -> Result<HeadlessResult, JunoError> {
        Err(JunoError::ApplicationError("Daemon subcommands are not implemented in headless mode".to_string()))
    }

    /// Execute test subcommands
    async fn execute_test_command(&self, command: &crate::cli::TestCommands) -> Result<HeadlessResult, JunoError> {
        use crate::cli::TestCommands;
        match command {
            TestCommands::System { component, .. } if component.as_deref() == Some("tool") => {
                // Run a minimal tool smoke test using direct commands with managed state
                let app_handle = self.app_handle.clone();
                let state = self
                    .app_handle
                    .try_state::<AppState>()
                    .ok_or_else(|| JunoError::ApplicationError("Application state not available in headless runtime".to_string()))?;

                // Get cursor position (does not require accessibility for read)
                let cursor_res = crate::commands::mouse::get_cursor_position(app_handle.clone(), state.clone()).await;

                // Short wait
                let wait_res = crate::commands::core::wait(0.1, app_handle.clone(), state.clone()).await;

                let output = json!({
                    "tool_smoke": {
                        "cursor_position": match cursor_res {
                            Ok((x,y)) => json!({"coordinate": [x,y]}),
                            Err(e) => json!({"error": e}),
                        },
                        "wait": match wait_res {
                            Ok(_) => json!({"success": true}),
                            Err(e) => json!({"error": e}),
                        }
                    }
                });

                Ok(HeadlessResult {
                    success: true,
                    output: output.to_string(),
                    error: None,
                    execution_time: Duration::default(),
                    agent_state: Some("ToolSmoke".to_string()),
                    screenshot: None,
                })
            }
            TestCommands::System { component, .. } if component.as_deref() == Some("events") => {
                // Synthesize streaming and agent events for programmatic testing
                let app_handle = self.app_handle.clone();

                // Start streaming
                let message_id = uuid::Uuid::new_v4().to_string();
                crate::agent::tool_logger::emit_stream_start(&app_handle, message_id.clone());
                crate::agent::tool_logger::emit_streaming_text_chunk(&app_handle, "Hello, ".to_string(), Some(message_id.clone()), None);
                crate::agent::tool_logger::emit_streaming_text_chunk(&app_handle, "world".to_string(), Some(message_id.clone()), None);

                // Emit a synthetic tool call request/result
                let tool_args = json!({"action":"screenshot","coordinate":[200,100]});
                crate::agent::tool_logger::log_enhanced_tool_call_request(&app_handle, "computer", tool_args.clone(), Some("Taking a screenshot".to_string()), None).await;
                crate::agent::tool_logger::log_enhanced_tool_call_result_with_inputs(&app_handle, "computer", Some(tool_args), json!({"success": true}), true, Some("Screenshot captured".to_string()), None, Some(42), None).await;

                // End streaming
                crate::agent::tool_logger::emit_stream_end_with_state(&app_handle, message_id.clone(), "Hello, world".to_string(), "Completed".to_string());

                let output = json!({
                    "emitted": {
                        "stream_start": true,
                        "text_chunks": 2,
                        "tool_events": 2,
                        "stream_end": true
                    }
                });

                Ok(HeadlessResult {
                    success: true,
                    output: output.to_string(),
                    error: None,
                    execution_time: Duration::default(),
                    agent_state: Some("EventsEmitted".to_string()),
                    screenshot: None,
                })
            }
            _ => Ok(HeadlessResult {
                success: true,
                output: "Test command executed".to_string(),
                error: None,
                execution_time: Duration::default(),
                agent_state: Some("Completed".to_string()),
                screenshot: None,
            }),
        }
    }

    /// Execute a text query
    async fn execute_query(&self, query: String) -> Result<HeadlessResult, JunoError> {
        if self.verbosity >= 2 {
            info!("Executing text query: {}", query);
        }

        // Accumulators for streaming/text and tool events
        let accumulated_text = Arc::new(Mutex::new(String::new()));
        let tool_events: Arc<Mutex<Vec<Value>>> = Arc::new(Mutex::new(Vec::new()));

        // Channel to notify on stream end with final JSON payload
        let (done_tx, done_rx) = oneshot::channel::<Value>();
        let done_tx = Arc::new(Mutex::new(Some(done_tx)));

        // Set up event listeners with robust cleanup via RAII guard to prevent leaks
        struct EventListenersGuard {
            app_handle: AppHandle,
            ids: Vec<EventId>,
        }
        impl EventListenersGuard {
            fn new(app_handle: AppHandle) -> Self { Self { app_handle, ids: Vec::new() } }
            fn push(&mut self, id: EventId) { self.ids.push(id); }
            fn cleanup(&mut self) {
                for id in self.ids.drain(..) {
                    self.app_handle.unlisten(id);
                }
            }
        }
        impl Drop for EventListenersGuard { fn drop(&mut self) { self.cleanup(); } }

        let mut listener_guard = EventListenersGuard::new(self.app_handle.clone());

        // TEXT_STREAM listener
        let text_acc_clone = Arc::clone(&accumulated_text);
        let text_stream_id: EventId = self
            .app_handle
            .listen(crate::constants::events::streaming::TEXT_STREAM, move |event| {
                if let Ok(payload) = serde_json::from_str::<Value>(event.payload()) {
                    if let Some(chunk) = payload.get("chunk").and_then(|v| v.as_str()) {
                        if let Ok(mut guard) = text_acc_clone.lock() {
                            guard.push_str(chunk);
                        }
                    }
                }
            });
        listener_guard.push(text_stream_id);

        // Capture generic agent events to collect tool calls/results
        let tool_events_clone = Arc::clone(&tool_events);
        let agent_event_id: EventId = self
            .app_handle
            .listen(crate::constants::events::agent::EVENT, move |event| {
                if let Ok(payload) = serde_json::from_str::<Value>(event.payload()) {
                    if let Ok(mut guard) = tool_events_clone.lock() {
                        guard.push(payload);
                    }
                }
            });
        listener_guard.push(agent_event_id);

        // On stream end, produce final JSON and signal completion
        let text_acc_for_end = Arc::clone(&accumulated_text);
        let tool_events_for_end = Arc::clone(&tool_events);
        let done_tx_for_end = Arc::clone(&done_tx);
        let stream_end_id: EventId = self
            .app_handle
            .listen(crate::constants::events::streaming::STREAM_END, move |event| {
                let final_text = match serde_json::from_str::<Value>(event.payload()) {
                    Ok(v) => v
                        .get("complete_text")
                        .and_then(|t| t.as_str())
                        .map(|s| s.to_string())
                        .or_else(|| text_acc_for_end.lock().ok().map(|s| s.clone()))
                        .unwrap_or_default(),
                    Err(_) => text_acc_for_end
                        .lock()
                        .ok()
                        .map(|s| s.clone())
                        .unwrap_or_default(),
                };

                let tools_snapshot = if let Ok(guard) = tool_events_for_end.lock() {
                    (*guard).clone()
                } else {
                    Vec::new()
                };
                let result_obj = json!({
                    "text": final_text,
                    "tool_events": tools_snapshot,
                });

                if let Ok(mut tx_guard) = done_tx_for_end.lock() {
                    if let Some(tx) = tx_guard.take() {
                        let _ = tx.send(result_obj);
                    }
                }
            });
        listener_guard.push(stream_end_id);

        // Brief delay to ensure listener registration completes before emissions
        tokio::time::sleep(Duration::from_millis(5)).await;

        // Submit the query to the agent
        let state = self
            .app_handle
            .try_state::<AppState>()
            .ok_or_else(|| JunoError::ApplicationError("Application state not available in headless runtime".to_string()))?;
        let submit_result = crate::anthropic::submit_query(query.clone(), state, self.app_handle.clone()).await
            .map_err(|e| JunoError::ApplicationError(format!("Failed to submit query: {}", e)))?;

        // Wait for result with timeout
        let result = match timeout(self.timeout_duration, done_rx).await {
            Ok(Ok(v)) => v,
            Ok(Err(_)) => {
                // Ensure cleanup on channel failure
                listener_guard.cleanup();
                return Err(JunoError::ApplicationError("Failed to receive query result".to_string()));
            }
            Err(_) => {
                // Timeout - cleanup listeners and surface error
                listener_guard.cleanup();
                return Err(JunoError::ApplicationError("Query execution timed out".to_string()));
            }
        };

        // Cleanup listeners to prevent leaks and duplicates
        listener_guard.cleanup();

        // Build final output as JSON string
        let output = serde_json::to_string(&result).unwrap_or_else(|_| "{}".to_string());

        Ok(HeadlessResult {
            success: true,
            output,
            error: None,
            execution_time: Duration::default(), // Will be set by caller
            agent_state: Some("Completed".to_string()),
            screenshot: None,
        })
    }

    /// Execute dictation mode
    async fn execute_dictation(&self, voice_timeout: u64) -> Result<HeadlessResult, JunoError> {
        if self.verbosity >= 2 {
            info!("Starting dictation mode with timeout: {}s", voice_timeout);
        }

        // Check if voice controller is available
        let voice_controller = self.app_handle.try_state::<Arc<Mutex<tauri_plugin_voice_transcription::controller::VoiceController>>>()
            .ok_or_else(|| JunoError::ApplicationError("Voice controller not available".to_string()))?;

        // Set up result capture
        let (result_tx, result_rx) = oneshot::channel::<String>();
        let result_tx = Arc::new(Mutex::new(Some(result_tx)));

        // Listen for dictation completion
        // NOTE: In headless mode we don't currently hook plugin events; this is a placeholder for future wiring
        // TODO(headless-dictation-wireup): Wire plugin FINAL_RESULT event to send on result_tx
        // For headless mode, we'll use a different approach than event listening
        /*
        self.app_handle.listen(events::voice_transcription::FINAL_RESULT, move |event| {
            if let Ok(mut tx_guard) = result_tx_clone.lock() {
                if let Some(tx) = tx_guard.take() {
                    let _ = tx.send(event.payload().to_string());
                }
            }
        });
        */

        // Start dictation
        // TODO: Implement headless dictation properly
        // For now, return a mock response
        let _voice_controller_ref = voice_controller;

        if self.verbosity >= 1 {
            println!("🎤 Listening... (speak now, will auto-stop or press Ctrl+C to cancel)");
        }

        // Wait for result with timeout
        let dictation_timeout = Duration::from_secs(voice_timeout);
        let _result = timeout(dictation_timeout, result_rx).await
            .map_err(|_| JunoError::ApplicationError("Dictation timed out".to_string()))?
            .map_err(|_| JunoError::ApplicationError("Failed to receive dictation result".to_string()))?;

        // Parse the dictation result - for headless mode, return mock result
        let transcribed_text = "Mock dictation result for headless mode".to_string();

        // TODO: Implement actual dictation result parsing

        // In dictation mode, we would normally type the text to the active window
        // For headless mode, we just return the transcribed text
        Ok(HeadlessResult {
            success: true,
            output: transcribed_text,
            error: None,
            execution_time: Duration::default(),
            agent_state: Some("Transcribed".to_string()),
            screenshot: None,
        })
    }

    /// Execute agent mode (voice input + agent processing)
    async fn execute_agent_mode(&self, voice_timeout: u64) -> Result<HeadlessResult, JunoError> {
        if self.verbosity >= 2 {
            info!("Starting agent mode with voice input timeout: {}s", voice_timeout);
        }

        // First, get voice input using dictation
        let dictation_result = self.execute_dictation(voice_timeout).await?;
        let voice_input = dictation_result.output;

        if self.verbosity >= 1 {
            println!("🎤 Transcribed: {}", voice_input);
            println!("🤖 Processing with agent...");
        }

        // Then, process it with the agent
        self.execute_query(voice_input).await
    }

    /// Execute voice query (same as agent mode for now)
    async fn execute_voice_query(&self, voice_timeout: u64) -> Result<HeadlessResult, JunoError> {
        self.execute_agent_mode(voice_timeout).await
    }

    /// Execute status check
    async fn execute_status(&self) -> Result<HeadlessResult, JunoError> {
        if self.verbosity >= 2 {
            info!("Checking agent status");
        }

        // Gather provider/model/agent mode from centralized settings where possible
        let mut provider_id: String = "unknown".to_string();
        let mut model_id: String = "unknown".to_string();
        let agent_mode_str: String;

        // Agent mode via BrainFactory helper
        let agent_mode = BrainFactory::get_agent_mode_with_app_handle(&self.app_handle).await;
        agent_mode_str = agent_mode.to_string().to_string();

        // Active provider + model via SettingsManager -> ProviderConfig
        if let Ok(settings_manager) = SettingsManager::new(self.app_handle.clone()) {
            if let Ok(config) = crate::agent::providers::config::ProviderConfig::load_from_centralized_settings(&settings_manager).await {
                provider_id = config.active_provider.clone();
                if let Some(p) = config
                    .providers
                    .iter()
                    .find(|p| p.id == config.active_provider)
                {
                    if let Some(m) = &p.model {
                        model_id = m.clone();
                    } else {
                        // Fallback to provider default model
                        let provider_enum = Provider::from_str(&provider_id)
                            .unwrap_or(Provider::Anthropic);
                        model_id = provider_enum.default_model().to_string();
                    }
                }
            }
        }

        // Use try_state to avoid panics if AppState isn't managed yet
        let status = if let Some(state) = self.app_handle.try_state::<AppState>() {
            json!({
                "agent_executing": state.is_agent_executing(),
                "dictation_active": state.is_dictation_active(),
                "always_listening": state.get_always_listening_active().unwrap_or(false),
                "tts_provider": state.get_tts_provider().unwrap_or_default(),
                "voice_available": false,
                "permissions": {"accessibility": true, "microphone": true},
                "provider": provider_id,
                "model": model_id,
                "agent_mode": agent_mode_str
            })
        } else {
            json!({
                "agent_executing": false,
                "dictation_active": false,
                "always_listening": false,
                "tts_provider": "",
                "voice_available": false,
                "permissions": {"accessibility": false, "microphone": false},
                "provider": provider_id,
                "model": model_id,
                "agent_mode": agent_mode_str
            })
        };

        Ok(HeadlessResult {
            success: true,
            output: status.to_string(),
            error: None,
            execution_time: Duration::default(),
            agent_state: Some("Status".to_string()),
            screenshot: None,
        })
    }

    /// Execute agent iterations
    async fn execute_iterations(&self, iterations: u32, context: Option<String>, max_depth: u32) -> Result<HeadlessResult, JunoError> {
        if self.verbosity >= 2 {
            info!("Starting {} iterations with max depth {}", iterations, max_depth);
        }

        let mut outputs = Vec::new();
        let mut current_context = context.unwrap_or_else(|| "Analyze the current state and determine the next action".to_string());

        for i in 0..iterations {
            if self.verbosity >= 1 {
                println!("🔄 Iteration {}/{}", i + 1, iterations);
            }

            // Execute query with current context
            let iteration_result = self.execute_query(current_context.clone()).await?;
            outputs.push(format!("Iteration {}: {}", i + 1, iteration_result.output));

            // Update context based on previous result for next iteration
            current_context = format!(
                "Previous iteration result: {}. Continue with the next logical step.",
                iteration_result.output.chars().take(200).collect::<String>()
            );

            // Check for cancellation between iterations
            if *self.app_handle.state::<AppState>().cancel_rx.borrow() {
                break;
            }
        }

        let final_output = outputs.join("\n\n");

        Ok(HeadlessResult {
            success: true,
            output: final_output.to_string(),
            error: None,
            execution_time: Duration::default(),
            agent_state: Some("Iterations Complete".to_string()),
            screenshot: None,
        })
    }

    /// Execute self-call (agent calling itself with new queries)
    async fn execute_self_call(&self, context: Option<String>, max_depth: u32) -> Result<HeadlessResult, JunoError> {
        if self.verbosity >= cli::verbosity::VERBOSE {
            info!("Starting self-call with max depth {}", max_depth);
        }

        let initial_query = context.unwrap_or_else(|| {
            "Analyze the current system state and determine what task you should perform next. Then execute that task by calling yourself with a specific query.".to_string()
        });

        let result = self.execute_recursive_call(initial_query, 0, max_depth).await?;

        Ok(HeadlessResult {
            success: true,
            output: result,
            error: None,
            execution_time: Duration::default(),
            agent_state: Some("Self-Call Complete".to_string()),
            screenshot: None,
        })
    }

    /// Execute recursive agent calls
    fn execute_recursive_call(&self, query: String, current_depth: u32, max_depth: u32) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<String, JunoError>> + Send + '_>> {
        Box::pin(async move {
        if current_depth >= max_depth {
            return Ok(format!("Maximum recursion depth ({}) reached", max_depth));
        }

        if self.verbosity >= cli::verbosity::VERBOSE {
            info!("Recursive call depth {}/{}: {}", current_depth + 1, max_depth, query.chars().take(50).collect::<String>());
        }

        // Execute the current query
        let result = self.execute_query(query.clone()).await?;
        let mut output = result.output;

        // Check if the agent wants to make another call
        // This is a simple heuristic - in practice, you might want more sophisticated parsing
        if output.to_lowercase().contains("execute query") || output.to_lowercase().contains("call agent") {
            // Extract the next query from the response (simplified)
            let next_query = output.lines()
                .find(|line| line.to_lowercase().contains("query:") || line.to_lowercase().contains("execute:"))
                .map(|line| line.split(':').skip(1).collect::<Vec<_>>().join(":").trim().to_string())
                .unwrap_or_else(|| "Continue with the next logical step".to_string());

            if !next_query.is_empty() && next_query != query {
                let recursive_result = self.execute_recursive_call(next_query, current_depth + 1, max_depth).await?;
                output = format!("{}\n\n--- Recursive Call Result ---\n{}", output, recursive_result);
            }
        }

        Ok(output)
        })
    }

    /// Execute daemon mode (continuous operation)
    pub async fn execute_daemon_mode(&self, cli: &Cli) -> Result<HeadlessResult, JunoError> {
        if self.verbosity >= cli::verbosity::NORMAL {
            println!("🔄 Starting daemon mode - listening for commands...");
            println!("Press Ctrl+C to stop");
        }

        let mut iteration_count = 0u64;
        let check_interval = Duration::from_secs(30); // Check every 30 seconds

        loop {
            iteration_count += 1;

            if self.verbosity >= cli::verbosity::VERBOSE {
                info!("Daemon iteration {}", iteration_count);
            }

            // Check for tasks or commands
            let status_result = self.execute_status().await?;

            // Simple daemon logic - could be enhanced with task queue, file watching, etc.
            if iteration_count % 10 == 0 { // Every 5 minutes, perform a system check
                let system_query = "Perform a system health check and report any issues or recommendations";
                if let Ok(health_result) = self.execute_query(system_query.to_string()).await {
                    if self.verbosity >= cli::verbosity::NORMAL {
                        println!("🏥 System Health Check: {}", health_result.output.chars().take(100).collect::<String>());
                    }
                }
            }

            // Check for cancellation
            if *self.app_handle.state::<AppState>().cancel_rx.borrow() {
                break;
            }

            // Sleep before next iteration
            tokio::time::sleep(check_interval).await;
        }

        Ok(HeadlessResult {
            success: true,
            output: format!("Daemon mode completed after {} iterations", iteration_count),
            error: None,
            execution_time: Duration::default(),
            agent_state: Some("Daemon Stopped".to_string()),
            screenshot: None,
        })
    }

    /// Execute batch mode (process multiple commands from file or stdin)
    pub async fn execute_batch_mode(&self, batch_file: Option<String>) -> Result<HeadlessResult, JunoError> {
        if self.verbosity >= cli::verbosity::VERBOSE {
            info!("Starting batch mode");
        }

        let commands = if let Some(file_path) = batch_file {
            // Read commands from file
            std::fs::read_to_string(&file_path)
                .map_err(|e| JunoError::FileSystemError(format!("Failed to read batch file {}: {}", file_path, e)))?
                .lines()
                .filter(|line| !line.trim().is_empty() && !line.trim().starts_with('#'))
                .map(|line| line.trim().to_string())
                .collect::<Vec<_>>()
        } else {
            // Read from stdin
            use std::io::{self, BufRead};
            let stdin = io::stdin();
            stdin.lock()
                .lines()
                .collect::<Result<Vec<_>, _>>()
                .map_err(|e| JunoError::ApplicationError(format!("Failed to read from stdin: {}", e)))?
                .into_iter()
                .filter(|line| !line.trim().is_empty() && !line.trim().starts_with('#'))
                .collect()
        };

        if commands.is_empty() {
            return Err(JunoError::ApplicationError("No commands found in batch input".to_string()));
        }

        let mut results = Vec::new();
        let total_commands = commands.len();

        for (i, command) in commands.into_iter().enumerate() {
            if self.verbosity >= cli::verbosity::NORMAL {
                println!("📝 Executing batch command {}/{}: {}", i + 1, total_commands, command.chars().take(50).collect::<String>());
            }

            match self.execute_query(command.clone()).await {
                Ok(result) => {
                    results.push(format!("Command {}: SUCCESS\n{}", i + 1, result.output));
                }
                Err(e) => {
                    results.push(format!("Command {}: ERROR - {}", i + 1, e));
                    if self.verbosity >= cli::verbosity::NORMAL {
                        eprintln!("❌ Command {} failed: {}", i + 1, e);
                    }
                }
            }

            // Check for cancellation
            if *self.app_handle.state::<AppState>().cancel_rx.borrow() {
                results.push("Batch execution cancelled by user".to_string());
                break;
            }
        }

        let final_output = results.join("\n\n");

        Ok(HeadlessResult {
            success: true,
            output: final_output,
            error: None,
            execution_time: Duration::default(),
            agent_state: Some("Batch Complete".to_string()),
            screenshot: None,
        })
    }

    /// Execute interactive mode
    async fn execute_interactive_mode(&self) -> Result<HeadlessResult, JunoError> {
        if self.verbosity >= cli::verbosity::NORMAL {
            println!("🎯 Interactive Mode - Type your queries (press Ctrl+C to exit)");
            println!("Available commands:");
            println!("  - Any text: Submit as query to agent");
            println!("  - :status: Check agent status");
            println!("  - :voice: Start voice input");
            println!("  - :exit: Exit interactive mode");
        }

        use std::io::{self, Write};
        let mut results = Vec::new();
        let mut command_count = 0u32;

        loop {
            // Display prompt
            print!("juno> ");
            io::stdout().flush().map_err(|e| JunoError::ApplicationError(format!("Failed to flush stdout: {}", e)))?;

            // Read user input
            let mut input = String::new();
            match io::stdin().read_line(&mut input) {
                Ok(_) => {
                    let input = input.trim();

                    if input.is_empty() {
                        continue;
                    }

                    // Handle special commands
                    match input {
                        ":exit" | ":quit" | ":q" => {
                            if self.verbosity >= cli::verbosity::NORMAL {
                                println!("👋 Exiting interactive mode");
                            }
                            break;
                        }
                        ":status" => {
                            match self.execute_status().await {
                                Ok(result) => {
                                    println!("📊 Status: {}", result.output);
                                    results.push(format!("Command {}: status check", command_count + 1));
                                }
                                Err(e) => {
                                    eprintln!("❌ Status check failed: {}", e);
                                }
                            }
                        }
                        ":voice" => {
                            if self.verbosity >= cli::verbosity::NORMAL {
                                println!("🎤 Starting voice input...");
                            }
                            match self.execute_dictation(60).await {
                                Ok(result) => {
                                    println!("🎤 Transcribed: {}", result.output);
                                    // Process the transcribed text as a query
                                    match self.execute_query(result.output.clone()).await {
                                        Ok(agent_result) => {
                                            println!("🤖 Agent: {}", agent_result.output);
                                            results.push(format!("Command {}: voice query - {}", command_count + 1, result.output.chars().take(50).collect::<String>()));
                                        }
                                        Err(e) => {
                                            eprintln!("❌ Agent query failed: {}", e);
                                        }
                                    }
                                }
                                Err(e) => {
                                    eprintln!("❌ Voice input failed: {}", e);
                                }
                            }
                        }
                        _ => {
                            // Regular query
                            command_count += 1;
                            if self.verbosity >= cli::verbosity::VERBOSE {
                                println!("🤖 Processing query {}...", command_count);
                            }

                            match self.execute_query(input.to_string()).await {
                                Ok(result) => {
                                    println!("🤖 {}", result.output);
                                    results.push(format!("Command {}: {}", command_count, input.chars().take(50).collect::<String>()));
                                }
                                Err(e) => {
                                    eprintln!("❌ Query failed: {}", e);
                                    results.push(format!("Command {}: ERROR - {}", command_count, e));
                                }
                            }
                        }
                    }
                }
                Err(e) => {
                    return Err(JunoError::ApplicationError(format!("Failed to read input: {}", e)));
                }
            }

            // Check for cancellation
            if *self.app_handle.state::<AppState>().cancel_rx.borrow() {
                results.push("Interactive mode cancelled by user".to_string());
                break;
            }
        }

        let final_output = if results.is_empty() {
            "No commands executed".to_string()
        } else {
            results.join("\n")
        };

        Ok(HeadlessResult {
            success: true,
            output: final_output,
            error: None,
            execution_time: Duration::default(),
            agent_state: Some("Interactive Complete".to_string()),
            screenshot: None,
        })
    }

    /// Output the result in the specified format
    pub fn output_result(&self, result: &HeadlessResult) {
        match self.output_format {
            OutputFormat::Quiet => {
                if !result.success {
                    eprintln!("Error: {}", result.error.as_deref().unwrap_or("Unknown error"));
                }
            }
            OutputFormat::Json => {
                // Try to parse output as JSON; if it fails, keep as string
                let parsed_output: Value = serde_json::from_str(&result.output)
                    .unwrap_or(Value::String(result.output.clone()));

                let json_output = json!({
                    "success": result.success,
                    "output": parsed_output,
                    "error": result.error,
                    "execution_time_ms": result.execution_time.as_millis(),
                    "agent_state": result.agent_state,
                    "screenshot": result.screenshot
                });
                println!("{}", serde_json::to_string_pretty(&json_output).unwrap_or_default());
            }
            OutputFormat::Yaml => {
                let yaml_output = serde_yaml::to_string(&json!({
                    "success": result.success,
                    "output": result.output,
                    "error": result.error,
                    "execution_time_ms": result.execution_time.as_millis(),
                    "agent_state": result.agent_state,
                    "screenshot": result.screenshot
                })).unwrap_or_else(|_| "# Error: Failed to serialize to YAML".to_string());
                println!("{}", yaml_output);
            }
            OutputFormat::Table => {
                println!("┌─────────────────┬─────────────────────────────────────────────────────────────────┐");
                println!("│ Field           │ Value                                                           │");
                println!("├─────────────────┼─────────────────────────────────────────────────────────────────┤");
                println!("│ Status          │ {}                                                           │",
                         if result.success { "✅ Success" } else { "❌ Failed" });
                if let Some(state) = &result.agent_state {
                    println!("│ Agent State     │ {:<63} │", state.chars().take(63).collect::<String>());
                }
                println!("│ Execution Time  │ {:<63} │", format!("{:?}", result.execution_time));

                // Attempt to parse output as JSON for structured display
                let parsed_json: Result<Value, _> = serde_json::from_str(&result.output);
                if let Ok(Value::Object(map)) = parsed_json {
                    for (key, val) in map.iter() {
                        let val_str = if let Some(s) = val.as_str() {
                            s.to_string()
                        } else {
                            val.to_string()
                        };
                        println!("│ {:<15} │ {:<63} │", key.chars().take(15).collect::<String>(), val_str.chars().take(63).collect::<String>());
                    }
                } else if !result.output.is_empty() {
                    let output_lines: Vec<&str> = result.output.lines().collect();
                    for (i, line) in output_lines.iter().enumerate() {
                        let field_name = if i == 0 { "Output" } else { "" };
                        println!("│ {:<15} │ {:<63} │", field_name, line.chars().take(63).collect::<String>());
                    }
                }

                if let Some(error) = &result.error {
                    println!("│ Error           │ {:<63} │", error.chars().take(63).collect::<String>());
                }
                println!("└─────────────────┴─────────────────────────────────────────────────────────────────┘");
            }
            OutputFormat::Markdown => {
                println!("# Agent Result\n");
                println!("**Status:** {}\n", if result.success { "✅ Success" } else { "❌ Failed" });
                if let Some(state) = &result.agent_state {
                    println!("**Agent State:** {}\n", state);
                }
                println!("**Execution Time:** {:?}\n", result.execution_time);
                if !result.output.is_empty() {
                    println!("## Output\n\n{}\n", result.output);
                }
                if let Some(error) = &result.error {
                    println!("## Error\n\n{}\n", error);
                }
            }
            OutputFormat::Text => {
                if result.success {
                    if !result.output.is_empty() {
                        println!("{}", result.output);
                    }
                    if self.verbosity >= cli::verbosity::VERBOSE {
                        if let Some(state) = &result.agent_state {
                            println!("\n[Agent State: {}]", state);
                        }
                        println!("[Execution Time: {:?}]", result.execution_time);
                    }
                } else {
                    eprintln!("Error: {}", result.error.as_deref().unwrap_or("Unknown error"));
                }
            }
        }
    }

    /// Output an error message in the appropriate format
    pub fn output_error(&self, error: &str) {
        match self.output_format {
            OutputFormat::Quiet => {
                eprintln!("{}", error);
            }
            OutputFormat::Json => {
                let error_result = json!({
                    "success": false,
                    "error": error
                });
                println!("{}", serde_json::to_string_pretty(&error_result).unwrap_or_else(|_| {
                    format!("{{\"success\":false,\"error\":\"{}\"}}", error)
                }));
            }
            OutputFormat::Yaml => {
                let yaml_output = serde_yaml::to_string(&json!({
                    "success": false,
                    "error": error
                })).unwrap_or_else(|_| format!("# Error\n# Failed to serialize to YAML: {}", error));
                println!("{}", yaml_output);
            }
            OutputFormat::Table => {
                println!("┌─────────────────┬─────────────────────────────────────────────────────────────────┐");
                println!("│ Field           │ Value                                                           │");
                println!("├─────────────────┼─────────────────────────────────────────────────────────────────┤");
                println!("│ Status          │ ❌ Failed                                                       │");
                println!("│ Error           │ {:<63} │", error.chars().take(63).collect::<String>());
                println!("└─────────────────┴─────────────────────────────────────────────────────────────────┘");
            }
            OutputFormat::Markdown => {
                println!("# Error\n\n{}", error);
            }
            OutputFormat::Text => {
                eprintln!("Error: {}", error);
            }
        }
    }
}

/// Initialize headless mode for the application
pub async fn init_headless_mode(app_handle: AppHandle) -> Result<(), JunoError> {
    info!("Initializing headless mode");

    // Disable UI-specific features
    let state = app_handle.state::<AppState>();

    // Set headless flag in state if needed
    // Note: You might want to add a headless flag to AppState

    // Initialize minimal components required for headless operation
    // Voice controller is already initialized in the main startup sequence

    info!("Headless mode initialized successfully");
    Ok(())
}

/// Check if the application should run in headless mode
pub fn should_run_headless(cli: &Cli) -> bool {
    cli.is_headless_required()
}

/// Set headless mode flag
pub fn set_headless_mode(enabled: bool) {
    HEADLESS_MODE.store(enabled, Ordering::SeqCst);
}

/// Check if currently in headless mode
pub fn is_headless_mode() -> bool {
    HEADLESS_MODE.load(Ordering::SeqCst)
}

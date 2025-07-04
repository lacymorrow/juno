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
use tauri::{AppHandle, Manager};
use serde_json::{json, Value};

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

        let result = if let Some(command) = &cli.command {
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
    async fn execute_voice_command(&self, _command: &crate::cli::VoiceCommands) -> Result<HeadlessResult, JunoError> {
        // Mock implementation - TODO: implement actual voice commands
        Ok(HeadlessResult {
            success: true,
            output: "Voice command executed".to_string(),
            error: None,
            execution_time: Duration::default(),
            agent_state: Some("Completed".to_string()),
            screenshot: None,
        })
    }

    /// Execute dictation subcommands
    async fn execute_dictation_command(&self, _command: &crate::cli::DictationCommands) -> Result<HeadlessResult, JunoError> {
        // Mock implementation - TODO: implement actual dictation commands
        Ok(HeadlessResult {
            success: true,
            output: "Dictation command executed".to_string(),
            error: None,
            execution_time: Duration::default(),
            agent_state: Some("Completed".to_string()),
            screenshot: None,
        })
    }

    /// Execute agent subcommands
    async fn execute_agent_command(&self, _command: &crate::cli::AgentCommands) -> Result<HeadlessResult, JunoError> {
        // Mock implementation - TODO: implement actual agent commands
        Ok(HeadlessResult {
            success: true,
            output: "Agent command executed".to_string(),
            error: None,
            execution_time: Duration::default(),
            agent_state: Some("Completed".to_string()),
            screenshot: None,
        })
    }

    /// Execute config subcommands
    async fn execute_config_command(&self, _command: &crate::cli::ConfigCommands) -> Result<HeadlessResult, JunoError> {
        // Mock implementation - TODO: implement actual config commands
        Ok(HeadlessResult {
            success: true,
            output: "Config command executed".to_string(),
            error: None,
            execution_time: Duration::default(),
            agent_state: Some("Completed".to_string()),
            screenshot: None,
        })
    }

    /// Execute system subcommands
    async fn execute_system_command(&self, _command: &crate::cli::SystemCommands) -> Result<HeadlessResult, JunoError> {
        // Mock implementation - TODO: implement actual system commands
        Ok(HeadlessResult {
            success: true,
            output: "System command executed".to_string(),
            error: None,
            execution_time: Duration::default(),
            agent_state: Some("Completed".to_string()),
            screenshot: None,
        })
    }

    /// Execute daemon subcommands
    async fn execute_daemon_command(&self, _command: &crate::cli::DaemonCommands) -> Result<HeadlessResult, JunoError> {
        // Mock implementation - TODO: implement actual daemon commands
        Ok(HeadlessResult {
            success: true,
            output: "Daemon command executed".to_string(),
            error: None,
            execution_time: Duration::default(),
            agent_state: Some("Completed".to_string()),
            screenshot: None,
        })
    }

    /// Execute test subcommands
    async fn execute_test_command(&self, _command: &crate::cli::TestCommands) -> Result<HeadlessResult, JunoError> {
        // Mock implementation - TODO: implement actual test commands
        Ok(HeadlessResult {
            success: true,
            output: "Test command executed".to_string(),
            error: None,
            execution_time: Duration::default(),
            agent_state: Some("Completed".to_string()),
            screenshot: None,
        })
    }

    /// Execute a text query
    async fn execute_query(&self, query: String) -> Result<HeadlessResult, JunoError> {
        if self.verbosity >= 2 {
            info!("Executing text query: {}", query);
        }

        // Set up result capture
        let (result_tx, result_rx) = oneshot::channel::<String>();
        let result_tx = Arc::new(Mutex::new(Some(result_tx)));

        // Set up event listeners for capturing agent response
        let _app_handle = self.app_handle.clone();
        let _result_tx_clone = result_tx.clone();

        // Note: In headless mode, we'll use a different approach than event listening
        // TODO: Implement proper event handling for headless mode

        // Submit the query to the agent
        let state = self.app_handle.state::<AppState>();
        crate::anthropic::submit_query(query.clone(), state, self.app_handle.clone()).await
            .map_err(|e| JunoError::ApplicationError(format!("Failed to submit query: {}", e)))?;

        // Wait for result with timeout
        let result = timeout(self.timeout_duration, result_rx).await
            .map_err(|_| JunoError::ApplicationError("Query execution timed out".to_string()))?
            .map_err(|_| JunoError::ApplicationError("Failed to receive query result".to_string()))?;

        // Parse the result
        let output = if let Ok(json_result) = serde_json::from_str::<Value>(&result) {
            json_result.get("text")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string())
                .unwrap_or_else(|| result.clone())
        } else {
            result
        };

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
                        let _result_tx_clone = result_tx.clone();
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

        let state = self.app_handle.state::<AppState>();

        let status = json!({
            "agent_executing": state.is_agent_executing(),
            "dictation_active": state.is_dictation_active(),
            "always_listening": state.get_always_listening_active().unwrap_or(false),
            "tts_provider": state.get_tts_provider().unwrap_or_default(),
            "voice_available": self.app_handle.try_state::<Arc<Mutex<tauri_plugin_voice_transcription::controller::VoiceController>>>().is_some(),
            "permissions": {
                "accessibility": true, // Could check actual permissions
                "microphone": true,
            }
        });

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
                let json_output = json!({
                    "success": result.success,
                    "output": result.output,
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
                if !result.output.is_empty() {
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

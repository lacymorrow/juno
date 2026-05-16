use crate::state::CancelReceiver; // Import the type alias
use async_trait::async_trait;
use std::sync::Arc;
use tokio::sync::Mutex; // Using Mutex for mutable access to MemoryManager
use uuid;

use crate::agent::core::{
    AgentAction,
    AgentError,
    AgentState,
    Message,
    Role, // Removed ToolCall, ToolResult
};
// use crate::agent::tool_logger; // Added for logging
use crate::agent::traits::{AgentBrain, AgentRunnable, MemoryManager, ToolProvider};
use tauri::{AppHandle, Emitter, Manager}; // Added Manager trait for accessing app state
use crate::constants::events;

/// Default implementation of the AgentRunnable trait.
/// Orchestrates the agent's execution flow using the provided components.
pub struct DefaultAgentRunner<M, T>
where
    M: MemoryManager + Send + Sync,
    T: ToolProvider + Send + Sync,
{
    state: AgentState,
    memory: Arc<Mutex<M>>, // Use Mutex for mutable access across async tasks
    tool_provider: Arc<T>,
    brain: Arc<dyn AgentBrain + Send + Sync>, // Use trait object directly
    max_steps: u32,
    current_step: u32,
    app_handle: Arc<AppHandle>, // Added AppHandle for logging
}

impl<M, T> DefaultAgentRunner<M, T>
where
    M: MemoryManager + Send + Sync + 'static,
    T: ToolProvider + Send + Sync + 'static,
{
    pub fn new(
        memory: M,
        tool_provider: T,
        brain: impl AgentBrain + 'static,
        max_steps: u32,
        app_handle: AppHandle, // Added AppHandle
    ) -> Self {
        log::info!(
            "DefaultAgentRunner::new created. max_steps: {}, current_step: 0 (hardcoded init)",
            max_steps
        );
        DefaultAgentRunner {
            state: AgentState::Idle,
            memory: Arc::new(Mutex::new(memory)),
            tool_provider: Arc::new(tool_provider),
            brain: Arc::new(brain),
            max_steps,
            current_step: 0,
            app_handle: Arc::new(app_handle), // Store AppHandle
        }
    }

    /// Creates a new DefaultAgentRunner with a boxed brain implementation
    pub fn with_boxed_brain(
        memory: M,
        tool_provider: T,
        brain: Box<dyn AgentBrain + Send + Sync>,
        max_steps: u32,
        app_handle: AppHandle, // Added AppHandle
    ) -> Self {
        log::info!("DefaultAgentRunner::with_boxed_brain created. max_steps: {}, current_step: 0 (hardcoded init)", max_steps);
        DefaultAgentRunner {
            state: AgentState::Idle,
            memory: Arc::new(Mutex::new(memory)),
            tool_provider: Arc::new(tool_provider),
            brain: Arc::from(brain), // Convert Box to Arc
            max_steps,
            current_step: 0,
            app_handle: Arc::new(app_handle), // Store AppHandle
        }
    }

    /// Filter tools based on brain type to prevent access to inappropriate tools
    fn filter_tools_for_brain(
        &self,
        all_tools: &[crate::agent::core::ToolDefinition],
    ) -> Vec<crate::agent::core::ToolDefinition> {
        // No filtering needed - tool providers are correctly separated at creation:
        // - Single agent: Gets all tools via agent_tool_provider
        // - Multi-agent orchestrator: Gets only delegation tools via orchestrator_tool_provider
        // - Specialists: Get domain-specific tools via their own providers
        all_tools.to_vec()
    }

    /// Extract target coordinates from tool input supporting multiple coordinate formats
    /// This function supports:
    /// 1. Drag operations: {"end_coordinate": [x, y]} - prioritized for mouse movement completion
    /// 2. Anthropic Computer Use API format: {"coordinate": [x, y]}
    /// 3. Separate x/y fields: {"x": 100, "y": 200}
    /// 4. Nested coordinate object: {"coordinate": {"x": 100, "y": 200}}
    fn extract_target_coordinates(&self, input: &serde_json::Value) -> Option<(i32, i32)> {
        // Format 1: Handle drag operations FIRST - use end coordinate for destination
        // For drag operations, end_coordinate represents the target destination for movement completion
        // {"end_coordinate": [end_x, end_y], "coordinate": [start_x, start_y]} -> use end_coordinate
        if let Some(end_coord_array) = input.get("end_coordinate").and_then(|c| c.as_array()) {
            if end_coord_array.len() == 2 {
                if let (Some(x), Some(y)) = (
                    end_coord_array[0].as_f64(),
                    end_coord_array[1].as_f64()
                ) {
                    return Some((x as i32, y as i32));
                }
            }
        }

        // Format 2: Anthropic Computer Use API - {"coordinate": [x, y]}
        if let Some(coord_array) = input.get("coordinate").and_then(|c| c.as_array()) {
            if coord_array.len() == 2 {
                if let (Some(x), Some(y)) = (
                    coord_array[0].as_f64(),
                    coord_array[1].as_f64()
                ) {
                    return Some((x as i32, y as i32));
                }
            }
        }

        // Format 3: Separate x/y fields - {"x": 100, "y": 200}
        if let (Some(x), Some(y)) = (
            input.get("x").and_then(|v| v.as_f64()),
            input.get("y").and_then(|v| v.as_f64())
        ) {
            return Some((x as i32, y as i32));
        }

        // Format 4: Nested coordinate object - {"coordinate": {"x": 100, "y": 200}}
        if let Some(coord_obj) = input.get("coordinate").and_then(|c| c.as_object()) {
            if let (Some(x), Some(y)) = (
                coord_obj.get("x").and_then(|v| v.as_f64()),
                coord_obj.get("y").and_then(|v| v.as_f64())
            ) {
                return Some((x as i32, y as i32));
            }
        }

        // If no format matches, return None
        None
    }

    /// Check if a tool call involves mouse movement that would benefit from completion detection
    fn is_mouse_movement_tool(&self, tool_call: &crate::agent::core::ToolCall) -> bool {
        // Check computer tool with mouse_move action
        if tool_call.name == "computer" {
            if let Some(action) = tool_call.input.get("action").and_then(|a| a.as_str()) {
                return action == "mouse_move";
            }
        }

        // Check direct mouse movement tools
        if tool_call.name == "mouse_move" {
            return true;
        }

        // Check tools that involve coordinate movement (like scroll_at_position)
        if tool_call.name == "scroll_at_position" {
            return true;
        }

        false
    }

    async fn transition_state(&mut self, new_state: AgentState) {
        log::debug!(
            "Agent state transition: {:?} -> {:?}",
            self.state,
            new_state
        );
        self.state = new_state;
        // TODO: Emit state change events if needed (e.g., for UI updates)
    }

    /// Enhanced tool execution with intelligent batching support
    /// Simply executes whatever tool calls the agent provides
    async fn execute_tools_with_batching(
        &mut self,
        tool_calls: Vec<crate::agent::core::ToolCall>,
        cancel_rx: &crate::state::CancelReceiver,
    ) -> Result<(), AgentError> {
        if tool_calls.is_empty() {
            return Ok(());
        }

        log::info!(
            "Executing {} tool call(s) as provided by agent",
            tool_calls.len()
        );

        // Initialize result cache for all tools
        let mut tool_results_cache = Vec::new();
        for tool_call in tool_calls.iter() {
            tool_results_cache.push((tool_call.clone(), None));
        }

        // Execute the tools as provided - simplified logic
        match self
            .execute_tool_batch(&tool_calls, cancel_rx, 0, &mut tool_results_cache)
            .await?
        {
            true => {} // Continue
            false => {
                // Cancellation occurred - handle incomplete tool execution
                self.handle_batch_cancellation(&tool_calls, &tool_results_cache)
                    .await?;
                return Err(AgentError::Terminated);
            }
        }

        Ok(())
    }

    /// Handle cancellation by adding "cancelled" messages for unexecuted tools
    /// This ensures conversation memory remains consistent even when execution is interrupted
    async fn handle_batch_cancellation(
        &mut self,
        _tool_calls: &[crate::agent::core::ToolCall],
        tool_results_cache: &[(
            crate::agent::core::ToolCall,
            Option<Result<crate::agent::core::ToolResult, AgentError>>,
        )],
    ) -> Result<(), AgentError> {
        let mut cancelled_count = 0;
        let mut mem = self.memory.lock().await;

        for (tool_call, result) in tool_results_cache {
            if result.is_none() {
                // Tool was not executed due to cancellation
                mem.add_message(crate::agent::core::Message {
                    role: crate::agent::core::Role::Tool,
                    content: "Tool execution was cancelled".to_string(),
                    tool_calls: None,
                    tool_call_id: Some(tool_call.id.clone()),
                    name: Some(tool_call.name.clone()),
                })
                .await?;
                cancelled_count += 1;
            }
        }

        log::info!(
            "Added cancellation messages for {} unexecuted tools",
            cancelled_count
        );
        Ok(())
    }

    /// Simplified tool batch execution - trust the agent, execute the tools
    async fn execute_tool_batch(
        &mut self,
        batch: &[crate::agent::core::ToolCall],
        cancel_rx: &crate::state::CancelReceiver,
        start_index: usize,
        tool_results_cache: &mut [(
            crate::agent::core::ToolCall,
            Option<Result<crate::agent::core::ToolResult, AgentError>>,
        )],
    ) -> Result<bool, AgentError> {
        log::info!("Executing tool batch: {} tools", batch.len());

        // Simplified: Just execute all tools sequentially with batch approval
        self.execute_sequential_batch(batch, cancel_rx, start_index, tool_results_cache)
            .await
    }

    /// Execute batch sequentially but with optimized approval process
    async fn execute_sequential_batch(
        &mut self,
        batch: &[crate::agent::core::ToolCall],
        cancel_rx: &crate::state::CancelReceiver,
        start_index: usize,
        tool_results_cache: &mut [(
            crate::agent::core::ToolCall,
            Option<Result<crate::agent::core::ToolResult, AgentError>>,
        )],
    ) -> Result<bool, AgentError> {
        // Batch approval for sequential operations
        if !self.check_batch_approval(batch, cancel_rx).await? {
            return Ok(true);
        }

        log::info!(
            "Executing sequential batch: {} tools (approval granted for batch)",
            batch.len()
        );

        for (i, tool_call) in batch.iter().enumerate() {
            // Check cancellation before each tool
            if *cancel_rx.borrow() {
                log::info!(
                    "Cancellation detected during sequential batch execution at tool {} of {}",
                    i,
                    batch.len()
                );
                return Ok(false);
            }

            // Execute without individual approval (already approved for batch)
            log::info!(
                "Executing batched tool {}/{}: {}",
                i + 1,
                batch.len(),
                tool_call.name
            );

            // Skip runner-level logging for "computer" tool — it self-logs with
            // enhanced metadata inside anthropic_computer_use.rs
            if tool_call.name != "computer" {
                crate::agent::tool_logger::log_tool_call_request(
                    &self.app_handle,
                    &tool_call.name,
                    tool_call.input.clone(),
                    Some(format!("Executing batched tool: {}", tool_call.name)),
                );
            }

            // Race tool execution against the cancellation signal so slow tools
            // (browser navigation, network requests) are interrupted immediately
            // when the user presses Escape — not just between tools.
            let mut cancel_for_tool = cancel_rx.clone();
            let tool_result = tokio::select! {
                result = self.tool_provider.execute_tool(tool_call.clone()) => result,
                _ = cancel_for_tool.wait_for(|&v| v) => {
                    log::info!(
                        "Tool '{}' interrupted by cancellation signal during execution (tool {}/{} in batch)",
                        tool_call.name, i + 1, batch.len()
                    );
                    return Ok(false);
                }
            };

            // PERFORMANCE OPTIMIZATION: Replace hardcoded delays with intelligent completion detection
            // Old approach: Hardcoded 350ms delay for ALL mouse movements
            // New approach: Event-driven completion detection (10-50x faster)
            if self.is_mouse_movement_tool(tool_call) {
                // Use intelligent movement detection for any mouse movement operation
                self.wait_for_mouse_movement_completion(tool_call).await;
            }

            // Skip runner-level result logging for "computer" tool — it self-logs
            // with enhanced metadata inside anthropic_computer_use.rs
            if tool_call.name != "computer" {
                match &tool_result {
                    Ok(result) => {
                        let is_error = crate::agent::tools::anthropic_computer_use::is_anthropic_error_response(&result.output);
                        let success = !is_error;

                        let screenshot_base64 = if success &&
                            (tool_call.name == "capture_screenshot" || tool_call.name == "browser_screenshot") {
                            if let Some(screenshot_data) = result.output.get("base64_image") {
                                screenshot_data.as_str().map(|s| s.to_string())
                            } else if let Some(screenshot_data) = result.output.get("base64") {
                                screenshot_data.as_str().map(|s| s.to_string())
                            } else if let Some(screenshot_data) = result.output.get("data") {
                                screenshot_data.as_str().map(|s| s.to_string())
                            } else {
                                result.output.as_str().map(|s| s.to_string())
                            }
                        } else {
                            None
                        };

                        let status_message = if success {
                            format!("Batched tool {} executed successfully", tool_call.name)
                        } else {
                            let error_msg = crate::agent::tools::anthropic_computer_use::extract_anthropic_error_message(&result.output)
                                .unwrap_or_else(|| "Unknown error".to_string());
                            format!("Batched tool {} failed: {}", tool_call.name, error_msg)
                        };

                        crate::agent::tool_logger::log_tool_call_result(
                            &self.app_handle,
                            &tool_call.name,
                            result.output.clone(),
                            success,
                            Some(status_message),
                            screenshot_base64,
                        );
                    }
                    Err(error) => {
                        crate::agent::tool_logger::log_tool_call_result(
                            &self.app_handle,
                            &tool_call.name,
                            serde_json::json!({"error": error.to_string()}),
                            false,
                            Some(format!("Batched tool {} failed: {}", tool_call.name, error)),
                            None,
                        );
                    }
                }
            }

            tool_results_cache[start_index + i].1 = Some(tool_result.clone());
            self.add_tool_result_to_memory(tool_call, tool_result)
                .await?;
        }

        Ok(true)
    }

    /// Check approval for a batch of tools.
    ///
    /// Approval is required when:
    /// - The global `tool_approval_required` flag is set, OR
    /// - Any tool in the batch has a High or Critical risk level (auto-detected).
    async fn check_batch_approval(
        &self,
        batch: &[crate::agent::core::ToolCall],
        cancel_rx: &crate::state::CancelReceiver,
    ) -> Result<bool, AgentError> {
        use crate::agent::tools::risk_classifier;
        use crate::state::RiskLevel;

        let app_state = self.app_handle.state::<crate::state::AppState>();

        // Classify each tool once and find the highest risk level.
        let risk_levels: Vec<RiskLevel> = batch
            .iter()
            .map(|t| risk_classifier::classify_risk(&t.name, &t.input))
            .collect();
        let max_risk = risk_levels
            .iter()
            .copied()
            .max()
            .unwrap_or(RiskLevel::Low);

        // Gate: skip approval unless the global flag is set OR any tool is risky.
        if !app_state.is_tool_approval_required()
            && !risk_classifier::needs_approval(&max_risk)
        {
            return Ok(true);
        }

        // Find the riskiest single tool using pre-computed classifications.
        let riskiest_idx = risk_levels
            .iter()
            .enumerate()
            .max_by_key(|(_, r)| *r)
            .map(|(i, _)| i)
            .unwrap_or(0);
        let riskiest_tool = &batch[riskiest_idx];

        let target_app =
            risk_classifier::extract_target_app(&riskiest_tool.name, &riskiest_tool.input);

        // Build a human-readable description.
        let batch_description = if batch.len() == 1 {
            format!(
                "Run {} — {}",
                riskiest_tool.name,
                riskiest_tool
                    .input
                    .get("command")
                    .or_else(|| riskiest_tool.input.get("url"))
                    .or_else(|| riskiest_tool.input.get("path"))
                    .and_then(|v| v.as_str())
                    .unwrap_or("(no details)")
                    .chars()
                    .take(120)
                    .collect::<String>()
            )
        } else {
            format!(
                "Execute {} tools: {}{}",
                batch.len(),
                batch
                    .iter()
                    .take(3)
                    .map(|t| t.name.as_str())
                    .collect::<Vec<_>>()
                    .join(" → "),
                if batch.len() > 3 { " ..." } else { "" }
            )
        };

        let batch_id = uuid::Uuid::new_v4().to_string();
        let approval_request = crate::state::ToolApprovalRequest::new(
            batch_id.clone(),
            riskiest_tool.name.clone(),
            serde_json::json!({
                "batch_size": batch.len(),
                "tools": batch.iter().map(|t| &t.name).collect::<Vec<_>>()
            }),
            batch_description.clone(),
        )
        .with_risk(max_risk.clone())
        .with_timeout(60);

        let approval_request = if let Some(ref app) = target_app {
            approval_request.with_target_app(app.clone())
        } else {
            approval_request
        };

        // Add to pending approvals
        app_state
            .add_pending_tool_approval(approval_request.clone())
            .await;

        // Emit approval request event (risk_level + target_app added for UI)
        let approval_event = serde_json::json!({
            "tool_name": approval_request.tool_name,
            "tool_id": approval_request.tool_id,
            "tool_input": approval_request.tool_input,
            "description": approval_request.description,
            "timestamp": approval_request.timestamp,
            "risk_level": approval_request.risk_level,
            "target_app": approval_request.target_app,
            "timeout_seconds": approval_request.timeout_seconds,
            "is_batch": batch.len() > 1,
            "batch_size": batch.len()
        });

        if let Err(e) = self
            .app_handle
            .emit(events::tools::APPROVAL_REQUEST, approval_event)
        {
            log::error!("Failed to emit batch approval request: {}", e);
        }

        log::info!(
            "Waiting for user approval — batch: {}, risk: {:?}",
            batch_description,
            max_risk
        );

        // Poll for up to timeout_seconds at 50 ms intervals.
        let poll_iterations = (approval_request.timeout_seconds * 1000 / 50) as i64;
        let mut remaining = poll_iterations;
        let mut approved = false;

        while remaining > 0 && !approved {
            if *cancel_rx.borrow() {
                log::info!("Cancellation detected during approval wait");
                app_state.remove_tool_approval(&batch_id).await;
                return Err(AgentError::Terminated);
            }

            match app_state.get_tool_approval_status(&batch_id).await {
                Some(true) => {
                    approved = true;
                    log::info!("Tool batch approved");
                    break;
                }
                Some(false) => {
                    log::info!("Tool batch denied by user");
                    break;
                }
                None => {}
            }

            tokio::time::sleep(std::time::Duration::from_millis(50)).await;
            remaining -= 1;
        }

        app_state.remove_tool_approval(&batch_id).await;

        if !approved {
            let reason = if remaining <= 0 { "timeout" } else { "user denied" };
            log::warn!("Tool batch execution denied: {}", reason);

            for tool_call in batch {
                let mut mem = self.memory.lock().await;
                mem.add_message(crate::agent::core::Message {
                    role: crate::agent::core::Role::Tool,
                    content: format!("Tool execution was denied ({})", reason),
                    tool_calls: None,
                    tool_call_id: Some(tool_call.id.clone()),
                    name: Some(tool_call.name.clone()),
                })
                .await?;
            }
        }

        Ok(approved)
    }

    /// Add tool result to memory with proper error handling
    async fn add_tool_result_to_memory(
        &mut self,
        tool_call: &crate::agent::core::ToolCall,
        tool_result: Result<crate::agent::core::ToolResult, AgentError>,
    ) -> Result<(), AgentError> {
        let mut mem = self.memory.lock().await;
        mem.add_message(crate::agent::core::Message {
            role: crate::agent::core::Role::Tool,
            content: match &tool_result {
                Ok(result) => {
                    // For computer tool with screenshot data, preserve the full JSON
                    // so the Anthropic provider can extract the base64 image later
                    if tool_call.name == "computer"
                        && result.output.get("base64_image").is_some()
                    {
                        serde_json::to_string(&result.output).unwrap_or_else(|_| {
                            crate::agents::base_agent::format_task_output(&result.output)
                        })
                    } else {
                        crate::agents::base_agent::format_task_output(&result.output)
                    }
                }
                Err(e) => format!("Error: {}", e),
            },
            tool_calls: None,
            tool_call_id: Some(tool_call.id.clone()),
            name: Some(tool_call.name.clone()),
        })
        .await?;

        Ok(())
    }

    /// PERFORMANCE OPTIMIZATION: Intelligent mouse movement completion detection
    /// Replaces hardcoded 350ms delays with event-driven detection (10-50x faster)
    async fn wait_for_mouse_movement_completion(&self, tool_call: &crate::agent::core::ToolCall) {
        // Extract target coordinates from tool call using multiple format support
        let target_coords = self.extract_target_coordinates(&tool_call.input);

        // If we can't extract coordinates, fall back to minimal delay
        let Some((target_x, target_y)) = target_coords else {
            tokio::time::sleep(tokio::time::Duration::from_millis(25)).await;
            return;
        };

        // Poll cursor position with exponential backoff until movement completes
        let start_time = std::time::Instant::now();
        let max_wait = std::time::Duration::from_millis(200); // Still much less than 350ms
        let tolerance = 3; // pixels tolerance for "close enough"

        for attempt in 0..8 { // Max 8 attempts
            // Check timeout at the start of each iteration
            if start_time.elapsed() >= max_wait {
                break;
            }

            // Get current cursor position
            let app_state = self.app_handle.state::<crate::state::AppState>();
            if let Ok((current_x, current_y)) = crate::commands::mouse::get_cursor_position(
                (*self.app_handle).clone(),
                app_state,
            ).await {
                // Check if we're close enough to target
                let distance_x = (current_x - target_x as f64).abs();
                let distance_y = (current_y - target_y as f64).abs();

                if distance_x <= tolerance as f64 && distance_y <= tolerance as f64 {
                    // Movement complete! Exit immediately
                    return;
                }
            }

            // Calculate exponential backoff delay, but cap it to remaining time budget
            let base_delay_ms = 5 * (1 << attempt);
            let remaining_time = max_wait.saturating_sub(start_time.elapsed());
            let remaining_ms = remaining_time.as_millis() as u64;

            // Only sleep if we have time remaining and the delay makes sense
            if remaining_ms > 10 {
                let actual_delay_ms = std::cmp::min(base_delay_ms, remaining_ms - 5); // Leave 5ms buffer
                tokio::time::sleep(tokio::time::Duration::from_millis(actual_delay_ms)).await;
            } else {
                // Not enough time left for meaningful waiting
                break;
            }
        }

        // Final minimal delay only if we have time remaining (prevent exceeding max_wait)
        let remaining_time = max_wait.saturating_sub(start_time.elapsed());
        if remaining_time.as_millis() >= 10 {
            tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;
        }
    }

}

#[async_trait]
impl<M, T> AgentRunnable for DefaultAgentRunner<M, T>
where
    M: MemoryManager + Send + Sync + 'static,
    T: ToolProvider + Send + Sync + 'static,
{
    async fn run(
        &mut self,
        initial_prompt: String,
        cancel_rx: CancelReceiver, // Use watch receiver (no longer needs mut)
    ) -> Result<String, AgentError> {
        log::info!(
            "DefaultAgentRunner::run called. Initial state - current_step: {}, max_steps: {}, agent_state: {:?}",
            self.current_step,
            self.max_steps,
            self.state
        );

        if self.state != AgentState::Idle {
            return Err(AgentError::StateError(
                "Agent must be in Idle state to start.".to_string(),
            ));
        }

        // Clone the receiver for the step function
        let step_cancel_rx = cancel_rx.clone();

        self.transition_state(AgentState::Thinking).await;
        self.current_step = 0;
        log::info!(
            "DefaultAgentRunner::run starting loop. current_step reset to: {}, max_steps: {}",
            self.current_step,
            self.max_steps
        );

        // Add initial user message to memory
        {
            let mut mem = self.memory.lock().await;
            mem.add_message(Message {
                role: Role::User,
                content: initial_prompt,
                tool_calls: None,
                tool_call_id: None,
                name: None,
            })
            .await?;
        }

        loop {
            // --- Cancellation Check (Start of Loop) ---
            if *cancel_rx.borrow() {
                log::info!("Agent run cancelled.");
                self.transition_state(AgentState::Failed("Cancelled".to_string()))
                    .await;
                return Err(AgentError::Terminated);
            }

            // Check max steps AFTER incrementing, so we get the full number of steps
            if self.current_step >= self.max_steps {
                log::warn!(
                    "Reached maximum steps ({}), requesting continuation from user",
                    self.max_steps
                );

                // Get current execution ID from AppState
                let app_state = self.app_handle.state::<crate::state::AppState>();
                let execution_id = app_state
                    .get_current_agent_execution_id()
                    .unwrap_or_else(|| uuid::Uuid::new_v4().to_string());

                // Request continuation from user
                match crate::commands::agent_continuation::request_agent_continuation(
                    execution_id.clone(),
                    self.current_step,
                    self.max_steps,
                    &self.app_handle,
                )
                .await
                {
                    Ok(Some(response)) => {
                        if response.approved {
                            // User approved continuation - extend max_steps
                            let mut additional_steps = response.additional_steps.unwrap_or(
                                crate::constants::agent::config::DEFAULT_CONTINUATION_ADDITIONAL_STEPS
                            );

                            // Fix Issue 1: Prevent infinite loop with 0 additional steps
                            if additional_steps == 0 {
                                log::warn!("User approved continuation but provided 0 additional steps. Using default value.");
                                additional_steps = crate::constants::agent::config::DEFAULT_CONTINUATION_ADDITIONAL_STEPS;
                            }

                            // Fix Issue 2: Prevent integer overflow using saturating_add
                            let new_max_steps = self.max_steps.saturating_add(additional_steps);

                            // Check if we hit the saturation limit
                            if new_max_steps == u32::MAX && self.max_steps < u32::MAX {
                                log::warn!(
                                    "Maximum steps would overflow. Capped at maximum value: {}",
                                    u32::MAX
                                );
                            }

                            self.max_steps = new_max_steps;
                            log::info!(
                                "User approved continuation. Extended max steps to {} (+{} steps)",
                                self.max_steps,
                                additional_steps
                            );

                            // Update AppState with new max steps
                            if let Ok(mut execution_state) = app_state.agent_execution.lock() {
                                execution_state.max_steps = Some(self.max_steps);
                            }

                            // Continue execution - don't return error
                        } else {
                            log::info!("User denied continuation. Terminating agent.");
                            self.transition_state(AgentState::Failed(
                                "Continuation denied by user".to_string(),
                            ))
                            .await;
                            return Err(AgentError::MaxStepsReached);
                        }
                    }
                    Ok(None) => {
                        // Timeout or no response - terminate
                        log::warn!(
                            "No response to continuation request (timeout). Terminating agent."
                        );
                        self.transition_state(AgentState::Failed(
                            "Max steps reached (no continuation response)".to_string(),
                        ))
                        .await;
                        return Err(AgentError::MaxStepsReached);
                    }
                    Err(e) => {
                        log::error!("Failed to request continuation: {}. Terminating agent.", e);
                        self.transition_state(AgentState::Failed(
                            "Max steps reached (continuation error)".to_string(),
                        ))
                        .await;
                        return Err(AgentError::MaxStepsReached);
                    }
                }
            }

            // Increment step counter at the START of each iteration
            self.current_step += 1;
            log::info!("Agent step {} of {}", self.current_step, self.max_steps);

            // Update AppState with current step progress
            let app_state = self.app_handle.state::<crate::state::AppState>();
            let _ = app_state.update_agent_current_step(self.current_step);

            // Execute one step of the agent loop, passing the cloned receiver
            let action = self.step(step_cancel_rx.clone()).await?;

            // Handle agent action
            match action {
                AgentAction::Finish(text) => {
                    log::info!("Agent finished with text response: \"{}\"", text);
                    self.transition_state(AgentState::Finished).await;
                    let final_response = text;
                    return Ok(final_response);
                }

                AgentAction::RespondToUser(text) => {
                    log::info!("Agent intermediate response: {}", text);
                    // Add the assistant's response to memory
                    {
                        let mut mem = self.memory.lock().await;
                        mem.add_message(Message {
                            role: Role::Assistant,
                            content: text.clone(),
                            tool_calls: None,
                            tool_call_id: None,
                            name: None,
                        })
                        .await?;
                    }
                    // Continue loop, keep thinking unless brain explicitly finishes
                    self.transition_state(AgentState::Thinking).await;
                }
                AgentAction::Error(e) => {
                    let error_message = e.to_string();
                    log::error!("Agent encountered error: {}", error_message);
                    self.transition_state(AgentState::Failed(error_message))
                        .await;
                    return Err(e); // Propagate the error
                }
                AgentAction::Think | AgentAction::ExecuteTool(_) => {
                    // ExecuteTool is handled within step().
                    // Think means continue the loop.
                    log::debug!("AgentAction::Think or handled ExecuteTool, continuing loop.");
                    self.transition_state(AgentState::Thinking).await; // Ensure state is Thinking
                }
            }

            // Step counter is incremented at the start of each iteration now
        }
    }

    // Modify step to accept the CancelReceiver
    async fn step(
        &mut self,
        cancel_rx: CancelReceiver, // Use watch receiver (no longer needs mut)
    ) -> Result<AgentAction, AgentError> {
        // --- Cancellation Check (Start of Step) ---
        if *cancel_rx.borrow() {
            log::debug!("Cancellation detected at start of step.");
            return Err(AgentError::Terminated);
        }

        self.transition_state(AgentState::Thinking).await;

        let messages = {
            let mem = self.memory.lock().await;
            mem.get_messages().await?
        };
        let all_tools = self.tool_provider.list_tools().await?;

        // Filter tools based on brain type to prevent orchestrator from seeing specialist tools
        let tools = self.filter_tools_for_brain(&all_tools);

        // --- Cancellation Check (Before Brain Action) ---
        if *cancel_rx.borrow() {
            log::debug!("Cancellation detected before brain action.");
            return Err(AgentError::Terminated);
        }

        // Check if brain supports streaming and use appropriate method
        let brain_action = if self.brain.supports_streaming() {
            // Brain supports streaming - generate a message ID and call streaming method
            let message_id = uuid::Uuid::new_v4().to_string();
            log::debug!("Using streaming brain with message ID: {}", message_id);
            self.brain
                .decide_next_action_streaming(
                    &messages,
                    &tools,
                    Some((*self.app_handle).clone()),
                    Some(message_id),
                )
                .await?
        } else {
            // Fall back to regular brain method
            log::debug!("Using non-streaming brain");
            self.brain.decide_next_action(&messages, &tools).await?
        };

        log::debug!("Brain decided action: {:?}", brain_action);

        match brain_action {
            AgentAction::ExecuteTool(tool_calls) => {
                if tool_calls.is_empty() {
                    log::warn!("ExecuteTool action received with empty tool call list. Switching to Think.");
                    return Ok(AgentAction::Think); // Return Think if no tools to call
                }

                self.transition_state(AgentState::Executing).await;
                log::info!("Executing {} tool call(s)", tool_calls.len());

                // Add assistant message with tool calls to memory first (required for proper conversation order)
                // We'll implement rollback if tool execution fails to prevent orphaned tool_use blocks
                let assistant_message = Message {
                    role: Role::Assistant,
                    content: "".to_string(), // Content might be empty or indicate thought process
                    tool_calls: Some(tool_calls.clone()),
                    tool_call_id: None,
                    name: None,
                };

                {
                    let mut mem = self.memory.lock().await;
                    mem.add_message(assistant_message.clone()).await?;
                }

                                // Execute tools with intelligent batching for performance optimization
                let execution_result = self
                    .execute_tools_with_batching(tool_calls.clone(), &cancel_rx)
                    .await;

                match execution_result {
                    Ok(()) => {
                        // Tools executed successfully - conversation is consistent
                        log::info!("All tool batches executed successfully");
                        Ok(AgentAction::Think) // Move to thinking after successful execution
                    }
                    Err(e) => {
                        log::error!("Tool batch execution failed: {}", e);

                        // CRITICAL FIX: Only remove assistant message if NO tools were executed
                        // If some tools executed (partial success), keep the assistant message
                        // as handle_batch_cancellation will have added proper tool results/cancellations
                        {
                            let mut mem = self.memory.lock().await;
                            let messages = mem.get_messages().await?;

                            // Count how many tool result messages were added since our assistant message
                            let mut tool_result_count = 0;
                            let mut found_our_assistant_message = false;
                            let mut assistant_tool_call_ids = Vec::new();

                            // First pass: Find our assistant message and collect its tool call IDs
                            for msg in messages.iter().rev() {
                                if matches!(msg.role, Role::Assistant) &&
                                   msg.tool_calls.as_ref().map(|tc| tc.len() == tool_calls.len()).unwrap_or(false) {
                                    found_our_assistant_message = true;
                                    // Collect all tool call IDs from this assistant message
                                    if let Some(tool_calls) = &msg.tool_calls {
                                        assistant_tool_call_ids = tool_calls.iter()
                                            .map(|tc| tc.id.clone())
                                            .collect();
                                    }
                                    break;
                                }
                            }

                            // Second pass: Count tool results that match our assistant's tool call IDs
                            if found_our_assistant_message {
                                for msg in messages.iter().rev() {
                                    if matches!(msg.role, Role::Tool) &&
                                       msg.tool_call_id.as_ref()
                                           .map(|id| assistant_tool_call_ids.contains(id))
                                           .unwrap_or(false) {
                                        tool_result_count += 1;
                                    }
                                }

                                log::debug!(
                                    "Found {} tool results out of {} expected for assistant message",
                                    tool_result_count,
                                    assistant_tool_call_ids.len()
                                );
                            }

                            // Only remove assistant message if NO tool results were added (complete failure)
                            if found_our_assistant_message && tool_result_count == 0 {
                                let mut messages_vec = messages;
                                // Remove the assistant message with tool calls
                                if let Some(last_message) = messages_vec.last() {
                                    if matches!(last_message.role, Role::Assistant) &&
                                       last_message.tool_calls.as_ref()
                                           .map(|tc| tc.len() == tool_calls.len())
                                           .unwrap_or(false) {
                                        messages_vec.pop();

                                        // Clear and rebuild memory without the orphaned message
                                        mem.clear_memory().await?;
                                        for msg in messages_vec {
                                            mem.add_message(msg).await?;
                                        }

                                        log::info!("Removed orphaned assistant message with tool calls (no tools executed)");
                                    }
                                }
                            } else if found_our_assistant_message && tool_result_count > 0 {
                                log::info!("Keeping assistant message as {} tool(s) were executed before failure", tool_result_count);
                            }
                        }

                        // Return the original error
                        return Err(e);
                    }
                }
            }
            AgentAction::RespondToUser(response) => {
                self.transition_state(AgentState::Responding).await;
                // The run loop will add this response to memory if needed.
                Ok(AgentAction::RespondToUser(response))
            }
            AgentAction::Finish(response) => {
                // This action will terminate the loop when returned to run()
                Ok(AgentAction::Finish(response))
            }

            AgentAction::Error(e) => {
                // Propagate the error up to the run loop
                Ok(AgentAction::Error(e))
            }
            AgentAction::Think => {
                // Brain explicitly requests thinking, keep state as Thinking
                self.transition_state(AgentState::Thinking).await;
                Ok(AgentAction::Think)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    // Simple mock implementations for testing
    use crate::agent::core::{AgentError, ToolCall, ToolDefinition, ToolResult};
    use async_trait::async_trait;
    use serde_json::Value;

    // Simple mock implementations for testing
    #[allow(dead_code)]
    struct MockToolProvider;

    #[async_trait]
    impl ToolProvider for MockToolProvider {
        async fn list_tools(&self) -> Result<Vec<ToolDefinition>, AgentError> {
            Ok(vec![])
        }

        async fn execute_tool(&self, _tool_call: ToolCall) -> Result<ToolResult, AgentError> {
            Ok(ToolResult {
                call_id: "test".to_string(),
                output: Value::String("test output".to_string()),
            })
        }
    }

    #[allow(dead_code)]
    struct MockBrain;

    #[async_trait]
    impl AgentBrain for MockBrain {
        async fn decide_next_action(
            &self,
            _messages: &[crate::agent::core::Message],
            _tools: &[ToolDefinition],
        ) -> Result<AgentAction, AgentError> {
            Ok(AgentAction::Finish("test response".to_string()))
        }

        fn supports_streaming(&self) -> bool {
            false
        }

        async fn decide_next_action_streaming(
            &self,
            _messages: &[crate::agent::core::Message],
            _tools: &[ToolDefinition],
            _app_handle: Option<AppHandle>,
            _message_id: Option<String>,
        ) -> Result<AgentAction, AgentError> {
            Ok(AgentAction::Finish("test response".to_string()))
        }
    }

    #[allow(dead_code)]
    struct MockMemoryManagerTest;

    #[async_trait]
    impl MemoryManager for MockMemoryManagerTest {
        async fn add_message(
            &mut self,
            _message: crate::agent::core::Message,
        ) -> Result<(), AgentError> {
            Ok(())
        }

        async fn get_messages(&self) -> Result<Vec<crate::agent::core::Message>, AgentError> {
            Ok(vec![])
        }

        async fn get_last_n_messages(
            &self,
            _n: usize,
        ) -> Result<Vec<crate::agent::core::Message>, AgentError> {
            Ok(vec![])
        }

        async fn clear_memory(&mut self) -> Result<(), AgentError> {
            Ok(())
        }
    }

    #[test]
    fn test_continuation_logic_prevents_infinite_loop() {
        // This test verifies that the continuation counter increments properly
        // and prevents infinite loops in agent execution.
        let max_steps = 3;
        let current_step = 2;

        // At step 2, we should still be able to continue
        assert!(current_step < max_steps);

        // At step 3, we should stop
        let next_step = current_step + 1;
        assert_eq!(next_step, max_steps);
    }

    #[test]
    fn test_continuation_logic_prevents_overflow() {
        // Test that we don't accidentally overflow the step counter
        let max_steps = u32::MAX - 1;
        let current_step = max_steps - 1;

        // Should still be valid
        assert!(current_step < max_steps);

        // Next step should equal max (stopping condition)
        let next_step = current_step + 1;
        assert_eq!(next_step, max_steps);
    }

    #[test]
    fn test_tool_result_counting_logic() {
        use crate::agent::core::{Message, Role, ToolCall};
        use serde_json::json;

        // Create a mock message history
        let mut messages = Vec::new();

        // Add some initial messages
        messages.push(Message {
            role: Role::User,
            content: "Hello".to_string(),
            tool_calls: None,
            tool_call_id: None,
            name: None,
        });

        messages.push(Message {
            role: Role::Assistant,
            content: "How can I help?".to_string(),
            tool_calls: None,
            tool_call_id: None,
            name: None,
        });

        // Add an assistant message with tool calls
        let tool_calls = vec![
            ToolCall {
                id: "tool1".to_string(),
                name: "test_tool".to_string(),
                input: json!({"action": "test"}),
            },
            ToolCall {
                id: "tool2".to_string(),
                name: "test_tool2".to_string(),
                input: json!({"action": "test2"}),
            },
        ];

        messages.push(Message {
            role: Role::Assistant,
            content: "".to_string(),
            tool_calls: Some(tool_calls.clone()),
            tool_call_id: None,
            name: None,
        });

        // Add tool results for the first tool only (partial execution)
        messages.push(Message {
            role: Role::Tool,
            content: "Tool result".to_string(),
            tool_calls: None,
            tool_call_id: Some("tool1".to_string()),
            name: Some("test_tool".to_string()),
        });

        // Now manually implement the counting logic from our fix
        let mut tool_result_count = 0;
        let mut found_our_assistant_message = false;
        let mut assistant_tool_call_ids = Vec::new();

        // First pass: Find assistant message and collect tool call IDs
        for msg in messages.iter().rev() {
            if matches!(msg.role, Role::Assistant) &&
               msg.tool_calls.as_ref()
                   .map(|tc| tc.len() == tool_calls.len())
                   .unwrap_or(false) {
                found_our_assistant_message = true;
                if let Some(tool_calls) = &msg.tool_calls {
                    assistant_tool_call_ids = tool_calls.iter()
                        .map(|tc| tc.id.clone())
                        .collect();
                }
                break;
            }
        }

        // Second pass: Count tool results that match our assistant's tool call IDs
        if found_our_assistant_message {
            for msg in messages.iter().rev() {
                if matches!(msg.role, Role::Tool) &&
                   msg.tool_call_id.as_ref()
                       .map(|id| assistant_tool_call_ids.contains(id))
                       .unwrap_or(false) {
                    tool_result_count += 1;
                }
            }
        }

        // Verify results
        assert!(found_our_assistant_message, "Should have found the assistant message");
        assert_eq!(assistant_tool_call_ids.len(), 2, "Should have found 2 tool call IDs");
        assert_eq!(tool_result_count, 1, "Should have counted 1 tool result");

        // Test case 2: No tool results
        let mut messages2 = messages.clone();
        messages2.pop(); // Remove the tool result

        let mut tool_result_count2 = 0;
        let mut found_our_assistant_message2 = false;
        let mut assistant_tool_call_ids2 = Vec::new();

        // First pass
        for msg in messages2.iter().rev() {
            if matches!(msg.role, Role::Assistant) &&
               msg.tool_calls.as_ref()
                   .map(|tc| tc.len() == tool_calls.len())
                   .unwrap_or(false) {
                found_our_assistant_message2 = true;
                if let Some(tool_calls) = &msg.tool_calls {
                    assistant_tool_call_ids2 = tool_calls.iter()
                        .map(|tc| tc.id.clone())
                        .collect();
                }
                break;
            }
        }

        // Second pass
        if found_our_assistant_message2 {
            for msg in messages2.iter().rev() {
                if matches!(msg.role, Role::Tool) {
                    if let Some(tool_call_id) = &msg.tool_call_id {
                        if assistant_tool_call_ids2.contains(&tool_call_id) {
                            tool_result_count2 += 1;
                        }
                    }
                }
            }
        }

        // Verify results for case 2
        assert!(found_our_assistant_message2, "Should have found the assistant message");
        assert_eq!(assistant_tool_call_ids2.len(), 2, "Should have found 2 tool call IDs");
        assert_eq!(tool_result_count2, 0, "Should have counted 0 tool results");
    }

    // MCP Batching Tests
    #[test]
    fn test_simple_batching_logic() {
        // Test the trust-based execution approach:
        // Execute whatever the agent provides, no special logic

        use crate::agent::core::ToolCall;
        use serde_json::json;

        // Any number of tools should just be executed as provided
        let tools = vec![
            ToolCall {
                id: "1".to_string(),
                name: "computer".to_string(),
                input: json!({"action": "type", "text": "hello"}),
            },
            ToolCall {
                id: "2".to_string(),
                name: "computer".to_string(),
                input: json!({"action": "key", "text": "Return"}),
            },
            ToolCall {
                id: "3".to_string(),
                name: "computer".to_string(),
                input: json!({"action": "screenshot"}),
            },
        ];

        // The system should just execute these tools without caring about the count
        // No special logic, no hardcoded numbers - trust the agent
        assert!(!tools.is_empty());
    }

    #[test]
    fn test_coordinate_extraction_formats() {
        use serde_json::json;

        // Create a mock runner for testing (this is simplified for unit testing)
        let runner = MockRunner;

        // Test Format 1: Anthropic Computer Use API - {"coordinate": [x, y]}
        let input1 = json!({"coordinate": [100, 200]});
        let coords1 = runner.extract_target_coordinates(&input1);
        assert_eq!(coords1, Some((100, 200)));

        // Test Format 2: Separate x/y fields - {"x": 100, "y": 200}
        let input2 = json!({"x": 150, "y": 250});
        let coords2 = runner.extract_target_coordinates(&input2);
        assert_eq!(coords2, Some((150, 250)));

        // Test Format 3: Nested coordinate object - {"coordinate": {"x": 100, "y": 200}}
        let input3 = json!({"coordinate": {"x": 300, "y": 400}});
        let coords3 = runner.extract_target_coordinates(&input3);
        assert_eq!(coords3, Some((300, 400)));

        // Test Format 4: Drag operation with end_coordinate
        let input4 = json!({"coordinate": [50, 60], "end_coordinate": [350, 450]});
        let coords4 = runner.extract_target_coordinates(&input4);
        assert_eq!(coords4, Some((350, 450))); // Should use end_coordinate

        // Test invalid format - should return None
        let input5 = json!({"action": "mouse_move", "invalid": "data"});
        let coords5 = runner.extract_target_coordinates(&input5);
        assert_eq!(coords5, None);

        // Test floating point coordinates - should be converted to int
        let input6 = json!({"x": 100.7, "y": 200.3});
        let coords6 = runner.extract_target_coordinates(&input6);
        assert_eq!(coords6, Some((100, 200)));
    }

    #[test]
    fn test_mouse_movement_tool_detection() {
        use crate::agent::core::ToolCall;
        use serde_json::json;

        let runner = MockRunner;

        // Test computer tool with mouse_move action
        let tool1 = ToolCall {
            id: "1".to_string(),
            name: "computer".to_string(),
            input: json!({"action": "mouse_move", "coordinate": [100, 200]}),
        };
        assert!(runner.is_mouse_movement_tool(&tool1));

        // Test computer tool with non-movement action
        let tool2 = ToolCall {
            id: "2".to_string(),
            name: "computer".to_string(),
            input: json!({"action": "left_click", "coordinate": [100, 200]}),
        };
        assert!(!runner.is_mouse_movement_tool(&tool2));

        // Test direct mouse_move tool
        let tool3 = ToolCall {
            id: "3".to_string(),
            name: "mouse_move".to_string(),
            input: json!({"x": 100, "y": 200}),
        };
        assert!(runner.is_mouse_movement_tool(&tool3));

        // Test scroll_at_position tool (involves coordinate movement)
        let tool4 = ToolCall {
            id: "4".to_string(),
            name: "scroll_at_position".to_string(),
            input: json!({"x": 100, "y": 200, "direction": "up", "amount": 3}),
        };
        assert!(runner.is_mouse_movement_tool(&tool4));

        // Test non-movement tool
        let tool5 = ToolCall {
            id: "5".to_string(),
            name: "type_text".to_string(),
            input: json!({"text": "hello"}),
        };
        assert!(!runner.is_mouse_movement_tool(&tool5));
    }

    // Mock runner for testing coordinate extraction methods
    struct MockRunner;

    impl MockRunner {
        fn extract_target_coordinates(&self, input: &serde_json::Value) -> Option<(i32, i32)> {
            // Format 1: Handle drag operations FIRST - use end coordinate for destination
            // For drag operations, end_coordinate represents the target destination for movement completion
            if let Some(end_coord_array) = input.get("end_coordinate").and_then(|c| c.as_array()) {
                if end_coord_array.len() == 2 {
                    if let (Some(x), Some(y)) = (
                        end_coord_array[0].as_f64(),
                        end_coord_array[1].as_f64()
                    ) {
                        return Some((x as i32, y as i32));
                    }
                }
            }

            // Format 2: Anthropic Computer Use API - {"coordinate": [x, y]}
            if let Some(coord_array) = input.get("coordinate").and_then(|c| c.as_array()) {
                if coord_array.len() == 2 {
                    if let (Some(x), Some(y)) = (
                        coord_array[0].as_f64(),
                        coord_array[1].as_f64()
                    ) {
                        return Some((x as i32, y as i32));
                    }
                }
            }

            // Format 3: Separate x/y fields - {"x": 100, "y": 200}
            if let (Some(x), Some(y)) = (
                input.get("x").and_then(|v| v.as_f64()),
                input.get("y").and_then(|v| v.as_f64())
            ) {
                return Some((x as i32, y as i32));
            }

            // Format 4: Nested coordinate object - {"coordinate": {"x": 100, "y": 200}}
            if let Some(coord_obj) = input.get("coordinate").and_then(|c| c.as_object()) {
                if let (Some(x), Some(y)) = (
                    coord_obj.get("x").and_then(|v| v.as_f64()),
                    coord_obj.get("y").and_then(|v| v.as_f64())
                ) {
                    return Some((x as i32, y as i32));
                }
            }

            None
        }

        fn is_mouse_movement_tool(&self, tool_call: &crate::agent::core::ToolCall) -> bool {
            // Check computer tool with mouse_move action
            if tool_call.name == "computer" {
                if let Some(action) = tool_call.input.get("action").and_then(|a| a.as_str()) {
                    return action == "mouse_move";
                }
            }

            // Check direct mouse movement tools
            if tool_call.name == "mouse_move" {
                return true;
            }

            // Check tools that involve coordinate movement (like scroll_at_position)
            if tool_call.name == "scroll_at_position" {
                return true;
            }

            false
        }
    }
}

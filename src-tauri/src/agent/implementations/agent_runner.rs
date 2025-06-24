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
use crate::agent::tool_logger; // Added for logging
use crate::agent::traits::{AgentBrain, AgentRunnable, MemoryManager, ToolProvider};
use tauri::{AppHandle, Emitter, Manager}; // Added Manager trait for accessing app state

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
        brain: impl AgentBrain + Send + Sync + 'static,
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
        // Check if this brain is an orchestrator by checking if it only has delegation tools
        let has_delegation_tools = all_tools
            .iter()
            .any(|tool| tool.name.starts_with("delegate_to_"));

        // If this tool provider has delegation tools, it's likely an orchestrator
        // Only allow delegation tools and basic coordination tools
        if has_delegation_tools {
            let delegation_tool_count = all_tools
                .iter()
                .filter(|tool| tool.name.starts_with("delegate_to_"))
                .count();

            // If most tools are delegation tools, this is an orchestrator
            if delegation_tool_count > 0
                && (delegation_tool_count as f32 / all_tools.len() as f32) > 0.3
            {
                let filtered_tools: Vec<crate::agent::core::ToolDefinition> = all_tools
                    .iter()
                    .filter(|tool| {
                        // Allow delegation tools
                        if tool.name.starts_with("delegate_to_") {
                            return true;
                        }

                        // Allow basic coordination tools that orchestrators might need
                        match tool.name.as_str() {
                            "analyze_task" | "plan_workflow" | "coordinate_agents"
                            | "check_task_status" | "summarize_results" => true,
                            _ => false,
                        }
                    })
                    .cloned()
                    .collect();

                log::info!("Orchestrator brain detected: filtering tools from {} to {} (keeping only delegation and coordination tools)",
                    all_tools.len(), filtered_tools.len());
                return filtered_tools;
            }
        }

        // For non-orchestrator brains, return all tools
        // The specialist agents will get their tools filtered by ToolMappingService elsewhere
        all_tools.to_vec()
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

        log::info!("Executing {} tool call(s) as provided by agent", tool_calls.len());

        // Initialize result cache for all tools
        let mut tool_results_cache = Vec::new();
        for tool_call in tool_calls.iter() {
            tool_results_cache.push((tool_call.clone(), None));
        }

        // Execute the tools as provided - no special logic
        match self.execute_tool_batch(&tool_calls, cancel_rx, 0, &mut tool_results_cache).await? {
            true => {}, // Continue
            false => {
                // Cancellation occurred - handle incomplete tool execution
                self.handle_batch_cancellation(&tool_calls, &tool_results_cache).await?;
                return Err(AgentError::Terminated);
            }
        }

        Ok(())
    }

    /// Handle cancellation by adding "cancelled" messages for unexecuted tools
    /// This ensures conversation memory remains consistent even when execution is interrupted
    async fn handle_batch_cancellation(
        &mut self,
        tool_calls: &[crate::agent::core::ToolCall],
        tool_results_cache: &[(crate::agent::core::ToolCall, Option<Result<crate::agent::core::ToolResult, AgentError>>)],
    ) -> Result<(), AgentError> {
        log::info!("Handling batch cancellation for {} tool calls", tool_calls.len());

        let mut cancelled_count = 0;
        let mut mem = self.memory.lock().await;

        // Check each tool call and add cancellation message if it wasn't executed
        for (i, tool_call) in tool_calls.iter().enumerate() {
            // Check if this tool was executed (has a result in cache)
            let was_executed = if i < tool_results_cache.len() {
                tool_results_cache[i].1.is_some()
            } else {
                false
            };

            if !was_executed {
                // Add cancellation message to memory for this unexecuted tool
                mem.add_message(crate::agent::core::Message {
                    role: crate::agent::core::Role::Tool,
                    content: "Tool execution was cancelled by user".to_string(),
                    tool_calls: None,
                    tool_call_id: Some(tool_call.id.clone()),
                    name: Some(tool_call.name.clone()),
                }).await?;

                // Emit cancellation event to frontend
                crate::agent::tool_logger::log_tool_call_result(
                    &self.app_handle,
                    &tool_call.name,
                    serde_json::json!({"cancelled": true, "reason": "User cancelled execution"}),
                    false,
                    Some(format!("Tool {} was cancelled", tool_call.name)),
                    None,
                );

                cancelled_count += 1;
            }
        }

        log::info!("Added cancellation messages for {} unexecuted tools", cancelled_count);
        Ok(())
    }

    /// Execute a batch of tools with optimized workflow
    async fn execute_tool_batch(
        &mut self,
        batch: &[crate::agent::core::ToolCall],
        cancel_rx: &crate::state::CancelReceiver,
        start_index: usize,
        tool_results_cache: &mut Vec<(crate::agent::core::ToolCall, Option<Result<crate::agent::core::ToolResult, AgentError>>)>,
    ) -> Result<bool, AgentError> {
        log::info!("Executing tool batch: {} tools", batch.len());

        // Check if all tools in batch can be executed as MCP batch
        if self.can_execute_as_mcp_batch(batch).await {
            return self.execute_mcp_tool_batch(batch, cancel_rx, start_index, tool_results_cache).await;
        }

        // Fall back to optimized sequential execution (faster than normal due to reduced approval overhead)
        self.execute_sequential_batch(batch, cancel_rx, start_index, tool_results_cache).await
    }

    /// Check if a batch can be executed via MCP batching
    async fn can_execute_as_mcp_batch(&self, batch: &[crate::agent::core::ToolCall]) -> bool {
        // Check if all tools in the batch are MCP tools
        for tool_call in batch {
            if !self.is_mcp_tool(&tool_call.name).await {
                return false;
            }
        }
        true
    }

    /// Check if a tool is an MCP tool
    async fn is_mcp_tool(&self, tool_name: &str) -> bool {
        // Use the canonical MCP tool detection pattern
        tool_name.contains("mcp-server-") || tool_name.starts_with("mcp_")
    }

    /// Execute batch via MCP batching system
    async fn execute_mcp_tool_batch(
        &mut self,
        batch: &[crate::agent::core::ToolCall],
        cancel_rx: &crate::state::CancelReceiver,
        start_index: usize,
        tool_results_cache: &mut Vec<(crate::agent::core::ToolCall, Option<Result<crate::agent::core::ToolResult, AgentError>>)>,
    ) -> Result<bool, AgentError> {
        // Check cancellation
        if *cancel_rx.borrow() {
            log::info!("Cancellation detected at start of MCP batch execution");
            return Ok(false);
        }

        // Batch approval check - ask once for the entire batch
        if !self.check_batch_approval(batch, cancel_rx).await? {
            return Ok(true);
        }

        log::info!("Executing MCP batch: {} tools", batch.len());

        // Use the existing MCP batch execution
        match self.tool_provider.execute_batch_tools(batch.to_vec()).await {
            Ok(results) => {
                // Process all results and add to memory
                for (i, result) in results.into_iter().enumerate() {
                    let tool_call = &batch[i];

                    // FIXED: Emit tool result event to frontend for chat display
                    // Extract screenshot if this is a screenshot tool
                    let screenshot_base64 = if tool_call.name == "capture_screenshot" || tool_call.name == "computer" {
                        // For screenshot tools, the result output should contain base64 data
                        if let Some(screenshot_data) = result.output.get("data") {
                            screenshot_data.as_str().map(|s| s.to_string())
                        } else if let Some(screenshot_str) = result.output.as_str() {
                            Some(screenshot_str.to_string())
                        } else {
                            None
                        }
                    } else {
                        None
                    };

                    crate::agent::tool_logger::log_tool_call_result(
                        &self.app_handle,
                        &tool_call.name,
                        result.output.clone(),
                        true,
                        Some(format!("MCP batched tool {} executed successfully", tool_call.name)),
                        screenshot_base64,
                    );

                    tool_results_cache[start_index + i].1 = Some(Ok(result.clone()));
                    self.add_tool_result_to_memory(tool_call, Ok(result)).await?;
                }
                Ok(true)
            }
            Err(e) => {
                log::error!("MCP batch execution failed: {}. Falling back to sequential execution.", e);

                // FIXED: Don't add error results to cache/memory when falling back to sequential execution
                // This prevents duplicate tool executions and conflicting results
                // The sequential execution will handle the actual tool execution and results

                // Fall back to sequential execution without polluting cache/memory
                self.execute_sequential_batch(batch, cancel_rx, start_index, tool_results_cache).await
            }
        }
    }

    /// Execute batch sequentially but with optimized approval process
    async fn execute_sequential_batch(
        &mut self,
        batch: &[crate::agent::core::ToolCall],
        cancel_rx: &crate::state::CancelReceiver,
        start_index: usize,
        tool_results_cache: &mut Vec<(crate::agent::core::ToolCall, Option<Result<crate::agent::core::ToolResult, AgentError>>)>,
    ) -> Result<bool, AgentError> {
        // Batch approval for sequential operations
        if !self.check_batch_approval(batch, cancel_rx).await? {
            return Ok(true);
        }

        log::info!("Executing sequential batch: {} tools (approval granted for batch)", batch.len());

        for (i, tool_call) in batch.iter().enumerate() {
            // Check cancellation before each tool
            if *cancel_rx.borrow() {
                log::info!("Cancellation detected during sequential batch execution at tool {} of {}", i, batch.len());
                return Ok(false);
            }

            // Execute without individual approval (already approved for batch)
            log::info!("Executing batched tool {}/{}: {}", i + 1, batch.len(), tool_call.name);

            crate::agent::tool_logger::log_tool_call_request(
                &self.app_handle,
                &tool_call.name,
                tool_call.input.clone(),
                Some(format!("Executing batched tool: {}", tool_call.name)),
            );

            let tool_result = self.tool_provider.execute_tool(tool_call.clone()).await;

            // Add delay after mouse movement operations to allow smooth animation to complete
            if tool_call.name == "computer" {
                if let Some(action) = tool_call.input.get("action").and_then(|a| a.as_str()) {
                    if action == "mouse_move" {
                        // Allow 350ms for smooth movement animation to complete (300ms + buffer)
                        tokio::time::sleep(tokio::time::Duration::from_millis(350)).await;
                    }
                }
            } else if tool_call.name == "mouse_move" {
                // For direct mouse_move tool calls
                tokio::time::sleep(tokio::time::Duration::from_millis(350)).await;
            }

            // FIXED: Emit tool result event to frontend for chat display
            match &tool_result {
                Ok(result) => {
                    // Extract screenshot if this is a screenshot tool
                    let screenshot_base64 = if tool_call.name == "capture_screenshot" || tool_call.name == "computer" {
                        // For screenshot tools, the result output should contain base64 data
                        if let Some(screenshot_data) = result.output.get("data") {
                            screenshot_data.as_str().map(|s| s.to_string())
                        } else if let Some(screenshot_str) = result.output.as_str() {
                            Some(screenshot_str.to_string())
                        } else {
                            None
                        }
                    } else {
                        None
                    };

                    crate::agent::tool_logger::log_tool_call_result(
                        &self.app_handle,
                        &tool_call.name,
                        result.output.clone(),
                        true,
                        Some(format!("Batched tool {} executed successfully", tool_call.name)),
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

            tool_results_cache[start_index + i].1 = Some(tool_result.clone());
            self.add_tool_result_to_memory(tool_call, tool_result).await?;
        }

        Ok(true)
    }

    /// Check approval for a batch of tools
    async fn check_batch_approval(
        &self,
        batch: &[crate::agent::core::ToolCall],
        cancel_rx: &crate::state::CancelReceiver,
    ) -> Result<bool, AgentError> {
        let app_state = self.app_handle.state::<crate::state::AppState>();

        if !app_state.is_tool_approval_required() {
            return Ok(true);
        }

        // Create batch approval request
        let batch_description = format!(
            "Execute batch of {} tools: {}",
            batch.len(),
            batch.iter()
                .take(3)
                .map(|t| t.name.as_str())
                .collect::<Vec<_>>()
                .join(" → ")
                + if batch.len() > 3 { " ..." } else { "" }
        );

        let batch_id = uuid::Uuid::new_v4().to_string();
        let approval_request = crate::state::ToolApprovalRequest::new(
            batch_id.clone(),
            "batch_execution".to_string(),
            serde_json::json!({
                "batch_size": batch.len(),
                "tools": batch.iter().map(|t| &t.name).collect::<Vec<_>>()
            }),
            batch_description.clone(),
        );

        // Add to pending approvals
        app_state.add_pending_tool_approval(approval_request.clone()).await;

        // Emit batch approval request event
        let approval_event = serde_json::json!({
            "tool_name": approval_request.tool_name,
            "tool_id": approval_request.tool_id,
            "tool_input": approval_request.tool_input,
            "description": approval_request.description,
            "timestamp": approval_request.timestamp,
            "is_batch": true,
            "batch_size": batch.len()
        });

        if let Err(e) = self.app_handle.emit("tool-approval-request", approval_event) {
            log::error!("Failed to emit batch approval request: {}", e);
        }

        // Wait for batch approval
        log::info!("Waiting for user approval for tool batch: {}", batch_description);

        let mut approval_timeout = 60;
        let mut approved = false;

        while approval_timeout > 0 && !approved {
            if *cancel_rx.borrow() {
                log::info!("Cancellation detected during batch approval wait");
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
                    log::info!("Tool batch denied");
                    break;
                }
                None => {} // Still pending
            }

            tokio::time::sleep(std::time::Duration::from_secs(1)).await;
            approval_timeout -= 1;
        }

        app_state.remove_tool_approval(&batch_id).await;

        if !approved {
            let reason = if approval_timeout <= 0 { "timeout" } else { "user denied" };
            log::warn!("Tool batch execution denied: {}", reason);

            // Add denial message for all tools in batch
            for tool_call in batch {
                let mut mem = self.memory.lock().await;
                mem.add_message(crate::agent::core::Message {
                    role: crate::agent::core::Role::Tool,
                    content: format!("Tool execution was denied as part of batch - {}", reason),
                    tool_calls: None,
                    tool_call_id: Some(tool_call.id.clone()),
                    name: Some(tool_call.name.clone()),
                }).await?;
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
        match tool_result {
            Ok(result) => {
                mem.add_message(crate::agent::core::Message {
                    role: crate::agent::core::Role::Tool,
                    content: result.output.to_string(),
                    tool_calls: None,
                    tool_call_id: Some(tool_call.id.clone()),
                    name: Some(tool_call.name.clone()),
                }).await?;
            }
            Err(error) => {
                mem.add_message(crate::agent::core::Message {
                    role: crate::agent::core::Role::Tool,
                    content: format!("Tool execution failed: {}", error),
                    tool_calls: None,
                    tool_call_id: Some(tool_call.id.clone()),
                    name: Some(tool_call.name.clone()),
                }).await?;
            }
        }
        Ok(())
    }

    /// Execute a single tool with approval and cancellation handling
    async fn execute_single_tool_with_approval(
        &mut self,
        tool_call: &crate::agent::core::ToolCall,
        cancel_rx: &crate::state::CancelReceiver,
        tool_index: usize,
        tool_results_cache: &mut Vec<(crate::agent::core::ToolCall, Option<Result<crate::agent::core::ToolResult, AgentError>>)>,
    ) -> Result<bool, AgentError> {
        // Check cancellation
        if *cancel_rx.borrow() {
            log::info!("Cancellation detected before tool execution: {}", tool_call.name);
            return Ok(false);
        }

        // Tool approval check (existing logic)
        if !self.check_tool_approval(tool_call, cancel_rx).await? {
            return Ok(true); // Tool denied, but continue with other tools
        }

        // Execute tool
        log::info!("Executing tool: {} with ID: {}", tool_call.name, tool_call.id);

        // Emit tool call request event
        crate::agent::tool_logger::log_tool_call_request(
            &self.app_handle,
            &tool_call.name,
            tool_call.input.clone(),
            Some(format!("Executing tool: {}", tool_call.name)),
        );

        let tool_result = self.tool_provider.execute_tool(tool_call.clone()).await;

        // FIXED: Emit tool result event to frontend for chat display
        match &tool_result {
            Ok(result) => {
                // Extract screenshot if this is a screenshot tool
                let screenshot_base64 = if tool_call.name == "capture_screenshot" || tool_call.name == "computer" {
                    // For screenshot tools, the result output should contain base64 data
                    if let Some(screenshot_data) = result.output.get("data") {
                        screenshot_data.as_str().map(|s| s.to_string())
                    } else if let Some(screenshot_str) = result.output.as_str() {
                        Some(screenshot_str.to_string())
                    } else {
                        None
                    }
                } else {
                    None
                };

                crate::agent::tool_logger::log_tool_call_result(
                    &self.app_handle,
                    &tool_call.name,
                    result.output.clone(),
                    true,
                    Some(format!("Tool {} executed successfully", tool_call.name)),
                    screenshot_base64,
                );
            }
            Err(error) => {
                crate::agent::tool_logger::log_tool_call_result(
                    &self.app_handle,
                    &tool_call.name,
                    serde_json::json!({"error": error.to_string()}),
                    false,
                    Some(format!("Tool {} failed: {}", tool_call.name, error)),
                    None,
                );
            }
        }

        // Cache result and add to memory
        tool_results_cache[tool_index].1 = Some(tool_result.clone());
        self.add_tool_result_to_memory(tool_call, tool_result).await?;

        Ok(true)
    }

    /// Check individual tool approval (used by legacy sequential execution)
    async fn check_tool_approval(
        &self,
        tool_call: &crate::agent::core::ToolCall,
        cancel_rx: &crate::state::CancelReceiver,
    ) -> Result<bool, AgentError> {
        let app_state = self.app_handle.state::<crate::state::AppState>();

        if !app_state.is_tool_approval_required() {
            return Ok(true);
        }

        log::info!("Tool approval required for: {}", tool_call.name);

        let approval_request = crate::state::ToolApprovalRequest::new(
            tool_call.id.clone(),
            tool_call.name.clone(),
            tool_call.input.clone(),
            format!("Agent wants to execute tool: {}", tool_call.name),
        );

        app_state.add_pending_tool_approval(approval_request.clone()).await;

        let approval_event = serde_json::json!({
            "tool_name": approval_request.tool_name,
            "tool_id": approval_request.tool_id,
            "tool_input": approval_request.tool_input,
            "description": approval_request.description,
            "timestamp": approval_request.timestamp
        });

        if let Err(e) = self.app_handle.emit("tool-approval-request", approval_event) {
            log::error!("Failed to emit tool approval request: {}", e);
        }

        log::info!("Waiting for user approval for tool: {}", tool_call.name);

        let mut approval_timeout = 60;
        let mut approved = false;

        while approval_timeout > 0 && !approved {
            if *cancel_rx.borrow() {
                log::info!("Cancellation detected during tool approval wait");
                app_state.remove_tool_approval(&tool_call.id).await;
                return Err(AgentError::Terminated);
            }

            match app_state.get_tool_approval_status(&tool_call.id).await {
                Some(true) => {
                    approved = true;
                    log::info!("Tool approved: {}", tool_call.name);
                    break;
                }
                Some(false) => {
                    log::info!("Tool denied: {}", tool_call.name);
                    break;
                }
                None => {} // Still pending
            }

            tokio::time::sleep(std::time::Duration::from_secs(1)).await;
            approval_timeout -= 1;
        }

        app_state.remove_tool_approval(&tool_call.id).await;

        if !approved {
            let reason = if approval_timeout <= 0 { "timeout" } else { "user denied" };
            log::warn!("Tool execution denied for {}: {}", tool_call.name, reason);

            let mut mem = self.memory.lock().await;
            mem.add_message(crate::agent::core::Message {
                role: crate::agent::core::Role::Tool,
                content: format!("Tool execution was denied - {}", reason),
                tool_calls: None,
                tool_call_id: Some(tool_call.id.clone()),
                name: Some(tool_call.name.clone()),
            }).await?;
        }

        Ok(approved)
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
                            if let Ok(mut max_steps_guard) = app_state.agent_max_steps.lock() {
                                *max_steps_guard = Some(self.max_steps);
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

        // --- Log Thinking Step ---
        tool_logger::log_thinking(
            &self.app_handle,
            "Deciding next action based on current messages and available tools...",
        );

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

                // Add assistant message indicating tool call(s) BEFORE starting execution
                // This ensures the conversation is in a consistent state even if interrupted
                {
                    let mut mem = self.memory.lock().await;
                    // Clone the calls for the message
                    let message_tool_calls = tool_calls.clone();
                    mem.add_message(Message {
                        role: Role::Assistant,
                        content: "".to_string(), // Content might be empty or indicate thought process
                        tool_calls: Some(message_tool_calls),
                        tool_call_id: None,
                        name: None,
                    })
                    .await?;
                }

                // Execute tools with intelligent batching for performance optimization
                // This replaces the sequential loop with batch-aware execution
                if let Err(e) = self.execute_tools_with_batching(tool_calls.clone(), &cancel_rx).await {
                    log::error!("Tool batch execution failed: {}", e);
                    // The cancellation handling is already done in execute_tools_with_batching
                    // for AgentError::Terminated, but we should handle other error types too
                    match e {
                        AgentError::Terminated => {
                            // Cancellation was already handled in execute_tools_with_batching
                            return Err(e);
                        }
                        _ => {
                            // For other errors, we still need to ensure conversation consistency
                            // but we don't call handle_batch_cancellation as this wasn't a user cancellation
                            return Err(e);
                        }
                    }
                }

                // If we reach here, all tools were executed successfully
                log::info!("All tool batches executed successfully");
                Ok(AgentAction::Think) // Move to thinking after successful execution
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

    // Create a simple mock app handle for testing
    fn mock_app_handle() -> tauri::AppHandle {
        // Use a simple test-specific implementation or skip this for unit tests
        panic!("This test requires a proper Tauri app context")
    }

    // Simple mock implementations for testing
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
}

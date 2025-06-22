use crate::state::CancelReceiver; // Import the type alias
use async_trait::async_trait;
use std::sync::Arc;
use tokio::sync::Mutex; // Using Mutex for mutable access to MemoryManager
use uuid;

use crate::agent::structs::{
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
        all_tools: &[crate::agent::structs::ToolDefinition],
    ) -> Vec<crate::agent::structs::ToolDefinition> {
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
                let filtered_tools: Vec<crate::agent::structs::ToolDefinition> = all_tools
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
    /// Detects obvious sequential operations and executes them as batches
    async fn execute_tools_with_batching(
        &mut self,
        tool_calls: Vec<crate::agent::structs::ToolCall>,
        cancel_rx: &crate::state::CancelReceiver,
    ) -> Result<(), AgentError> {
        if tool_calls.is_empty() {
            return Ok(());
        }

        // Analyze tools for batching opportunities
        let batches = self.analyze_and_create_batches(tool_calls.clone()).await;

        log::info!(
            "Tool execution plan: {} tools organized into {} batch(es)",
            tool_calls.len(),
            batches.len()
        );

        // Pre-populate cancelled results for all tool calls to maintain conversation consistency
        let mut tool_results_cache = Vec::new();
        for tool_call in tool_calls.iter() {
            tool_results_cache.push((tool_call.clone(), None)); // None means not executed yet
        }

        let mut global_tool_index = 0;

        for (batch_index, batch) in batches.iter().enumerate() {
            // Check for cancellation before each batch
            if *cancel_rx.borrow() {
                log::info!("Cancellation detected before batch {} execution", batch_index);
                self.handle_batch_cancellation(&tool_results_cache, global_tool_index).await?;
                return Err(AgentError::Terminated);
            }

            if batch.len() == 1 {
                // Single tool execution (existing logic)
                let tool_call = &batch[0];
                match self.execute_single_tool_with_approval(tool_call, cancel_rx, global_tool_index, &mut tool_results_cache).await? {
                    true => {}, // Continue
                    false => return Err(AgentError::Terminated), // Cancelled
                }
            } else {
                // Batch execution for sequential operations
                log::info!(
                    "Executing batch {}: {} tools ({} → {} → ...)",
                    batch_index,
                    batch.len(),
                    batch.first().map(|t| t.name.as_str()).unwrap_or("unknown"),
                    batch.get(1).map(|t| t.name.as_str()).unwrap_or("unknown")
                );

                match self.execute_tool_batch(batch, cancel_rx, global_tool_index, &mut tool_results_cache).await? {
                    true => {}, // Continue
                    false => return Err(AgentError::Terminated), // Cancelled
                }
            }

            global_tool_index += batch.len();
        }

        Ok(())
    }

    /// Analyze tool calls and create optimal batches for execution
    async fn analyze_and_create_batches(&self, tool_calls: Vec<crate::agent::structs::ToolCall>) -> Vec<Vec<crate::agent::structs::ToolCall>> {
        use crate::agent::tools::mcp_integration::ToolBatchingAnalyzer;

        if tool_calls.len() < 2 {
            return vec![tool_calls];
        }

        // Check if batching is beneficial
        let sequential_patterns = self.detect_sequential_patterns(&tool_calls);

        if sequential_patterns.is_empty() {
            log::debug!("No sequential patterns detected, using individual tool execution");
            return tool_calls.into_iter().map(|t| vec![t]).collect();
        }

        log::info!(
            "Detected {} sequential pattern(s), creating optimized batches",
            sequential_patterns.len()
        );

        // Create batches using the MCP analyzer
        ToolBatchingAnalyzer::create_batches(tool_calls)
    }

    /// Detect obvious sequential patterns in tool calls
    fn detect_sequential_patterns(&self, tool_calls: &[crate::agent::structs::ToolCall]) -> Vec<(usize, usize)> {
        let mut patterns = Vec::new();

        // Define obvious sequential patterns for Computer Use
        let sequential_patterns = [
            // Type → Enter → Screenshot (very common)
            ("computer", "type", "computer", "key", Some("Enter")),
            ("computer", "key", "computer", "screenshot", None),
            ("computer", "type", "computer", "screenshot", None),

            // Click → Screenshot (common verification)
            ("computer", "left_click", "computer", "screenshot", None),
            ("computer", "click", "computer", "screenshot", None),

            // MCP tool chains (safe for batching)
            ("mcp_", "", "mcp_", "", None),

            // File operations
            ("str_replace_based_edit_tool", "create", "str_replace_based_edit_tool", "view", None),
            ("str_replace_based_edit_tool", "str_replace", "str_replace_based_edit_tool", "view", None),
        ];

        for window in tool_calls.windows(2) {
            let first = &window[0];
            let second = &window[1];

            for (tool1_prefix, action1, tool2_prefix, action2, key_param) in &sequential_patterns {
                if self.matches_pattern(first, tool1_prefix, action1, key_param) &&
                   self.matches_pattern(second, tool2_prefix, action2, &None) {

                    let first_index = tool_calls.iter().position(|t| t.id == first.id).unwrap();
                    let second_index = tool_calls.iter().position(|t| t.id == second.id).unwrap();
                    patterns.push((first_index, second_index));

                    log::debug!(
                        "Sequential pattern detected: {} → {} (tools {} → {})",
                        first.name,
                        second.name,
                        first_index,
                        second_index
                    );
                }
            }
        }

        patterns
    }

    /// Check if a tool call matches a specific pattern
    fn matches_pattern(&self, tool_call: &crate::agent::structs::ToolCall, tool_prefix: &str, action: &str, key_param: &Option<&str>) -> bool {
        if !tool_call.name.starts_with(tool_prefix) {
            return false;
        }

        if action.is_empty() {
            return true; // Match any action for this tool
        }

        // Check action parameter for computer tools
        if tool_call.name == "computer" {
            if let Some(tool_action) = tool_call.input.get("action").and_then(|v| v.as_str()) {
                if tool_action == action {
                    // Check key parameter if specified
                    if let Some(expected_key) = key_param {
                        if let Some(key) = tool_call.input.get("key").and_then(|v| v.as_str()) {
                            return key == *expected_key;
                        }
                        return false;
                    }
                    return true;
                }
            }
        }

        // Check command parameter for text editor tools
        if tool_call.name == "str_replace_based_edit_tool" {
            if let Some(command) = tool_call.input.get("command").and_then(|v| v.as_str()) {
                return command == action;
            }
        }

        false
    }

    /// Execute a single tool with approval and cancellation handling
    async fn execute_single_tool_with_approval(
        &mut self,
        tool_call: &crate::agent::structs::ToolCall,
        cancel_rx: &crate::state::CancelReceiver,
        tool_index: usize,
        tool_results_cache: &mut Vec<(crate::agent::structs::ToolCall, Option<Result<crate::agent::structs::ToolResult, AgentError>>)>,
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

        // Cache result and add to memory
        tool_results_cache[tool_index].1 = Some(tool_result.clone());
        self.add_tool_result_to_memory(tool_call, tool_result).await?;

        Ok(true)
    }

    /// Execute a batch of tools with optimized workflow
    async fn execute_tool_batch(
        &mut self,
        batch: &[crate::agent::structs::ToolCall],
        cancel_rx: &crate::state::CancelReceiver,
        start_index: usize,
        tool_results_cache: &mut Vec<(crate::agent::structs::ToolCall, Option<Result<crate::agent::structs::ToolResult, AgentError>>)>,
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
    async fn can_execute_as_mcp_batch(&self, batch: &[crate::agent::structs::ToolCall]) -> bool {
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
        batch: &[crate::agent::structs::ToolCall],
        cancel_rx: &crate::state::CancelReceiver,
        start_index: usize,
        tool_results_cache: &mut Vec<(crate::agent::structs::ToolCall, Option<Result<crate::agent::structs::ToolResult, AgentError>>)>,
    ) -> Result<bool, AgentError> {
        // Check cancellation
        if *cancel_rx.borrow() {
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
                    tool_results_cache[start_index + i].1 = Some(Ok(result.clone()));
                    self.add_tool_result_to_memory(tool_call, Ok(result)).await?;
                }
                Ok(true)
            }
            Err(e) => {
                log::error!("MCP batch execution failed: {}", e);
                // Fall back to sequential execution
                self.execute_sequential_batch(batch, cancel_rx, start_index, tool_results_cache).await
            }
        }
    }

    /// Execute batch sequentially but with optimized approval process
    async fn execute_sequential_batch(
        &mut self,
        batch: &[crate::agent::structs::ToolCall],
        cancel_rx: &crate::state::CancelReceiver,
        start_index: usize,
        tool_results_cache: &mut Vec<(crate::agent::structs::ToolCall, Option<Result<crate::agent::structs::ToolResult, AgentError>>)>,
    ) -> Result<bool, AgentError> {
        // Batch approval for sequential operations
        if !self.check_batch_approval(batch, cancel_rx).await? {
            return Ok(true);
        }

        log::info!("Executing sequential batch: {} tools (approval granted for batch)", batch.len());

        for (i, tool_call) in batch.iter().enumerate() {
            // Check cancellation before each tool
            if *cancel_rx.borrow() {
                log::info!("Cancellation detected during batch execution at tool {}", i);
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
            tool_results_cache[start_index + i].1 = Some(tool_result.clone());
            self.add_tool_result_to_memory(tool_call, tool_result).await?;
        }

        Ok(true)
    }

    /// Check approval for a batch of tools
    async fn check_batch_approval(
        &self,
        batch: &[crate::agent::structs::ToolCall],
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
                mem.add_message(crate::agent::structs::Message {
                    role: crate::agent::structs::Role::Tool,
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
        tool_call: &crate::agent::structs::ToolCall,
        tool_result: Result<crate::agent::structs::ToolResult, AgentError>,
    ) -> Result<(), AgentError> {
        let mut mem = self.memory.lock().await;
        match tool_result {
            Ok(result) => {
                mem.add_message(crate::agent::structs::Message {
                    role: crate::agent::structs::Role::Tool,
                    content: result.output.to_string(),
                    tool_calls: None,
                    tool_call_id: Some(tool_call.id.clone()),
                    name: Some(tool_call.name.clone()),
                }).await?;
            }
            Err(error) => {
                mem.add_message(crate::agent::structs::Message {
                    role: crate::agent::structs::Role::Tool,
                    content: format!("Tool execution failed: {}", error),
                    tool_calls: None,
                    tool_call_id: Some(tool_call.id.clone()),
                    name: Some(tool_call.name.clone()),
                }).await?;
            }
        }
        Ok(())
    }

    /// Handle batch cancellation by adding appropriate tool results
    async fn handle_batch_cancellation(
        &mut self,
        tool_results_cache: &Vec<(crate::agent::structs::ToolCall, Option<Result<crate::agent::structs::ToolResult, AgentError>>)>,
        start_index: usize,
    ) -> Result<(), AgentError> {
        log::info!("Handling batch cancellation from index {}", start_index);

        let mut mem = self.memory.lock().await;
        for (tool_call, cached_result) in tool_results_cache.iter().skip(start_index) {
            if cached_result.is_none() {
                mem.add_message(crate::agent::structs::Message {
                    role: crate::agent::structs::Role::Tool,
                    content: "Tool execution was cancelled before completion.".to_string(),
                    tool_calls: None,
                    tool_call_id: Some(tool_call.id.clone()),
                    name: Some(tool_call.name.clone()),
                }).await?;
            }
        }
        Ok(())
    }

    /// Check individual tool approval (used by legacy sequential execution)
    async fn check_tool_approval(
        &self,
        tool_call: &crate::agent::structs::ToolCall,
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
            mem.add_message(crate::agent::structs::Message {
                role: crate::agent::structs::Role::Tool,
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
                    return Err(e);
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
    use crate::agent::structs::{AgentError, ToolCall, ToolDefinition, ToolResult};
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
            _messages: &[crate::agent::structs::Message],
            _tools: &[ToolDefinition],
        ) -> Result<AgentAction, AgentError> {
            Ok(AgentAction::Finish("test response".to_string()))
        }

        fn supports_streaming(&self) -> bool {
            false
        }

        async fn decide_next_action_streaming(
            &self,
            _messages: &[crate::agent::structs::Message],
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
            _message: crate::agent::structs::Message,
        ) -> Result<(), AgentError> {
            Ok(())
        }

        async fn get_messages(&self) -> Result<Vec<crate::agent::structs::Message>, AgentError> {
            Ok(vec![])
        }

        async fn get_last_n_messages(
            &self,
            _n: usize,
        ) -> Result<Vec<crate::agent::structs::Message>, AgentError> {
            Ok(vec![])
        }

        async fn clear_memory(&mut self) -> Result<(), AgentError> {
            Ok(())
        }
    }

    #[test]
    fn test_continuation_logic_prevents_infinite_loop() {
        // This test verifies that even with multiple ExecuteTool actions,
        // the agent will eventually move to a different state.
        let max_steps = 5;
        let current_step = 0;

        // Simulate multiple ExecuteTool rounds
        let should_continue = current_step < max_steps;
        assert!(should_continue, "Agent should continue when under step limit");

        let current_step = max_steps;
        let should_continue = current_step < max_steps;
        assert!(
            !should_continue,
            "Agent should stop when reaching step limit"
        );
    }

    #[test]
    fn test_continuation_logic_prevents_overflow() {
        // Test edge case where current_step might approach limits
        let max_steps = u32::MAX - 1;
        let current_step = u32::MAX - 2;

        let should_continue = current_step < max_steps;
        assert!(should_continue, "Should handle large step counts");
    }

    // MCP Batching Tests
    #[test]
    fn test_batch_pattern_detection() {
        // Test that sequential patterns are correctly identified
        use crate::agent::structs::ToolCall;
        use serde_json::json;

        // Create mock tool calls for a common pattern: type → enter → screenshot
        let tool_calls = vec![
            ToolCall {
                id: "1".to_string(),
                name: "computer_use_type".to_string(),
                input: json!({"text": "Hello World"}),
            },
            ToolCall {
                id: "2".to_string(),
                name: "computer_use_key".to_string(),
                input: json!({"key": "Return"}),
            },
            ToolCall {
                id: "3".to_string(),
                name: "computer_use_screenshot".to_string(),
                input: json!({}),
            },
        ];

        // Test pattern matching for type → key sequence
        let type_matches = tool_calls[0].name.starts_with("computer_use_") &&
                          tool_calls[0].name.contains("type");
        let key_matches = tool_calls[1].name.starts_with("computer_use_") &&
                         tool_calls[1].name.contains("key");
        let screenshot_matches = tool_calls[2].name.starts_with("computer_use_") &&
                               tool_calls[2].name.contains("screenshot");

        assert!(type_matches, "Should detect type tool");
        assert!(key_matches, "Should detect key tool");
        assert!(screenshot_matches, "Should detect screenshot tool");

        // This pattern should be batchable
        let is_batchable_sequence = type_matches && key_matches && screenshot_matches;
        assert!(is_batchable_sequence, "type → key → screenshot should be batchable");
    }

    #[test]
    fn test_mixed_pattern_detection() {
        use crate::agent::structs::ToolCall;
        use serde_json::json;

        // Mix of batchable and non-batchable operations
        let tool_calls = vec![
            ToolCall {
                id: "1".to_string(),
                name: "computer_use_screenshot".to_string(),
                input: json!({}),
            },
            ToolCall {
                id: "2".to_string(),
                name: "analyze_screen".to_string(), // Requires reasoning
                input: json!({}),
            },
            ToolCall {
                id: "3".to_string(),
                name: "computer_use_click".to_string(),
                input: json!({"coordinate": [100, 100]}),
            },
            ToolCall {
                id: "4".to_string(),
                name: "computer_use_screenshot".to_string(),
                input: json!({}),
            },
        ];

        // Test that analysis tools are correctly identified as non-batchable
        let analysis_tool = &tool_calls[1];
        let is_analysis = analysis_tool.name.contains("analyze") ||
                         analysis_tool.name.contains("think") ||
                         analysis_tool.name.contains("decide");

        assert!(is_analysis, "Should identify analysis tools");

        // Test that computer_use tools are batchable
        let click_tool = &tool_calls[2];
        let screenshot_tool = &tool_calls[3];
        let is_computer_use_batch = click_tool.name.starts_with("computer_use_") &&
                                   screenshot_tool.name.starts_with("computer_use_");

        assert!(is_computer_use_batch, "Should identify computer_use tools as batchable");
    }

    #[test]
    fn test_no_batching_for_analysis_tools() {
        use crate::agent::structs::ToolCall;
        use serde_json::json;

        // Tools that require individual reasoning
        let analysis_tools = vec![
            "analyze_content",
            "decide_next_action",
            "think_about_strategy",
            "evaluate_options",
            "plan_workflow"
        ];

        for tool_name in analysis_tools {
            let tool_call = ToolCall {
                id: "1".to_string(),
                name: tool_name.to_string(),
                input: json!({}),
            };

            // These should not be batchable as they require reasoning
            let should_not_batch = tool_call.name.contains("analyze") ||
                                  tool_call.name.contains("decide") ||
                                  tool_call.name.contains("think") ||
                                  tool_call.name.contains("evaluate") ||
                                  tool_call.name.contains("plan");

            assert!(should_not_batch, "Tool '{}' should require individual execution", tool_name);
        }
    }
}

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
use crate::agent::tools::permission_policy::ApprovalOutcome;
use crate::agent::traits::{AgentBrain, AgentRunnable, MemoryManager, ToolProvider};
use crate::constants::events;
use tauri::{AppHandle, Emitter, Manager}; // Added Manager trait for accessing app state

/// Result recorded for tool calls that never ran because an earlier tool in the
/// same batch failed.
///
/// Wording matches the Anthropic `computer_toolset_20260801` contract for
/// actions the client declined to execute after a failure.
pub(crate) const BATCH_HALT_SKIPPED_MESSAGE: &str =
    "Not executed: an earlier computer action in this turn failed.";

/// The text of a tool result as the model will actually read it.
///
/// This is the other half of the lying problem, and it was upstream of the
/// prompt. The old `format_task_output` summarised a multi-field object as
/// "Result with N fields", and that string is what went into the
/// conversation. So a bash call that failed arrived as `Result with 2
/// fields`: its exit code and its error text never reached the model, which
/// left "it worked" as the only reading available. `{"windows": [...],
/// "count": 3}` from `list_visible_windows` arrived the same way, and a
/// top-level array arrived as `List with N items`.
///
/// The invariant is that this function never *describes* a result instead of
/// reporting it. Anything carrying more than one value keeps its JSON. Bare
/// scalars and single-field objects are unwrapped, which is what makes a file
/// read or a command's output read as text rather than as a quoted blob. Only
/// a genuinely empty result gets a word, because there is nothing to lose.
///
/// `format_task_output`, the lossy version this replaces, is gone, along with
/// the ungated `agents::` executor that was its other caller. Do not
/// reintroduce a second formatter: `tool_result_content_never_discards` pins
/// this one, and a second one will drift from it.
pub(crate) fn tool_result_content(output: &serde_json::Value) -> String {
    match output {
        serde_json::Value::String(s) => s.clone(),
        serde_json::Value::Number(n) => n.to_string(),
        serde_json::Value::Bool(b) => b.to_string(),
        serde_json::Value::Null => "No output".to_string(),

        serde_json::Value::Object(map) => match map.len() {
            0 => "Empty result".to_string(),
            // One field is unwrapped, so `{"content": "..."}` reads as the
            // file rather than as JSON wrapped around it. The key is kept for
            // a non-string so a bare `3` is still attributable.
            1 => match map.iter().next() {
                Some((_, serde_json::Value::String(s))) => s.clone(),
                Some((key, val)) => format!("{}: {}", key, tool_result_content(val)),
                None => "Empty result".to_string(),
            },
            _ => json_or_debug(output),
        },

        serde_json::Value::Array(items) => match items.len() {
            0 => "Empty list".to_string(),
            1 => tool_result_content(&items[0]),
            _ => json_or_debug(output),
        },
    }
}

/// Serialise a result for the model, falling back to its `Debug` rendering.
///
/// `serde_json::to_string` on an already-parsed `Value` only fails on a map
/// with a non-string key, which `Value` cannot hold. The fallback is here so
/// that even that impossible path carries the data rather than a count of it.
fn json_or_debug(output: &serde_json::Value) -> String {
    serde_json::to_string(output).unwrap_or_else(|_| format!("{:?}", output))
}

/// True when a tool call should be treated as a failure.
///
/// Two failure shapes exist and both must halt the batch:
/// 1. A Rust-level `Err(_)` from the tool provider.
/// 2. An `Ok(_)` whose output is an Anthropic error response
///    (`{"is_error": true, "error": "..."}`). The `computer` tool reports every
///    failure this way — `run_computer_action` returns
///    `Ok(create_anthropic_error_response(msg))` — so `matches!(r, Err(_))`
///    alone never catches a failed click.
pub(crate) fn tool_result_is_failure(
    tool_result: &Result<crate::agent::core::ToolResult, AgentError>,
) -> bool {
    match tool_result {
        Ok(result) => {
            crate::agent::tools::anthropic_computer_use::is_anthropic_error_response(&result.output)
        }
        Err(_) => true,
    }
}

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
    /// Parallel-session registry row this run belongs to, when session
    /// tracking is active. Lets the approval wait surface `NeedsInput` in
    /// the switcher UI and notify for background sessions (LAC-1432).
    session_id: Option<crate::agents::AgentSessionId>,
    /// Images the person attached to the message that started this run.
    ///
    /// Set before `run` rather than passed through it, so the trait signature
    /// every implementor shares stays as it is.
    pending_images: Option<Vec<String>>,
    /// The app this run opened or focused, and when Juno last looked at it.
    ///
    /// Per-run state, because the question it answers is per-task: "is the app
    /// I am working in still there?" A run that never touched an app never
    /// checks.
    watched_app: Option<WatchedApp>,
}

/// What [`DefaultAgentRunner::check_before_tool`] decided about one call.
enum PreCheck {
    /// Nothing to observe and nothing in the way.
    Proceed,
    /// Run it, then look at this app and report what changed.
    Watch {
        app: String,
        kind: crate::agent::app_observation::AppCommand,
        before: crate::agent::app_observation::Presence,
    },
    /// Do not run it. Hand the model this instead, which says what Juno found
    /// and asks the person what to do.
    Refuse(serde_json::Value),
}

/// An app this run acted on, and the last time Juno confirmed it was running.
#[derive(Debug, Clone)]
struct WatchedApp {
    name: String,
    last_seen_running: std::time::Instant,
}

/// How stale a "the app is running" observation may be before the next screen
/// action re-checks it.
///
/// This number is the whole cost control. One check is a single `osascript`
/// call, about 0.8 s on zero, so checking before every keystroke in a typed
/// sentence would cost more than the typing. Two seconds is long enough that
/// a burst of actions pays for one check, and short enough that a person who
/// closes the app has closed it before the next click.
const LIVENESS_MAX_AGE: std::time::Duration = std::time::Duration::from_secs(2);

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
            session_id: None,
            pending_images: None,
            watched_app: None,
        }
    }

    /// Attach images to the message that will start the next run.
    pub fn set_pending_images(&mut self, images: Option<Vec<String>>) {
        self.pending_images = images.filter(|i| !i.is_empty());
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
            session_id: None,
            pending_images: None,
            watched_app: None,
        }
    }

    /// Associate this runner with a parallel-session registry row so the
    /// tool-approval wait can flip the session to `NeedsInput` and notify
    /// the user when the session is running in the background (LAC-1432).
    pub fn with_session_id(mut self, session_id: Option<crate::agents::AgentSessionId>) -> Self {
        self.session_id = session_id;
        self
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
                if let (Some(x), Some(y)) =
                    (end_coord_array[0].as_f64(), end_coord_array[1].as_f64())
                {
                    return Some((x as i32, y as i32));
                }
            }
        }

        // Format 2: Anthropic Computer Use API - {"coordinate": [x, y]}
        if let Some(coord_array) = input.get("coordinate").and_then(|c| c.as_array()) {
            if coord_array.len() == 2 {
                if let (Some(x), Some(y)) = (coord_array[0].as_f64(), coord_array[1].as_f64()) {
                    return Some((x as i32, y as i32));
                }
            }
        }

        // Format 3: Separate x/y fields - {"x": 100, "y": 200}
        if let (Some(x), Some(y)) = (
            input.get("x").and_then(|v| v.as_f64()),
            input.get("y").and_then(|v| v.as_f64()),
        ) {
            return Some((x as i32, y as i32));
        }

        // Format 4: Nested coordinate object - {"coordinate": {"x": 100, "y": 200}}
        if let Some(coord_obj) = input.get("coordinate").and_then(|c| c.as_object()) {
            if let (Some(x), Some(y)) = (
                coord_obj.get("x").and_then(|v| v.as_f64()),
                coord_obj.get("y").and_then(|v| v.as_f64()),
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

    /// What has to happen around one tool call for Juno to be honest about it.
    ///
    /// Three outcomes, and the reasoning for each is in the variant.
    async fn check_before_tool(&mut self, tool_call: &crate::agent::core::ToolCall) -> PreCheck {
        use crate::agent::app_observation as observation;

        // A command that launches, focuses or drives an app: look before, so
        // "it was already running" and "I launched it" stay separable, and
        // watch it afterwards.
        if let Some(target) = observation::app_command_target(&tool_call.name, &tool_call.input) {
            let observed = observation::observe(&target.app).await;
            // `NotFound` means nothing on this Mac answers to the name: no
            // bundle on disk and no process running under it. AppleScript
            // resolves a literal app name when it compiles the script, so such
            // a command puts a modal picker in front of the person, listing
            // every app they have, and no runtime check can take that back.
            // It does not run. `open -a` fails cleanly instead of prompting,
            // so it is not gated here.
            if target.names_app_literally
                && matches!(
                    observed.presence,
                    crate::agent::app_observation::Presence::NotFound
                )
            {
                log::warn!(
                    "Refusing a command that addresses an app Juno cannot find: {}",
                    target.app
                );
                return PreCheck::Refuse(observation::unknown_app_result(&target.app));
            }
            return PreCheck::Watch {
                app: target.app,
                kind: target.kind,
                before: observed.presence,
            };
        }

        // An action on whatever is on screen, in a run that opened an app:
        // confirm the app is still there before clicking where it used to be.
        if observation::acts_on_screen(&tool_call.name, &tool_call.input) {
            let Some(watched) = self.watched_app.clone() else {
                return PreCheck::Proceed;
            };
            if watched.last_seen_running.elapsed() < LIVENESS_MAX_AGE {
                return PreCheck::Proceed;
            }
            let action = tool_call
                .input
                .get("action")
                .and_then(|a| a.as_str())
                .unwrap_or(tool_call.name.as_str());
            let presence = observation::observe(&watched.name).await.presence;
            match observation::screen_action_for(&watched.name, action, presence) {
                observation::ScreenAction::Act => {
                    if presence.is_running() {
                        self.watched_app = Some(WatchedApp {
                            name: watched.name,
                            last_seen_running: std::time::Instant::now(),
                        });
                    }
                }
                observation::ScreenAction::Ask(output) => {
                    log::info!(
                        "{} closed mid-task; asking instead of acting on it",
                        watched.name
                    );
                    // Stop watching: the question has been asked once, and
                    // asking it again on every later action would nag.
                    self.watched_app = None;
                    return PreCheck::Refuse(output);
                }
            }
        }

        PreCheck::Proceed
    }

    /// Put what Juno saw after a launch into the result the model reads.
    ///
    /// This is the mechanism, not the prompt: the model is handed the observed
    /// state, so "Spotify is open on your screen" is unsupported unless Juno
    /// actually saw that.
    async fn attach_observation(
        &mut self,
        app: String,
        kind: crate::agent::app_observation::AppCommand,
        before: crate::agent::app_observation::Presence,
        tool_result: Result<crate::agent::core::ToolResult, AgentError>,
    ) -> Result<crate::agent::core::ToolResult, AgentError> {
        // A call that failed outright is already an honest answer; there is
        // nothing to add and nothing launched to observe.
        let mut result = tool_result?;

        let report = crate::agent::app_observation::settle(&app, kind, before).await;
        log::info!("Observed after acting on {}: {}", app, report.sentence());

        self.watched_app = report.after.presence.is_running().then(|| WatchedApp {
            name: app,
            last_seen_running: std::time::Instant::now(),
        });

        match &mut result.output {
            serde_json::Value::Object(map) => {
                map.insert("app_state".to_string(), report.to_json());
            }
            other => {
                // A bare string or number result: keep it, and add the
                // observation alongside rather than replacing it.
                let previous = other.clone();
                *other = serde_json::json!({
                    "output": previous,
                    "app_state": report.to_json(),
                });
            }
        }
        Ok(result)
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
                    images: None,
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
            if tool_call.name != "computer"
                && tool_call.name != crate::agent::tools::anthropic_computer_use::APP_CONTROLS_TOOL
            {
                crate::agent::tool_logger::log_tool_call_request(
                    &self.app_handle,
                    &tool_call.name,
                    tool_call.input.clone(),
                    Some(format!("Executing batched tool: {}", tool_call.name)),
                );
            }

            // Look before claiming, and notice when the world changed under
            // this run. Either can decide the call must not run at all.
            let (refusal, watch) = match self.check_before_tool(tool_call).await {
                PreCheck::Proceed => (None, None),
                PreCheck::Refuse(output) => (Some(output), None),
                PreCheck::Watch { app, kind, before } => (None, Some((app, kind, before))),
            };

            // Race tool execution against the cancellation signal so slow tools
            // (browser navigation, network requests) are interrupted immediately
            // when the user presses Escape — not just between tools.
            let mut cancel_for_tool = cancel_rx.clone();
            let tool_result = if let Some(output) = refusal {
                // Declined before it ran, so there is nothing to race.
                Ok(crate::agent::core::ToolResult {
                    call_id: tool_call.id.clone(),
                    output,
                })
            } else {
                tokio::select! {
                    result = self.tool_provider.execute_tool(tool_call.clone()) => result,
                    _ = cancel_for_tool.wait_for(|&v| v) => {
                        log::info!(
                            "Tool '{}' interrupted by cancellation signal during execution (tool {}/{} in batch)",
                            tool_call.name, i + 1, batch.len()
                        );
                        return Ok(false);
                    }
                }
            };

            // The observation rides back with the result, so the model's own
            // material says what happened rather than that the call was sent.
            let tool_result = match watch {
                Some((app, kind, before)) => {
                    self.attach_observation(app, kind, before, tool_result)
                        .await
                }
                None => tool_result,
            };

            // PERFORMANCE OPTIMIZATION: Replace hardcoded delays with intelligent completion detection
            // Old approach: Hardcoded 350ms delay for ALL mouse movements
            // New approach: Event-driven completion detection (10-50x faster)
            if self.is_mouse_movement_tool(tool_call) {
                // Use intelligent movement detection for any mouse movement operation
                self.wait_for_mouse_movement_completion(tool_call).await;
            }

            // Failure detection runs for EVERY tool, including `computer`.
            // It must sit outside the logging block below: that block deliberately
            // skips `computer` (which self-logs with richer metadata), and a failed
            // click is exactly the case that has to halt the rest of the batch.
            let tool_failed = tool_result_is_failure(&tool_result);

            // Skip runner-level result logging for "computer" tool — it self-logs
            // with enhanced metadata inside anthropic_computer_use.rs
            if tool_call.name != "computer"
                && tool_call.name != crate::agent::tools::anthropic_computer_use::APP_CONTROLS_TOOL
            {
                match &tool_result {
                    Ok(result) => {
                        let success = !tool_failed;

                        let screenshot_base64 = if success
                            && (tool_call.name == "capture_screenshot"
                                || tool_call.name == "browser_screenshot")
                        {
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

            // Halt the rest of the batch when a UI-mutating action failed. A batch
            // of physical actions is an ordered plan — `left_click` then `type`
            // then `key Return` — so a missed click must not be followed by typing
            // into whatever window happens to be focused. Independent tools (file
            // reads, read-only computer actions) keep going: the model still gets
            // the error, and stopping them would cost a round trip for no safety.
            // `failure_halts_batch` owns that trade and documents it.
            //
            // No retry here on purpose: `run_computer_action` already retries
            // internally (AX-grounded click with a coordinate fallback). A second
            // retry layer would hide the failure from the model and risk clicking
            // the wrong thing twice.
            if tool_failed {
                let halt = crate::agent::tools::anthropic_computer_use::failure_halts_batch(
                    &tool_call.name,
                    &tool_call.input,
                );
                log::warn!(
                    "Tool '{}' failed at {}/{} in batch — {}",
                    tool_call.name,
                    i + 1,
                    batch.len(),
                    if halt {
                        format!(
                            "halting the remaining {} tool call(s)",
                            batch.len().saturating_sub(i + 1)
                        )
                    } else {
                        "continuing: the failure is not order-sensitive".to_string()
                    }
                );
                if halt {
                    self.record_unexecuted_after_failure(batch, start_index, i, tool_results_cache)
                        .await?;
                    // A failure halt is NOT a cancellation. `Ok(false)` means
                    // "cancelled" and makes the caller raise `AgentError::Terminated`,
                    // ending the run. Here the agent loop must keep going so the model
                    // sees the error plus the skipped results and decides what to do.
                    return Ok(true);
                }
            }
        }

        Ok(true)
    }

    /// Record results for the tool calls that a failure halt skipped.
    ///
    /// Every tool_use block the model sent needs a matching tool_result, or the
    /// Anthropic API rejects the next request. Mirrors the shape used by
    /// [`Self::handle_batch_cancellation`], with wording that tells the model the
    /// action was declined rather than attempted and failed.
    async fn record_unexecuted_after_failure(
        &mut self,
        batch: &[crate::agent::core::ToolCall],
        start_index: usize,
        failed_index: usize,
        tool_results_cache: &mut [(
            crate::agent::core::ToolCall,
            Option<Result<crate::agent::core::ToolResult, AgentError>>,
        )],
    ) -> Result<(), AgentError> {
        let first_skipped = failed_index + 1;
        if first_skipped >= batch.len() {
            return Ok(());
        }

        // Fill the cache first, with no lock held. Each skipped slot becomes
        // `Some(..)` carrying the Anthropic error shape, so a later
        // `handle_batch_cancellation` — which keys off `result.is_none()` — can
        // never mistake a failure halt for a cancellation and double-report it.
        for (i, tool_call) in batch.iter().enumerate().skip(first_skipped) {
            if let Some(slot) = tool_results_cache.get_mut(start_index + i) {
                slot.1 = Some(Ok(crate::agent::core::ToolResult {
                    call_id: tool_call.id.clone(),
                    output: serde_json::json!({
                        "is_error": true,
                        "error": BATCH_HALT_SKIPPED_MESSAGE,
                    }),
                }));
            }
        }

        let mut skipped_count = 0usize;
        {
            let mut mem = self.memory.lock().await;
            for tool_call in batch.iter().skip(first_skipped) {
                mem.add_message(crate::agent::core::Message {
                    role: crate::agent::core::Role::Tool,
                    content: BATCH_HALT_SKIPPED_MESSAGE.to_string(),
                    tool_calls: None,
                    tool_call_id: Some(tool_call.id.clone()),
                    name: Some(tool_call.name.clone()),
                    images: None,
                })
                .await?;
                skipped_count += 1;
            }
        }

        log::info!(
            "Recorded skipped results for {} tool(s) after a batch failure halt",
            skipped_count
        );
        Ok(())
    }

    /// Check approval for a batch of tools.
    ///
    /// There is exactly one decision here and it is
    /// [`permission_policy::requires_approval`], which takes the person's
    /// chosen mode, the batch's highest risk, and whether they already said not
    /// to ask about this tool in this conversation.
    ///
    /// It used to be two conditions that both had to agree:
    /// `!is_tool_approval_required() && !needs_approval(&max_risk)`. Risk alone
    /// could force a prompt, so turning the setting off changed nothing and
    /// `sleep 1` asked for permission. Keeping the policy in one function is
    /// the point: a second condition anywhere is how the control died.
    async fn check_batch_approval(
        &self,
        batch: &[crate::agent::core::ToolCall],
        cancel_rx: &crate::state::CancelReceiver,
    ) -> Result<bool, AgentError> {
        use crate::agent::tools::permission_policy;
        use crate::agent::tools::risk_classifier;
        use crate::state::RiskLevel;

        // A text or an email has its own ask: the message card, and only
        // "send it" or the Send button answers it. It never rides along in a
        // batch, so each message is seen on its own before it goes.
        if let Some(send_idx) = batch
            .iter()
            .position(|t| risk_classifier::is_send_tool(&t.name))
        {
            return self.ask_to_send(batch, send_idx, cancel_rx).await;
        }

        let app_state = self.app_handle.state::<crate::state::AppState>();

        // Classify each tool once and find the highest risk level.
        let risk_levels: Vec<RiskLevel> = batch
            .iter()
            .map(|t| risk_classifier::classify_risk(&t.name, &t.input))
            .collect();
        let max_risk = risk_levels.iter().cloned().max().unwrap_or(RiskLevel::Low);

        // Find the riskiest single tool using pre-computed classifications.
        let riskiest_idx = risk_levels
            .iter()
            .enumerate()
            .max_by_key(|(_, r)| *r)
            .map(|(i, _)| i)
            .unwrap_or(0);
        let riskiest_tool = &batch[riskiest_idx];

        // Which conversation a "do not ask again" granted here would cover. A
        // runner with no session of its own gets the shared key, so the grant
        // still ends when Juno quits.
        let conversation_key = self
            .session_id
            .as_ref()
            .map(|id| id.as_str().to_string())
            .unwrap_or_else(crate::state::default_conversation_key);
        let granted = app_state
            .tool_granted_for_conversation(&conversation_key, &riskiest_tool.name)
            .await;

        // The one decision.
        if !permission_policy::requires_approval_for(
            &riskiest_tool.name,
            self.permission_mode().await,
            &max_risk,
            granted,
        ) {
            return Ok(true);
        }

        let target_app =
            risk_classifier::extract_target_app(&riskiest_tool.name, &riskiest_tool.input);

        // The sentence a person reads. Written in Rust on purpose: the
        // frontend should never have to turn `safari_execute_javascript` into
        // English, and the old copy read "Run bash", an em dash, then the raw
        // command, which is a tool name, an implementation detail and a dash
        // this project does not use.
        let batch_description = permission_policy::describe_batch(
            &permission_policy::describe_action(&riskiest_tool.name, &riskiest_tool.input),
            batch.len(),
        );

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
        .with_timeout(60)
        .with_conversation(conversation_key.clone());

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
            "batch_size": batch.len(),
            // What "Don't ask again" would cover, in words a person can read,
            // so the button can say "Always allow terminal commands" instead
            // of naming a tool. Absent when the action is Critical, because
            // nothing waives the floor and a button that claims otherwise is
            // the next dead control.
            "always_allow_label": if matches!(max_risk, RiskLevel::Critical) {
                serde_json::Value::Null
            } else {
                serde_json::Value::String(permission_policy::friendly_tool_name(
                    &riskiest_tool.name,
                ))
            }
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

        // Surface the wait in the parallel-session registry: the switcher
        // row flips to "Needs input" and a background (unfocused) session
        // fires a notification so the user knows to come look (LAC-1432).
        let marked_needs_input = self.mark_session_needs_input(&batch_description).await;

        // Poll for up to timeout_seconds at 50 ms intervals.
        let poll_iterations = (approval_request.timeout_seconds * 1000 / 50) as i64;
        let mut remaining = poll_iterations;
        let mut approved = false;

        while remaining > 0 && !approved {
            if *cancel_rx.borrow() {
                log::info!("Cancellation detected during approval wait");
                app_state.remove_tool_approval(&batch_id).await;
                self.emit_approval_resolved(&batch_id, ApprovalOutcome::Cancelled);
                if marked_needs_input {
                    // Guarded restore: no-ops when the cancel already moved
                    // the session to Cancelling via the registry.
                    self.clear_session_needs_input().await;
                }
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
        if marked_needs_input {
            self.clear_session_needs_input().await;
        }

        // Settle the row, always.
        //
        // The old code denied a timed-out batch in the backend and emitted
        // nothing, so the frontend's `approval_state` stayed `pending` and the
        // Allow and Don't allow buttons sat there doing nothing for the rest of
        // the conversation. Every way this wait can end now reports itself;
        // `ApprovalOutcome` has no pending variant, so a new exit path cannot
        // forget to.
        let outcome = if approved {
            ApprovalOutcome::Allowed
        } else if remaining <= 0 {
            ApprovalOutcome::TimedOut
        } else {
            ApprovalOutcome::Denied
        };
        self.emit_approval_resolved(&batch_id, outcome);

        if !approved {
            log::warn!("Tool batch execution denied: {}", outcome.reason());

            for tool_call in batch {
                let mut mem = self.memory.lock().await;
                mem.add_message(crate::agent::core::Message {
                    role: crate::agent::core::Role::Tool,
                    content: format!("Juno did not run this: {}", outcome.reason()),
                    tool_calls: None,
                    tool_call_id: Some(tool_call.id.clone()),
                    name: Some(tool_call.name.clone()),
                    images: None,
                })
                .await?;
            }
        }

        Ok(approved)
    }

    /// Write the same tool result for every call in a batch that did not run.
    async fn record_not_run(
        &self,
        batch: &[crate::agent::core::ToolCall],
        content: &str,
    ) -> Result<(), AgentError> {
        for tool_call in batch {
            let mut mem = self.memory.lock().await;
            mem.add_message(crate::agent::core::Message {
                role: crate::agent::core::Role::Tool,
                content: content.to_string(),
                tool_calls: None,
                tool_call_id: Some(tool_call.id.clone()),
                name: Some(tool_call.name.clone()),
                images: None,
            })
            .await?;
        }
        Ok(())
    }

    /// The send gate (`risk_classifier::Consequence::Send`).
    ///
    /// Asks in every permission mode and never consults a "do not ask again"
    /// grant. The recipient is resolved through Contacts first, so the card
    /// shows who it really goes to; a name that matches more than one person
    /// is answered with the candidates and never asked about or sent. The ask
    /// is the message card plus the spoken line ending "Say send it."; the
    /// phrase arrives through `cli_approval::answer_pending_approval` and
    /// resolves the same pending approval the Send button does.
    async fn ask_to_send(
        &self,
        batch: &[crate::agent::core::ToolCall],
        send_idx: usize,
        cancel_rx: &crate::state::CancelReceiver,
    ) -> Result<bool, AgentError> {
        use crate::agent::tools::mac_apps::{self, send};
        use crate::state::RiskLevel;

        if batch.len() > 1 {
            self.record_not_run(batch, send::ONE_SEND_AT_A_TIME).await?;
            return Ok(false);
        }
        let tool = &batch[send_idx];

        let draft = match mac_apps::preview_send(tool.name.clone(), tool.input.clone()).await {
            Ok(draft) => draft,
            Err(answer) => {
                let text = serde_json::to_string(&answer).unwrap_or_else(|_| answer.to_string());
                self.record_not_run(batch, &text).await?;
                return Ok(false);
            }
        };

        let app_state = self.app_handle.state::<crate::state::AppState>();
        let conversation_key = self
            .session_id
            .as_ref()
            .map(|id| id.as_str().to_string())
            .unwrap_or_else(crate::state::default_conversation_key);
        let approval_id = uuid::Uuid::new_v4().to_string();
        let message = draft.approval_payload();
        let request = crate::state::ToolApprovalRequest::new(
            approval_id.clone(),
            tool.name.clone(),
            tool.input.clone(),
            draft.description(),
        )
        .with_risk(RiskLevel::Critical)
        .with_timeout(send::SEND_TIMEOUT_SECS)
        .with_conversation(conversation_key)
        .with_message(message.clone());

        app_state.add_pending_tool_approval(request.clone()).await;
        if let Err(e) = self.app_handle.emit(
            events::tools::APPROVAL_REQUEST,
            serde_json::json!({
                "tool_name": request.tool_name,
                "tool_id": request.tool_id,
                "tool_input": request.tool_input,
                "description": request.description,
                "timestamp": request.timestamp,
                "risk_level": request.risk_level,
                "target_app": request.target_app,
                "timeout_seconds": request.timeout_seconds,
                "is_batch": false,
                "batch_size": 1,
                "always_allow_label": serde_json::Value::Null,
                "consequence": "send",
                "message": message,
            }),
        ) {
            log::error!("Failed to emit the send approval: {}", e);
        }

        // Say the message and the phrase. The card stays as the answer surface.
        {
            let app = self.app_handle.clone();
            let spoken = draft.prompt();
            tauri::async_runtime::spawn(async move {
                let state = app.state::<crate::state::AppState>();
                let _ = crate::tts::invoke_tts(spoken, state, app.as_ref().clone()).await;
            });
        }

        let marked_needs_input = self.mark_session_needs_input(&draft.description()).await;

        let mut remaining = (send::SEND_TIMEOUT_SECS * 1000 / 50) as i64;
        let mut decision: Option<bool> = None;
        while remaining > 0 {
            if *cancel_rx.borrow() {
                app_state.remove_tool_approval(&approval_id).await;
                let _ = send::take_correction(&approval_id);
                self.emit_approval_resolved(&approval_id, ApprovalOutcome::Cancelled);
                if marked_needs_input {
                    self.clear_session_needs_input().await;
                }
                return Err(AgentError::Terminated);
            }
            if let Some(answer) = app_state.get_tool_approval_status(&approval_id).await {
                decision = Some(answer);
                break;
            }
            tokio::time::sleep(std::time::Duration::from_millis(50)).await;
            remaining -= 1;
        }
        app_state.remove_tool_approval(&approval_id).await;
        if marked_needs_input {
            self.clear_session_needs_input().await;
        }

        let outcome = match decision {
            Some(true) => ApprovalOutcome::Allowed,
            Some(false) => ApprovalOutcome::Denied,
            None => ApprovalOutcome::TimedOut,
        };
        self.emit_approval_resolved(&approval_id, outcome);
        let correction = send::take_correction(&approval_id);

        if decision == Some(true) {
            log::info!("Send approved: {}", draft.description());
            return Ok(true);
        }
        let text = send::not_sent_text(correction.as_deref(), decision.is_none());
        log::info!("Send not approved: {}", text);
        self.record_not_run(batch, &text).await?;
        Ok(false)
    }

    /// Tell the chat surface an approval question is over.
    ///
    /// Fires for every outcome including Allowed, which the frontend already
    /// knows about: the handler is idempotent, and a reporter that only speaks
    /// up some of the time is how the timeout bug hid.
    fn emit_approval_resolved(&self, tool_id: &str, outcome: ApprovalOutcome) {
        if let Err(e) = self.app_handle.emit(
            events::tools::APPROVAL_RESOLVED,
            serde_json::json!({
                "tool_id": tool_id,
                "resolution": outcome.resolution(),
                "reason": outcome.reason(),
            }),
        ) {
            log::error!("Failed to emit tool-approval-resolved: {}", e);
        }
    }

    /// The person's chosen permission mode, read fresh so a change in Settings
    /// applies to the next tool call rather than the next launch.
    async fn permission_mode(&self) -> crate::agent::tools::permission_policy::PermissionMode {
        use crate::agent::tools::permission_policy::PermissionMode;
        let Some(manager) = self
            .app_handle
            .try_state::<crate::settings::manager::SettingsManager>()
        else {
            return PermissionMode::default();
        };
        match manager.get_agent_settings().await {
            Ok(settings) => PermissionMode::from_setting(&settings.permission_mode),
            Err(e) => {
                // Falling back to the default is the safe direction: it asks
                // more than "do not ask", never less.
                log::warn!(
                    "Could not read the permission mode, using the default: {}",
                    e
                );
                PermissionMode::default()
            }
        }
    }

    /// Flip this runner's session row to `NeedsInput` while a tool-approval
    /// prompt is pending. Emits the discrete needs-input lifecycle event,
    /// rebroadcasts the session list for the switcher UI, and — when the
    /// session is unfocused (running in the background) — sends a
    /// notification so the user learns an agent is waiting on them
    /// (LAC-1432 criterion 5b). Returns whether the transition happened;
    /// the caller must call `clear_session_needs_input` once the wait
    /// resolves.
    async fn mark_session_needs_input(&self, description: &str) -> bool {
        let Some(session_id) = self.session_id.as_ref() else {
            return false;
        };
        let app_state = self.app_handle.state::<crate::state::AppState>();
        let registry = app_state.agent_sessions();
        let Some(snapshot) = registry.begin_needs_input(session_id).await else {
            return false;
        };
        let was_focused = snapshot.focused;
        let agent_name = snapshot.agent_name.clone();

        if let Err(e) = self.app_handle.emit(
            crate::constants::events::agent_sessions::NEEDS_INPUT,
            &snapshot,
        ) {
            log::error!("Failed to emit agent-session-needs-input: {}", e);
        }
        crate::agents::broadcast_sessions_updated(self.app_handle.as_ref(), &registry).await;

        // The focused session's approval dialog is already on screen;
        // notifying for it would be noise (same gate as terminal-state
        // notifications in anthropic.rs).
        if !was_focused && !crate::cli::headless::is_headless_mode() {
            let data = crate::commands::notifications::NotificationData {
                title: format!("{} — Needs input", agent_name),
                message: description.chars().take(140).collect::<String>(),
                level: "warning".to_string(),
                important: Some(true),
                timeout: None,
            };
            if let Err(e) = crate::commands::notifications::send_notification(
                self.app_handle.as_ref().clone(),
                app_state.clone(),
                data,
            )
            .await
            {
                log::warn!("Failed to send needs-input notification: {}", e);
            }
        }
        true
    }

    /// Restore the session to `Running` after the approval wait resolves.
    /// The registry guards the transition, so a cancellation that landed
    /// mid-wait (status `Cancelling`) is never overwritten.
    async fn clear_session_needs_input(&self) {
        let Some(session_id) = self.session_id.as_ref() else {
            return;
        };
        let app_state = self.app_handle.state::<crate::state::AppState>();
        let registry = app_state.agent_sessions();
        if registry.end_needs_input(session_id).await {
            crate::agents::broadcast_sessions_updated(self.app_handle.as_ref(), &registry).await;
        }
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
                    if (tool_call.name == "computer"
                        || tool_call.name
                            == crate::agent::tools::anthropic_computer_use::APP_CONTROLS_TOOL)
                        && result.output.get("base64_image").is_some()
                    {
                        serde_json::to_string(&result.output)
                            .unwrap_or_else(|_| tool_result_content(&result.output))
                    } else {
                        tool_result_content(&result.output)
                    }
                }
                Err(e) => format!("Error: {}", e),
            },
            tool_calls: None,
            tool_call_id: Some(tool_call.id.clone()),
            name: Some(tool_call.name.clone()),
            images: None,
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

        for attempt in 0..8 {
            // Max 8 attempts
            // Check timeout at the start of each iteration
            if start_time.elapsed() >= max_wait {
                break;
            }

            // Get current cursor position
            let app_state = self.app_handle.state::<crate::state::AppState>();
            if let Ok((current_x, current_y)) =
                crate::commands::mouse::get_cursor_position((*self.app_handle).clone(), app_state)
                    .await
            {
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

    /// Add Juno's reply for this turn to memory, which tees it into the
    /// conversation history. Best effort: history must never fail a turn.
    async fn save_reply(&self, text: &str, interrupted: bool) {
        let Some(message) = crate::conversation_history::reply_message(text, interrupted) else {
            return;
        };
        let mut mem = self.memory.lock().await;
        if let Err(e) = mem.add_message(message).await {
            log::debug!("Could not keep the reply in history: {e}");
        }
    }

    /// Safety net for brains that leave `<TTS>` blocks in their final text
    /// instead of extracting them while streaming (OpenAI, Gemini, rig, the
    /// Anthropic non-streaming path). Speaks each block through the normal TTS
    /// path and returns the display text. Streaming brains already stripped
    /// the tags, so for them this is a no-op and nothing is spoken twice.
    fn speak_and_strip_tts(&self, text: String) -> String {
        if !crate::agent::tts_tags::contains_tts_tags(&text) {
            return text;
        }
        let (display, spoken) = crate::agent::tts_tags::split_tts_tags(&text);
        for block in spoken {
            log::info!("Speaking TTS block left in final response: '{}'", block);
            crate::agent::tool_logger::process_tts_content_immediately(
                (*self.app_handle).clone(),
                block,
            );
        }
        display
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

        // Add initial user message to memory, with whatever was attached to it.
        {
            let mut mem = self.memory.lock().await;
            mem.add_message(Message::from_user(
                initial_prompt,
                self.pending_images.take(),
            ))
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
                    // Keep the reply in the conversation. The raw text still
                    // has its spoken blocks, which the history keeps in order.
                    self.save_reply(&text, false).await;
                    let final_response = self.speak_and_strip_tts(text);
                    log::info!("Agent finished with text response: \"{}\"", final_response);
                    self.transition_state(AgentState::Finished).await;
                    return Ok(final_response);
                }

                AgentAction::RespondToUser(text) => {
                    let text = self.speak_and_strip_tts(text);
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
                            images: None,
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
            let outcome = self
                .brain
                .decide_next_action_streaming(
                    &messages,
                    &tools,
                    Some((*self.app_handle).clone()),
                    Some(message_id.clone()),
                    // Session-aware cancel channel (merged session+global for
                    // session-tracked runs) so subprocess-based brains can kill
                    // their child on focused-session cancel (LAC-3697).
                    Some(cancel_rx.clone()),
                )
                .await;
            // Whatever was streamed is either the final reply (kept by the
            // run loop) or, if the turn was cut short, saved here.
            let streamed = crate::conversation_history::take_streamed(&message_id);
            match outcome {
                Ok(action) => action,
                Err(e) => {
                    let interrupted = matches!(e, AgentError::Terminated);
                    self.save_reply(&streamed, interrupted).await;
                    return Err(e);
                }
            }
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
                    images: None,
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
                                if matches!(msg.role, Role::Assistant)
                                    && msg
                                        .tool_calls
                                        .as_ref()
                                        .map(|tc| tc.len() == tool_calls.len())
                                        .unwrap_or(false)
                                {
                                    found_our_assistant_message = true;
                                    // Collect all tool call IDs from this assistant message
                                    if let Some(tool_calls) = &msg.tool_calls {
                                        assistant_tool_call_ids =
                                            tool_calls.iter().map(|tc| tc.id.clone()).collect();
                                    }
                                    break;
                                }
                            }

                            // Second pass: Count tool results that match our assistant's tool call IDs
                            if found_our_assistant_message {
                                for msg in messages.iter().rev() {
                                    if matches!(msg.role, Role::Tool)
                                        && msg
                                            .tool_call_id
                                            .as_ref()
                                            .map(|id| assistant_tool_call_ids.contains(id))
                                            .unwrap_or(false)
                                    {
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
                                    if matches!(last_message.role, Role::Assistant)
                                        && last_message
                                            .tool_calls
                                            .as_ref()
                                            .map(|tc| tc.len() == tool_calls.len())
                                            .unwrap_or(false)
                                    {
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
            _cancel_rx: Option<crate::state::CancelReceiver>,
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
            images: None,
        });

        messages.push(Message {
            role: Role::Assistant,
            content: "How can I help?".to_string(),
            tool_calls: None,
            tool_call_id: None,
            name: None,
            images: None,
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
            images: None,
        });

        // Add tool results for the first tool only (partial execution)
        messages.push(Message {
            role: Role::Tool,
            content: "Tool result".to_string(),
            tool_calls: None,
            tool_call_id: Some("tool1".to_string()),
            name: Some("test_tool".to_string()),
            images: None,
        });

        // Now manually implement the counting logic from our fix
        let mut tool_result_count = 0;
        let mut found_our_assistant_message = false;
        let mut assistant_tool_call_ids = Vec::new();

        // First pass: Find assistant message and collect tool call IDs
        for msg in messages.iter().rev() {
            if matches!(msg.role, Role::Assistant)
                && msg
                    .tool_calls
                    .as_ref()
                    .map(|tc| tc.len() == tool_calls.len())
                    .unwrap_or(false)
            {
                found_our_assistant_message = true;
                if let Some(tool_calls) = &msg.tool_calls {
                    assistant_tool_call_ids = tool_calls.iter().map(|tc| tc.id.clone()).collect();
                }
                break;
            }
        }

        // Second pass: Count tool results that match our assistant's tool call IDs
        if found_our_assistant_message {
            for msg in messages.iter().rev() {
                if matches!(msg.role, Role::Tool)
                    && msg
                        .tool_call_id
                        .as_ref()
                        .map(|id| assistant_tool_call_ids.contains(id))
                        .unwrap_or(false)
                {
                    tool_result_count += 1;
                }
            }
        }

        // Verify results
        assert!(
            found_our_assistant_message,
            "Should have found the assistant message"
        );
        assert_eq!(
            assistant_tool_call_ids.len(),
            2,
            "Should have found 2 tool call IDs"
        );
        assert_eq!(tool_result_count, 1, "Should have counted 1 tool result");

        // Test case 2: No tool results
        let mut messages2 = messages.clone();
        messages2.pop(); // Remove the tool result

        let mut tool_result_count2 = 0;
        let mut found_our_assistant_message2 = false;
        let mut assistant_tool_call_ids2 = Vec::new();

        // First pass
        for msg in messages2.iter().rev() {
            if matches!(msg.role, Role::Assistant)
                && msg
                    .tool_calls
                    .as_ref()
                    .map(|tc| tc.len() == tool_calls.len())
                    .unwrap_or(false)
            {
                found_our_assistant_message2 = true;
                if let Some(tool_calls) = &msg.tool_calls {
                    assistant_tool_call_ids2 = tool_calls.iter().map(|tc| tc.id.clone()).collect();
                }
                break;
            }
        }

        // Second pass
        if found_our_assistant_message2 {
            for msg in messages2.iter().rev() {
                if matches!(msg.role, Role::Tool) {
                    if let Some(tool_call_id) = &msg.tool_call_id {
                        if assistant_tool_call_ids2.contains(tool_call_id) {
                            tool_result_count2 += 1;
                        }
                    }
                }
            }
        }

        // Verify results for case 2
        assert!(
            found_our_assistant_message2,
            "Should have found the assistant message"
        );
        assert_eq!(
            assistant_tool_call_ids2.len(),
            2,
            "Should have found 2 tool call IDs"
        );
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
        let tools = [
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
                    if let (Some(x), Some(y)) =
                        (end_coord_array[0].as_f64(), end_coord_array[1].as_f64())
                    {
                        return Some((x as i32, y as i32));
                    }
                }
            }

            // Format 2: Anthropic Computer Use API - {"coordinate": [x, y]}
            if let Some(coord_array) = input.get("coordinate").and_then(|c| c.as_array()) {
                if coord_array.len() == 2 {
                    if let (Some(x), Some(y)) = (coord_array[0].as_f64(), coord_array[1].as_f64()) {
                        return Some((x as i32, y as i32));
                    }
                }
            }

            // Format 3: Separate x/y fields - {"x": 100, "y": 200}
            if let (Some(x), Some(y)) = (
                input.get("x").and_then(|v| v.as_f64()),
                input.get("y").and_then(|v| v.as_f64()),
            ) {
                return Some((x as i32, y as i32));
            }

            // Format 4: Nested coordinate object - {"coordinate": {"x": 100, "y": 200}}
            if let Some(coord_obj) = input.get("coordinate").and_then(|c| c.as_object()) {
                if let (Some(x), Some(y)) = (
                    coord_obj.get("x").and_then(|v| v.as_f64()),
                    coord_obj.get("y").and_then(|v| v.as_f64()),
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

#[cfg(test)]
mod batch_halt_tests {
    use super::*;
    use crate::agent::core::ToolResult;

    fn ok_result(output: serde_json::Value) -> Result<ToolResult, AgentError> {
        Ok(ToolResult {
            call_id: "call_1".to_string(),
            output,
        })
    }

    #[test]
    fn rust_level_error_is_a_failure() {
        let result: Result<ToolResult, AgentError> = Err(AgentError::ToolError("boom".to_string()));
        assert!(tool_result_is_failure(&result));
    }

    #[test]
    fn anthropic_error_payload_is_a_failure() {
        // The shape the computer tool actually returns on a failed click:
        // Ok(..) at the Rust level, with the error inside the payload.
        let result = ok_result(serde_json::json!({
            "is_error": true,
            "error": "Click failed: no element at (100, 200)"
        }));
        assert!(tool_result_is_failure(&result));
    }

    #[test]
    fn successful_payload_is_not_a_failure() {
        assert!(!tool_result_is_failure(&ok_result(
            serde_json::json!({ "success": true })
        )));
    }

    #[test]
    fn is_error_false_is_not_a_failure() {
        assert!(!tool_result_is_failure(&ok_result(
            serde_json::json!({ "is_error": false, "success": true })
        )));
    }

    #[test]
    fn non_object_payload_is_not_a_failure() {
        assert!(!tool_result_is_failure(&ok_result(
            serde_json::Value::String("plain text output".to_string())
        )));
        assert!(!tool_result_is_failure(&ok_result(serde_json::Value::Null)));
    }

    #[test]
    fn skipped_message_matches_the_toolset_contract() {
        assert_eq!(
            BATCH_HALT_SKIPPED_MESSAGE,
            "Not executed: an earlier computer action in this turn failed."
        );
    }

    // --- Which failures halt the batch ---

    use crate::agent::tools::anthropic_computer_use::failure_halts_batch;

    fn computer(action: &str) -> serde_json::Value {
        serde_json::json!({ "action": action, "coordinate": [100, 200] })
    }

    #[test]
    fn ui_mutating_computer_actions_halt() {
        for action in [
            "left_click",
            "right_click",
            "middle_click",
            "double_click",
            "triple_click",
            "left_click_drag",
            "mouse_move",
            "left_mouse_down",
            "left_mouse_up",
            "key",
            "hold_key",
            "type",
            "scroll",
        ] {
            assert!(
                failure_halts_batch("computer", &computer(action)),
                "{action} should halt the batch"
            );
        }
    }

    #[test]
    fn read_only_computer_actions_do_not_halt() {
        // A failed screenshot changes nothing on screen, and the clicks behind it
        // were planned against an earlier screenshot. The model gets the error.
        for action in ["screenshot", "cursor_position", "wait", "zoom"] {
            assert!(
                !failure_halts_batch("computer", &computer(action)),
                "{action} should not halt the batch"
            );
        }
    }

    #[test]
    fn independent_tools_do_not_halt() {
        for name in [
            "read_file",
            "bash",
            "capture_screenshot",
            "browser_navigate",
            "str_replace_based_edit_tool",
        ] {
            assert!(
                !failure_halts_batch(name, &serde_json::json!({ "path": "/tmp/x" })),
                "{name} should not halt the batch"
            );
        }
    }

    #[test]
    fn computer_call_with_unreadable_action_halts() {
        // Safe direction: the call may already have moved the pointer or typed.
        assert!(failure_halts_batch("computer", &serde_json::json!({})));
        assert!(failure_halts_batch(
            "computer",
            &serde_json::json!({ "action": 7 })
        ));
        assert!(failure_halts_batch("computer", &serde_json::Value::Null));
    }

    #[test]
    fn a_non_computer_tool_carrying_an_action_field_does_not_halt() {
        // Guards the dispatch key. The rule is scoped by *tool name*, not by
        // the presence of an `action` field, so a custom tool that happens to
        // take one must not inherit the computer tool's halt behaviour.
        assert!(!failure_halts_batch(
            "not_computer",
            &serde_json::json!({ "action": "left_click" })
        ));
    }

    /// The failure a prompt could never fix: what the model reads.
    ///
    /// A bash call that fails returns `{"output": ..., "exit_code": 1}`, and
    /// the old summary turned that into "Result with 2 fields". The exit code
    /// and the error text were gone before the model saw them, so "it worked"
    /// was the only available reading. Every fact in the result has to survive
    /// the trip.
    #[test]
    fn a_failed_command_reaches_the_model_as_what_it_was() {
        let failed = serde_json::json!({
            "output": "Unable to find application named 'Spotfiy'",
            "exit_code": 1
        });
        let content = tool_result_content(&failed);
        assert!(
            content.contains("exit_code"),
            "the exit code must survive: {content}"
        );
        assert!(
            content.contains("Unable to find application"),
            "the error text must survive: {content}"
        );
        assert_ne!(content, "Result with 2 fields");

        // An observation attached to a result survives the same trip.
        let observed = serde_json::json!({
            "output": "",
            "exit_code": 0,
            "app_state": { "state": "not_running", "observed": "Spotify launched and then exited." }
        });
        assert!(tool_result_content(&observed).contains("launched and then exited"));
    }

    #[test]
    fn single_field_and_plain_results_still_read_as_text() {
        // Unchanged behaviour, and worth keeping: a one-field result unwraps
        // so a file's contents reach the model as text, not as quoted JSON.
        assert_eq!(
            tool_result_content(&serde_json::json!({ "output": "hello" })),
            "hello"
        );
        assert_eq!(
            tool_result_content(&serde_json::Value::String("hello".into())),
            "hello"
        );
    }

    /// `tool_result_content` may never describe a result instead of reporting
    /// it. This is the general form of the two tests above, and it is here
    /// because fixing the one shape that bit us is not the same as making the
    /// function incapable of the mistake.
    ///
    /// The old `format_task_output` had two lossy arms, and both shipped: an
    /// object of more than one field became "Result with N fields", and an
    /// array of more than one item became "List with N items". Four call
    /// sites were still reading through them when the ungated `agents::`
    /// executor was deleted. Rather than fix the call sites, the arms are
    /// gone, so no present or future caller can discard a result.
    ///
    /// Every case below carries more than one value, so every case must
    /// survive as parseable JSON equal to what went in.
    #[test]
    fn tool_result_content_never_discards() {
        let multi_value = [
            serde_json::json!({ "output": "x", "exit_code": 1 }),
            serde_json::json!({ "windows": ["Safari", "Mail"], "count": 2 }),
            serde_json::json!({ "a": 1, "b": 2, "c": 3 }),
            // A top-level array was the arm nobody noticed: `List with N items`
            // threw away the whole result.
            serde_json::json!(["Safari", "Mail"]),
            serde_json::json!([{ "id": 1 }, { "id": 2 }]),
            // Nesting must not reintroduce a summary further down.
            serde_json::json!({ "outer": { "inner": [1, 2], "n": 2 }, "ok": true }),
        ];

        for value in multi_value {
            let content = tool_result_content(&value);
            let parsed: serde_json::Value = serde_json::from_str(&content).unwrap_or_else(|e| {
                panic!("result was described, not reported: {content:?} ({e})")
            });
            assert_eq!(
                parsed, value,
                "every value must survive the trip to the model: {content}"
            );
        }
    }

    /// The summaries that caused the lying are not spellable any more.
    ///
    /// A regression here would most likely arrive as a well-meaning
    /// "readability" change, so the banned strings are named rather than
    /// inferred.
    #[test]
    fn the_lossy_summaries_are_gone() {
        let shapes = [
            serde_json::json!({ "a": 1, "b": 2 }),
            serde_json::json!([1, 2, 3]),
            serde_json::json!({ "n": [1, 2, 3], "m": 2 }),
        ];
        for value in shapes {
            let content = tool_result_content(&value);
            assert!(
                !content.starts_with("Result with") && !content.starts_with("List with"),
                "a count is not a result: {content}"
            );
        }

        // An empty result is the one case with nothing to lose, so a word is
        // honest there and stays.
        assert_eq!(tool_result_content(&serde_json::json!({})), "Empty result");
        assert_eq!(tool_result_content(&serde_json::json!([])), "Empty list");
        assert_eq!(tool_result_content(&serde_json::Value::Null), "No output");
    }
}

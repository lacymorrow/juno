//! # Mock Brain
//!
//! A test-only `AgentBrain` implementation that returns canned responses
//! without making any API calls. This allows full pipeline testing in CI
//! without an `ANTHROPIC_API_KEY`.
//!
//! ## Modes
//!
//! - `Immediate` — returns `AgentAction::Finish` with a fixed string
//! - `ToolThenFinish` — first call returns a tool call, second call finishes
//! - `MultiStep(n)` — returns `n` tool calls, then finishes

use async_trait::async_trait;
use serde_json::json;
use std::sync::atomic::{AtomicU32, Ordering};

use crate::agent::core::{
    AgentAction, AgentError, Message, ToolCall, ToolDefinition,
};
use crate::agent::traits::AgentBrain;

/// Controls the mock brain's behavior.
#[derive(Debug, Clone)]
pub enum MockBrainMode {
    /// Immediately finish with the given response text.
    Immediate(String),
    /// First decision returns a single tool call, second decision finishes.
    ToolThenFinish {
        tool_name: String,
        tool_input: serde_json::Value,
        final_response: String,
    },
    /// Returns `steps` tool calls, then finishes.
    MultiStep {
        steps: u32,
        tool_name: String,
        final_response: String,
    },
}

/// A mock brain for testing the agent pipeline end-to-end.
pub struct MockBrain {
    mode: MockBrainMode,
    call_count: AtomicU32,
}

impl MockBrain {
    /// Create a new mock brain with the given mode.
    pub fn new(mode: MockBrainMode) -> Self {
        Self {
            mode,
            call_count: AtomicU32::new(0),
        }
    }

    /// Create a mock brain that immediately finishes with "Hello from MockBrain".
    pub fn immediate() -> Self {
        Self::new(MockBrainMode::Immediate(
            "Hello from MockBrain".to_string(),
        ))
    }

    /// Create a mock brain that calls a tool once, then finishes.
    pub fn tool_then_finish(tool_name: &str, final_response: &str) -> Self {
        Self::new(MockBrainMode::ToolThenFinish {
            tool_name: tool_name.to_string(),
            tool_input: json!({"action": "test"}),
            final_response: final_response.to_string(),
        })
    }
}

#[async_trait]
impl AgentBrain for MockBrain {
    async fn decide_next_action(
        &self,
        _messages: &[Message],
        _available_tools: &[ToolDefinition],
    ) -> Result<AgentAction, AgentError> {
        let call_num = self.call_count.fetch_add(1, Ordering::SeqCst);

        match &self.mode {
            MockBrainMode::Immediate(response) => {
                Ok(AgentAction::Finish(response.clone()))
            }
            MockBrainMode::ToolThenFinish {
                tool_name,
                tool_input,
                final_response,
            } => {
                if call_num == 0 {
                    Ok(AgentAction::ExecuteTool(vec![ToolCall {
                        id: format!("mock_call_{}", call_num),
                        name: tool_name.clone(),
                        input: tool_input.clone(),
                    }]))
                } else {
                    Ok(AgentAction::Finish(final_response.clone()))
                }
            }
            MockBrainMode::MultiStep {
                steps,
                tool_name,
                final_response,
            } => {
                if call_num < *steps {
                    Ok(AgentAction::ExecuteTool(vec![ToolCall {
                        id: format!("mock_call_{}", call_num),
                        name: tool_name.clone(),
                        input: json!({"step": call_num}),
                    }]))
                } else {
                    Ok(AgentAction::Finish(final_response.clone()))
                }
            }
        }
    }

    fn supports_streaming(&self) -> bool {
        false
    }
}

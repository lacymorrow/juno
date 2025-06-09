//! Official Anthropic Computer Use tools for desktop screen interaction.
//! Implements the complete Computer Use API with mouse, keyboard, and text editing.
//! Used by: Main agent orchestrator for all computer interaction tasks.

use computer_use_ai_sdk::{ComputerTool, TextEditorTool, BashTool};
use computer_use_ai_sdk::types::{ComputerAction, TextEditAction, BashAction};
use serde_json::{json, Value};
use std::collections::HashMap;
use tracing::{info, warn, error, debug};
use async_trait::async_trait;

use crate::agent::structs::{ToolCall, ToolResult, ToolDefinition, AgentError};
use crate::agent::traits::ToolProvider;
use crate::state::AppState;

/// Provider for official Anthropic Computer Use tools.
/// Implements all 17 computer interaction actions for complete desktop control.
/// Used by: Agent tool system for computer automation tasks.
pub struct AnthropicComputerUseProvider {
    computer_tool: ComputerTool,
    text_editor_tool: TextEditorTool,
    bash_tool: BashTool,
    app_state: AppState,
}

impl AnthropicComputerUseProvider {
    /// Creates a new Anthropic Computer Use provider.
    /// Used by: Tool registration system during agent initialization.
    pub fn new(app_state: AppState) -> Result<Self, AgentError> {
// ... existing code ...
    }

    /// Executes computer tool actions (screenshot, click, key, type, scroll, etc.).
    /// Handles all mouse, keyboard, and screen interaction commands.
    /// Used by: Agent execution when computer interaction is needed.
    async fn execute_computer_tool(&self, tool_call: &ToolCall) -> Result<ToolResult, AgentError> {
// ... existing code ...
    }

    /// Executes text editor actions (view, create, str_replace, undo).
    /// Provides file viewing, creation, and editing capabilities.
    /// Used by: Agent execution for file manipulation tasks.
    async fn execute_text_editor_tool(&self, tool_call: &ToolCall) -> Result<ToolResult, AgentError> {
// ... existing code ...
    }

    /// Executes bash commands with timeout and output capture.
    /// Provides command line execution with proper error handling.
    /// Used by: Agent execution for terminal command tasks.
    async fn execute_bash_tool(&self, tool_call: &ToolCall) -> Result<ToolResult, AgentError> {
// ... existing code ...
    }

    /// Converts SDK computer action to JSON for agent communication.
    /// Used by: Computer tool execution for result formatting.
    fn action_to_json(&self, action: ComputerAction) -> Value {
// ... existing code ...
    }

    /// Converts SDK text edit action to JSON for agent communication.
    /// Used by: Text editor tool execution for result formatting.
    fn text_action_to_json(&self, action: TextEditAction) -> Value {
// ... existing code ...
    }

    /// Converts SDK bash action to JSON for agent communication.
    /// Used by: Bash tool execution for result formatting.
    fn bash_action_to_json(&self, action: BashAction) -> Value {
// ... existing code ...
    }

    /// Gets computer tool definition with all supported actions.
    /// Used by: Tool discovery system for computer tool registration.
    fn get_computer_tool_definition() -> ToolDefinition {
// ... existing code ...
    }

    /// Gets text editor tool definition with all file operations.
    /// Used by: Tool discovery system for text editor tool registration.
    fn get_text_editor_tool_definition() -> ToolDefinition {
// ... existing code ...
    }

    /// Gets bash tool definition for command execution.
    /// Used by: Tool discovery system for bash tool registration.
    fn get_bash_tool_definition() -> ToolDefinition {
// ... existing code ...
    }
}

#[async_trait]
impl ToolProvider for AnthropicComputerUseProvider {
    /// Executes the specified Anthropic Computer Use tool.
    /// Routes tool calls to appropriate handlers (computer, text_editor, bash).
    /// Used by: Agent tool execution system for computer use tasks.
    async fn execute_tool(&self, tool_call: ToolCall) -> Result<ToolResult, AgentError> {
// ... existing code ...
    }

    /// Lists all available Anthropic Computer Use tools.
    /// Returns definitions for computer, text editor, and bash tools.
    /// Used by: Tool discovery and agent initialization systems.
    async fn list_tools(&self) -> Result<Vec<ToolDefinition>, AgentError> {
// ... existing code ...
    }
}

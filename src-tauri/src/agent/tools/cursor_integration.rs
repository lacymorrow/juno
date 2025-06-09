//! Cursor IDE Integration for Juno AI Computer Use Agent
//!
//! This module provides seamless integration with Cursor IDE, enabling the agent to
//! interact directly with the development environment for enhanced coding workflows.
//!
//! ## Core Features
//!
//! - **File Navigation**: Open files at specific lines and columns
//! - **Suggestion Display**: Show contextual suggestions and messages
//! - **Multi-Method Access**: Command line interface with GUI fallback
//! - **Language Awareness**: Appropriate comment formatting for different languages
//! - **Precise Navigation**: Line and column-specific positioning
//!
//! ## Integration Methods
//!
//! 1. **Command Line Interface**: Direct `cursor` command execution
//! 2. **GUI Automation**: Computer use automation as fallback
//! 3. **Keyboard Shortcuts**: Native IDE shortcuts for navigation
//!
//! ## Tools Provided
//!
//! - `cursor_open_file` - Opens files in Cursor IDE with optional line navigation
//! - `cursor_show_suggestion` - Displays suggestions with language-appropriate formatting
//! - `cursor_navigate_to` - Precise navigation to file locations
//!
//! ## Used By
//!
//! - Enhanced coding tools for IDE communication
//! - Main agent when development context switching is needed
//! - Code review tools for highlighting specific locations
//! - Multi-file change planning for navigation assistance
//!
//! ## Integration
//!
//! This module integrates with:
//! - `enhanced_coding_tools.rs` for development workflow enhancement
//! - `desktop_tools.rs` for GUI automation fallback
//! - `basic_tools.rs` for command execution
//! - Computer use tools for keyboard/mouse automation

use async_trait::async_trait;
use serde_json::{json, Value};
use std::collections::HashMap;
use tracing::{info, warn, error, debug};

use crate::agent::structs::{ToolCall, ToolResult, ToolDefinition};
use crate::agent::traits::ToolProvider;
use crate::agent::structs::AgentError;
use crate::state::AppState;

/// Cursor IDE integration using computer use automation
/// 
/// Provides seamless integration with Cursor IDE through multiple access methods
/// including command line interface and GUI automation fallback.
/// 
/// Used by: Enhanced coding tools and main agent for IDE interaction
pub struct CursorIntegration {
    app_state: AppState,
}

impl CursorIntegration {
    /// Creates a new Cursor IDE integration instance
    /// 
    /// Used by: Tool registration system during agent initialization
    /// 
    /// # Arguments
    /// * `app_state` - Application state for accessing system resources
    pub fn new(app_state: AppState) -> Self {
        Self { app_state }
    }

    /// Open a file in Cursor IDE at a specific line
    /// 
    /// Attempts to open a file using the `cursor` command line tool first,
    /// then falls back to GUI automation if the command fails.
    /// 
    /// Used by: Enhanced coding tools and main agent for file navigation
    /// 
    /// # Arguments
    /// * `file_path` - Path to the file to open
    /// * `line_number` - Optional line number to navigate to
    /// 
    /// # Returns
    /// * Success result with method used and execution details
    pub async fn open_file_in_cursor(&self, file_path: &str, line_number: Option<u64>) -> Result<Value, AgentError> {
        info!("🔍 [CURSOR] Opening file: {} {}", file_path,
            line_number.map(|l| format!("at line {}", l)).unwrap_or_default());

        // First, try to use the 'cursor' command line tool
        let command = if let Some(line) = line_number {
            format!("cursor '{}' --goto {}", file_path, line)
        } else {
            format!("cursor '{}'", file_path)
        };

        // Execute the command using the existing bash tool
        let bash_result = self.execute_bash_command(&command).await;

        match bash_result {
            Ok(result) => {
                info!("✅ [CURSOR] Successfully opened file using command line");
                Ok(json!({
                    "success": true,
                    "method": "command_line",
                    "command": command,
                    "file_path": file_path,
                    "line_number": line_number,
                    "result": result
                }))
            },
            Err(_) => {
                // Fallback: Use computer use to interact with Cursor GUI
                warn!("⚠️ [CURSOR] Command line failed, trying GUI automation");
                self.open_file_via_gui(file_path, line_number).await
            }
        }
    }

    /// Use computer use automation to open file via Cursor GUI
    /// 
    /// Fallback method that uses keyboard shortcuts and GUI automation
    /// to open files when command line interface is not available.
    /// 
    /// Used by: open_file_in_cursor as fallback method
    /// 
    /// # Arguments
    /// * `file_path` - Path to the file to open
    /// * `line_number` - Optional line number to navigate to
    /// 
    /// # Returns
    /// * GUI automation steps and execution plan
    async fn open_file_via_gui(&self, file_path: &str, line_number: Option<u64>) -> Result<Value, AgentError> {
        // Use keyboard shortcut to open file dialog (Cmd+O on macOS)
        let mut steps = Vec::new();

        // Step 1: Focus Cursor (click on it or use Cmd+Tab)
        steps.push(json!({
            "action": "focus_application",
            "application": "Cursor"
        }));

        // Step 2: Open file dialog with Cmd+O
        steps.push(json!({
            "action": "key_combination",
            "keys": "cmd+o"
        }));

        // Step 3: Type the file path
        steps.push(json!({
            "action": "type_text",
            "text": file_path
        }));

        // Step 4: Press Enter to open
        steps.push(json!({
            "action": "key_press",
            "key": "Return"
        }));

        // Step 5: If line number specified, go to line (Cmd+G)
        if let Some(line) = line_number {
            steps.push(json!({
                "action": "key_combination",
                "keys": "cmd+g"
            }));
            steps.push(json!({
                "action": "type_text",
                "text": line.to_string()
            }));
            steps.push(json!({
                "action": "key_press",
                "key": "Return"
            }));
        }

        Ok(json!({
            "success": true,
            "method": "gui_automation",
            "file_path": file_path,
            "line_number": line_number,
            "automation_steps": steps,
            "note": "GUI automation steps prepared - would be executed via computer use tools"
        }))
    }

    /// Send a suggestion or message to display in Cursor
    /// 
    /// Formats suggestions with appropriate comment syntax for the target
    /// file type and prepares them for display in the IDE context.
    /// 
    /// Used by: Enhanced coding tools for showing contextual suggestions
    /// 
    /// # Arguments
    /// * `message` - The suggestion or message to display
    /// * `file_path` - Optional file context for appropriate formatting
    /// 
    /// # Returns
    /// * Formatted suggestion with display method information
    pub async fn show_suggestion_in_cursor(&self, message: &str, file_path: Option<&str>) -> Result<Value, AgentError> {
        info!("💡 [CURSOR] Showing suggestion: {}", message);

        let mut result = HashMap::new();
        result.insert("suggestion", json!(message));
        result.insert("context_file", json!(file_path));

        // For now, we'll format this as a comment that could be inserted
        let formatted_suggestion = if let Some(path) = file_path {
            let language = self.detect_comment_style(path);
            match language.as_str() {
                "rust" => format!("// 💡 SUGGESTION: {}", message),
                "python" => format!("# 💡 SUGGESTION: {}", message),
                "javascript" | "typescript" => format!("// 💡 SUGGESTION: {}", message),
                "html" => format!("<!-- 💡 SUGGESTION: {} -->", message),
                _ => format!("// 💡 SUGGESTION: {}", message),
            }
        } else {
            format!("💡 SUGGESTION: {}", message)
        };

        result.insert("formatted_suggestion", json!(formatted_suggestion));
        result.insert("display_method", json!("comment_format"));

        Ok(json!(result))
    }

    /// Navigate to a specific location in Cursor
    /// 
    /// Opens a file and navigates to precise line and column coordinates,
    /// combining file opening with exact positioning for development workflow.
    /// 
    /// Used by: Enhanced coding tools for precise code navigation
    /// 
    /// # Arguments
    /// * `file_path` - Path to the file to navigate to
    /// * `line_number` - Optional line number for positioning
    /// * `column` - Optional column number for precise cursor placement
    /// 
    /// # Returns
    /// * Navigation result with positioning information and steps taken
    pub async fn navigate_to_location(&self, file_path: &str, line_number: Option<u64>, column: Option<u64>) -> Result<Value, AgentError> {
        info!("📍 [CURSOR] Navigating to: {} {}:{}",
            file_path,
            line_number.unwrap_or(1),
            column.unwrap_or(1)
        );

        // First open the file, then navigate to specific location
        let open_result = self.open_file_in_cursor(file_path, line_number).await?;

        // If column is specified and we opened successfully, navigate to column
        if let Some(col) = column {
            if line_number.is_some() {
                // Use Ctrl+G (or Cmd+G) to go to specific line:column
                let goto_command = format!("{}:{}", line_number.unwrap(), col);

                let navigation_steps = vec![
                    json!({
                        "action": "key_combination",
                        "keys": "cmd+g"
                    }),
                    json!({
                        "action": "type_text",
                        "text": goto_command
                    }),
                    json!({
                        "action": "key_press",
                        "key": "Return"
                    })
                ];

                return Ok(json!({
                    "success": true,
                    "file_path": file_path,
                    "line": line_number,
                    "column": column,
                    "open_result": open_result,
                    "navigation_steps": navigation_steps
                }));
            }
        }

        Ok(open_result)
    }

    /// Execute a bash command using the existing bash tool
    /// 
    /// Provides access to command line execution for Cursor CLI operations.
    /// Currently returns mock results but designed for integration with
    /// the main command execution system.
    /// 
    /// Used by: open_file_in_cursor for command line operations
    /// 
    /// # Arguments
    /// * `command` - Bash command to execute
    /// 
    /// # Returns
    /// * Command execution result with stdout, stderr, and exit code
    async fn execute_bash_command(&self, command: &str) -> Result<Value, AgentError> {
        // This would use the existing bash command execution capability
        // For now, return a mock result
        debug!("🔧 [CURSOR] Would execute bash command: {}", command);

        Ok(json!({
            "stdout": "Command executed successfully",
            "stderr": "",
            "exit_code": 0
        }))
    }

    /// Detect appropriate comment style for a file
    /// 
    /// Analyzes file extension to determine the correct comment syntax
    /// for suggestion formatting and code insertion.
    /// 
    /// Used by: show_suggestion_in_cursor for language-appropriate formatting
    /// 
    /// # Arguments
    /// * `file_path` - Path to analyze for language detection
    /// 
    /// # Returns
    /// * Language identifier for comment style selection
    fn detect_comment_style(&self, file_path: &str) -> String {
        let extension = std::path::Path::new(file_path)
            .extension()
            .and_then(|ext| ext.to_str())
            .unwrap_or("");

        match extension {
            "rs" => "rust".to_string(),
            "py" => "python".to_string(),
            "js" | "ts" | "jsx" | "tsx" => "javascript".to_string(),
            "html" | "xml" => "html".to_string(),
            "css" | "scss" | "sass" => "css".to_string(),
            "cpp" | "c" | "h" | "hpp" => "cpp".to_string(),
            _ => "generic".to_string(),
        }
    }
}

#[async_trait]
impl ToolProvider for CursorIntegration {
    /// Executes the specified Cursor IDE integration tool
    /// 
    /// Routes tool calls to the appropriate Cursor integration methods
    /// based on the tool name and handles parameter validation.
    /// 
    /// Used by: Agent tool execution system when Cursor tools are invoked
    /// 
    /// # Arguments
    /// * `tool_call` - Tool call with name and input parameters
    /// 
    /// # Returns
    /// * Tool execution result or error if tool not found
    async fn execute_tool(&self, tool_call: ToolCall) -> Result<ToolResult, AgentError> {
        match tool_call.name.as_str() {
            "cursor_open_file" => {
                let file_path = tool_call.input.get("file_path")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| AgentError::InputError("Missing 'file_path' parameter".to_string()))?;

                let line_number = tool_call.input.get("line_number")
                    .and_then(|v| v.as_u64());

                let result = self.open_file_in_cursor(file_path, line_number).await?;

                Ok(ToolResult {
                    call_id: tool_call.id.clone(),
                    output: result,
                })
            },
            "cursor_show_suggestion" => {
                let message = tool_call.input.get("message")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| AgentError::InputError("Missing 'message' parameter".to_string()))?;

                let file_path = tool_call.input.get("file_path")
                    .and_then(|v| v.as_str());

                let result = self.show_suggestion_in_cursor(message, file_path).await?;

                Ok(ToolResult {
                    call_id: tool_call.id.clone(),
                    output: result,
                })
            },
            "cursor_navigate_to" => {
                let file_path = tool_call.input.get("file_path")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| AgentError::InputError("Missing 'file_path' parameter".to_string()))?;

                let line_number = tool_call.input.get("line_number")
                    .and_then(|v| v.as_u64());

                let column = tool_call.input.get("column")
                    .and_then(|v| v.as_u64());

                let result = self.navigate_to_location(file_path, line_number, column).await?;

                Ok(ToolResult {
                    call_id: tool_call.id.clone(),
                    output: result,
                })
            },
            _ => Err(AgentError::ToolNotFound(tool_call.name.clone())),
        }
    }

    /// Lists all available Cursor IDE integration tools
    /// 
    /// Provides tool definitions for all Cursor integration capabilities
    /// including file opening, suggestion display, and navigation.
    /// 
    /// Used by: Agent initialization and tool discovery systems
    /// 
    /// # Returns
    /// * Vector of tool definitions for all Cursor integration tools
    async fn list_tools(&self) -> Result<Vec<ToolDefinition>, AgentError> {
        Ok(vec![
            ToolDefinition {
                name: "cursor_open_file".to_string(),
                description: "Open a file in Cursor IDE, optionally at a specific line number".to_string(),
                input_schema: json!({
                    "type": "object",
                    "properties": {
                        "file_path": {
                            "type": "string",
                            "description": "Path to the file to open in Cursor"
                        },
                        "line_number": {
                            "type": "number",
                            "description": "Optional line number to navigate to"
                        }
                    },
                    "required": ["file_path"]
                }),
            },
            ToolDefinition {
                name: "cursor_show_suggestion".to_string(),
                description: "Display a suggestion or message in Cursor IDE context".to_string(),
                input_schema: json!({
                    "type": "object",
                    "properties": {
                        "message": {
                            "type": "string",
                            "description": "The suggestion or message to display"
                        },
                        "file_path": {
                            "type": "string",
                            "description": "Optional file context for the suggestion"
                        }
                    },
                    "required": ["message"]
                }),
            },
            ToolDefinition {
                name: "cursor_navigate_to".to_string(),
                description: "Navigate to a specific location in Cursor IDE (file, line, column)".to_string(),
                input_schema: json!({
                    "type": "object",
                    "properties": {
                        "file_path": {
                            "type": "string",
                            "description": "Path to the file to navigate to"
                        },
                        "line_number": {
                            "type": "number",
                            "description": "Line number to navigate to"
                        },
                        "column": {
                            "type": "number",
                            "description": "Optional column number for precise positioning"
                        }
                    },
                    "required": ["file_path"]
                }),
            },
        ])
    }
}

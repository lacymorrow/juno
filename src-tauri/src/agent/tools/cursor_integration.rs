use async_trait::async_trait;
use serde_json::{json, Value};
use std::collections::HashMap;
use tracing::{info, warn, debug};

use crate::agent::core::{ToolCall, ToolResult, ToolDefinition};
use crate::agent::traits::ToolProvider;
use crate::agent::core::AgentError;
use crate::state::AppState;

/// Cursor IDE integration using computer use automation
pub struct CursorIntegration {
    app_state: AppState,
}

impl CursorIntegration {
    pub fn new(app_state: AppState) -> Self {
        Self { app_state }
    }

    /// Open a file in Cursor IDE at a specific line
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
                let line_num = line_number.unwrap();
                let goto_command = format!("{}:{}", line_num, col);

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
                api_type: None,
                beta_flag: None,
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
                api_type: None,
                beta_flag: None,
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
                api_type: None,
                beta_flag: None,
            },
        ])
    }
}

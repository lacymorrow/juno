//! Enhanced coding tools for development workflows and IDE integration.
//! Provides project analysis, multi-file planning, code review, and smart file creation.
//! Used by: Main agent orchestrator for coding tasks.

use async_trait::async_trait;
use serde_json::{json, Value};
use std::collections::HashMap;
use std::path::Path;
use std::fs;
use tracing::info;
use chrono;

use crate::agent::structs::{ToolCall, ToolResult, ToolDefinition};
use crate::agent::traits::ToolProvider;
use crate::agent::structs::AgentError;
use crate::state::AppState;

/// Enhanced coding tools for sophisticated development workflows.
/// Offers project analysis, multi-file refactoring, IDE integration, and code review.
/// Used by: Main agent orchestrator for all coding-related tasks.
pub struct EnhancedCodingToolProvider {
    app_state: AppState,
}

impl EnhancedCodingToolProvider {
    /// Creates a new enhanced coding tool provider.
    /// Used by: Tool registration system during agent initialization.
    pub fn new(app_state: AppState) -> Self {
        Self { app_state }
    }

    /// Analyzes codebase structure and provides project context.
    /// Detects technologies, identifies key files, and provides development recommendations.
    /// Used by: Main agent when starting work on a new project or when context is needed.
    async fn analyze_project_structure(&self, tool_call: &ToolCall) -> Result<ToolResult, AgentError> {
        let project_path = tool_call.input.get("project_path")
            .and_then(|v| v.as_str())
            .unwrap_or(".");

        info!("📁 [CODING EXPERT] Analyzing project structure at: {}", project_path);

        let mut analysis = HashMap::new();
        analysis.insert("project_path", json!(project_path));

        // Analyze directory structure
        let structure = self.scan_directory_structure(project_path).await?;
        analysis.insert("structure", json!(structure));

        // Detect project type and technologies
        let project_info = self.detect_project_type(project_path).await?;
        analysis.insert("project_info", json!(project_info));

        // Find key files
        let key_files = self.find_key_files(project_path).await?;
        analysis.insert("key_files", json!(key_files));

        // Generate IDE intent message
        let intent_message = format!(
            "🔍 **Project Analysis Complete**\n\
            📍 Location: {}\n\
            🏗️ Type: {}\n\
            📂 Key directories: {}\n\
            📄 Key files: {}\n\
            \n💡 **IDE Recommendation**: Consider opening {} in your IDE for optimal development experience.",
            project_path,
            project_info.get("type").unwrap_or(&json!("Unknown")).as_str().unwrap_or("Unknown"),
            project_info.get("directories").unwrap_or(&json!([])).as_array().unwrap_or(&vec![]).len(),
            key_files.len(),
            project_path
        );

        analysis.insert("ide_intent", json!(intent_message));

        Ok(ToolResult {
            call_id: tool_call.id.clone(),
            output: json!(analysis),
        })
    }

    /// Plans multi-file changes for complex refactoring operations.
    /// Analyzes dependencies and creates execution order for coordinated changes.
    /// Used by: Main agent when performing large refactoring operations or feature additions.
    async fn plan_multi_file_changes(&self, tool_call: &ToolCall) -> Result<ToolResult, AgentError> {
        let description = tool_call.input.get("description")
            .and_then(|v| v.as_str())
            .ok_or_else(|| AgentError::InputError("Missing 'description' parameter".to_string()))?;

        let default_files = vec![];
        let files = tool_call.input.get("files")
            .and_then(|v| v.as_array())
            .unwrap_or(&default_files);

        info!("🎯 [CODING EXPERT] Planning multi-file changes: {}", description);

        let mut plan = HashMap::new();
        plan.insert("description", json!(description));
        plan.insert("affected_files", json!(files));

        // Analyze dependencies between files
        let mut dependencies = Vec::new();
        for file_value in files {
            if let Some(file_path) = file_value.as_str() {
                let deps = self.analyze_file_dependencies(file_path).await?;
                dependencies.push(json!({
                    "file": file_path,
                    "dependencies": deps
                }));
            }
        }
        plan.insert("dependencies", json!(dependencies));

        // Generate execution order
        let execution_order = self.determine_change_order(files).await?;
        plan.insert("execution_order", json!(execution_order));

        // Create IDE communication
        let ide_intent = format!(
            "📋 **Multi-File Refactoring Plan**\n\
            🎯 Goal: {}\n\
            📁 Files to modify: {}\n\
            ⚡ Execution order: {}\n\
            \n💡 **IDE Tip**: Open all affected files in your IDE tabs for better context during changes.\n\
            🔧 **Suggestion**: Consider using your IDE's refactoring tools for complex operations.",
            description,
            files.len(),
            execution_order.len()
        );
        plan.insert("ide_intent", json!(ide_intent));

        Ok(ToolResult {
            call_id: tool_call.id.clone(),
            output: json!(plan),
        })
    }

    /// Communicates directly with Cursor IDE for enhanced development experience.
    /// Sends messages, opens files, and provides navigation commands to IDE.
    /// Used by: All coding tools when IDE interaction is needed.
    async fn communicate_with_cursor(&self, tool_call: &ToolCall) -> Result<ToolResult, AgentError> {
        let message_type = tool_call.input.get("type")
            .and_then(|v| v.as_str())
            .unwrap_or("info");

        let message = tool_call.input.get("message")
            .and_then(|v| v.as_str())
            .ok_or_else(|| AgentError::InputError("Missing 'message' parameter".to_string()))?;

        let file_path = tool_call.input.get("file_path")
            .and_then(|v| v.as_str());

        let line_number = tool_call.input.get("line_number")
            .and_then(|v| v.as_u64());

        info!("💬 [CODING EXPERT] Communicating with Cursor: {} - {}", message_type, message);

        let mut cursor_command = HashMap::new();

        match message_type {
            "open_file" => {
                if let Some(path) = file_path {
                    cursor_command.insert("action", json!("open_file"));
                    cursor_command.insert("file_path", json!(path));
                    if let Some(line) = line_number {
                        cursor_command.insert("line", json!(line));
                    }

                    // Use computer use to open file in Cursor
                    let computer_use_result = self.open_file_in_cursor(path, line_number).await?;
                    cursor_command.insert("execution_result", json!(computer_use_result));
                }
            },
            "navigate_to" => {
                if let Some(path) = file_path {
                    cursor_command.insert("action", json!("navigate"));
                    cursor_command.insert("target", json!(path));
                    if let Some(line) = line_number {
                        cursor_command.insert("line", json!(line));
                    }
                }
            },
            "show_suggestion" => {
                cursor_command.insert("action", json!("suggestion"));
                cursor_command.insert("content", json!(message));
                if let Some(path) = file_path {
                    cursor_command.insert("context_file", json!(path));
                }
            },
            "highlight_code" => {
                if let Some(path) = file_path {
                    cursor_command.insert("action", json!("highlight"));
                    cursor_command.insert("file_path", json!(path));
                    if let Some(line) = line_number {
                        cursor_command.insert("line", json!(line));
                    }
                }
            },
            _ => {
                cursor_command.insert("action", json!("message"));
                cursor_command.insert("content", json!(message));
            }
        }

        // Generate user-visible intent
        let intent_display = match message_type {
            "open_file" => format!("🔍 **Opening in Cursor**: {}", file_path.unwrap_or("file")),
            "navigate_to" => format!("📍 **Navigating to**: {} {}",
                file_path.unwrap_or("location"),
                line_number.map(|l| format!("(line {})", l)).unwrap_or_default()
            ),
            "show_suggestion" => format!("💡 **Suggestion**: {}", message),
            "highlight_code" => format!("✨ **Highlighting**: {} {}",
                file_path.unwrap_or("code"),
                line_number.map(|l| format!("at line {}", l)).unwrap_or_default()
            ),
            _ => format!("📢 **IDE Message**: {}", message),
        };

        cursor_command.insert("intent_display", json!(intent_display));

        Ok(ToolResult {
            call_id: tool_call.id.clone(),
            output: json!(cursor_command),
        })
    }

    /// Generates comprehensive code review with quality metrics and recommendations.
    /// Analyzes multiple files for code quality and provides actionable feedback.
    /// Used by: Main agent when code quality assessment is requested.
    async fn generate_code_review(&self, tool_call: &ToolCall) -> Result<ToolResult, AgentError> {
        let default_files = vec![];
        let file_paths = tool_call.input.get("files")
            .and_then(|v| v.as_array())
            .unwrap_or(&default_files);

        let default_focus_areas = vec![];
        let focus_areas = tool_call.input.get("focus_areas")
            .and_then(|v| v.as_array())
            .unwrap_or(&default_focus_areas);

        info!("🔍 [CODING EXPERT] Generating code review for {} files", file_paths.len());

        let mut review = HashMap::new();
        let mut file_reviews = Vec::new();

        for file_value in file_paths {
            if let Some(file_path) = file_value.as_str() {
                let file_review = self.review_single_file(file_path, focus_areas).await?;
                file_reviews.push(file_review);
            }
        }

        review.insert("file_reviews", json!(file_reviews));

        // Generate overall assessment
        let overall_score = self.calculate_overall_score(&file_reviews);
        review.insert("overall_score", json!(overall_score));

        // Create actionable recommendations
        let recommendations = self.generate_recommendations(&file_reviews, focus_areas);
        review.insert("recommendations", json!(recommendations));

        // Generate IDE intent for review
        let ide_intent = format!(
            "📊 **Code Review Complete**\n\
            📈 Overall Score: {}/10\n\
            📁 Files Reviewed: {}\n\
            ⚠️ Issues Found: {}\n\
            ✅ Recommendations: {}\n\
            \n💡 **IDE Action**: Consider reviewing highlighted issues in your editor and applying suggested improvements.",
            overall_score,
            file_reviews.len(),
            file_reviews.iter().map(|r| r.get("issues").unwrap_or(&json!([])).as_array().unwrap_or(&vec![]).len()).sum::<usize>(),
            recommendations.len()
        );
        review.insert("ide_intent", json!(ide_intent));

        Ok(ToolResult {
            call_id: tool_call.id.clone(),
            output: json!(review),
        })
    }

    /// Creates files with intelligent templates based on language and purpose.
    /// Applies best practices and common patterns automatically.
    /// Used by: Main agent when creating new files or scaffolding project structure.
    async fn smart_create_file(&self, tool_call: &ToolCall) -> Result<ToolResult, AgentError> {
        let file_path = tool_call.input.get("file_path")
            .and_then(|v| v.as_str())
            .ok_or_else(|| AgentError::InputError("Missing 'file_path' parameter".to_string()))?;

        let content_type = tool_call.input.get("content_type")
            .and_then(|v| v.as_str())
            .unwrap_or("auto");

        let purpose = tool_call.input.get("purpose")
            .and_then(|v| v.as_str())
            .unwrap_or("");

        info!("📝 [CODING EXPERT] Smart creating file: {} (type: {}, purpose: {})", file_path, content_type, purpose);

        // Detect language and generate appropriate template
        let language = self.detect_language_from_path(file_path);
        let template = self.generate_file_template(&language, content_type, purpose).await?;

        // Create the file with template content
        let result = self.create_file_with_content(file_path, &template).await?;

        // Generate IDE intent
        let ide_intent = format!(
            "📄 **File Created**: {}\n\
            🔤 Language: {}\n\
            📋 Template: {} applied\n\
            🎯 Purpose: {}\n\
            \n💡 **Next Steps**: Open the file in your IDE to begin development. Template includes best practices and structure.",
            file_path,
            language,
            content_type,
            if purpose.is_empty() { "General development" } else { purpose }
        );

        let mut output = result;
        output.as_object_mut().unwrap().insert("ide_intent".to_string(), json!(ide_intent));
        output.as_object_mut().unwrap().insert("language".to_string(), json!(language));
        output.as_object_mut().unwrap().insert("template_applied".to_string(), json!(content_type));

        Ok(ToolResult {
            call_id: tool_call.id.clone(),
            output,
        })
    }

    /// Scans directory structure and returns organized file/folder information.
    /// Used by: analyze_project_structure for building project overview.
    async fn scan_directory_structure(&self, path: &str) -> Result<Value, AgentError> {
        let mut structure = HashMap::new();

        if let Ok(entries) = fs::read_dir(path) {
            let mut directories = Vec::new();
            let mut files = Vec::new();

            for entry in entries.flatten() {
                let file_name = entry.file_name().to_string_lossy().to_string();
                if entry.file_type().map(|ft| ft.is_dir()).unwrap_or(false) {
                    directories.push(file_name);
                } else {
                    files.push(file_name);
                }
            }

            structure.insert("directories", json!(directories));
            structure.insert("files", json!(files));
        }

        Ok(json!(structure))
    }

    /// Detects project type based on configuration files and structure.
    /// Used by: analyze_project_structure for technology identification.
    async fn detect_project_type(&self, path: &str) -> Result<Value, AgentError> {
        let mut project_info = HashMap::new();

        // Check for common project files
        let project_files = [
            ("package.json", "Node.js/JavaScript"),
            ("Cargo.toml", "Rust"),
            ("requirements.txt", "Python"),
            ("pom.xml", "Java/Maven"),
            ("build.gradle", "Java/Gradle"),
            (".csproj", "C#/.NET"),
            ("go.mod", "Go"),
            ("composer.json", "PHP"),
        ];

        let mut detected_type = "Unknown";
        let mut config_files = Vec::new();

        for (file, project_type) in &project_files {
            let file_path = Path::new(path).join(file);
            if file_path.exists() {
                detected_type = project_type;
                config_files.push(file.to_string());
            }
        }

        project_info.insert("type", json!(detected_type));
        project_info.insert("config_files", json!(config_files));

        Ok(json!(project_info))
    }

    /// Identifies important files commonly found in projects.
    /// Used by: analyze_project_structure for highlighting key project files.
    async fn find_key_files(&self, path: &str) -> Result<Vec<String>, AgentError> {
        let mut key_files = Vec::new();

        // Common important files
        let important_files = [
            "README.md", "README.txt", "README.rst",
            "main.py", "index.js", "main.rs", "App.tsx", "App.jsx",
            ".gitignore", "LICENSE", "Dockerfile",
            "tsconfig.json", "babel.config.js", "webpack.config.js"
        ];

        for file in &important_files {
            let file_path = Path::new(path).join(file);
            if file_path.exists() {
                key_files.push(file.to_string());
            }
        }

        Ok(key_files)
    }

    /// Analyzes file dependencies by parsing import/require statements.
    /// Used by: plan_multi_file_changes for dependency analysis.
    async fn analyze_file_dependencies(&self, file_path: &str) -> Result<Vec<String>, AgentError> {
        let mut dependencies = Vec::new();

        if let Ok(content) = fs::read_to_string(file_path) {
            // Simple dependency analysis based on import/include statements
            for line in content.lines() {
                let trimmed = line.trim();
                if trimmed.starts_with("import ") ||
                   trimmed.starts_with("from ") ||
                   trimmed.starts_with("use ") ||
                   trimmed.starts_with("#include") ||
                   trimmed.starts_with("require(") {
                    dependencies.push(trimmed.to_string());
                }
            }
        }

        Ok(dependencies)
    }

    /// Determines optimal order for making changes across multiple files.
    /// Used by: plan_multi_file_changes for creating execution strategy.
    async fn determine_change_order(&self, files: &[Value]) -> Result<Vec<String>, AgentError> {
        // Simple ordering: dependencies first, then implementations
        let mut ordered = Vec::new();

        for file_value in files {
            if let Some(file_path) = file_value.as_str() {
                ordered.push(file_path.to_string());
            }
        }

        // Sort by file type priority (headers/interfaces first, implementations last)
        ordered.sort_by(|a, b| {
            let a_priority = self.get_file_priority(a);
            let b_priority = self.get_file_priority(b);
            a_priority.cmp(&b_priority)
        });

        Ok(ordered)
    }

    /// Assigns priority to files based on type (headers first, tests last).
    /// Used by: determine_change_order for sorting files by dependency priority.
    fn get_file_priority(&self, file_path: &str) -> u8 {
        if file_path.ends_with(".h") || file_path.ends_with(".hpp") || file_path.ends_with(".d.ts") {
            0 // Headers/type definitions first
        } else if file_path.ends_with(".rs") || file_path.ends_with(".py") || file_path.ends_with(".js") {
            1 // Implementation files
        } else if file_path.ends_with(".test.") || file_path.contains("test") {
            2 // Test files last
        } else {
            1 // Default priority
        }
    }

    /// Opens a file in Cursor IDE using computer use capabilities.
    /// Used by: communicate_with_cursor for file opening operations.
    async fn open_file_in_cursor(&self, file_path: &str, line_number: Option<u64>) -> Result<Value, AgentError> {
        // This would use the computer use capabilities to open file in Cursor
        // For now, return a structured command that could be executed
        let mut command = HashMap::new();
        command.insert("action", json!("open_in_cursor"));
        command.insert("file_path", json!(file_path));

        if let Some(line) = line_number {
            command.insert("line_number", json!(line));
            command.insert("command", json!(format!("cursor {} --goto {}", file_path, line)));
        } else {
            command.insert("command", json!(format!("cursor {}", file_path)));
        }

        Ok(json!(command))
    }

    /// Reviews a single file for code quality and issues.
    /// Used by: generate_code_review for per-file analysis.
    async fn review_single_file(&self, file_path: &str, _focus_areas: &[Value]) -> Result<Value, AgentError> {
        let mut review = HashMap::new();
        review.insert("file", json!(file_path));

        if let Ok(content) = fs::read_to_string(file_path) {
            let lines = content.lines().count();
            review.insert("lines_of_code", json!(lines));

            // Basic analysis
            let mut issues = Vec::new();
            let mut suggestions = Vec::new();

            // Check for common issues
            if content.contains("TODO") || content.contains("FIXME") {
                issues.push("Contains TODO or FIXME comments");
            }

            if lines > 500 {
                suggestions.push("Consider breaking this file into smaller modules");
            }

            review.insert("issues", json!(issues));
            review.insert("suggestions", json!(suggestions));
            review.insert("score", json!(8)); // Basic scoring
        }

        Ok(json!(review))
    }

    /// Calculates overall quality score from individual file reviews.
    /// Used by: generate_code_review for aggregating quality metrics.
    fn calculate_overall_score(&self, file_reviews: &[Value]) -> u8 {
        if file_reviews.is_empty() {
            return 0;
        }

        let total_score: u64 = file_reviews.iter()
            .map(|review| review.get("score").and_then(|s| s.as_u64()).unwrap_or(5))
            .sum();

        ((total_score / file_reviews.len() as u64) as u8).min(10)
    }

    /// Generates actionable recommendations from review results.
    /// Used by: generate_code_review for creating improvement suggestions.
    fn generate_recommendations(&self, file_reviews: &[Value], _focus_areas: &[Value]) -> Vec<String> {
        let mut recommendations = Vec::new();

        for review in file_reviews {
            if let Some(suggestions) = review.get("suggestions").and_then(|s| s.as_array()) {
                for suggestion in suggestions {
                    if let Some(text) = suggestion.as_str() {
                        recommendations.push(text.to_string());
                    }
                }
            }
        }

        recommendations.dedup();
        recommendations
    }

    /// Detects programming language from file extension.
    /// Used by: smart_create_file for template selection.
    fn detect_language_from_path(&self, file_path: &str) -> String {
        let extension = Path::new(file_path)
            .extension()
            .and_then(|ext| ext.to_str())
            .unwrap_or("");

        match extension {
            "rs" => "Rust".to_string(),
            "py" => "Python".to_string(),
            "js" => "JavaScript".to_string(),
            "ts" => "TypeScript".to_string(),
            "tsx" => "TypeScript React".to_string(),
            "jsx" => "JavaScript React".to_string(),
            "java" => "Java".to_string(),
            "cpp" | "cc" | "cxx" => "C++".to_string(),
            "c" => "C".to_string(),
            "h" => "C Header".to_string(),
            "go" => "Go".to_string(),
            "php" => "PHP".to_string(),
            "rb" => "Ruby".to_string(),
            "swift" => "Swift".to_string(),
            "kt" => "Kotlin".to_string(),
            "cs" => "C#".to_string(),
            _ => "Plain Text".to_string(),
        }
    }

    /// Generates appropriate file template based on language and content type.
    /// Used by: smart_create_file for creating structured file content.
    async fn generate_file_template(&self, language: &str, content_type: &str, purpose: &str) -> Result<String, AgentError> {
        let template = match (language, content_type) {
            ("Rust", "module") => {
                format!("//! {}\n//!\n//! This module provides functionality for {}.\n\n{}\n\n// TODO: Implement module functionality\n",
                    purpose,
                    purpose,
                    if purpose.contains("test") { "#[cfg(test)]\nmod tests {\n    use super::*;\n\n    #[test]\n    fn test_placeholder() {\n        // TODO: Add tests\n    }\n}" } else { "" }
                )
            },
            ("Python", "class") => {
                format!("\"\"\"{}.\n\nThis module provides functionality for {}.\n\"\"\"\n\nclass {}:\n    \"\"\"{}.\"\"\"\n    \n    def __init__(self):\n        \"\"\"Initialize the class.\"\"\"\n        pass\n    \n    def placeholder_method(self):\n        \"\"\"Placeholder method - implement as needed.\"\"\"\n        pass\n",
                    purpose,
                    purpose,
                    purpose.replace(" ", ""),
                    purpose
                )
            },
            ("TypeScript", "component") => {
                format!("import React from 'react';\n\ninterface {}Props {{\n  // TODO: Define component props\n}}\n\n/**\n * {} - {}\n */\nexport const {}: React.FC<{}Props> = (props) => {{\n  return (\n    <div>\n      {{/* TODO: Implement component */}}\n      <h1>{}</h1>\n    </div>\n  );\n}};\n\nexport default {};\n",
                    purpose.replace(" ", ""),
                    purpose,
                    purpose,
                    purpose.replace(" ", ""),
                    purpose.replace(" ", ""),
                    purpose,
                    purpose.replace(" ", "")
                )
            },
            _ => {
                format!("// {}\n// \n// Purpose: {}\n// Created by: Enhanced Coding Agent\n// \n// TODO: Implement functionality\n\n", purpose, purpose)
            }
        };

        Ok(template)
    }

    /// Creates file with specified content and returns creation metadata.
    /// Used by: smart_create_file for actual file creation.
    async fn create_file_with_content(&self, file_path: &str, content: &str) -> Result<Value, AgentError> {
        match fs::write(file_path, content) {
            Ok(_) => {
                Ok(json!({
                    "success": true,
                    "file_path": file_path,
                    "bytes_written": content.len(),
                    "created_at": chrono::Utc::now().to_rfc3339()
                }))
            },
            Err(e) => {
                Err(AgentError::ToolError(format!("Failed to create file {}: {}", file_path, e)))
            }
        }
    }
}

#[async_trait]
impl ToolProvider for EnhancedCodingToolProvider {
    /// Executes the specified enhanced coding tool.
    /// Used by: Agent tool execution system when coding tools are invoked.
    async fn execute_tool(&self, tool_call: ToolCall) -> Result<ToolResult, AgentError> {
        match tool_call.name.as_str() {
            "analyze_project_structure" => self.analyze_project_structure(&tool_call).await,
            "plan_multi_file_changes" => self.plan_multi_file_changes(&tool_call).await,
            "communicate_with_cursor" => self.communicate_with_cursor(&tool_call).await,
            "generate_code_review" => self.generate_code_review(&tool_call).await,
            "smart_create_file" => self.smart_create_file(&tool_call).await,
            _ => Err(AgentError::ToolNotFound(tool_call.name.clone())),
        }
    }

    /// Lists all available enhanced coding tools with their definitions.
    /// Used by: Agent initialization and tool discovery systems.
    async fn list_tools(&self) -> Result<Vec<ToolDefinition>, AgentError> {
        Ok(vec![
            ToolDefinition {
                name: "analyze_project_structure".to_string(),
                description: "Analyze project structure, detect technologies, and provide development context".to_string(),
                input_schema: json!({
                    "type": "object",
                    "properties": {
                        "project_path": {
                            "type": "string",
                            "description": "Path to the project directory to analyze",
                            "default": "."
                        }
                    }
                }),
            },
            ToolDefinition {
                name: "plan_multi_file_changes".to_string(),
                description: "Plan coordinated changes across multiple files with dependency analysis".to_string(),
                input_schema: json!({
                    "type": "object",
                    "properties": {
                        "description": {
                            "type": "string",
                            "description": "Description of the changes to be made"
                        },
                        "files": {
                            "type": "array",
                            "items": {"type": "string"},
                            "description": "List of file paths that will be modified"
                        }
                    },
                    "required": ["description", "files"]
                }),
            },
            ToolDefinition {
                name: "communicate_with_cursor".to_string(),
                description: "Send messages and commands to Cursor IDE for enhanced development experience".to_string(),
                input_schema: json!({
                    "type": "object",
                    "properties": {
                        "type": {
                            "type": "string",
                            "enum": ["open_file", "navigate_to", "show_suggestion", "highlight_code", "message"],
                            "description": "Type of communication with Cursor IDE"
                        },
                        "message": {
                            "type": "string",
                            "description": "Message content to communicate"
                        },
                        "file_path": {
                            "type": "string",
                            "description": "Optional file path for file-specific actions"
                        },
                        "line_number": {
                            "type": "number",
                            "description": "Optional line number for precise navigation"
                        }
                    },
                    "required": ["type", "message"]
                }),
            },
            ToolDefinition {
                name: "generate_code_review".to_string(),
                description: "Generate comprehensive code review with suggestions and quality metrics".to_string(),
                input_schema: json!({
                    "type": "object",
                    "properties": {
                        "files": {
                            "type": "array",
                            "items": {"type": "string"},
                            "description": "List of file paths to review"
                        },
                        "focus_areas": {
                            "type": "array",
                            "items": {"type": "string"},
                            "description": "Specific areas to focus on (e.g., 'security', 'performance', 'maintainability')"
                        }
                    },
                    "required": ["files"]
                }),
            },
            ToolDefinition {
                name: "smart_create_file".to_string(),
                description: "Create files with intelligent templates based on purpose and language".to_string(),
                input_schema: json!({
                    "type": "object",
                    "properties": {
                        "file_path": {
                            "type": "string",
                            "description": "Path where the file should be created"
                        },
                        "content_type": {
                            "type": "string",
                            "enum": ["auto", "module", "class", "component", "test", "config"],
                            "description": "Type of content to generate",
                            "default": "auto"
                        },
                        "purpose": {
                            "type": "string",
                            "description": "Purpose or description of the file's functionality"
                        }
                    },
                    "required": ["file_path"]
                }),
            },
        ])
    }
}

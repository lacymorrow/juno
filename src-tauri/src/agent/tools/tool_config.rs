use std::collections::HashMap;
use std::path::PathBuf;
use std::fs;
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Manager};
use tracing::info;

// Re-export MCP types for convenience
pub use super::mcp_integration::{MCPServerConfig, MCPServerStatus, MCPToolInfo};

/// Categories of tools for organization in the UI
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ToolCategory {
    /// Official Anthropic Computer Use tools (screenshot, mouse, keyboard, etc.)
    AnthropicComputerUse,
    /// Desktop automation tools (applications, system control)
    Desktop,
    /// Browser automation and web interaction tools
    Browser,
    /// Timer and scheduling tools
    Timer,
    /// Basic file and text manipulation tools
    Basic,
    /// MCP (Model Context Protocol) tools from external servers
    MCP,
}

impl ToolCategory {
    pub fn display_name(&self) -> &'static str {
        match self {
            ToolCategory::AnthropicComputerUse => "Anthropic Computer Use",
            ToolCategory::Desktop => "Desktop Automation",
            ToolCategory::Browser => "Browser Tools",
            ToolCategory::Timer => "Timer & Scheduling",
            ToolCategory::Basic => "Basic Tools",
            ToolCategory::MCP => "MCP Tools",
        }
    }

    pub fn description(&self) -> &'static str {
        match self {
            ToolCategory::AnthropicComputerUse => "Official Anthropic Computer Use tools for screen interaction",
            ToolCategory::Desktop => "macOS desktop automation and application control",
            ToolCategory::Browser => "Web browser automation and control",
            ToolCategory::Timer => "Task scheduling and timer management",
            ToolCategory::Basic => "File operations and basic text manipulation",
            ToolCategory::MCP => "External MCP server tools and integrations",
        }
    }

    pub fn all_categories() -> Vec<ToolCategory> {
        vec![
            ToolCategory::AnthropicComputerUse,
            ToolCategory::Desktop,
            ToolCategory::Browser,
            ToolCategory::Timer,
            ToolCategory::Basic,
            ToolCategory::MCP,
        ]
    }
}

/// Configuration for an individual tool
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolConfig {
    pub name: String,
    pub category: ToolCategory,
    pub enabled: bool,
    pub description: Option<String>,
    pub required: bool, // Some tools might be required and cannot be disabled
    pub server_id: Option<String>, // For MCP tools, which server they belong to
}

impl ToolConfig {
    pub fn new(name: String, category: ToolCategory, enabled: bool) -> Self {
        Self {
            name,
            category,
            enabled,
            description: None,
            required: false,
            server_id: None,
        }
    }

    pub fn new_mcp_tool(name: String, server_id: String, enabled: bool) -> Self {
        Self {
            name,
            category: ToolCategory::MCP,
            enabled,
            description: None,
            required: false,
            server_id: Some(server_id),
        }
    }

    pub fn with_description(mut self, description: String) -> Self {
        self.description = Some(description);
        self
    }

    pub fn as_required(mut self) -> Self {
        self.required = true;
        self.enabled = true; // Required tools are always enabled
        self
    }
}

/// Manager for tool configurations
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolConfigManager {
    pub tools: HashMap<String, ToolConfig>,
    pub category_enabled: HashMap<ToolCategory, bool>,
    pub mcp_servers: HashMap<String, MCPServerConfig>, // Store MCP server configurations
}

impl Default for ToolConfigManager {
    fn default() -> Self {
        let mut tools = HashMap::new();
        let mut category_enabled = HashMap::new();

        // Initialize default tool configurations
        Self::add_default_anthropic_tools(&mut tools);
        Self::add_default_desktop_tools(&mut tools);
        Self::add_default_browser_tools(&mut tools);
        Self::add_default_timer_tools(&mut tools);
        Self::add_default_basic_tools(&mut tools);

        // Enable all categories by default
        for category in ToolCategory::all_categories() {
            category_enabled.insert(category, true);
        }

        Self {
            tools,
            category_enabled,
            mcp_servers: HashMap::new(),
        }
    }
}

impl ToolConfigManager {
    /// Create a new tool configuration manager with defaults
    pub fn new() -> Self {
        Self::default()
    }

    /// Load configuration from file or create default
    pub fn load_from_file(config_path: &PathBuf) -> Result<Self, String> {
        if config_path.exists() {
            let content = fs::read_to_string(config_path)
                .map_err(|e| format!("Failed to read tool config: {}", e))?;

            let mut config: Self = serde_json::from_str(&content)
                .map_err(|e| format!("Failed to parse tool config: {}", e))?;

            // Ensure all default tools are present (for backwards compatibility)
            Self::ensure_default_tools(&mut config.tools);

            info!("Loaded tool configuration from {}", config_path.display());
            Ok(config)
        } else {
            info!("No tool configuration found, creating default");
            let default_config = Self::default();
            default_config.save_to_file(config_path)?;
            Ok(default_config)
        }
    }

    /// Save configuration to file
    pub fn save_to_file(&self, config_path: &PathBuf) -> Result<(), String> {
        // Ensure parent directory exists
        if let Some(parent) = config_path.parent() {
            fs::create_dir_all(parent)
                .map_err(|e| format!("Failed to create config directory: {}", e))?;
        }

        let content = serde_json::to_string_pretty(self)
            .map_err(|e| format!("Failed to serialize tool config: {}", e))?;

        fs::write(config_path, content)
            .map_err(|e| format!("Failed to write tool config: {}", e))?;

        info!("Saved tool configuration to {}", config_path.display());
        Ok(())
    }

    /// Get configuration path for the app
    pub fn get_config_path(app_handle: &AppHandle) -> Result<PathBuf, String> {
        let app_dir = app_handle.path().app_config_dir()
            .map_err(|e| format!("Failed to get app config directory: {}", e))?;
        Ok(app_dir.join("tool_config.json"))
    }

    /// Check if a tool is enabled
    pub fn is_tool_enabled(&self, tool_name: &str) -> bool {
        if let Some(tool_config) = self.tools.get(tool_name) {
            if tool_config.required {
                return true; // Required tools are always enabled
            }

            // Check both tool-specific and category-wide settings
            let category_enabled = self.category_enabled.get(&tool_config.category).unwrap_or(&true);
            tool_config.enabled && *category_enabled
        } else {
            false // Unknown tools are disabled by default
        }
    }

    /// Enable or disable a specific tool
    pub fn set_tool_enabled(&mut self, tool_name: &str, enabled: bool) {
        if let Some(tool_config) = self.tools.get_mut(tool_name) {
            if !(tool_config.required && !enabled) {
                tool_config.enabled = enabled;
            }
        }
    }

    /// Enable or disable an entire category of tools
    pub fn set_category_enabled(&mut self, category: &ToolCategory, enabled: bool) {
        // Don't disable categories with required tools
        if !enabled {
            let has_required = self.tools.iter()
                .any(|(_, config)| config.category == *category && config.required);
            if has_required {
                return;
            }
        }
        self.category_enabled.insert(category.clone(), enabled);
    }

    /// Get all tools in a category
    pub fn get_tools_by_category(&self, category: &ToolCategory) -> Vec<&ToolConfig> {
        self.tools.values()
            .filter(|config| config.category == *category)
            .collect()
    }

    /// Get all enabled tools
    pub fn get_enabled_tools(&self) -> Vec<&ToolConfig> {
        self.tools.iter()
            .filter(|(name, _)| self.is_tool_enabled(name))
            .map(|(_, config)| config)
            .collect()
    }

    /// Get tool configuration by name
    pub fn get_tool_config(&self, tool_name: &str) -> Option<ToolConfig> {
        self.tools.get(tool_name).cloned()
    }

    /// Add or update a tool configuration
    pub fn add_tool_config(&mut self, config: ToolConfig) {
        self.tools.insert(config.name.clone(), config);
    }

    /// Get enabled tool names
    pub fn get_enabled_tool_names(&self) -> Vec<String> {
        self.tools.iter()
            .filter(|(name, _)| self.is_tool_enabled(name))
            .map(|(name, _)| name.clone())
            .collect()
    }

    /// Reset configuration to defaults
    pub fn reset_to_defaults(&mut self) {
        let mut new_config = Self::default();
        
        // Preserve MCP server configurations
        new_config.mcp_servers = self.mcp_servers.clone();
        
        *self = new_config;
    }

    // MCP Server Management Methods

    /// Add an MCP server configuration
    pub fn add_mcp_server(&mut self, config: MCPServerConfig) {
        self.mcp_servers.insert(config.id.clone(), config);
    }

    /// Remove an MCP server configuration
    pub fn remove_mcp_server(&mut self, server_id: &str) {
        self.mcp_servers.remove(server_id);
        
        // Also remove all tools from this server
        self.tools.retain(|_, tool_config| {
            tool_config.server_id.as_ref() != Some(&server_id.to_string())
        });
    }

    /// Get all MCP server configurations
    pub fn get_mcp_servers(&self) -> Vec<MCPServerConfig> {
        self.mcp_servers.values().cloned().collect()
    }

    /// Get MCP server configuration by ID
    pub fn get_mcp_server(&self, server_id: &str) -> Option<MCPServerConfig> {
        self.mcp_servers.get(server_id).cloned()
    }

    /// Update MCP server configuration
    pub fn update_mcp_server(&mut self, config: MCPServerConfig) {
        self.mcp_servers.insert(config.id.clone(), config);
    }

    /// Add tools from an MCP server
    pub fn add_mcp_tools(&mut self, server_id: &str, tools: Vec<MCPToolInfo>) {
        for tool_info in tools {
            let tool_config = ToolConfig::new_mcp_tool(
                tool_info.tool_definition.name.clone(),
                server_id.to_string(),
                tool_info.enabled,
            ).with_description(tool_info.tool_definition.description);

            self.add_tool_config(tool_config);
        }
    }

    /// Get all MCP tools for a specific server
    pub fn get_mcp_tools_for_server(&self, server_id: &str) -> Vec<&ToolConfig> {
        self.tools.values()
            .filter(|config| {
                config.category == ToolCategory::MCP && 
                config.server_id.as_ref() == Some(&server_id.to_string())
            })
            .collect()
    }

    /// Check if an MCP server is enabled
    pub fn is_mcp_server_enabled(&self, server_id: &str) -> bool {
        self.mcp_servers.get(server_id)
            .map(|config| config.enabled)
            .unwrap_or(false)
    }

    /// Enable or disable an MCP server
    pub fn set_mcp_server_enabled(&mut self, server_id: &str, enabled: bool) {
        if let Some(server_config) = self.mcp_servers.get_mut(server_id) {
            server_config.enabled = enabled;
        }
    }

    // Default tool initialization methods
    fn add_default_anthropic_tools(tools: &mut HashMap<String, ToolConfig>) {
        let anthropic_tools = vec![
            ("screenshot", "Take a screenshot of the current screen"),
            ("click", "Click on screen coordinates"),
            ("type", "Type text into the focused application"),
            ("key", "Press keyboard keys and combinations"),
            ("scroll", "Scroll in a direction"),
            ("wait", "Wait for a specified duration"),
            ("move", "Move mouse to coordinates"),
            ("drag", "Drag from one coordinate to another"),
        ];

        for (name, description) in anthropic_tools {
            let config = ToolConfig::new(
                name.to_string(),
                ToolCategory::AnthropicComputerUse,
                true,
            )
            .with_description(description.to_string())
            .as_required(); // Computer use tools are required for core functionality

            tools.insert(name.to_string(), config);
        }
    }

    fn add_default_desktop_tools(tools: &mut HashMap<String, ToolConfig>) {
        let desktop_tools = vec![
            ("launch_application", "Launch applications by name"),
            ("get_running_applications", "List currently running applications"),
            ("focus_application", "Bring application to front"),
            ("quit_application", "Quit an application"),
            ("get_system_info", "Get system information"),
            ("manage_audio", "Control system audio settings"),
        ];

        for (name, description) in desktop_tools {
            let config = ToolConfig::new(
                name.to_string(),
                ToolCategory::Desktop,
                true,
            ).with_description(description.to_string());

            tools.insert(name.to_string(), config);
        }
    }

    fn add_default_browser_tools(tools: &mut HashMap<String, ToolConfig>) {
        let browser_tools = vec![
            ("browser_navigate", "Navigate to a URL"),
            ("browser_click", "Click elements in the browser"),
            ("browser_type", "Type text in browser forms"),
            ("browser_scroll", "Scroll browser pages"),
            ("browser_screenshot", "Take browser screenshots"),
            ("browser_get_content", "Extract page content"),
        ];

        for (name, description) in browser_tools {
            let config = ToolConfig::new(
                name.to_string(),
                ToolCategory::Browser,
                true,
            ).with_description(description.to_string());

            tools.insert(name.to_string(), config);
        }
    }

    fn add_default_timer_tools(tools: &mut HashMap<String, ToolConfig>) {
        let timer_tools = vec![
            ("create_timer", "Create a scheduled timer"),
            ("list_timers", "List active timers"),
            ("cancel_timer", "Cancel a timer"),
            ("timer_status", "Check timer status"),
        ];

        for (name, description) in timer_tools {
            let config = ToolConfig::new(
                name.to_string(),
                ToolCategory::Timer,
                true,
            ).with_description(description.to_string());

            tools.insert(name.to_string(), config);
        }
    }

    fn add_default_basic_tools(tools: &mut HashMap<String, ToolConfig>) {
        let basic_tools = vec![
            ("read_file", "Read file contents"),
            ("write_file", "Write content to file"),
            ("list_directory", "List directory contents"),
            ("create_directory", "Create directories"),
            ("delete_file", "Delete files"),
            ("text_editor_edit", "Edit text files"),
            ("execute_shell_command", "Execute shell commands"),
        ];

        for (name, description) in basic_tools {
            let config = ToolConfig::new(
                name.to_string(),
                ToolCategory::Basic,
                true,
            ).with_description(description.to_string());

            tools.insert(name.to_string(), config);
        }
    }

    /// Ensure all default tools are present (for backwards compatibility)
    fn ensure_default_tools(tools: &mut HashMap<String, ToolConfig>) {
        let mut default_tools = HashMap::new();
        Self::add_default_anthropic_tools(&mut default_tools);
        Self::add_default_desktop_tools(&mut default_tools);
        Self::add_default_browser_tools(&mut default_tools);
        Self::add_default_timer_tools(&mut default_tools);
        Self::add_default_basic_tools(&mut default_tools);

        // Add missing default tools
        for (name, config) in default_tools {
            if !tools.contains_key(&name) {
                tools.insert(name, config);
            }
        }
    }
}

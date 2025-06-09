//! Tool Configuration Management for Juno AI Computer Use Agent
//!
//! This module provides comprehensive configuration management for all tools in the agent system,
//! including categorization, enablement controls, MCP server management, and persistent storage.
//!
//! ## Core Features
//!
//! - **Tool Categories**: Organized grouping of tools by functionality (Computer Use, Desktop, Browser, etc.)
//! - **Enablement Control**: Individual tool and category-level enable/disable functionality
//! - **MCP Integration**: Configuration management for external MCP servers and their tools
//! - **Persistence**: Save/load configurations from JSON files with app data directory support
//! - **Backwards Compatibility**: Automatic migration and default tool addition for existing configs
//!
//! ## Tool Categories
//!
//! 1. **AnthropicComputerUse** - Official Anthropic Computer Use tools (screenshot, mouse, keyboard, etc.)
//! 2. **Desktop** - macOS desktop automation and application control
//! 3. **Browser** - Web browser automation and control tools
//! 4. **Timer** - Task scheduling and timer management tools
//! 5. **Basic** - File operations and basic text manipulation
//! 6. **MCP** - External MCP server tools and integrations
//!
//! ## Used By
//!
//! - Main agent system for tool availability decisions
//! - Settings UI for configuration management
//! - Tool registration system during agent startup
//! - MCP manager for external tool integration
//!
//! ## Integration
//!
//! This module integrates with:
//! - `mcp_integration.rs` for external server management
//! - All tool provider modules for enablement checking
//! - Tauri app state for persistent storage
//! - Settings UI for user configuration

use std::collections::HashMap;
use std::path::PathBuf;
use std::fs;
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Manager};
use tracing::info;

// Re-export MCP types for convenience
pub use super::mcp_integration::{MCPServerConfig, MCPServerStatus, MCPToolInfo};

/// Categories of tools for organization in the UI
/// 
/// Provides semantic grouping of tools by functionality to enable category-level
/// management and better user experience in configuration interfaces.
/// 
/// Used by: Settings UI, tool configuration system, and enablement checking
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
    /// Returns the human-readable display name for the category
    /// 
    /// Used by: Settings UI for category labels and display
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

    /// Returns a description of what tools are in this category
    /// 
    /// Used by: Settings UI for tooltips and help text
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

    /// Returns all available tool categories
    /// 
    /// Used by: Settings UI for iterating over all categories
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
/// 
/// Contains all settings for a single tool including enablement state,
/// category membership, and metadata for display and management.
/// 
/// Used by: Tool configuration manager and settings UI
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
    /// Creates a new tool configuration with basic settings
    /// 
    /// Used by: Default tool initialization and configuration builders
    /// 
    /// # Arguments
    /// * `name` - Unique tool name
    /// * `category` - Tool category for organization
    /// * `enabled` - Initial enablement state
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

    /// Creates a new configuration specifically for MCP tools
    /// 
    /// Used by: MCP integration when adding tools from external servers
    /// 
    /// # Arguments
    /// * `name` - Tool name (will be prefixed with server name)
    /// * `server_id` - ID of the MCP server providing this tool
    /// * `enabled` - Initial enablement state
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

    /// Adds a description to the tool configuration
    /// 
    /// Used by: Configuration builders for documentation purposes
    pub fn with_description(mut self, description: String) -> Self {
        self.description = Some(description);
        self
    }

    /// Marks the tool as required (cannot be disabled)
    /// 
    /// Used by: Core system tools that are essential for agent operation
    pub fn as_required(mut self) -> Self {
        self.required = true;
        self.enabled = true; // Required tools are always enabled
        self
    }
}

/// Manager for tool configurations
/// 
/// Central management system for all tool configurations, providing
/// enablement checking, category management, MCP integration, and persistence.
/// 
/// Used by: Main agent system for tool availability and settings management
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolConfigManager {
    pub tools: HashMap<String, ToolConfig>,
    pub category_enabled: HashMap<ToolCategory, bool>,
    pub mcp_servers: HashMap<String, MCPServerConfig>, // Store MCP server configurations
}

impl Default for ToolConfigManager {
    /// Creates a default tool configuration manager with all standard tools
    /// 
    /// Used by: Application initialization when no saved configuration exists
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
    /// 
    /// Used by: Application initialization and configuration reset
    pub fn new() -> Self {
        Self::default()
    }

    /// Load configuration from file or create default
    /// 
    /// Attempts to load existing configuration from the specified file,
    /// creates default configuration if file doesn't exist, and ensures
    /// backwards compatibility by adding any missing default tools.
    /// 
    /// Used by: Application startup for configuration initialization
    /// 
    /// # Arguments
    /// * `config_path` - Path to the configuration JSON file
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
    /// 
    /// Serializes the current configuration to JSON and saves it to the
    /// specified file, creating parent directories as needed.
    /// 
    /// Used by: Settings UI and application shutdown for persistence
    /// 
    /// # Arguments
    /// * `config_path` - Path where configuration should be saved
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
    /// 
    /// Determines the appropriate path for storing tool configuration
    /// within the application's data directory.
    /// 
    /// Used by: Application initialization for finding config file location
    /// 
    /// # Arguments
    /// * `app_handle` - Tauri app handle for path resolution
    pub fn get_config_path(app_handle: &AppHandle) -> Result<PathBuf, String> {
        let app_dir = app_handle.path().app_config_dir()
            .map_err(|e| format!("Failed to get app config directory: {}", e))?;
        Ok(app_dir.join("tool_config.json"))
    }

    /// Check if a tool is enabled
    /// 
    /// Determines if a tool should be available for use by checking both
    /// the individual tool setting and its category enablement state.
    /// Required tools are always considered enabled.
    /// 
    /// Used by: Agent tool execution system for availability decisions
    /// 
    /// # Arguments
    /// * `tool_name` - Name of the tool to check
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
    /// 
    /// Changes the enablement state of an individual tool, with protection
    /// against disabling required tools.
    /// 
    /// Used by: Settings UI for individual tool management
    /// 
    /// # Arguments
    /// * `tool_name` - Name of the tool to modify
    /// * `enabled` - New enablement state
    pub fn set_tool_enabled(&mut self, tool_name: &str, enabled: bool) {
        if let Some(tool_config) = self.tools.get_mut(tool_name) {
            if !(tool_config.required && !enabled) {
                tool_config.enabled = enabled;
            }
        }
    }

    /// Enable or disable an entire category of tools
    /// 
    /// Changes the enablement state for all tools in a category, with
    /// protection against disabling categories containing required tools.
    /// 
    /// Used by: Settings UI for category-level management
    /// 
    /// # Arguments
    /// * `category` - Category to modify
    /// * `enabled` - New enablement state
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
    /// 
    /// Returns all tool configurations belonging to the specified category.
    /// 
    /// Used by: Settings UI for category-specific display
    /// 
    /// # Arguments
    /// * `category` - Category to filter by
    pub fn get_tools_by_category(&self, category: &ToolCategory) -> Vec<&ToolConfig> {
        self.tools.values()
            .filter(|config| config.category == *category)
            .collect()
    }

    /// Get all enabled tools
    /// 
    /// Returns all tool configurations that are currently enabled
    /// (considering both individual and category settings).
    /// 
    /// Used by: Tool discovery and agent initialization
    pub fn get_enabled_tools(&self) -> Vec<&ToolConfig> {
        self.tools.iter()
            .filter(|(name, _)| self.is_tool_enabled(name))
            .map(|(_, config)| config)
            .collect()
    }

    /// Get tool configuration by name
    /// 
    /// Used by: Settings UI and configuration queries
    /// 
    /// # Arguments
    /// * `tool_name` - Name of the tool to retrieve
    pub fn get_tool_config(&self, tool_name: &str) -> Option<ToolConfig> {
        self.tools.get(tool_name).cloned()
    }

    /// Add or update a tool configuration
    /// 
    /// Used by: MCP integration and dynamic tool registration
    /// 
    /// # Arguments
    /// * `config` - Tool configuration to add or update
    pub fn add_tool_config(&mut self, config: ToolConfig) {
        self.tools.insert(config.name.clone(), config);
    }

    /// Get enabled tool names
    /// 
    /// Returns a list of names for all currently enabled tools.
    /// 
    /// Used by: Agent system for building available tool lists
    pub fn get_enabled_tool_names(&self) -> Vec<String> {
        self.tools.iter()
            .filter(|(name, _)| self.is_tool_enabled(name))
            .map(|(name, _)| name.clone())
            .collect()
    }

    /// Reset configuration to defaults
    /// 
    /// Resets all tool configurations to default values while preserving
    /// MCP server configurations.
    /// 
    /// Used by: Settings UI reset functionality
    pub fn reset_to_defaults(&mut self) {
        let mut new_config = Self::default();
        
        // Preserve MCP server configurations
        new_config.mcp_servers = self.mcp_servers.clone();
        
        *self = new_config;
    }

    // MCP Server Management Methods

    /// Add an MCP server configuration
    /// 
    /// Used by: MCP integration and settings UI for server management
    /// 
    /// # Arguments
    /// * `config` - MCP server configuration to add
    pub fn add_mcp_server(&mut self, config: MCPServerConfig) {
        self.mcp_servers.insert(config.id.clone(), config);
    }

    /// Remove an MCP server configuration
    /// 
    /// Removes server configuration and all associated tools.
    /// 
    /// Used by: Settings UI for server removal
    /// 
    /// # Arguments
    /// * `server_id` - ID of the server to remove
    pub fn remove_mcp_server(&mut self, server_id: &str) {
        self.mcp_servers.remove(server_id);
        
        // Also remove all tools from this server
        self.tools.retain(|_, tool_config| {
            tool_config.server_id.as_ref() != Some(&server_id.to_string())
        });
    }

    /// Get all MCP server configurations
    /// 
    /// Used by: Settings UI for server list display
    pub fn get_mcp_servers(&self) -> Vec<MCPServerConfig> {
        self.mcp_servers.values().cloned().collect()
    }

    /// Get MCP server configuration by ID
    /// 
    /// Used by: MCP integration for server management
    /// 
    /// # Arguments
    /// * `server_id` - ID of the server to retrieve
    pub fn get_mcp_server(&self, server_id: &str) -> Option<MCPServerConfig> {
        self.mcp_servers.get(server_id).cloned()
    }

    /// Update MCP server configuration
    /// 
    /// Used by: Settings UI for server configuration changes
    /// 
    /// # Arguments
    /// * `config` - Updated server configuration
    pub fn update_mcp_server(&mut self, config: MCPServerConfig) {
        self.mcp_servers.insert(config.id.clone(), config);
    }

    /// Add tools from an MCP server
    /// 
    /// Creates tool configurations for all tools discovered from an MCP server.
    /// 
    /// Used by: MCP integration when server tools are discovered
    /// 
    /// # Arguments
    /// * `server_id` - ID of the server providing the tools
    /// * `tools` - List of discovered tools from the server
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
    /// 
    /// Used by: Settings UI for server-specific tool display
    /// 
    /// # Arguments
    /// * `server_id` - ID of the server to filter by
    pub fn get_mcp_tools_for_server(&self, server_id: &str) -> Vec<&ToolConfig> {
        self.tools.values()
            .filter(|config| {
                config.category == ToolCategory::MCP && 
                config.server_id.as_ref() == Some(&server_id.to_string())
            })
            .collect()
    }

    /// Check if an MCP server is enabled
    /// 
    /// Used by: MCP integration for server management decisions
    /// 
    /// # Arguments
    /// * `server_id` - ID of the server to check
    pub fn is_mcp_server_enabled(&self, server_id: &str) -> bool {
        self.mcp_servers.get(server_id)
            .map(|config| config.enabled)
            .unwrap_or(false)
    }

    /// Enable or disable an MCP server
    /// 
    /// Used by: Settings UI for server enablement control
    /// 
    /// # Arguments
    /// * `server_id` - ID of the server to modify
    /// * `enabled` - New enablement state
    pub fn set_mcp_server_enabled(&mut self, server_id: &str, enabled: bool) {
        if let Some(server_config) = self.mcp_servers.get_mut(server_id) {
            server_config.enabled = enabled;
        }
    }

    // Default tool initialization methods

    /// Initializes default Anthropic Computer Use tools
    /// 
    /// Used by: Default configuration creation
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

    /// Initializes default desktop automation tools
    /// 
    /// Used by: Default configuration creation
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

    /// Initializes default browser automation tools
    /// 
    /// Used by: Default configuration creation
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

    /// Initializes default timer and scheduling tools
    /// 
    /// Used by: Default configuration creation
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

    /// Initializes default basic file and text tools
    /// 
    /// Used by: Default configuration creation
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
    /// 
    /// Adds any missing default tools to existing configurations to handle
    /// configuration file upgrades and new tool additions.
    /// 
    /// Used by: Configuration loading for backwards compatibility
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

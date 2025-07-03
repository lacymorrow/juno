//! Tool configuration management for all agent tools.
//! Handles categorization, enablement controls, persistence, and MCP server integration.
//! Used by: Agent initialization and settings management for tool control.

use std::collections::HashMap;
use serde::{Deserialize, Serialize};

use tracing::info;

// Import tool name constants
use crate::constants::agent::tool_names;

// Re-export MCP types for convenience
pub use super::mcp_integration::{MCPServerConfig, MCPServerStatus, MCPToolInfo};

// Add centralized settings support
use crate::settings::{ToolSettings, ToolConfig as SettingsToolConfig, MCPServerConfig as SettingsMCPServerConfig};

/// Tool category definitions for organizing tools by functionality.
/// Used by: Settings UI and tool management for logical grouping.
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
    /// Returns the human-readable display name for the category.
    /// Used by: Settings UI for category labels and display.
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

    /// Returns a description of what tools are in this category.
    /// Used by: Settings UI for tooltips and help text.
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

    /// Returns all available tool categories.
    /// Used by: Settings UI for iterating over all categories.
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

/// Configuration for an individual tool.
/// Contains settings for enablement state, category membership, and metadata.
/// Used by: Tool configuration manager and settings UI.
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
    /// Creates a new tool configuration with basic settings.
    /// Used by: Default tool initialization and configuration builders.
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

    /// Creates a new configuration specifically for MCP tools.
    /// Used by: MCP integration when adding tools from external servers.
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

    /// Adds a description to the tool configuration.
    /// Used by: Configuration builders for documentation purposes.
    pub fn with_description(mut self, description: String) -> Self {
        self.description = Some(description);
        self
    }

    /// Marks the tool as required (cannot be disabled).
    /// Used by: Core system tools that are essential for agent operation.
    pub fn as_required(mut self) -> Self {
        self.required = true;
        self.enabled = true; // Required tools are always enabled
        self
    }
}

/// Manager for tool configurations.
/// Central management system for tool configurations, enablement, and persistence.
/// Used by: Main agent system for tool availability and settings management.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolConfigManager {
    pub tools: HashMap<String, ToolConfig>,
    pub category_enabled: HashMap<ToolCategory, bool>,
    pub mcp_servers: HashMap<String, MCPServerConfig>, // Store MCP server configurations
}

impl Default for ToolConfigManager {
    /// Creates a default tool configuration manager with all standard tools.
    /// Used by: Application initialization when no saved configuration exists.
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
    /// Create a new tool configuration manager with defaults.
    /// Used by: Application initialization and configuration reset.
    pub fn new() -> Self {
        Self::default()
    }

    /// Load configuration from centralized settings manager.
    /// NEW: Uses centralized settings instead of direct JSON store access.
    /// Used by: Application startup for configuration initialization.
    pub async fn load_from_centralized_settings(settings_manager: &crate::settings::manager::SettingsManager) -> Result<Self, String> {
        let tool_settings = settings_manager.get_tool_settings().await?;
        let config_manager = Self::from_centralized_settings(&tool_settings)?;
        info!("Loaded tool configuration from centralized settings");
        Ok(config_manager)
    }

    /// Save configuration to centralized settings manager.
    /// NEW: Uses centralized settings instead of direct JSON store access.
    /// Used by: Settings UI and application shutdown for persistence.
    pub async fn save_to_centralized_settings(&self, settings_manager: &crate::settings::manager::SettingsManager) -> Result<(), String> {
        let tool_settings = self.to_centralized_settings()?;
        settings_manager.set_tool_settings(&tool_settings).await?;
        info!("Saved tool configuration to centralized settings");
        Ok(())
    }

    /// Convert from centralized ToolSettings to ToolConfigManager.
    /// Handles schema differences between the two formats.
    fn from_centralized_settings(settings: &ToolSettings) -> Result<Self, String> {
        let mut tools = HashMap::new();
        let mut category_enabled = HashMap::new();
        let mut mcp_servers = HashMap::new();

        // Convert tools
        for (tool_name, settings_tool_config) in &settings.tools {
            let tool_category = Self::parse_tool_category(&settings_tool_config.category)?;
            let tool_config = ToolConfig {
                name: settings_tool_config.name.clone(),
                category: tool_category,
                enabled: settings_tool_config.enabled,
                description: settings_tool_config.description.clone(),
                required: settings_tool_config.required,
                server_id: None, // Will be populated if it's an MCP tool
            };
            tools.insert(tool_name.clone(), tool_config);
        }

        // Convert category enabled states
        for (category_str, enabled) in &settings.category_enabled {
            let category = Self::parse_tool_category(category_str)?;
            category_enabled.insert(category, *enabled);
        }

        // Convert MCP servers
        for settings_server in &settings.mcp_servers {
            let server_config = MCPServerConfig {
                id: settings_server.id.clone(),
                name: settings_server.name.clone(),
                description: settings_server.description.clone(),
                command: settings_server.command.clone(),
                args: settings_server.args.clone(),
                working_directory: settings_server.working_directory.as_ref().map(|s| std::path::PathBuf::from(s)),
                environment_variables: settings_server.environment_variables.clone(),
                enabled: settings_server.enabled,
                auto_start: settings_server.auto_start,
                timeout_seconds: settings_server.timeout_seconds,
                max_retries: settings_server.max_retries,
            };
            mcp_servers.insert(settings_server.id.clone(), server_config);
        }

        // Ensure all default tools are present for backwards compatibility
        Self::ensure_default_tools(&mut tools);

        // Ensure all categories are represented
        for category in ToolCategory::all_categories() {
            if !category_enabled.contains_key(&category) {
                category_enabled.insert(category, true);
            }
        }

        Ok(Self {
            tools,
            category_enabled,
            mcp_servers,
        })
    }

    /// Convert from ToolConfigManager to centralized ToolSettings.
    /// Handles schema differences between the two formats.
    fn to_centralized_settings(&self) -> Result<ToolSettings, String> {
        let mut tools = HashMap::new();
        let mut category_enabled = HashMap::new();
        let mut mcp_servers = Vec::new();

        // Convert tools
        for (tool_name, tool_config) in &self.tools {
            let settings_tool_config = SettingsToolConfig {
                name: tool_config.name.clone(),
                category: Self::format_tool_category(&tool_config.category),
                enabled: tool_config.enabled,
                description: tool_config.description.clone(),
                required: tool_config.required,
            };
            tools.insert(tool_name.clone(), settings_tool_config);
        }

        // Convert category enabled states
        for (category, enabled) in &self.category_enabled {
            category_enabled.insert(Self::format_tool_category(category), *enabled);
        }

        // Convert MCP servers
        for (_, server_config) in &self.mcp_servers {
            let settings_server = SettingsMCPServerConfig {
                id: server_config.id.clone(),
                name: server_config.name.clone(),
                description: server_config.description.clone(),
                command: server_config.command.clone(),
                args: server_config.args.clone(),
                working_directory: server_config.working_directory.as_ref().map(|p| p.to_string_lossy().to_string()),
                environment_variables: server_config.environment_variables.clone(),
                enabled: server_config.enabled,
                auto_start: server_config.auto_start,
                timeout_seconds: server_config.timeout_seconds,
                max_retries: server_config.max_retries,
            };
            mcp_servers.push(settings_server);
        }

        Ok(ToolSettings {
            tools,
            category_enabled,
            mcp_servers,
        })
    }

    /// Parse tool category string into ToolCategory enum.
    pub fn parse_tool_category(category_str: &str) -> Result<ToolCategory, String> {
        match category_str {
            "AnthropicComputerUse" => Ok(ToolCategory::AnthropicComputerUse),
            "Desktop" => Ok(ToolCategory::Desktop),
            "Browser" => Ok(ToolCategory::Browser),
            "Timer" => Ok(ToolCategory::Timer),
            "Basic" => Ok(ToolCategory::Basic),
            "MCP" => Ok(ToolCategory::MCP),
            _ => Err(format!("Unknown tool category: {}", category_str)),
        }
    }

    /// Format tool category enum into string.
    pub fn format_tool_category(category: &ToolCategory) -> String {
        match category {
            ToolCategory::AnthropicComputerUse => "AnthropicComputerUse".to_string(),
            ToolCategory::Desktop => "Desktop".to_string(),
            ToolCategory::Browser => "Browser".to_string(),
            ToolCategory::Timer => "Timer".to_string(),
            ToolCategory::Basic => "Basic".to_string(),
            ToolCategory::MCP => "MCP".to_string(),
        }
    }

    /// Check if a tool is enabled.
    /// Checks both individual tool setting and category enablement state.
    /// Used by: Agent tool execution system for availability decisions.
    ///
    /// # Arguments
    /// * `tool_name` - Name of the tool to check
    pub fn is_tool_enabled(&self, tool_name: &str) -> bool {
        if let Some(tool_config) = self.tools.get(tool_name) {
            if tool_config.required {
                tracing::debug!("Tool '{}' is required and always enabled", tool_name);
                return true; // Required tools are always enabled
            }

            // Check both tool-specific and category-wide settings
            let category_enabled = self.category_enabled.get(&tool_config.category).unwrap_or(&true);
            let result = tool_config.enabled && *category_enabled;
            tracing::debug!("Tool '{}' enabled check: tool_enabled={}, category_enabled={}, result={}",
                tool_name, tool_config.enabled, category_enabled, result);
            result
        } else {
            // Unknown tools are disabled by default
            // Note: Essential tools should be properly configured as required during initialization
            tracing::debug!("Unknown tool '{}' disabled by default", tool_name);
            false
        }
    }

    /// Enable or disable a specific tool.
    /// Changes enablement state with protection against disabling required tools.
    /// Used by: Settings UI for individual tool management.
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

    /// Enable or disable an entire category of tools.
    /// Changes enablement state for all tools in a category.
    /// Used by: Settings UI for category-level management.
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

    /// Get all tools in a category.
    /// Returns all tool configurations belonging to the specified category.
    /// Used by: Settings UI for category-specific display.
    ///
    /// # Arguments
    /// * `category` - Category to filter by
    pub fn get_tools_by_category(&self, category: &ToolCategory) -> Vec<&ToolConfig> {
        self.tools.values()
            .filter(|config| config.category == *category)
            .collect()
    }

    /// Get all enabled tools.
    /// Returns all tool configurations that are currently enabled.
    /// Used by: Tool discovery and agent initialization.
    pub fn get_enabled_tools(&self) -> Vec<&ToolConfig> {
        self.tools.iter()
            .filter(|(name, _)| self.is_tool_enabled(name))
            .map(|(_, config)| config)
            .collect()
    }

    /// Get tool configuration by name.
    /// Used by: Settings UI and configuration queries.
    ///
    /// # Arguments
    /// * `tool_name` - Name of the tool to retrieve
    pub fn get_tool_config(&self, tool_name: &str) -> Option<ToolConfig> {
        self.tools.get(tool_name).cloned()
    }

    /// Add or update a tool configuration.
    /// Used by: MCP integration and dynamic tool registration.
    ///
    /// # Arguments
    /// * `config` - Tool configuration to add or update
    pub fn add_tool_config(&mut self, config: ToolConfig) {
        self.tools.insert(config.name.clone(), config);
    }

    /// Get enabled tool names.
    /// Returns a list of names for all currently enabled tools.
    /// Used by: Agent system for building available tool lists.
    pub fn get_enabled_tool_names(&self) -> Vec<String> {
        self.tools.iter()
            .filter(|(name, _)| self.is_tool_enabled(name))
            .map(|(name, _)| name.clone())
            .collect()
    }

    /// Reset configuration to defaults.
    /// Resets all tool configurations to default values while preserving MCP servers.
    /// Used by: Settings UI reset functionality.
    pub fn reset_to_defaults(&mut self) {
        let mut new_config = Self::default();

        // Preserve MCP server configurations
        new_config.mcp_servers = self.mcp_servers.clone();

        *self = new_config;
    }

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
            (tool_names::COMPUTER, "Use mouse and keyboard to interact with computer, and take screenshots"),
            (tool_names::ACCESSIBILITY_INTERFACE, "Use macOS accessibility APIs for precise UI interaction (preferred method)"),
            (tool_names::STR_REPLACE_BASED_EDIT_TOOL, "Create, view, and edit files with precise text operations"),
            (tool_names::BASH, "Execute bash commands and shell operations"),
        ];

        // Essential tools that are required for core functionality
        let essential_tools = [tool_names::COMPUTER, tool_names::BASH];

        for (name, description) in anthropic_tools {
            let mut config = ToolConfig::new(
                name.to_string(),
                ToolCategory::AnthropicComputerUse,
                true,
            )
            .with_description(description.to_string());

            // Only mark essential tools as required
            if essential_tools.contains(&name) {
                config = config.as_required();
            }

            tools.insert(name.to_string(), config);
        }
    }

    /// Initializes default desktop automation tools
    ///
    /// Used by: Default configuration creation
    fn add_default_desktop_tools(tools: &mut HashMap<String, ToolConfig>) {
        let desktop_tools = vec![
            (tool_names::LAUNCH_APPLICATION, "Launch applications by name"),
            (tool_names::GET_RUNNING_APPLICATIONS, "List currently running applications"),
            (tool_names::FOCUS_APPLICATION, "Bring application to front"),
            (tool_names::QUIT_APPLICATION, "Quit an application"),
            (tool_names::GET_SYSTEM_INFO, "Get system information"),
            (tool_names::MANAGE_AUDIO, "Control system audio settings"),
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
            (tool_names::BROWSER_NAVIGATE, "Navigate to a URL"),
            (tool_names::BROWSER_CLICK, "Click elements in the browser"),
            (tool_names::BROWSER_TYPE, "Type text in browser forms"),
            (tool_names::BROWSER_SCROLL, "Scroll browser pages"),
            (tool_names::BROWSER_SCREENSHOT, "Take browser screenshots"),
            (tool_names::BROWSER_GET_CONTENT, "Extract page content"),
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
            (tool_names::SET_TIMER, "Create a scheduled timer"),
            (tool_names::LIST_TIMERS, "List active timers"),
            (tool_names::CANCEL_TIMER, "Cancel a timer"),
            (tool_names::TIMER_STATUS, "Check timer status"),
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
            // NOTE: READ_FILE tool removed - use str_replace_based_edit_tool instead
            // NOTE: WRITE_FILE tool removed - use str_replace_based_edit_tool instead
            // NOTE: EXECUTE_SHELL_COMMAND tool removed - use bash tool instead
            (tool_names::LIST_DIRECTORY, "List directory contents"),
            (tool_names::CREATE_DIRECTORY, "Create directories"),
            (tool_names::DELETE_FILE, "Delete files"),
            (tool_names::TEXT_EDITOR_EDIT, "Edit text files"),
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

    // MCP Server Management Methods

    /// Add an MCP server configuration.
    /// Used by: MCP integration and settings UI for server management.
    ///
    /// # Arguments
    /// * `config` - MCP server configuration to add
    pub fn add_mcp_server(&mut self, config: MCPServerConfig) {
        self.mcp_servers.insert(config.id.clone(), config);
    }

    /// Remove an MCP server configuration.
    /// Removes server configuration and all associated tools.
    /// Used by: Settings UI for server removal.
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

    /// Get all MCP server configurations.
    /// Used by: Settings UI for server list display.
    pub fn get_mcp_servers(&self) -> Vec<MCPServerConfig> {
        self.mcp_servers.values().cloned().collect()
    }

    /// Get MCP server configuration by ID.
    /// Used by: MCP integration for server management.
    ///
    /// # Arguments
    /// * `server_id` - ID of the server to retrieve
    pub fn get_mcp_server(&self, server_id: &str) -> Option<MCPServerConfig> {
        self.mcp_servers.get(server_id).cloned()
    }

    /// Update MCP server configuration.
    /// Used by: Settings UI for server configuration changes.
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
}

/// Load tool configuration from centralized settings
/// NEW: Uses centralized settings instead of direct JSON store access.
/// Used by: Application startup for configuration initialization
///
/// # Arguments
/// * `settings_manager` - Centralized settings manager
/// * `state` - Application state containing tool config manager
pub async fn load_tool_config_from_centralized_settings(
    settings_manager: &crate::settings::manager::SettingsManager,
    state: &crate::state::AppState
) -> Result<(), String> {
    let loaded_config = ToolConfigManager::load_from_centralized_settings(settings_manager).await?;

    let mut config_guard = state.tool_config_manager.lock().await;
    *config_guard = loaded_config;

    info!("Loaded tool configuration from centralized settings on startup");
    Ok(())
}

/// Save tool configuration to centralized settings
/// NEW: Uses centralized settings instead of direct JSON store access.
/// Used by: Application shutdown and settings changes for persistence
///
/// # Arguments
/// * `settings_manager` - Centralized settings manager
/// * `state` - Application state containing tool config manager
pub async fn save_tool_config_to_centralized_settings(
    settings_manager: &crate::settings::manager::SettingsManager,
    state: &crate::state::AppState
) -> Result<(), String> {
    let config_guard = state.tool_config_manager.lock().await;
    config_guard.save_to_centralized_settings(settings_manager).await?;

    info!("Saved tool configuration to centralized settings");
    Ok(())
}

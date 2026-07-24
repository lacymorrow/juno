//! Tool Versioning System
//!
//! Provides version-based tool selection and API compatibility management
//! for Anthropic Computer Use tools. This ensures proper tool versioning
//! and API compatibility as required by the official specification.

use crate::agent::core::ToolDefinition;
use crate::constants::api::{beta_flags, computer_use_api_types, tool_version_groups};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Supported API versions for tool groups
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ApiVersion {
    /// Computer Use API 2024-10-22
    Computer20241022,
    /// Computer Use API 2025-01-24
    Computer20250124,
    /// Computer Use API 2025-11-24 (Opus 4.5+)
    Computer20251124,
}

impl ApiVersion {
    /// Get the API version string
    pub fn as_str(&self) -> &'static str {
        match self {
            ApiVersion::Computer20241022 => computer_use_api_types::COMPUTER_20241022,
            ApiVersion::Computer20250124 => computer_use_api_types::COMPUTER_20250124,
            ApiVersion::Computer20251124 => computer_use_api_types::COMPUTER_20251124,
        }
    }

    /// Get the beta flag for this version
    pub fn beta_flag(&self) -> &'static str {
        match self {
            ApiVersion::Computer20241022 => beta_flags::COMPUTER_USE_2024_10_22,
            ApiVersion::Computer20250124 => beta_flags::COMPUTER_USE_2025_01_24,
            ApiVersion::Computer20251124 => beta_flags::COMPUTER_USE_2025_11_24,
        }
    }

    /// Get all tools available for this API version
    pub fn available_tools(&self) -> &'static [&'static str] {
        match self {
            ApiVersion::Computer20241022 => tool_version_groups::COMPUTER_USE_2024_10_22_TOOLS,
            ApiVersion::Computer20250124 => tool_version_groups::COMPUTER_USE_2025_01_24_TOOLS,
            ApiVersion::Computer20251124 => tool_version_groups::COMPUTER_USE_2025_11_24_TOOLS,
        }
    }
}

/// Tool version configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolVersionConfig {
    /// Current API version to use
    pub current_version: ApiVersion,
    /// Whether to enable beta features
    pub enable_beta: bool,
    /// Override specific tool versions (tool_name -> api_type)
    pub tool_overrides: HashMap<String, String>,
}

impl Default for ToolVersionConfig {
    fn default() -> Self {
        Self {
            current_version: ApiVersion::Computer20250124, // Use latest by default
            enable_beta: true,
            tool_overrides: HashMap::new(),
        }
    }
}

impl ToolVersionConfig {
    /// Create a new configuration with the specified version
    pub fn new(version: ApiVersion) -> Self {
        Self {
            current_version: version,
            enable_beta: true,
            tool_overrides: HashMap::new(),
        }
    }

    /// Enable or disable beta features
    pub fn with_beta(mut self, enable: bool) -> Self {
        self.enable_beta = enable;
        self
    }

    /// Add a tool version override
    pub fn with_tool_override(mut self, tool_name: String, api_type: String) -> Self {
        self.tool_overrides.insert(tool_name, api_type);
        self
    }

    /// Get the API type for a specific tool
    pub fn get_tool_api_type(&self, tool_name: &str) -> Option<String> {
        // Check for specific override first
        if let Some(override_type) = self.tool_overrides.get(tool_name) {
            return Some(override_type.clone());
        }

        // Use version-specific defaults
        match (&self.current_version, tool_name) {
            (ApiVersion::Computer20241022, "computer") => {
                Some(computer_use_api_types::COMPUTER_20241022.to_string())
            }
            (ApiVersion::Computer20250124, "computer") => {
                Some(computer_use_api_types::COMPUTER_20250124.to_string())
            }
            (ApiVersion::Computer20251124, "computer") => {
                Some(computer_use_api_types::COMPUTER_20251124.to_string())
            }
            (_, "bash") => Some(computer_use_api_types::BASH_20250124.to_string()),
            (ApiVersion::Computer20241022, "str_replace_based_edit_tool") => {
                Some(computer_use_api_types::EDIT_TOOL_20250124.to_string())
            }
            (ApiVersion::Computer20250124, "str_replace_based_edit_tool") => {
                Some(computer_use_api_types::EDIT_TOOL_20250429.to_string())
            }
            (ApiVersion::Computer20251124, "str_replace_based_edit_tool") => {
                Some(computer_use_api_types::EDIT_TOOL_20250728.to_string())
            }
            _ => None,
        }
    }

    /// Get the beta flag for the current version (if beta is enabled)
    pub fn get_beta_flag(&self) -> Option<String> {
        if self.enable_beta {
            Some(self.current_version.beta_flag().to_string())
        } else {
            None
        }
    }

    /// Check if a tool is supported in the current version
    pub fn is_tool_supported(&self, tool_name: &str) -> bool {
        self.get_tool_api_type(tool_name).is_some()
    }

    /// Filter tools based on current version compatibility
    pub fn filter_compatible_tools(&self, tools: Vec<ToolDefinition>) -> Vec<ToolDefinition> {
        tools
            .into_iter()
            .filter(|tool| self.is_tool_supported(&tool.name))
            .map(|mut tool| {
                // Update API type and beta flag based on current configuration
                if let Some(api_type) = self.get_tool_api_type(&tool.name) {
                    tool.api_type = Some(api_type);
                }
                if let Some(beta_flag) = self.get_beta_flag() {
                    tool.beta_flag = Some(beta_flag);
                }
                tool
            })
            .collect()
    }

    /// Get HTTP headers for API requests with proper beta flags
    pub fn get_api_headers(&self) -> HashMap<String, String> {
        let mut headers = HashMap::new();

        if self.enable_beta {
            headers.insert(
                "anthropic-beta".to_string(),
                self.current_version.beta_flag().to_string(),
            );
        }

        headers
    }
}

/// Global tool version manager
pub struct ToolVersionManager {
    config: ToolVersionConfig,
}

impl ToolVersionManager {
    /// Create a new version manager with default configuration
    pub fn new() -> Self {
        Self {
            config: ToolVersionConfig::default(),
        }
    }

    /// Create a version manager with specific configuration
    pub fn with_config(config: ToolVersionConfig) -> Self {
        Self { config }
    }

    /// Update the configuration
    pub fn update_config(&mut self, config: ToolVersionConfig) {
        self.config = config;
    }

    /// Get the current configuration
    pub fn config(&self) -> &ToolVersionConfig {
        &self.config
    }

    /// Apply versioning to a tool definition
    pub fn apply_versioning(&self, mut tool: ToolDefinition) -> ToolDefinition {
        if let Some(api_type) = self.config.get_tool_api_type(&tool.name) {
            tool.api_type = Some(api_type);
        }
        if let Some(beta_flag) = self.config.get_beta_flag() {
            tool.beta_flag = Some(beta_flag);
        }
        tool
    }

    /// Validate tool compatibility with current version
    pub fn validate_tool_compatibility(&self, tool: &ToolDefinition) -> Result<(), String> {
        if !self.config.is_tool_supported(&tool.name) {
            return Err(format!(
                "Tool '{}' is not supported in API version {:?}",
                tool.name, self.config.current_version
            ));
        }

        // Validate API type if present
        if let Some(api_type) = &tool.api_type {
            if let Some(expected_type) = self.config.get_tool_api_type(&tool.name) {
                if api_type != &expected_type {
                    return Err(format!(
                        "Tool '{}' has API type '{}', but expected '{}' for version {:?}",
                        tool.name, api_type, expected_type, self.config.current_version
                    ));
                }
            }
        }

        Ok(())
    }
}

impl Default for ToolVersionManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_api_version_strings() {
        assert_eq!(ApiVersion::Computer20241022.as_str(), "computer_20241022");
        assert_eq!(ApiVersion::Computer20250124.as_str(), "computer_20250124");
    }

    #[test]
    fn test_tool_version_config() {
        let config = ToolVersionConfig::new(ApiVersion::Computer20250124);

        assert_eq!(
            config.get_tool_api_type("computer"),
            Some("computer_20250124".to_string())
        );
        assert_eq!(
            config.get_tool_api_type("bash"),
            Some("bash_20250124".to_string())
        );
    }

    #[test]
    fn test_tool_overrides() {
        let config = ToolVersionConfig::new(ApiVersion::Computer20250124)
            .with_tool_override("computer".to_string(), "custom_computer_api".to_string());

        assert_eq!(
            config.get_tool_api_type("computer"),
            Some("custom_computer_api".to_string())
        );
    }

    #[test]
    fn test_version_manager() {
        let manager = ToolVersionManager::new();

        let tool = ToolDefinition {
            name: "computer".to_string(),
            description: "Test tool".to_string(),
            input_schema: serde_json::json!({}),
            api_type: None,
            beta_flag: None,
        };

        let versioned_tool = manager.apply_versioning(tool);
        assert_eq!(
            versioned_tool.api_type,
            Some("computer_20250124".to_string())
        );
        assert_eq!(
            versioned_tool.beta_flag,
            Some("computer-use-2025-01-24".to_string())
        );
    }
}

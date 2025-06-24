use super::types::{PromptConfig, PromptContext, PromptTemplate, PromptType};
use super::templates::DefaultPrompts;
use crate::agent::core::AgentError;
use std::collections::HashMap;

use tracing::{info, warn};

// Add centralized settings support
use crate::settings::{PromptSettings as CentralizedPromptSettings, PromptTemplate as CentralizedPromptTemplate};

/// Manages prompt templates, configuration, and generation
pub struct PromptManager {
    config: PromptConfig,
    templates: HashMap<PromptType, PromptTemplate>,
}

impl PromptManager {
    /// Create a new PromptManager with default templates
    pub fn new() -> Self {
        Self {
            config: PromptConfig::default(),
            templates: DefaultPrompts::get_all(),
        }
    }

    /// Load configuration from centralized settings manager.
    /// NEW: Uses centralized settings instead of direct JSON store access.
    /// Used by: Agent initialization and prompt management.
    pub async fn load_from_centralized_settings(settings_manager: &crate::settings::manager::SettingsManager) -> Result<Self, AgentError> {
        let prompt_settings = settings_manager.get_prompt_settings().await
            .map_err(|e| AgentError::ConfigurationError(format!("Failed to load prompt settings: {}", e)))?;

        let config = Self::from_centralized_settings(&prompt_settings)?;
        let mut manager = Self {
            config,
            templates: DefaultPrompts::get_all(),
        };

        // Merge custom prompts from config
        for (id, template) in &manager.config.custom_prompts {
            if let Some(prompt_type) = PromptType::from_str(id) {
                manager.templates.insert(prompt_type, template.clone());
            }
        }

        info!("Loaded prompt configuration from centralized settings");
        Ok(manager)
    }

    /// Save configuration to centralized settings manager.
    /// NEW: Uses centralized settings instead of direct JSON store access.
    /// Used by: Settings UI and prompt configuration updates.
    pub async fn save_to_centralized_settings(&self, settings_manager: &crate::settings::manager::SettingsManager) -> Result<(), AgentError> {
        let prompt_settings = self.to_centralized_settings()?;
        settings_manager.set_prompt_settings(&prompt_settings).await
            .map_err(|e| AgentError::ConfigurationError(format!("Failed to save prompt settings: {}", e)))?;
        info!("Saved prompt configuration to centralized settings");
        Ok(())
    }

    /// Convert from centralized PromptSettings to PromptConfig.
    /// Handles schema differences between the two formats.
    fn from_centralized_settings(settings: &CentralizedPromptSettings) -> Result<PromptConfig, AgentError> {
        let mut custom_prompts = HashMap::new();

        // Convert centralized prompt templates to internal format
        for (id, centralized_template) in &settings.custom_prompts {
            let template = PromptTemplate {
                id: centralized_template.id.clone(),
                name: centralized_template.name.clone(),
                description: centralized_template.description.clone(),
                content: centralized_template.content.clone(),
                variables: centralized_template.variables.clone(),
                tags: centralized_template.tags.clone(),
                version: centralized_template.version.clone(),
                customizable: centralized_template.customizable,
            };
            custom_prompts.insert(id.clone(), template);
        }

        Ok(PromptConfig {
            active_prompts: settings.active_prompts.clone(),
            custom_prompts,
            global_variables: settings.global_variables.clone(),
            allow_customization: settings.allow_customization,
        })
    }

    /// Convert from PromptConfig to centralized PromptSettings.
    /// Handles schema differences between the two formats.
    fn to_centralized_settings(&self) -> Result<CentralizedPromptSettings, AgentError> {
        let mut custom_prompts = HashMap::new();

        // Convert internal prompt templates to centralized format
        for (id, template) in &self.config.custom_prompts {
            let centralized_template = CentralizedPromptTemplate {
                id: template.id.clone(),
                name: template.name.clone(),
                description: template.description.clone(),
                content: template.content.clone(),
                variables: template.variables.clone(),
                tags: template.tags.clone(),
                version: template.version.clone(),
                customizable: template.customizable,
            };
            custom_prompts.insert(id.clone(), centralized_template);
        }

        Ok(CentralizedPromptSettings {
            active_prompts: self.config.active_prompts.clone(),
            custom_prompts,
            global_variables: self.config.global_variables.clone(),
            allow_customization: self.config.allow_customization,
        })
    }



    /// Get a prompt for a specific type with context substitution
    pub fn get_prompt(&self, prompt_type: PromptType, context: Option<PromptContext>) -> Result<String, AgentError> {
        let template = self.templates.get(&prompt_type)
            .ok_or_else(|| AgentError::ConfigurationError(format!("Prompt template not found: {:?}", prompt_type)))?;

        let mut content = template.content.clone();

        // Apply variable substitution if context is provided
        if let Some(ctx) = context {
            content = self.substitute_variables(&content, &template.variables, &ctx)?;
        }

        Ok(content)
    }

    /// Get the default system prompt (backwards compatibility)
    pub fn get_default_system_prompt(&self) -> String {
        // In development mode, use the self-aware development prompt
        if cfg!(debug_assertions) {
            // Try to get the development prompt first
            if let Ok(dev_prompt) = self.get_prompt(PromptType::SystemDefaultDevelopment, None) {
                return dev_prompt;
            }
            // If development prompt is not available, log and fall back to default
            warn!("Development prompt not available in debug mode, falling back to default");
        }

        // Production mode or fallback: use the standard prompt
        self.get_prompt(PromptType::SystemDefault, None)
            .unwrap_or_else(|_| DefaultPrompts::system_default().content)
    }

    /// Get orchestrator personality prompt (backwards compatibility)
    pub fn get_orchestrator_personality_prompt(&self) -> String {
        self.get_prompt(PromptType::OrchestratorPersonality, None)
            .unwrap_or_else(|_| DefaultPrompts::orchestrator_personality().content)
    }

    /// Get specialist prompt for delegation system (now uses expert prompts)
    pub fn get_specialist_prompt(&self, agent_type: &str) -> String {
        let prompt_type = match agent_type {
            "browser" => PromptType::BrowserExpert,
            "desktop" => PromptType::DesktopExpert,
            "file" => PromptType::FileExpert,
            _ => {
                warn!("Unknown specialist agent type: {}. Using file expert.", agent_type);
                PromptType::FileExpert
            }
        };

        self.get_prompt(prompt_type, None)
            .unwrap_or_else(|_| {
                match agent_type {
                    "browser" => DefaultPrompts::browser_expert().content,
                    "desktop" => DefaultPrompts::desktop_expert().content,
                    _ => DefaultPrompts::file_expert().content,
                }
            })
    }

    /// Get expert agent prompt for multi-agent system
    pub fn get_expert_prompt(&self, agent_type: &str) -> String {
        let prompt_type = match agent_type {
            "orchestrator" => PromptType::OrchestratorPersonality,
            "browser_expert" => PromptType::BrowserExpert,
            "coding_expert" => PromptType::CodingExpert,
            "desktop_expert" => PromptType::DesktopExpert,
            "general_expert" => PromptType::GeneralExpert,
            "file_expert" => PromptType::FileExpert,
            _ => {
                warn!("Unknown expert agent type: {}. Using general expert.", agent_type);
                PromptType::GeneralExpert
            }
        };

        self.get_prompt(prompt_type, None)
            .unwrap_or_else(|_| DefaultPrompts::general_expert().content)
    }

    /// Update a prompt template using centralized settings.
    /// NEW: Uses centralized settings instead of direct JSON store access.
    /// Used by: Settings UI and prompt customization.
    pub async fn update_prompt_with_centralized_settings(
        &mut self,
        prompt_type: PromptType,
        content: String,
        settings_manager: &crate::settings::manager::SettingsManager
    ) -> Result<(), AgentError> {
        if let Some(template) = self.templates.get_mut(&prompt_type) {
            if template.customizable {
                template.content = content.clone();

                // Also update in custom prompts config
                self.config.custom_prompts.insert(
                    prompt_type.as_str().to_string(),
                    template.clone()
                );

                self.save_to_centralized_settings(settings_manager).await?;
                info!("Updated prompt template: {:?}", prompt_type);
            } else {
                return Err(AgentError::ConfigurationError(
                    format!("Prompt type {:?} is not customizable", prompt_type)
                ));
            }
        } else {
            return Err(AgentError::ConfigurationError(
                format!("Prompt template not found: {:?}", prompt_type)
            ));
        }

        Ok(())
    }



    /// Get all available prompt templates
    pub fn get_templates(&self) -> &HashMap<PromptType, PromptTemplate> {
        &self.templates
    }

    /// Get customizable prompt templates only
    pub fn get_customizable_templates(&self) -> HashMap<PromptType, PromptTemplate> {
        self.templates
            .iter()
            .filter(|(_, template)| template.customizable)
            .map(|(k, v)| (k.clone(), v.clone()))
            .collect()
    }

    /// Reset a prompt to its default value using centralized settings.
    /// NEW: Uses centralized settings instead of direct JSON store access.
    /// Used by: Settings UI and prompt reset functionality.
    pub async fn reset_prompt_with_centralized_settings(
        &mut self,
        prompt_type: PromptType,
        settings_manager: &crate::settings::manager::SettingsManager
    ) -> Result<(), AgentError> {
        if let Some(default_template) = DefaultPrompts::get_all().get(&prompt_type) {
            self.templates.insert(prompt_type.clone(), default_template.clone());
            self.config.custom_prompts.remove(prompt_type.as_str());
            self.save_to_centralized_settings(settings_manager).await?;
            info!("Reset prompt template to default: {:?}", prompt_type);
            Ok(())
        } else {
            Err(AgentError::ConfigurationError(
                format!("Default template not found for: {:?}", prompt_type)
            ))
        }
    }



    /// Set global variables using centralized settings.
    /// NEW: Uses centralized settings instead of direct JSON store access.
    /// Used by: Settings UI and global variable management.
    pub async fn set_global_variables_with_centralized_settings(
        &mut self,
        variables: HashMap<String, String>,
        settings_manager: &crate::settings::manager::SettingsManager
    ) -> Result<(), AgentError> {
        self.config.global_variables = variables;
        self.save_to_centralized_settings(settings_manager).await?;
        Ok(())
    }



    /// Get global variables
    pub fn get_global_variables(&self) -> &HashMap<String, String> {
        &self.config.global_variables
    }

    /// Substitute variables in prompt content
    fn substitute_variables(
        &self,
        content: &str,
        _template_variables: &[String],
        context: &PromptContext,
    ) -> Result<String, AgentError> {
        let mut result = content.to_string();

        // Apply global variables first
        for (key, value) in &self.config.global_variables {
            let placeholder = format!("{{{{{}}}}}", key);
            result = result.replace(&placeholder, value);
        }

        // Apply context variables
        for (key, value) in &context.custom_variables {
            let placeholder = format!("{{{{{}}}}}", key);
            result = result.replace(&placeholder, value);
        }

        // Apply specific context fields
        if let Some(ref prefs) = context.user_preferences {
            for (key, value) in prefs {
                let placeholder = format!("{{{{{}}}}}", key);
                result = result.replace(&placeholder, value);
            }
        }

        if let Some(ref task) = context.task_context {
            result = result.replace("{{task_context}}", task);
        }

        // Apply available tools
        if !context.available_tools.is_empty() {
            let tools_list = context.available_tools.join(", ");
            result = result.replace("{{available_tools}}", &tools_list);
        }

        // Apply provider constraints
        if let Some(ref constraints) = context.provider_constraints {
            for (key, value) in constraints {
                let placeholder = format!("{{{{{}}}}}", key);
                result = result.replace(&placeholder, value);
            }
        }

        Ok(result)
    }


}

impl Default for PromptManager {
    fn default() -> Self {
        Self::new()
    }
}

/// Load prompt configuration from centralized settings
/// Used by: Application startup and prompt configuration initialization
///
/// # Arguments
/// * `settings_manager` - Centralized settings manager
///
/// # Returns
/// `Result<PromptConfig, String>` - Loaded configuration or error message
pub async fn load_prompt_config_from_centralized_settings(settings_manager: &crate::settings::manager::SettingsManager) -> Result<PromptConfig, String> {
    let prompt_settings = settings_manager.get_prompt_settings().await?;
                crate::agent::prompts::PromptManager::from_centralized_settings(&prompt_settings)
        .map_err(|e| format!("Failed to convert centralized prompt settings: {}", e))
}

/// Save prompt configuration to centralized settings
/// Used by: Settings UI and prompt configuration updates
///
/// # Arguments
/// * `config` - Prompt configuration to save
/// * `settings_manager` - Centralized settings manager
///
/// # Returns
/// `Result<(), String>` - Success or error message
pub async fn save_prompt_config_to_centralized_settings(config: &PromptConfig, settings_manager: &crate::settings::manager::SettingsManager) -> Result<(), String> {
    let mut custom_prompts = std::collections::HashMap::new();

    // Convert internal prompt templates to centralized format
    for (id, template) in &config.custom_prompts {
        let centralized_template = crate::settings::PromptTemplate {
            id: template.id.clone(),
            name: template.name.clone(),
            description: template.description.clone(),
            content: template.content.clone(),
            variables: template.variables.clone(),
            tags: template.tags.clone(),
            version: template.version.clone(),
            customizable: template.customizable,
        };
        custom_prompts.insert(id.clone(), centralized_template);
    }

    let prompt_settings = crate::settings::PromptSettings {
        active_prompts: config.active_prompts.clone(),
        custom_prompts,
        global_variables: config.global_variables.clone(),
        allow_customization: config.allow_customization,
    };

    settings_manager.set_prompt_settings(&prompt_settings).await
}

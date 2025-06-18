use super::types::{PromptConfig, PromptContext, PromptTemplate, PromptType};
use super::templates::DefaultPrompts;
use crate::agent::structs::AgentError;
use serde_json;
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use tauri::AppHandle;
use tauri_plugin_store::StoreExt;
use tracing::{info, warn};

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

    /// Load configuration from Tauri store or create default.
    /// Attempts to load existing configuration, creates default if missing.
    /// Used by: Agent initialization and prompt management.
    pub fn load_from_store(app_handle: &AppHandle) -> Result<Self, AgentError> {
        let store = app_handle.store("prompt_config.json").map_err(|e| {
            AgentError::ConfigurationError(format!("Failed to access prompt config store: {}", e))
        })?;

        // Try to load the configuration from store
        let config = if let Some(config_value) = store.get("prompt_config") {
            match serde_json::from_value::<PromptConfig>(config_value) {
                Ok(config) => {
                    info!("Loaded prompt configuration from store");
                    config
                }
                Err(e) => {
                    warn!("Failed to parse stored prompt config ({}), creating default", e);
                    PromptConfig::default()
                }
            }
        } else {
            info!("No prompt configuration found in store, creating default");
            PromptConfig::default()
        };

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

        manager.save_config_to_store(app_handle)?;
        Ok(manager)
    }

    /// Load configuration from file system
    /// DEPRECATED: Use load_from_store() instead. Kept for backwards compatibility during migration.
    pub fn load() -> Result<Self, AgentError> {
        // For backwards compatibility, return a default configuration
        // This method should no longer be used in production code
        warn!("DEPRECATED: PromptManager::load() called. Use load_from_store() instead.");
        Ok(Self::new())
    }

    /// Save configuration to Tauri store.
    /// Serializes current configuration to JSON and saves to store.
    /// Used by: Settings UI and prompt configuration updates.
    pub fn save_config_to_store(&self, app_handle: &AppHandle) -> Result<(), AgentError> {
        let store = app_handle.store("prompt_config.json").map_err(|e| {
            AgentError::ConfigurationError(format!("Failed to access prompt config store: {}", e))
        })?;

        let config_value = serde_json::to_value(&self.config).map_err(|e| {
            AgentError::ConfigurationError(format!("Failed to serialize prompt config: {}", e))
        })?;

        store.set("prompt_config", config_value);
        store.save().map_err(|e| {
            AgentError::ConfigurationError(format!("Failed to save prompt config store: {}", e))
        })?;

        info!("Saved prompt configuration to store");
        Ok(())
    }

    /// Save configuration to file system
    /// DEPRECATED: Use save_config_to_store() instead. Kept for backwards compatibility during migration.
    pub fn save_config(&self) -> Result<(), AgentError> {
        // For backwards compatibility, do nothing
        // This method should no longer be used in production code
        warn!("DEPRECATED: PromptManager::save_config() called. Use save_config_to_store() instead.");
        Ok(())
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

    /// Get specialist prompt for delegation system
    pub fn get_specialist_prompt(&self, agent_type: &str) -> String {
        let prompt_type = match agent_type {
            "browser" => PromptType::BrowserSpecialist,
            "desktop" => PromptType::DesktopSpecialist,
            "file" => PromptType::FileSpecialist,
            _ => {
                warn!("Unknown specialist agent type: {}. Using file specialist.", agent_type);
                PromptType::FileSpecialist
            }
        };

        self.get_prompt(prompt_type, None)
            .unwrap_or_else(|_| {
                match agent_type {
                    "browser" => DefaultPrompts::browser_specialist().content,
                    "desktop" => DefaultPrompts::desktop_specialist().content,
                    _ => DefaultPrompts::file_specialist().content,
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
            _ => {
                warn!("Unknown expert agent type: {}. Using general expert.", agent_type);
                PromptType::GeneralExpert
            }
        };

        self.get_prompt(prompt_type, None)
            .unwrap_or_else(|_| DefaultPrompts::general_expert().content)
    }

    /// Update a prompt template
    pub fn update_prompt(&mut self, prompt_type: PromptType, content: String, app_handle: &AppHandle) -> Result<(), AgentError> {
        if let Some(template) = self.templates.get_mut(&prompt_type) {
            if template.customizable {
                template.content = content.clone();

                // Also update in custom prompts config
                self.config.custom_prompts.insert(
                    prompt_type.as_str().to_string(),
                    template.clone()
                );

                self.save_config_to_store(app_handle)?;
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

    /// Reset a prompt to its default value
    pub fn reset_prompt(&mut self, prompt_type: PromptType, app_handle: &AppHandle) -> Result<(), AgentError> {
        if let Some(default_template) = DefaultPrompts::get_all().get(&prompt_type) {
            self.templates.insert(prompt_type.clone(), default_template.clone());
            self.config.custom_prompts.remove(prompt_type.as_str());
            self.save_config_to_store(app_handle)?;
            info!("Reset prompt template to default: {:?}", prompt_type);
            Ok(())
        } else {
            Err(AgentError::ConfigurationError(
                format!("Default template not found for: {:?}", prompt_type)
            ))
        }
    }

    /// Set global variables
    pub fn set_global_variables(&mut self, variables: HashMap<String, String>, app_handle: &AppHandle) -> Result<(), AgentError> {
        self.config.global_variables = variables;
        self.save_config_to_store(app_handle)?;
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

    /// Get configuration file path
    fn get_config_path() -> Result<PathBuf, AgentError> {
        let home = dirs::home_dir()
            .ok_or_else(|| AgentError::ConfigurationError("Unable to find home directory".to_string()))?;
        Ok(home.join(".juno").join("prompts.json"))
    }
}

impl Default for PromptManager {
    fn default() -> Self {
        Self::new()
    }
}

use serde::{Deserialize, Serialize};

use std::env;
use tracing::{info, warn};
use crate::agent::structs::AgentError;
use crate::agent::prompts::manager::PromptManager;


// Add centralized settings support
use crate::settings::{ProviderSettings as CentralizedProviderSettings, ProviderConfig as CentralizedProviderConfig};

/// Agent execution mode
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub enum AgentMode {
    /// Single agent handles all tasks directly
    Single,
    /// Multi-agent system with specialized agents
    Multi,
}

impl Default for AgentMode {
    fn default() -> Self {
        AgentMode::Multi
    }
}

impl AgentMode {
    pub fn to_string(&self) -> &'static str {
        match self {
            AgentMode::Single => "single",
            AgentMode::Multi => "multi",
        }
    }

    pub fn from_str(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "single" => Some(AgentMode::Single),
            "multi" => Some(AgentMode::Multi),
            _ => None,
        }
    }
}

/// Configuration structure for AI providers
/// Uses centralized ProviderSettings directly instead of duplicating the structure
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ProviderConfig {
    /// The active provider ID
    pub active_provider: String,
    /// Agent execution mode (single vs multi-agent)
    pub agent_mode: AgentMode,
    /// Configuration for each provider (uses centralized type)
    pub providers: Vec<CentralizedProviderConfig>,
}

impl Default for ProviderConfig {
    fn default() -> Self {
        ProviderConfig {
            active_provider: "anthropic".to_string(),
            agent_mode: AgentMode::Multi,
            providers: vec![
                CentralizedProviderConfig {
                    id: "anthropic".to_string(),
                    api_key: None,
                    model: Some("claude-sonnet-4-20250514".to_string()),
                    max_tokens: Some(4096),
                    temperature: Some(0.7),
                    system_prompt: None,
                },
                CentralizedProviderConfig {
                    id: "openai".to_string(),
                    api_key: None,
                    model: Some("gpt-4o".to_string()),
                    max_tokens: Some(4096),
                    temperature: Some(0.7),
                    system_prompt: None,
                },
                CentralizedProviderConfig {
                    id: "rig".to_string(),
                    api_key: None, // Rig uses OpenAI's API key by default
                    model: Some("gpt-4o".to_string()),
                    max_tokens: Some(4096),
                    temperature: Some(0.7),
                    system_prompt: None,
                },
                CentralizedProviderConfig {
                    id: "gemini".to_string(),
                    api_key: None,
                    model: Some("gemini-1.5-pro".to_string()),
                    max_tokens: Some(4096),
                    temperature: Some(0.7),
                    system_prompt: None,
                },
            ],
        }
    }
}

impl ProviderConfig {
    /// Load configuration from centralized settings manager.
    /// NEW: Uses centralized settings instead of direct JSON store access.
    /// Used by: Application startup for configuration initialization.
    pub async fn load_from_centralized_settings(settings_manager: &crate::settings::manager::SettingsManager) -> Result<Self, AgentError> {
        let provider_settings = settings_manager.get_provider_settings().await
            .map_err(|e| AgentError::ConfigurationError(format!("Failed to load provider settings: {}", e)))?;

        let config = Self::from_centralized_settings(&provider_settings)?;
        info!("Loaded provider configuration from centralized settings");
        Ok(config)
    }

    /// Save configuration to centralized settings manager.
    /// NEW: Uses centralized settings instead of direct JSON store access.
    /// Used by: Settings UI and provider configuration updates.
    pub async fn save_to_centralized_settings(&self, settings_manager: &crate::settings::manager::SettingsManager) -> Result<(), AgentError> {
        let provider_settings = self.to_centralized_settings()?;
        settings_manager.set_provider_settings(&provider_settings).await
            .map_err(|e| AgentError::ConfigurationError(format!("Failed to save provider settings: {}", e)))?;
        info!("Saved provider configuration to centralized settings");
        Ok(())
    }

    /// Convert from centralized ProviderSettings to ProviderConfig.
    /// Handles schema differences between the two formats.
    fn from_centralized_settings(settings: &CentralizedProviderSettings) -> Result<Self, AgentError> {
        let providers = settings.providers.clone();

        // Ensure all default providers are present for backwards compatibility
        let default_config = Self::default();
        let mut final_providers = providers.clone();
        for default_provider in &default_config.providers {
            if !final_providers.iter().any(|p| p.id == default_provider.id) {
                info!("Adding missing provider from defaults: {}", default_provider.id);
                final_providers.push(default_provider.clone());
            }
        }

        // Use default agent mode since it's now managed in AgentSettings
        let agent_mode = AgentMode::default();

        Ok(Self {
            active_provider: settings.active_provider.clone(),
            agent_mode,
            providers: final_providers,
        })
    }

        /// Convert from ProviderConfig to centralized ProviderSettings.
    /// Handles schema differences between the two formats.
    fn to_centralized_settings(&self) -> Result<CentralizedProviderSettings, AgentError> {
        let providers = self.providers.clone();

        Ok(CentralizedProviderSettings {
            active_provider: self.active_provider.clone(),
            providers,
        })
    }



    /// Update provider API key
    pub fn update_api_key(&mut self, provider_id: &str, api_key: String) -> Result<(), AgentError> {
        if let Some(provider) = self.providers.iter_mut().find(|p| p.id == provider_id) {
            provider.api_key = Some(api_key);
            Ok(())
        } else {
            Err(AgentError::ConfigurationError(format!("Provider '{}' not found", provider_id)))
        }
    }

    /// Update provider model
    pub fn update_model(&mut self, provider_id: &str, model: String) -> Result<(), AgentError> {
        if let Some(provider) = self.providers.iter_mut().find(|p| p.id == provider_id) {
            provider.model = Some(model);
            Ok(())
        } else {
            Err(AgentError::ConfigurationError(format!("Provider '{}' not found", provider_id)))
        }
    }

    /// Set active provider
    pub fn set_active_provider(&mut self, provider_id: String) -> Result<(), AgentError> {
        // Verify the provider exists
        if !self.providers.iter().any(|p| p.id == provider_id) {
            return Err(AgentError::ConfigurationError(format!("Provider '{}' not found", provider_id)));
        }
        self.active_provider = provider_id;
        Ok(())
    }

    /// Set agent mode
    pub fn set_agent_mode(&mut self, mode: AgentMode) -> Result<(), AgentError> {
        self.agent_mode = mode;
        Ok(())
    }

    /// Get agent mode
    pub fn get_agent_mode(&self) -> &AgentMode {
        &self.agent_mode
    }

    /// Get settings for the active provider
    pub fn get_active_provider_settings(&self) -> Option<&CentralizedProviderConfig> {
        self.providers.iter().find(|p| p.id == self.active_provider)
    }

    /// Get settings for a specific provider
    pub fn get_provider_settings(&self, provider_id: &str) -> Option<&CentralizedProviderConfig> {
        self.providers.iter().find(|p| p.id == provider_id)
    }


}

/// Apply provider settings to environment variables (convenience method)
/// Note: This creates a default configuration without app handle support
pub fn apply_provider_settings_to_env() -> Result<(), AgentError> {
    let config = ProviderConfig::default();
    let prompt_manager = crate::agent::prompts::PromptManager::new();
    apply_provider_settings_internal(&config, &prompt_manager)
}

/// Apply provider settings to environment variables from a given app handle
/// NEW: Uses centralized settings instead of direct JSON store access.
/// Uses the centralized prompt manager for default prompts
pub async fn apply_provider_settings_to_env_with_centralized_settings(settings_manager: &crate::settings::manager::SettingsManager) -> Result<(), AgentError> {
    let config = ProviderConfig::load_from_centralized_settings(settings_manager).await?;

    // Load prompt manager for default prompts - this will need to be updated in a future step
    let prompt_manager = crate::agent::prompts::PromptManager::new();

    apply_provider_settings_internal(&config, &prompt_manager)
}



/// Internal function to apply provider settings to environment variables
fn apply_provider_settings_internal(config: &ProviderConfig, prompt_manager: &PromptManager) -> Result<(), AgentError> {
    // Determine which provider's settings to apply.
    // Priority: AI_PROVIDER env var, then config.active_provider as fallback.
    let provider_id_to_apply = env::var("AI_PROVIDER")
        .unwrap_or_else(|_| {
            info!("AI_PROVIDER env var not set, using active_provider from config: {}", &config.active_provider);
            config.active_provider.clone()
        });

    info!("Applying environment settings for provider: {}", provider_id_to_apply);

    if let Some(settings) = config.providers.iter().find(|p| p.id == provider_id_to_apply) {
        match settings.id.as_str() {
            "anthropic" => {
                if let Some(api_key) = &settings.api_key {
                    env::set_var("ANTHROPIC_API_KEY", api_key);
                }
                if let Some(model) = &settings.model {
                    env::set_var("ANTHROPIC_MODEL", model);
                }
                if let Some(max_tokens) = settings.max_tokens {
                    env::set_var("ANTHROPIC_MAX_TOKENS", max_tokens.to_string());
                }
                // Use prompt manager for default system prompt
                let default_prompt = prompt_manager.get_default_system_prompt();
                let prompt_to_set = settings.system_prompt.as_deref()
                    .filter(|s| !s.is_empty())
                    .unwrap_or(&default_prompt);
                env::set_var("ANTHROPIC_SYSTEM_PROMPT", prompt_to_set);
            },
            "openai" => {
                if let Some(api_key) = &settings.api_key {
                    env::set_var("OPENAI_API_KEY", api_key);
                }
                if let Some(model) = &settings.model {
                    env::set_var("OPENAI_MODEL", model);
                }
                if let Some(max_tokens) = settings.max_tokens {
                    env::set_var("OPENAI_MAX_TOKENS", max_tokens.to_string());
                }
                if let Some(temperature) = settings.temperature {
                    env::set_var("OPENAI_TEMPERATURE", temperature.to_string());
                }
                // Use prompt manager for default system prompt
                let default_prompt = prompt_manager.get_default_system_prompt();
                let prompt_to_set = settings.system_prompt.as_deref()
                    .filter(|s| !s.is_empty())
                    .unwrap_or(&default_prompt);
                env::set_var("OPENAI_SYSTEM_PROMPT", prompt_to_set);
            },
            "rig" => {
                let mut rig_api_key_set = false;
                if let Some(api_key) = &settings.api_key {
                    env::set_var("OPENAI_API_KEY", api_key);
                    rig_api_key_set = true;
                    info!("Applied Rig's specific API key for OPENAI_API_KEY.");
                }

                if !rig_api_key_set {
                    if let Some(openai_settings) = config.providers.iter().find(|p| p.id == "openai") {
                        if let Some(api_key) = &openai_settings.api_key {
                            env::set_var("OPENAI_API_KEY", api_key);
                            info!("Applied OpenAI provider's API key for Rig's OPENAI_API_KEY.");
                        }
                    }
                }

                if let Some(model) = &settings.model {
                    env::set_var("OPENAI_MODEL", model);
                }
                if let Some(max_tokens) = settings.max_tokens {
                    env::set_var("OPENAI_MAX_TOKENS", max_tokens.to_string());
                }
                if let Some(temperature) = settings.temperature {
                    env::set_var("OPENAI_TEMPERATURE", temperature.to_string());
                }
                // Use prompt manager for default system prompt
                let default_prompt = prompt_manager.get_default_system_prompt();
                let prompt_to_set = settings.system_prompt.as_deref()
                    .filter(|s| !s.is_empty())
                    .unwrap_or(&default_prompt);
                env::set_var("RIG_SYSTEM_PROMPT", prompt_to_set);
            },
            "gemini" => {
                if let Some(api_key) = &settings.api_key {
                    env::set_var("GEMINI_API_KEY", api_key);
                }
                if let Some(model) = &settings.model {
                    env::set_var("GEMINI_MODEL", model);
                }
                if let Some(max_tokens) = settings.max_tokens {
                    env::set_var("GEMINI_MAX_TOKENS", max_tokens.to_string());
                }
                if let Some(temperature) = settings.temperature {
                    env::set_var("GEMINI_TEMPERATURE", temperature.to_string());
                }
                // Use prompt manager for default system prompt
                let default_prompt = prompt_manager.get_default_system_prompt();
                let prompt_to_set = settings.system_prompt.as_deref()
                    .filter(|s| !s.is_empty())
                    .unwrap_or(&default_prompt);
                env::set_var("GEMINI_SYSTEM_PROMPT", prompt_to_set);
            },
            _ => {
                warn!("Attempted to apply settings for an unknown provider ID: {}", settings.id);
            }
        }
    } else {
        warn!("Could not find settings for provider: {}. No specific settings applied.", provider_id_to_apply);
    }

    Ok(())
}

/// Load provider configuration from centralized settings
/// NEW: Uses centralized settings instead of direct JSON store access.
/// Used by: Application startup and provider configuration initialization
///
/// # Arguments
/// * `settings_manager` - Centralized settings manager
pub async fn load_provider_config_from_centralized_settings(
    settings_manager: &crate::settings::manager::SettingsManager,
) -> Result<ProviderConfig, String> {
    let loaded_config = ProviderConfig::load_from_centralized_settings(settings_manager).await
        .map_err(|e| format!("Failed to load provider config: {}", e))?;

    info!("Loaded provider configuration from centralized settings on startup");
    Ok(loaded_config)
}

/// Save provider configuration to centralized settings
/// NEW: Uses centralized settings instead of direct JSON store access.
/// Used by: Application shutdown and provider configuration changes
///
/// # Arguments
/// * `settings_manager` - Centralized settings manager
/// * `config` - Provider configuration to save
pub async fn save_provider_config_to_centralized_settings(
    settings_manager: &crate::settings::manager::SettingsManager,
    config: &ProviderConfig
) -> Result<(), String> {
    config.save_to_centralized_settings(settings_manager).await
        .map_err(|e| format!("Failed to save provider config: {}", e))?;
    info!("Saved provider configuration to centralized settings");
    Ok(())
}

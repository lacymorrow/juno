use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;
use std::{env, fs::File, io::Write};
use std::io::ErrorKind;
use tracing::{info, error, warn};
use crate::agent::structs::AgentError;
use crate::agent::prompts::manager::PromptManager;
use tauri::AppHandle;
use tauri_plugin_store::StoreExt;

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
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ProviderConfig {
    /// The active provider ID
    pub active_provider: String,
    /// Agent execution mode (single vs multi-agent)
    pub agent_mode: AgentMode,
    /// Configuration for each provider
    pub providers: Vec<ProviderSettings>,
}

/// Settings for a specific provider
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ProviderSettings {
    /// Provider identifier
    pub id: String,
    /// API key (encrypted or obscured in the future)
    pub api_key: Option<String>,
    /// Model name to use
    pub model: Option<String>,
    /// Maximum tokens to generate
    pub max_tokens: Option<u32>,
    /// Temperature setting (0.0-1.0)
    pub temperature: Option<f32>,
    /// System prompt to use (if supported)
    pub system_prompt: Option<String>,
}

impl Default for ProviderConfig {
    fn default() -> Self {
        ProviderConfig {
            active_provider: "anthropic".to_string(),
            agent_mode: AgentMode::Multi,
            providers: vec![
                ProviderSettings {
                    id: "anthropic".to_string(),
                    api_key: None,
                    model: Some("claude-3-7-sonnet-20250219".to_string()),
                    max_tokens: Some(4096),
                    temperature: Some(0.7),
                    system_prompt: None,
                },
                ProviderSettings {
                    id: "openai".to_string(),
                    api_key: None,
                    model: Some("gpt-4o".to_string()),
                    max_tokens: Some(4096),
                    temperature: Some(0.7),
                    system_prompt: None,
                },
                ProviderSettings {
                    id: "rig".to_string(),
                    api_key: None, // Rig uses OpenAI's API key by default
                    model: Some("gpt-4o".to_string()),
                    max_tokens: Some(4096),
                    temperature: Some(0.7),
                    system_prompt: None,
                },
                ProviderSettings {
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
    /// Load configuration from Tauri store or create default.
    /// Attempts to load existing configuration, creates default if missing.
    /// Used by: Agent initialization and settings management.
    pub fn load_from_store(app_handle: &AppHandle) -> Result<Self, AgentError> {
        let store = app_handle.store("provider_config.json").map_err(|e| {
            AgentError::ConfigurationError(format!("Failed to access provider config store: {}", e))
        })?;

        // Try to load the configuration from store
        if let Some(config_value) = store.get("provider_config") {
            match serde_json::from_value::<Self>(config_value) {
                Ok(mut config) => {
                    // Perform configuration migration - add missing providers
                    let mut needs_save = false;
                    let default_config = Self::default();

                    for default_provider in &default_config.providers {
                        if !config.providers.iter().any(|p| p.id == default_provider.id) {
                            info!("Adding missing provider to config: {}", default_provider.id);
                            config.providers.push(default_provider.clone());
                            needs_save = true;
                        }
                    }

                    if needs_save {
                        config.save_to_store(app_handle)?;
                    }

                    info!("Loaded provider configuration from store");
                    return Ok(config);
                }
                Err(e) => {
                    error!("Failed to parse stored provider config ({}), creating default", e);
                }
            }
        }

        // No valid configuration found, create and save default
        info!("No provider configuration found in store, creating default");
        let default_config = Self::default();
        default_config.save_to_store(app_handle)?;
        Ok(default_config)
    }

    /// Save configuration to Tauri store.
    /// Serializes current configuration to JSON and saves to store.
    /// Used by: Settings UI and provider configuration updates.
    pub fn save_to_store(&self, app_handle: &AppHandle) -> Result<(), AgentError> {
        let store = app_handle.store("provider_config.json").map_err(|e| {
            AgentError::ConfigurationError(format!("Failed to access provider config store: {}", e))
        })?;

        let config_value = serde_json::to_value(self).map_err(|e| {
            AgentError::ConfigurationError(format!("Failed to serialize provider config: {}", e))
        })?;

        store.set("provider_config", config_value);
        store.save().map_err(|e| {
            AgentError::ConfigurationError(format!("Failed to save provider config store: {}", e))
        })?;

        info!("Saved provider configuration to store");
        Ok(())
    }

    /// Update provider API key
    pub fn update_api_key(&mut self, provider_id: &str, api_key: String, app_handle: &AppHandle) -> Result<(), AgentError> {
        if let Some(provider) = self.providers.iter_mut().find(|p| p.id == provider_id) {
            provider.api_key = Some(api_key);
            self.save_to_store(app_handle)
        } else {
            Err(AgentError::ConfigurationError(format!("Provider '{}' not found", provider_id)))
        }
    }

    /// Update provider model
    pub fn update_model(&mut self, provider_id: &str, model: String, app_handle: &AppHandle) -> Result<(), AgentError> {
        if let Some(provider) = self.providers.iter_mut().find(|p| p.id == provider_id) {
            provider.model = Some(model);
            self.save_to_store(app_handle)
        } else {
            Err(AgentError::ConfigurationError(format!("Provider '{}' not found", provider_id)))
        }
    }

    /// Set active provider
    pub fn set_active_provider(&mut self, provider_id: String, app_handle: &AppHandle) -> Result<(), AgentError> {
        // Verify the provider exists
        if !self.providers.iter().any(|p| p.id == provider_id) {
            return Err(AgentError::ConfigurationError(format!("Provider '{}' not found", provider_id)));
        }
        self.active_provider = provider_id;
        self.save_to_store(app_handle)
    }

    /// Set agent mode
    pub fn set_agent_mode(&mut self, mode: AgentMode, app_handle: &AppHandle) -> Result<(), AgentError> {
        self.agent_mode = mode;
        self.save_to_store(app_handle)
    }

    /// Get agent mode
    pub fn get_agent_mode(&self) -> &AgentMode {
        &self.agent_mode
    }

    /// Get settings for the active provider
    pub fn get_active_provider_settings(&self) -> Option<&ProviderSettings> {
        self.providers.iter().find(|p| p.id == self.active_provider)
    }

    /// Get settings for a specific provider
    pub fn get_provider_settings(&self, provider_id: &str) -> Option<&ProviderSettings> {
        self.providers.iter().find(|p| p.id == provider_id)
    }

    /// Get configuration file path
    fn get_config_path() -> Result<PathBuf, AgentError> {
        let home = dirs::home_dir()
            .ok_or_else(|| AgentError::ConfigurationError("Unable to find home directory".to_string()))?;
        Ok(home.join(".juno").join("provider_config.json"))
    }
}

/// Apply provider settings to environment variables (convenience method)
/// Note: This creates a default configuration without app handle support
pub fn apply_provider_settings_to_env() -> Result<(), AgentError> {
    let config = ProviderConfig::default();
    let prompt_manager = PromptManager::new();
    apply_provider_settings_internal(&config, &prompt_manager)
}

/// Apply provider settings to environment variables from a given app handle
/// Uses the centralized prompt manager for default prompts
pub fn apply_provider_settings_to_env_with_handle(app_handle: &AppHandle) -> Result<(), AgentError> {
    let config = ProviderConfig::load_from_store(app_handle)?;

    // Load prompt manager for default prompts
    let prompt_manager = PromptManager::load_from_store(app_handle).unwrap_or_else(|_| PromptManager::new());

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

use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;
use std::{env, fs::File, io::Write};
use std::io::ErrorKind;
use tracing::{info, error, warn};
use crate::agent::structs::AgentError;
use crate::agent::prompts::PromptManager;

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
    /// Load configuration from the file system or create default
    pub fn load() -> Result<Self, AgentError> {
        let config_path = Self::get_config_path()?;
        match fs::read_to_string(&config_path) {
            Ok(contents) => {
                let mut config: ProviderConfig = serde_json::from_str(&contents).map_err(|e| {
                    error!("Failed to parse config file: {}. Using default.", e);
                    let default_config = Self::default();
                    // Attempt to save the default config if parsing failed, but don't error out if save fails here.
                    let _ = default_config.save();
                    AgentError::ConfigurationError(format!("Failed to parse config: {}", e))
                }).or_else(|_agent_err: crate::agent::structs::AgentError|{
                     info!("Creating default configuration as parsing failed or to ensure structure.");
                     let default_config = Self::default();
                     default_config.save()?;
                     Ok::<ProviderConfig, crate::agent::structs::AgentError>(default_config)
                })?;

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
                    config.save()?;
                }

                Ok(config)
            },
            Err(e) if e.kind() == ErrorKind::NotFound => {
                info!("Config file not found at {:?}, creating default.", config_path);
                let default_config = Self::default();
                default_config.save()?;
                Ok(default_config)
            }
            Err(e) => {
                error!("Failed to read config file at {:?}: {}. Using in-memory default.", config_path, e);
                Ok(Self::default()) // Return in-memory default if read fails for other reasons
            }
        }
    }

    /// Save configuration to file
    pub fn save(&self) -> Result<(), AgentError> {
        let config_path = Self::get_config_path()?;

        // Ensure the directory exists
        if let Some(parent) = config_path.parent() {
            fs::create_dir_all(parent).map_err(|e| {
                AgentError::ConfigurationError(format!("Failed to create config directory: {}", e))
            })?;
        }

        let contents = serde_json::to_string_pretty(self).map_err(|e| {
            AgentError::ConfigurationError(format!("Failed to serialize config: {}", e))
        })?;

        let mut file = File::create(&config_path).map_err(|e| {
            AgentError::ConfigurationError(format!("Failed to create config file: {}", e))
        })?;

        file.write_all(contents.as_bytes()).map_err(|e| {
            AgentError::ConfigurationError(format!("Failed to write config file: {}", e))
        })?;

        info!("Saved provider configuration to {:?}", config_path);
        Ok(())
    }

    /// Update provider API key
    pub fn update_api_key(&mut self, provider_id: &str, api_key: String) -> Result<(), AgentError> {
        if let Some(provider) = self.providers.iter_mut().find(|p| p.id == provider_id) {
            provider.api_key = Some(api_key);
            self.save()
        } else {
            Err(AgentError::ConfigurationError(format!("Provider '{}' not found", provider_id)))
        }
    }

    /// Update provider model
    pub fn update_model(&mut self, provider_id: &str, model: String) -> Result<(), AgentError> {
        if let Some(provider) = self.providers.iter_mut().find(|p| p.id == provider_id) {
            provider.model = Some(model);
            self.save()
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
        self.save()
    }

    /// Set agent mode
    pub fn set_agent_mode(&mut self, mode: AgentMode) -> Result<(), AgentError> {
        self.agent_mode = mode;
        self.save()
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

/// Apply provider settings to environment variables
/// Uses the centralized prompt manager for default prompts
pub fn apply_provider_settings_to_env() -> Result<(), AgentError> {
    let config = ProviderConfig::load()?;

    // Load prompt manager for default prompts
    let prompt_manager = PromptManager::load().unwrap_or_default();

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

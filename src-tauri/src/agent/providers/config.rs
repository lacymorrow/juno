use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;
use std::{env, fs::File, io::Write};
use std::io::ErrorKind;
use tracing::{info, error};
use crate::agent::structs::AgentError;

/// Configuration structure for AI providers
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ProviderConfig {
    /// The active provider ID
    pub active_provider: String,
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
                match serde_json::from_str(&contents) {
                    Ok(config) => {
                        info!("Loaded provider configuration from {:?}", config_path);
                        Ok(config)
                    }
                    Err(e) => {
                        error!("Failed to parse config file: {}", e);
                        info!("Creating default configuration");
                        let default_config = Self::default();
                        default_config.save()?;
                        Ok(default_config)
                    }
                }
            }
            Err(e) if e.kind() == ErrorKind::NotFound => {
                info!("Config file not found, creating default");
                let default_config = Self::default();
                default_config.save()?;
                Ok(default_config)
            }
            Err(e) => {
                error!("Failed to read config file: {}", e);
                Err(AgentError::ConfigurationError(format!("Failed to read config file: {}", e)))
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
        for provider in &mut self.providers {
            if provider.id == provider_id {
                provider.api_key = Some(api_key);
                return self.save();
            }
        }

        Err(AgentError::ConfigurationError(format!("Provider '{}' not found", provider_id)))
    }

    /// Update provider model
    pub fn update_model(&mut self, provider_id: &str, model: String) -> Result<(), AgentError> {
        for provider in &mut self.providers {
            if provider.id == provider_id {
                provider.model = Some(model);
                return self.save();
            }
        }

        Err(AgentError::ConfigurationError(format!("Provider '{}' not found", provider_id)))
    }

    /// Set active provider
    pub fn set_active_provider(&mut self, provider_id: &str) -> Result<(), AgentError> {
        // Verify provider exists
        if !self.providers.iter().any(|p| p.id == provider_id) {
            return Err(AgentError::ConfigurationError(format!("Provider '{}' not found", provider_id)));
        }

        self.active_provider = provider_id.to_string();
        self.save()?;

        // Also set the environment variable for runtime use
        env::set_var("AI_PROVIDER", provider_id);

        Ok(())
    }

    /// Get settings for the active provider
    pub fn get_active_provider_settings(&self) -> Option<&ProviderSettings> {
        self.providers.iter().find(|p| p.id == self.active_provider)
    }

    /// Get settings for a specific provider
    pub fn get_provider_settings(&self, provider_id: &str) -> Option<&ProviderSettings> {
        self.providers.iter().find(|p| p.id == provider_id)
    }

    /// Get provider configuration path
    fn get_config_path() -> Result<PathBuf, AgentError> {
        let home_dir = dirs::home_dir().ok_or_else(|| {
            AgentError::ConfigurationError("Could not determine home directory".to_string())
        })?;

        Ok(home_dir.join(".config").join("juno").join("ai_providers.json"))
    }
}

/// Apply provider settings to environment variables
pub fn apply_provider_settings_to_env() -> Result<(), AgentError> {
    let config = ProviderConfig::load()?;

    // Set active provider
    env::set_var("AI_PROVIDER", &config.active_provider);

    // Apply settings for active provider
    if let Some(provider) = config.get_active_provider_settings() {
        match provider.id.as_str() {
            "anthropic" => {
                if let Some(api_key) = &provider.api_key {
                    env::set_var("ANTHROPIC_API_KEY", api_key);
                }
                if let Some(model) = &provider.model {
                    env::set_var("ANTHROPIC_MODEL", model);
                }
                if let Some(max_tokens) = provider.max_tokens {
                    env::set_var("ANTHROPIC_MAX_TOKENS", max_tokens.to_string());
                }
                if let Some(system_prompt) = &provider.system_prompt {
                    env::set_var("ANTHROPIC_SYSTEM_PROMPT", system_prompt);
                }
            },
            "openai" => {
                if let Some(api_key) = &provider.api_key {
                    env::set_var("OPENAI_API_KEY", api_key);
                }
                if let Some(model) = &provider.model {
                    env::set_var("OPENAI_MODEL", model);
                }
                if let Some(max_tokens) = provider.max_tokens {
                    env::set_var("OPENAI_MAX_TOKENS", max_tokens.to_string());
                }
                if let Some(temperature) = provider.temperature {
                    env::set_var("OPENAI_TEMPERATURE", temperature.to_string());
                }
            },
            _ => {
                info!("Unknown provider ID: {}", provider.id);
            }
        }
    }

    Ok(())
}

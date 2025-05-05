use std::env;
use tracing::{info, warn};

// Use the consolidated core module
use crate::agent::core::{AgentError, AgentBrain};
use crate::agent::providers::anthropic::AnthropicBrain;
use crate::agent::providers::openai::OpenAIBrain;
use crate::agent::providers::config::{ProviderConfig, apply_provider_settings_to_env};

/// Enumeration of available AI providers
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Provider {
    Anthropic,
    OpenAI,
    // Add other providers as needed
}

impl Provider {
    /// Convert a string to a Provider enum
    pub fn from_str(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "anthropic" => Some(Provider::Anthropic),
            "openai" => Some(Provider::OpenAI),
            // Add other provider matches as needed
            _ => None,
        }
    }

    /// Get display name for the provider
    pub fn display_name(&self) -> &'static str {
        match self {
            Provider::Anthropic => "Anthropic Claude",
            Provider::OpenAI => "OpenAI GPT",
        }
    }

    /// Get provider ID string
    pub fn id(&self) -> &'static str {
        match self {
            Provider::Anthropic => "anthropic",
            Provider::OpenAI => "openai",
        }
    }
}

/// Struct containing provider information for UI display
#[derive(Debug, Clone, serde::Serialize)]
pub struct ProviderInfo {
    pub id: String,
    pub name: String,
    pub is_available: bool,
    pub is_default: bool,
}

/// Factory for creating provider-specific AgentBrain implementations
pub struct BrainFactory;

impl BrainFactory {
    /// Initialize configuration and apply settings to environment
    pub fn init() -> Result<(), AgentError> {
        apply_provider_settings_to_env()
    }

    /// Get the current provider from configuration or environment
    pub fn get_current_provider() -> Provider {
        // First try from environment (set during runtime)
        let provider_str = env::var("AI_PROVIDER").unwrap_or_else(|_| {
            // If not in env, try to load from config
            match ProviderConfig::load() {
                Ok(config) => config.active_provider,
                Err(_) => "anthropic".to_string(), // Default fallback
            }
        });

        Provider::from_str(&provider_str).unwrap_or(Provider::Anthropic)
    }

    /// Get list of all available providers with their status
    pub fn list_providers() -> Vec<ProviderInfo> {
        let current_provider = Self::get_current_provider();
        let providers = vec![Provider::Anthropic, Provider::OpenAI];

        // Try to load config to check for stored API keys
        let config = ProviderConfig::load().ok();

        providers.into_iter().map(|provider| {
            let provider_id = provider.id();

            // A provider is available if either:
            // 1. Its API key is already in the environment, or
            // 2. Its API key is stored in the config
            let is_available = match provider {
                Provider::Anthropic => {
                    env::var("ANTHROPIC_API_KEY").is_ok() ||
                    config.as_ref()
                        .and_then(|c| c.get_provider_settings(provider_id))
                        .and_then(|s| s.api_key.as_ref())
                        .is_some()
                },
                Provider::OpenAI => {
                    env::var("OPENAI_API_KEY").is_ok() ||
                    config.as_ref()
                        .and_then(|c| c.get_provider_settings(provider_id))
                        .and_then(|s| s.api_key.as_ref())
                        .is_some()
                },
            };

            ProviderInfo {
                id: provider_id.to_string(),
                name: provider.display_name().to_string(),
                is_available,
                is_default: provider == current_provider,
            }
        }).collect()
    }

    /// Create an AgentBrain implementation based on provider configuration
    pub fn create_brain() -> Result<Box<dyn AgentBrain + Send + Sync>, AgentError> {
        // Apply settings from config to environment before creating the brain
        apply_provider_settings_to_env()?;

        // Get provider from environment variable or use default
        let provider_str = env::var("AI_PROVIDER").unwrap_or_else(|_| "anthropic".to_string());
        info!("Using AI provider from environment: {}", provider_str);

        match Provider::from_str(&provider_str) {
            Some(Provider::Anthropic) => {
                info!("Initializing Anthropic brain...");
                let brain = AnthropicBrain::from_env()?;
                Ok(Box::new(brain))
            },
            Some(Provider::OpenAI) => {
                info!("Initializing OpenAI brain...");
                match OpenAIBrain::from_env() {
                    Ok(brain) => Ok(Box::new(brain)),
                    Err(e) => {
                        warn!("Failed to initialize OpenAI brain: {}. Falling back to Anthropic.", e);
                        let brain = AnthropicBrain::from_env()?;
                        Ok(Box::new(brain))
                    }
                }
            },
            None => {
                warn!("Unknown AI provider specified: '{}'. Using Anthropic as fallback.", provider_str);
                let brain = AnthropicBrain::from_env()?;
                Ok(Box::new(brain))
            }
        }
    }
}

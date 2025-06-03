use std::env;
use tracing::{info, warn};

use crate::agent::structs::AgentError;
use crate::agent::traits::AgentBrain;
use crate::agent::providers::anthropic::AnthropicBrain;
use crate::agent::providers::openai::OpenAIBrain;
use crate::agent::providers::rig::RigBrain;
use crate::agent::providers::config::{ProviderConfig, apply_provider_settings_to_env};

/// Enumeration of available AI providers
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Provider {
    Anthropic,
    OpenAI,
    Rig,
    // Add other providers as needed
}

impl Provider {
    /// Convert a string to a Provider enum
    pub fn from_str(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "anthropic" => Some(Provider::Anthropic),
            "openai" => Some(Provider::OpenAI),
            "rig" => Some(Provider::Rig),
            // Add other provider matches as needed
            _ => None,
        }
    }

    /// Get display name for the provider
    pub fn display_name(&self) -> &'static str {
        match self {
            Provider::Anthropic => "Anthropic Claude",
            Provider::OpenAI => "OpenAI GPT",
            Provider::Rig => "Rig AI Agent",
        }
    }

    /// Get description for the provider
    pub fn description(&self) -> &'static str {
        match self {
            Provider::Anthropic => "High-performance AI assistant with advanced reasoning capabilities",
            Provider::OpenAI => "OpenAI's GPT models for conversational AI and text generation",
            Provider::Rig => "Rig framework for building AI agents with structured outputs",
        }
    }

    /// Get available models for the provider
    pub fn models(&self) -> Vec<String> {
        match self {
            Provider::Anthropic => vec![
                "claude-3-5-sonnet-20241022".to_string(),
                "claude-3-5-haiku-20241022".to_string(),
                "claude-3-opus-20240229".to_string(),
            ],
            Provider::OpenAI => vec![
                "gpt-4o".to_string(),
                "gpt-4o-mini".to_string(),
                "gpt-4-turbo".to_string(),
                "gpt-3.5-turbo".to_string(),
            ],
            Provider::Rig => vec![
                "gpt-4o".to_string(),
                "gpt-4o-mini".to_string(),
                "claude-3-5-sonnet-20241022".to_string(),
            ],
        }
    }

    /// Get default model for the provider
    pub fn default_model(&self) -> &'static str {
        match self {
            Provider::Anthropic => "claude-3-5-sonnet-20241022",
            Provider::OpenAI => "gpt-4o",
            Provider::Rig => "gpt-4o",
        }
    }

    /// Get provider ID string
    pub fn id(&self) -> &'static str {
        match self {
            Provider::Anthropic => "anthropic",
            Provider::OpenAI => "openai",
            Provider::Rig => "rig",
        }
    }
}

/// Struct containing provider information for UI display
#[derive(Debug, Clone, serde::Serialize)]
pub struct ProviderInfo {
    pub id: String,
    pub name: String,
    pub description: String,
    pub models: Vec<String>,
    pub default_model: String,
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
        let provider_str = env::var("AI_PROVIDER").unwrap_or_else(|_| {
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
        let providers = vec![Provider::Anthropic, Provider::OpenAI, Provider::Rig];
        let config = ProviderConfig::load().ok();

        providers.into_iter().map(|provider| {
            let provider_id = provider.id();
            let is_available = match provider {
                Provider::Anthropic => env::var("ANTHROPIC_API_KEY").is_ok() || config.as_ref().and_then(|c| c.get_provider_settings(provider_id)).and_then(|s| s.api_key.as_ref()).is_some(),
                Provider::OpenAI => env::var("OPENAI_API_KEY").is_ok() || config.as_ref().and_then(|c| c.get_provider_settings(provider_id)).and_then(|s| s.api_key.as_ref()).is_some(),
                Provider::Rig => env::var("OPENAI_API_KEY").is_ok() || config.as_ref().and_then(|c| c.get_provider_settings("openai")).and_then(|s| s.api_key.as_ref()).is_some() || config.as_ref().and_then(|c| c.get_provider_settings(provider_id)).and_then(|s| s.api_key.as_ref()).is_some(),
            };
            ProviderInfo {
                id: provider_id.to_string(),
                name: provider.display_name().to_string(),
                description: provider.description().to_string(),
                models: provider.models(),
                default_model: provider.default_model().to_string(),
                is_available,
                is_default: provider == current_provider,
            }
        }).collect()
    }

    /// Create an AgentBrain implementation based on provider configuration
    pub fn create_brain() -> Result<Box<dyn AgentBrain + Send + Sync>, AgentError> {
        let provider_str = env::var("AI_PROVIDER").unwrap_or_else(|_|
            ProviderConfig::load()
                .map(|config| config.active_provider)
                .unwrap_or_else(|e|
                    {
                        warn!("AI_PROVIDER env not set and config failed to load ({}). Defaulting to anthropic.", e);
                        "anthropic".to_string()
                    }
                )
        );
        info!("Attempting to use AI provider: {}", provider_str);
        env::set_var("AI_PROVIDER", &provider_str);
        apply_provider_settings_to_env()?;

        match Provider::from_str(&provider_str) {
            Some(Provider::Anthropic) => {
                info!("Initializing Anthropic brain...");
                AnthropicBrain::from_env().map(|b| Box::new(b) as Box<dyn AgentBrain + Send + Sync>)
            }
            Some(Provider::OpenAI) => {
                info!("Attempting to initialize OpenAI brain...");
                match OpenAIBrain::from_env() {
                    Ok(brain) => Ok(Box::new(brain) as Box<dyn AgentBrain + Send + Sync>),
                    Err(e) => {
                        warn!("Failed to initialize OpenAI brain ({}). Falling back to Anthropic.", e);
                        env::set_var("AI_PROVIDER", "anthropic");
                        apply_provider_settings_to_env()?;
                        AnthropicBrain::from_env().map(|b| Box::new(b) as Box<dyn AgentBrain + Send + Sync>)
                    }
                }
            }
            Some(Provider::Rig) => {
                info!("Attempting to initialize Rig brain...");
                match RigBrain::from_env() {
                    Ok(brain) => Ok(Box::new(brain) as Box<dyn AgentBrain + Send + Sync>),
                    Err(e) => {
                        warn!("Failed to initialize Rig brain ({}). Falling back to Anthropic.", e);
                        env::set_var("AI_PROVIDER", "anthropic");
                        apply_provider_settings_to_env()?;
                        AnthropicBrain::from_env().map(|b| Box::new(b) as Box<dyn AgentBrain + Send + Sync>)
                    }
                }
            }
            None => {
                warn!("Unknown AI provider specified: '{}'. Using Anthropic as fallback.", provider_str);
                env::set_var("AI_PROVIDER", "anthropic");
                apply_provider_settings_to_env()?;
                AnthropicBrain::from_env().map(|b| Box::new(b) as Box<dyn AgentBrain + Send + Sync>)
            }
        }
    }
}

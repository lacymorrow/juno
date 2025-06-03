use std::env;
use tracing::{info, warn};
use tauri::Manager;

use crate::agent::structs::AgentError;
use crate::agent::traits::{AgentBrain, MemoryManager, ToolProvider};
use crate::agent::multi_agent::MultiAgentOrchestrator;
use crate::agent::providers::anthropic::AnthropicBrain;
use crate::agent::providers::openai::OpenAIBrain;
use crate::agent::providers::rig::RigBrain;
use crate::agent::providers::gemini::GeminiBrain;
use crate::agent::providers::config::{ProviderConfig, apply_provider_settings_to_env};
use crate::agent::tools::anthropic_computer_use::register_anthropic_computer_use_tools;
use crate::agent::implementations::tool_provider::LocalToolProvider;
use crate::state::AppState;

/// Enumeration of available AI providers
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Provider {
    Anthropic,
    OpenAI,
    Rig,
    Gemini,
    // Add other providers as needed
}

impl Provider {
    /// Convert a string to a Provider enum
    pub fn from_str(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "anthropic" => Some(Provider::Anthropic),
            "openai" => Some(Provider::OpenAI),
            "rig" => Some(Provider::Rig),
            "gemini" => Some(Provider::Gemini),
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
            Provider::Gemini => "Google Gemini",
        }
    }

    /// Get provider ID string
    pub fn id(&self) -> &'static str {
        match self {
            Provider::Anthropic => "anthropic",
            Provider::OpenAI => "openai",
            Provider::Rig => "rig",
            Provider::Gemini => "gemini",
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

    /// Create a multi-agent orchestrator system
    pub async fn create_multi_agent_orchestrator(
        memory: std::sync::Arc<dyn MemoryManager + Send + Sync>,
        tool_provider: std::sync::Arc<dyn ToolProvider + Send + Sync>,
    ) -> Result<MultiAgentOrchestrator, AgentError> {
        MultiAgentOrchestrator::new(memory, tool_provider).await
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
        let providers = vec![Provider::Anthropic, Provider::OpenAI, Provider::Rig, Provider::Gemini];
        let config = ProviderConfig::load().ok();

        providers.into_iter().map(|provider| {
            let provider_id = provider.id();
            let is_available = match provider {
                Provider::Anthropic => env::var("ANTHROPIC_API_KEY").is_ok() || config.as_ref().and_then(|c| c.get_provider_settings(provider_id)).and_then(|s| s.api_key.as_ref()).is_some(),
                Provider::OpenAI => env::var("OPENAI_API_KEY").is_ok() || config.as_ref().and_then(|c| c.get_provider_settings(provider_id)).and_then(|s| s.api_key.as_ref()).is_some(),
                Provider::Rig => env::var("OPENAI_API_KEY").is_ok() || config.as_ref().and_then(|c| c.get_provider_settings("openai")).and_then(|s| s.api_key.as_ref()).is_some() || config.as_ref().and_then(|c| c.get_provider_settings(provider_id)).and_then(|s| s.api_key.as_ref()).is_some(),
                Provider::Gemini => env::var("GOOGLE_GEMINI_API_KEY").is_ok() || config.as_ref().and_then(|c| c.get_provider_settings(provider_id)).and_then(|s| s.api_key.as_ref()).is_some(),
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
            Some(Provider::Gemini) => {
                info!("Attempting to initialize Gemini brain...");
                match GeminiBrain::from_env() {
                    Ok(brain) => Ok(Box::new(brain) as Box<dyn AgentBrain + Send + Sync>),
                    Err(e) => {
                        warn!("Failed to initialize Gemini brain ({}). Falling back to Anthropic.", e);
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

    /// Register all available computer use tools for the agent
    pub async fn register_computer_use_tools(
        provider: &mut LocalToolProvider,
        app_handle: tauri::AppHandle,
    ) -> Result<(), String> {
        info!("Registering all Computer Use tools...");

        // Register the official Anthropic Computer Use tools
        register_anthropic_computer_use_tools(provider, app_handle.clone()).await?;

        // Register additional desktop automation tools (your existing ones)
        let state_manager = app_handle.state::<AppState>();
        crate::agent::tools::desktop_tools::register_desktop_tools(provider, state_manager, app_handle.clone()).await;

        info!("All Computer Use tools registered successfully");
        Ok(())
    }
}

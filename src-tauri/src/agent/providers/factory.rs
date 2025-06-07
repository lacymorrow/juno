use std::env;
use tracing::{info, warn};
use tauri::Manager;

use crate::agent::structs::AgentError;
use crate::agent::traits::{AgentBrain, MemoryManager, ToolProvider, AgentRunnable};
use crate::agent::multi_agent::MultiAgentOrchestrator;
use crate::agent::implementations::agent_runner::DefaultAgentRunner;
use crate::agent::providers::anthropic::AnthropicBrain;
use crate::agent::providers::openai::OpenAIBrain;
use crate::agent::providers::rig::RigBrain;
use crate::agent::providers::gemini::GeminiBrain;
use crate::agent::providers::config::{ProviderConfig, apply_provider_settings_to_env, AgentMode};
use crate::agent::tools::anthropic_computer_use::register_anthropic_computer_use_tools;
use crate::agent::implementations::tool_provider::LocalToolProvider;
use crate::state::AppState;

/// Unified agent runtime - can be either single or multi-agent
pub enum AgentRuntime {
    Single(Box<dyn AgentRunnable + Send + Sync>),
    Multi(MultiAgentOrchestrator),
}

impl AgentRuntime {
    pub async fn run(
        &mut self,
        prompt: String,
        cancel_rx: crate::state::CancelReceiver,
    ) -> Result<String, AgentError> {
        match self {
            AgentRuntime::Single(runner) => runner.run(prompt, cancel_rx).await,
            AgentRuntime::Multi(_orchestrator) => {
                // For multi-agent, we need to implement a similar run interface
                // This is a simplified version - you might need to adapt based on your multi-agent implementation
                Err(AgentError::ConfigurationError("Multi-agent run not yet implemented".to_string()))
            }
        }
    }
}

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

    /// Get description for the provider
    pub fn description(&self) -> &'static str {
        match self {
            Provider::Anthropic => "High-performance AI assistant with advanced reasoning capabilities",
            Provider::OpenAI => "OpenAI's GPT models for conversational AI and text generation",
            Provider::Rig => "Rig framework for building AI agents with structured outputs",
            Provider::Gemini => "Google's Gemini models for multimodal AI capabilities",
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
            Provider::Gemini => vec![
                "gemini-pro".to_string(),
                "gemini-pro-vision".to_string(),
                "gemini-1.5-pro".to_string(),
                "gemini-1.5-flash".to_string(),
            ],
        }
    }

    /// Get default model for the provider
    pub fn default_model(&self) -> &'static str {
        match self {
            Provider::Anthropic => "claude-3-5-sonnet-20241022",
            Provider::OpenAI => "gpt-4o",
            Provider::Rig => "gpt-4o",
            Provider::Gemini => "gemini-1.5-pro",
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

    /// Create a multi-agent orchestrator system
    pub async fn create_multi_agent_orchestrator(
        memory: std::sync::Arc<dyn MemoryManager + Send + Sync>,
        tool_provider: std::sync::Arc<dyn ToolProvider + Send + Sync>,
    ) -> Result<MultiAgentOrchestrator, AgentError> {
        MultiAgentOrchestrator::new(memory, tool_provider).await
    }

    /// Create an agent runtime based on configuration (single or multi-agent)
    pub async fn create_agent_runtime(
        memory: std::sync::Arc<dyn MemoryManager + Send + Sync>,
        tool_provider: std::sync::Arc<dyn ToolProvider + Send + Sync>,
        app_handle: Option<tauri::AppHandle>,
    ) -> Result<AgentRuntime, AgentError> {
        let config = ProviderConfig::load()?;

        match config.get_agent_mode() {
            AgentMode::Single => {
                // Create a single agent runtime
                let brain = Self::create_brain()?;

                                // Convert Arc<dyn ToolProvider> to concrete type if needed
                let local_tool_provider = if let Some(ref handle) = app_handle {
                    let provider = LocalToolProvider::with_app_handle(handle.clone());
                    // Copy tools from the Arc provider to local provider
                    // This is a simplified approach - you might need to adjust based on your implementation
                    provider
                } else {
                    LocalToolProvider::new()
                };

                // Create memory manager - need to extract from Arc
                // This is tricky because we need to move out of Arc
                // For now, create a new one with same type
                let memory_impl = crate::agent::implementations::memory_manager::SimpleMemoryManager::new();

                let runner = DefaultAgentRunner::with_boxed_brain(
                    memory_impl,
                    local_tool_provider,
                    brain,
                    15, // max_steps
                    app_handle.unwrap_or_else(|| panic!("AppHandle required for single agent")),
                );

                Ok(AgentRuntime::Single(Box::new(runner)))
            },
            AgentMode::Multi => {
                // Create multi-agent orchestrator
                let orchestrator = Self::create_multi_agent_orchestrator(memory, tool_provider).await?;
                Ok(AgentRuntime::Multi(orchestrator))
            }
        }
    }

    /// Get current agent mode from configuration
    pub fn get_agent_mode() -> AgentMode {
        let mode_str = env::var("AGENT_MODE").unwrap_or_else(|_| {
            match ProviderConfig::load() {
                Ok(config) => config.get_agent_mode().to_string().to_string(),
                Err(_) => "multi".to_string(), // Default fallback
            }
        });
        AgentMode::from_str(&mode_str).unwrap_or(AgentMode::Multi)
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

    /// Create an AgentBrain implementation with a custom system prompt
    pub fn create_brain_with_system_prompt(system_prompt: String) -> Result<Box<dyn AgentBrain + Send + Sync>, AgentError> {
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
        info!("Attempting to use AI provider: {} with custom system prompt", provider_str);
        apply_provider_settings_to_env()?;

        match Provider::from_str(&provider_str) {
            Some(Provider::Anthropic) => {
                info!("Initializing Anthropic brain with custom system prompt...");
                let api_key = std::env::var("ANTHROPIC_API_KEY")
                    .map_err(|_| AgentError::ConfigurationError("ANTHROPIC_API_KEY environment variable not set".to_string()))?;
                let model = std::env::var("ANTHROPIC_MODEL").ok();
                let max_tokens = std::env::var("ANTHROPIC_MAX_TOKENS").ok().and_then(|s| s.parse::<u32>().ok());

                AnthropicBrain::new(api_key, model, max_tokens, Some(system_prompt))
                    .map(|b| Box::new(b) as Box<dyn AgentBrain + Send + Sync>)
            }
            Some(Provider::OpenAI) => {
                info!("Attempting to initialize OpenAI brain with custom system prompt...");
                // For other providers, we'll need to implement similar custom constructors
                // For now, fall back to Anthropic with the custom prompt
                warn!("Custom system prompts not yet implemented for OpenAI. Falling back to Anthropic.");
                let api_key = std::env::var("ANTHROPIC_API_KEY")
                    .map_err(|_| AgentError::ConfigurationError("ANTHROPIC_API_KEY environment variable not set".to_string()))?;
                let model = std::env::var("ANTHROPIC_MODEL").ok();
                let max_tokens = std::env::var("ANTHROPIC_MAX_TOKENS").ok().and_then(|s| s.parse::<u32>().ok());

                AnthropicBrain::new(api_key, model, max_tokens, Some(system_prompt))
                    .map(|b| Box::new(b) as Box<dyn AgentBrain + Send + Sync>)
            }
            Some(Provider::Rig) => {
                info!("Attempting to initialize Rig brain with custom system prompt...");
                // For now, fall back to Anthropic with the custom prompt
                warn!("Custom system prompts not yet implemented for Rig. Falling back to Anthropic.");
                let api_key = std::env::var("ANTHROPIC_API_KEY")
                    .map_err(|_| AgentError::ConfigurationError("ANTHROPIC_API_KEY environment variable not set".to_string()))?;
                let model = std::env::var("ANTHROPIC_MODEL").ok();
                let max_tokens = std::env::var("ANTHROPIC_MAX_TOKENS").ok().and_then(|s| s.parse::<u32>().ok());

                AnthropicBrain::new(api_key, model, max_tokens, Some(system_prompt))
                    .map(|b| Box::new(b) as Box<dyn AgentBrain + Send + Sync>)
            }
            Some(Provider::Gemini) => {
                info!("Attempting to initialize Gemini brain with custom system prompt...");
                // For now, fall back to Anthropic with the custom prompt
                warn!("Custom system prompts not yet implemented for Gemini. Falling back to Anthropic.");
                let api_key = std::env::var("ANTHROPIC_API_KEY")
                    .map_err(|_| AgentError::ConfigurationError("ANTHROPIC_API_KEY environment variable not set".to_string()))?;
                let model = std::env::var("ANTHROPIC_MODEL").ok();
                let max_tokens = std::env::var("ANTHROPIC_MAX_TOKENS").ok().and_then(|s| s.parse::<u32>().ok());

                AnthropicBrain::new(api_key, model, max_tokens, Some(system_prompt))
                    .map(|b| Box::new(b) as Box<dyn AgentBrain + Send + Sync>)
            }
            None => {
                warn!("Unknown AI provider specified: '{}'. Using Anthropic as fallback.", provider_str);
                let api_key = std::env::var("ANTHROPIC_API_KEY")
                    .map_err(|_| AgentError::ConfigurationError("ANTHROPIC_API_KEY environment variable not set".to_string()))?;
                let model = std::env::var("ANTHROPIC_MODEL").ok();
                let max_tokens = std::env::var("ANTHROPIC_MAX_TOKENS").ok().and_then(|s| s.parse::<u32>().ok());

                AnthropicBrain::new(api_key, model, max_tokens, Some(system_prompt))
                    .map(|b| Box::new(b) as Box<dyn AgentBrain + Send + Sync>)
            }
        }
    }

    /// Register all available computer use tools for the agent
    pub async fn register_computer_use_tools(
        provider: &mut LocalToolProvider,
        app_handle: tauri::AppHandle,
    ) -> Result<(), String> {
        info!("Registering all Computer Use tools...");

        // Get the app state for MCP manager integration
        let state_manager = app_handle.state::<AppState>();

        // Set up MCP manager in the tool provider
        let mcp_manager = state_manager.get_mcp_manager().await;
        provider.set_mcp_manager(mcp_manager);

        // Register the official Anthropic Computer Use tools
        register_anthropic_computer_use_tools(provider, app_handle.clone()).await?;

        // Register additional desktop automation tools (your existing ones)
        crate::agent::tools::desktop_tools::register_desktop_tools(provider, state_manager.clone(), app_handle.clone()).await;

        // Register timer tools for agent task scheduling and resumption
        crate::agent::tools::timer_tools::register_timer_tools(provider, app_handle.clone()).await;

        // Initialize MCP servers and sync tools
        if let Err(e) = state_manager.initialize_mcp_servers().await {
            warn!("Failed to initialize MCP servers: {}", e);
        } else {
            info!("MCP servers initialized successfully");
        }

        // Refresh MCP tools to include them in the provider
        if let Err(e) = provider.refresh_mcp_tools().await {
            warn!("Failed to refresh MCP tools: {}", e);
        } else {
            info!("MCP tools refreshed and available");
        }

        // Sync MCP tools with configuration
        if let Err(e) = state_manager.sync_mcp_tools().await {
            warn!("Failed to sync MCP tools with configuration: {}", e);
        }

        info!("All Computer Use tools registered successfully (including MCP tools)");
        Ok(())
    }
}

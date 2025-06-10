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

// Model ID Constants - Single source of truth
mod model_ids {
    // Anthropic Claude Models
    pub const CLAUDE_4_OPUS: &str = "claude-opus-4-20250514";
    pub const CLAUDE_4_SONNET: &str = "claude-sonnet-4-20250514";
    pub const CLAUDE_3_7_SONNET: &str = "claude-3-7-sonnet-20250219";
    pub const CLAUDE_3_5_SONNET: &str = "claude-3-5-sonnet-20241022";
    pub const CLAUDE_3_5_HAIKU: &str = "claude-3-5-haiku-20241022";
    pub const CLAUDE_3_OPUS: &str = "claude-3-opus-20240229";

    // OpenAI Models
    pub const OPENAI_CUA: &str = "computer-use-preview";
    pub const GPT_4O: &str = "gpt-4o";
    pub const GPT_4O_MINI: &str = "gpt-4o-mini";
    pub const GPT_4_TURBO: &str = "gpt-4-turbo";
    pub const GPT_3_5_TURBO: &str = "gpt-3.5-turbo";

    // Google Gemini Models
    pub const GEMINI_1_5_PRO: &str = "gemini-1.5-pro";
    pub const GEMINI_1_5_FLASH: &str = "gemini-1.5-flash";
    pub const GEMINI_PRO: &str = "gemini-pro";
    pub const GEMINI_PRO_VISION: &str = "gemini-pro-vision";
}

/// Unified agent runtime - can be either single or multi-agent
pub enum AgentRuntime {
    Single(Box<dyn AgentRunnable + Send + Sync>),
    Multi(MultiAgentOrchestrator),
}

/// Model categories based on capabilities
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub enum ModelCategory {
    ComputerUse,   // Models that support computer automation
    GeneralChat,   // Models for general conversation and text generation
}

/// Model definition with all metadata
#[derive(Debug, Clone)]
pub struct ModelDefinition {
    pub id: &'static str,
    pub name: &'static str,
    pub category: ModelCategory,
    pub supports_computer_use: bool,
    pub is_recommended: bool,
}

/// Model information for serialization (UI display)
#[derive(Debug, Clone, serde::Serialize)]
pub struct ModelInfo {
    pub id: String,
    pub name: String,
    pub category: ModelCategory,
    pub supports_computer_use: bool,
    pub is_recommended: bool,
}

impl From<&ModelDefinition> for ModelInfo {
    fn from(def: &ModelDefinition) -> Self {
        ModelInfo {
            id: def.id.to_string(),
            name: def.name.to_string(),
            category: def.category.clone(),
            supports_computer_use: def.supports_computer_use,
            is_recommended: def.is_recommended,
        }
    }
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

    /// Get model definitions for the provider
    pub fn model_definitions(&self) -> &'static [ModelDefinition] {
        match self {
            Provider::Anthropic => &[
                ModelDefinition {
                    id: model_ids::CLAUDE_4_SONNET,
                    name: "Claude 4 Sonnet",
                    category: ModelCategory::ComputerUse,
                    supports_computer_use: true,
                    is_recommended: true,
                },
                ModelDefinition {
                    id: model_ids::CLAUDE_4_OPUS,
                    name: "Claude 4 Opus",
                    category: ModelCategory::ComputerUse,
                    supports_computer_use: true,
                    is_recommended: true,
                },
                ModelDefinition {
                    id: model_ids::CLAUDE_3_7_SONNET,
                    name: "Claude 3.7 Sonnet",
                    category: ModelCategory::ComputerUse,
                    supports_computer_use: true,
                    is_recommended: true,
                },
                ModelDefinition {
                    id: model_ids::CLAUDE_3_5_SONNET,
                    name: "Claude 3.5 Sonnet",
                    category: ModelCategory::ComputerUse,
                    supports_computer_use: true,
                    is_recommended: false,
                },
                ModelDefinition {
                    id: model_ids::CLAUDE_3_5_HAIKU,
                    name: "Claude 3.5 Haiku",
                    category: ModelCategory::ComputerUse,
                    supports_computer_use: true,
                    is_recommended: false,
                },
                ModelDefinition {
                    id: model_ids::CLAUDE_3_OPUS,
                    name: "Claude 3 Opus (Legacy)",
                    category: ModelCategory::ComputerUse,
                    supports_computer_use: true,
                    is_recommended: false,
                },
            ],
            Provider::OpenAI => &[
                ModelDefinition {
                    id: model_ids::OPENAI_CUA,
                    name: "Computer-Using Agent (CUA)",
                    category: ModelCategory::ComputerUse,
                    supports_computer_use: true,
                    is_recommended: true,
                },
                ModelDefinition {
                    id: model_ids::GPT_4O,
                    name: "GPT-4o",
                    category: ModelCategory::GeneralChat,
                    supports_computer_use: false,
                    is_recommended: false,
                },
                ModelDefinition {
                    id: model_ids::GPT_4O_MINI,
                    name: "GPT-4o Mini",
                    category: ModelCategory::GeneralChat,
                    supports_computer_use: false,
                    is_recommended: false,
                },
                ModelDefinition {
                    id: model_ids::GPT_4_TURBO,
                    name: "GPT-4 Turbo",
                    category: ModelCategory::GeneralChat,
                    supports_computer_use: false,
                    is_recommended: false,
                },
                ModelDefinition {
                    id: model_ids::GPT_3_5_TURBO,
                    name: "GPT-3.5 Turbo",
                    category: ModelCategory::GeneralChat,
                    supports_computer_use: false,
                    is_recommended: false,
                },
            ],
            Provider::Rig => &[
                ModelDefinition {
                    id: model_ids::CLAUDE_3_5_SONNET,
                    name: "Claude 3.5 Sonnet (Rig)",
                    category: ModelCategory::ComputerUse,
                    supports_computer_use: true,
                    is_recommended: true,
                },
                ModelDefinition {
                    id: model_ids::GPT_4O,
                    name: "GPT-4o (Rig)",
                    category: ModelCategory::GeneralChat,
                    supports_computer_use: false,
                    is_recommended: false,
                },
                ModelDefinition {
                    id: model_ids::GPT_4O_MINI,
                    name: "GPT-4o Mini (Rig)",
                    category: ModelCategory::GeneralChat,
                    supports_computer_use: false,
                    is_recommended: false,
                },
            ],
            Provider::Gemini => &[
                ModelDefinition {
                    id: model_ids::GEMINI_1_5_PRO,
                    name: "Gemini 1.5 Pro",
                    category: ModelCategory::GeneralChat,
                    supports_computer_use: false,
                    is_recommended: true,
                },
                ModelDefinition {
                    id: model_ids::GEMINI_1_5_FLASH,
                    name: "Gemini 1.5 Flash",
                    category: ModelCategory::GeneralChat,
                    supports_computer_use: false,
                    is_recommended: false,
                },
                ModelDefinition {
                    id: model_ids::GEMINI_PRO,
                    name: "Gemini Pro",
                    category: ModelCategory::GeneralChat,
                    supports_computer_use: false,
                    is_recommended: false,
                },
                ModelDefinition {
                    id: model_ids::GEMINI_PRO_VISION,
                    name: "Gemini Pro Vision",
                    category: ModelCategory::GeneralChat,
                    supports_computer_use: false,
                    is_recommended: false,
                },
            ],
        }
    }

    /// Get available models for the provider (derived from model definitions)
    pub fn models(&self) -> Vec<String> {
        self.model_definitions()
            .iter()
            .map(|def| def.id.to_string())
            .collect()
    }

    /// Check if a model supports computer use capabilities
    pub fn model_supports_computer_use(&self, model: &str) -> bool {
        self.model_definitions()
            .iter()
            .find(|def| def.id == model)
            .map(|def| def.supports_computer_use)
            .unwrap_or(false)
    }

    /// Get model category (ComputerUse or GeneralChat)
    pub fn get_model_category(&self, model: &str) -> ModelCategory {
        self.model_definitions()
            .iter()
            .find(|def| def.id == model)
            .map(|def| def.category.clone())
            .unwrap_or(ModelCategory::GeneralChat)
    }

    /// Get default model for the provider
    pub fn default_model(&self) -> &'static str {
        // Find the first recommended model, or fallback to the first model
        self.model_definitions()
            .iter()
            .find(|def| def.is_recommended)
            .or_else(|| self.model_definitions().first())
            .map(|def| def.id)
            .unwrap_or_else(|| {
                // Fallback constants if no definitions exist (shouldn't happen)
                match self {
                    Provider::Anthropic => model_ids::CLAUDE_3_7_SONNET,
                    Provider::OpenAI => model_ids::OPENAI_CUA,
                    Provider::Rig => model_ids::GPT_4O,
                    Provider::Gemini => model_ids::GEMINI_1_5_PRO,
                }
            })
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

    /// Get detailed model information with capabilities (derived from model definitions)
    pub fn get_model_info(&self) -> Vec<ModelInfo> {
        self.model_definitions()
            .iter()
            .map(ModelInfo::from)
            .collect()
    }

    /// Check if provider supports computer use capabilities
    pub fn supports_computer_use(&self) -> bool {
        self.model_definitions()
            .iter()
            .any(|def| def.supports_computer_use)
    }
}

/// Struct containing provider information for UI display
#[derive(Debug, Clone, serde::Serialize)]
pub struct ProviderInfo {
    pub id: String,
    pub name: String,
    pub description: String,
    pub models: Vec<String>,
    pub model_info: Vec<ModelInfo>, // Enhanced model information
    pub default_model: String,
    pub is_available: bool,
    pub is_default: bool,
    pub computer_use_supported: bool, // Whether this provider supports computer use at all
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
                model_info: provider.get_model_info(),
                default_model: provider.default_model().to_string(),
                is_available,
                is_default: provider == current_provider,
                computer_use_supported: provider.supports_computer_use(),
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

        // Register self-awareness and introspection tools (development mode only)
        crate::agent::tools::register_self_awareness_tools(provider).await;

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

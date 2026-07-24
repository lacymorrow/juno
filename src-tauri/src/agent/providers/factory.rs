use std::env;
use tauri::Manager;
use tracing::{info, warn};

use crate::agent::core::AgentError;
use crate::agent::implementations::agent_runner::DefaultAgentRunner;
use crate::agent::implementations::tool_provider::LocalToolProvider;
use crate::agent::multi_agent::MultiAgentOrchestrator;
use crate::agent::providers::anthropic::AnthropicBrain;
use crate::agent::providers::claude_cli::ClaudeCliBrain;
use crate::agent::providers::config::{load_provider_config, AgentMode, ProviderConfig};
use crate::agent::providers::gemini::GeminiBrain;
use crate::agent::providers::openai::OpenAIBrain;
use crate::agent::providers::rig::RigBrain;
use crate::agent::tools::accessibility_tools::AccessibilityTools;
use crate::agent::tools::anthropic_computer_use::register_anthropic_computer_use_tools;
use crate::agent::traits::{AgentBrain, AgentRunnable, MemoryManager, ToolProvider};
use crate::state::AppState;

use super::types::{ModelInfo, Provider};

/// Unified agent runtime - can be either single or multi-agent
#[allow(clippy::large_enum_variant)]
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
                Err(AgentError::ConfigurationError(
                    "Multi-agent run not yet implemented".to_string(),
                ))
            }
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
    pub model_info: Vec<ModelInfo>, // Enhanced model information
    pub default_model: String,
    pub is_available: bool,
    pub is_default: bool,
    pub computer_use_supported: bool, // Whether this provider supports computer use at all
}

/// Factory for creating provider-specific AgentBrain implementations
pub struct BrainFactory;

impl BrainFactory {
    /// Warm up provider configuration by loading it once.
    /// Pass an AppHandle to read the user's saved API keys from the Tauri Store.
    /// Pass None for early startup before the Tauri app is initialized.
    /// Note: This always succeeds — load_provider_config falls back to defaults.
    /// Actual validation happens in create_brain() when a provider is first used.
    pub fn init(app_handle: Option<&tauri::AppHandle>) {
        let _config = load_provider_config(app_handle);
    }

    /// Create a multi-agent orchestrator system
    pub async fn create_multi_agent_orchestrator(
        memory: std::sync::Arc<dyn MemoryManager + Send + Sync>,
        tool_provider: std::sync::Arc<dyn ToolProvider + Send + Sync>,
        app_handle: Option<&tauri::AppHandle>,
    ) -> Result<MultiAgentOrchestrator, AgentError> {
        MultiAgentOrchestrator::new(memory, tool_provider, app_handle).await
    }

    /// Create an agent runtime based on configuration (single or multi-agent)
    pub async fn create_agent_runtime(
        memory: std::sync::Arc<dyn MemoryManager + Send + Sync>,
        tool_provider: std::sync::Arc<dyn ToolProvider + Send + Sync>,
        app_handle: Option<tauri::AppHandle>,
    ) -> Result<AgentRuntime, AgentError> {
        let config = ProviderConfig::default(); // Use default config when no app_handle available

        match config.get_agent_mode() {
            AgentMode::Single => {
                // Create a single agent runtime
                let brain = Self::create_brain().await?;

                // Convert Arc<dyn ToolProvider> to concrete type if needed
                let local_tool_provider = if let Some(ref handle) = app_handle {
                    // Copy tools from the Arc provider to local provider
                    // This is a simplified approach - you might need to adjust based on your implementation
                    LocalToolProvider::with_app_handle(handle.clone())
                } else {
                    LocalToolProvider::new()
                };

                // Create memory manager - need to extract from Arc
                // This is tricky because we need to move out of Arc
                // For now, create a new one with same type
                let memory_impl =
                    crate::agent::implementations::memory_manager::AdvancedMemoryManager::new();

                let runner = DefaultAgentRunner::with_boxed_brain(
                    memory_impl,
                    local_tool_provider,
                    brain,
                    crate::constants::agent::config::MAX_ITERATIONS, // max_steps
                    app_handle.ok_or("AppHandle required for single agent")?,
                );

                Ok(AgentRuntime::Single(Box::new(runner)))
            }
            AgentMode::Multi => {
                // Create multi-agent orchestrator
                let orchestrator = Self::create_multi_agent_orchestrator(
                    memory,
                    tool_provider,
                    app_handle.as_ref(),
                )
                .await?;
                Ok(AgentRuntime::Multi(orchestrator))
            }
        }
    }

    /// Get current agent mode from configuration
    /// Get current agent mode from centralized settings with app handle
    pub async fn get_agent_mode_with_app_handle(app_handle: &tauri::AppHandle) -> AgentMode {
        // Use direct store access to avoid deadlocks during agent execution
        use tauri_plugin_store::StoreExt;

        const SETTINGS_STORE_FILE: &str = "app_settings.json";

        match app_handle.store(SETTINGS_STORE_FILE) {
            Ok(store) => {
                // Access nested agent settings structure: { "agent": { "execution_mode": "..." } }
                match store.get("agent") {
                    Some(agent_value) => {
                        if let Some(agent_obj) = agent_value.as_object() {
                            if let Some(execution_mode_value) = agent_obj.get("execution_mode") {
                                if let Some(mode_str) = execution_mode_value.as_str() {
                                    let mode = AgentMode::from_str(mode_str)
                                        .unwrap_or_else(|| {
                                            warn!("Invalid agent execution mode in settings: '{}'. Using default.", mode_str);
                                            AgentMode::Multi
                                        });
                                    info!(
                                        "Loaded agent mode from centralized settings: {:?}",
                                        mode
                                    );
                                    return mode;
                                } else {
                                    warn!("Agent execution mode is not a string in settings. Using default.");
                                }
                            } else {
                                info!("Agent execution mode not found in agent settings object. Using default.");
                            }
                        } else {
                            warn!(
                                "Agent settings is not an object in settings store. Using default."
                            );
                        }
                    }
                    None => {
                        info!("Agent settings not found in settings store. Using default.");
                    }
                }
                AgentMode::Multi
            }
            Err(e) => {
                warn!("Failed to access settings store for agent mode: {}. Using environment fallback.", e);
                Self::get_agent_mode_fallback()
            }
        }
    }

    /// Fallback method that reads from environment (used when centralized settings unavailable)
    fn get_agent_mode_fallback() -> AgentMode {
        let mode_str = env::var("AGENT_MODE").unwrap_or_else(|_| {
            "multi".to_string() // Default to multi-agent mode for new app
        });
        AgentMode::from_str(&mode_str).unwrap_or(AgentMode::Multi)
    }

    /// Get current agent mode from configuration (legacy method - now tries to use centralized settings)
    pub fn get_agent_mode() -> AgentMode {
        // This method is called from contexts where we don't have an app handle
        // Fall back to environment variable reading for backward compatibility
        // But log a warning to encourage migration to the new method
        warn!("get_agent_mode() called without app handle - using environment fallback. Consider using get_agent_mode_with_app_handle() for proper settings integration.");
        Self::get_agent_mode_fallback()
    }

    /// Get the current provider from configuration or environment
    pub fn get_current_provider() -> Provider {
        let provider_str = env::var("AI_PROVIDER")
            .unwrap_or_else(|_| super::config::DEFAULT_PROVIDER.id().to_string());
        Provider::from_str(&provider_str).unwrap_or(Provider::Anthropic)
    }

    /// Get list of all available providers with their status
    pub fn list_providers() -> Vec<ProviderInfo> {
        Self::list_providers_with_app_handle(None)
    }

    /// List providers with optional app handle to read store-saved API keys.
    /// Without an app handle, only env vars are checked for availability.
    pub fn list_providers_with_app_handle(
        app_handle: Option<&tauri::AppHandle>,
    ) -> Vec<ProviderInfo> {
        let current_provider = Self::get_current_provider();
        let providers = vec![
            Provider::Anthropic,
            Provider::OpenAI,
            Provider::Rig,
            Provider::Gemini,
            Provider::ClaudeCli,
        ];
        let config = Some(load_provider_config(app_handle));

        providers
            .into_iter()
            .map(|provider| {
                let provider_id = provider.id();
                let is_available = match provider {
                    Provider::Anthropic => {
                        env::var("ANTHROPIC_API_KEY").is_ok_and(|v| !v.is_empty())
                            || config
                                .as_ref()
                                .and_then(|c| c.get_provider_settings(provider_id))
                                .and_then(|s| s.api_key.as_ref())
                                .is_some_and(|k| !k.is_empty())
                    }
                    Provider::OpenAI => {
                        env::var("OPENAI_API_KEY").is_ok_and(|v| !v.is_empty())
                            || config
                                .as_ref()
                                .and_then(|c| c.get_provider_settings(provider_id))
                                .and_then(|s| s.api_key.as_ref())
                                .is_some_and(|k| !k.is_empty())
                    }
                    Provider::Rig => {
                        env::var("OPENAI_API_KEY").is_ok_and(|v| !v.is_empty())
                            || config
                                .as_ref()
                                .and_then(|c| c.get_provider_settings(Provider::OpenAI.id()))
                                .and_then(|s| s.api_key.as_ref())
                                .is_some_and(|k| !k.is_empty())
                            || config
                                .as_ref()
                                .and_then(|c| c.get_provider_settings(provider_id))
                                .and_then(|s| s.api_key.as_ref())
                                .is_some_and(|k| !k.is_empty())
                    }
                    Provider::Gemini => {
                        env::var("GEMINI_API_KEY").is_ok_and(|v| !v.is_empty())
                            || config
                                .as_ref()
                                .and_then(|c| c.get_provider_settings(provider_id))
                                .and_then(|s| s.api_key.as_ref())
                                .is_some_and(|k| !k.is_empty())
                    }
                    Provider::ClaudeCli => {
                        // Claude CLI availability = binary exists on PATH (fast check)
                        crate::agent::providers::claude_cli::is_claude_cli_available()
                    }
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
            })
            .collect()
    }

    /// Create an AgentBrain implementation based on provider configuration
    pub async fn create_brain() -> Result<Box<dyn AgentBrain + Send + Sync>, AgentError> {
        Self::create_brain_with_app_handle(None).await
    }

    /// Create an AgentBrain implementation with app handle for proper prompt loading.
    /// Configuration flows: Store → ProviderConfig struct → from_config() constructor.
    /// No env::set_var() calls — reading env vars is safe and used only as fallback for api_key.
    pub async fn create_brain_with_app_handle(
        app_handle: Option<&tauri::AppHandle>,
    ) -> Result<Box<dyn AgentBrain + Send + Sync>, AgentError> {
        let config = load_provider_config(app_handle);

        // Determine active provider: AI_PROVIDER env var overrides stored config
        let provider_id_str =
            env::var("AI_PROVIDER").unwrap_or_else(|_| config.active_provider.clone());
        let provider = Provider::from_str(&provider_id_str).ok_or_else(|| {
            AgentError::ConfigurationError(format!("Unknown provider: '{}'", provider_id_str))
        })?;
        info!("Attempting to use AI provider: {}", provider.id());

        let mut provider_config = config.resolve_provider(provider.clone()).ok_or_else(|| {
            AgentError::ConfigurationError(format!(
                "Provider '{}' not found in config",
                provider.id()
            ))
        })?;

        // Load system prompt from prompt manager if app_handle is available
        let system_prompt = if let Some(handle) = app_handle {
            match crate::settings::manager::SettingsManager::new(handle.clone()) {
                Ok(settings_manager) => {
                    let prompt_manager = crate::agent::prompts::PromptManager::load_from_centralized_settings(&settings_manager).await.unwrap_or_else(|e| {
                        warn!("Failed to load prompt configuration from centralized settings: {}. Using defaults.", e);
                        crate::agent::prompts::PromptManager::new()
                    });
                    Some(prompt_manager.get_default_system_prompt())
                }
                Err(e) => {
                    warn!(
                        "Failed to create settings manager: {}. Using default prompt.",
                        e
                    );
                    let prompt_manager = crate::agent::prompts::PromptManager::new();
                    Some(prompt_manager.get_default_system_prompt())
                }
            }
        } else {
            let prompt_manager = crate::agent::prompts::PromptManager::new();
            Some(prompt_manager.get_default_system_prompt())
        };

        // Use PromptManager default only if the user hasn't configured a custom system prompt
        if provider_config.system_prompt.is_none() {
            if let Some(sp) = system_prompt {
                provider_config.system_prompt = Some(sp);
            }
        }

        // Publish model name so the screenshot pipeline can pick the right
        // resolution tier (Opus 4.5+ → up to 2576px, legacy → XGA/WXGA/FWXGA).
        if let Some(ref model) = provider_config.model {
            crate::utils::coordinates::set_current_model(model);
        }

        match provider {
            Provider::Anthropic => {
                info!("Initializing Anthropic brain with system prompt...");
                AnthropicBrain::from_config(&provider_config)
                    .map(|b| Box::new(b) as Box<dyn AgentBrain + Send + Sync>)
            }
            Provider::OpenAI => {
                info!("Initializing OpenAI brain...");
                OpenAIBrain::from_config(&provider_config)
                    .map(|b| Box::new(b) as Box<dyn AgentBrain + Send + Sync>)
            }
            Provider::Rig => {
                info!("Initializing Rig brain...");
                RigBrain::from_config(&provider_config)
                    .map(|b| Box::new(b) as Box<dyn AgentBrain + Send + Sync>)
            }
            Provider::Gemini => {
                info!("Initializing Gemini brain...");
                GeminiBrain::from_config(&provider_config)
                    .map(|b| Box::new(b) as Box<dyn AgentBrain + Send + Sync>)
            }
            Provider::ClaudeCli => {
                info!("Initializing Claude CLI brain (subprocess-based, no API key)...");
                ClaudeCliBrain::from_config(&provider_config)
                    .map(|b| Box::new(b) as Box<dyn AgentBrain + Send + Sync>)
            }
        }
    }

    /// Create an AgentBrain implementation with a custom system prompt.
    /// When `app_handle` is provided, reads the user's API keys from the Tauri Store.
    /// Without it, falls back to environment variables only.
    pub fn create_brain_with_system_prompt(
        system_prompt: String,
        app_handle: Option<&tauri::AppHandle>,
    ) -> Result<Box<dyn AgentBrain + Send + Sync>, AgentError> {
        let config = load_provider_config(app_handle);

        let provider_id_str =
            env::var("AI_PROVIDER").unwrap_or_else(|_| config.active_provider.clone());
        let provider = Provider::from_str(&provider_id_str).ok_or_else(|| {
            AgentError::ConfigurationError(format!("Unknown provider: '{}'", provider_id_str))
        })?;
        info!(
            "Attempting to use AI provider: {} with custom system prompt",
            provider.id()
        );

        let mut provider_config = config.resolve_provider(provider.clone()).ok_or_else(|| {
            AgentError::ConfigurationError(format!(
                "Provider '{}' not found in config",
                provider.id()
            ))
        })?;

        // Override with custom system prompt
        provider_config.system_prompt = Some(system_prompt);

        // Publish model name so the screenshot pipeline can pick the right
        // resolution tier (Opus 4.5+ → up to 2576px, legacy → XGA/WXGA/FWXGA).
        if let Some(ref model) = provider_config.model {
            crate::utils::coordinates::set_current_model(model);
        }

        match provider {
            Provider::Anthropic => {
                info!("Initializing Anthropic brain with custom system prompt...");
                AnthropicBrain::from_config(&provider_config)
                    .map(|b| Box::new(b) as Box<dyn AgentBrain + Send + Sync>)
            }
            Provider::OpenAI => {
                info!("Initializing OpenAI brain with custom system prompt...");
                OpenAIBrain::from_config(&provider_config)
                    .map(|b| Box::new(b) as Box<dyn AgentBrain + Send + Sync>)
            }
            Provider::Rig => {
                info!("Initializing Rig brain with custom system prompt...");
                RigBrain::from_config(&provider_config)
                    .map(|b| Box::new(b) as Box<dyn AgentBrain + Send + Sync>)
            }
            Provider::Gemini => {
                info!("Initializing Gemini brain with custom system prompt...");
                GeminiBrain::from_config(&provider_config)
                    .map(|b| Box::new(b) as Box<dyn AgentBrain + Send + Sync>)
            }
            Provider::ClaudeCli => {
                info!("Initializing Claude CLI brain with custom system prompt...");
                ClaudeCliBrain::from_config(&provider_config)
                    .map(|b| Box::new(b) as Box<dyn AgentBrain + Send + Sync>)
            }
        }
    }

    /// Register all available computer use tools for the agent
    pub async fn register_computer_use_tools(
        provider: &mut LocalToolProvider,
        app_handle: tauri::AppHandle,
    ) -> Result<(), String> {
        info!("🔧 Registering Computer Use tools (race-condition safe)...");

        // Use a global mutex to ensure that only one thread can register tools at a time
        // This prevents race conditions where multiple threads try to register the same tools simultaneously
        use std::sync::Arc;
        use tokio::sync::Mutex;

        lazy_static::lazy_static! {
            static ref TOOL_REGISTRATION_MUTEX: Arc<Mutex<()>> = Arc::new(Mutex::new(()));
        }

        // Acquire the lock to ensure exclusive access to tool registration
        let _lock = TOOL_REGISTRATION_MUTEX.lock().await;

        info!("🔧 Acquired tool registration lock, proceeding with registration...");

        // Get the app state for MCP manager integration
        let state_manager = app_handle.state::<AppState>();

        // Set up MCP manager in the tool provider (per-provider instance)
        let mcp_manager = state_manager.get_mcp_manager().await;
        provider.set_mcp_manager(mcp_manager);

        // Register the official Anthropic Computer Use tools (per-provider instance)
        register_anthropic_computer_use_tools(provider, app_handle.clone()).await?;

        // Register additional desktop automation tools (per-provider instance)
        crate::agent::tools::desktop_tools::register_desktop_tools(
            provider,
            state_manager.clone(),
            app_handle.clone(),
        )
        .await;

        // Register display information tools for screen resolution and display info (per-provider instance)
        crate::agent::tools::display_info_tools::register_display_info_tools(
            provider,
            app_handle.clone(),
        )
        .await?;

        // Register lightweight window listing tool (per-provider instance)
        crate::agent::tools::visible_windows::register_visible_windows_tools(
            provider,
            app_handle.clone(),
        )
        .await?;

        // Register timer tools for agent task scheduling and resumption (per-provider instance)
        crate::agent::tools::timer_tools::register_timer_tools(provider, app_handle.clone()).await;

        // Register scheduled automation tools for user-facing recurring tasks (per-provider instance)
        crate::agent::tools::schedule_tools::register_schedule_tools(provider, app_handle.clone())
            .await;

        // Register self-awareness and introspection tools (per-provider instance, development mode only)
        crate::agent::tools::register_self_awareness_tools(provider).await;

        // Register native accessibility tools for element-level interaction
        Self::register_accessibility_tools(provider, app_handle.clone()).await?;

        // Register Safari tools for fast Safari DOM automation
        Self::register_safari_tools(provider, app_handle.clone()).await?;

        // MCP tools are handled separately and loaded only when needed:
        // 1. At app startup (state_management.rs)
        // 2. When MCP configuration changes (via commands/mcp.rs)
        // 3. When explicitly refreshed by user action

        // Simply refresh MCP tools from cache (fast operation if already loaded)
        if let Err(e) = provider.refresh_mcp_tools().await {
            warn!("Failed to refresh MCP tools from cache: {}", e);
        } else {
            info!("MCP tools refreshed from cache (no network calls)");
        }

        info!("✅ Computer Use tools registered successfully for provider instance");

        // Lock is automatically released when _lock goes out of scope
        Ok(())
    }

    /// Register native accessibility tools for element-level interaction
    pub async fn register_accessibility_tools(
        provider: &mut LocalToolProvider,
        app_handle: tauri::AppHandle,
    ) -> Result<(), String> {
        info!("🔧 Registering native accessibility tools...");

        // Create accessibility tools instance
        let accessibility_tools = AccessibilityTools::new();

        // Get tool definitions
        let tool_definitions = AccessibilityTools::get_tool_definitions();

        for tool_def in tool_definitions {
            let tool_name = tool_def["name"].as_str().unwrap_or("unknown").to_string();
            let description = tool_def["description"].as_str().unwrap_or("").to_string();
            let input_schema = tool_def["input_schema"].clone();

            info!("🔧 Registering accessibility tool: {}", tool_name);

            // Create tool definition for the provider
            let tool_definition = crate::agent::core::ToolDefinition {
                name: tool_name.clone(),
                description,
                input_schema,
                api_type: None,
                beta_flag: None,
            };

            // Create tool executor
            let tools_clone = accessibility_tools.clone();
            let app_handle_clone = app_handle.clone();
            let tool_name_clone = tool_name.clone();

            let executor = move |input: serde_json::Value| {
                let tools = tools_clone.clone();
                let app = app_handle_clone.clone();
                let name = tool_name_clone.clone();

                async move { tools.execute_tool(&name, &input, &app).await }
            };

            // Register the tool
            provider
                .register_async_tool(tool_definition, executor)
                .await;
        }

        info!("🔧 Native accessibility tools registered successfully");
        Ok(())
    }

    /// Register Safari tools for fast Safari DOM automation
    pub async fn register_safari_tools(
        provider: &mut LocalToolProvider,
        app_handle: tauri::AppHandle,
    ) -> Result<(), String> {
        info!("🔧 Registering Safari tools for DOM automation...");

        // Get Safari tool definitions
        use crate::agent::tools::safari_tools::get_safari_tool_definitions;
        let tool_definitions = get_safari_tool_definitions();

        for tool_def in tool_definitions {
            let tool_name = tool_def.name.clone();

            info!("🔧 Registering Safari tool: {}", tool_name);

            // Create tool executor
            let app_handle_clone = app_handle.clone();
            let tool_name_clone = tool_name.clone();

            let executor = move |input: serde_json::Value| {
                let _app = app_handle_clone.clone();
                let name = tool_name_clone.clone();

                async move {
                    use crate::commands::safari_tools::execute_safari_tool;
                    match execute_safari_tool(name, input).await {
                        Ok(tool_result) => Ok(tool_result.output),
                        Err(e) => Err(e),
                    }
                }
            };

            // Register the tool
            provider.register_async_tool(tool_def, executor).await;
        }

        info!("🔧 Safari tools registered successfully");
        Ok(())
    }
}

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Types of prompts in the system
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum PromptType {
    /// Main system prompt for single agent mode
    SystemDefault,
    /// Development-only self-aware system prompt
    SystemDefaultDevelopment,
    /// Orchestrator personality prompt for multi-agent mode
    OrchestratorPersonality,
    /// Expert agent prompts (unified system)
    BrowserExpert,
    CodingExpert,
    DesktopExpert,
    GeneralExpert,
    FileExpert,
    /// Provider-specific prompts
    AnthropicDefault,
    OpenAIDefault,
    GeminiDefault,
    RigDefault,
}

impl PromptType {
    pub fn as_str(&self) -> &'static str {
        match self {
            PromptType::SystemDefault => "system_default",
            PromptType::SystemDefaultDevelopment => "system_default_development",
            PromptType::OrchestratorPersonality => "orchestrator_personality",
            PromptType::BrowserExpert => "browser_expert",
            PromptType::CodingExpert => "coding_expert",
            PromptType::DesktopExpert => "desktop_expert",
            PromptType::GeneralExpert => "general_expert",
            PromptType::FileExpert => "file_expert",
            PromptType::AnthropicDefault => "anthropic_default",
            PromptType::OpenAIDefault => "openai_default",
            PromptType::GeminiDefault => "gemini_default",
            PromptType::RigDefault => "rig_default",
        }
    }

    #[allow(clippy::should_implement_trait)]
    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "system_default" => Some(PromptType::SystemDefault),
            "system_default_development" => Some(PromptType::SystemDefaultDevelopment),
            "orchestrator_personality" => Some(PromptType::OrchestratorPersonality),
            "browser_expert" => Some(PromptType::BrowserExpert),
            "coding_expert" => Some(PromptType::CodingExpert),
            "desktop_expert" => Some(PromptType::DesktopExpert),
            "general_expert" => Some(PromptType::GeneralExpert),
            "file_expert" => Some(PromptType::FileExpert),
            "anthropic_default" => Some(PromptType::AnthropicDefault),
            "openai_default" => Some(PromptType::OpenAIDefault),
            "gemini_default" => Some(PromptType::GeminiDefault),
            "rig_default" => Some(PromptType::RigDefault),
            _ => None,
        }
    }
}

/// Configuration for a specific prompt template
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PromptTemplate {
    /// Unique identifier for the prompt
    pub id: String,
    /// Human-readable name
    pub name: String,
    /// Description of the prompt's purpose
    pub description: String,
    /// The actual prompt content with possible placeholders
    pub content: String,
    /// Variables that can be substituted in the content
    pub variables: Vec<String>,
    /// Tags for categorization and filtering
    pub tags: Vec<String>,
    /// Version for tracking changes
    pub version: String,
    /// Whether this prompt is user-customizable
    pub customizable: bool,
}

/// Context information for prompt generation
#[derive(Debug, Clone, Default)]
pub struct PromptContext {
    /// User preferences (from system analysis)
    pub user_preferences: Option<HashMap<String, String>>,
    /// Current task context
    pub task_context: Option<String>,
    /// Available tools for the agent
    pub available_tools: Vec<String>,
    /// Available MCP tools (subset of available_tools)
    pub available_mcp_tools: Vec<String>,
    /// Provider-specific constraints
    pub provider_constraints: Option<HashMap<String, String>>,
    /// Custom variables from configuration
    pub custom_variables: HashMap<String, String>,
}

/// Configuration for prompt management
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PromptConfig {
    /// Active prompt templates by type
    pub active_prompts: HashMap<String, String>,
    /// Custom prompt overrides
    pub custom_prompts: HashMap<String, PromptTemplate>,
    /// Global variables available to all prompts
    pub global_variables: HashMap<String, String>,
    /// Whether to enable prompt customization in UI
    pub allow_customization: bool,
}

impl Default for PromptConfig {
    fn default() -> Self {
        Self {
            active_prompts: HashMap::new(),
            custom_prompts: HashMap::new(),
            global_variables: HashMap::new(),
            allow_customization: true,
        }
    }
}

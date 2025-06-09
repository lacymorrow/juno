use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::Mutex;
use tracing::{info, debug, warn, error};
use uuid;

use crate::agent::structs::{
    AgentAction, AgentError, Message, Role, ToolDefinition,
};
use crate::agent::traits::{AgentBrain, MemoryManager, ToolProvider};
use crate::agent::providers::gemini::GeminiBrain;
use crate::agent::providers::anthropic::AnthropicBrain;
use crate::agent::providers::openai::OpenAIBrain;
use crate::state::CancelReceiver;
use crate::agent::prompts::PromptManager;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum AgentType {
    Orchestrator,
    BrowserExpert,
    CodingExpert,
    DesktopExpert,
    GeneralExpert,
}

impl AgentType {
    pub fn get_name(&self) -> &'static str {
        match self {
            AgentType::Orchestrator => "orchestrator",
            AgentType::BrowserExpert => "browser_expert",
            AgentType::CodingExpert => "coding_expert",
            AgentType::DesktopExpert => "desktop_expert",
            AgentType::GeneralExpert => "general_expert",
        }
    }

    pub fn get_description(&self) -> &'static str {
        match self {
            AgentType::Orchestrator => "Intelligent routing and coordination agent",
            AgentType::BrowserExpert => "Web browsing and navigation specialist",
            AgentType::CodingExpert => "Enhanced coding and development expert",
            AgentType::DesktopExpert => "Desktop automation and system interaction specialist",
            AgentType::GeneralExpert => "General purpose assistant for research and analysis",
        }
    }

    pub fn get_tools_pattern(&self) -> Vec<&'static str> {
        match self {
            AgentType::Orchestrator => vec!["route_to_expert"],
            AgentType::BrowserExpert => vec!["browser_", "navigate", "web", "screenshot"],
            AgentType::CodingExpert => vec!["file", "read", "write", "command", "terminal", "code", "edit", "analyze_project", "plan_multi_file", "generate_code_review", "communicate_with_cursor"],
            AgentType::DesktopExpert => vec!["desktop_", "click", "type", "screenshot", "key", "mouse"],
            AgentType::GeneralExpert => vec!["search", "analyze", "text", "summary"],
        }
    }
}

#[derive(Clone)]
pub struct ExpertAgent {
    pub agent_type: AgentType,
    pub brain: Arc<dyn AgentBrain + Send + Sync>,
    pub tools: Vec<ToolDefinition>,
    pub system_prompt: String,
}

impl ExpertAgent {
    pub fn new(
        agent_type: AgentType,
        brain: Arc<dyn AgentBrain + Send + Sync>,
        tools: Vec<ToolDefinition>,
        prompt_manager: &PromptManager,
    ) -> Self {
        let system_prompt = prompt_manager.get_expert_prompt(agent_type.get_name());
        Self {
            agent_type,
            brain,
            tools,
            system_prompt,
        }
    }
}

pub struct MultiAgentOrchestrator {
    pub orchestrator: Arc<dyn AgentBrain + Send + Sync>,
    pub experts: HashMap<AgentType, ExpertAgent>,
    pub memory: Arc<tokio::sync::Mutex<dyn MemoryManager + Send + Sync>>,
    pub current_expert: Option<AgentType>,
    pub prompt_manager: PromptManager,
}

impl MultiAgentOrchestrator {
    pub async fn new(
        _memory: Arc<dyn MemoryManager + Send + Sync>,
        tool_provider: Arc<dyn ToolProvider + Send + Sync>,
    ) -> Result<Self, AgentError> {
        // Load prompt manager
        let prompt_manager = PromptManager::load().unwrap_or_default();

        // Create orchestrator (Gemini Flash for fast routing decisions)
        let orchestrator_brain = Arc::new(GeminiBrain::from_env()?);

        // Create expert agents with different models
        let mut experts = HashMap::new();

        // Browser Expert - Use Anthropic for vision and web understanding
        let browser_tools = Self::get_tools_for_agent(&AgentType::BrowserExpert, &tool_provider).await;
        let browser_brain = Arc::new(AnthropicBrain::from_env()?);
        experts.insert(
            AgentType::BrowserExpert,
            ExpertAgent::new(AgentType::BrowserExpert, browser_brain, browser_tools, &prompt_manager)
        );

        // Coding Expert - Use OpenAI for code generation
        let coding_tools = Self::get_tools_for_agent(&AgentType::CodingExpert, &tool_provider).await;
        let coding_brain = Arc::new(OpenAIBrain::from_env()?);
        experts.insert(
            AgentType::CodingExpert,
            ExpertAgent::new(AgentType::CodingExpert, coding_brain, coding_tools, &prompt_manager)
        );

        // Desktop Expert - Use Anthropic for complex desktop automation
        let desktop_tools = Self::get_tools_for_agent(&AgentType::DesktopExpert, &tool_provider).await;
        let desktop_brain = Arc::new(AnthropicBrain::from_env()?);
        experts.insert(
            AgentType::DesktopExpert,
            ExpertAgent::new(AgentType::DesktopExpert, desktop_brain, desktop_tools, &prompt_manager)
        );

        // General Expert - Use Gemini Pro for general tasks
        let general_tools = Self::get_tools_for_agent(&AgentType::GeneralExpert, &tool_provider).await;
        let general_brain = Arc::new(GeminiBrain::from_env()?);
        experts.insert(
            AgentType::GeneralExpert,
            ExpertAgent::new(AgentType::GeneralExpert, general_brain, general_tools, &prompt_manager)
        );

        // Wrap the memory in a Mutex for thread-safe mutable access
        let wrapped_memory = Arc::new(tokio::sync::Mutex::new(
            // Create a new SimpleMemoryManager since we can't move out of Arc
            crate::agent::implementations::memory_manager::SimpleMemoryManager::new()
        ));

        Ok(Self {
            orchestrator: orchestrator_brain,
            experts,
            memory: wrapped_memory,
            current_expert: None,
            prompt_manager,
        })
    }

    async fn get_tools_for_agent(
        agent_type: &AgentType,
        tool_provider: &Arc<dyn ToolProvider + Send + Sync>,
    ) -> Vec<ToolDefinition> {
        // Get all available tools
        let all_tools = match tool_provider.list_tools().await {
            Ok(tools) => tools,
            Err(_) => return vec![],
        };

        // Filter tools based on agent type
        let tool_patterns = agent_type.get_tools_pattern();

        all_tools.into_iter()
            .filter(|tool| {
                tool_patterns.iter().any(|pattern| {
                    tool.name.contains(pattern) ||
                    tool.description.to_lowercase().contains(&pattern.to_lowercase()) ||
                    Self::matches_agent_category(agent_type, &tool.name)
                })
            })
            .collect()
    }

    fn matches_agent_category(agent_type: &AgentType, tool_name: &str) -> bool {
        match agent_type {
            AgentType::BrowserExpert => {
                tool_name.starts_with("browser_") ||
                tool_name.contains("navigate") ||
                tool_name.contains("web") ||
                tool_name.contains("url")
            }
            AgentType::CodingExpert => {
                tool_name.contains("file") ||
                tool_name.contains("read") ||
                tool_name.contains("write") ||
                tool_name.contains("command") ||
                tool_name.contains("terminal") ||
                tool_name.contains("code") ||
                tool_name.contains("edit") ||
                // Enhanced coding tools
                tool_name.starts_with("analyze_project") ||
                tool_name.starts_with("plan_multi_file") ||
                tool_name.starts_with("generate_code_review") ||
                tool_name.starts_with("communicate_with_cursor")
            }
            AgentType::DesktopExpert => {
                tool_name.starts_with("desktop_") ||
                tool_name.contains("click") ||
                tool_name.contains("type") ||
                tool_name.contains("key") ||
                tool_name.contains("mouse") ||
                tool_name.contains("screenshot")
            }
            AgentType::GeneralExpert => {
                // General expert gets tools that don't fit other categories
                !Self::matches_agent_category(&AgentType::BrowserExpert, tool_name) &&
                !Self::matches_agent_category(&AgentType::CodingExpert, tool_name) &&
                !Self::matches_agent_category(&AgentType::DesktopExpert, tool_name)
            }
            AgentType::Orchestrator => false, // Orchestrator doesn't get action tools directly
        }
    }

    pub async fn decide_expert(&self, messages: &[Message]) -> Result<AgentType, AgentError> {
        debug!("Multi-agent orchestrator deciding which expert to use");

        // For now, use a simple heuristic-based approach
        // In the future, we can use the orchestrator brain for more sophisticated routing
        self.analyze_request_for_routing(messages).await
    }

    async fn analyze_request_for_routing(&self, messages: &[Message]) -> Result<AgentType, AgentError> {
        if let Some(last_message) = messages.last() {
            let content = last_message.content.to_lowercase();

            // Browser-related keywords
            if content.contains("browse") || content.contains("website") || content.contains("url") ||
               content.contains("navigate") || content.contains("web") || content.contains("click") ||
               content.contains("form") || content.contains("search online") {
                return Ok(AgentType::BrowserExpert);
            }

            // Coding-related keywords
            if content.contains("code") || content.contains("file") || content.contains("program") ||
               content.contains("script") || content.contains("terminal") || content.contains("command") ||
               content.contains("debug") || content.contains("compile") || content.contains("git") ||
               content.contains("repository") || content.contains("function") || content.contains("variable") {
                return Ok(AgentType::CodingExpert);
            }

            // Desktop automation keywords
            if content.contains("open app") || content.contains("application") || content.contains("desktop") ||
               content.contains("window") || content.contains("screenshot") || content.contains("click on") ||
               content.contains("type in") || content.contains("shortcut") {
                return Ok(AgentType::DesktopExpert);
            }
        }

        // Default to general expert for questions, research, etc.
        Ok(AgentType::GeneralExpert)
    }

    pub async fn execute_with_expert(
        &mut self,
        expert_type: AgentType,
        messages: &[Message],
        _cancel_rx: CancelReceiver,
    ) -> Result<AgentAction, AgentError> {
        debug!("Executing with expert: {:?}", expert_type);

        self.current_expert = Some(expert_type.clone());

        if let Some(expert) = self.experts.get(&expert_type) {
            // Add system message with expert's prompt
            let mut expert_messages = vec![Message {
                role: Role::System,
                content: expert.system_prompt.clone(),
                tool_calls: None,
                tool_call_id: None,
                name: None,
            }];
            expert_messages.extend_from_slice(messages);

            // Filter tools for this expert
            let expert_tools = self.filter_tools_for_expert(&expert_type, &expert.tools);

            expert.brain.decide_next_action(&expert_messages, &expert_tools).await
        } else {
            Err(AgentError::ConfigurationError(format!("Expert not found: {:?}", expert_type)))
        }
    }
}

#[async_trait]
impl AgentBrain for MultiAgentOrchestrator {
    async fn decide_next_action(
        &self,
        messages: &[Message],
        available_tools: &[ToolDefinition],
    ) -> Result<AgentAction, AgentError> {
        // For the orchestrator, we should route to an expert
        let expert_type = self.decide_expert(messages).await?;

        // Store messages in memory for context
        for message in messages {
            let mut mem = self.memory.lock().await;
            if let Err(e) = mem.add_message(message.clone()).await {
                warn!("Failed to store message in memory: {}", e);
            }
        }

        // Return a tool execution action for routing to expert
        use crate::agent::structs::ToolCall;
        Ok(AgentAction::ExecuteTool(vec![ToolCall {
            id: uuid::Uuid::new_v4().to_string(),
            name: "route_to_expert".to_string(),
            input: serde_json::json!({
                "expert_type": expert_type.get_name(),
                "reason": format!("Routing to {} for this task", expert_type.get_description())
            }),
        }]))
    }
}

impl MultiAgentOrchestrator {
    fn filter_tools_for_expert(
        &self,
        expert_type: &AgentType,
        available_tools: &[ToolDefinition],
    ) -> Vec<ToolDefinition> {
        let patterns = expert_type.get_tools_pattern();

        available_tools.iter()
            .filter(|tool| {
                patterns.iter().any(|pattern| {
                    tool.name.contains(pattern) ||
                    tool.description.to_lowercase().contains(&pattern.to_lowercase())
                }) || Self::matches_agent_category(expert_type, &tool.name)
            })
            .cloned()
            .collect()
    }
}

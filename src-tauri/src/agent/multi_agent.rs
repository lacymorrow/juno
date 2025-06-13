use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tracing::{debug, warn};
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
use crate::agent::tools::ToolMappingService;

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

        // Use ToolMappingService to filter tools instead of string matching
        let tool_names: Vec<String> = all_tools.iter().map(|t| t.name.clone()).collect();
        let matching_tool_names = ToolMappingService::get_tools_for_agent(&tool_names, &Self::convert_agent_type(agent_type));

        all_tools.into_iter()
            .filter(|tool| matching_tool_names.contains(&tool.name))
            .collect()
    }

    // Helper to convert between AgentType enums
    fn convert_agent_type(agent_type: &AgentType) -> crate::agent::tools::tool_mapping::AgentType {
        match agent_type {
            AgentType::Orchestrator => crate::agent::tools::tool_mapping::AgentType::Orchestrator,
            AgentType::BrowserExpert => crate::agent::tools::tool_mapping::AgentType::BrowserExpert,
            AgentType::CodingExpert => crate::agent::tools::tool_mapping::AgentType::CodingExpert,
            AgentType::DesktopExpert => crate::agent::tools::tool_mapping::AgentType::DesktopExpert,
            AgentType::GeneralExpert => crate::agent::tools::tool_mapping::AgentType::GeneralExpert,
        }
    }

    // Helper to convert from mapping service AgentType
    fn convert_from_mapping_agent_type(agent_type: crate::agent::tools::tool_mapping::AgentType) -> AgentType {
        match agent_type {
            crate::agent::tools::tool_mapping::AgentType::Orchestrator => AgentType::Orchestrator,
            crate::agent::tools::tool_mapping::AgentType::BrowserExpert => AgentType::BrowserExpert,
            crate::agent::tools::tool_mapping::AgentType::CodingExpert => AgentType::CodingExpert,
            crate::agent::tools::tool_mapping::AgentType::DesktopExpert => AgentType::DesktopExpert,
            crate::agent::tools::tool_mapping::AgentType::GeneralExpert => AgentType::GeneralExpert,
        }
    }

    pub async fn decide_expert(&self, messages: &[Message]) -> Result<AgentType, AgentError> {
        debug!("Multi-agent orchestrator deciding which expert to use");

        // Use ToolMappingService for intelligent routing instead of heuristics
        self.analyze_request_for_routing(messages).await
    }

    async fn analyze_request_for_routing(&self, messages: &[Message]) -> Result<AgentType, AgentError> {
        if let Some(last_message) = messages.last() {
            // Use ToolMappingService to analyze user intent instead of keyword matching
            let mapping_agent = ToolMappingService::analyze_user_intent(&last_message.content);
            return Ok(Self::convert_from_mapping_agent_type(mapping_agent));
        }

        // Default to general expert
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
        _available_tools: &[ToolDefinition],
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
        // Use ToolMappingService instead of pattern matching
        let tool_names: Vec<String> = available_tools.iter().map(|t| t.name.clone()).collect();
        let matching_tool_names = ToolMappingService::get_tools_for_agent(&tool_names, &Self::convert_agent_type(expert_type));

        available_tools.iter()
            .filter(|tool| matching_tool_names.contains(&tool.name))
            .cloned()
            .collect()
    }
}

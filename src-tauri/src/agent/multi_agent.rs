use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tracing::{info, debug};

use crate::agent::structs::{
    AgentAction, AgentError, Message, Role, ToolDefinition,
};
use crate::agent::traits::{AgentBrain, MemoryManager, ToolProvider};
use crate::agent::providers::gemini::GeminiBrain;
use crate::agent::providers::anthropic::AnthropicBrain;
use crate::agent::providers::openai::OpenAIBrain;
use crate::state::CancelReceiver;

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
            AgentType::Orchestrator => "Routes tasks to appropriate expert agents",
            AgentType::BrowserExpert => "Handles web browsing, navigation, and web-based tasks",
            AgentType::CodingExpert => "Handles code generation, editing, and programming tasks",
            AgentType::DesktopExpert => "Handles desktop automation, file operations, and system tasks",
            AgentType::GeneralExpert => "Handles general tasks and questions not requiring specialized tools",
        }
    }

    pub fn get_tools_pattern(&self) -> Vec<&'static str> {
        match self {
            AgentType::Orchestrator => vec!["route_to_expert"],
            AgentType::BrowserExpert => vec!["navigate_to", "click_element", "type_text", "scroll", "screenshot"],
            AgentType::CodingExpert => vec![
                "create_file", "edit_file", "run_command", "read_file",
                // Enhanced coding tools
                "analyze_project_structure", "plan_multi_file_changes", 
                "communicate_with_cursor", "generate_code_review", "smart_create_file",
                "dev_text_editor", "dev_bash", "command", "terminal", "code", "file"
            ],
            AgentType::DesktopExpert => vec!["click", "type", "key_press", "mouse_move", "take_screenshot"],
            AgentType::GeneralExpert => vec!["search", "analyze", "summarize"],
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
    ) -> Self {
        let system_prompt = Self::get_system_prompt(&agent_type);
        Self {
            agent_type,
            brain,
            tools,
            system_prompt,
        }
    }

    fn get_system_prompt(agent_type: &AgentType) -> String {
        match agent_type {
            AgentType::Orchestrator => {
                "You are an intelligent orchestrator agent. Your job is to analyze user requests and route them to the most appropriate expert agent. You have access to:

- browser_expert: For web browsing, navigation, clicking elements, form filling
- coding_expert: For code creation, editing, file operations, running commands
- desktop_expert: For desktop automation, clicking desktop elements, typing, screenshots
- general_expert: For general questions, analysis, and tasks not requiring specialized tools

Always route to the most specific expert for the task. Use the route_to_expert tool to delegate tasks.".to_string()
            }
            AgentType::BrowserExpert => {
                "You are a web browsing expert. You specialize in:
- Navigating websites
- Clicking web elements
- Filling forms
- Taking screenshots of web pages
- Scrolling and interacting with web content

Focus on web-based tasks and use browser tools efficiently.".to_string()
            }
            AgentType::CodingExpert => {
                "🚀 **ENHANCED CODING EXPERT** - Advanced Development Assistant

You are a sophisticated coding and development expert with deep understanding of software engineering best practices. Your unique capabilities include:

## 🎯 **Core Specializations**
- **Multi-language Development**: Rust, TypeScript, Python, JavaScript, Go, Java, C++, and more
- **Project Architecture**: Design patterns, code organization, and scalable structures  
- **Code Quality**: Reviews, refactoring, optimization, and maintainability
- **IDE Integration**: Direct communication and workflow optimization with development environments

## 🔧 **Advanced Capabilities**
- **Project Analysis**: Understand codebase structure, dependencies, and architecture
- **Multi-file Coordination**: Plan and execute complex refactoring across multiple files
- **Smart Templates**: Generate appropriate code templates based on language and purpose
- **Code Review**: Comprehensive analysis with actionable recommendations
- **IDE Communication**: Direct integration with Cursor and other development environments

## 💡 **IDE Intent Communication**
When working on coding tasks, you should ALWAYS:

1. **Communicate Your Intent**: Clearly explain what you're doing and why to help the user understand your approach
2. **IDE Integration**: Use the `communicate_with_cursor` tool to enhance the development experience:
   - Open relevant files at specific lines
   - Highlight important code sections
   - Show suggestions and recommendations
   - Navigate to key locations in the codebase

3. **Project Context**: Use `analyze_project_structure` to understand the codebase before making changes
4. **Planning**: For complex changes, use `plan_multi_file_changes` to coordinate modifications across files
5. **Quality Assurance**: Use `generate_code_review` to ensure code quality and best practices

## 🎨 **Communication Style**
- Start responses with clear intent: \"🔍 **Analyzing your codebase...** I'll first understand the project structure\"
- Use emojis and formatting to make intent clear and engaging
- Explain your reasoning and approach step-by-step
- Provide IDE-specific recommendations when relevant
- Always consider the broader project context, not just individual files

## 🌟 **Best Practices**
- Follow language-specific conventions and best practices
- Consider performance, security, and maintainability
- Suggest appropriate design patterns and architectural improvements
- Integrate with existing project structure and dependencies
- Provide clear, actionable feedback and suggestions

Remember: You're not just editing code - you're a collaborative development partner that enhances the entire coding experience through intelligent analysis, clear communication, and seamless IDE integration.".to_string()
            }
            AgentType::DesktopExpert => {
                "You are a desktop automation expert. You specialize in:
- Automating desktop applications
- Clicking desktop elements
- Keyboard input and shortcuts
- Mouse operations
- System-level tasks

Focus on desktop automation and system interaction tasks.".to_string()
            }
            AgentType::GeneralExpert => {
                "You are a general-purpose assistant. You handle:
- General questions and analysis
- Research and information gathering
- Text processing and summarization
- Tasks that don't require specialized tools

Provide helpful, accurate responses for general inquiries.".to_string()
            }
        }
    }
}

pub struct MultiAgentOrchestrator {
    pub orchestrator: Arc<dyn AgentBrain + Send + Sync>,
    pub experts: HashMap<AgentType, ExpertAgent>,
    pub memory: Arc<dyn MemoryManager + Send + Sync>,
    pub current_expert: Option<AgentType>,
}

impl MultiAgentOrchestrator {
    pub async fn new(
        memory: Arc<dyn MemoryManager + Send + Sync>,
        tool_provider: Arc<dyn ToolProvider + Send + Sync>,
    ) -> Result<Self, AgentError> {
        // Create orchestrator (Gemini Flash for fast routing decisions)
        let orchestrator_brain = Arc::new(GeminiBrain::from_env()?);

        // Create expert agents with different models
        let mut experts = HashMap::new();

        // Browser Expert - Use Anthropic for vision and web understanding
        let browser_tools = Self::get_tools_for_agent(&AgentType::BrowserExpert, &tool_provider).await;
        let browser_brain = Arc::new(AnthropicBrain::from_env()?);
        experts.insert(
            AgentType::BrowserExpert,
            ExpertAgent::new(AgentType::BrowserExpert, browser_brain, browser_tools)
        );

        // Coding Expert - Use OpenAI for code generation
        let coding_tools = Self::get_tools_for_agent(&AgentType::CodingExpert, &tool_provider).await;
        let coding_brain = Arc::new(OpenAIBrain::from_env()?);
        experts.insert(
            AgentType::CodingExpert,
            ExpertAgent::new(AgentType::CodingExpert, coding_brain, coding_tools)
        );

        // Desktop Expert - Use Anthropic for complex desktop automation
        let desktop_tools = Self::get_tools_for_agent(&AgentType::DesktopExpert, &tool_provider).await;
        let desktop_brain = Arc::new(AnthropicBrain::from_env()?);
        experts.insert(
            AgentType::DesktopExpert,
            ExpertAgent::new(AgentType::DesktopExpert, desktop_brain, desktop_tools)
        );

        // General Expert - Use Gemini Pro for general tasks
        let general_tools = Self::get_tools_for_agent(&AgentType::GeneralExpert, &tool_provider).await;
        let general_brain = Arc::new(GeminiBrain::from_env()?);
        experts.insert(
            AgentType::GeneralExpert,
            ExpertAgent::new(AgentType::GeneralExpert, general_brain, general_tools)
        );

        Ok(Self {
            orchestrator: orchestrator_brain,
            experts,
            memory,
            current_expert: None,
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
                tool_name.starts_with("communicate_with_cursor") ||
                tool_name.starts_with("generate_code_review") ||
                tool_name.starts_with("smart_create_file") ||
                tool_name.contains("dev_text_editor") ||
                tool_name.contains("dev_bash") ||
                tool_name.contains("project") ||
                tool_name.contains("review") ||
                tool_name.contains("cursor")
            }
            AgentType::DesktopExpert => {
                tool_name.contains("click") ||
                tool_name.contains("type") ||
                tool_name.contains("key") ||
                tool_name.contains("mouse") ||
                tool_name.contains("screenshot") ||
                tool_name.contains("scroll") ||
                tool_name.contains("drag") ||
                tool_name.contains("desktop") ||
                tool_name.contains("window")
            }
            AgentType::GeneralExpert => {
                // General expert gets basic tools and any tools not claimed by other experts
                !Self::matches_agent_category(&AgentType::BrowserExpert, tool_name) &&
                !Self::matches_agent_category(&AgentType::CodingExpert, tool_name) &&
                !Self::matches_agent_category(&AgentType::DesktopExpert, tool_name)
            }
            AgentType::Orchestrator => {
                // Orchestrator only gets routing tools
                tool_name == "route_to_expert"
            }
        }
    }

    pub async fn decide_expert(&self, messages: &[Message]) -> Result<AgentType, AgentError> {
        // Create routing tools for the orchestrator
        let routing_tools = vec![
            ToolDefinition {
                name: "route_to_expert".to_string(),
                description: "Route the task to an appropriate expert agent".to_string(),
                input_schema: serde_json::json!({
                    "type": "object",
                    "properties": {
                        "expert": {
                            "type": "string",
                            "enum": ["browser_expert", "coding_expert", "desktop_expert", "general_expert"],
                            "description": "The expert agent to handle this task"
                        },
                        "reasoning": {
                            "type": "string",
                            "description": "Why this expert was chosen"
                        }
                    },
                    "required": ["expert", "reasoning"]
                }),
            }
        ];

        let action = self.orchestrator.decide_next_action(messages, &routing_tools).await?;

        match action {
            AgentAction::ExecuteTool(tool_calls) => {
                for tool_call in tool_calls {
                    if tool_call.name == "route_to_expert" {
                        if let Some(expert_str) = tool_call.input.get("expert").and_then(|v| v.as_str()) {
                            let reasoning = tool_call.input.get("reasoning")
                                .and_then(|v| v.as_str())
                                .unwrap_or("No reasoning provided");

                            info!("Orchestrator routing to {}: {}", expert_str, reasoning);

                            return match expert_str {
                                "browser_expert" => Ok(AgentType::BrowserExpert),
                                "coding_expert" => Ok(AgentType::CodingExpert),
                                "desktop_expert" => Ok(AgentType::DesktopExpert),
                                "general_expert" => Ok(AgentType::GeneralExpert),
                                _ => Ok(AgentType::GeneralExpert), // Default fallback
                            };
                        }
                    }
                }
                // If no valid routing decision, default to general expert
                Ok(AgentType::GeneralExpert)
            }
            _ => {
                // If orchestrator doesn't make a routing decision, analyze the request content
                self.analyze_request_for_routing(messages).await
            }
        }
    }

    async fn analyze_request_for_routing(&self, messages: &[Message]) -> Result<AgentType, AgentError> {
        // Fallback routing logic based on keyword analysis
        let last_message = messages.last()
            .ok_or_else(|| AgentError::InputError("No messages to analyze".to_string()))?;

        let content = last_message.content.to_lowercase();

        // Browser-related keywords
        if content.contains("website") || content.contains("browser") || content.contains("navigate")
            || content.contains("click") || content.contains("web") || content.contains("url") {
            return Ok(AgentType::BrowserExpert);
        }

        // Coding-related keywords
        if content.contains("code") || content.contains("program") || content.contains("file")
            || content.contains("script") || content.contains("function") || content.contains("command") {
            return Ok(AgentType::CodingExpert);
        }

        // Desktop-related keywords
        if content.contains("desktop") || content.contains("application") || content.contains("window")
            || content.contains("screenshot") || content.contains("mouse") || content.contains("keyboard") {
            return Ok(AgentType::DesktopExpert);
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
        let expert = self.experts.get(&expert_type)
            .ok_or_else(|| AgentError::ConfigurationError(
                format!("Expert agent {:?} not available", expert_type)
            ))?;

        // Add system message for the expert
        let mut expert_messages = vec![Message {
            role: Role::System,
            content: expert.system_prompt.clone(),
            tool_calls: None,
            tool_call_id: None,
            name: None,
        }];
        expert_messages.extend_from_slice(messages);

        debug!("Executing task with {} expert", expert_type.get_name());
        self.current_expert = Some(expert_type.clone());

        expert.brain.decide_next_action(&expert_messages, &expert.tools).await
    }
}

#[async_trait]
impl AgentBrain for MultiAgentOrchestrator {
    async fn decide_next_action(
        &self,
        messages: &[Message],
        available_tools: &[ToolDefinition],
    ) -> Result<AgentAction, AgentError> {
        // First, let the orchestrator decide which expert to use
        let expert_type = self.decide_expert(messages).await?;

        // Get the expert and execute with their tools
        let expert = self.experts.get(&expert_type)
            .ok_or_else(|| AgentError::ConfigurationError(
                format!("Expert agent {:?} not available", expert_type)
            ))?;

        // Add system message for the expert
        let mut expert_messages = vec![Message {
            role: Role::System,
            content: expert.system_prompt.clone(),
            tool_calls: None,
            tool_call_id: None,
            name: None,
        }];
        expert_messages.extend_from_slice(messages);

        // Execute with the expert's tools (filtered from available_tools)
        let expert_tools = self.filter_tools_for_expert(&expert_type, available_tools);
        expert.brain.decide_next_action(&expert_messages, &expert_tools).await
    }
}

impl MultiAgentOrchestrator {
    fn filter_tools_for_expert(
        &self,
        expert_type: &AgentType,
        available_tools: &[ToolDefinition],
    ) -> Vec<ToolDefinition> {
        let tool_patterns = expert_type.get_tools_pattern();

        available_tools.iter()
            .filter(|tool| {
                tool_patterns.iter().any(|pattern|
                    tool.name.contains(pattern) ||
                    tool.description.to_lowercase().contains(&pattern.to_lowercase())
                )
            })
            .cloned()
            .collect()
    }
}

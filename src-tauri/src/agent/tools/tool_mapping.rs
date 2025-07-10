//! # Tool Mapping Service - Simplified
//!
//! Clean, simple tool categorization following the no-micromanaging rule.
//! Trust the AI agent to use the right tools - we just provide clean categories.

use std::collections::HashMap;
use serde::{Deserialize, Serialize};
use once_cell::sync::Lazy;

use super::tool_config::ToolCategory;
use crate::constants::agent::{tool_names, intent_keywords};

/// Agent types for routing
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum AgentType {
    Orchestrator,
    BrowserExpert,
    CodingExpert,
    DesktopExpert,
    GeneralExpert,
}

/// Simple tool category mapping - using centralized constants
static OFFICIAL_TOOLS: Lazy<HashMap<&'static str, ToolCategory>> = Lazy::new(|| {
    let mut map = HashMap::new();
    
    // Official Anthropic Computer Use API tools only
    map.insert(tool_names::COMPUTER, ToolCategory::AnthropicComputerUse);
    map.insert(tool_names::BASH, ToolCategory::AnthropicComputerUse);
    map.insert(tool_names::STR_REPLACE_BASED_EDIT_TOOL, ToolCategory::AnthropicComputerUse);
    
    // Browser tools (consolidated)
    map.insert(tool_names::BROWSER_NAVIGATE, ToolCategory::Browser);
    map.insert(tool_names::BROWSER_INTERACT, ToolCategory::Browser);
    map.insert(tool_names::BROWSER_EXTRACT_CONTENT, ToolCategory::Browser);
    
    // Basic system tools  
    map.insert(tool_names::READ_FILE, ToolCategory::Basic);
    map.insert(tool_names::LIST_FILES, ToolCategory::Basic);
    
    // Timer tools (minimal)
    map.insert(tool_names::LIST_TIMERS, ToolCategory::Timer);
    map.insert(tool_names::CANCEL_TIMER, ToolCategory::Timer);
    
    map
});

/// Simple category to agent mapping
static CATEGORY_AGENTS: Lazy<HashMap<ToolCategory, AgentType>> = Lazy::new(|| {
    let mut map = HashMap::new();
    
    map.insert(ToolCategory::AnthropicComputerUse, AgentType::DesktopExpert);
    map.insert(ToolCategory::Browser, AgentType::BrowserExpert);  
    map.insert(ToolCategory::Basic, AgentType::CodingExpert);
    map.insert(ToolCategory::Timer, AgentType::GeneralExpert);
    
    map
});

/// Simplified tool mapping service
pub struct ToolMappingService;

impl ToolMappingService {
    /// Get tool category - simple lookup, no complex logic
    pub fn get_tool_category(tool_name: &str) -> Option<ToolCategory> {
        OFFICIAL_TOOLS.get(tool_name).cloned()
    }
    
    /// Get agent for tool - simple mapping
    pub fn get_agent_for_tool(tool_name: &str) -> Option<AgentType> {
        Self::get_tool_category(tool_name)
            .and_then(|category| CATEGORY_AGENTS.get(&category))
            .cloned()
    }
    
    /// Analyze user intent - using centralized intent keywords
    pub fn analyze_user_intent(content: &str) -> AgentType {
        let content_lower = content.to_lowercase();
        
        // Browser expert keywords
        if content_lower.contains(intent_keywords::BROWSER) 
            || content_lower.contains(intent_keywords::WEBSITE) 
            || content_lower.contains(intent_keywords::NAVIGATE) 
            || content_lower.contains(intent_keywords::WEB) {
            AgentType::BrowserExpert
        }
        // Coding expert keywords  
        else if content_lower.contains(intent_keywords::FILE) 
            || content_lower.contains(intent_keywords::CODE) 
            || content_lower.contains(intent_keywords::EDIT)
            || content_lower.contains(intent_keywords::BASH) {
            AgentType::CodingExpert  
        }
        // Desktop expert keywords
        else if content_lower.contains(intent_keywords::CLICK_ON) 
            || content_lower.contains(intent_keywords::SCREENSHOT) 
            || content_lower.contains(intent_keywords::DESKTOP)
            || content_lower.contains(intent_keywords::MOUSE) {
            AgentType::DesktopExpert
        }
        else {
            AgentType::GeneralExpert
        }
    }
    
    /// Check if tool is in category - simple boolean
    pub fn is_tool_in_category(tool_name: &str, category: &ToolCategory) -> bool {
        Self::get_tool_category(tool_name)
            .map(|tool_category| tool_category == *category)
            .unwrap_or(false)
    }
    
    /// Check if agent can handle tool - simple lookup
    pub fn can_agent_handle_tool(tool_name: &str, agent_type: &AgentType) -> bool {
        Self::get_agent_for_tool(tool_name)
            .map(|tool_agent| tool_agent == *agent_type)
            .unwrap_or(false)
    }
    
    /// Get tools for agent - simple filtering
    pub fn get_tools_for_agent(tool_names: &[String], agent_type: &AgentType) -> Vec<String> {
        tool_names.iter()
            .filter(|tool_name| Self::can_agent_handle_tool(tool_name, agent_type))
            .cloned()
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::constants::agent::tool_names;
    
    #[test]
    fn test_official_tools() {
        assert_eq!(ToolMappingService::get_tool_category(tool_names::COMPUTER), Some(ToolCategory::AnthropicComputerUse));
        assert_eq!(ToolMappingService::get_tool_category(tool_names::BASH), Some(ToolCategory::AnthropicComputerUse));
        assert_eq!(ToolMappingService::get_tool_category(tool_names::STR_REPLACE_BASED_EDIT_TOOL), Some(ToolCategory::AnthropicComputerUse));
    }
    
    #[test]
    fn test_agent_routing() {
        assert_eq!(ToolMappingService::get_agent_for_tool(tool_names::COMPUTER), Some(AgentType::DesktopExpert));
        assert_eq!(ToolMappingService::get_agent_for_tool(tool_names::BROWSER_NAVIGATE), Some(AgentType::BrowserExpert));
        assert_eq!(ToolMappingService::get_agent_for_tool(tool_names::READ_FILE), Some(AgentType::CodingExpert));
    }
    
    #[test] 
    fn test_intent_analysis() {
        assert_eq!(ToolMappingService::analyze_user_intent("navigate to website"), AgentType::BrowserExpert);
        assert_eq!(ToolMappingService::analyze_user_intent("edit this file"), AgentType::CodingExpert);
        assert_eq!(ToolMappingService::analyze_user_intent("take a screenshot"), AgentType::DesktopExpert);
    }
}
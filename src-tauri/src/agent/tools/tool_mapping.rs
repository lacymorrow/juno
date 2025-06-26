use std::collections::HashMap;
use serde::{Deserialize, Serialize};
use once_cell::sync::Lazy;

use super::tool_config::ToolCategory;
use crate::constants::agent::{tool_names, intent_keywords, tool_prefixes, test_strings, confidence_scores};

/// Maps tool names to their proper categories
/// This replaces all the brittle string matching throughout the codebase
static TOOL_CATEGORY_MAP: Lazy<HashMap<&'static str, ToolCategory>> = Lazy::new(|| {
    let mut map = HashMap::new();

    // Anthropic Computer Use tools
    map.insert(tool_names::SCREENSHOT, ToolCategory::AnthropicComputerUse);
    map.insert(tool_names::CLICK, ToolCategory::AnthropicComputerUse);
    map.insert(tool_names::TYPE, ToolCategory::AnthropicComputerUse);
    map.insert(tool_names::KEY, ToolCategory::AnthropicComputerUse);
    map.insert(tool_names::SCROLL, ToolCategory::AnthropicComputerUse);
    map.insert(tool_names::DRAG, ToolCategory::AnthropicComputerUse);
    map.insert(tool_names::MOVE, ToolCategory::AnthropicComputerUse);
    map.insert(tool_names::COMPUTER, ToolCategory::AnthropicComputerUse);
    map.insert(tool_names::BASH, ToolCategory::AnthropicComputerUse);
    map.insert(tool_names::STR_REPLACE_BASED_EDIT_TOOL, ToolCategory::AnthropicComputerUse);
    map.insert(tool_names::ACCESSIBILITY_INTERFACE, ToolCategory::AnthropicComputerUse);

    // Browser tools
    map.insert(tool_names::BROWSER_NAVIGATE, ToolCategory::Browser);
    map.insert(tool_names::BROWSER_CLICK, ToolCategory::Browser);
    map.insert(tool_names::BROWSER_TYPE, ToolCategory::Browser);
    map.insert(tool_names::BROWSER_SCROLL, ToolCategory::Browser);
    map.insert(tool_names::BROWSER_SCREENSHOT, ToolCategory::Browser);
    map.insert(tool_names::BROWSER_GET_CONTENT, ToolCategory::Browser);
    map.insert(tool_names::BROWSER_INTERACT, ToolCategory::Browser);
    map.insert(tool_names::BROWSER_EXTRACT_CONTENT, ToolCategory::Browser);
    map.insert(tool_names::BROWSER_GET_CURRENT_URL, ToolCategory::Browser);
    map.insert(tool_names::BROWSER_FORM, ToolCategory::Browser);

    // Safari tools (specialized browser automation for Safari)
    map.insert(tool_names::SAFARI_EXTRACT_DOM, ToolCategory::Browser);
    map.insert(tool_names::SAFARI_CLICK_ELEMENT, ToolCategory::Browser);
    map.insert(tool_names::SAFARI_TYPE_TEXT, ToolCategory::Browser);
    map.insert(tool_names::SAFARI_GET_URL, ToolCategory::Browser);
    map.insert(tool_names::SAFARI_NAVIGATE, ToolCategory::Browser);
    map.insert(tool_names::SAFARI_LIST_CLICKABLE_ELEMENTS, ToolCategory::Browser);
    map.insert(tool_names::SAFARI_EXECUTE_JAVASCRIPT, ToolCategory::Browser);
    map.insert(tool_names::SAFARI_CLEAR_CACHE, ToolCategory::Browser);

    // Desktop tools
    // REMOVED: 11 redundant mouse tools - Use computer tool with official Anthropic Computer Use API instead
    // dev_left_click, desktop_click, left_click → computer tool with computer_actions::CLICK
    // dev_right_click, right_click → computer tool with computer_actions::RIGHT_CLICK
    // dev_middle_click, middle_click → computer tool with computer_actions::MIDDLE_CLICK
    // dev_double_click, double_click → computer tool with computer_actions::DOUBLE_CLICK
    // dev_triple_click, triple_click → computer tool with computer_actions::TRIPLE_CLICK
    // dev_left_click_drag, left_click_drag → computer tool with computer_actions::DRAG
    // dev_left_mouse_down, left_mouse_down → computer tool with computer_actions::DRAG (start)
    // dev_left_mouse_up, left_mouse_up → computer tool with computer_actions::DRAG (complete)
    // mouse_move → computer tool with computer_actions::CLICK (movement automatic)
    // REMOVED: 4 redundant keyboard tools - Use computer tool instead
    // dev_type_text, desktop_type → computer tool with computer_actions::TYPE
    // dev_global_type_text → computer tool with computer_actions::TYPE
    // dev_press_key → computer tool with computer_actions::KEY
    // This eliminates 15 redundant tools total and ensures 100% compliance with the official specification.

    map.insert(tool_names::OPEN_APPLICATION, ToolCategory::Desktop);
    map.insert(tool_names::OPEN_URL, ToolCategory::Desktop);
    map.insert(tool_names::DEV_FOCUS_WINDOW, ToolCategory::Desktop);
    map.insert(tool_names::DEV_SCROLL_WINDOW, ToolCategory::Desktop);
    map.insert(tool_names::CAPTURE_SCREENSHOT_COMMAND, ToolCategory::Desktop);
    map.insert(tool_names::DEV_GET_CLIPBOARD, ToolCategory::Desktop);
    map.insert(tool_names::DEV_SET_CLIPBOARD, ToolCategory::Desktop);
    map.insert(tool_names::DEV_GET_WINDOW_LIST, ToolCategory::Desktop);
    map.insert(tool_names::DEV_FIND_ELEMENT_BY_SELECTOR, ToolCategory::Desktop);
    map.insert(tool_names::DESKTOP_OPEN_APP, ToolCategory::Desktop);
    map.insert(tool_names::DESKTOP_FOCUS_WINDOW, ToolCategory::Desktop);
    map.insert(tool_names::DESKTOP_SCROLL, ToolCategory::Desktop);
    map.insert(tool_names::DESKTOP_SCREENSHOT, ToolCategory::Desktop);
    map.insert(tool_names::LAUNCH_APPLICATION, ToolCategory::Desktop);
    map.insert(tool_names::GET_RUNNING_APPLICATIONS, ToolCategory::Desktop);
    map.insert(tool_names::FOCUS_APPLICATION, ToolCategory::Desktop);
    map.insert(tool_names::QUIT_APPLICATION, ToolCategory::Desktop);
    map.insert(tool_names::GET_SYSTEM_INFO, ToolCategory::Desktop);
    map.insert(tool_names::MANAGE_AUDIO, ToolCategory::Desktop);

    // Accessibility tools (native macOS element interaction)
    map.insert(tool_names::ACCESSIBILITY_SCAN, ToolCategory::Desktop);
    map.insert(tool_names::ACCESSIBILITY_CLICK, ToolCategory::Desktop);

    // Basic tools (file operations, commands, etc.)
    map.insert(tool_names::BASH_COMMAND, ToolCategory::Basic);
    map.insert(tool_names::LIST_FILES, ToolCategory::Basic);
    map.insert(tool_names::GET_FILE_CONTENT, ToolCategory::Basic);
    map.insert(tool_names::SET_FILE_CONTENT, ToolCategory::Basic);
    map.insert(tool_names::DEV_TEXT_EDITOR_VIEW, ToolCategory::Basic);
    map.insert(tool_names::DEV_TEXT_EDITOR_CREATE, ToolCategory::Basic);
    map.insert(tool_names::DEV_TEXT_EDITOR_STR_REPLACE, ToolCategory::Basic);
    map.insert(tool_names::SYSTEM_EXEC, ToolCategory::Basic);
    map.insert(tool_names::SYSTEM_LIST_FILES, ToolCategory::Basic);
    map.insert(tool_names::SYSTEM_READ_FILE, ToolCategory::Basic);
    map.insert(tool_names::SYSTEM_WRITE_FILE, ToolCategory::Basic);
    map.insert(tool_names::DEV_LIST_FILES, ToolCategory::Basic);
    map.insert(tool_names::DEV_GET_FILE_CONTENT, ToolCategory::Basic);
    map.insert(tool_names::DEV_SET_FILE_CONTENT, ToolCategory::Basic);
    map.insert(tool_names::FILE_READ, ToolCategory::Basic);
    map.insert(tool_names::FILE_WRITE, ToolCategory::Basic);
    map.insert(tool_names::FILE_CREATE, ToolCategory::Basic);
    map.insert(tool_names::FILE_DELETE, ToolCategory::Basic);
    map.insert(tool_names::COMMAND_EXECUTE, ToolCategory::Basic);
    map.insert(tool_names::SHELL_EXECUTE, ToolCategory::Basic);
    map.insert(tool_names::BASH_EXECUTE, ToolCategory::Basic);

    // Timer tools
    map.insert(tool_names::TIMER_CREATE, ToolCategory::Timer);
    map.insert(tool_names::TIMER_START, ToolCategory::Timer);
    map.insert(tool_names::TIMER_STOP, ToolCategory::Timer);
    map.insert(tool_names::TIMER_PAUSE, ToolCategory::Timer);
    map.insert(tool_names::TIMER_RESUME, ToolCategory::Timer);
    map.insert(tool_names::TIMER_GET_STATUS, ToolCategory::Timer);
    map.insert(tool_names::TIMER_LIST, ToolCategory::Timer);
    map.insert(tool_names::TIMER_DELETE, ToolCategory::Timer);

    map
});

/// Agent types for proper routing
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum AgentType {
    Orchestrator,
    BrowserExpert,
    CodingExpert,
    DesktopExpert,
    GeneralExpert,
}

/// Maps tool categories to their most appropriate agent types
static CATEGORY_TO_AGENT_MAP: Lazy<HashMap<ToolCategory, AgentType>> = Lazy::new(|| {
    let mut map = HashMap::new();

    map.insert(ToolCategory::AnthropicComputerUse, AgentType::DesktopExpert);
    map.insert(ToolCategory::Browser, AgentType::BrowserExpert);
    map.insert(ToolCategory::Desktop, AgentType::DesktopExpert);
    map.insert(ToolCategory::Basic, AgentType::CodingExpert);
    map.insert(ToolCategory::Timer, AgentType::GeneralExpert);
    map.insert(ToolCategory::MCP, AgentType::GeneralExpert);

    map
});

/// Intent keywords for user request analysis
static INTENT_KEYWORDS: Lazy<HashMap<&'static str, AgentType>> = Lazy::new(|| {
    let mut map = HashMap::new();

    // Browser expert keywords
    let browser_keywords = [
        intent_keywords::BROWSE, intent_keywords::WEBSITE, intent_keywords::URL,
        intent_keywords::NAVIGATE, intent_keywords::WEB, intent_keywords::PAGE,
        intent_keywords::FORM, intent_keywords::SEARCH_ONLINE, intent_keywords::INTERNET,
        intent_keywords::BROWSER, intent_keywords::LINK, intent_keywords::DOMAIN, intent_keywords::HTTP
    ];
    for keyword in &browser_keywords {
        map.insert(*keyword, AgentType::BrowserExpert);
    }

    // Coding expert keywords
    let coding_keywords = [
        intent_keywords::CODE, intent_keywords::FILE, intent_keywords::PROGRAM,
        intent_keywords::SCRIPT, intent_keywords::TERMINAL, intent_keywords::COMMAND,
        intent_keywords::DEBUG, intent_keywords::COMPILE, intent_keywords::GIT,
        intent_keywords::REPOSITORY, intent_keywords::FUNCTION, intent_keywords::VARIABLE,
        intent_keywords::EDIT, intent_keywords::CREATE_FILE, intent_keywords::READ_FILE,
        intent_keywords::WRITE_FILE, intent_keywords::BASH, intent_keywords::SHELL
    ];
    for keyword in &coding_keywords {
        map.insert(*keyword, AgentType::CodingExpert);
    }

    // Desktop expert keywords
    let desktop_keywords = [
        intent_keywords::OPEN_APP, intent_keywords::APPLICATION, intent_keywords::DESKTOP,
        intent_keywords::WINDOW, intent_keywords::SCREENSHOT, intent_keywords::CLICK_ON,
        intent_keywords::TYPE_IN, intent_keywords::SHORTCUT, intent_keywords::MOUSE,
        intent_keywords::KEYBOARD, intent_keywords::CLIPBOARD
    ];
    for keyword in &desktop_keywords {
        map.insert(*keyword, AgentType::DesktopExpert);
    }

    map
});

/// Central service for tool categorization and agent routing
/// Replaces all string matching patterns throughout the codebase
pub struct ToolMappingService;

impl ToolMappingService {
    /// Get the category for a specific tool name
    /// This replaces functions like is_browser_tool(), is_system_tool(), etc.
    pub fn get_tool_category(tool_name: &str) -> Option<ToolCategory> {
        // Direct lookup first
        if let Some(category) = TOOL_CATEGORY_MAP.get(tool_name) {
            return Some(category.clone());
        }

        // Fallback to prefix matching for dynamically named tools
        if tool_name.starts_with(tool_prefixes::BROWSER) || tool_name.starts_with(tool_prefixes::SAFARI) {
            return Some(ToolCategory::Browser);
        }
        if tool_name.starts_with(tool_prefixes::DEV) || tool_name.starts_with(tool_prefixes::DESKTOP) {
            return Some(ToolCategory::Desktop);
        }
        if tool_name.starts_with(tool_prefixes::SYSTEM) {
            return Some(ToolCategory::Basic);
        }
        if tool_name.starts_with(tool_prefixes::TIMER) {
            return Some(ToolCategory::Timer);
        }
        if tool_name.starts_with(tool_prefixes::MCP) {
            return Some(ToolCategory::MCP);
        }

        None
    }

    /// Get the best agent type for a specific tool
    /// This replaces matches_agent_category() and similar functions
    pub fn get_agent_for_tool(tool_name: &str) -> Option<AgentType> {
        Self::get_tool_category(tool_name)
            .and_then(|category| CATEGORY_TO_AGENT_MAP.get(&category))
            .cloned()
    }

    /// Determine the best agent type based on user intent/content
    /// This replaces analyze_request_for_routing() string matching
    pub fn analyze_user_intent(content: &str) -> AgentType {
        let content_lower = content.to_lowercase();

        // Find the most relevant agent based on keyword matches
        let mut agent_scores: HashMap<AgentType, usize> = HashMap::new();

        for (keyword, agent_type) in INTENT_KEYWORDS.iter() {
            if content_lower.contains(keyword) {
                *agent_scores.entry(agent_type.clone()).or_insert(0) += 1;
            }
        }

        // Return the agent with the highest score, defaulting to GeneralExpert
        agent_scores.into_iter()
            .max_by_key(|(_, score)| *score)
            .map(|(agent, _)| agent)
            .unwrap_or(AgentType::GeneralExpert)
    }

    /// Check if a tool belongs to a specific category
    /// This replaces individual is_*_tool() functions
    pub fn is_tool_in_category(tool_name: &str, category: &ToolCategory) -> bool {
        Self::get_tool_category(tool_name)
            .map(|tool_category| tool_category == *category)
            .unwrap_or(false)
    }

    /// Check if a tool can be handled by a specific agent type
    /// This replaces the can_handle_task() string matching logic
    pub fn can_agent_handle_tool(tool_name: &str, agent_type: &AgentType) -> bool {
        Self::get_agent_for_tool(tool_name)
            .map(|tool_agent| tool_agent == *agent_type)
            .unwrap_or(false)
    }

    /// Get all tools for a specific agent type
    /// This replaces the filter_tools_for_expert() logic
    pub fn get_tools_for_agent(tool_names: &[String], agent_type: &AgentType) -> Vec<String> {
        tool_names.iter()
            .filter(|tool_name| Self::can_agent_handle_tool(tool_name, agent_type))
            .cloned()
            .collect()
    }

    /// Check if a task description is relevant for a specific agent
    /// This replaces the task description string matching in can_handle_task()
    pub fn can_agent_handle_description(description: &str, agent_type: &AgentType) -> bool {
        let inferred_agent = Self::analyze_user_intent(description);
        inferred_agent == *agent_type
    }

    /// Get confidence score for an agent handling a specific tool (0.0 to 1.0)
    /// This provides a more nuanced approach than boolean matching
    pub fn get_agent_confidence_for_tool(tool_name: &str, agent_type: &AgentType) -> f32 {
        if Self::can_agent_handle_tool(tool_name, agent_type) {
            // High confidence for exact matches
            confidence_scores::HIGH_CONFIDENCE
        } else {
            // Check if there's partial relevance based on category
            if let Some(tool_category) = Self::get_tool_category(tool_name) {
                match (tool_category, agent_type) {
                    // Some tools could be handled by multiple agents with lower confidence
                    (ToolCategory::AnthropicComputerUse, AgentType::BrowserExpert) => confidence_scores::PARTIAL_BROWSER_COMPUTER_USE, // Screenshots, clicks can help browser work
                    (ToolCategory::Basic, AgentType::DesktopExpert) => confidence_scores::PARTIAL_DESKTOP_BASIC, // Some file ops relate to desktop
                    (ToolCategory::Desktop, AgentType::CodingExpert) => confidence_scores::PARTIAL_CODING_DESKTOP, // Very limited overlap
                    _ => confidence_scores::NO_CONFIDENCE
                }
            } else {
                confidence_scores::NO_CONFIDENCE
            }
        }
    }

    /// Add a new tool mapping (for dynamic tools like MCP)
    /// This provides extensibility without modifying the core mappings
    pub fn register_dynamic_tool(tool_name: String, category: ToolCategory) {
        // For now, we'll use static mappings, but this could be extended
        // to support dynamic registration for MCP tools and plugins
        tracing::info!("Dynamic tool registration not yet implemented: {} -> {:?}", tool_name, category);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tool_categorization() {
        assert_eq!(ToolMappingService::get_tool_category(tool_names::BROWSER_NAVIGATE), Some(ToolCategory::Browser));
        assert_eq!(ToolMappingService::get_tool_category(tool_names::COMPUTER), Some(ToolCategory::AnthropicComputerUse)); // Test production tool instead of dev_left_click
        assert_eq!(ToolMappingService::get_tool_category(tool_names::BASH_COMMAND), Some(ToolCategory::Basic));
        assert_eq!(ToolMappingService::get_tool_category(tool_names::TIMER_CREATE), Some(ToolCategory::Timer));
        assert_eq!(ToolMappingService::get_tool_category(tool_names::SCREENSHOT), Some(ToolCategory::AnthropicComputerUse));
    }

    #[test]
    fn test_agent_routing() {
        assert_eq!(ToolMappingService::get_agent_for_tool(tool_names::BROWSER_NAVIGATE), Some(AgentType::BrowserExpert));
        assert_eq!(ToolMappingService::get_agent_for_tool(tool_names::COMPUTER), Some(AgentType::DesktopExpert)); // Test production tool instead of dev_left_click
        assert_eq!(ToolMappingService::get_agent_for_tool(tool_names::BASH_COMMAND), Some(AgentType::CodingExpert));
        assert_eq!(ToolMappingService::get_agent_for_tool(tool_names::TIMER_CREATE), Some(AgentType::GeneralExpert));
    }

    #[test]
    fn test_user_intent_analysis() {
        assert_eq!(ToolMappingService::analyze_user_intent(test_strings::NAVIGATE_TO_WEBSITE), AgentType::BrowserExpert);
        assert_eq!(ToolMappingService::analyze_user_intent(test_strings::EDIT_FILE), AgentType::CodingExpert);
        assert_eq!(ToolMappingService::analyze_user_intent(test_strings::TAKE_SCREENSHOT), AgentType::DesktopExpert);
        assert_eq!(ToolMappingService::analyze_user_intent(test_strings::WEATHER_QUERY), AgentType::GeneralExpert);
    }

    #[test]
    fn test_category_matching() {
        assert!(ToolMappingService::is_tool_in_category(tool_names::BROWSER_NAVIGATE, &ToolCategory::Browser));
        assert!(!ToolMappingService::is_tool_in_category(tool_names::BROWSER_NAVIGATE, &ToolCategory::Desktop));
        assert!(ToolMappingService::is_tool_in_category(tool_names::COMPUTER, &ToolCategory::AnthropicComputerUse)); // Test production tool instead of dev_left_click
    }

    #[test]
    fn test_agent_capability() {
        assert!(ToolMappingService::can_agent_handle_tool(tool_names::BROWSER_NAVIGATE, &AgentType::BrowserExpert));
        assert!(!ToolMappingService::can_agent_handle_tool(tool_names::BROWSER_NAVIGATE, &AgentType::DesktopExpert));
        assert!(ToolMappingService::can_agent_handle_tool(tool_names::COMPUTER, &AgentType::DesktopExpert)); // Test production tool instead of dev_left_click
    }

    #[test]
    fn test_confidence_scoring() {
        assert_eq!(ToolMappingService::get_agent_confidence_for_tool(tool_names::BROWSER_NAVIGATE, &AgentType::BrowserExpert), confidence_scores::HIGH_CONFIDENCE);
        assert_eq!(ToolMappingService::get_agent_confidence_for_tool(tool_names::BROWSER_NAVIGATE, &AgentType::DesktopExpert), confidence_scores::NO_CONFIDENCE);
        assert!(ToolMappingService::get_agent_confidence_for_tool(tool_names::SCREENSHOT, &AgentType::BrowserExpert) > confidence_scores::NO_CONFIDENCE);
    }
}

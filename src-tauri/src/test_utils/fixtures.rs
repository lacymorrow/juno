/// Test fixtures for Juno AI Computer Use Agent
/// 
/// This module provides pre-configured test data and common test setups:
/// - Common agent responses and tool calls
/// - System state fixtures
/// - Error scenarios
/// - Performance test data
/// - Security test vectors

use std::collections::HashMap;
use serde_json::{json, Value};
use uuid::Uuid;

use crate::agent::structs::{AgentResponse, ToolCall};

/// Common agent response fixtures
pub struct AgentResponseFixtures;

impl AgentResponseFixtures {
    /// Successful screenshot response
    pub fn screenshot_success() -> AgentResponse {
        AgentResponse {
            content: "I've taken a screenshot of your screen. The image has been saved and is ready for analysis.".to_string(),
            tool_calls: vec![
                ToolCall {
                    id: "call_screenshot_001".to_string(),
                    name: "screenshot".to_string(),
                    input: json!({"display": 1}),
                }
            ],
            conversation_id: Some("conv_screenshot_test".to_string()),
            message_id: Some("msg_screenshot_001".to_string()),
            success: true,
            error_message: None,
            execution_time_ms: Some(250),
            tokens_used: Some(75),
        }
    }

    /// File operation response
    pub fn file_operation_success() -> AgentResponse {
        AgentResponse {
            content: "I've successfully created the file 'notes.txt' with your content.".to_string(),
            tool_calls: vec![
                ToolCall {
                    id: "call_write_file_001".to_string(),
                    name: "write_file".to_string(),
                    input: json!({
                        "path": "notes.txt",
                        "content": "Meeting notes from today's discussion"
                    }),
                }
            ],
            conversation_id: Some("conv_file_test".to_string()),
            message_id: Some("msg_file_001".to_string()),
            success: true,
            error_message: None,
            execution_time_ms: Some(120),
            tokens_used: Some(45),
        }
    }

    /// Multi-step workflow response
    pub fn multi_step_workflow() -> AgentResponse {
        AgentResponse {
            content: "I'll help you search for information and create a summary. Let me start by taking a screenshot, then opening your browser, searching for the topic, and creating a document with the findings.".to_string(),
            tool_calls: vec![
                ToolCall {
                    id: "call_workflow_001".to_string(),
                    name: "screenshot".to_string(),
                    input: json!({"display": 1}),
                },
                ToolCall {
                    id: "call_workflow_002".to_string(),
                    name: "open_app".to_string(),
                    input: json!({"name": "Chrome"}),
                },
                ToolCall {
                    id: "call_workflow_003".to_string(),
                    name: "navigate".to_string(),
                    input: json!({"url": "https://google.com/search?q=artificial+intelligence+trends"}),
                },
            ],
            conversation_id: Some("conv_workflow_test".to_string()),
            message_id: Some("msg_workflow_001".to_string()),
            success: true,
            error_message: None,
            execution_time_ms: Some(1500),
            tokens_used: Some(200),
        }
    }

    /// Permission denied error
    pub fn permission_denied_error() -> AgentResponse {
        AgentResponse {
            content: "I'm unable to complete this action because I don't have the necessary permissions.".to_string(),
            tool_calls: Vec::new(),
            conversation_id: Some("conv_permission_test".to_string()),
            message_id: Some("msg_permission_001".to_string()),
            success: false,
            error_message: Some("Permission denied: Accessibility access is required for desktop automation".to_string()),
            execution_time_ms: Some(50),
            tokens_used: Some(30),
        }
    }

    /// Tool execution timeout error
    pub fn timeout_error() -> AgentResponse {
        AgentResponse {
            content: "The operation timed out and couldn't be completed within the expected time.".to_string(),
            tool_calls: vec![
                ToolCall {
                    id: "call_timeout_001".to_string(),
                    name: "slow_operation".to_string(),
                    input: json!({"timeout_ms": 30000}),
                }
            ],
            conversation_id: Some("conv_timeout_test".to_string()),
            message_id: Some("msg_timeout_001".to_string()),
            success: false,
            error_message: Some("Operation timed out after 30 seconds".to_string()),
            execution_time_ms: Some(30000),
            tokens_used: Some(40),
        }
    }

    /// Invalid input error
    pub fn invalid_input_error() -> AgentResponse {
        AgentResponse {
            content: "I couldn't process your request due to invalid input parameters.".to_string(),
            tool_calls: Vec::new(),
            conversation_id: Some("conv_invalid_test".to_string()),
            message_id: Some("msg_invalid_001".to_string()),
            success: false,
            error_message: Some("Invalid input: coordinates must be positive integers".to_string()),
            execution_time_ms: Some(25),
            tokens_used: Some(35),
        }
    }
}

/// Tool call fixtures
pub struct ToolCallFixtures;

impl ToolCallFixtures {
    /// Basic click tool call
    pub fn click_action(x: i32, y: i32) -> ToolCall {
        ToolCall {
            id: format!("call_click_{}", Uuid::new_v4().to_string()[..8]),
            name: "click".to_string(),
            input: json!({
                "x": x,
                "y": y,
                "button": "left"
            }),
        }
    }

    /// Type text tool call
    pub fn type_text(text: &str) -> ToolCall {
        ToolCall {
            id: format!("call_type_{}", Uuid::new_v4().to_string()[..8]),
            name: "type".to_string(),
            input: json!({
                "text": text
            }),
        }
    }

    /// Screenshot tool call
    pub fn screenshot() -> ToolCall {
        ToolCall {
            id: format!("call_screenshot_{}", Uuid::new_v4().to_string()[..8]),
            name: "screenshot".to_string(),
            input: json!({
                "display": 1
            }),
        }
    }

    /// File read tool call
    pub fn read_file(path: &str) -> ToolCall {
        ToolCall {
            id: format!("call_read_{}", Uuid::new_v4().to_string()[..8]),
            name: "read_file".to_string(),
            input: json!({
                "path": path
            }),
        }
    }

    /// File write tool call
    pub fn write_file(path: &str, content: &str) -> ToolCall {
        ToolCall {
            id: format!("call_write_{}", Uuid::new_v4().to_string()[..8]),
            name: "write_file".to_string(),
            input: json!({
                "path": path,
                "content": content
            }),
        }
    }

    /// Browser navigation tool call
    pub fn navigate(url: &str) -> ToolCall {
        ToolCall {
            id: format!("call_navigate_{}", Uuid::new_v4().to_string()[..8]),
            name: "navigate".to_string(),
            input: json!({
                "url": url
            }),
        }
    }

    /// Open application tool call
    pub fn open_app(app_name: &str) -> ToolCall {
        ToolCall {
            id: format!("call_open_{}", Uuid::new_v4().to_string()[..8]),
            name: "open_app".to_string(),
            input: json!({
                "name": app_name
            }),
        }
    }

    /// Scroll action tool call
    pub fn scroll(direction: &str, amount: i32) -> ToolCall {
        ToolCall {
            id: format!("call_scroll_{}", Uuid::new_v4().to_string()[..8]),
            name: "scroll".to_string(),
            input: json!({
                "direction": direction,
                "amount": amount
            }),
        }
    }

    /// Search tool call
    pub fn search(query: &str) -> ToolCall {
        ToolCall {
            id: format!("call_search_{}", Uuid::new_v4().to_string()[..8]),
            name: "search".to_string(),
            input: json!({
                "query": query,
                "engine": "google"
            }),
        }
    }

    /// Tool call with invalid parameters (for error testing)
    pub fn invalid_parameters() -> ToolCall {
        ToolCall {
            id: "call_invalid_001".to_string(),
            name: "click".to_string(),
            input: json!({
                "x": -100,  // Invalid negative coordinate
                "y": "invalid",  // Invalid string instead of number
                "button": "invalid_button"  // Invalid button name
            }),
        }
    }
}

/// System state fixtures
pub struct SystemStateFixtures;

impl SystemStateFixtures {
    /// Normal desktop state
    pub fn normal_desktop_state() -> HashMap<String, Value> {
        let mut state = HashMap::new();
        state.insert("screen_resolution".to_string(), json!([1920, 1080]));
        state.insert("focused_app".to_string(), json!("Chrome"));
        state.insert("running_apps".to_string(), json!([
            "Finder", "Chrome", "Terminal", "Visual Studio Code", "Slack"
        ]));
        state.insert("cpu_usage".to_string(), json!(25.5));
        state.insert("memory_usage".to_string(), json!(65.2));
        state.insert("network_connected".to_string(), json!(true));
        state.insert("permissions".to_string(), json!({
            "accessibility": true,
            "screen_recording": true,
            "microphone": false
        }));
        state
    }

    /// High resource usage state
    pub fn high_resource_state() -> HashMap<String, Value> {
        let mut state = HashMap::new();
        state.insert("screen_resolution".to_string(), json!([2560, 1440]));
        state.insert("focused_app".to_string(), json!("Photoshop"));
        state.insert("running_apps".to_string(), json!([
            "Finder", "Photoshop", "Chrome", "Final Cut Pro", "Xcode", "Docker Desktop"
        ]));
        state.insert("cpu_usage".to_string(), json!(89.3));
        state.insert("memory_usage".to_string(), json!(92.1));
        state.insert("network_connected".to_string(), json!(true));
        state.insert("permissions".to_string(), json!({
            "accessibility": true,
            "screen_recording": true,
            "microphone": true
        }));
        state
    }

    /// Limited permissions state
    pub fn limited_permissions_state() -> HashMap<String, Value> {
        let mut state = HashMap::new();
        state.insert("screen_resolution".to_string(), json!([1440, 900]));
        state.insert("focused_app".to_string(), json!("System Preferences"));
        state.insert("running_apps".to_string(), json!([
            "Finder", "System Preferences", "Safari"
        ]));
        state.insert("cpu_usage".to_string(), json!(15.2));
        state.insert("memory_usage".to_string(), json!(45.8));
        state.insert("network_connected".to_string(), json!(true));
        state.insert("permissions".to_string(), json!({
            "accessibility": false,
            "screen_recording": false,
            "microphone": false
        }));
        state
    }

    /// Offline/disconnected state
    pub fn offline_state() -> HashMap<String, Value> {
        let mut state = HashMap::new();
        state.insert("screen_resolution".to_string(), json!([1920, 1080]));
        state.insert("focused_app".to_string(), json!("TextEdit"));
        state.insert("running_apps".to_string(), json!([
            "Finder", "TextEdit", "Calculator"
        ]));
        state.insert("cpu_usage".to_string(), json!(8.5));
        state.insert("memory_usage".to_string(), json!(32.1));
        state.insert("network_connected".to_string(), json!(false));
        state.insert("permissions".to_string(), json!({
            "accessibility": true,
            "screen_recording": true,
            "microphone": false
        }));
        state
    }
}

/// Test query fixtures
pub struct QueryFixtures;

impl QueryFixtures {
    /// Simple single-action queries
    pub fn simple_queries() -> Vec<String> {
        vec![
            "Take a screenshot".to_string(),
            "What time is it?".to_string(),
            "Open Calculator".to_string(),
            "Close this window".to_string(),
            "Type 'Hello World'".to_string(),
            "Click at coordinates 100, 200".to_string(),
            "Scroll down".to_string(),
            "Show me the desktop".to_string(),
        ]
    }

    /// Medium complexity queries
    pub fn medium_queries() -> Vec<String> {
        vec![
            "Take a screenshot and save it to my desktop".to_string(),
            "Open Chrome and navigate to google.com".to_string(),
            "Create a new text file with today's date".to_string(),
            "Find and open the Calculator app".to_string(),
            "Search for 'weather forecast' in my browser".to_string(),
            "Copy the selected text and paste it in a new document".to_string(),
            "Adjust the volume to 50%".to_string(),
            "Switch to the next tab in my browser".to_string(),
        ]
    }

    /// Complex multi-step queries
    pub fn complex_queries() -> Vec<String> {
        vec![
            "Research 'artificial intelligence trends' online, summarize the key points, and create a document with the findings".to_string(),
            "Take screenshots of all open windows, organize them in a folder, and create a report with timestamps".to_string(),
            "Find all PDF files modified this week, sort them by date, and email me a summary".to_string(),
            "Monitor my email for the next 30 minutes, respond to any urgent messages, and schedule follow-ups".to_string(),
            "Create a backup of my important documents, compress them, and upload to cloud storage".to_string(),
            "Analyze the data in 'sales_report.xlsx', create charts, and prepare a presentation".to_string(),
        ]
    }

    /// Queries that should fail (for error testing)
    pub fn invalid_queries() -> Vec<String> {
        vec![
            "Delete all files on my computer".to_string(),  // Dangerous operation
            "Install malicious software".to_string(),       // Security threat
            "Access someone else's private files".to_string(), // Privacy violation
            "Execute arbitrary code: rm -rf /".to_string(),  // Command injection attempt
            "".to_string(),                                  // Empty query
            "a".repeat(10000),                              // Extremely long query
        ]
    }
}

/// Error scenario fixtures
pub struct ErrorFixtures;

impl ErrorFixtures {
    /// Permission denied scenarios
    pub fn permission_scenarios() -> Vec<(String, String)> {
        vec![
            ("Take a screenshot".to_string(), "Screen recording permission required".to_string()),
            ("Click at coordinates 100, 200".to_string(), "Accessibility permission required".to_string()),
            ("Record audio".to_string(), "Microphone permission required".to_string()),
            ("Monitor system events".to_string(), "Input monitoring permission required".to_string()),
        ]
    }

    /// Network error scenarios
    pub fn network_scenarios() -> Vec<(String, String)> {
        vec![
            ("Search online for information".to_string(), "Network connection unavailable".to_string()),
            ("Download file from URL".to_string(), "Unable to connect to server".to_string()),
            ("Send email notification".to_string(), "SMTP server unreachable".to_string()),
            ("Sync data to cloud".to_string(), "Cloud service temporarily unavailable".to_string()),
        ]
    }

    /// Resource constraint scenarios
    pub fn resource_scenarios() -> Vec<(String, String)> {
        vec![
            ("Process large video file".to_string(), "Insufficient memory available".to_string()),
            ("Run intensive analysis".to_string(), "CPU usage limit exceeded".to_string()),
            ("Save large document".to_string(), "Disk space full".to_string()),
            ("Open multiple applications".to_string(), "System resource limit reached".to_string()),
        ]
    }

    /// Input validation scenarios
    pub fn validation_scenarios() -> Vec<(String, String)> {
        vec![
            ("Click at coordinates -100, 200".to_string(), "Invalid coordinates: negative values not allowed".to_string()),
            ("Type text with null bytes".to_string(), "Invalid characters in input text".to_string()),
            ("Open file with path ../../../etc/passwd".to_string(), "Invalid file path: directory traversal detected".to_string()),
            ("Execute command with | rm -rf /".to_string(), "Dangerous command pattern detected".to_string()),
        ]
    }
}

/// Performance test fixtures
pub struct PerformanceFixtures;

impl PerformanceFixtures {
    /// Fast operations (should complete quickly)
    pub fn fast_operations() -> Vec<(String, u64)> {
        vec![
            ("Get current time".to_string(), 50),           // 50ms max
            ("Get focused window".to_string(), 100),        // 100ms max
            ("Check permissions".to_string(), 75),          // 75ms max
            ("Get system info".to_string(), 150),          // 150ms max
        ]
    }

    /// Medium operations
    pub fn medium_operations() -> Vec<(String, u64)> {
        vec![
            ("Take screenshot".to_string(), 500),          // 500ms max
            ("Open application".to_string(), 2000),        // 2s max
            ("Read small file".to_string(), 200),          // 200ms max
            ("Simple web search".to_string(), 1500),       // 1.5s max
        ]
    }

    /// Slow operations (acceptable longer duration)
    pub fn slow_operations() -> Vec<(String, u64)> {
        vec![
            ("Complex web scraping".to_string(), 10000),   // 10s max
            ("Large file processing".to_string(), 15000),  // 15s max
            ("System analysis".to_string(), 8000),         // 8s max
            ("Multi-step workflow".to_string(), 12000),    // 12s max
        ]
    }

    /// Memory usage expectations (in MB)
    pub fn memory_limits() -> HashMap<String, usize> {
        let mut limits = HashMap::new();
        limits.insert("baseline".to_string(), 100);        // 100MB baseline
        limits.insert("simple_operation".to_string(), 150); // 150MB for simple ops
        limits.insert("medium_operation".to_string(), 300); // 300MB for medium ops
        limits.insert("complex_operation".to_string(), 500); // 500MB for complex ops
        limits.insert("maximum_allowed".to_string(), 1000); // 1GB absolute maximum
        limits
    }
}

/// Test conversation fixtures
pub struct ConversationFixtures;

impl ConversationFixtures {
    /// Simple Q&A conversation
    pub fn simple_conversation() -> Vec<(String, String)> {
        vec![
            ("user".to_string(), "Hello, can you help me?".to_string()),
            ("assistant".to_string(), "Of course! I'm here to help you with computer tasks. What would you like me to do?".to_string()),
            ("user".to_string(), "Take a screenshot please".to_string()),
            ("assistant".to_string(), "I've taken a screenshot of your screen. The image is ready for you to view or save.".to_string()),
        ]
    }

    /// Multi-turn workflow conversation
    pub fn workflow_conversation() -> Vec<(String, String)> {
        vec![
            ("user".to_string(), "I need to research AI trends and create a summary".to_string()),
            ("assistant".to_string(), "I'll help you research AI trends and create a summary. Let me start by opening your browser and searching for relevant information.".to_string()),
            ("user".to_string(), "Great, also save the findings to a document".to_string()),
            ("assistant".to_string(), "I've found several articles about AI trends. I'm now creating a document with the key findings and saving it to your desktop.".to_string()),
            ("user".to_string(), "Can you also create a presentation from this?".to_string()),
            ("assistant".to_string(), "I'll create a presentation based on the research findings. Opening PowerPoint and organizing the content into slides.".to_string()),
        ]
    }

    /// Error handling conversation
    pub fn error_conversation() -> Vec<(String, String)> {
        vec![
            ("user".to_string(), "Take a screenshot".to_string()),
            ("assistant".to_string(), "I'm unable to take a screenshot because I don't have screen recording permissions. Would you like me to guide you through granting these permissions?".to_string()),
            ("user".to_string(), "Yes, please help me fix the permissions".to_string()),
            ("assistant".to_string(), "I'll open System Preferences and guide you to the Privacy & Security settings where you can grant screen recording access to this application.".to_string()),
        ]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_agent_response_fixtures() {
        let screenshot_response = AgentResponseFixtures::screenshot_success();
        assert!(screenshot_response.success);
        assert_eq!(screenshot_response.tool_calls.len(), 1);
        assert_eq!(screenshot_response.tool_calls[0].name, "screenshot");

        let error_response = AgentResponseFixtures::permission_denied_error();
        assert!(!error_response.success);
        assert!(error_response.error_message.is_some());
    }

    #[test]
    fn test_tool_call_fixtures() {
        let click_call = ToolCallFixtures::click_action(100, 200);
        assert_eq!(click_call.name, "click");
        assert_eq!(click_call.input["x"], 100);
        assert_eq!(click_call.input["y"], 200);

        let type_call = ToolCallFixtures::type_text("Hello World");
        assert_eq!(type_call.name, "type");
        assert_eq!(type_call.input["text"], "Hello World");
    }

    #[test]
    fn test_system_state_fixtures() {
        let normal_state = SystemStateFixtures::normal_desktop_state();
        assert!(normal_state.contains_key("screen_resolution"));
        assert!(normal_state.contains_key("permissions"));

        let limited_state = SystemStateFixtures::limited_permissions_state();
        let permissions = &limited_state["permissions"];
        assert_eq!(permissions["accessibility"], false);
    }

    #[test]
    fn test_query_fixtures() {
        let simple_queries = QueryFixtures::simple_queries();
        assert!(!simple_queries.is_empty());
        assert!(simple_queries.iter().all(|q| !q.is_empty()));

        let invalid_queries = QueryFixtures::invalid_queries();
        assert!(invalid_queries.contains(&"".to_string())); // Empty query should be in invalid list
    }

    #[test]
    fn test_error_fixtures() {
        let permission_errors = ErrorFixtures::permission_scenarios();
        assert!(!permission_errors.is_empty());
        
        for (query, expected_error) in &permission_errors {
            assert!(!query.is_empty());
            assert!(!expected_error.is_empty());
        }
    }

    #[test]
    fn test_performance_fixtures() {
        let fast_ops = PerformanceFixtures::fast_operations();
        assert!(fast_ops.iter().all(|(_, max_ms)| *max_ms < 1000)); // All fast ops should be under 1s

        let memory_limits = PerformanceFixtures::memory_limits();
        assert!(memory_limits["baseline"] < memory_limits["maximum_allowed"]);
    }

    #[test]
    fn test_conversation_fixtures() {
        let simple_conv = ConversationFixtures::simple_conversation();
        assert!(simple_conv.len() >= 2); // At least user and assistant messages
        
        // Check conversation flow
        assert_eq!(simple_conv[0].0, "user");
        assert_eq!(simple_conv[1].0, "assistant");
    }
}
/// Test data generators for Juno AI Computer Use Agent
/// 
/// This module provides generators for creating realistic test data:
/// - Agent queries and responses
/// - Tool calls and configurations
/// - System states and contexts
/// - User interactions and workflows

use fake::{Fake, Faker, faker::{name::en::Name, lorem::en::*}};
use rand::Rng;
use chrono::{DateTime, Utc};
use std::collections::HashMap;
use uuid::Uuid;

use crate::agent::structs::{AgentResponse, ToolCall};

/// Generate realistic agent queries for testing
pub fn generate_agent_queries(count: usize, complexity: QueryComplexity) -> Vec<String> {
    let mut queries = Vec::new();
    let mut rng = rand::thread_rng();

    for _ in 0..count {
        let query = match complexity {
            QueryComplexity::Simple => generate_simple_query(&mut rng),
            QueryComplexity::Medium => generate_medium_query(&mut rng),
            QueryComplexity::Complex => generate_complex_query(&mut rng),
            QueryComplexity::Mixed => {
                match rng.gen_range(0..3) {
                    0 => generate_simple_query(&mut rng),
                    1 => generate_medium_query(&mut rng),
                    _ => generate_complex_query(&mut rng),
                }
            }
        };
        queries.push(query);
    }

    queries
}

#[derive(Debug, Clone)]
pub enum QueryComplexity {
    Simple,
    Medium,
    Complex,
    Mixed,
}

fn generate_simple_query(rng: &mut impl Rng) -> String {
    let simple_queries = [
        "What time is it?",
        "Take a screenshot",
        "Open calculator",
        "What's the weather?",
        "Play music",
        "Check my calendar",
        "Show me my files",
        "Open settings",
        "Close this window",
        "Minimize all windows",
    ];
    
    simple_queries[rng.gen_range(0..simple_queries.len())].to_string()
}

fn generate_medium_query(rng: &mut impl Rng) -> String {
    let templates = [
        "Open {app} and {action}",
        "Search for '{term}' in {location}",
        "Create a new {document_type} with {content}",
        "Find and open the {file_type} file from {timeframe}",
        "Send a message to {contact} saying '{message}'",
        "Set a reminder for {time} to {task}",
        "Take a screenshot and save it as '{filename}'",
        "Check my {account} for {item}",
    ];

    let template = templates[rng.gen_range(0..templates.len())];
    fill_template(template, rng)
}

fn generate_complex_query(rng: &mut impl Rng) -> String {
    let templates = [
        "Search for '{term}' in my browser, open the first result, read the content, and create a summary document saved as '{filename}'",
        "Find all {file_type} files modified in the last {timeframe}, organize them into a folder called '{folder_name}', and send me a list via email",
        "Open {app1}, create a new {document_type}, research '{topic}' using {app2}, and compile the findings into the document",
        "Monitor my {account} for {activity}, take screenshots of any {condition}, and create a report with timestamps",
        "Analyze the data in '{filename}', create visualizations, and present the findings in a new presentation saved as '{output_name}'",
        "Set up a recurring workflow that checks {source} every {interval}, processes any new {item_type}, and notifies me of results",
    ];

    let template = templates[rng.gen_range(0..templates.len())];
    fill_template(template, rng)
}

fn fill_template(template: &str, rng: &mut impl Rng) -> String {
    let mut result = template.to_string();

    // Replace placeholders with realistic values
    let replacements = [
        ("{app}", get_random_app(rng).to_string()),
        ("{app1}", get_random_app(rng).to_string()),
        ("{app2}", get_random_app(rng).to_string()),
        ("{action}", get_random_action(rng).to_string()),
        ("{term}", get_random_search_term(rng).to_string()),
        ("{location}", get_random_location(rng).to_string()),
        ("{document_type}", get_random_document_type(rng).to_string()),
        ("{content}", get_random_content(rng)),
        ("{file_type}", get_random_file_type(rng).to_string()),
        ("{timeframe}", get_random_timeframe(rng).to_string()),
        ("{contact}", Name().fake::<String>()),
        ("{message}", Sentence(3..8).fake::<String>()),
        ("{time}", get_random_time(rng)),
        ("{task}", get_random_task(rng).to_string()),
        ("{filename}", get_random_filename(rng)),
        ("{account}", get_random_account(rng).to_string()),
        ("{item}", get_random_item(rng).to_string()),
        ("{topic}", get_random_topic(rng).to_string()),
        ("{folder_name}", get_random_folder_name(rng)),
        ("{activity}", get_random_activity(rng).to_string()),
        ("{condition}", get_random_condition(rng).to_string()),
        ("{output_name}", get_random_filename(rng)),
        ("{source}", get_random_source(rng).to_string()),
        ("{interval}", get_random_interval(rng).to_string()),
        ("{item_type}", get_random_item_type(rng).to_string()),
    ];

    for (placeholder, replacement) in &replacements {
        result = result.replace(placeholder, replacement);
    }

    result
}

fn get_random_app(rng: &mut impl Rng) -> &'static str {
    let apps = [
        "Chrome", "Safari", "Firefox", "Visual Studio Code", "Terminal", 
        "Finder", "Calculator", "Calendar", "Mail", "Notes", "TextEdit",
        "Photoshop", "Excel", "Word", "PowerPoint", "Slack", "Discord",
        "Spotify", "iTunes", "QuickTime", "Preview", "System Preferences",
    ];
    apps[rng.gen_range(0..apps.len())]
}

fn get_random_action(rng: &mut impl Rng) -> &'static str {
    let actions = [
        "navigate to google.com", "create a new document", "check for updates",
        "search for files", "open recent documents", "clear cache", "refresh the page",
        "zoom in", "save current work", "export data", "import settings",
    ];
    actions[rng.gen_range(0..actions.len())]
}

fn get_random_search_term(rng: &mut impl Rng) -> &'static str {
    let terms = [
        "machine learning", "web development", "data science", "artificial intelligence",
        "cryptocurrency", "climate change", "renewable energy", "space exploration",
        "quantum computing", "biotechnology", "cybersecurity", "blockchain",
    ];
    terms[rng.gen_range(0..terms.len())]
}

fn get_random_location(rng: &mut impl Rng) -> &'static str {
    let locations = [
        "my documents", "downloads folder", "desktop", "recent files", "cloud storage",
        "email", "browser history", "bookmarks", "notes app", "calendar",
    ];
    locations[rng.gen_range(0..locations.len())]
}

fn get_random_document_type(rng: &mut impl Rng) -> &'static str {
    let types = [
        "document", "spreadsheet", "presentation", "note", "report", "memo",
        "proposal", "invoice", "letter", "resume", "agenda", "checklist",
    ];
    types[rng.gen_range(0..types.len())]
}

fn get_random_content(rng: &mut impl Rng) -> String {
    let content_types = [
        "meeting notes", "project summary", "todo list", "research findings",
        "weekly report", "budget analysis", "client feedback", "progress update",
    ];
    content_types[rng.gen_range(0..content_types.len())].to_string()
}

fn get_random_file_type(rng: &mut impl Rng) -> &'static str {
    let types = [
        "PDF", "Word", "Excel", "PowerPoint", "text", "image", "video", "audio",
        "CSV", "JSON", "XML", "log", "zip", "Photoshop", "Illustrator",
    ];
    types[rng.gen_range(0..types.len())]
}

fn get_random_timeframe(rng: &mut impl Rng) -> &'static str {
    let timeframes = [
        "week", "month", "day", "hour", "year", "2 weeks", "3 days", "6 months",
        "yesterday", "last week", "this morning", "past 24 hours",
    ];
    timeframes[rng.gen_range(0..timeframes.len())]
}

fn get_random_time(rng: &mut impl Rng) -> String {
    let times = [
        "tomorrow at 9 AM", "next Friday", "in 2 hours", "tonight at 8 PM",
        "Monday morning", "end of the week", "next month", "in 30 minutes",
    ];
    times[rng.gen_range(0..times.len())].to_string()
}

fn get_random_task(rng: &mut impl Rng) -> &'static str {
    let tasks = [
        "call John", "submit report", "review proposal", "backup files",
        "update software", "schedule meeting", "pay bills", "buy groceries",
        "exercise", "take medication", "prepare presentation", "send follow-up email",
    ];
    tasks[rng.gen_range(0..tasks.len())]
}

fn get_random_filename(rng: &mut impl Rng) -> String {
    let prefixes = ["report", "summary", "notes", "analysis", "data", "backup", "project"];
    let suffixes = ["final", "v2", "draft", "complete", "updated", "2024", "backup"];
    let extensions = [".pdf", ".docx", ".xlsx", ".txt", ".png", ".jpg"];
    
    let prefix = prefixes[rng.gen_range(0..prefixes.len())];
    let suffix = suffixes[rng.gen_range(0..suffixes.len())];
    let extension = extensions[rng.gen_range(0..extensions.len())];
    
    format!("{}_{}{}", prefix, suffix, extension)
}

fn get_random_account(rng: &mut impl Rng) -> &'static str {
    let accounts = [
        "email", "calendar", "bank account", "social media", "cloud storage",
        "project management", "CRM", "analytics", "messaging", "notes",
    ];
    accounts[rng.gen_range(0..accounts.len())]
}

fn get_random_item(rng: &mut impl Rng) -> &'static str {
    let items = [
        "new messages", "updates", "notifications", "changes", "tasks",
        "appointments", "deadlines", "transactions", "mentions", "alerts",
    ];
    items[rng.gen_range(0..items.len())]
}

fn get_random_topic(rng: &mut impl Rng) -> &'static str {
    let topics = [
        "market trends", "competitor analysis", "customer feedback", "product features",
        "industry news", "best practices", "case studies", "research papers",
    ];
    topics[rng.gen_range(0..topics.len())]
}

fn get_random_folder_name(rng: &mut impl Rng) -> String {
    let adjectives = ["organized", "sorted", "archived", "backup", "important", "recent"];
    let nouns = ["files", "documents", "reports", "data", "projects", "resources"];
    
    let adjective = adjectives[rng.gen_range(0..adjectives.len())];
    let noun = nouns[rng.gen_range(0..nouns.len())];
    
    format!("{}_{}", adjective, noun)
}

fn get_random_activity(rng: &mut impl Rng) -> &'static str {
    let activities = [
        "new messages", "file changes", "login attempts", "transactions",
        "system updates", "user activity", "error logs", "performance metrics",
    ];
    activities[rng.gen_range(0..activities.len())]
}

fn get_random_condition(rng: &mut impl Rng) -> &'static str {
    let conditions = [
        "errors", "warnings", "unusual activity", "changes", "new entries",
        "threshold breaches", "anomalies", "patterns", "issues", "updates",
    ];
    conditions[rng.gen_range(0..conditions.len())]
}

fn get_random_source(rng: &mut impl Rng) -> &'static str {
    let sources = [
        "email", "RSS feed", "API", "database", "file system", "cloud storage",
        "web service", "monitoring system", "sensor data", "user input",
    ];
    sources[rng.gen_range(0..sources.len())]
}

fn get_random_interval(rng: &mut impl Rng) -> &'static str {
    let intervals = [
        "hour", "day", "week", "month", "15 minutes", "30 minutes",
        "2 hours", "6 hours", "12 hours", "2 days", "3 days",
    ];
    intervals[rng.gen_range(0..intervals.len())]
}

fn get_random_item_type(rng: &mut impl Rng) -> &'static str {
    let types = [
        "files", "messages", "updates", "notifications", "reports", "data",
        "tasks", "events", "transactions", "logs", "alerts", "requests",
    ];
    types[rng.gen_range(0..types.len())]
}

/// Generate realistic agent responses for testing
pub fn generate_agent_responses(count: usize) -> Vec<AgentResponse> {
    let mut responses = Vec::new();
    let mut rng = rand::thread_rng();

    for _ in 0..count {
        let success = rng.gen_bool(0.8); // 80% success rate
        let content = if success {
            generate_success_response(&mut rng)
        } else {
            generate_error_response(&mut rng)
        };

        let response = AgentResponse {
            content,
            tool_calls: generate_tool_calls(&mut rng, rng.gen_range(0..=3)),
            conversation_id: Some(Uuid::new_v4().to_string()),
            message_id: Some(Uuid::new_v4().to_string()),
            success,
            error_message: if success { None } else { Some("Test error".to_string()) },
            execution_time_ms: Some(rng.gen_range(50..5000)),
            tokens_used: Some(rng.gen_range(10..500)),
        };

        responses.push(response);
    }

    responses
}

fn generate_success_response(rng: &mut impl Rng) -> String {
    let templates = [
        "I've successfully {action}. {result}",
        "Done! I {action} and {additional_action}.",
        "Completed the task. {details}",
        "I've {action} as requested. {confirmation}",
        "Task completed successfully. {summary}",
    ];

    let actions = [
        "taken a screenshot", "opened the application", "created the file",
        "searched for the information", "sent the message", "updated the settings",
        "saved the document", "processed the data", "generated the report",
    ];

    let results = [
        "The file has been saved to your desktop",
        "You should see it in your default application",
        "The information is now available",
        "All changes have been applied",
        "The task is ready for your review",
    ];

    let template = templates[rng.gen_range(0..templates.len())];
    let action = actions[rng.gen_range(0..actions.len())];
    let result = results[rng.gen_range(0..results.len())];

    template
        .replace("{action}", action)
        .replace("{result}", result)
        .replace("{additional_action}", &actions[rng.gen_range(0..actions.len())])
        .replace("{details}", result)
        .replace("{confirmation}", result)
        .replace("{summary}", result)
}

fn generate_error_response(rng: &mut impl Rng) -> String {
    let error_messages = [
        "I encountered an error while trying to complete this task.",
        "Sorry, I wasn't able to complete that request.",
        "There was a problem accessing the required resource.",
        "I need additional permissions to complete this action.",
        "The requested file or application couldn't be found.",
        "The operation timed out. Please try again.",
        "I'm unable to perform this action in the current context.",
    ];

    error_messages[rng.gen_range(0..error_messages.len())].to_string()
}

/// Generate tool calls for testing
pub fn generate_tool_calls(rng: &mut impl Rng, count: usize) -> Vec<ToolCall> {
    let mut tool_calls = Vec::new();

    let tool_names = [
        "screenshot", "click", "type", "scroll", "navigate", "read_file",
        "write_file", "open_app", "close_app", "search", "create_folder",
    ];

    for _ in 0..count {
        let tool_name = tool_names[rng.gen_range(0..tool_names.len())];
        let input = generate_tool_parameters(tool_name, rng);

        let tool_call = ToolCall {
            id: Uuid::new_v4().to_string(),
            name: tool_name.to_string(),
            input: serde_json::Value::Object(input.into_iter().collect()),
        };

        tool_calls.push(tool_call);
    }

    tool_calls
}

fn generate_tool_parameters(tool_name: &str, rng: &mut impl Rng) -> HashMap<String, serde_json::Value> {
    let mut parameters = HashMap::new();

    match tool_name {
        "screenshot" => {
            parameters.insert("display".to_string(), serde_json::Value::Number(rng.gen_range(1..=3).into()));
        }
        "click" => {
            parameters.insert("x".to_string(), serde_json::Value::Number(rng.gen_range(0..1920).into()));
            parameters.insert("y".to_string(), serde_json::Value::Number(rng.gen_range(0..1080).into()));
        }
        "type" => {
            let text = Sentence(1..5).fake::<String>();
            parameters.insert("text".to_string(), serde_json::Value::String(text));
        }
        "scroll" => {
            parameters.insert("direction".to_string(), serde_json::Value::String("down".to_string()));
            parameters.insert("amount".to_string(), serde_json::Value::Number(rng.gen_range(1..10).into()));
        }
        "navigate" => {
            let url = format!("https://example{}.com", rng.gen_range(1..100));
            parameters.insert("url".to_string(), serde_json::Value::String(url));
        }
        "read_file" | "write_file" => {
            let filename = get_random_filename(rng);
            parameters.insert("path".to_string(), serde_json::Value::String(filename));
        }
        "open_app" | "close_app" => {
            let app = get_random_app(rng);
            parameters.insert("name".to_string(), serde_json::Value::String(app.to_string()));
        }
        "search" => {
            let query = get_random_search_term(rng);
            parameters.insert("query".to_string(), serde_json::Value::String(query.to_string()));
        }
        "create_folder" => {
            let folder_name = get_random_folder_name(rng);
            parameters.insert("name".to_string(), serde_json::Value::String(folder_name));
        }
        _ => {
            // Generic parameters for unknown tools
            parameters.insert("action".to_string(), serde_json::Value::String(tool_name.to_string()));
        }
    }

    parameters
}

/// Generate system context data for testing
pub fn generate_system_contexts(count: usize) -> Vec<SystemContextData> {
    let mut contexts = Vec::new();
    let mut rng = rand::thread_rng();

    for _ in 0..count {
        let context = SystemContextData {
            timestamp: Utc::now(),
            focused_app: Some(get_random_app(&mut rng).to_string()),
            screen_resolution: (rng.gen_range(1280..3840), rng.gen_range(720..2160)),
            cpu_usage: rng.gen_range(5.0..95.0),
            memory_usage: rng.gen_range(20.0..90.0),
            disk_usage: rng.gen_range(10.0..95.0),
            running_apps: generate_running_apps(&mut rng),
            network_connected: rng.gen_bool(0.9),
        };
        contexts.push(context);
    }

    contexts
}

#[derive(Debug, Clone)]
pub struct SystemContextData {
    pub timestamp: DateTime<Utc>,
    pub focused_app: Option<String>,
    pub screen_resolution: (u32, u32),
    pub cpu_usage: f64,
    pub memory_usage: f64,
    pub disk_usage: f64,
    pub running_apps: Vec<String>,
    pub network_connected: bool,
}

fn generate_running_apps(rng: &mut impl Rng) -> Vec<String> {
    let all_apps = [
        "Finder", "Chrome", "Safari", "Terminal", "Visual Studio Code",
        "Slack", "Mail", "Calendar", "Notes", "Calculator", "System Preferences",
        "Activity Monitor", "Disk Utility", "TextEdit", "Preview", "QuickTime",
    ];

    let app_count = rng.gen_range(5..12);
    let mut running_apps = Vec::new();

    for _ in 0..app_count {
        let app = all_apps[rng.gen_range(0..all_apps.len())];
        if !running_apps.contains(&app.to_string()) {
            running_apps.push(app.to_string());
        }
    }

    running_apps
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_generate_agent_queries() {
        let queries = generate_agent_queries(10, QueryComplexity::Simple);
        assert_eq!(queries.len(), 10);
        
        for query in &queries {
            assert!(!query.is_empty());
            assert!(query.len() < 100); // Simple queries should be short
        }
    }

    #[test]
    fn test_generate_complex_queries() {
        let queries = generate_agent_queries(5, QueryComplexity::Complex);
        assert_eq!(queries.len(), 5);
        
        for query in &queries {
            assert!(query.len() > 50); // Complex queries should be longer
        }
    }

    #[test]
    fn test_generate_agent_responses() {
        let responses = generate_agent_responses(10);
        assert_eq!(responses.len(), 10);
        
        let success_count = responses.iter().filter(|r| r.success).count();
        assert!(success_count >= 5); // Should have some successful responses
        
        for response in &responses {
            assert!(!response.content.is_empty());
            assert!(response.conversation_id.is_some());
            assert!(response.message_id.is_some());
        }
    }

    #[test]
    fn test_generate_tool_calls() {
        let mut rng = rand::thread_rng();
        let tool_calls = generate_tool_calls(&mut rng, 5);
        assert_eq!(tool_calls.len(), 5);
        
        for tool_call in &tool_calls {
            assert!(!tool_call.id.is_empty());
            assert!(!tool_call.name.is_empty());
            // Parameters may be empty for some tools
        }
    }

    #[test]
    fn test_generate_system_contexts() {
        let contexts = generate_system_contexts(3);
        assert_eq!(contexts.len(), 3);
        
        for context in &contexts {
            assert!(context.cpu_usage >= 0.0 && context.cpu_usage <= 100.0);
            assert!(context.memory_usage >= 0.0 && context.memory_usage <= 100.0);
            assert!(context.disk_usage >= 0.0 && context.disk_usage <= 100.0);
            assert!(!context.running_apps.is_empty());
        }
    }

    #[test]
    fn test_template_filling() {
        let mut rng = rand::thread_rng();
        let template = "Open {app} and {action}";
        let result = fill_template(template, &mut rng);
        
        assert!(!result.contains("{app}"));
        assert!(!result.contains("{action}"));
        assert!(result.starts_with("Open"));
    }
}
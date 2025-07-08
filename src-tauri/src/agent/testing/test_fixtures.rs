//! Test Fixtures and Stub Implementations
//!
//! Common test fixtures, stubs, and mock objects for testing

use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use async_trait::async_trait;

use crate::agent::events::{EventHandler, JunoAgentEvent};

/// Stub event handler for testing
pub struct StubEventHandler {
    pub name: String,
    pub event_types: Vec<String>,
    pub call_count: Arc<AtomicUsize>,
    pub should_fail: bool,
    pub processing_delay_ms: u64,
}

impl StubEventHandler {
    pub fn new(name: &str, event_types: Vec<&str>) -> Self {
        Self {
            name: name.to_string(),
            event_types: event_types.into_iter().map(|s| s.to_string()).collect(),
            call_count: Arc::new(AtomicUsize::new(0)),
            should_fail: false,
            processing_delay_ms: 0,
        }
    }

    pub fn with_failure(mut self, should_fail: bool) -> Self {
        self.should_fail = should_fail;
        self
    }

    pub fn with_delay(mut self, delay_ms: u64) -> Self {
        self.processing_delay_ms = delay_ms;
        self
    }

    pub fn get_call_count(&self) -> usize {
        self.call_count.load(Ordering::Relaxed)
    }

    pub fn reset_call_count(&self) {
        self.call_count.store(0, Ordering::Relaxed);
    }
}

#[async_trait]
impl EventHandler for StubEventHandler {
    async fn handle_event(&self, _event: &JunoAgentEvent) -> Result<Vec<JunoAgentEvent>, String> {
        self.call_count.fetch_add(1, Ordering::Relaxed);

        if self.processing_delay_ms > 0 {
            tokio::time::sleep(std::time::Duration::from_millis(self.processing_delay_ms)).await;
        }

        if self.should_fail {
            Err(format!("Simulated failure in handler '{}'", self.name))
        } else {
            Ok(vec![])
        }
    }

    fn event_types(&self) -> Vec<&'static str> {
        // This is a bit of a hack since we need to return static strings
        // In real tests, we'd typically use a different approach
        vec!["test_event"]
    }

    fn name(&self) -> &'static str {
        "StubEventHandler"
    }

    fn priority(&self) -> u8 {
        50
    }
}

// Additional test fixtures can be added here as needed
pub mod sample_data {
    use crate::agent::core::{Message, Role, ToolCall};
    use serde_json::json;

    pub fn simple_conversation() -> Vec<Message> {
        vec![
            Message {
                role: Role::User,
                content: "Hello".to_string(),
                tool_calls: None,
                tool_call_id: None,
                name: None,
            },
            Message {
                role: Role::Assistant,
                content: "Hi there! How can I help you?".to_string(),
                tool_calls: None,
                tool_call_id: None,
                name: None,
            },
        ]
    }

    pub fn conversation_with_tool_use() -> Vec<Message> {
        vec![
            Message {
                role: Role::User,
                content: "What's the weather like?".to_string(),
                tool_calls: None,
                tool_call_id: None,
                name: None,
            },
            Message {
                role: Role::Assistant,
                content: "I'll check the weather for you.".to_string(),
                tool_calls: Some(vec![ToolCall {
                    id: "weather_1".to_string(),
                    name: "get_weather".to_string(),
                    input: json!({"location": "current"}),
                }]),
                tool_call_id: None,
                name: None,
            },
            Message {
                role: Role::Tool,
                content: "Sunny, 22°C".to_string(),
                tool_calls: None,
                tool_call_id: Some("weather_1".to_string()),
                name: Some("get_weather".to_string()),
            },
            Message {
                role: Role::Assistant,
                content: "It's sunny and 22°C today!".to_string(),
                tool_calls: None,
                tool_call_id: None,
                name: None,
            },
        ]
    }
}
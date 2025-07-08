//! Event-Message Conversion System
//!
//! Provides bidirectional conversion between TARS event streams and Juno message formats.
//! Maintains semantic fidelity while enabling both event-driven and message-based access
//! to conversation history.

use serde_json::Value;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{debug, warn};

use crate::agent::core::{Message, Role, ToolCall};
use crate::agent::events::JunoAgentEvent;

/// Converts events to messages for backward compatibility
pub struct EventToMessageConverter {
    /// Cache for tool calls that are split across multiple events
    pending_tool_calls: HashMap<String, ToolCall>,
}

impl EventToMessageConverter {
    pub fn new() -> Self {
        Self {
            pending_tool_calls: HashMap::new(),
        }
    }
    
    /// Convert a sequence of events to a conversation message history
    pub async fn convert_events_to_messages(&self, events: &[JunoAgentEvent]) -> Result<Vec<Message>, String> {
        let mut messages = Vec::new();
        let mut pending_tool_calls: HashMap<String, ToolCall> = HashMap::new();
        let mut tool_results: HashMap<String, (String, Option<String>)> = HashMap::new(); // call_id -> (content, name)
        
        for event in events {
            match event {
                JunoAgentEvent::UserMessage { content, .. } => {
                    messages.push(Message {
                        role: Role::User,
                        content: content.clone(),
                        tool_calls: None,
                        tool_call_id: None,
                        name: None,
                    });
                }
                
                JunoAgentEvent::AssistantMessage { content, .. } => {
                    messages.push(Message {
                        role: Role::Assistant,
                        content: content.clone(),
                        tool_calls: None,
                        tool_call_id: None,
                        name: None,
                    });
                }
                
                JunoAgentEvent::AssistantStreamingMessage { content, is_partial, .. } => {
                    // Only add final streaming messages
                    if !is_partial {
                        messages.push(Message {
                            role: Role::Assistant,
                            content: content.clone(),
                            tool_calls: None,
                            tool_call_id: None,
                            name: None,
                        });
                    }
                }
                
                JunoAgentEvent::ToolCall { tool_name, id, args, .. } => {
                    // Convert input Value to arguments format expected by ToolCall
                    let tool_call = ToolCall {
                        id: id.clone(),
                        name: tool_name.clone(),
                        input: args.clone(),
                    };
                    
                    pending_tool_calls.insert(id.clone(), tool_call);
                }
                
                JunoAgentEvent::ToolResult { tool_call_id, result, success, .. } => {
                    // Find the corresponding tool call
                    if let Some(tool_call) = pending_tool_calls.remove(tool_call_id) {
                        // Create assistant message with tool call
                        let assistant_message = Message {
                            role: Role::Assistant,
                            content: "Using tool".to_string(), // Generic content for tool usage
                            tool_calls: Some(vec![tool_call.clone()]),
                            tool_call_id: None,
                            name: None,
                        };
                        messages.push(assistant_message);
                        
                        // Create tool result message
                        let result_content = if *success {
                            match serde_json::to_string_pretty(result) {
                                Ok(json_str) => json_str,
                                Err(_) => result.to_string(),
                            }
                        } else {
                            format!("Tool execution failed: {}", result.as_str().unwrap_or("Unknown error"))
                        };
                        
                        let tool_result_message = Message {
                            role: Role::Tool,
                            content: result_content,
                            tool_calls: None,
                            tool_call_id: Some(tool_call_id.clone()),
                            name: Some(tool_call.name.clone()),
                        };
                        messages.push(tool_result_message);
                    } else {
                        // Store tool result for later matching
                        let result_content = if *success {
                            match serde_json::to_string_pretty(result) {
                                Ok(json_str) => json_str,
                                Err(_) => result.to_string(),
                            }
                        } else {
                            format!("Tool execution failed: {}", result.as_str().unwrap_or("Unknown error"))
                        };
                        
                        tool_results.insert(tool_call_id.clone(), (result_content, None));
                    }
                }
                
                JunoAgentEvent::SystemMessage { message, level, .. } => {
                    // Convert system messages to user messages with special formatting
                    let formatted_content = format!("[SYSTEM {}] {}", level.to_uppercase(), message);
                    messages.push(Message {
                        role: Role::User,
                        content: formatted_content,
                        tool_calls: None,
                        tool_call_id: None,
                        name: Some("system".to_string()),
                    });
                }
                
                JunoAgentEvent::ErrorOccurred { error_type, message, .. } => {
                    // Convert errors to system messages
                    let formatted_content = format!("[ERROR {}] {}", error_type, message);
                    messages.push(Message {
                        role: Role::User,
                        content: formatted_content,
                        tool_calls: None,
                        tool_call_id: None,
                        name: Some("system".to_string()),
                    });
                }
                
                // Voice events are typically not included in conversation history
                // but could be converted if needed for debugging
                JunoAgentEvent::VoiceTranscriptionEnd { final_text, .. } => {
                    if !final_text.is_empty() {
                        messages.push(Message {
                            role: Role::User,
                            content: final_text.clone(),
                            tool_calls: None,
                            tool_call_id: None,
                            name: Some("voice".to_string()),
                        });
                    }
                }
                
                // Other events don't typically map to conversation messages
                _ => {
                    debug!("Skipping event type in message conversion: {:?}", event);
                }
            }
        }
        
        // Handle any remaining pending tool calls or results
        for (call_id, tool_call) in pending_tool_calls {
            warn!("Found orphaned tool call: {}", call_id);
            
            // Create assistant message with tool call
            let assistant_message = Message {
                role: Role::Assistant,
                content: "Using tool".to_string(),
                tool_calls: Some(vec![tool_call.clone()]),
                tool_call_id: None,
                name: None,
            };
            messages.push(assistant_message);
            
            // Check if we have a result for this call
            if let Some((result_content, _)) = tool_results.remove(&call_id) {
                let tool_result_message = Message {
                    role: Role::Tool,
                    content: result_content,
                    tool_calls: None,
                    tool_call_id: Some(call_id),
                    name: Some(tool_call.name),
                };
                messages.push(tool_result_message);
            }
        }
        
        // Handle any remaining orphaned tool results
        for (call_id, (result_content, tool_name)) in tool_results {
            warn!("Found orphaned tool result: {}", call_id);
            
            let tool_result_message = Message {
                role: Role::Tool,
                content: result_content,
                tool_calls: None,
                tool_call_id: Some(call_id),
                name: tool_name,
            };
            messages.push(tool_result_message);
        }
        
        debug!("Converted {} events to {} messages", events.len(), messages.len());
        Ok(messages)
    }
    
    /// Convert a single event to a message if applicable
    pub async fn convert_event_to_message(&self, event: &JunoAgentEvent) -> Result<Option<Message>, String> {
        let messages = self.convert_events_to_messages(&[event.clone()]).await?;
        Ok(messages.into_iter().next())
    }
}

/// Converts messages to events for event-driven processing
pub struct MessageToEventConverter {
    /// Session tracking for event correlation
    current_session_id: Arc<RwLock<Option<String>>>,
}

impl MessageToEventConverter {
    pub fn new() -> Self {
        Self {
            current_session_id: Arc::new(RwLock::new(None)),
        }
    }
    
    /// Set the current session ID for event generation
    pub async fn set_session_id(&self, session_id: String) {
        let mut session = self.current_session_id.write().await;
        *session = Some(session_id);
    }
    
    /// Convert a message to corresponding events
    pub async fn convert_message_to_events(&self, message: &Message) -> Result<Vec<JunoAgentEvent>, String> {
        let mut events = Vec::new();
        let timestamp = chrono::Utc::now().timestamp_millis() as u64;
        let session_id = self.current_session_id.read().await.clone();
        
        match message.role {
            Role::User => {
                events.push(JunoAgentEvent::UserMessage {
                    content: message.content.clone(),
                    timestamp,
                    session_id: session_id.clone(),
                });
            }
            
            Role::Assistant => {
                // If the message has tool calls, emit them as separate events
                if let Some(tool_calls) = &message.tool_calls {
                    for tool_call in tool_calls {
                        events.push(JunoAgentEvent::ToolCall {
                            tool_name: tool_call.name.clone(),
                            id: tool_call.id.clone(),
                            args: tool_call.input.clone(),
                            timestamp,
                            session_id: session_id.clone(),
                        });
                    }
                }
                
                // Always emit the assistant message (even if it has tool calls)
                events.push(JunoAgentEvent::AssistantMessage {
                    content: message.content.clone(),
                    timestamp,
                    session_id: session_id.clone(),
                });
            }
            
            Role::Tool => {
                // Convert tool result to ToolResult event
                let tool_call_id = message.tool_call_id.clone()
                    .unwrap_or_else(|| "unknown".to_string());
                
                // Try to parse the content as JSON, otherwise use as text
                let output = match serde_json::from_str::<Value>(&message.content) {
                    Ok(json_value) => json_value,
                    Err(_) => Value::String(message.content.clone()),
                };
                
                // Determine success based on content (heuristic)
                let success = !message.content.contains("error") && 
                             !message.content.contains("failed") &&
                             !message.content.contains("Error") &&
                             !message.content.contains("Failed");
                
                events.push(JunoAgentEvent::ToolResult {
                    tool_call_id,
                    result: output,
                    success,
                    execution_time_ms: None,
                    timestamp,
                });
            }
            
            Role::System => {
                // Convert system messages to SystemMessage events
                events.push(JunoAgentEvent::SystemMessage {
                    level: "info".to_string(),
                    message: message.content.clone(),
                    category: Some("system".to_string()),
                    timestamp,
                });
            }
        }
        
        debug!("Converted message to {} events", events.len());
        Ok(events)
    }
    
    /// Convert a single message to a single primary event
    pub async fn convert_message_to_event(&self, message: &Message) -> Result<JunoAgentEvent, String> {
        let events = self.convert_message_to_events(message).await?;
        
        // Return the most representative event
        events.into_iter().next()
            .ok_or_else(|| "No events generated from message".to_string())
    }
    
    /// Convert a sequence of messages to events
    pub async fn convert_messages_to_events(&self, messages: &[Message]) -> Result<Vec<JunoAgentEvent>, String> {
        let mut all_events = Vec::new();
        
        for message in messages {
            let events = self.convert_message_to_events(message).await?;
            all_events.extend(events);
        }
        
        debug!("Converted {} messages to {} events", messages.len(), all_events.len());
        Ok(all_events)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    
    #[tokio::test]
    async fn test_user_message_conversion() {
        let converter = MessageToEventConverter::new();
        
        let message = Message {
            role: Role::User,
            content: "Hello, AI!".to_string(),
            tool_calls: None,
            tool_call_id: None,
            name: None,
        };
        
        let event = converter.convert_message_to_event(&message).await.unwrap();
        
        match event {
            JunoAgentEvent::UserMessage { content, .. } => {
                assert_eq!(content, "Hello, AI!");
            }
            _ => panic!("Expected UserMessage event"),
        }
    }
    
    #[tokio::test]
    async fn test_tool_call_conversion() {
        let converter = MessageToEventConverter::new();
        
        let tool_call = ToolCall {
            id: "call_123".to_string(),
            name: "test_tool".to_string(),
            input: json!({"param": "value"}),
        };
        
        let message = Message {
            role: Role::Assistant,
            content: "Using test tool".to_string(),
            tool_calls: Some(vec![tool_call]),
            tool_call_id: None,
            name: None,
        };
        
        let events = converter.convert_message_to_events(&message).await.unwrap();
        assert_eq!(events.len(), 2); // ToolCall + AssistantMessage
        
        // Check first event is ToolCall
        match &events[0] {
            JunoAgentEvent::ToolCall { tool_name, id, .. } => {
                assert_eq!(tool_name, "test_tool");
                assert_eq!(id, "call_123");
            }
            _ => panic!("Expected ToolCall event"),
        }
        
        // Check second event is AssistantMessage
        match &events[1] {
            JunoAgentEvent::AssistantMessage { content, .. } => {
                assert_eq!(content, "Using test tool");
            }
            _ => panic!("Expected AssistantMessage event"),
        }
    }
    
    #[tokio::test]
    async fn test_event_to_message_conversion() {
        let converter = EventToMessageConverter::new();
        
        let events = vec![
            JunoAgentEvent::UserMessage {
                content: "Hello".to_string(),
                timestamp: 123456789,
                session_id: None,
            },
            JunoAgentEvent::AssistantMessage {
                content: "Hi there!".to_string(),
                timestamp: 123456790,
                session_id: None,
            },
        ];
        
        let messages = converter.convert_events_to_messages(&events).await.unwrap();
        assert_eq!(messages.len(), 2);
        
        assert_eq!(messages[0].role, Role::User);
        assert_eq!(messages[0].content, "Hello");
        
        assert_eq!(messages[1].role, Role::Assistant);
        assert_eq!(messages[1].content, "Hi there!");
    }
    
    #[tokio::test]
    async fn test_tool_call_and_result_conversion() {
        let converter = EventToMessageConverter::new();
        
        let events = vec![
            JunoAgentEvent::ToolCall {
                tool_name: "calculator".to_string(),
                id: "calc_001".to_string(),
                args: json!({"operation": "add", "a": 2, "b": 3}),
                timestamp: 123456789,
                session_id: None,
            },
            JunoAgentEvent::ToolResult {
                tool_call_id: "calc_001".to_string(),
                result: json!({"result": 5}),
                success: true,
                execution_time_ms: Some(100),
                timestamp: 123456790,
            },
        ];
        
        let messages = converter.convert_events_to_messages(&events).await.unwrap();
        assert_eq!(messages.len(), 2);
        
        // First message should be assistant with tool call
        assert_eq!(messages[0].role, Role::Assistant);
        assert!(messages[0].tool_calls.is_some());
        assert_eq!(messages[0].tool_calls.as_ref().unwrap()[0].name, "calculator");
        
        // Second message should be tool result
        assert_eq!(messages[1].role, Role::Tool);
        assert_eq!(messages[1].tool_call_id, Some("calc_001".to_string()));
        assert!(messages[1].content.contains("5"));
    }
}
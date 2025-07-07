use async_trait::async_trait;
use tracing::{info, warn};

use crate::agent::events::{EventHandler, JunoAgentEvent, generate_session_id, now};

/// Handles user input events and converts them into agent execution requests
pub struct UserInputHandler {
    /// Maximum query length to process
    max_query_length: usize,
}

impl UserInputHandler {
    pub fn new() -> Self {
        Self {
            max_query_length: 10000, // Reasonable limit for user queries
        }
    }
    
    /// Validate and clean user input
    fn validate_and_clean_input(&self, input: &str) -> Result<String, String> {
        let cleaned = input.trim();
        
        if cleaned.is_empty() {
            return Err("Empty query not allowed".to_string());
        }
        
        if cleaned.len() > self.max_query_length {
            return Err(format!("Query too long (max {} characters)", self.max_query_length));
        }
        
        // Basic content validation
        if cleaned.chars().all(|c| c.is_whitespace()) {
            return Err("Query contains only whitespace".to_string());
        }
        
        Ok(cleaned.to_string())
    }
}

impl Default for UserInputHandler {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl EventHandler for UserInputHandler {
    async fn handle_event(&self, event: &JunoAgentEvent) -> Result<Vec<JunoAgentEvent>, String> {
        match event {
            JunoAgentEvent::UserMessage { content, session_id, .. } => {
                info!("Processing user input: {}", content);
                
                // Validate and clean the input
                if let Err(e) = self.validate_and_clean_input(content) {
                    warn!("Invalid user input: {}", e);
                    return Ok(vec![
                        JunoAgentEvent::ErrorOccurred {
                            error_type: "invalid_input".to_string(),
                            message: e,
                            recoverable: true,
                            timestamp: now(),
                            context: Some(serde_json::json!({
                                "original_content": content,
                                "session_id": session_id
                            })),
                        }
                    ]);
                }
                
                // Generate session ID if none provided
                let session_id = session_id.clone().unwrap_or_else(generate_session_id);
                
                // Create agent run start event with the validated user query
                Ok(vec![
                    JunoAgentEvent::AgentRunStart {
                        session_id,
                        agent_type: "orchestrator".to_string(),
                        max_iterations: 15, // Use constant from agent config
                        user_query: content.clone(), // Include the original user query
                        timestamp: now(),
                    }
                ])
            }
            _ => {
                // This handler only processes UserMessage events
                Ok(vec![])
            }
        }
    }
    
    fn event_types(&self) -> Vec<&'static str> {
        vec!["user_message"]
    }
    
    fn name(&self) -> &'static str {
        "UserInputHandler"
    }
    
    fn priority(&self) -> u8 {
        100 // High priority - process user input first
    }
}
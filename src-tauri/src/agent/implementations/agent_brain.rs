use async_trait::async_trait;

use crate::agent::structs::{
    AgentAction, AgentError, Message, Role, ToolDefinition,
};
use crate::agent::traits::AgentBrain;

// --- SimpleBrain Implementation (Example) --- //
// This is a very basic brain that just echoes back the last user message or a canned response.

#[derive(Clone)]
pub struct SimpleBrain;

impl SimpleBrain {
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl AgentBrain for SimpleBrain {
    async fn decide_next_action(
        &self,
        messages: &[Message],
        _available_tools: &[ToolDefinition],
    ) -> Result<AgentAction, AgentError> {
        let last_user_message = messages
            .iter()
            .filter(|m| m.role == Role::User)
            .last();

        if let Some(msg) = last_user_message {
            Ok(AgentAction::Finish(format!(
                "SimpleBrain echoes: {}",
                msg.content
            )))
        } else {
            Ok(AgentAction::Finish(
                "SimpleBrain: No user message to echo.".to_string(),
            ))
        }
    }
}

impl Default for SimpleBrain {
    fn default() -> Self {
        Self::new()
    }
}

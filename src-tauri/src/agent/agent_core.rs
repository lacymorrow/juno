#![allow(dead_code)] // TEMP: Remove later

use super::memory::{Message, Role}; // Import memory structures

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AgentState {
    Running,
    Finished,
    Failed,
}

#[derive(Debug)]
pub struct Agent {
    state: AgentState,
    memory: Vec<Message>, // Add the memory field
    max_steps: u32,
    current_step: u32,
}

impl Agent {
    pub fn new(max_steps: u32) -> Self {
        Agent {
            state: AgentState::Running,
            memory: Vec::new(), // Initialize memory
            max_steps,
            current_step: 0,
        }
    }

    pub fn get_state(&self) -> &AgentState {
        &self.state
    }

    pub fn get_memory(&self) -> &Vec<Message> {
        &self.memory
    }

    // Placeholder for the main loop
    // pub fn run(&mut self, initial_prompt: String) -> Result<String, String> {
    //     // Add initial prompt to memory
    //     self.memory.push(Message {
    //         role: Role::User,
    //         content: initial_prompt,
    //         tool_calls: None,
    //         tool_call_id: None,
    //         name: None,
    //     });
    //
    //     // Implementation to follow in later steps
    //     Ok("Agent finished".to_string())
    // }

    // Placeholder for the think phase
    // fn think(&mut self) -> bool {
    //     // Implementation to follow in later steps
    //     false // Return true if action is needed
    // }

    // Placeholder for the act phase
    // fn act(&mut self) {
    //     // Implementation to follow in later steps
    // }
}

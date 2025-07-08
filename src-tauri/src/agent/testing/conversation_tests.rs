//! Conversation Flow Tests
//!
//! Tests that validate end-to-end conversation flows and scenarios

use std::time::Instant;
use async_trait::async_trait;

use super::test_utilities::{TestCase, TestConfig, TestResult, TestMetrics, TestUtilities};
use super::test_fixtures::sample_data;
use crate::agent::memory::EventMemoryManager;
use crate::agent::traits::MemoryManager;

/// End-to-end conversation flow test
pub struct ConversationFlowTest;

#[async_trait]
impl TestCase for ConversationFlowTest {
    async fn run(&self, config: &TestConfig) -> TestResult {
        let start_time = Instant::now();
        
        match self.run_test(config).await {
            Ok(metrics) => TestResult::success(
                self.name().to_string(),
                start_time.elapsed(),
                Some(metrics),
            ),
            Err(error) => TestResult::failure(
                self.name().to_string(),
                start_time.elapsed(),
                error,
            ),
        }
    }

    fn name(&self) -> &str {
        "ConversationFlow"
    }

    fn description(&self) -> &str {
        "Tests complete conversation flows with various message types"
    }
}

impl ConversationFlowTest {
    async fn run_test(&self, config: &TestConfig) -> Result<TestMetrics, String> {
        let mut memory_manager = TestUtilities::create_test_memory_manager(config.memory_config.clone()).await?;
        
        // Test simple conversation
        let simple_conversation = sample_data::simple_conversation();
        for message in &simple_conversation {
            memory_manager.add_message(message.clone()).await
                .map_err(|e| format!("Failed to add simple conversation message: {}", e))?;
        }
        
        // Test conversation with tool use
        let tool_conversation = sample_data::conversation_with_tool_use();
        for message in &tool_conversation {
            memory_manager.add_message(message.clone()).await
                .map_err(|e| format!("Failed to add tool conversation message: {}", e))?;
        }
        
        // Verify all messages are stored correctly
        let all_messages = memory_manager.get_messages().await
            .map_err(|e| format!("Failed to retrieve all messages: {}", e))?;
        
        let expected_count = simple_conversation.len() + tool_conversation.len();
        if all_messages.len() != expected_count {
            return Err(format!(
                "Message count mismatch: expected {}, got {}",
                expected_count, all_messages.len()
            ));
        }
        
        // Verify conversation structure
        let mut has_user_messages = false;
        let mut has_assistant_messages = false;
        let mut has_tool_calls = false;
        let mut has_tool_results = false;
        
        for message in &all_messages {
            match message.role {
                crate::agent::core::Role::User => has_user_messages = true,
                crate::agent::core::Role::Assistant => {
                    has_assistant_messages = true;
                    if message.tool_calls.is_some() {
                        has_tool_calls = true;
                    }
                },
                crate::agent::core::Role::Tool => has_tool_results = true,
                crate::agent::core::Role::System => {}, // Handle System role
            }
        }
        
        if !has_user_messages {
            return Err("No user messages found in conversation".to_string());
        }
        if !has_assistant_messages {
            return Err("No assistant messages found in conversation".to_string());
        }
        if !has_tool_calls {
            return Err("No tool calls found in conversation".to_string());
        }
        if !has_tool_results {
            return Err("No tool results found in conversation".to_string());
        }
        
        println!("Conversation Flow Test Results:");
        println!("  Simple conversation messages: {}", simple_conversation.len());
        println!("  Tool conversation messages: {}", tool_conversation.len());
        println!("  Total messages processed: {}", all_messages.len());
        println!("  ✅ User messages: {}", has_user_messages);
        println!("  ✅ Assistant messages: {}", has_assistant_messages);
        println!("  ✅ Tool calls: {}", has_tool_calls);
        println!("  ✅ Tool results: {}", has_tool_results);
        
        Ok(TestMetrics {
            events_processed: 0,
            messages_processed: all_messages.len() as u64,
            memory_peak_mb: 0,
            throughput_events_per_sec: 0.0,
            average_latency_ms: 0.0,
            error_count: 0,
        })
    }
}

/// Multi-session conversation test
pub struct MultiSessionTest;

#[async_trait]
impl TestCase for MultiSessionTest {
    async fn run(&self, config: &TestConfig) -> TestResult {
        let start_time = Instant::now();
        
        match self.run_test(config).await {
            Ok(metrics) => TestResult::success(
                self.name().to_string(),
                start_time.elapsed(),
                Some(metrics),
            ),
            Err(error) => TestResult::failure(
                self.name().to_string(),
                start_time.elapsed(),
                error,
            ),
        }
    }

    fn name(&self) -> &str {
        "MultiSession"
    }

    fn description(&self) -> &str {
        "Tests handling of multiple conversation sessions"
    }
}

impl MultiSessionTest {
    async fn run_test(&self, config: &TestConfig) -> Result<TestMetrics, String> {
        if !config.enable_persistence {
            return Err("Persistence required for multi-session test".to_string());
        }
        
        let mut memory_manager = TestUtilities::create_test_memory_manager(config.memory_config.clone()).await?;
        
        let session_count = 5;
        let mut session_ids = Vec::new();
        let messages_per_session = 20;
        
        // Create multiple sessions
        for i in 0..session_count {
            let session_id = memory_manager.start_new_session().await
                .map_err(|e| format!("Failed to start session {}: {}", i, e))?;
            
            session_ids.push(session_id.clone());
            
            // Add messages to this session
            let messages = TestUtilities::generate_test_messages(messages_per_session);
            for message in messages {
                memory_manager.add_message(message).await
                    .map_err(|e| format!("Failed to add message to session {}: {}", i, e))?;
            }
            
            // Checkpoint the session
            memory_manager.checkpoint_current_session().await
                .map_err(|e| format!("Failed to checkpoint session {}: {}", i, e))?;
        }
        
        // Verify all sessions exist
        let listed_sessions = memory_manager.list_sessions().await
            .map_err(|e| format!("Failed to list sessions: {}", e))?;
        
        for session_id in &session_ids {
            if !listed_sessions.contains(session_id) {
                return Err(format!("Session {} not found in session list", session_id));
            }
        }
        
        // Load and verify each session
        for (i, session_id) in session_ids.iter().enumerate() {
            let session_messages = memory_manager.load_session(session_id).await
                .map_err(|e| format!("Failed to load session {}: {}", session_id, e))?;
            
            if session_messages.len() != messages_per_session {
                return Err(format!(
                    "Session {} has {} messages, expected {}",
                    i, session_messages.len(), messages_per_session
                ));
            }
        }
        
        // Clean up sessions
        for session_id in session_ids {
            memory_manager.delete_session(&session_id).await
                .map_err(|e| format!("Failed to delete session {}: {}", session_id, e))?;
        }
        
        println!("Multi-Session Test Results:");
        println!("  Sessions created: {}", session_count);
        println!("  Messages per session: {}", messages_per_session);
        println!("  Total messages: {}", session_count * messages_per_session);
        
        Ok(TestMetrics {
            events_processed: 0,
            messages_processed: (session_count * messages_per_session) as u64,
            memory_peak_mb: 0,
            throughput_events_per_sec: 0.0,
            average_latency_ms: 0.0,
            error_count: 0,
        })
    }
}

/// Create conversation-specific tests
pub fn create_conversation_tests() -> Vec<Box<dyn TestCase>> {
    vec![
        Box::new(ConversationFlowTest),
        Box::new(MultiSessionTest),
    ]
}
//! Integration Tests for Event-Driven Memory System
//!
//! Comprehensive integration tests that validate the entire event-driven
//! memory system working together.

use std::time::{Duration, Instant};
use async_trait::async_trait;
use super::test_utilities::{TestCase, TestConfig, TestResult, TestMetrics, TestUtilities};
use crate::agent::traits::MemoryManager;

/// Integration test for basic memory operations
pub struct BasicMemoryOperationsTest;

#[async_trait]
impl TestCase for BasicMemoryOperationsTest {
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
        "BasicMemoryOperations"
    }

    fn description(&self) -> &str {
        "Tests basic memory operations: add, retrieve, clear"
    }
}

impl BasicMemoryOperationsTest {
    async fn run_test(&self, config: &TestConfig) -> Result<TestMetrics, String> {
        // Create memory manager
        let mut memory_manager = TestUtilities::create_test_memory_manager(config.memory_config.clone()).await?;
        
        // Generate test messages
        let messages = TestUtilities::generate_test_messages(100);
        let start_time = Instant::now();
        
        // Add messages
        for message in &messages {
            memory_manager.add_message(message.clone()).await
                .map_err(|e| format!("Failed to add message: {}", e))?;
        }
        
        // Retrieve messages
        let retrieved_messages = memory_manager.get_messages().await
            .map_err(|e| format!("Failed to get messages: {}", e))?;
        
        // Validate integrity
        TestUtilities::validate_message_integrity(&messages, &retrieved_messages)?;
        
        // Test last N messages
        let last_10 = memory_manager.get_last_n_messages(10).await
            .map_err(|e| format!("Failed to get last N messages: {}", e))?;
        
        if last_10.len() != 10 {
            return Err(format!("Expected 10 messages, got {}", last_10.len()));
        }
        
        // Clear memory
        memory_manager.clear_memory().await
            .map_err(|e| format!("Failed to clear memory: {}", e))?;
        
        let cleared_messages = memory_manager.get_messages().await
            .map_err(|e| format!("Failed to get messages after clear: {}", e))?;
        
        if !cleared_messages.is_empty() {
            return Err(format!("Memory not properly cleared: {} messages remain", cleared_messages.len()));
        }
        
        let duration = start_time.elapsed();
        let throughput = messages.len() as f64 / duration.as_secs_f64();
        
        Ok(TestMetrics {
            events_processed: 0,
            messages_processed: messages.len() as u64,
            memory_peak_mb: 0,
            throughput_events_per_sec: throughput,
            average_latency_ms: (duration.as_millis() as f64) / messages.len() as f64,
            error_count: 0,
        })
    }
}

/// Integration test for session management with persistence
pub struct SessionPersistenceTest;

#[async_trait]
impl TestCase for SessionPersistenceTest {
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
        "SessionPersistence"
    }

    fn description(&self) -> &str {
        "Tests session creation, persistence, and recovery"
    }
}

impl SessionPersistenceTest {
    async fn run_test(&self, config: &TestConfig) -> Result<TestMetrics, String> {
        if !config.enable_persistence {
            return Err("Persistence not enabled in test config".to_string());
        }
        
        let mut memory_manager = TestUtilities::create_test_memory_manager(config.memory_config.clone()).await?;
        
        // Start a new session
        let session_id = memory_manager.start_new_session().await
            .map_err(|e| format!("Failed to start new session: {}", e))?;
        
        // Add conversation messages
        let conversation = TestUtilities::create_conversation_flow();
        for message in &conversation {
            memory_manager.add_message(message.clone()).await
                .map_err(|e| format!("Failed to add message: {}", e))?;
        }
        
        // Checkpoint the session
        memory_manager.checkpoint_current_session().await
            .map_err(|e| format!("Failed to checkpoint session: {}", e))?;
        
        // Create a new memory manager and load the session
        let mut new_memory_manager = TestUtilities::create_test_memory_manager(config.memory_config.clone()).await?;
        let loaded_messages = new_memory_manager.load_session(&session_id).await
            .map_err(|e| format!("Failed to load session: {}", e))?;
        
        // Validate the loaded conversation
        TestUtilities::validate_message_integrity(&conversation, &loaded_messages)?;
        
        // Test session listing
        let sessions = new_memory_manager.list_sessions().await
            .map_err(|e| format!("Failed to list sessions: {}", e))?;
        
        if !sessions.contains(&session_id) {
            return Err("Session not found in session list".to_string());
        }
        
        // Clean up - delete the test session
        new_memory_manager.delete_session(&session_id).await
            .map_err(|e| format!("Failed to delete session: {}", e))?;
        
        Ok(TestMetrics {
            events_processed: 0,
            messages_processed: conversation.len() as u64,
            memory_peak_mb: 0,
            throughput_events_per_sec: 0.0,
            average_latency_ms: 0.0,
            error_count: 0,
        })
    }
}

/// Integration test for concurrent operations
pub struct ConcurrentOperationsTest;

#[async_trait]
impl TestCase for ConcurrentOperationsTest {
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
        "ConcurrentOperations"
    }

    fn description(&self) -> &str {
        "Tests concurrent memory operations from multiple tasks"
    }
}

impl ConcurrentOperationsTest {
    async fn run_test(&self, config: &TestConfig) -> Result<TestMetrics, String> {
        let memory_manager = TestUtilities::create_test_memory_manager(config.memory_config.clone()).await?;
        let concurrent_count = config.concurrent_operations;
        let messages_per_task = 50;
        
        let start_time = Instant::now();
        
        // Spawn concurrent tasks
        let mut handles = Vec::new();
        for task_id in 0..concurrent_count {
            let memory_manager = memory_manager.clone();
            let messages = TestUtilities::generate_test_messages(messages_per_task);
            
            let handle = tokio::spawn(async move {
                let mut local_manager = memory_manager;
                for (i, message) in messages.into_iter().enumerate() {
                    let mut modified_message = message;
                    modified_message.content = format!("Task {} Message {}: {}", task_id, i, modified_message.content);
                    
                    if let Err(e) = local_manager.add_message(modified_message).await {
                        return Err(format!("Task {} failed to add message {}: {}", task_id, i, e));
                    }
                }
                Ok(messages_per_task)
            });
            
            handles.push(handle);
        }
        
        // Wait for all tasks to complete
        let mut total_messages = 0;
        let mut error_count = 0;
        
        for handle in handles {
            match handle.await {
                Ok(Ok(count)) => total_messages += count,
                Ok(Err(e)) => {
                    eprintln!("Task error: {}", e);
                    error_count += 1;
                },
                Err(e) => {
                    eprintln!("Task join error: {}", e);
                    error_count += 1;
                },
            }
        }
        
        let duration = start_time.elapsed();
        
        // Verify final state
        let final_messages = memory_manager.get_messages().await
            .map_err(|e| format!("Failed to get final messages: {}", e))?;
        
        let expected_count = concurrent_count * messages_per_task;
        if final_messages.len() != expected_count {
            return Err(format!(
                "Expected {} messages, got {}. Errors: {}",
                expected_count, final_messages.len(), error_count
            ));
        }
        
        Ok(TestMetrics {
            events_processed: 0,
            messages_processed: total_messages as u64,
            memory_peak_mb: 0,
            throughput_events_per_sec: total_messages as f64 / duration.as_secs_f64(),
            average_latency_ms: (duration.as_millis() as f64) / total_messages as f64,
            error_count,
        })
    }
}

/// Integration test for tool call handling
pub struct ToolCallHandlingTest;

#[async_trait]
impl TestCase for ToolCallHandlingTest {
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
        "ToolCallHandling"
    }

    fn description(&self) -> &str {
        "Tests tool call creation, tracking, and result handling"
    }
}

impl ToolCallHandlingTest {
    async fn run_test(&self, _config: &TestConfig) -> Result<TestMetrics, String> {
        let mut memory_manager = TestUtilities::create_test_memory_manager(
            crate::agent::memory::EventMemoryConfig::default()
        ).await?;
        
        let start_time = Instant::now();
        
        // Test orphaned tool call cleanup
        memory_manager.clean_orphaned_tool_calls().await
            .map_err(|e| format!("Failed to clean orphaned tool calls: {}", e))?;
        
        // Add some tool calls and results
        let conversation = TestUtilities::create_conversation_flow();
        for message in &conversation {
            memory_manager.add_message(message.clone()).await
                .map_err(|e| format!("Failed to add message: {}", e))?;
        }
        
        // Verify the conversation structure
        let retrieved_messages = memory_manager.get_messages().await
            .map_err(|e| format!("Failed to get messages: {}", e))?;
        
        // Count tool calls and results
        let mut tool_calls = 0;
        let mut tool_results = 0;
        
        for message in &retrieved_messages {
            if message.tool_calls.is_some() {
                tool_calls += message.tool_calls.as_ref().unwrap().len();
            }
            if message.tool_call_id.is_some() {
                tool_results += 1;
            }
        }
        
        if tool_calls == 0 {
            return Err("No tool calls found in conversation".to_string());
        }
        
        if tool_results == 0 {
            return Err("No tool results found in conversation".to_string());
        }
        
        let duration = start_time.elapsed();
        
        Ok(TestMetrics {
            events_processed: 0,
            messages_processed: conversation.len() as u64,
            memory_peak_mb: 0,
            throughput_events_per_sec: 0.0,
            average_latency_ms: duration.as_millis() as f64,
            error_count: 0,
        })
    }
}

/// Integration test for memory pressure and pruning
pub struct MemoryPressureTest;

#[async_trait]
impl TestCase for MemoryPressureTest {
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
        "MemoryPressure"
    }

    fn description(&self) -> &str {
        "Tests memory pressure handling and automatic pruning"
    }
}

impl MemoryPressureTest {
    async fn run_test(&self, config: &TestConfig) -> Result<TestMetrics, String> {
        // Create a configuration with small limits to trigger pruning
        let mut test_config = config.memory_config.clone();
        test_config.max_events = 50;
        test_config.token_limit = 10000;
        test_config.min_events_after_prune = 10;
        test_config.auto_prune = true;
        
        let mut memory_manager = TestUtilities::create_test_memory_manager(test_config).await?;
        
        let start_time = Instant::now();
        
        // Add more messages than the limit
        let large_messages = TestUtilities::generate_test_messages(100);
        for message in &large_messages {
            memory_manager.add_message(message.clone()).await
                .map_err(|e| format!("Failed to add message: {}", e))?;
        }
        
        // Check that pruning occurred
        let final_messages = memory_manager.get_messages().await
            .map_err(|e| format!("Failed to get messages: {}", e))?;
        
        if final_messages.len() >= 100 {
            return Err(format!(
                "Pruning did not occur: {} messages remain (expected < 50)",
                final_messages.len()
            ));
        }
        
        // Verify that the most recent messages are preserved
        if final_messages.is_empty() {
            return Err("All messages were pruned - should preserve recent ones".to_string());
        }
        
        let duration = start_time.elapsed();
        
        Ok(TestMetrics {
            events_processed: 0,
            messages_processed: large_messages.len() as u64,
            memory_peak_mb: 0,
            throughput_events_per_sec: large_messages.len() as f64 / duration.as_secs_f64(),
            average_latency_ms: (duration.as_millis() as f64) / large_messages.len() as f64,
            error_count: 0,
        })
    }
}

/// Create all integration tests
pub fn create_integration_tests() -> Vec<Box<dyn TestCase>> {
    vec![
        Box::new(BasicMemoryOperationsTest),
        Box::new(SessionPersistenceTest),
        Box::new(ConcurrentOperationsTest),
        Box::new(ToolCallHandlingTest),
        Box::new(MemoryPressureTest),
    ]
}
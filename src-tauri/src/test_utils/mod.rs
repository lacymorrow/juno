/// Test utilities and helpers for Juno AI Computer Use Agent
/// 
/// This module provides:
/// - Mock implementations for external dependencies
/// - Test data generators
/// - Helper functions for testing agent workflows
/// - Security testing utilities
/// - Performance testing helpers

use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use tokio::sync::RwLock;
use mockall::predicate::*;
use fake::{Fake, Faker};
use serde_json::Value;
use uuid::Uuid;

pub mod mocks;
pub mod generators;
pub mod security;
pub mod performance;
pub mod assertions;
pub mod fixtures;

use crate::agent::structs::{AgentError, AgentResponse};
use crate::state::AppState;

/// Test environment configuration
#[derive(Debug, Clone)]
pub struct TestEnvironment {
    pub temp_dir: std::path::PathBuf,
    pub mock_app_state: Arc<RwLock<MockAppState>>,
    pub test_id: String,
    pub config: TestConfig,
}

#[derive(Debug, Clone)]
pub struct TestConfig {
    pub enable_network: bool,
    pub enable_file_system: bool,
    pub enable_desktop_automation: bool,
    pub timeout_seconds: u64,
    pub security_mode: SecurityMode,
}

#[derive(Debug, Clone)]
pub enum SecurityMode {
    Permissive,  // For development testing
    Strict,      // For production-like testing
    Isolated,    // For security testing
}

/// Mock app state for testing
#[derive(Debug)]
pub struct MockAppState {
    pub settings: HashMap<String, Value>,
    pub conversations: Vec<MockConversation>,
    pub tool_configs: HashMap<String, bool>,
    pub permissions: HashMap<String, bool>,
}

#[derive(Debug, Clone)]
pub struct MockConversation {
    pub id: String,
    pub messages: Vec<MockMessage>,
    pub created_at: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, Clone)]
pub struct MockMessage {
    pub id: String,
    pub content: String,
    pub role: String,
    pub timestamp: chrono::DateTime<chrono::Utc>,
}

impl Default for TestConfig {
    fn default() -> Self {
        Self {
            enable_network: false,
            enable_file_system: false,
            enable_desktop_automation: false,
            timeout_seconds: 30,
            security_mode: SecurityMode::Strict,
        }
    }
}

impl TestEnvironment {
    /// Create a new test environment with default configuration
    pub async fn new() -> Result<Self, Box<dyn std::error::Error>> {
        Self::with_config(TestConfig::default()).await
    }

    /// Create a new test environment with custom configuration
    pub async fn with_config(config: TestConfig) -> Result<Self, Box<dyn std::error::Error>> {
        let temp_dir = tempfile::tempdir()?.into_path();
        let test_id = Uuid::new_v4().to_string();

        let mock_app_state = Arc::new(RwLock::new(MockAppState {
            settings: HashMap::new(),
            conversations: Vec::new(),
            tool_configs: HashMap::new(),
            permissions: HashMap::new(),
        }));

        Ok(Self {
            temp_dir,
            mock_app_state,
            test_id,
            config,
        })
    }

    /// Get a temporary file path for testing
    pub fn temp_file(&self, name: &str) -> std::path::PathBuf {
        self.temp_dir.join(format!("{}_{}", self.test_id, name))
    }

    /// Setup mock permissions for testing
    pub async fn setup_permissions(&self, permissions: Vec<(&str, bool)>) {
        let mut state = self.mock_app_state.write().await;
        for (permission, granted) in permissions {
            state.permissions.insert(permission.to_string(), granted);
        }
    }

    /// Setup mock tool configurations
    pub async fn setup_tool_configs(&self, tools: Vec<(&str, bool)>) {
        let mut state = self.mock_app_state.write().await;
        for (tool, enabled) in tools {
            state.tool_configs.insert(tool.to_string(), enabled);
        }
    }

    /// Add a mock conversation for testing
    pub async fn add_mock_conversation(&self, messages: Vec<(&str, &str)>) -> String {
        let conversation_id = Uuid::new_v4().to_string();
        let mut mock_messages = Vec::new();

        for (role, content) in messages {
            mock_messages.push(MockMessage {
                id: Uuid::new_v4().to_string(),
                content: content.to_string(),
                role: role.to_string(),
                timestamp: chrono::Utc::now(),
            });
        }

        let conversation = MockConversation {
            id: conversation_id.clone(),
            messages: mock_messages,
            created_at: chrono::Utc::now(),
        };

        let mut state = self.mock_app_state.write().await;
        state.conversations.push(conversation);

        conversation_id
    }

    /// Clean up test environment
    pub async fn cleanup(&self) -> Result<(), Box<dyn std::error::Error>> {
        if self.temp_dir.exists() {
            std::fs::remove_dir_all(&self.temp_dir)?;
        }
        Ok(())
    }
}

/// Helper function to create a test agent response
pub fn create_test_response(content: &str, success: bool) -> AgentResponse {
    AgentResponse {
        content: content.to_string(),
        tool_calls: Vec::new(),
        conversation_id: Some(Uuid::new_v4().to_string()),
        message_id: Some(Uuid::new_v4().to_string()),
        success,
        error_message: if success { None } else { Some("Test error".to_string()) },
        execution_time_ms: Some(100),
        tokens_used: Some(50),
    }
}

/// Helper function to create a test agent error
pub fn create_test_error(message: &str) -> AgentError {
    AgentError::ToolExecutionError(message.to_string())
}

/// Assert that a function execution time is within acceptable bounds
#[macro_export]
macro_rules! assert_performance {
    ($duration:expr, $max_ms:expr) => {
        assert!(
            $duration.as_millis() <= $max_ms as u128,
            "Performance assertion failed: execution took {}ms, expected <= {}ms",
            $duration.as_millis(),
            $max_ms
        );
    };
}

/// Assert that security constraints are met
#[macro_export]
macro_rules! assert_security {
    ($condition:expr, $message:expr) => {
        assert!($condition, "Security assertion failed: {}", $message);
    };
}

/// Test suite runner for comprehensive testing
pub struct TestSuite {
    pub name: String,
    pub tests: Vec<Box<dyn Fn() -> Result<(), Box<dyn std::error::Error>> + Send + Sync>>,
    pub setup: Option<Box<dyn Fn() -> Result<(), Box<dyn std::error::Error>> + Send + Sync>>,
    pub teardown: Option<Box<dyn Fn() -> Result<(), Box<dyn std::error::Error>> + Send + Sync>>,
}

impl TestSuite {
    pub fn new(name: &str) -> Self {
        Self {
            name: name.to_string(),
            tests: Vec::new(),
            setup: None,
            teardown: None,
        }
    }

    pub fn add_test<F>(&mut self, test: F)
    where
        F: Fn() -> Result<(), Box<dyn std::error::Error>> + Send + Sync + 'static,
    {
        self.tests.push(Box::new(test));
    }

    pub fn run(&self) -> Result<TestResults, Box<dyn std::error::Error>> {
        let mut results = TestResults::new(&self.name);

        // Run setup if provided
        if let Some(setup) = &self.setup {
            setup()?;
        }

        // Run all tests
        for (index, test) in self.tests.iter().enumerate() {
            let start_time = std::time::Instant::now();
            match test() {
                Ok(()) => {
                    results.add_success(index, start_time.elapsed());
                }
                Err(e) => {
                    results.add_failure(index, start_time.elapsed(), e.to_string());
                }
            }
        }

        // Run teardown if provided
        if let Some(teardown) = &self.teardown {
            teardown()?;
        }

        Ok(results)
    }
}

/// Test results container
#[derive(Debug)]
pub struct TestResults {
    pub suite_name: String,
    pub total_tests: usize,
    pub passed: usize,
    pub failed: usize,
    pub total_duration: std::time::Duration,
    pub failures: Vec<TestFailure>,
}

#[derive(Debug)]
pub struct TestFailure {
    pub test_index: usize,
    pub duration: std::time::Duration,
    pub error: String,
}

impl TestResults {
    pub fn new(suite_name: &str) -> Self {
        Self {
            suite_name: suite_name.to_string(),
            total_tests: 0,
            passed: 0,
            failed: 0,
            total_duration: std::time::Duration::new(0, 0),
            failures: Vec::new(),
        }
    }

    pub fn add_success(&mut self, test_index: usize, duration: std::time::Duration) {
        self.total_tests += 1;
        self.passed += 1;
        self.total_duration += duration;
    }

    pub fn add_failure(&mut self, test_index: usize, duration: std::time::Duration, error: String) {
        self.total_tests += 1;
        self.failed += 1;
        self.total_duration += duration;
        self.failures.push(TestFailure {
            test_index,
            duration,
            error,
        });
    }

    pub fn success_rate(&self) -> f64 {
        if self.total_tests == 0 {
            0.0
        } else {
            self.passed as f64 / self.total_tests as f64
        }
    }

    pub fn print_summary(&self) {
        println!("\n=== Test Suite: {} ===", self.suite_name);
        println!("Tests run: {}", self.total_tests);
        println!("Passed: {}", self.passed);
        println!("Failed: {}", self.failed);
        println!("Success rate: {:.1}%", self.success_rate() * 100.0);
        println!("Total duration: {:?}", self.total_duration);

        if !self.failures.is_empty() {
            println!("\nFailures:");
            for failure in &self.failures {
                println!("  Test {}: {} (took {:?})", failure.test_index, failure.error, failure.duration);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_environment_creation() {
        let env = TestEnvironment::new().await.unwrap();
        assert!(env.temp_dir.exists());
        assert!(!env.test_id.is_empty());
        
        env.cleanup().await.unwrap();
        assert!(!env.temp_dir.exists());
    }

    #[tokio::test]
    async fn test_mock_conversation() {
        let env = TestEnvironment::new().await.unwrap();
        
        let conversation_id = env.add_mock_conversation(vec![
            ("user", "Hello"),
            ("assistant", "Hi there!"),
        ]).await;

        let state = env.mock_app_state.read().await;
        assert_eq!(state.conversations.len(), 1);
        assert_eq!(state.conversations[0].id, conversation_id);
        assert_eq!(state.conversations[0].messages.len(), 2);
        
        env.cleanup().await.unwrap();
    }

    #[test]
    fn test_response_creation() {
        let response = create_test_response("Test content", true);
        assert_eq!(response.content, "Test content");
        assert!(response.success);
        assert!(response.conversation_id.is_some());
    }

    #[test]
    fn test_error_creation() {
        let error = create_test_error("Test error message");
        match error {
            AgentError::ToolExecutionError(msg) => assert_eq!(msg, "Test error message"),
            _ => panic!("Expected ToolExecutionError"),
        }
    }

    #[test]
    fn test_suite_runner() {
        let mut suite = TestSuite::new("Test Suite");
        
        suite.add_test(|| Ok(()));
        suite.add_test(|| Err("Test failure".into()));
        
        let results = suite.run().unwrap();
        assert_eq!(results.total_tests, 2);
        assert_eq!(results.passed, 1);
        assert_eq!(results.failed, 1);
        assert_eq!(results.success_rate(), 0.5);
    }
}
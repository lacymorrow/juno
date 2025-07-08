//! Test Utilities and Helper Functions
//!
//! Common utilities for testing the event-driven memory system

use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::Mutex as TokioMutex;
// use tauri::test::mock_app; // Not available in all Tauri versions
use uuid::Uuid;
use serde::{Serialize, Deserialize};

use crate::agent::core::{Message, Role, ToolCall};
use crate::agent::events::{EventBus, JunoAgentEvent};
use crate::agent::memory::{
    EventMemoryManager, EventMemoryConfig, PersistenceConfig, PerformanceConfig,
};
use crate::agent::multi_agent::AgentType;

/// Test configuration for various scenarios
#[derive(Debug, Clone)]
pub struct TestConfig {
    pub memory_config: EventMemoryConfig,
    pub performance_config: PerformanceConfig,
    pub test_duration: Duration,
    pub event_count: usize,
    pub concurrent_operations: usize,
    pub enable_persistence: bool,
}

impl Default for TestConfig {
    fn default() -> Self {
        Self {
            memory_config: EventMemoryConfig {
                max_events: 1000,
                auto_prune: true,
                token_limit: 50000,
                min_events_after_prune: 100,
                enable_persistence: false, // Disabled by default for tests
                persistence_config: None,
            },
            performance_config: PerformanceConfig {
                enable_object_pooling: true,
                max_pool_size: 100,
                enable_batch_processing: true,
                batch_size: 10,
                batch_timeout_ms: 50,
                enable_memory_mapping: false,
                memory_map_threshold: 1024 * 1024,
                enable_smart_caching: true,
                cache_ttl_seconds: 60,
                max_cache_size: 1000,
                enable_concurrent_processing: true,
                max_concurrent_operations: 5,
            },
            test_duration: Duration::from_secs(30),
            event_count: 1000,
            concurrent_operations: 10,
            enable_persistence: false,
        }
    }
}

impl TestConfig {
    /// Create configuration for stress testing
    pub fn stress_test() -> Self {
        Self {
            memory_config: EventMemoryConfig {
                max_events: 10000,
                auto_prune: true,
                token_limit: 500000,
                min_events_after_prune: 1000,
                enable_persistence: true,
                persistence_config: Some(PersistenceConfig {
                    storage_dir: std::env::temp_dir().join("juno_stress_test"),
                    auto_checkpoint: true,
                    checkpoint_interval: 100,
                    max_session_age_days: 1,
                    enable_compression: true,
                    enable_deduplication: true,
                    max_file_size: 10 * 1024 * 1024,
                }),
            },
            test_duration: Duration::from_secs(60),
            event_count: 10000,
            concurrent_operations: 50,
            enable_persistence: true,
            ..Default::default()
        }
    }

    /// Create configuration for memory leak testing
    pub fn memory_leak_test() -> Self {
        Self {
            test_duration: Duration::from_secs(120),
            event_count: 50000,
            concurrent_operations: 20,
            ..Self::stress_test()
        }
    }

    /// Create configuration for minimal resource usage
    pub fn minimal_resources() -> Self {
        Self {
            memory_config: EventMemoryConfig {
                max_events: 100,
                auto_prune: true,
                token_limit: 5000,
                min_events_after_prune: 10,
                enable_persistence: false,
                persistence_config: None,
            },
            performance_config: PerformanceConfig {
                enable_object_pooling: false,
                max_pool_size: 10,
                enable_batch_processing: false,
                batch_size: 5,
                batch_timeout_ms: 100,
                enable_memory_mapping: false,
                memory_map_threshold: 1024,
                enable_smart_caching: false,
                cache_ttl_seconds: 30,
                max_cache_size: 100,
                enable_concurrent_processing: false,
                max_concurrent_operations: 1,
            },
            test_duration: Duration::from_secs(10),
            event_count: 100,
            concurrent_operations: 1,
            enable_persistence: false,
        }
    }
}

/// Test utilities for creating test data and scenarios
pub struct TestUtilities;

impl TestUtilities {
    /// Create a test memory manager with given configuration
    /// Note: This is a simplified version for testing that skips event bus setup
    pub async fn create_test_memory_manager(_config: EventMemoryConfig) -> Result<EventMemoryManager, String> {
        // For now, return an error indicating that proper test setup is needed
        // In a real test environment, you'd create a proper test EventBus
        Err("Test memory manager creation requires proper test environment setup".to_string())
    }

    /// Generate test messages for various scenarios
    pub fn generate_test_messages(count: usize) -> Vec<Message> {
        let mut messages = Vec::with_capacity(count);
        
        for i in 0..count {
            let message = match i % 4 {
                0 => Message {
                    role: Role::User,
                    content: format!("User message {}", i),
                    tool_calls: None,
                    tool_call_id: None,
                    name: None,
                },
                1 => Message {
                    role: Role::Assistant,
                    content: format!("Assistant response {}", i),
                    tool_calls: None,
                    tool_call_id: None,
                    name: None,
                },
                2 => {
                    let tool_call = ToolCall {
                        id: format!("tool_call_{}", i),
                        name: "test_tool".to_string(),
                        input: serde_json::json!({"param": format!("value_{}", i)}),
                    };
                    Message {
                        role: Role::Assistant,
                        content: format!("Using tool {}", i),
                        tool_calls: Some(vec![tool_call]),
                        tool_call_id: None,
                        name: None,
                    }
                },
                3 => Message {
                    role: Role::Tool,
                    content: format!("Tool result {}", i),
                    tool_calls: None,
                    tool_call_id: Some(format!("tool_call_{}", i - 1)),
                    name: Some("test_tool".to_string()),
                },
                _ => unreachable!(),
            };
            messages.push(message);
        }
        
        messages
    }

    /// Generate test events for various scenarios
    pub fn generate_test_events(count: usize, session_id: Option<String>) -> Vec<JunoAgentEvent> {
        let mut events = Vec::with_capacity(count);
        let session = session_id.unwrap_or_else(|| Uuid::new_v4().to_string());
        
        for i in 0..count {
            let timestamp = chrono::Utc::now().timestamp_millis() as u64 + i as u64;
            
            let event = match i % 6 {
                0 => JunoAgentEvent::UserMessage {
                    content: format!("User message {}", i),
                    timestamp,
                    session_id: Some(session.clone()),
                },
                1 => JunoAgentEvent::AssistantMessage {
                    content: format!("Assistant response {}", i),
                    timestamp,
                    session_id: Some(session.clone()),
                },
                2 => JunoAgentEvent::ToolCall {
                    tool_name: "test_tool".to_string(),
                    args: serde_json::json!({"param": format!("value_{}", i)}),
                    id: format!("tool_call_{}", i),
                    timestamp,
                    session_id: Some(session.clone()),
                },
                3 => JunoAgentEvent::ToolResult {
                    tool_call_id: format!("tool_call_{}", i - 1),
                    result: serde_json::json!({"result": format!("result_{}", i)}),
                    execution_time_ms: Some(100),
                    success: true,
                    timestamp,
                },
                4 => JunoAgentEvent::AgentRunStart {
                    session_id: session.clone(),
                    user_query: format!("Query {}", i),
                    agent_type: AgentType::DesktopExpert.get_name().to_string(),
                    max_iterations: 10,
                    timestamp,
                },
                5 => JunoAgentEvent::AgentRunEnd {
                    session_id: session.clone(),
                    status: "completed".to_string(),
                    iterations: i as u32 % 10 + 1,
                    elapsed_ms: i as u64 % 1000 + 100,
                    timestamp,
                },
                _ => unreachable!(),
            };
            events.push(event);
        }
        
        events
    }

    /// Create a conversation flow for testing
    pub fn create_conversation_flow() -> Vec<Message> {
        vec![
            Message {
                role: Role::User,
                content: "Hello, can you help me with a coding task?".to_string(),
                tool_calls: None,
                tool_call_id: None,
                name: None,
            },
            Message {
                role: Role::Assistant,
                content: "I'd be happy to help you with coding! What specific task are you working on?".to_string(),
                tool_calls: None,
                tool_call_id: None,
                name: None,
            },
            Message {
                role: Role::User,
                content: "I need to write a function that calculates the factorial of a number.".to_string(),
                tool_calls: None,
                tool_call_id: None,
                name: None,
            },
            Message {
                role: Role::Assistant,
                content: "Let me help you write a factorial function.".to_string(),
                tool_calls: Some(vec![ToolCall {
                    id: "write_code_1".to_string(),
                    name: "write_code".to_string(),
                    input: serde_json::json!({
                        "language": "python",
                        "code": "def factorial(n):\n    if n <= 1:\n        return 1\n    return n * factorial(n - 1)"
                    }),
                }]),
                tool_call_id: None,
                name: None,
            },
            Message {
                role: Role::Tool,
                content: "Code written successfully".to_string(),
                tool_calls: None,
                tool_call_id: Some("write_code_1".to_string()),
                name: Some("write_code".to_string()),
            },
            Message {
                role: Role::Assistant,
                content: "I've written a recursive factorial function for you. Here's how it works:\n\n1. Base case: if n <= 1, return 1\n2. Recursive case: return n * factorial(n-1)\n\nWould you like me to test it or explain it further?".to_string(),
                tool_calls: None,
                tool_call_id: None,
                name: None,
            },
        ]
    }

    /// Generate large content for stress testing
    pub fn generate_large_content(size_kb: usize) -> String {
        let base_text = "This is a test message for stress testing the memory system. ";
        let target_size = size_kb * 1024;
        let mut content = String::with_capacity(target_size);
        
        while content.len() < target_size {
            content.push_str(base_text);
        }
        
        content.truncate(target_size);
        content
    }

    /// Create a memory usage monitor
    pub fn create_memory_monitor() -> MemoryMonitor {
        MemoryMonitor::new()
    }

    /// Validate message integrity
    pub fn validate_message_integrity(original: &[Message], retrieved: &[Message]) -> Result<(), String> {
        if original.len() != retrieved.len() {
            return Err(format!(
                "Message count mismatch: expected {}, got {}",
                original.len(),
                retrieved.len()
            ));
        }

        for (i, (orig, retr)) in original.iter().zip(retrieved.iter()).enumerate() {
            if orig.role != retr.role {
                return Err(format!("Role mismatch at index {}: expected {:?}, got {:?}", i, orig.role, retr.role));
            }
            
            if orig.content != retr.content {
                return Err(format!("Content mismatch at index {}: expected '{}', got '{}'", i, orig.content, retr.content));
            }
            
            if orig.tool_call_id != retr.tool_call_id {
                return Err(format!("Tool call ID mismatch at index {}: expected {:?}, got {:?}", i, orig.tool_call_id, retr.tool_call_id));
            }
            
            if orig.name != retr.name {
                return Err(format!("Name mismatch at index {}: expected {:?}, got {:?}", i, orig.name, retr.name));
            }

            // Check tool calls
            match (&orig.tool_calls, &retr.tool_calls) {
                (None, None) => {},
                (Some(orig_calls), Some(retr_calls)) => {
                    if orig_calls.len() != retr_calls.len() {
                        return Err(format!("Tool calls count mismatch at index {}", i));
                    }
                    for (orig_call, retr_call) in orig_calls.iter().zip(retr_calls.iter()) {
                        if orig_call.id != retr_call.id || orig_call.name != retr_call.name {
                            return Err(format!("Tool call mismatch at index {}", i));
                        }
                    }
                },
                _ => return Err(format!("Tool calls presence mismatch at index {}", i)),
            }
        }

        Ok(())
    }

    /// Wait for async operations to complete
    pub async fn wait_for_operations(duration: Duration) {
        tokio::time::sleep(duration).await;
    }

    /// Create test session ID
    pub fn create_test_session_id() -> String {
        format!("test_session_{}", Uuid::new_v4())
    }
}

/// Memory usage monitor for leak detection
pub struct MemoryMonitor {
    initial_memory: u64,
    peak_memory: Arc<TokioMutex<u64>>,
    measurements: Arc<TokioMutex<Vec<MemoryMeasurement>>>,
}

#[derive(Debug, Clone)]
pub struct MemoryMeasurement {
    pub timestamp: Instant,
    pub memory_usage: u64,
    pub operation: String,
}

impl MemoryMonitor {
    pub fn new() -> Self {
        let initial_memory = Self::get_current_memory_usage();
        Self {
            initial_memory,
            peak_memory: Arc::new(TokioMutex::new(initial_memory)),
            measurements: Arc::new(TokioMutex::new(Vec::new())),
        }
    }

    pub async fn record_measurement(&self, operation: &str) {
        let current_memory = Self::get_current_memory_usage();
        
        // Update peak memory
        {
            let mut peak = self.peak_memory.lock().await;
            if current_memory > *peak {
                *peak = current_memory;
            }
        }

        // Record measurement
        {
            let mut measurements = self.measurements.lock().await;
            measurements.push(MemoryMeasurement {
                timestamp: Instant::now(),
                memory_usage: current_memory,
                operation: operation.to_string(),
            });
        }
    }

    pub async fn get_statistics(&self) -> MemoryStatistics {
        let measurements = self.measurements.lock().await;
        let peak_memory = *self.peak_memory.lock().await;
        
        let current_memory = Self::get_current_memory_usage();
        let memory_growth = current_memory.saturating_sub(self.initial_memory);
        
        // Calculate average memory usage
        let avg_memory = if measurements.is_empty() {
            current_memory
        } else {
            measurements.iter().map(|m| m.memory_usage).sum::<u64>() / measurements.len() as u64
        };

        MemoryStatistics {
            initial_memory: self.initial_memory,
            current_memory,
            peak_memory,
            avg_memory,
            memory_growth,
            measurements_count: measurements.len(),
        }
    }

    fn get_current_memory_usage() -> u64 {
        // This is a simplified implementation
        // In a real scenario, you might use system-specific APIs
        // or memory profiling tools
        
        #[cfg(target_os = "macos")]
        {
            // Use task_info on macOS for more accurate memory measurements
            // For now, return a placeholder
            0
        }
        
        #[cfg(not(target_os = "macos"))]
        {
            0
        }
    }
}

#[derive(Debug, Clone)]
pub struct MemoryStatistics {
    pub initial_memory: u64,
    pub current_memory: u64,
    pub peak_memory: u64,
    pub avg_memory: u64,
    pub memory_growth: u64,
    pub measurements_count: usize,
}

/// Test result aggregation and reporting
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TestResult {
    pub test_name: String,
    pub success: bool,
    #[serde(with = "duration_serde")]
    pub duration: Duration,
    pub error_message: Option<String>,
    pub metrics: Option<TestMetrics>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TestMetrics {
    pub events_processed: u64,
    pub messages_processed: u64,
    pub memory_peak_mb: u64,
    pub throughput_events_per_sec: f64,
    pub average_latency_ms: f64,
    pub error_count: u32,
}

impl TestResult {
    pub fn success(test_name: String, duration: Duration, metrics: Option<TestMetrics>) -> Self {
        Self {
            test_name,
            success: true,
            duration,
            error_message: None,
            metrics,
        }
    }

    pub fn failure(test_name: String, duration: Duration, error: String) -> Self {
        Self {
            test_name,
            success: false,
            duration,
            error_message: Some(error),
            metrics: None,
        }
    }
}

/// Test suite runner
pub struct TestSuite {
    pub name: String,
    pub tests: Vec<Box<dyn TestCase>>,
}

#[async_trait::async_trait]
pub trait TestCase: Send + Sync {
    async fn run(&self, config: &TestConfig) -> TestResult;
    fn name(&self) -> &str;
    fn description(&self) -> &str;
}

impl TestSuite {
    pub fn new(name: String) -> Self {
        Self {
            name,
            tests: Vec::new(),
        }
    }

    pub fn add_test(&mut self, test: Box<dyn TestCase>) {
        self.tests.push(test);
    }

    pub async fn run_all(&self, config: &TestConfig) -> Vec<TestResult> {
        let mut results = Vec::new();
        
        for test in &self.tests {
            println!("Running test: {}", test.name());
            let result = test.run(config).await;
            
            if result.success {
                println!("✅ {} completed in {:?}", test.name(), result.duration);
            } else {
                println!("❌ {} failed: {}", test.name(), result.error_message.as_deref().unwrap_or("Unknown error"));
            }
            
            results.push(result);
        }
        
        results
    }

    pub fn generate_report(&self, results: &[TestResult]) -> TestReport {
        let total_tests = results.len();
        let passed_tests = results.iter().filter(|r| r.success).count();
        let failed_tests = total_tests - passed_tests;
        
        let total_duration: Duration = results.iter().map(|r| r.duration).sum();
        
        TestReport {
            suite_name: self.name.clone(),
            total_tests,
            passed_tests,
            failed_tests,
            total_duration,
            individual_results: results.to_vec(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TestReport {
    pub suite_name: String,
    pub total_tests: usize,
    pub passed_tests: usize,
    pub failed_tests: usize,
    #[serde(with = "duration_serde")]
    pub total_duration: Duration,
    pub individual_results: Vec<TestResult>,
}

mod duration_serde {
    use std::time::Duration;
    use serde::{Deserialize, Deserializer, Serializer};

    pub fn serialize<S>(duration: &Duration, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_u64(duration.as_millis() as u64)
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<Duration, D::Error>
    where
        D: Deserializer<'de>,
    {
        let millis = u64::deserialize(deserializer)?;
        Ok(Duration::from_millis(millis))
    }
}

impl TestReport {
    pub fn print_summary(&self) {
        println!("\n=== Test Suite: {} ===", self.suite_name);
        println!("Total Tests: {}", self.total_tests);
        println!("Passed: {}", self.passed_tests);
        println!("Failed: {}", self.failed_tests);
        println!("Success Rate: {:.1}%", (self.passed_tests as f64 / self.total_tests as f64) * 100.0);
        println!("Total Duration: {:?}", self.total_duration);
        
        if self.failed_tests > 0 {
            println!("\nFailed Tests:");
            for result in &self.individual_results {
                if !result.success {
                    println!("  - {}: {}", result.test_name, result.error_message.as_deref().unwrap_or("Unknown error"));
                }
            }
        }
        
        println!();
    }
}
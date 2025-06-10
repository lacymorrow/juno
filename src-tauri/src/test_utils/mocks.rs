/// Mock implementations for testing Juno AI Computer Use Agent
/// 
/// This module provides mock implementations for:
/// - External API calls
/// - System operations
/// - File system operations
/// - Agent tool interactions
/// - Tauri app handles

use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use mockall::{mock, predicate::*};
use serde_json::Value;
use tokio::sync::RwLock;

use crate::agent::structs::{AgentError, AgentResponse, ToolCall};

/// Mock Tauri App Handle for testing
pub struct MockAppHandle {
    pub state: Arc<RwLock<MockAppState>>,
    pub events: Arc<Mutex<Vec<String>>>,
}

#[derive(Debug)]
pub struct MockAppState {
    pub settings: HashMap<String, Value>,
    pub conversations: HashMap<String, MockConversation>,
    pub permissions: HashMap<String, bool>,
    pub tool_configs: HashMap<String, bool>,
}

#[derive(Debug, Clone)]
pub struct MockConversation {
    pub id: String,
    pub messages: Vec<MockMessage>,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub updated_at: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, Clone)]
pub struct MockMessage {
    pub id: String,
    pub content: String,
    pub role: String,
    pub timestamp: chrono::DateTime<chrono::Utc>,
    pub tool_calls: Option<Vec<ToolCall>>,
}

impl MockAppHandle {
    pub fn new() -> Self {
        Self {
            state: Arc::new(RwLock::new(MockAppState {
                settings: HashMap::new(),
                conversations: HashMap::new(),
                permissions: HashMap::new(),
                tool_configs: HashMap::new(),
            })),
            events: Arc::new(Mutex::new(Vec::new())),
        }
    }

    pub async fn emit_event(&self, event: &str) {
        let mut events = self.events.lock().unwrap();
        events.push(event.to_string());
    }

    pub async fn get_events(&self) -> Vec<String> {
        self.events.lock().unwrap().clone()
    }

    pub async fn set_setting(&self, key: &str, value: Value) {
        let mut state = self.state.write().await;
        state.settings.insert(key.to_string(), value);
    }

    pub async fn get_setting(&self, key: &str) -> Option<Value> {
        let state = self.state.read().await;
        state.settings.get(key).cloned()
    }

    pub async fn add_conversation(&self, conversation: MockConversation) {
        let mut state = self.state.write().await;
        state.conversations.insert(conversation.id.clone(), conversation);
    }

    pub async fn get_conversation(&self, id: &str) -> Option<MockConversation> {
        let state = self.state.read().await;
        state.conversations.get(id).cloned()
    }

    pub async fn set_permission(&self, permission: &str, granted: bool) {
        let mut state = self.state.write().await;
        state.permissions.insert(permission.to_string(), granted);
    }

    pub async fn has_permission(&self, permission: &str) -> bool {
        let state = self.state.read().await;
        state.permissions.get(permission).copied().unwrap_or(false)
    }
}

impl Default for MockAppHandle {
    fn default() -> Self {
        Self::new()
    }
}

/// Mock external API responses
#[derive(Debug, Clone)]
pub struct MockApiResponse {
    pub status: u16,
    pub body: String,
    pub headers: HashMap<String, String>,
    pub latency_ms: u64,
}

impl MockApiResponse {
    pub fn success(body: &str) -> Self {
        Self {
            status: 200,
            body: body.to_string(),
            headers: HashMap::new(),
            latency_ms: 100,
        }
    }

    pub fn error(status: u16, message: &str) -> Self {
        Self {
            status,
            body: message.to_string(),
            headers: HashMap::new(),
            latency_ms: 50,
        }
    }

    pub fn with_latency(mut self, latency_ms: u64) -> Self {
        self.latency_ms = latency_ms;
        self
    }
}

/// Mock HTTP client for testing external API calls
pub struct MockHttpClient {
    responses: Arc<Mutex<HashMap<String, MockApiResponse>>>,
    request_log: Arc<Mutex<Vec<MockHttpRequest>>>,
}

#[derive(Debug, Clone)]
pub struct MockHttpRequest {
    pub method: String,
    pub url: String,
    pub headers: HashMap<String, String>,
    pub body: Option<String>,
    pub timestamp: chrono::DateTime<chrono::Utc>,
}

impl MockHttpClient {
    pub fn new() -> Self {
        Self {
            responses: Arc::new(Mutex::new(HashMap::new())),
            request_log: Arc::new(Mutex::new(Vec::new())),
        }
    }

    pub fn expect_request(&self, method: &str, url: &str, response: MockApiResponse) {
        let key = format!("{} {}", method, url);
        let mut responses = self.responses.lock().unwrap();
        responses.insert(key, response);
    }

    pub async fn make_request(
        &self,
        method: &str,
        url: &str,
        headers: HashMap<String, String>,
        body: Option<String>,
    ) -> Result<MockApiResponse, String> {
        // Log the request
        let request = MockHttpRequest {
            method: method.to_string(),
            url: url.to_string(),
            headers,
            body,
            timestamp: chrono::Utc::now(),
        };

        {
            let mut log = self.request_log.lock().unwrap();
            log.push(request);
        }

        // Find matching response
        let key = format!("{} {}", method, url);
        let responses = self.responses.lock().unwrap();
        
        if let Some(response) = responses.get(&key) {
            // Simulate network latency
            tokio::time::sleep(std::time::Duration::from_millis(response.latency_ms)).await;
            Ok(response.clone())
        } else {
            Err(format!("No mock response configured for {} {}", method, url))
        }
    }

    pub fn get_requests(&self) -> Vec<MockHttpRequest> {
        self.request_log.lock().unwrap().clone()
    }

    pub fn clear_requests(&self) {
        self.request_log.lock().unwrap().clear();
    }
}

impl Default for MockHttpClient {
    fn default() -> Self {
        Self::new()
    }
}

/// Mock file system operations
pub struct MockFileSystem {
    files: Arc<Mutex<HashMap<String, MockFile>>>,
    operations: Arc<Mutex<Vec<MockFileOperation>>>,
}

#[derive(Debug, Clone)]
pub struct MockFile {
    pub path: String,
    pub content: Vec<u8>,
    pub permissions: u32,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub modified_at: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, Clone)]
pub struct MockFileOperation {
    pub operation: String,
    pub path: String,
    pub success: bool,
    pub timestamp: chrono::DateTime<chrono::Utc>,
}

impl MockFileSystem {
    pub fn new() -> Self {
        Self {
            files: Arc::new(Mutex::new(HashMap::new())),
            operations: Arc::new(Mutex::new(Vec::new())),
        }
    }

    pub fn create_file(&self, path: &str, content: &[u8]) -> Result<(), String> {
        let operation = MockFileOperation {
            operation: "create".to_string(),
            path: path.to_string(),
            success: true,
            timestamp: chrono::Utc::now(),
        };

        let file = MockFile {
            path: path.to_string(),
            content: content.to_vec(),
            permissions: 0o644,
            created_at: chrono::Utc::now(),
            modified_at: chrono::Utc::now(),
        };

        {
            let mut files = self.files.lock().unwrap();
            files.insert(path.to_string(), file);
        }

        {
            let mut operations = self.operations.lock().unwrap();
            operations.push(operation);
        }

        Ok(())
    }

    pub fn read_file(&self, path: &str) -> Result<Vec<u8>, String> {
        let operation = MockFileOperation {
            operation: "read".to_string(),
            path: path.to_string(),
            success: true,
            timestamp: chrono::Utc::now(),
        };

        let result = {
            let files = self.files.lock().unwrap();
            files.get(path).map(|f| f.content.clone())
        };

        {
            let mut operations = self.operations.lock().unwrap();
            operations.push(operation);
        }

        result.ok_or_else(|| format!("File not found: {}", path))
    }

    pub fn write_file(&self, path: &str, content: &[u8]) -> Result<(), String> {
        let operation = MockFileOperation {
            operation: "write".to_string(),
            path: path.to_string(),
            success: true,
            timestamp: chrono::Utc::now(),
        };

        {
            let mut files = self.files.lock().unwrap();
            if let Some(file) = files.get_mut(path) {
                file.content = content.to_vec();
                file.modified_at = chrono::Utc::now();
            } else {
                return Err(format!("File not found: {}", path));
            }
        }

        {
            let mut operations = self.operations.lock().unwrap();
            operations.push(operation);
        }

        Ok(())
    }

    pub fn delete_file(&self, path: &str) -> Result<(), String> {
        let operation = MockFileOperation {
            operation: "delete".to_string(),
            path: path.to_string(),
            success: true,
            timestamp: chrono::Utc::now(),
        };

        {
            let mut files = self.files.lock().unwrap();
            files.remove(path);
        }

        {
            let mut operations = self.operations.lock().unwrap();
            operations.push(operation);
        }

        Ok(())
    }

    pub fn file_exists(&self, path: &str) -> bool {
        let files = self.files.lock().unwrap();
        files.contains_key(path)
    }

    pub fn get_operations(&self) -> Vec<MockFileOperation> {
        self.operations.lock().unwrap().clone()
    }

    pub fn clear_operations(&self) {
        self.operations.lock().unwrap().clear();
    }
}

impl Default for MockFileSystem {
    fn default() -> Self {
        Self::new()
    }
}

/// Mock AI agent for testing tool interactions
pub struct MockAgent {
    responses: Arc<Mutex<Vec<AgentResponse>>>,
    call_log: Arc<Mutex<Vec<MockAgentCall>>>,
}

#[derive(Debug, Clone)]
pub struct MockAgentCall {
    pub query: String,
    pub context: HashMap<String, Value>,
    pub timestamp: chrono::DateTime<chrono::Utc>,
    pub response_time_ms: u64,
}

impl MockAgent {
    pub fn new() -> Self {
        Self {
            responses: Arc::new(Mutex::new(Vec::new())),
            call_log: Arc::new(Mutex::new(Vec::new())),
        }
    }

    pub fn add_response(&self, response: AgentResponse) {
        let mut responses = self.responses.lock().unwrap();
        responses.push(response);
    }

    pub async fn process_query(
        &self,
        query: &str,
        context: HashMap<String, Value>,
    ) -> Result<AgentResponse, AgentError> {
        let start_time = std::time::Instant::now();

        // Log the call
        let call = MockAgentCall {
            query: query.to_string(),
            context,
            timestamp: chrono::Utc::now(),
            response_time_ms: 0, // Will be updated below
        };

        // Get pre-configured response or create default
        let response = {
            let mut responses = self.responses.lock().unwrap();
            if !responses.is_empty() {
                responses.remove(0)
            } else {
                AgentResponse {
                    content: format!("Mock response to: {}", query),
                    tool_calls: Vec::new(),
                    conversation_id: Some(uuid::Uuid::new_v4().to_string()),
                    message_id: Some(uuid::Uuid::new_v4().to_string()),
                    success: true,
                    error_message: None,
                    execution_time_ms: Some(100),
                    tokens_used: Some(50),
                }
            }
        };

        // Update call log with actual response time
        let mut call_with_time = call;
        call_with_time.response_time_ms = start_time.elapsed().as_millis() as u64;

        {
            let mut log = self.call_log.lock().unwrap();
            log.push(call_with_time);
        }

        Ok(response)
    }

    pub fn get_calls(&self) -> Vec<MockAgentCall> {
        self.call_log.lock().unwrap().clone()
    }

    pub fn clear_calls(&self) {
        self.call_log.lock().unwrap().clear();
    }
}

impl Default for MockAgent {
    fn default() -> Self {
        Self::new()
    }
}

/// Helper function to create a test environment with all mocks
pub fn create_test_environment() -> TestEnvironment {
    TestEnvironment {
        app_handle: MockAppHandle::new(),
        http_client: MockHttpClient::new(),
        file_system: MockFileSystem::new(),
        agent: MockAgent::new(),
    }
}

pub struct TestEnvironment {
    pub app_handle: MockAppHandle,
    pub http_client: MockHttpClient,
    pub file_system: MockFileSystem,
    pub agent: MockAgent,
}

impl TestEnvironment {
    pub async fn setup_basic_permissions(&self) {
        self.app_handle.set_permission("accessibility", true).await;
        self.app_handle.set_permission("screen_recording", true).await;
        self.app_handle.set_permission("microphone", false).await;
    }

    pub async fn setup_sample_conversation(&self) -> String {
        let conversation = MockConversation {
            id: uuid::Uuid::new_v4().to_string(),
            messages: vec![
                MockMessage {
                    id: uuid::Uuid::new_v4().to_string(),
                    content: "Hello, can you help me?".to_string(),
                    role: "user".to_string(),
                    timestamp: chrono::Utc::now(),
                    tool_calls: None,
                },
                MockMessage {
                    id: uuid::Uuid::new_v4().to_string(),
                    content: "Of course! How can I assist you?".to_string(),
                    role: "assistant".to_string(),
                    timestamp: chrono::Utc::now(),
                    tool_calls: None,
                },
            ],
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
        };

        let conversation_id = conversation.id.clone();
        self.app_handle.add_conversation(conversation).await;
        conversation_id
    }

    pub fn setup_sample_files(&self) {
        self.file_system.create_file("test.txt", b"Hello, World!").unwrap();
        self.file_system.create_file("config.json", br#"{"setting": "value"}"#).unwrap();
        self.file_system.create_file("large_file.txt", &vec![b'A'; 1024 * 1024]).unwrap(); // 1MB file
    }

    pub async fn teardown(&self) {
        // Clear all mock data
        self.http_client.clear_requests();
        self.file_system.clear_operations();
        self.agent.clear_calls();
        // App handle state is automatically dropped
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_mock_app_handle() {
        let handle = MockAppHandle::new();
        
        handle.set_setting("test_key", Value::String("test_value".to_string())).await;
        let value = handle.get_setting("test_key").await;
        
        assert_eq!(value, Some(Value::String("test_value".to_string())));
    }

    #[tokio::test]
    async fn test_mock_http_client() {
        let client = MockHttpClient::new();
        
        client.expect_request("GET", "https://api.example.com/test", 
            MockApiResponse::success(r#"{"result": "success"}"#));
        
        let response = client.make_request("GET", "https://api.example.com/test", 
            HashMap::new(), None).await.unwrap();
        
        assert_eq!(response.status, 200);
        assert_eq!(response.body, r#"{"result": "success"}"#);
        
        let requests = client.get_requests();
        assert_eq!(requests.len(), 1);
        assert_eq!(requests[0].method, "GET");
        assert_eq!(requests[0].url, "https://api.example.com/test");
    }

    #[test]
    fn test_mock_file_system() {
        let fs = MockFileSystem::new();
        
        fs.create_file("test.txt", b"Hello, World!").unwrap();
        assert!(fs.file_exists("test.txt"));
        
        let content = fs.read_file("test.txt").unwrap();
        assert_eq!(content, b"Hello, World!");
        
        fs.write_file("test.txt", b"Updated content").unwrap();
        let updated_content = fs.read_file("test.txt").unwrap();
        assert_eq!(updated_content, b"Updated content");
        
        fs.delete_file("test.txt").unwrap();
        assert!(!fs.file_exists("test.txt"));
        
        let operations = fs.get_operations();
        assert_eq!(operations.len(), 4); // create, read, write, delete
    }

    #[tokio::test]
    async fn test_mock_agent() {
        let agent = MockAgent::new();
        
        let expected_response = AgentResponse {
            content: "Test response".to_string(),
            tool_calls: Vec::new(),
            conversation_id: Some("test_conv".to_string()),
            message_id: Some("test_msg".to_string()),
            success: true,
            error_message: None,
            execution_time_ms: Some(50),
            tokens_used: Some(25),
        };
        
        agent.add_response(expected_response.clone());
        
        let response = agent.process_query("Test query", HashMap::new()).await.unwrap();
        assert_eq!(response.content, "Test response");
        
        let calls = agent.get_calls();
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].query, "Test query");
    }

    #[tokio::test]
    async fn test_test_environment() {
        let env = create_test_environment();
        
        env.setup_basic_permissions().await;
        assert!(env.app_handle.has_permission("accessibility").await);
        assert!(env.app_handle.has_permission("screen_recording").await);
        assert!(!env.app_handle.has_permission("microphone").await);
        
        let conversation_id = env.setup_sample_conversation().await;
        let conversation = env.app_handle.get_conversation(&conversation_id).await;
        assert!(conversation.is_some());
        assert_eq!(conversation.unwrap().messages.len(), 2);
        
        env.setup_sample_files();
        assert!(env.file_system.file_exists("test.txt"));
        assert!(env.file_system.file_exists("config.json"));
        assert!(env.file_system.file_exists("large_file.txt"));
        
        env.teardown().await;
    }
}
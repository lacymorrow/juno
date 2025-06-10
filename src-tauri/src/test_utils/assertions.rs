/// Custom assertions for Juno AI Computer Use Agent testing
/// 
/// This module provides specialized assertion helpers for:
/// - Agent response validation
/// - Tool call verification
/// - Security constraint checking
/// - Performance validation
/// - State consistency checking

use std::time::Duration;
use serde_json::Value;

use crate::agent::structs::{AgentResponse, ToolCall};

/// Agent-specific assertions
pub struct AgentAssertions;

impl AgentAssertions {
    /// Assert that an agent response is successful
    pub fn assert_success(response: &AgentResponse, context: &str) {
        assert!(
            response.success,
            "Agent response should be successful for '{}': {:?}",
            context, response.error_message
        );
    }

    /// Assert that an agent response failed with expected error
    pub fn assert_failure(response: &AgentResponse, expected_error: Option<&str>, context: &str) {
        assert!(
            !response.success,
            "Agent response should have failed for '{}' but was successful",
            context
        );

        if let Some(expected) = expected_error {
            if let Some(error_msg) = &response.error_message {
                assert!(
                    error_msg.contains(expected),
                    "Error message '{}' should contain '{}' for context '{}'",
                    error_msg, expected, context
                );
            } else {
                panic!("Expected error message containing '{}' but got None for context '{}'", expected, context);
            }
        }
    }

    /// Assert response content contains expected text
    pub fn assert_content_contains(response: &AgentResponse, expected: &str, context: &str) {
        assert!(
            response.content.contains(expected),
            "Response content should contain '{}' for '{}'. Got: '{}'",
            expected, context, response.content
        );
    }

    /// Assert response has tool calls
    pub fn assert_has_tool_calls(response: &AgentResponse, min_count: usize, context: &str) {
        assert!(
            response.tool_calls.len() >= min_count,
            "Response should have at least {} tool calls for '{}'. Got: {}",
            min_count, context, response.tool_calls.len()
        );
    }

    /// Assert specific tool was called
    pub fn assert_tool_called(response: &AgentResponse, tool_name: &str, context: &str) {
        let tool_found = response.tool_calls.iter()
            .any(|call| call.name == tool_name);
        
        assert!(
            tool_found,
            "Tool '{}' should have been called for '{}'. Called tools: {:?}",
            tool_name, context, response.tool_calls.iter().map(|c| &c.name).collect::<Vec<_>>()
        );
    }

    /// Assert response time is within bounds
    pub fn assert_response_time(response: &AgentResponse, max_ms: u64, context: &str) {
        if let Some(execution_time) = response.execution_time_ms {
            assert!(
                execution_time <= max_ms,
                "Response time should be <= {}ms for '{}'. Got: {}ms",
                max_ms, context, execution_time
            );
        } else {
            panic!("Response should have execution time for context '{}'", context);
        }
    }

    /// Assert token usage is reasonable
    pub fn assert_reasonable_token_usage(response: &AgentResponse, max_tokens: u32, context: &str) {
        if let Some(tokens_used) = response.tokens_used {
            assert!(
                tokens_used <= max_tokens,
                "Token usage should be <= {} for '{}'. Got: {}",
                max_tokens, context, tokens_used
            );
        }
    }
}

/// Tool call assertions
pub struct ToolAssertions;

impl ToolAssertions {
    /// Assert tool call has required parameters
    pub fn assert_has_parameter(tool_call: &ToolCall, param_name: &str, context: &str) {
        assert!(
            tool_call.input.get(param_name).is_some(),
            "Tool call '{}' should have parameter '{}' for context '{}'",
            tool_call.name, param_name, context
        );
    }

    /// Assert parameter has expected value
    pub fn assert_parameter_equals(
        tool_call: &ToolCall,
        param_name: &str,
        expected_value: &Value,
        context: &str,
    ) {
        let actual_value = tool_call.input.get(param_name);
        assert!(
            actual_value == Some(expected_value),
            "Parameter '{}' should equal {:?} for tool '{}' in context '{}'. Got: {:?}",
            param_name, expected_value, tool_call.name, context, actual_value
        );
    }

    /// Assert parameter is within range (for numeric values)
    pub fn assert_parameter_in_range(
        tool_call: &ToolCall,
        param_name: &str,
        min_val: f64,
        max_val: f64,
        context: &str,
    ) {
        let param_value = tool_call.input.get(param_name);
        if let Some(Value::Number(num)) = param_value {
            let val = num.as_f64().unwrap_or(0.0);
            assert!(
                val >= min_val && val <= max_val,
                "Parameter '{}' should be in range [{}, {}] for tool '{}' in context '{}'. Got: {}",
                param_name, min_val, max_val, tool_call.name, context, val
            );
        } else {
            panic!(
                "Parameter '{}' should be a number for tool '{}' in context '{}'. Got: {:?}",
                param_name, tool_call.name, context, param_value
            );
        }
    }

    /// Assert tool call has valid ID
    pub fn assert_valid_id(tool_call: &ToolCall, context: &str) {
        assert!(
            !tool_call.id.is_empty(),
            "Tool call should have non-empty ID for context '{}'",
            context
        );
        
        assert!(
            tool_call.id.len() >= 4,
            "Tool call ID should be at least 4 characters for context '{}'. Got: '{}'",
            context, tool_call.id
        );
    }
}

/// Security assertions
pub struct SecurityAssertions;

impl SecurityAssertions {
    /// Assert file path is safe (no path traversal)
    pub fn assert_safe_file_path(path: &str, context: &str) {
        assert!(
            !path.contains(".."),
            "File path should not contain '..' for security in context '{}'. Got: '{}'",
            context, path
        );
        
        assert!(
            !path.starts_with('/'),
            "File path should not be absolute for security in context '{}'. Got: '{}'",
            context, path
        );
    }

    /// Assert command is whitelisted
    pub fn assert_safe_command(command: &str, context: &str) {
        let dangerous_patterns = [
            "rm ", "del ", "format", "sudo", "su ", "chmod 777",
            "; ", " | ", " && ", "$(", "`", "curl", "wget",
            "nc ", "netcat", "bash -c", "sh -c", "eval",
        ];
        
        for pattern in &dangerous_patterns {
            assert!(
                !command.contains(pattern),
                "Command contains dangerous pattern '{}' in context '{}'. Command: '{}'",
                pattern, context, command
            );
        }
    }

    /// Assert input size is within limits
    pub fn assert_input_size_limit(input: &str, max_bytes: usize, context: &str) {
        assert!(
            input.len() <= max_bytes,
            "Input size {} bytes exceeds limit {} bytes for context '{}'",
            input.len(), max_bytes, context
        );
    }

    /// Assert no sensitive data in logs
    pub fn assert_no_sensitive_data(text: &str, context: &str) {
        let sensitive_patterns = [
            "password", "api_key", "secret", "token", "credential",
            "ssh_key", "private_key", "access_token", "refresh_token",
        ];
        
        let text_lower = text.to_lowercase();
        for pattern in &sensitive_patterns {
            assert!(
                !text_lower.contains(pattern),
                "Text contains sensitive pattern '{}' in context '{}'. Partial text: '{}'",
                pattern, context, &text[..text.len().min(100)]
            );
        }
    }
}

/// State consistency assertions
pub struct StateAssertions;

impl StateAssertions {
    /// Assert state transition is valid
    pub fn assert_valid_state_transition(
        from_state: &str,
        to_state: &str,
        allowed_transitions: &[(&str, &str)],
        context: &str,
    ) {
        let transition_allowed = allowed_transitions
            .iter()
            .any(|(from, to)| from == &from_state && to == &to_state);
        
        assert!(
            transition_allowed,
            "Invalid state transition from '{}' to '{}' in context '{}'. Allowed transitions: {:?}",
            from_state, to_state, context, allowed_transitions
        );
    }

    /// Assert required fields are present
    pub fn assert_required_fields<T>(
        object: &T,
        field_checks: &[(&str, fn(&T) -> bool)],
        context: &str,
    ) {
        for (field_name, check_fn) in field_checks {
            assert!(
                check_fn(object),
                "Required field '{}' is missing or invalid in context '{}'",
                field_name, context
            );
        }
    }
}

/// Performance assertions (extends the performance module)
pub struct ExtendedPerformanceAssertions;

impl ExtendedPerformanceAssertions {
    /// Assert operation completed within timeout
    pub fn assert_completed_within_timeout<T>(
        result: Result<T, String>,
        max_duration: Duration,
        actual_duration: Duration,
        operation: &str,
    ) {
        assert!(
            result.is_ok(),
            "Operation '{}' should have completed successfully within {:?}. Error: {:?}",
            operation, max_duration, result.err()
        );
        
        assert!(
            actual_duration <= max_duration,
            "Operation '{}' took {:?} but should complete within {:?}",
            operation, actual_duration, max_duration
        );
    }

    /// Assert memory usage didn't increase significantly
    pub fn assert_memory_stable(
        before_bytes: usize,
        after_bytes: usize,
        max_increase_percent: f64,
        operation: &str,
    ) {
        let increase = after_bytes.saturating_sub(before_bytes);
        let increase_percent = (increase as f64 / before_bytes as f64) * 100.0;
        
        assert!(
            increase_percent <= max_increase_percent,
            "Memory increased by {:.1}% ({} bytes) during '{}', expected <= {:.1}%",
            increase_percent, increase, operation, max_increase_percent
        );
    }

    /// Assert concurrent operations don't interfere
    pub fn assert_no_race_conditions<T: PartialEq + std::fmt::Debug>(
        results: &[T],
        expected_result: &T,
        operation: &str,
    ) {
        for (i, result) in results.iter().enumerate() {
            assert!(
                result == expected_result,
                "Concurrent operation {} for '{}' produced different result: {:?} != {:?}",
                i, operation, result, expected_result
            );
        }
    }
}

/// Helper macros for common assertion patterns
#[macro_export]
macro_rules! assert_agent_success {
    ($response:expr) => {
        $crate::test_utils::assertions::AgentAssertions::assert_success($response, "test")
    };
    ($response:expr, $context:expr) => {
        $crate::test_utils::assertions::AgentAssertions::assert_success($response, $context)
    };
}

#[macro_export]
macro_rules! assert_tool_called {
    ($response:expr, $tool:expr) => {
        $crate::test_utils::assertions::AgentAssertions::assert_tool_called($response, $tool, "test")
    };
    ($response:expr, $tool:expr, $context:expr) => {
        $crate::test_utils::assertions::AgentAssertions::assert_tool_called($response, $tool, $context)
    };
}

#[macro_export]
macro_rules! assert_safe_path {
    ($path:expr) => {
        $crate::test_utils::assertions::SecurityAssertions::assert_safe_file_path($path, "test")
    };
    ($path:expr, $context:expr) => {
        $crate::test_utils::assertions::SecurityAssertions::assert_safe_file_path($path, $context)
    };
}

/// Collection of all assertion helpers
pub struct Assertions;

impl Assertions {
    pub fn agent() -> AgentAssertions {
        AgentAssertions
    }
    
    pub fn tool() -> ToolAssertions {
        ToolAssertions
    }
    
    pub fn security() -> SecurityAssertions {
        SecurityAssertions
    }
    
    pub fn state() -> StateAssertions {
        StateAssertions
    }
    
    pub fn performance() -> ExtendedPerformanceAssertions {
        ExtendedPerformanceAssertions
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    fn create_test_response(success: bool, content: &str) -> AgentResponse {
        AgentResponse {
            content: content.to_string(),
            tool_calls: Vec::new(),
            conversation_id: Some("test_conv".to_string()),
            message_id: Some("test_msg".to_string()),
            success,
            error_message: if success { None } else { Some("Test error".to_string()) },
            execution_time_ms: Some(100),
            tokens_used: Some(50),
        }
    }

    fn create_test_tool_call(name: &str, params: HashMap<String, Value>) -> ToolCall {
        ToolCall {
            id: "test_id_123".to_string(),
            name: name.to_string(),
            input: Value::Object(params.into_iter().collect()),
        }
    }

    #[test]
    fn test_agent_success_assertion() {
        let response = create_test_response(true, "Success");
        AgentAssertions::assert_success(&response, "test");
    }

    #[test]
    #[should_panic(expected = "Agent response should be successful")]
    fn test_agent_success_assertion_failure() {
        let response = create_test_response(false, "Error");
        AgentAssertions::assert_success(&response, "test");
    }

    #[test]
    fn test_agent_failure_assertion() {
        let response = create_test_response(false, "Error");
        AgentAssertions::assert_failure(&response, Some("Test error"), "test");
    }

    #[test]
    fn test_content_contains_assertion() {
        let response = create_test_response(true, "Hello World");
        AgentAssertions::assert_content_contains(&response, "World", "test");
    }

    #[test]
    fn test_tool_parameter_assertions() {
        let mut params = HashMap::new();
        params.insert("x".to_string(), Value::Number(100.into()));
        params.insert("y".to_string(), Value::Number(200.into()));
        
        let tool_call = create_test_tool_call("click", params);
        
        ToolAssertions::assert_has_parameter(&tool_call, "x", "test");
        ToolAssertions::assert_parameter_in_range(&tool_call, "x", 0.0, 1000.0, "test");
        ToolAssertions::assert_valid_id(&tool_call, "test");
    }

    #[test]
    fn test_security_assertions() {
        SecurityAssertions::assert_safe_file_path("documents/file.txt", "test");
        SecurityAssertions::assert_safe_command("ls -la", "test");
        SecurityAssertions::assert_input_size_limit("small input", 1000, "test");
        SecurityAssertions::assert_no_sensitive_data("public information", "test");
    }

    #[test]
    #[should_panic(expected = "File path should not contain")]
    fn test_security_path_traversal_detection() {
        SecurityAssertions::assert_safe_file_path("../../../etc/passwd", "test");
    }

    #[test]
    #[should_panic(expected = "Command contains dangerous pattern")]
    fn test_security_dangerous_command_detection() {
        SecurityAssertions::assert_safe_command("rm -rf /", "test");
    }

    #[test]
    fn test_state_transition_assertion() {
        let allowed_transitions = [
            ("idle", "thinking"),
            ("thinking", "executing"),
            ("executing", "responding"),
            ("responding", "finished"),
        ];
        
        StateAssertions::assert_valid_state_transition(
            "idle",
            "thinking",
            &allowed_transitions,
            "test"
        );
    }

    #[test]
    #[should_panic(expected = "Invalid state transition")]
    fn test_invalid_state_transition() {
        let allowed_transitions = [("idle", "thinking")];
        
        StateAssertions::assert_valid_state_transition(
            "idle",
            "finished", // Invalid transition
            &allowed_transitions,
            "test"
        );
    }

    #[test]
    fn test_performance_assertions() {
        let result: Result<String, String> = Ok("success".to_string());
        ExtendedPerformanceAssertions::assert_completed_within_timeout(
            result,
            Duration::from_secs(1),
            Duration::from_millis(500),
            "test operation"
        );
        
        ExtendedPerformanceAssertions::assert_memory_stable(
            1024 * 1024, // 1MB before
            1024 * 1024 + 1024, // 1MB + 1KB after
            5.0, // 5% allowed increase
            "test operation"
        );
    }

    #[test]
    fn test_no_race_conditions() {
        let results = vec!["result1", "result1", "result1"];
        ExtendedPerformanceAssertions::assert_no_race_conditions(
            &results,
            &"result1",
            "concurrent test"
        );
    }

    #[test]
    fn test_assertion_factory() {
        let _agent_assertions = Assertions::agent();
        let _tool_assertions = Assertions::tool();
        let _security_assertions = Assertions::security();
        let _state_assertions = Assertions::state();
        let _performance_assertions = Assertions::performance();
    }
}
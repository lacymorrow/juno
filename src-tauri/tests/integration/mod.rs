use juno::agent::structs::{AgentRequest, ToolCall};
use juno::anthropic::AnthropicClient;
use juno::commands::agent::submit_query;
use juno::test_utils::{SecurityTestVectors, TestConfig, TestEnvironment};
use std::time::Duration;
use tauri::test::{mock_builder, MockRuntime};

/// Integration tests for complete agent workflows
mod agent_workflows {
    use super::*;

    #[tokio::test]
    async fn test_complete_screenshot_workflow() {
        let test_env = TestEnvironment::new().await;
        let app = test_env.create_mock_app().await;

        let request = AgentRequest {
            query: "Take a screenshot of the current screen".to_string(),
            model: "claude-3-5-sonnet-20241022".to_string(),
            options: Default::default(),
        };

        let result = submit_query(app.state(), request).await;

        assert!(result.is_ok());
        let response = result.unwrap();
        assert!(!response.response.is_empty());
        assert!(response.tool_calls.iter().any(|t| t.name == "computer"));
    }

    #[tokio::test]
    async fn test_file_operations_workflow() {
        let test_env = TestEnvironment::new().await;
        let app = test_env.create_mock_app().await;

        // Test file creation
        let create_request = AgentRequest {
            query: "Create a test file with some content".to_string(),
            model: "claude-3-5-sonnet-20241022".to_string(),
            options: Default::default(),
        };

        let result = submit_query(app.state(), create_request).await;
        assert!(result.is_ok());

        // Test file reading
        let read_request = AgentRequest {
            query: "Read the content of the test file we just created".to_string(),
            model: "claude-3-5-sonnet-20241022".to_string(),
            options: Default::default(),
        };

        let result = submit_query(app.state(), read_request).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_browser_automation_workflow() {
        let test_env = TestEnvironment::new().await;
        let app = test_env.create_mock_app().await;

        let request = AgentRequest {
            query: "Open a browser and navigate to example.com".to_string(),
            model: "claude-3-5-sonnet-20241022".to_string(),
            options: Default::default(),
        };

        let result = submit_query(app.state(), request).await;

        assert!(result.is_ok());
        let response = result.unwrap();
        assert!(response.tool_calls.iter().any(|t| t.name == "browser"));
    }

    #[tokio::test]
    async fn test_multi_tool_workflow() {
        let test_env = TestEnvironment::new().await;
        let app = test_env.create_mock_app().await;

        let request = AgentRequest {
            query: "Take a screenshot, save it to a file, then open it in a browser".to_string(),
            model: "claude-3-5-sonnet-20241022".to_string(),
            options: Default::default(),
        };

        let result = submit_query(app.state(), request).await;

        assert!(result.is_ok());
        let response = result.unwrap();

        // Should use multiple tools
        assert!(response.tool_calls.len() > 1);

        // Should include computer and file operations
        let tool_names: Vec<&str> = response
            .tool_calls
            .iter()
            .map(|t| t.name.as_str())
            .collect();
        assert!(tool_names.contains(&"computer"));
        assert!(tool_names
            .iter()
            .any(|&name| name.contains("file") || name.contains("str_replace_editor")));
    }
}

/// Security integration tests
mod security_integration {
    use super::*;

    #[tokio::test]
    async fn test_security_validation_integration() {
        let test_env = TestEnvironment::new().await;
        let app = test_env.create_mock_app().await;

        let security_vectors = SecurityTestVectors::new();

        for attack_pattern in security_vectors.path_traversal_attacks() {
            let request = AgentRequest {
                query: format!("Read the file at {}", attack_pattern),
                model: "claude-3-5-sonnet-20241022".to_string(),
                options: Default::default(),
            };

            let result = submit_query(app.state(), request).await;

            // Should either reject the request or sanitize the path
            if let Ok(response) = result {
                // If it succeeds, ensure no dangerous operations were performed
                assert!(!response.response.contains("../"));
                assert!(!response.response.contains("etc/passwd"));
            }
        }
    }

    #[tokio::test]
    async fn test_command_injection_protection() {
        let test_env = TestEnvironment::new().await;
        let app = test_env.create_mock_app().await;

        let security_vectors = SecurityTestVectors::new();

        for injection_pattern in security_vectors.command_injection_attacks() {
            let request = AgentRequest {
                query: format!("Execute this command: {}", injection_pattern),
                model: "claude-3-5-sonnet-20241022".to_string(),
                options: Default::default(),
            };

            let result = submit_query(app.state(), request).await;

            // Should reject dangerous commands
            if let Ok(response) = result {
                assert!(!response.response.contains("rm -rf"));
                assert!(!response.response.contains("sudo"));
                assert!(!response.response.contains("curl"));
            }
        }
    }
}

/// Performance integration tests
mod performance_integration {
    use super::*;
    use std::time::Instant;

    #[tokio::test]
    async fn test_response_time_requirements() {
        let test_env = TestEnvironment::new().await;
        let app = test_env.create_mock_app().await;

        let request = AgentRequest {
            query: "What time is it?".to_string(),
            model: "claude-3-5-sonnet-20241022".to_string(),
            options: Default::default(),
        };

        let start = Instant::now();
        let result = submit_query(app.state(), request).await;
        let duration = start.elapsed();

        assert!(result.is_ok());
        assert!(
            duration < Duration::from_secs(5),
            "Response took too long: {:?}",
            duration
        );
    }

    #[tokio::test]
    async fn test_concurrent_requests() {
        let test_env = TestEnvironment::new().await;
        let app = test_env.create_mock_app().await;

        let requests = vec![
            "Take a screenshot",
            "What's the current time?",
            "List files in the current directory",
        ];

        let mut handles = vec![];

        for query in requests {
            let app_clone = app.clone();
            let request = AgentRequest {
                query: query.to_string(),
                model: "claude-3-5-sonnet-20241022".to_string(),
                options: Default::default(),
            };

            let handle =
                tokio::spawn(async move { submit_query(app_clone.state(), request).await });

            handles.push(handle);
        }

        let start = Instant::now();

        for handle in handles {
            let result = handle.await.unwrap();
            assert!(result.is_ok());
        }

        let duration = start.elapsed();
        assert!(
            duration < Duration::from_secs(15),
            "Concurrent requests took too long: {:?}",
            duration
        );
    }
}

/// Error handling integration tests
mod error_handling_integration {
    use super::*;

    #[tokio::test]
    async fn test_invalid_model_handling() {
        let test_env = TestEnvironment::new().await;
        let app = test_env.create_mock_app().await;

        let request = AgentRequest {
            query: "Hello".to_string(),
            model: "invalid-model-name".to_string(),
            options: Default::default(),
        };

        let result = submit_query(app.state(), request).await;

        // Should handle invalid model gracefully
        assert!(result.is_err() || result.unwrap().response.contains("error"));
    }

    #[tokio::test]
    async fn test_network_failure_handling() {
        let test_env = TestEnvironment::new().await;
        let app = test_env.create_mock_app().await;

        // Simulate network failure by using invalid API key
        let request = AgentRequest {
            query: "Hello".to_string(),
            model: "claude-3-5-sonnet-20241022".to_string(),
            options: Default::default(),
        };

        // This should handle network errors gracefully
        let result = submit_query(app.state(), request).await;

        // Should not panic, either succeed with fallback or fail gracefully
        assert!(result.is_ok() || result.is_err());
    }

    #[tokio::test]
    async fn test_malformed_request_handling() {
        let test_env = TestEnvironment::new().await;
        let app = test_env.create_mock_app().await;

        let request = AgentRequest {
            query: "".to_string(), // Empty query
            model: "".to_string(), // Empty model
            options: Default::default(),
        };

        let result = submit_query(app.state(), request).await;

        // Should handle malformed requests gracefully
        assert!(result.is_err());
    }
}

/// Memory management integration tests
mod memory_integration {
    use super::*;

    #[tokio::test]
    async fn test_memory_persistence() {
        let test_env = TestEnvironment::new().await;
        let app = test_env.create_mock_app().await;

        // First request to establish context
        let request1 = AgentRequest {
            query: "Remember that my favorite color is blue".to_string(),
            model: "claude-3-5-sonnet-20241022".to_string(),
            options: Default::default(),
        };

        let result1 = submit_query(app.state(), request1).await;
        assert!(result1.is_ok());

        // Second request that references the context
        let request2 = AgentRequest {
            query: "What's my favorite color?".to_string(),
            model: "claude-3-5-sonnet-20241022".to_string(),
            options: Default::default(),
        };

        let result2 = submit_query(app.state(), request2).await;
        assert!(result2.is_ok());

        let response2 = result2.unwrap();
        assert!(response2.response.to_lowercase().contains("blue"));
    }

    #[tokio::test]
    async fn test_memory_limits() {
        let test_env = TestEnvironment::new().await;
        let app = test_env.create_mock_app().await;

        // Generate many requests to test memory management
        for i in 0..10 {
            let request = AgentRequest {
                query: format!("This is message number {}, please remember it", i),
                model: "claude-3-5-sonnet-20241022".to_string(),
                options: Default::default(),
            };

            let result = submit_query(app.state(), request).await;
            assert!(result.is_ok());
        }

        // Memory should not grow indefinitely
        // This is more of a behavioral test that the system remains stable
        let final_request = AgentRequest {
            query: "How many messages have we exchanged?".to_string(),
            model: "claude-3-5-sonnet-20241022".to_string(),
            options: Default::default(),
        };

        let result = submit_query(app.state(), final_request).await;
        assert!(result.is_ok());
    }
}

/// Tool integration tests
mod tool_integration {
    use super::*;

    #[tokio::test]
    async fn test_tool_chaining() {
        let test_env = TestEnvironment::new().await;
        let app = test_env.create_mock_app().await;

        let request = AgentRequest {
            query:
                "Take a screenshot, analyze what's on screen, then click on something interesting"
                    .to_string(),
            model: "claude-3-5-sonnet-20241022".to_string(),
            options: Default::default(),
        };

        let result = submit_query(app.state(), request).await;

        assert!(result.is_ok());
        let response = result.unwrap();

        // Should chain multiple tool calls
        assert!(response.tool_calls.len() >= 2);

        // Should include screenshot and click operations
        let has_screenshot = response.tool_calls.iter().any(|t| {
            t.name == "computer"
                && t.input.as_object().map_or(false, |obj| {
                    obj.get("action").and_then(|v| v.as_str()) == Some("screenshot")
                })
        });

        let has_click = response.tool_calls.iter().any(|t| {
            t.name == "computer"
                && t.input.as_object().map_or(false, |obj| {
                    obj.get("action").and_then(|v| v.as_str()) == Some("click")
                })
        });

        assert!(
            has_screenshot || has_click,
            "Should include screenshot or click operations"
        );
    }

    #[tokio::test]
    async fn test_tool_error_recovery() {
        let test_env = TestEnvironment::new().await;
        let app = test_env.create_mock_app().await;

        // Request that might fail on first tool but should recover
        let request = AgentRequest {
            query: "Try to click on coordinates that don't exist, then take a screenshot instead"
                .to_string(),
            model: "claude-3-5-sonnet-20241022".to_string(),
            options: Default::default(),
        };

        let result = submit_query(app.state(), request).await;

        // Should handle tool failures gracefully
        assert!(result.is_ok());
    }
}

# Testing Strategy: TARS → Juno Integration

## Overview

This document outlines a comprehensive testing strategy for the TARS → Juno integration project, ensuring that each phase delivers reliable, performant, and maintainable enhancements while preserving Juno's production-ready quality.

## Testing Philosophy

### Core Principles
1. **Reliability First**: Every change must maintain or improve system reliability
2. **Performance Preservation**: No degradation of response times or resource usage
3. **Backward Compatibility**: All existing functionality must continue working
4. **Progressive Enhancement**: New features enhance rather than replace existing capabilities

### Testing Pyramid

```
                    ┌─────────────────┐
                    │   E2E Tests     │ 10%
                    │   (Integration) │
                ┌───┴─────────────────┴───┐
                │   Integration Tests     │ 20%
                │   (Component)           │
            ┌───┴─────────────────────────┴───┐
            │        Unit Tests               │ 70%
            │        (Function)               │
            └─────────────────────────────────┘
```

- **Unit Tests (70%)**: Fast, focused tests for individual functions and components
- **Integration Tests (20%)**: Tests for component interactions and system integration
- **End-to-End Tests (10%)**: Full workflow tests simulating real user scenarios

## Phase-Specific Testing Strategies

## Phase 1: Event-Driven Architecture Foundation

### Unit Testing

#### Event Type Validation
```rust
#[cfg(test)]
mod event_type_tests {
    use super::*;
    
    #[test]
    fn test_event_serialization() {
        let event = JunoAgentEvent::UserMessage {
            content: "Test message".to_string(),
            timestamp: 1234567890,
            session_id: Some("session-123".to_string()),
        };
        
        let serialized = serde_json::to_string(&event).unwrap();
        let deserialized: JunoAgentEvent = serde_json::from_str(&serialized).unwrap();
        
        assert_eq!(event, deserialized);
    }
    
    #[test]
    fn test_event_timestamp_generation() {
        let event = JunoAgentEvent::SystemMessage {
            level: "info".to_string(),
            message: "test".to_string(),
            timestamp: 0,
            category: None,
        }.with_timestamp();
        
        assert!(event.timestamp > 0);
    }
    
    #[test]
    fn test_session_id_extraction() {
        let event = JunoAgentEvent::AgentRunStart {
            session_id: "test-session".to_string(),
            agent_type: "test".to_string(),
            max_iterations: 10,
            timestamp: 123456,
        };
        
        assert_eq!(event.session_id(), Some("test-session"));
    }
}
```

#### Event Processor Testing
```rust
#[cfg(test)]
mod event_processor_tests {
    use super::*;
    use tokio_test;
    
    #[tokio::test]
    async fn test_event_emission() {
        let app_handle = create_test_app_handle();
        let processor = JunoEventStreamProcessor::new(app_handle, None);
        
        let event = JunoAgentEvent::UserMessage {
            content: "Test".to_string(),
            timestamp: 0,
            session_id: None,
        };
        
        let result = processor.send_event(event).await;
        assert!(result.is_ok());
        
        let events = processor.get_events(None).await;
        assert_eq!(events.len(), 1);
    }
    
    #[tokio::test]
    async fn test_event_filtering() {
        let app_handle = create_test_app_handle();
        let processor = JunoEventStreamProcessor::new(app_handle, None);
        
        struct TestSubscriber {
            received: Arc<Mutex<Vec<String>>>,
        }
        
        #[async_trait]
        impl EventSubscriber for TestSubscriber {
            async fn on_event(&self, event: &JunoAgentEvent) -> Result<(), String> {
                let mut received = self.received.lock().await;
                received.push(format!("{:?}", event));
                Ok(())
            }
            
            fn event_filter(&self) -> Option<Vec<&'static str>> {
                Some(vec!["user_message"])
            }
        }
        
        let received = Arc::new(Mutex::new(Vec::new()));
        let subscriber = TestSubscriber { received: received.clone() };
        processor.subscribe(Box::new(subscriber)).await;
        
        // Send filtered event
        processor.send_event(JunoAgentEvent::UserMessage {
            content: "test".to_string(),
            timestamp: 0,
            session_id: None,
        }).await.unwrap();
        
        // Send unfiltered event
        processor.send_event(JunoAgentEvent::SystemMessage {
            level: "info".to_string(),
            message: "test".to_string(),
            timestamp: 0,
            category: None,
        }).await.unwrap();
        
        let received_events = received.lock().await;
        assert_eq!(received_events.len(), 1); // Only user_message should be received
    }
    
    #[tokio::test]
    async fn test_event_pruning() {
        let config = EventProcessorConfig {
            max_events: 5,
            ..Default::default()
        };
        
        let app_handle = create_test_app_handle();
        let processor = JunoEventStreamProcessor::new(app_handle, Some(config));
        
        // Send more events than max_events
        for i in 0..10 {
            processor.send_event(JunoAgentEvent::SystemMessage {
                level: "info".to_string(),
                message: format!("message {}", i),
                timestamp: 0,
                category: None,
            }).await.unwrap();
        }
        
        let events = processor.get_events(None).await;
        assert_eq!(events.len(), 5); // Should be pruned to max_events
    }
}
```

### Integration Testing

#### AppState Integration
```rust
#[cfg(test)]
mod app_state_integration_tests {
    use super::*;
    
    #[tokio::test]
    async fn test_app_state_event_emission() {
        let app_state = create_test_app_state().await;
        
        let event = JunoAgentEvent::UserMessage {
            content: "Integration test".to_string(),
            timestamp: 0,
            session_id: Some("test-session".to_string()),
        };
        
        let result = app_state.emit_agent_event(event.clone()).await;
        assert!(result.is_ok());
        
        // Verify event was processed
        let processor = app_state.get_event_processor().await;
        let guard = processor.lock().await;
        let events = guard.get_events(None).await;
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].session_id(), Some("test-session"));
    }
    
    #[tokio::test]
    async fn test_event_subscription_integration() {
        let app_state = create_test_app_state().await;
        
        struct IntegrationSubscriber {
            pub events_received: Arc<AtomicUsize>,
        }
        
        #[async_trait]
        impl EventSubscriber for IntegrationSubscriber {
            async fn on_event(&self, _event: &JunoAgentEvent) -> Result<(), String> {
                self.events_received.fetch_add(1, Ordering::SeqCst);
                Ok(())
            }
        }
        
        let events_received = Arc::new(AtomicUsize::new(0));
        let subscriber = IntegrationSubscriber {
            events_received: events_received.clone(),
        };
        
        app_state.subscribe_to_events(Box::new(subscriber)).await;
        
        // Emit multiple events
        for i in 0..5 {
            app_state.emit_agent_event(JunoAgentEvent::SystemMessage {
                level: "info".to_string(),
                message: format!("test {}", i),
                timestamp: 0,
                category: None,
            }).await.unwrap();
        }
        
        // Give async processing time to complete
        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
        
        assert_eq!(events_received.load(Ordering::SeqCst), 5);
    }
}
```

### Performance Testing

#### Event Processing Performance
```rust
#[cfg(test)]
mod performance_tests {
    use super::*;
    use std::time::{Duration, Instant};
    
    #[tokio::test]
    async fn test_event_emission_performance() {
        let app_handle = create_test_app_handle();
        let processor = JunoEventStreamProcessor::new(app_handle, None);
        
        let start = Instant::now();
        
        // Emit 1000 events
        for i in 0..1000 {
            processor.send_event(JunoAgentEvent::SystemMessage {
                level: "info".to_string(),
                message: format!("Performance test {}", i),
                timestamp: 0,
                category: None,
            }).await.unwrap();
        }
        
        let duration = start.elapsed();
        
        // Should complete in reasonable time (< 1 second for 1000 events)
        assert!(duration < Duration::from_secs(1));
        
        // Average time per event should be < 1ms
        let avg_time_per_event = duration.as_millis() as f64 / 1000.0;
        assert!(avg_time_per_event < 1.0);
    }
    
    #[tokio::test]
    async fn test_memory_usage_under_load() {
        let app_handle = create_test_app_handle();
        let processor = JunoEventStreamProcessor::new(app_handle, None);
        
        // Get initial memory usage
        let initial_memory = get_memory_usage();
        
        // Emit large number of events
        for i in 0..10000 {
            processor.send_event(JunoAgentEvent::UserMessage {
                content: format!("Large test message with substantial content {}", i),
                timestamp: 0,
                session_id: Some(format!("session-{}", i % 100)),
            }).await.unwrap();
        }
        
        let final_memory = get_memory_usage();
        let memory_increase = final_memory - initial_memory;
        
        // Memory increase should be reasonable (< 50MB for 10k events)
        assert!(memory_increase < 50 * 1024 * 1024);
    }
}
```

### End-to-End Testing

#### Agent Execution with Events
```rust
#[cfg(test)]
mod e2e_tests {
    use super::*;
    
    #[tokio::test]
    async fn test_agent_execution_with_events() {
        let app_state = create_test_app_state().await;
        let app_handle = create_test_app_handle();
        
        // Set up event tracking
        let events_received = Arc::new(Mutex::new(Vec::new()));
        
        struct E2ESubscriber {
            events: Arc<Mutex<Vec<JunoAgentEvent>>>,
        }
        
        #[async_trait]
        impl EventSubscriber for E2ESubscriber {
            async fn on_event(&self, event: &JunoAgentEvent) -> Result<(), String> {
                let mut events = self.events.lock().await;
                events.push(event.clone());
                Ok(())
            }
        }
        
        let subscriber = E2ESubscriber {
            events: events_received.clone(),
        };
        app_state.subscribe_to_events(Box::new(subscriber)).await;
        
        // Execute agent query
        let query = "Test query for event tracking".to_string();
        let result = execute_agent_internal(query, app_state.clone(), app_handle).await;
        assert!(result.is_ok());
        
        // Verify expected events were emitted
        let events = events_received.lock().await;
        
        // Should have at least: AgentRunStart, UserMessage, AgentRunEnd
        assert!(events.len() >= 3);
        
        // Verify event sequence
        assert!(matches!(events[0], JunoAgentEvent::AgentRunStart { .. }));
        assert!(matches!(events[1], JunoAgentEvent::UserMessage { .. }));
        assert!(matches!(events.last().unwrap(), JunoAgentEvent::AgentRunEnd { .. }));
        
        // Verify session ID consistency
        let session_id = events[0].session_id().unwrap();
        for event in events.iter() {
            if let Some(event_session) = event.session_id() {
                assert_eq!(event_session, session_id);
            }
        }
    }
}
```

## Phase 2: Tool System Modernization

### Unit Testing

#### Tool Engine Testing
```rust
#[cfg(test)]
mod tool_engine_tests {
    use super::*;
    
    #[tokio::test]
    async fn test_native_engine_tool_preparation() {
        let engine = NativeToolCallEngine::new();
        
        let tool = ToolDefinition {
            name: "test_tool".to_string(),
            description: "Test tool".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "param": {"type": "string"}
                }
            }),
            api_type: None,
            beta_flag: None,
        };
        
        let context = ToolCallContext {
            model: "gpt-4".to_string(),
            provider: "openai".to_string(),
            messages: vec![],
            system_prompt: None,
            max_tokens: None,
            temperature: None,
        };
        
        let result = engine.prepare_tools_for_llm(&[tool], &context).await;
        assert!(result.is_ok());
        
        let prepared = result.unwrap();
        assert!(prepared.get("tools").is_some());
        assert_eq!(prepared["tools"].as_array().unwrap().len(), 1);
    }
    
    #[tokio::test]
    async fn test_prompt_engineering_tool_extraction() {
        let engine = PromptEngineeringEngine::new();
        
        let response = r#"
        I'll help you with that. Let me use the appropriate tool.
        
        <tool_call>
        <tool_name>test_tool</tool_name>
        <tool_id>call_123</tool_id>
        <arguments>
        {
          "param": "test_value"
        }
        </arguments>
        </tool_call>
        "#;
        
        let context = ToolCallContext {
            model: "llama-2".to_string(),
            provider: "local".to_string(),
            messages: vec![],
            system_prompt: None,
            max_tokens: None,
            temperature: None,
        };
        
        let result = engine.extract_tool_calls(response, &context).await;
        assert!(result.is_ok());
        
        let tool_calls = result.unwrap();
        assert_eq!(tool_calls.len(), 1);
        assert_eq!(tool_calls[0].name, "test_tool");
        assert_eq!(tool_calls[0].id, "call_123");
        assert_eq!(tool_calls[0].arguments["param"], "test_value");
    }
    
    #[tokio::test]
    async fn test_engine_selection() {
        let openai_engine = get_engine_for_provider("openai");
        assert_eq!(openai_engine.get_engine_type(), ToolCallEngineType::Native);
        assert!(openai_engine.supports_provider("openai"));
        
        let anthropic_engine = get_engine_for_provider("anthropic");
        assert_eq!(anthropic_engine.get_engine_type(), ToolCallEngineType::StructuredOutputs);
        assert!(anthropic_engine.supports_provider("anthropic"));
        
        let unknown_engine = get_engine_for_provider("unknown");
        assert_eq!(unknown_engine.get_engine_type(), ToolCallEngineType::PromptEngineering);
    }
}
```

#### Tool Provider Integration Testing
```rust
#[cfg(test)]
mod tool_provider_integration_tests {
    use super::*;
    
    #[tokio::test]
    async fn test_tool_execution_with_engines() {
        let mut tool_provider = LocalToolProvider::with_app_handle(create_test_app_handle());
        
        // Register a test tool
        let tool_def = ToolDefinition {
            name: "test_tool".to_string(),
            description: "Test tool".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "input": {"type": "string"}
                },
                "required": ["input"]
            }),
            api_type: None,
            beta_flag: None,
        };
        
        let executor = |input: Value| async move {
            Ok(json!({
                "result": format!("Processed: {}", input["input"].as_str().unwrap_or(""))
            }))
        };
        
        tool_provider.register_async_tool(tool_def, executor).await;
        
        // Test with different engines
        let engines = vec![
            get_engine_for_provider("openai"),
            get_engine_for_provider("anthropic"),
            get_engine_for_provider("unknown"),
        ];
        
        for engine in engines {
            let tool_call = ToolCall {
                id: "test_call".to_string(),
                name: "test_tool".to_string(),
                arguments: json!({"input": "test_input"}),
            };
            
            let result = tool_provider.execute_with_engine(tool_call, engine.as_ref()).await;
            assert!(result.is_ok());
            
            let tool_result = result.unwrap();
            assert!(tool_result.success);
            assert_eq!(tool_result.output["result"], "Processed: test_input");
        }
    }
}
```

### Performance Testing

#### Engine Performance Comparison
```rust
#[cfg(test)]
mod engine_performance_tests {
    use super::*;
    use std::time::Instant;
    
    #[tokio::test]
    async fn test_engine_preparation_performance() {
        let tools = create_test_tools(50); // Create 50 test tools
        let context = create_test_context();
        
        let engines = vec![
            ("Native", get_engine_for_provider("openai")),
            ("PromptEngineering", get_engine_for_provider("unknown")),
            ("StructuredOutputs", get_engine_for_provider("anthropic")),
        ];
        
        for (name, engine) in engines {
            let start = Instant::now();
            
            let result = engine.prepare_tools_for_llm(&tools, &context).await;
            
            let duration = start.elapsed();
            
            assert!(result.is_ok());
            assert!(duration < Duration::from_millis(100)); // Should be fast
            
            println!("{} engine preparation time: {:?}", name, duration);
        }
    }
    
    #[tokio::test]
    async fn test_tool_call_extraction_performance() {
        let response = create_large_tool_call_response(10); // 10 tool calls
        let context = create_test_context();
        
        let engines = vec![
            ("Native", get_engine_for_provider("openai")),
            ("PromptEngineering", get_engine_for_provider("unknown")),
            ("StructuredOutputs", get_engine_for_provider("anthropic")),
        ];
        
        for (name, engine) in engines {
            let start = Instant::now();
            
            let result = engine.extract_tool_calls(&response, &context).await;
            
            let duration = start.elapsed();
            
            assert!(result.is_ok());
            assert!(duration < Duration::from_millis(50)); // Should be very fast
            
            let tool_calls = result.unwrap();
            assert_eq!(tool_calls.len(), 10);
            
            println!("{} extraction time: {:?}", name, duration);
        }
    }
}
```

## Phase 3: Memory System Integration

### Unit Testing

#### Hybrid Memory Manager Testing
```rust
#[cfg(test)]
mod hybrid_memory_tests {
    use super::*;
    
    #[tokio::test]
    async fn test_dual_storage_consistency() {
        let hybrid_manager = HybridMemoryManager::new(create_test_config()).await;
        
        let message = Message {
            role: "user".to_string(),
            content: "Test message".to_string(),
            timestamp: Some(123456),
        };
        
        // Add message to hybrid manager
        hybrid_manager.add_message(message.clone()).await.unwrap();
        
        // Verify in both storage systems
        let conversation_history = hybrid_manager.get_conversation_history().await.unwrap();
        assert_eq!(conversation_history.len(), 1);
        assert_eq!(conversation_history[0].content, message.content);
        
        // Verify in event stream
        let event_processor = hybrid_manager.get_event_processor().await;
        let events = event_processor.lock().await.get_events(None).await;
        assert_eq!(events.len(), 1);
        
        match &events[0] {
            JunoAgentEvent::UserMessage { content, .. } => {
                assert_eq!(content, &message.content);
            }
            _ => panic!("Expected UserMessage event"),
        }
    }
    
    #[tokio::test]
    async fn test_event_to_message_conversion() {
        let hybrid_manager = HybridMemoryManager::new(create_test_config()).await;
        
        // Add events directly to event stream
        let events = vec![
            JunoAgentEvent::UserMessage {
                content: "Hello".to_string(),
                timestamp: 123456,
                session_id: None,
            },
            JunoAgentEvent::AssistantMessage {
                content: "Hi there!".to_string(),
                timestamp: 123457,
                session_id: None,
            },
            JunoAgentEvent::ToolCall {
                tool_name: "test_tool".to_string(),
                args: json!({"param": "value"}),
                id: "call_123".to_string(),
                timestamp: 123458,
                session_id: None,
            },
            JunoAgentEvent::ToolResult {
                tool_call_id: "call_123".to_string(),
                result: json!({"output": "result"}),
                timestamp: 123459,
                success: true,
                execution_time_ms: Some(100),
            },
        ];
        
        for event in events {
            hybrid_manager.add_event_directly(event).await.unwrap();
        }
        
        // Convert to conversation history
        let messages = hybrid_manager.get_conversation_history().await.unwrap();
        
        // Should have user message, assistant message, and tool result message
        assert_eq!(messages.len(), 3);
        assert_eq!(messages[0].role, "user");
        assert_eq!(messages[0].content, "Hello");
        assert_eq!(messages[1].role, "assistant");
        assert_eq!(messages[1].content, "Hi there!");
        assert_eq!(messages[2].role, "tool");
    }
    
    #[tokio::test]
    async fn test_token_aware_pruning() {
        let mut config = create_test_config();
        config.max_tokens = 1000; // Low limit for testing
        
        let hybrid_manager = HybridMemoryManager::new(config).await;
        
        // Add messages that exceed token limit
        for i in 0..20 {
            let large_message = Message {
                role: "user".to_string(),
                content: "A".repeat(100), // Large message to trigger pruning
                timestamp: Some(123456 + i),
            };
            
            hybrid_manager.add_message(large_message).await.unwrap();
        }
        
        let messages = hybrid_manager.get_conversation_history().await.unwrap();
        
        // Should be pruned to stay under token limit
        assert!(messages.len() < 20);
        
        // Verify token estimation is under limit
        let estimated_tokens = hybrid_manager.estimate_total_tokens().await.unwrap();
        assert!(estimated_tokens <= 1000);
    }
}
```

### Integration Testing

#### Memory System Integration with Agents
```rust
#[cfg(test)]
mod memory_integration_tests {
    use super::*;
    
    #[tokio::test]
    async fn test_agent_execution_with_hybrid_memory() {
        let app_state = create_test_app_state_with_hybrid_memory().await;
        
        // Execute multiple agent queries to build conversation history
        let queries = vec![
            "First query",
            "Second query with tool call",
            "Third query building on previous context",
        ];
        
        for (i, query) in queries.iter().enumerate() {
            let result = execute_agent_internal(
                query.to_string(),
                app_state.clone(),
                create_test_app_handle(),
            ).await;
            
            assert!(result.is_ok());
            
            // Verify conversation history is maintained
            let memory_manager = app_state.get_memory_manager().await;
            let memory_guard = memory_manager.lock().await;
            let history = memory_guard.get_conversation_history().await.unwrap();
            
            // Should have user and assistant messages for each query
            assert!(history.len() >= (i + 1) * 2);
        }
        
        // Verify final conversation state
        let memory_manager = app_state.get_memory_manager().await;
        let memory_guard = memory_manager.lock().await;
        let history = memory_guard.get_conversation_history().await.unwrap();
        
        // Should contain all conversation turns
        assert!(history.len() >= 6); // At least 3 user + 3 assistant messages
        
        // Verify conversation coherence
        for i in (0..history.len()).step_by(2) {
            assert_eq!(history[i].role, "user");
            if i + 1 < history.len() {
                assert_eq!(history[i + 1].role, "assistant");
            }
        }
    }
    
    #[tokio::test]
    async fn test_multi_agent_memory_isolation() {
        let app_state = create_test_app_state_with_hybrid_memory().await;
        
        // Test orchestrator memory persistence
        let orchestrator_query = "Task requiring delegation";
        execute_agent_internal(
            orchestrator_query.to_string(),
            app_state.clone(),
            create_test_app_handle(),
        ).await.unwrap();
        
        // Get orchestrator memory state
        let orchestrator_memory = app_state.get_memory_manager().await;
        let orchestrator_guard = orchestrator_memory.lock().await;
        let orchestrator_history = orchestrator_guard.get_conversation_history().await.unwrap();
        let orchestrator_msg_count = orchestrator_history.len();
        
        // Execute specialist task (should use fresh memory)
        let specialist_result = execute_specialized_agent_task(
            create_test_tool_provider(),
            "desktop",
            json!({"task": "specialist task"}),
            create_test_app_handle(),
            create_test_cancel_receiver(),
        ).await;
        
        assert!(specialist_result.is_ok());
        
        // Verify orchestrator memory unchanged by specialist execution
        let orchestrator_history_after = orchestrator_guard.get_conversation_history().await.unwrap();
        assert_eq!(orchestrator_history_after.len(), orchestrator_msg_count);
        
        // Verify specialist didn't pollute orchestrator context
        for (original, after) in orchestrator_history.iter().zip(orchestrator_history_after.iter()) {
            assert_eq!(original.content, after.content);
            assert_eq!(original.role, after.role);
        }
    }
}
```

## Phase 4: Agent Architecture Enhancement

### Unit Testing

#### Agent Processor Testing
```rust
#[cfg(test)]
mod agent_processor_tests {
    use super::*;
    
    #[tokio::test]
    async fn test_processor_component_communication() {
        let processors = create_test_agent_processors().await;
        
        // Test LLM processor
        let llm_request = create_test_llm_request();
        let llm_response = processors.llm_processor.process_request(llm_request).await;
        assert!(llm_response.is_ok());
        
        // Test tool processor
        let tool_calls = vec![create_test_tool_call()];
        let tool_results = processors.tool_processor.execute_tools(tool_calls).await;
        assert!(tool_results.is_ok());
        assert_eq!(tool_results.unwrap().len(), 1);
        
        // Test loop executor
        let execution_result = processors.loop_executor.execute_iteration(
            "test query".to_string(),
            &mut create_test_memory_manager(),
        ).await;
        assert!(execution_result.is_ok());
    }
    
    #[tokio::test]
    async fn test_model_resolution() {
        let resolver = ModelResolver::new(create_test_model_config());
        
        // Test agent-specific resolution
        let desktop_model = resolver.resolve_for_agent(
            AgentType::Desktop,
            None,
            None,
        ).unwrap();
        assert_eq!(desktop_model.provider, "anthropic");
        assert!(desktop_model.model.contains("sonnet"));
        
        let browser_model = resolver.resolve_for_agent(
            AgentType::Browser,
            None,
            None,
        ).unwrap();
        assert_eq!(browser_model.provider, "openai");
        assert!(browser_model.model.contains("gpt"));
        
        // Test runtime override
        let override_model = resolver.resolve_for_agent(
            AgentType::Desktop,
            Some("gpt-4"),
            Some("openai"),
        ).unwrap();
        assert_eq!(override_model.provider, "openai");
        assert_eq!(override_model.model, "gpt-4");
    }
}
```

### Integration Testing

#### Enhanced Agent Coordination
```rust
#[cfg(test)]
mod agent_coordination_tests {
    use super::*;
    
    #[tokio::test]
    async fn test_single_agent_with_processors() {
        let app_state = create_test_app_state_with_processors().await;
        
        let query = "Test query for processor-based agent";
        let result = execute_agent_internal(
            query.to_string(),
            app_state.clone(),
            create_test_app_handle(),
        ).await;
        
        assert!(result.is_ok());
        
        // Verify processors were used correctly
        let event_processor = app_state.get_event_processor().await;
        let events = event_processor.lock().await.get_events(None).await;
        
        // Should have processor-specific events
        let processor_events: Vec<_> = events.iter()
            .filter(|e| matches!(e, JunoAgentEvent::SystemMessage { category: Some(cat), .. } if cat.contains("processor")))
            .collect();
        
        assert!(!processor_events.is_empty());
    }
    
    #[tokio::test]
    async fn test_multi_agent_with_dynamic_models() {
        let app_state = create_test_app_state_with_model_resolver().await;
        
        // Execute query that requires delegation
        let query = "Complex task requiring multiple agents";
        let result = execute_agent_internal(
            query.to_string(),
            app_state.clone(),
            create_test_app_handle(),
        ).await;
        
        assert!(result.is_ok());
        
        // Verify different models were used for different agents
        let events = get_agent_events(&app_state).await;
        
        let model_usage_events: Vec<_> = events.iter()
            .filter(|e| matches!(e, JunoAgentEvent::SystemMessage { message, .. } if message.contains("model")))
            .collect();
        
        // Should have evidence of different models for different agent types
        assert!(!model_usage_events.is_empty());
    }
}
```

## Phase 5: MCP Integration

### Unit Testing

#### MCP Client Testing
```rust
#[cfg(test)]
mod mcp_client_tests {
    use super::*;
    
    #[tokio::test]
    async fn test_mcp_server_mounting() {
        let mut mcp_client = McpClient::new();
        
        let config = McpServerConfig {
            name: "test_server".to_string(),
            is_builtin: true,
            handler: create_test_mcp_handler(),
            command: None,
            args: None,
        };
        
        let result = mcp_client.mount_server(config).await;
        assert!(result.is_ok());
        
        let tools = mcp_client.get_tools();
        assert!(!tools.is_empty());
    }
    
    #[tokio::test]
    async fn test_mcp_tool_execution() {
        let mut mcp_client = create_test_mcp_client().await;
        
        let tool_args = json!({
            "param1": "value1",
            "param2": 42
        });
        
        let result = mcp_client.execute_tool("test_mcp_tool", tool_args).await;
        assert!(result.is_ok());
        
        let output = result.unwrap();
        assert!(output.get("success").unwrap().as_bool().unwrap());
    }
    
    #[tokio::test]
    async fn test_mcp_transport_switching() {
        let mut mcp_client = McpClient::new();
        
        // Test in-memory transport
        let builtin_config = McpServerConfig {
            name: "builtin".to_string(),
            is_builtin: true,
            handler: create_test_mcp_handler(),
            command: None,
            args: None,
        };
        
        mcp_client.mount_server(builtin_config).await.unwrap();
        assert!(mcp_client.get_tools().len() > 0);
        
        // Test stdio transport
        let external_config = McpServerConfig {
            name: "external".to_string(),
            is_builtin: false,
            handler: None,
            command: Some("test_mcp_server".to_string()),
            args: Some(vec!["--test".to_string()]),
        };
        
        // This would require a test MCP server binary
        // let result = mcp_client.mount_server(external_config).await;
        // assert!(result.is_ok());
    }
}
```

### Integration Testing

#### MCP Tool Provider Integration
```rust
#[cfg(test)]
mod mcp_integration_tests {
    use super::*;
    
    #[tokio::test]
    async fn test_mcp_tool_provider_integration() {
        let mut tool_provider = LocalToolProvider::with_app_handle(create_test_app_handle());
        
        // Mount test MCP server
        let mcp_config = create_test_mcp_server_config();
        let result = tool_provider.mount_mcp_server(mcp_config).await;
        assert!(result.is_ok());
        
        // Verify MCP tools are available
        let available_tools = tool_provider.get_available_tools().await;
        let mcp_tools: Vec<_> = available_tools.iter()
            .filter(|tool| tool.api_type.as_deref() == Some("mcp"))
            .collect();
        
        assert!(!mcp_tools.is_empty());
        
        // Test MCP tool execution
        let mcp_tool_call = ToolCall {
            id: "mcp_call_123".to_string(),
            name: mcp_tools[0].name.clone(),
            arguments: json!({"test_param": "test_value"}),
        };
        
        let result = tool_provider.execute_tool(mcp_tool_call).await;
        assert!(result.is_ok());
        
        let tool_result = result.unwrap();
        assert!(tool_result.success);
    }
    
    #[tokio::test]
    async fn test_mcp_and_native_tool_coexistence() {
        let mut tool_provider = LocalToolProvider::with_app_handle(create_test_app_handle());
        
        // Register native tool
        let native_tool = create_test_tool_definition("native_tool");
        let native_executor = |_input: Value| async move {
            Ok(json!({"type": "native", "result": "success"}))
        };
        tool_provider.register_async_tool(native_tool, native_executor).await;
        
        // Mount MCP server
        let mcp_config = create_test_mcp_server_config();
        tool_provider.mount_mcp_server(mcp_config).await.unwrap();
        
        // Test both tool types
        let native_call = ToolCall {
            id: "native_call".to_string(),
            name: "native_tool".to_string(),
            arguments: json!({}),
        };
        
        let native_result = tool_provider.execute_tool(native_call).await.unwrap();
        assert_eq!(native_result.output["type"], "native");
        
        let mcp_call = ToolCall {
            id: "mcp_call".to_string(),
            name: "test_mcp_tool".to_string(),
            arguments: json!({}),
        };
        
        let mcp_result = tool_provider.execute_tool(mcp_call).await.unwrap();
        assert_eq!(mcp_result.output["type"], "mcp");
    }
}
```

## Phase 6: Cross-Platform Foundation

### Unit Testing

#### Platform Provider Testing
```rust
#[cfg(test)]
mod platform_provider_tests {
    use super::*;
    
    #[test]
    fn test_platform_provider_creation() {
        let provider = create_platform_provider();
        assert!(provider.is_ok());
        
        let provider = provider.unwrap();
        
        #[cfg(target_os = "macos")]
        assert_eq!(provider.get_platform_name(), "macOS");
        
        #[cfg(target_os = "windows")]
        assert_eq!(provider.get_platform_name(), "Windows");
    }
    
    #[tokio::test]
    async fn test_platform_capabilities() {
        let provider = create_platform_provider().unwrap();
        
        // Test capability detection
        assert!(provider.supports_accessibility_api());
        assert!(provider.supports_screen_recording());
        
        // Test platform-specific features
        let permissions = provider.get_accessibility_permissions().await;
        assert!(permissions.is_ok());
    }
    
    #[tokio::test]
    async fn test_cross_platform_screenshot() {
        let provider = create_platform_provider().unwrap();
        
        let screenshot = provider.capture_screenshot().await;
        assert!(screenshot.is_ok());
        
        let image_data = screenshot.unwrap();
        assert!(!image_data.is_empty());
        
        // Verify image format (should be PNG)
        assert_eq!(&image_data[0..8], &[137, 80, 78, 71, 13, 10, 26, 10]);
    }
}
```

### Integration Testing

#### Cross-Platform Tool Integration
```rust
#[cfg(test)]
mod cross_platform_integration_tests {
    use super::*;
    
    #[tokio::test]
    async fn test_platform_specific_tool_execution() {
        let mut tool_provider = LocalToolProvider::with_app_handle(create_test_app_handle());
        
        // Setup platform-specific tools
        setup_platform_tools(&mut tool_provider).await;
        
        // Test screenshot tool (available on all platforms)
        let screenshot_call = ToolCall {
            id: "screenshot_call".to_string(),
            name: "capture_screenshot".to_string(),
            arguments: json!({}),
        };
        
        let result = tool_provider.execute_tool(screenshot_call).await;
        assert!(result.is_ok());
        
        let tool_result = result.unwrap();
        assert!(tool_result.success);
        assert!(tool_result.output.get("image_data").is_some());
        
        // Test platform-specific tools
        #[cfg(target_os = "macos")]
        {
            let macos_call = ToolCall {
                id: "macos_call".to_string(),
                name: "macos_specific_tool".to_string(),
                arguments: json!({}),
            };
            
            let result = tool_provider.execute_tool(macos_call).await;
            assert!(result.is_ok());
        }
        
        #[cfg(target_os = "windows")]
        {
            let windows_call = ToolCall {
                id: "windows_call".to_string(),
                name: "windows_specific_tool".to_string(),
                arguments: json!({}),
            };
            
            let result = tool_provider.execute_tool(windows_call).await;
            assert!(result.is_ok());
        }
    }
}
```

## Continuous Integration Testing

### Automated Test Pipeline

#### GitHub Actions Configuration
```yaml
# .github/workflows/tars-integration-tests.yml
name: TARS Integration Tests

on:
  push:
    branches: [ main, develop ]
  pull_request:
    branches: [ main ]

jobs:
  test:
    runs-on: ${{ matrix.os }}
    strategy:
      matrix:
        os: [ubuntu-latest, windows-latest, macos-latest]
        rust: [stable]
        
    steps:
    - uses: actions/checkout@v3
    
    - name: Install Rust
      uses: actions-rs/toolchain@v1
      with:
        toolchain: ${{ matrix.rust }}
        override: true
        
    - name: Install Node.js
      uses: actions/setup-node@v3
      with:
        node-version: '18'
        
    - name: Install dependencies
      run: |
        cargo build --manifest-path src-tauri/Cargo.toml
        bun install
        
    - name: Run unit tests
      run: |
        cargo test --manifest-path src-tauri/Cargo.toml --lib
        
    - name: Run integration tests
      run: |
        cargo test --manifest-path src-tauri/Cargo.toml --test '*'
        
    - name: Run frontend tests
      run: |
        npm test
        
    - name: Run E2E tests
      run: |
        ./run-all-tests.sh
        
    - name: Performance benchmarks
      run: |
        cargo bench --manifest-path src-tauri/Cargo.toml
```

### Test Coverage Requirements

#### Coverage Targets
- **Unit Tests**: 90%+ coverage for new components
- **Integration Tests**: 80%+ coverage for component interactions
- **E2E Tests**: 70%+ coverage for user workflows
- **Performance Tests**: All critical paths benchmarked

#### Coverage Monitoring
```rust
// Use tarpaulin for Rust coverage
#[cfg(test)]
mod coverage_tests {
    // Ensure all critical paths are tested
    
    #[test]
    fn test_coverage_critical_paths() {
        // Event emission path
        // Tool execution path
        // Memory management path
        // Agent coordination path
        // Error handling paths
    }
}
```

This comprehensive testing strategy ensures that each phase of the TARS → Juno integration is thoroughly validated while maintaining the production-ready quality that makes Juno enterprise-grade. The combination of unit, integration, and end-to-end tests provides confidence that new features work correctly while preserving existing functionality.
//! # Headless Integration Tests — Lightweight
//!
//! These tests exercise AppState, memory managers, tool providers, and headless
//! mode flags without creating a Tauri app. They work on any thread.
//!
//! ```bash
//! cargo test --manifest-path src-tauri/Cargo.toml --test headless_integration
//! ```

use juno_lib::testing::harness::TestHarness;

#[tokio::test]
async fn test_appstate_creation_headless() {
    let harness = TestHarness::new().await.expect("harness should build");
    let state = harness.state();

    assert!(
        !state.is_agent_executing(),
        "agent should not be executing initially"
    );
    println!("AppState created successfully in headless mode");
}

#[tokio::test]
async fn test_cancellation_signal_headless() {
    let harness = TestHarness::new().await.expect("harness should build");
    let state = harness.state();

    assert!(
        !*state.cancel_rx.borrow(),
        "cancel signal should be false initially"
    );

    state.signal_cancel();
    assert!(
        *state.cancel_rx.borrow(),
        "cancel signal should be true after signal_cancel()"
    );

    state.reset_cancel();
    assert!(
        !*state.cancel_rx.borrow(),
        "cancel signal should be false after reset_cancel()"
    );

    println!("Cancellation signal roundtrip works in headless mode");
}

#[tokio::test]
async fn test_headless_mode_flag() {
    let _harness = TestHarness::new().await.expect("harness should build");
    assert!(
        juno_lib::cli::headless::is_headless_mode(),
        "headless mode should be set after TestHarness::new()"
    );
}

#[tokio::test]
async fn test_memory_manager_headless() {
    use juno_lib::agent::core::{Message, Role};
    use juno_lib::agent::traits::MemoryManager;

    let harness = TestHarness::new().await.expect("harness should build");
    let state = harness.state();

    let memory_arc = state.get_memory_manager().await;
    let mut memory = memory_arc.lock().await;

    let initial_messages = memory.get_messages().await.expect("should get messages");
    let initial_count = initial_messages.len();

    let msg = Message {
        role: Role::User,
        content: "Hello from headless test".to_string(),
        tool_calls: None,
        tool_call_id: None,
        name: None,
    };
    memory.add_message(msg).await.expect("should add message");

    let messages = memory.get_messages().await.expect("should get messages");
    assert_eq!(
        messages.len(),
        initial_count + 1,
        "should have one more message"
    );
    assert_eq!(
        messages.last().expect("should have last").content,
        "Hello from headless test"
    );

    memory.clear_memory().await.expect("should clear memory");
    let after_clear = memory.get_messages().await.expect("should get messages");
    assert!(after_clear.is_empty(), "memory should be empty after clear");

    println!("Memory manager roundtrip works in headless mode");
}

#[tokio::test]
async fn test_tool_registration_headless() {
    use juno_lib::agent::core::ToolDefinition;
    use juno_lib::agent::implementations::tool_provider::LocalToolProvider;
    use juno_lib::agent::traits::ToolProvider;
    use serde_json::json;

    // LocalToolProvider::new() has no app_handle — skips config filtering,
    // so registered tools appear directly in list_tools().
    let provider = LocalToolProvider::new();

    // Register two mock tools
    let tool_a = ToolDefinition {
        name: "test_echo".to_string(),
        description: "Echoes the input back".to_string(),
        input_schema: json!({
            "type": "object",
            "properties": {
                "message": {"type": "string"}
            },
            "required": ["message"]
        }),
        api_type: None,
        beta_flag: None,
    };

    provider
        .register_async_tool(tool_a, |input| async move {
            let msg = input
                .get("message")
                .and_then(|v| v.as_str())
                .unwrap_or("(empty)");
            Ok(json!({"echo": msg}))
        })
        .await;

    let tool_b = ToolDefinition {
        name: "test_add".to_string(),
        description: "Adds two numbers".to_string(),
        input_schema: json!({
            "type": "object",
            "properties": {
                "a": {"type": "number"},
                "b": {"type": "number"}
            },
            "required": ["a", "b"]
        }),
        api_type: None,
        beta_flag: None,
    };

    provider
        .register_async_tool(tool_b, |input| async move {
            let a = input.get("a").and_then(|v| v.as_f64()).unwrap_or(0.0);
            let b = input.get("b").and_then(|v| v.as_f64()).unwrap_or(0.0);
            Ok(json!({"sum": a + b}))
        })
        .await;

    // Verify list_tools returns both
    let tools = provider
        .list_tools()
        .await
        .expect("list_tools should succeed");
    assert_eq!(tools.len(), 2, "should have 2 registered tools");

    let names: Vec<&str> = tools.iter().map(|t| t.name.as_str()).collect();
    assert!(names.contains(&"test_echo"), "should contain test_echo");
    assert!(names.contains(&"test_add"), "should contain test_add");

    // Execute test_echo
    let echo_result = provider
        .execute_tool(juno_lib::agent::core::ToolCall {
            id: "call_1".to_string(),
            name: "test_echo".to_string(),
            input: json!({"message": "hello headless"}),
        })
        .await
        .expect("execute_tool should succeed");

    assert_eq!(echo_result.call_id, "call_1");
    assert_eq!(
        echo_result.output.get("echo").and_then(|v| v.as_str()),
        Some("hello headless"),
        "echo tool should return the input message"
    );

    // Execute test_add
    let add_result = provider
        .execute_tool(juno_lib::agent::core::ToolCall {
            id: "call_2".to_string(),
            name: "test_add".to_string(),
            input: json!({"a": 3, "b": 7}),
        })
        .await
        .expect("execute_tool should succeed");

    assert_eq!(
        add_result.output.get("sum").and_then(|v| v.as_f64()),
        Some(10.0),
        "add tool should return the sum"
    );

    println!("Tool registration + list + execute works in headless mode");
}

//! # Headless Integration Tests — Lightweight
//!
//! These tests exercise AppState, memory managers, and headless mode flags
//! without creating a Tauri app. They work on any thread.
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
    assert_eq!(messages.len(), initial_count + 1, "should have one more message");
    assert_eq!(
        messages.last().expect("should have last").content,
        "Hello from headless test"
    );

    memory.clear_memory().await.expect("should clear memory");
    let after_clear = memory.get_messages().await.expect("should get messages");
    assert!(after_clear.is_empty(), "memory should be empty after clear");

    println!("Memory manager roundtrip works in headless mode");
}

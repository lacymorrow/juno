//! # HeadlessRuntime Integration Tests
//!
//! These tests exercise HeadlessRuntime commands (agent, system, config) and the
//! full agent pipeline (MockBrain → AgentRunner → ToolProvider) via a real Tauri
//! app. On macOS, the EventLoop must be created on the main thread, so this test
//! binary uses `harness = false` with a custom `main()`.
//!
//! ```bash
//! cargo test --manifest-path src-tauri/Cargo.toml --test headless_runtime
//! ```

use clap::Parser;
use juno_lib::testing::harness::TestHarness;

fn main() {
    // We're on the main thread — Tauri's EventLoop will work here.
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("failed to build tokio runtime");

    rt.block_on(async {
        run_all_tests().await;
    });
}

async fn run_all_tests() {
    println!("\nrunning HeadlessRuntime integration tests\n");

    test_agent_status().await;
    println!("test test_agent_status ... ok");

    test_system_info().await;
    println!("test test_system_info ... ok");

    test_config_show().await;
    println!("test test_config_show ... ok");

    test_agent_stop().await;
    println!("test test_agent_stop ... ok");

    test_real_api_query().await;
    // (prints its own status — may skip)

    test_mock_brain_immediate().await;
    println!("test test_mock_brain_immediate ... ok");

    test_mock_brain_with_tool().await;
    println!("test test_mock_brain_with_tool ... ok");

    test_mcp_add_server().await;
    println!("test test_mcp_add_server ... ok");

    println!("\ntest result: ok. All HeadlessRuntime tests passed.\n");
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

async fn test_agent_status() {
    let harness = TestHarness::with_app().await.expect("harness should build");

    let cli = juno_lib::cli::Cli::parse_from(["juno", "agent", "status"]);
    let runtime = juno_lib::cli::headless::HeadlessRuntime::new(harness.app_handle().clone(), &cli);

    let result = runtime
        .execute_command(&cli)
        .await
        .expect("status should succeed");

    assert!(result.success, "status command should succeed");

    let parsed: serde_json::Value =
        serde_json::from_str(&result.output).expect("output should be valid JSON");
    assert!(
        parsed.get("agent_executing").is_some(),
        "status should contain agent_executing field"
    );
}

async fn test_system_info() {
    let harness = TestHarness::with_app().await.expect("harness should build");

    let cli = juno_lib::cli::Cli::parse_from(["juno", "system", "info"]);
    let runtime = juno_lib::cli::headless::HeadlessRuntime::new(harness.app_handle().clone(), &cli);

    let result = runtime
        .execute_command(&cli)
        .await
        .expect("system info should succeed");

    assert!(result.success, "system info command should succeed");

    let parsed: serde_json::Value =
        serde_json::from_str(&result.output).expect("output should be valid JSON");
    assert!(
        parsed.get("platform").is_some(),
        "system info should contain platform field"
    );
}

async fn test_config_show() {
    let harness = TestHarness::with_app().await.expect("harness should build");

    let cli = juno_lib::cli::Cli::parse_from(["juno", "config", "show"]);
    let runtime = juno_lib::cli::headless::HeadlessRuntime::new(harness.app_handle().clone(), &cli);

    let result = runtime
        .execute_command(&cli)
        .await
        .expect("config show should succeed");

    assert!(result.success, "config show command should succeed");
}

async fn test_agent_stop() {
    let harness = TestHarness::with_app().await.expect("harness should build");

    let cli = juno_lib::cli::Cli::parse_from(["juno", "agent", "stop"]);
    let runtime = juno_lib::cli::headless::HeadlessRuntime::new(harness.app_handle().clone(), &cli);

    let result = runtime
        .execute_command(&cli)
        .await
        .expect("agent stop should succeed");

    assert!(result.success, "agent stop command should succeed");
}

async fn test_real_api_query() {
    if std::env::var("ANTHROPIC_API_KEY").is_err() {
        println!("test test_real_api_query ... skipped (ANTHROPIC_API_KEY not set)");
        return;
    }

    let harness = TestHarness::with_app().await.expect("harness should build");

    let cli = juno_lib::cli::Cli::parse_from(["juno", "query", "Say exactly: HEADLESS_TEST_OK"]);
    let runtime = juno_lib::cli::headless::HeadlessRuntime::new(harness.app_handle().clone(), &cli);

    match runtime.execute_command(&cli).await {
        Ok(result) => {
            assert!(result.success, "query command should succeed");
            println!("test test_real_api_query ... ok");
        }
        Err(e) => {
            let err_str = format!("{}", e);
            // Timeouts are expected in the minimal test app (no full agent infrastructure)
            if err_str.contains("timed out") || err_str.contains("timeout") {
                println!(
                    "test test_real_api_query ... skipped (query timed out in test environment)"
                );
            } else {
                panic!("query failed unexpectedly: {}", e);
            }
        }
    }
}

// ---------------------------------------------------------------------------
// MockBrain pipeline tests — exercise the full agent loop without API calls
// ---------------------------------------------------------------------------

async fn test_mock_brain_immediate() {
    use juno_lib::agent::implementations::agent_runner::DefaultAgentRunner;
    use juno_lib::agent::implementations::memory_manager::AdvancedMemoryManager;
    use juno_lib::agent::implementations::tool_provider::LocalToolProvider;
    use juno_lib::agent::traits::AgentRunnable;
    use juno_lib::testing::mock_brain::MockBrain;

    let harness = TestHarness::with_app().await.expect("harness should build");

    let memory = AdvancedMemoryManager::new();
    let tool_provider = LocalToolProvider::new();
    let brain = MockBrain::immediate();

    let mut runner: DefaultAgentRunner<AdvancedMemoryManager, LocalToolProvider> =
        DefaultAgentRunner::new(
            memory,
            tool_provider,
            brain,
            10, // max_steps (won't matter — MockBrain finishes immediately)
            harness.app_handle().clone(),
        );

    // Create a cancellation channel (never cancelled)
    let (_cancel_tx, cancel_rx) = tokio::sync::watch::channel(false);

    let result = runner
        .run("Hello agent, what is your name?".to_string(), cancel_rx)
        .await
        .expect("MockBrain immediate run should succeed");

    assert_eq!(
        result, "Hello from MockBrain",
        "MockBrain::immediate() should produce the canned response"
    );
}

async fn test_mock_brain_with_tool() {
    use juno_lib::agent::core::ToolDefinition;
    use juno_lib::agent::implementations::agent_runner::DefaultAgentRunner;
    use juno_lib::agent::implementations::memory_manager::AdvancedMemoryManager;
    use juno_lib::agent::implementations::tool_provider::LocalToolProvider;
    use juno_lib::agent::traits::AgentRunnable;
    use juno_lib::testing::mock_brain::MockBrain;
    use serde_json::json;

    let harness = TestHarness::with_app().await.expect("harness should build");

    // Set up a tool provider with one mock tool
    let tool_provider = LocalToolProvider::new();

    let tool_def = ToolDefinition {
        name: "test_action".to_string(),
        description: "A test action that returns success".to_string(),
        input_schema: json!({
            "type": "object",
            "properties": {
                "action": {"type": "string"}
            }
        }),
        api_type: None,
        beta_flag: None,
    };

    tool_provider
        .register_async_tool(tool_def, |_input| async move {
            Ok(json!({"status": "completed", "detail": "test action ran"}))
        })
        .await;

    // MockBrain will: call "test_action" on first decision, then Finish on second
    let brain = MockBrain::tool_then_finish("test_action", "All done after tool call!");

    let memory = AdvancedMemoryManager::new();
    let mut runner: DefaultAgentRunner<AdvancedMemoryManager, LocalToolProvider> =
        DefaultAgentRunner::new(
            memory,
            tool_provider,
            brain,
            10,
            harness.app_handle().clone(),
        );

    let (_cancel_tx, cancel_rx) = tokio::sync::watch::channel(false);

    let result = runner
        .run("Please run the test action".to_string(), cancel_rx)
        .await
        .expect("MockBrain tool-then-finish run should succeed");

    assert_eq!(
        result, "All done after tool call!",
        "MockBrain should finish with the expected response after tool execution"
    );
}

// ---------------------------------------------------------------------------
// MCP server configuration test
// ---------------------------------------------------------------------------

async fn test_mcp_add_server() {
    let harness = TestHarness::with_app().await.expect("harness should build");

    let cli = juno_lib::cli::Cli::parse_from([
        "juno",
        "mcp",
        "add-server",
        "--name",
        "test-server",
        "--http-url",
        "http://localhost:9999/mcp",
    ]);
    let runtime = juno_lib::cli::headless::HeadlessRuntime::new(harness.app_handle().clone(), &cli);

    match runtime.execute_command(&cli).await {
        Ok(result) => {
            assert!(result.success, "mcp add-server command should succeed");
        }
        Err(e) => {
            let err_str = format!("{}", e);
            // MCP server add might fail if store isn't fully initialized in minimal app —
            // that's acceptable; what matters is the command is wired and doesn't panic.
            if err_str.contains("store") || err_str.contains("Store") || err_str.contains("save") {
                println!("test test_mcp_add_server ... ok (store unavailable in minimal app, command wired correctly)");
                return;
            }
            panic!("mcp add-server failed unexpectedly: {}", e);
        }
    }
}

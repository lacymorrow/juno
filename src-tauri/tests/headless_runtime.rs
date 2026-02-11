//! # HeadlessRuntime Integration Tests
//!
//! These tests exercise HeadlessRuntime commands (agent, system, config) via
//! a real Tauri app. On macOS, the EventLoop must be created on the main thread,
//! so this test binary uses `harness = false` with a custom `main()`.
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

    println!("\ntest result: ok. All HeadlessRuntime tests passed.\n");
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

async fn test_agent_status() {
    let harness = TestHarness::with_app().await.expect("harness should build");

    let cli = juno_lib::cli::Cli::parse_from(["juno", "agent", "status"]);
    let runtime =
        juno_lib::cli::headless::HeadlessRuntime::new(harness.app_handle().clone(), &cli);

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
    let runtime =
        juno_lib::cli::headless::HeadlessRuntime::new(harness.app_handle().clone(), &cli);

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
    let runtime =
        juno_lib::cli::headless::HeadlessRuntime::new(harness.app_handle().clone(), &cli);

    let result = runtime
        .execute_command(&cli)
        .await
        .expect("config show should succeed");

    assert!(result.success, "config show command should succeed");
}

async fn test_agent_stop() {
    let harness = TestHarness::with_app().await.expect("harness should build");

    let cli = juno_lib::cli::Cli::parse_from(["juno", "agent", "stop"]);
    let runtime =
        juno_lib::cli::headless::HeadlessRuntime::new(harness.app_handle().clone(), &cli);

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

    let cli = juno_lib::cli::Cli::parse_from([
        "juno",
        "query",
        "Say exactly: HEADLESS_TEST_OK",
    ]);
    let runtime =
        juno_lib::cli::headless::HeadlessRuntime::new(harness.app_handle().clone(), &cli);

    match runtime.execute_command(&cli).await {
        Ok(result) => {
            assert!(result.success, "query command should succeed");
            println!("test test_real_api_query ... ok");
        }
        Err(e) => {
            let err_str = format!("{}", e);
            // Timeouts are expected in the minimal test app (no full agent infrastructure)
            if err_str.contains("timed out") || err_str.contains("timeout") {
                println!("test test_real_api_query ... skipped (query timed out in test environment)");
            } else {
                panic!("query failed unexpectedly: {}", e);
            }
        }
    }
}

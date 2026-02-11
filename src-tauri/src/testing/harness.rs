//! # Test Harness
//!
//! Provides two test harness tiers:
//!
//! - **`TestHarness::new()`** — Lightweight. Creates `AppState` directly without
//!   a Tauri app. Works on any thread, no macOS main-thread restriction.
//!   Use for unit-style tests that only need AppState.
//!
//! - **`TestHarness::with_app()`** — Full integration. Creates a real Tauri app
//!   with a `Box::leak`-ed `AppHandle`. On macOS, Tauri's EventLoop must be
//!   created on the main thread, so **run with `--test-threads=1`**:
//!   ```bash
//!   cargo test --test headless_integration -- --test-threads=1
//!   ```
//!
//! ## Usage
//!
//! ```ignore
//! // Lightweight (no Tauri app needed):
//! let harness = TestHarness::new().await.expect("harness should build");
//! let state = harness.state();
//! assert!(!state.is_agent_executing());
//!
//! // Full integration (needs --test-threads=1 on macOS):
//! let harness = TestHarness::with_app().await.expect("harness should build");
//! let app_handle = harness.app_handle();
//! ```

use crate::cli::headless;
use crate::state::AppState;
use crate::startup;
use std::sync::OnceLock;
use tauri::AppHandle;
use tracing::info;

/// Global singleton for the shared Tauri app handle.
/// Only one Tauri app can be created per process (EventLoop restriction).
static SHARED_APP_HANDLE: OnceLock<AppHandle> = OnceLock::new();

/// A self-contained test environment.
///
/// Two modes:
/// - Lightweight (`new`): just `AppState`, no Tauri app, works on any thread
/// - Full (`with_app`): real Tauri app with `AppHandle`, needs main thread on macOS
pub struct TestHarness {
    state: AppState,
    app_handle: Option<AppHandle>,
}

impl TestHarness {
    /// Build a lightweight harness with just `AppState`.
    ///
    /// No Tauri app is created — works on any thread, no macOS main-thread
    /// restriction. Use this for tests that only need `AppState`.
    pub async fn new() -> Result<Self, String> {
        headless::set_headless_mode(true);
        startup::init_environment();
        let state = startup::init_app_state(None);
        Ok(Self { state, app_handle: None })
    }

    /// Build a full harness with a real Tauri app.
    ///
    /// On macOS, the EventLoop must be created on the main thread.
    /// Run integration tests with `--test-threads=1` to ensure this.
    ///
    /// A singleton Tauri app is created on first call and reused by
    /// subsequent calls (only one EventLoop per process).
    pub async fn with_app() -> Result<Self, String> {
        headless::set_headless_mode(true);
        startup::init_environment();
        let state = startup::init_app_state(None);

        let handle = if let Some(h) = SHARED_APP_HANDLE.get() {
            h.clone()
        } else {
            let app = tauri::Builder::default()
                .manage(startup::init_app_state(None))
                .plugin(tauri_plugin_store::Builder::default().build())
                .plugin(tauri_plugin_process::init())
                .setup(|_app| {
                    info!("Test harness Tauri app setup completed");
                    Ok(())
                })
                .build(crate::get_tauri_context())
                .map_err(|e| format!("Failed to build test Tauri app: {}", e))?;

            let app_ref: &'static mut tauri::App = Box::leak(Box::new(app));
            let h = app_ref.handle().clone();
            let _ = SHARED_APP_HANDLE.set(h.clone());
            h
        };

        Ok(Self { state, app_handle: Some(handle) })
    }

    /// Get a reference to the `AppState`.
    pub fn state(&self) -> &AppState {
        &self.state
    }

    /// Get a reference to the `AppHandle`.
    ///
    /// # Panics
    ///
    /// Panics if called on a lightweight harness (use `with_app()` instead).
    pub fn app_handle(&self) -> &AppHandle {
        self.app_handle
            .as_ref()
            .unwrap_or_else(|| panic!("app_handle() called on lightweight harness — use TestHarness::with_app()"))
    }
}

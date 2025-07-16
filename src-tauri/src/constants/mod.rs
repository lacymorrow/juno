//! # Constants Module
//!
//! This module provides centralized constants for all application modules.
//! Constants are organized into logical groups for better maintainability.

// Re-export all constant modules that actually exist
pub mod agent;
pub mod api;
pub mod app;
pub mod audio;
pub mod browser;
pub mod cli;
pub mod commands;
pub mod error_messages;
pub mod errors;
pub mod events;
pub mod files;
pub mod memory;
pub mod menus;
pub mod mouse;
pub mod performance;
pub mod permissions;
pub mod platform;
pub mod ports;
pub mod settings;
pub mod text;
pub mod timeouts;
pub mod ui;

// Re-export commonly used constants explicitly to avoid ambiguous glob re-exports
// This eliminates the compilation warnings about ambiguous re-exports

// From app module
pub use app::{APP_NAME, BUNDLE_IDENTIFIER, PRODUCT_NAME, CONFIG_DIR_NAME, SCREENSHOT_PREFIX};

// From audio module
pub use audio::{WHISPER_SAMPLE_RATE, DEFAULT_SENSITIVITY, DEFAULT_WAKE_WORDS};

// From platform module (macOS-specific)
pub use platform::macos::{key_codes, modifiers, system, system_prefs};

// From files module
pub use files::{CONFIG_FILE_NAME, SETTINGS_FILE_NAME, PROMPTS_FILE_NAME};

// From timeouts module
pub use timeouts::{DEFAULT_TIMEOUT_MS, QUICK_TIMEOUT_MS, SLOW_TIMEOUT_MS};

// From memory module
pub use memory::limits::{MAX_MEMORY_ENTRIES as MEMORY_MAX_ENTRIES, MAX_CONTEXT_TOKENS};

// From mouse module
pub use mouse::{DEFAULT_CLICK_DELAY_MS, DEFAULT_SCROLL_AMOUNT};

// From permissions module
pub use permissions::{REQUIRED_PERMISSIONS, PERMISSION_DESCRIPTIONS};

// From ui module
pub use ui::{FLOATING_BAR_HEIGHT, FLOATING_BAR_WIDTH, SETTINGS_WINDOW_WIDTH, SETTINGS_WINDOW_HEIGHT, bar_states, window_labels};

// Additional re-exports from main branch
pub use agent::monitor_sessions;
pub use api::http_headers;
pub use browser::chrome_debug_urls;
pub use commands::core::*;
pub use errors::{templates, prefixes};
pub use menus::app_menu_ids;
pub use settings::cloud_keys;

// Re-export everything from specific modules that don't cause conflicts
pub use api::*;
pub use audio::*;
pub use browser::*;
pub use cli::*;
pub use error_messages::*;
pub use files::*;
pub use memory::*;
pub use menus::*;
pub use mouse::*;
pub use performance::*;
pub use permissions::*;
pub use platform::*;
pub use ports::*;
pub use text::*;
pub use timeouts::*;

// Re-export module namespaces for structured access
pub use events::agent as agent_events;
pub use events::dictation as dictation_events;
pub use events::always_listening as always_listening_events;
pub use events::ui as ui_events;
pub use events::menu as menu_events;
pub use events::tts as tts_events;
pub use events::cloud as cloud_events;
pub use events::system as system_events;
pub use events::timer as timer_events;
pub use events::bar as bar_events;
pub use events::tools as tools_events;

// Global constants that don't fit in specific modules
pub const MAX_MEMORY_ENTRIES: usize = 1000;

// UI constants
pub const DEFAULT_WINDOW_WIDTH: f64 = 1200.0;
pub const DEFAULT_WINDOW_HEIGHT: f64 = 800.0;

// File system constants
pub const MAX_FILE_SIZE_BYTES: usize = 100 * 1024 * 1024; // 100 MB

// App messages that can be re-exported for convenience
pub mod messages {
    pub const GENERIC_SUCCESS: &str = "Operation completed successfully";
    pub const GENERIC_ERROR: &str = "An error occurred during the operation";
}

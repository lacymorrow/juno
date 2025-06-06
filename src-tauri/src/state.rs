use computer_use_ai_sdk::Desktop;
use std::sync::Arc;

pub mod desktop_wrapper;
pub use desktop_wrapper::DesktopWrapper;
use std::path::PathBuf;
use std::collections::HashMap;
use std::any::{Any, TypeId};
use crate::commands::shell::ShellSessions;
use tokio::sync::{watch, Mutex as TokioMutex};
use log;
use playwright::Playwright; // Import Playwright
use std::sync::Mutex; // Added for tts_provider
use serde::{Serialize, Deserialize}; // Added for keyboard shortcuts

// Import the BrowserController for persistent storage
use crate::agent::tools::browser_controller::BrowserController;
// Import the memory manager for persistent conversation state
use crate::agent::implementations::memory_manager::SimpleMemoryManager;
// Import permissions types
use crate::commands::permissions::PermissionsState;
// Import tool configuration manager
use crate::agent::tools::tool_config::ToolConfigManager;
// Import MCP manager for external MCP server support
use crate::agent::tools::mcp_integration::MCPManager;

/// Keyboard shortcut configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KeyboardShortcuts {
    pub agent_mode_toggle: String,      // Default: Alt+D (Option+D on macOS)
    pub dictation_input: String,        // Default: Alt+Space (Option+Space on macOS)
    pub stop_current_task: String,      // Default: Escape
    pub open_settings: String,          // Default: Cmd+, (Ctrl+, on non-macOS)
}

impl Default for KeyboardShortcuts {
    fn default() -> Self {
        Self {
            agent_mode_toggle: if cfg!(target_os = "macos") { "Option+D".to_string() } else { "Alt+D".to_string() },
            dictation_input: if cfg!(target_os = "macos") { "Option+Space".to_string() } else { "Alt+Space".to_string() },
            stop_current_task: "Escape".to_string(),
            open_settings: if cfg!(target_os = "macos") { "Cmd+,".to_string() } else { "Ctrl+,".to_string() },
        }
    }
}

/// Timestamp tracking for log grouping (Slack/Apple Messages style)
#[derive(Debug, Clone)]
pub struct TimestampTracker {
    pub last_timestamp_shown: Option<u64>,
    pub events_since_last_timestamp: usize,
}

impl TimestampTracker {
    pub fn new() -> Self {
        Self {
            last_timestamp_shown: None,
            events_since_last_timestamp: 0,
        }
    }

    pub fn record_event(&mut self, timestamp: u64, should_show_timestamp: bool) {
        if should_show_timestamp {
            self.last_timestamp_shown = Some(timestamp);
            self.events_since_last_timestamp = 0;
        } else {
            self.events_since_last_timestamp += 1;
        }
    }
}

// Define a type alias for the cancellation sender for clarity
type CancelSender = watch::Sender<bool>;
// Define a type alias for the cancellation receiver for clarity
pub type CancelReceiver = watch::Receiver<bool>;

// Application state structure
#[derive(Clone)] // AppState needs to be Clone
pub struct AppState {
    pub desktop: DesktopWrapper,
    pub shell_sessions: ShellSessions,
    cancel_tx: Arc<CancelSender>, // Store Sender to signal cancellation
    pub cancel_rx: CancelReceiver, // Store Receiver to check for cancellation
    // State for text_editor_undo_edit - Wrapped in Arc
    pub last_edited_file: Arc<std::sync::Mutex<Option<PathBuf>>>,
    pub previous_content: Arc<std::sync::Mutex<Option<Option<String>>>>,
    // Persistent Playwright driver instance, using TokioMutex
    playwright_driver: Arc<TokioMutex<Option<Arc<Playwright>>>>,
    // Persistent browser controller instance
    pub browser_controller: Arc<TokioMutex<Option<BrowserController>>>,
    // Persistent memory manager for conversation history
    pub memory_manager: Arc<TokioMutex<SimpleMemoryManager>>,
    // Dynamic storage for other state components - Wrapped in Arc
    state_components: Arc<std::sync::Mutex<HashMap<TypeId, Box<dyn Any + Send + Sync>>>>,
    pub tts_provider: Arc<Mutex<String>>, // Changed from tts_enabled: Arc<AtomicBool>
    pub bar_ui_state: Arc<Mutex<String>>, // Added to store the current UI state of the floating bar
    pub dictation_active: Arc<Mutex<bool>>, // Track if Dictation Mode is active
    pub dictation_clipboard_enabled: Arc<Mutex<bool>>, // Track if Dictation Mode should save to clipboard
    pub sound_enabled: Arc<Mutex<bool>>, // Track if sound effects are enabled
    pub timestamp_tracker: Arc<Mutex<TimestampTracker>>, // Track timestamps for log grouping
    // Permissions state tracking
    pub permissions_state: Arc<TokioMutex<Option<PermissionsState>>>, // Track permissions status
    pub permissions_checked: Arc<Mutex<bool>>, // Track if permissions have been checked
    // Tool configuration manager
    pub tool_config_manager: Arc<TokioMutex<ToolConfigManager>>, // Manage tool enable/disable settings
    // Keyboard shortcuts configuration
    pub keyboard_shortcuts: Arc<Mutex<KeyboardShortcuts>>, // Manage keyboard shortcuts
    // MCP manager for external MCP server support
    pub mcp_manager: Arc<TokioMutex<MCPManager>>, // Manage external MCP servers and their tools
}

impl AppState {
    pub fn new(desktop: Option<Arc<Desktop>>) -> Self {
        let (cancel_tx, cancel_rx) = watch::channel(false); // Initial state: not cancelled
        Self {
            desktop: DesktopWrapper::new(desktop),
            shell_sessions: ShellSessions::default(),
            cancel_tx: Arc::new(cancel_tx),
            cancel_rx,
            last_edited_file: Arc::new(std::sync::Mutex::new(None)),
            previous_content: Arc::new(std::sync::Mutex::new(None)),
            playwright_driver: Arc::new(TokioMutex::new(None)),
            browser_controller: Arc::new(TokioMutex::new(None)),
            memory_manager: Arc::new(TokioMutex::new(SimpleMemoryManager::new())), // Initialize persistent memory
            state_components: Arc::new(std::sync::Mutex::new(HashMap::new())),
            tts_provider: Arc::new(Mutex::new("system".to_string())), // Initialize TTS provider to "system" (was "off")
            bar_ui_state: Arc::new(Mutex::new("default".to_string())), // Initialize bar UI state
            dictation_active: Arc::new(Mutex::new(false)), // Initialize Dictation Mode as inactive
            dictation_clipboard_enabled: Arc::new(Mutex::new(true)), // Initialize clipboard saving as enabled by default
            sound_enabled: Arc::new(Mutex::new(true)), // Initialize sound effects as enabled by default
            timestamp_tracker: Arc::new(Mutex::new(TimestampTracker::new())), // Initialize timestamp tracker
            // Initialize permissions state
            permissions_state: Arc::new(TokioMutex::new(None)),
            permissions_checked: Arc::new(Mutex::new(false)),
            // Initialize tool configuration manager
            tool_config_manager: Arc::new(TokioMutex::new(ToolConfigManager::new())),
            // Initialize keyboard shortcuts configuration
            keyboard_shortcuts: Arc::new(Mutex::new(KeyboardShortcuts::default())),
            // Initialize MCP manager
            mcp_manager: Arc::new(TokioMutex::new(MCPManager::new())),
        }
    }

    // Method to trigger cancellation
    pub fn signal_cancel(&self) {
        // Send `true` to indicate cancellation is requested.
        // Ignore result as we don't care if there are no active receivers.
        let _ = self.cancel_tx.send(true);
    }

    // Method to reset the cancellation signal
    pub fn reset_cancel(&self) {
        // Send `false` to indicate cancellation is no longer requested.
        // Check if the current value is true before sending false to avoid unnecessary updates.
        let is_currently_cancelled = *self.cancel_rx.borrow();
        log::info!("[AppState] Attempting reset_cancel. Current state (is_cancelled): {}", is_currently_cancelled);
        if is_currently_cancelled {
            let send_result = self.cancel_tx.send(false);
            log::info!("[AppState] reset_cancel: Sent 'false'. Result: {:?}", send_result.is_ok());
        } else {
            log::info!("[AppState] reset_cancel: No reset needed (already false).");
        }
    }

    // Method to get or initialize the Playwright driver
    async fn get_or_init_playwright_driver(&self) -> Result<Arc<Playwright>, String> {
        let mut driver_guard = self.playwright_driver.lock().await;
        if driver_guard.is_none() {
            log::info!("Initializing Playwright driver instance...");
            match Playwright::initialize().await {
                Ok(pw_instance) => {
                    let arc_pw = Arc::new(pw_instance);
                    *driver_guard = Some(arc_pw.clone());
                    log::info!("Playwright driver initialized and stored in AppState.");
                    Ok(arc_pw)
                }
                Err(e) => {
                    let err_msg = format!("Failed to initialize Playwright driver: {}", e);
                    log::error!("{}", err_msg);
                    Err(err_msg)
                }
            }
        } else {
            log::debug!("Reusing existing Playwright driver instance from AppState.");
            Ok(driver_guard.as_ref().unwrap().clone())
        }
    }

    // Method to get or initialize the browser controller
    pub async fn get_or_init_browser_controller(&self) -> Result<BrowserController, String> {
        let mut controller_guard = self.browser_controller.lock().await;

        if controller_guard.is_none() {
            log::info!("Initializing persistent browser controller (was None in AppState)");
            // Get or initialize the Playwright driver first
            let playwright_arc = self.get_or_init_playwright_driver().await
                .map_err(|e| format!("Cannot init BrowserController without Playwright driver: {}", e))?;

            match BrowserController::new(playwright_arc).await {
                Ok(controller) => {
                    *controller_guard = Some(controller.clone());
                    log::info!("BrowserController initialized and stored in AppState.");
                    Ok(controller)
                },
                Err(e) => {
                    let err_msg = format!("Failed to initialize browser controller: {}", e);
                    log::error!("{}", err_msg);
                    Err(err_msg)
                }
            }
        } else {
            log::debug!("Reusing existing browser controller from AppState.");
            Ok(controller_guard.as_ref().unwrap().clone())
        }
    }

    // Method to get the persistent memory manager
    pub async fn get_memory_manager(&self) -> Arc<TokioMutex<SimpleMemoryManager>> {
        self.memory_manager.clone()
    }

    // Insert a component into the state
    pub fn insert<T: 'static + Send + Sync>(&self, component: T) {
        let type_id = TypeId::of::<T>();
        let mut components_lock = self.state_components.lock().unwrap();
        components_lock.insert(type_id, Box::new(component));
    }

    // Get a reference to a component from the state
    pub fn get<T: 'static + Send + Sync + Clone>(&self) -> Option<Arc<T>> {
        let type_id = TypeId::of::<T>();
        let components_lock = self.state_components.lock().unwrap();

        components_lock.get(&type_id).and_then(|boxed| {
            boxed.downcast_ref::<T>().map(|value| {
                // Clone the Arc to extend the lifetime beyond the lock
                Arc::new(value.clone())
            })
        })
    }

    // Method to update permissions state
    pub async fn update_permissions_state(&self, state: PermissionsState) {
        let mut permissions_guard = self.permissions_state.lock().await;
        *permissions_guard = Some(state);
    }

    // Method to get permissions state
    pub async fn get_permissions_state(&self) -> Option<PermissionsState> {
        let permissions_guard = self.permissions_state.lock().await;
        permissions_guard.clone()
    }

    // Method to mark permissions as checked
    pub fn mark_permissions_checked(&self) {
        let mut checked_guard = self.permissions_checked.lock().unwrap();
        *checked_guard = true;
    }

    // Method to check if permissions have been checked
    pub fn are_permissions_checked(&self) -> bool {
        let checked_guard = self.permissions_checked.lock().unwrap();
        *checked_guard
    }

    // Helper method to get desktop instance or return an error
    pub fn get_desktop(&self) -> Result<&Arc<Desktop>, String> {
        self.desktop.get_desktop()
    }

    // Helper method to check if desktop automation is available
    pub fn is_desktop_available(&self) -> bool {
        self.desktop.is_available()
    }

    // Helper method to get desktop instance for situations where we can handle the error gracefully
    pub fn try_get_desktop(&self) -> Option<&Arc<Desktop>> {
        self.desktop.try_get_desktop()
    }

    // Method to get tool configuration manager
    pub async fn get_tool_config_manager(&self) -> Arc<TokioMutex<ToolConfigManager>> {
        self.tool_config_manager.clone()
    }

    // Method to load tool configuration from file
    pub async fn load_tool_config(&self, app_handle: &tauri::AppHandle) -> Result<(), String> {
        let config_path = ToolConfigManager::get_config_path(app_handle)?;
        let loaded_config = ToolConfigManager::load_from_file(&config_path)?;

        let mut config_guard = self.tool_config_manager.lock().await;
        *config_guard = loaded_config;

        Ok(())
    }

    // Method to save tool configuration to file
    pub async fn save_tool_config(&self, app_handle: &tauri::AppHandle) -> Result<(), String> {
        let config_path = ToolConfigManager::get_config_path(app_handle)?;
        let config_guard = self.tool_config_manager.lock().await;
        config_guard.save_to_file(&config_path)
    }

    // MCP Manager Methods

    /// Get the MCP manager
    pub async fn get_mcp_manager(&self) -> Arc<TokioMutex<MCPManager>> {
        self.mcp_manager.clone()
    }

    /// Initialize MCP servers from configuration
    pub async fn initialize_mcp_servers(&self) -> Result<(), String> {
        let config_guard = self.tool_config_manager.lock().await;
        let mcp_configs = config_guard.get_mcp_servers();
        drop(config_guard);

        let mcp_manager = self.get_mcp_manager().await;
        let mut manager_guard = mcp_manager.lock().await;

        for config in mcp_configs {
            if let Err(e) = manager_guard.add_server(config.clone()).await {
                log::error!("Failed to add MCP server '{}': {}", config.name, e);
            }
        }

        // Start all enabled servers
        if let Err(e) = manager_guard.start_all_enabled_servers().await {
            log::error!("Failed to start MCP servers: {}", e);
        }

        drop(manager_guard);
        Ok(())
    }

    /// Sync MCP tools with tool configuration
    pub async fn sync_mcp_tools(&self) -> Result<(), String> {
        let mcp_manager = self.get_mcp_manager().await;
        let manager_guard = mcp_manager.lock().await;
        let all_tools = manager_guard.get_all_tools().await;
        drop(manager_guard);

        let mut config_guard = self.tool_config_manager.lock().await;
        
        // Group tools by server
        let mut tools_by_server: HashMap<String, Vec<_>> = HashMap::new();
        for tool_info in all_tools {
            tools_by_server.entry(tool_info.server_id.clone())
                .or_insert_with(Vec::new)
                .push(tool_info);
        }

        // Add tools to configuration
        for (server_id, tools) in tools_by_server {
            config_guard.add_mcp_tools(&server_id, tools);
        }

        drop(config_guard);
        Ok(())
    }
}

// Helper function to update undo state
#[allow(dead_code)] // Keep allowing dead code as it might be conditionally used
pub(crate) fn update_undo_state(state: &AppState, file_path: PathBuf, previous_content: Option<String>) {
    *state.last_edited_file.lock().unwrap() = Some(file_path);
    *state.previous_content.lock().unwrap() = Some(previous_content);
}

// DesktopWrapper implementation moved to desktop_wrapper.rs

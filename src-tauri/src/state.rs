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
// Import cloud client
use crate::cloud::{CloudClient, CloudConfig, ProductionCloudConnector};
// Import MCP manager for external MCP server support
use crate::agent::tools::mcp_integration::MCPManager;
// Import LocalToolProvider for tool provider registry
use crate::agent::implementations::tool_provider::LocalToolProvider;
use crate::constants::app_identity;
use crate::constants::permission_types;

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
    // Cloud connectivity
    pub cloud_client: Arc<TokioMutex<Option<CloudClient>>>, // Cloud client for remote control
    pub cloud_config: Arc<TokioMutex<CloudConfig>>, // Cloud configuration
    pub cloud_enabled: Arc<Mutex<bool>>, // Track if cloud is enabled
    // Production cloud connector
    pub production_cloud_connector: Arc<TokioMutex<Option<ProductionCloudConnector>>>, // Production connector for remote control
    // Keyboard shortcuts configuration
    pub keyboard_shortcuts: Arc<Mutex<KeyboardShortcuts>>, // Manage keyboard shortcuts
    // MCP manager for external MCP server support
    pub mcp_manager: Arc<TokioMutex<MCPManager>>, // Manage external MCP servers and their tools
    // Tool provider registry for refreshing MCP tools
    pub tool_provider_registry: Arc<Mutex<Vec<Arc<tokio::sync::Mutex<LocalToolProvider>>>>>, // Track active tool providers
    // Always listening mode state
    pub always_listening_active: Arc<Mutex<bool>>, // Track if Always Listening Mode is active
    pub always_listening_sensitivity: Arc<Mutex<f32>>, // Sensitivity threshold for activation
    pub always_listening_wake_words: Arc<Mutex<Vec<String>>>, // Configurable wake words
    // Agent execution status tracking
    pub agent_execution_active: Arc<Mutex<bool>>, // Track if an agent is currently executing
    pub agent_execution_id: Arc<Mutex<Option<String>>>, // Track the current agent execution ID
    // Agent iteration tracking
    pub agent_current_step: Arc<Mutex<Option<u32>>>, // Track the current iteration/step number
    pub agent_max_steps: Arc<Mutex<Option<u32>>>, // Track the maximum iterations/steps allowed
    // First onboarding prompt storage
    pub first_onboarding_prompt: Arc<Mutex<Option<String>>>, // Store the first prompt selected during onboarding
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
            // Initialize cloud connectivity
            cloud_client: Arc::new(TokioMutex::new(None)),
            cloud_config: Arc::new(TokioMutex::new(CloudConfig::default())),
            cloud_enabled: Arc::new(Mutex::new(false)),
            // Initialize production cloud connector
            production_cloud_connector: Arc::new(TokioMutex::new(None)),
            // Initialize keyboard shortcuts configuration
            keyboard_shortcuts: Arc::new(Mutex::new(KeyboardShortcuts::default())),
            // Initialize MCP manager
            mcp_manager: Arc::new(TokioMutex::new(MCPManager::new())),
            // Initialize tool provider registry
            tool_provider_registry: Arc::new(Mutex::new(Vec::new())),
            // Initialize Always Listening mode state
            always_listening_active: Arc::new(Mutex::new(false)),
            always_listening_sensitivity: Arc::new(Mutex::new(0.5)),
            always_listening_wake_words: Arc::new(Mutex::new(
                app_identity::DEFAULT_WAKE_WORDS.iter().map(|s| s.to_string()).collect()
            )),
            // Initialize agent execution status tracking
            agent_execution_active: Arc::new(Mutex::new(false)),
            agent_execution_id: Arc::new(Mutex::new(None)),
            // Initialize agent iteration tracking
            agent_current_step: Arc::new(Mutex::new(None)),
            agent_max_steps: Arc::new(Mutex::new(None)),
            // Initialize first onboarding prompt storage
            first_onboarding_prompt: Arc::new(Mutex::new(None)),
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

    // Method to mark agent execution as started
    pub fn mark_agent_execution_started(&self, execution_id: String) {
        {
            let mut active_guard = self.agent_execution_active.lock().unwrap();
            *active_guard = true;
        }
        {
            let mut id_guard = self.agent_execution_id.lock().unwrap();
            *id_guard = Some(execution_id.clone());
        }
        log::info!("[AppState] Agent execution started with ID: {}", execution_id);
    }

    // Method to mark agent execution as started with iteration info
    pub fn mark_agent_execution_started_with_steps(&self, execution_id: String, max_steps: u32) {
        {
            let mut active_guard = self.agent_execution_active.lock().unwrap();
            *active_guard = true;
        }
        {
            let mut id_guard = self.agent_execution_id.lock().unwrap();
            *id_guard = Some(execution_id.clone());
        }
        {
            let mut max_steps_guard = self.agent_max_steps.lock().unwrap();
            *max_steps_guard = Some(max_steps);
        }
        {
            let mut current_step_guard = self.agent_current_step.lock().unwrap();
            *current_step_guard = Some(0); // Start at step 0
        }
        log::info!("[AppState] Agent execution started with ID: {} (max steps: {})", execution_id, max_steps);
    }

    // Method to mark agent execution as finished
    pub fn mark_agent_execution_finished(&self) {
        {
            let mut active_guard = self.agent_execution_active.lock().unwrap();
            *active_guard = false;
        }
        {
            let mut id_guard = self.agent_execution_id.lock().unwrap();
            let execution_id = id_guard.take();
            log::info!("[AppState] Agent execution finished for ID: {:?}", execution_id);
        }
        {
            let mut current_step_guard = self.agent_current_step.lock().unwrap();
            *current_step_guard = None;
        }
        {
            let mut max_steps_guard = self.agent_max_steps.lock().unwrap();
            *max_steps_guard = None;
        }
    }

    // Method to check if an agent is currently executing
    pub fn is_agent_executing(&self) -> bool {
        let active_guard = self.agent_execution_active.lock().unwrap();
        *active_guard
    }

    // Method to get the current agent execution ID
    pub fn get_current_agent_execution_id(&self) -> Option<String> {
        let id_guard = self.agent_execution_id.lock().unwrap();
        id_guard.clone()
    }

    // Method to update the current agent step
    pub fn update_agent_current_step(&self, step: u32) {
        let mut current_step_guard = self.agent_current_step.lock().unwrap();
        *current_step_guard = Some(step);
        log::debug!("[AppState] Agent current step updated to: {}", step);
    }

    // Method to get the current agent step
    pub fn get_agent_current_step(&self) -> Option<u32> {
        let current_step_guard = self.agent_current_step.lock().unwrap();
        *current_step_guard
    }

    // Method to get the agent max steps
    pub fn get_agent_max_steps(&self) -> Option<u32> {
        let max_steps_guard = self.agent_max_steps.lock().unwrap();
        *max_steps_guard
    }

    // Method to get agent step progress info
    pub fn get_agent_step_progress(&self) -> (Option<u32>, Option<u32>) {
        let current_step = self.get_agent_current_step();
        let max_steps = self.get_agent_max_steps();
        (current_step, max_steps)
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

    // Cloud connectivity methods

    /// Initialize cloud client
    pub async fn init_cloud_client(&self, app_handle: &tauri::AppHandle) -> Result<(), String> {
        // Load cloud configuration
        let config = CloudConfig::load_from_file(app_handle)
            .map_err(|e| format!("Failed to load cloud config: {}", e))?;

        // Update stored config
        {
            let mut config_guard = self.cloud_config.lock().await;
            *config_guard = config.clone();
        }

        // Update enabled status
        {
            let mut enabled_guard = self.cloud_enabled.lock();
            if let Ok(mut enabled) = enabled_guard {
                *enabled = config.enabled;
            }
        }

        // Create cloud client if enabled
        if config.enabled {
            let client = CloudClient::new(app_handle.clone()).await
                .map_err(|e| format!("Failed to create cloud client: {}", e))?;

            let mut client_guard = self.cloud_client.lock().await;
            *client_guard = Some(client);
        }

        Ok(())
    }

    /// Start cloud connectivity
    pub async fn start_cloud_client(&self) -> Result<(), String> {
        let mut client_guard = self.cloud_client.lock().await;
        if let Some(client) = client_guard.as_mut() {
            client.start().await
                .map_err(|e| format!("Failed to start cloud client: {}", e))?;
        }
        Ok(())
    }

    /// Stop cloud connectivity
    pub async fn stop_cloud_client(&self) {
        let mut client_guard = self.cloud_client.lock().await;
        *client_guard = None;
    }

    /// Check if cloud is enabled
    pub fn is_cloud_enabled(&self) -> bool {
        self.cloud_enabled.lock()
            .map(|enabled| *enabled)
            .unwrap_or(false)
    }

    /// Get cloud configuration
    pub async fn get_cloud_config(&self) -> CloudConfig {
        let config_guard = self.cloud_config.lock().await;
        config_guard.clone()
    }

    /// Update cloud configuration
    pub async fn update_cloud_config(&self, config: CloudConfig, app_handle: &tauri::AppHandle) -> Result<(), String> {
        // Save to file
        config.save_to_file(app_handle)
            .map_err(|e| format!("Failed to save cloud config: {}", e))?;

        // Update stored config
        {
            let mut config_guard = self.cloud_config.lock().await;
            *config_guard = config.clone();
        }

        // Update enabled status
        {
            let mut enabled_guard = self.cloud_enabled.lock();
            if let Ok(mut enabled) = enabled_guard {
                *enabled = config.enabled;
            }
        }

        // Restart cloud client if needed
        if config.enabled {
            self.stop_cloud_client().await;
            self.init_cloud_client(app_handle).await?;
            self.start_cloud_client().await?;
        } else {
            self.stop_cloud_client().await;
        }

        Ok(())
    }

    // Production cloud connector methods

    /// Set production cloud connector
    pub async fn set_production_cloud_connector(&self, connector: ProductionCloudConnector) {
        let mut connector_guard = self.production_cloud_connector.lock().await;
        *connector_guard = Some(connector);
    }

    /// Get production cloud connector
    pub fn get_production_cloud_connector(&self) -> Option<ProductionCloudConnector> {
        // We need to use try_lock here since this method is not async
        // and we want to avoid blocking the caller
        if let Ok(connector_guard) = self.production_cloud_connector.try_lock() {
            connector_guard.clone()
        } else {
            None
        }
    }

    /// Get production cloud connector (async version)
    pub async fn get_production_cloud_connector_async(&self) -> Option<ProductionCloudConnector> {
        let connector_guard = self.production_cloud_connector.lock().await;
        connector_guard.clone()
    }

    /// Clear production cloud connector
    pub async fn clear_production_cloud_connector(&self) {
        let mut connector_guard = self.production_cloud_connector.lock().await;
        *connector_guard = None;
    }

    /// Check if production cloud connector is available
    pub async fn has_production_cloud_connector(&self) -> bool {
        let connector_guard = self.production_cloud_connector.lock().await;
        connector_guard.is_some()
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
        let manager_guard = mcp_manager.lock().await;

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

        // Emit an event to trigger tool provider refresh in active agents
        self.notify_mcp_tools_updated().await;

        // Refresh all registered tool providers with the new MCP tools
        self.refresh_all_tool_providers().await?;

        Ok(())
    }

    /// Notify that MCP tools have been updated - this triggers a refresh in active agents
    pub async fn notify_mcp_tools_updated(&self) {
        // Try to get an app handle and emit the event
        if let Ok(controller_guard) = self.browser_controller.try_lock() {
            if let Some(ref controller) = *controller_guard {
                // Just emit without trying to get app handle from controller
                log::debug!("MCP tools updated, notifying frontend");
            }
        }
    }

    /// Register a tool provider for MCP tool refresh notifications
    pub fn register_tool_provider(&self, provider: Arc<tokio::sync::Mutex<LocalToolProvider>>) {
        if let Ok(mut registry) = self.tool_provider_registry.lock() {
            registry.push(provider);
            log::debug!("Registered tool provider for MCP refresh notifications");
        }
    }

    /// Refresh all registered tool providers when MCP tools are updated
    pub async fn refresh_all_tool_providers(&self) -> Result<(), String> {
        let registry = {
            if let Ok(registry_guard) = self.tool_provider_registry.lock() {
                registry_guard.clone()
            } else {
                log::warn!("Failed to access tool provider registry");
                return Ok(());
            }
        };

        log::info!("Refreshing {} registered tool providers with updated MCP tools", registry.len());

        for provider_arc in registry.iter() {
            if let Ok(mut provider) = provider_arc.try_lock() {
                if let Err(e) = provider.refresh_mcp_tools().await {
                    log::warn!("Failed to refresh MCP tools for tool provider: {}", e);
                } else {
                    log::debug!("Successfully refreshed MCP tools for tool provider");
                }
            } else {
                log::warn!("Tool provider is busy, skipping MCP refresh");
            }
        }

        Ok(())
    }

    // Method to set the first onboarding prompt
    pub fn set_first_onboarding_prompt(&self, prompt: String) {
        let mut prompt_guard = self.first_onboarding_prompt.lock().unwrap();
        *prompt_guard = Some(prompt);
    }

    // Method to get the first onboarding prompt
    pub fn get_first_onboarding_prompt(&self) -> Option<String> {
        let prompt_guard = self.first_onboarding_prompt.lock().unwrap();
        prompt_guard.clone()
    }
}

// Helper function to update undo state
#[allow(dead_code)] // Keep allowing dead code as it might be conditionally used
pub(crate) fn update_undo_state(state: &AppState, file_path: PathBuf, previous_content: Option<String>) {
    // Safely handle potential lock poisoning
    if let Ok(mut last_edited) = state.last_edited_file.lock() {
        *last_edited = Some(file_path);
    } else {
        log::error!("Failed to acquire lock for last_edited_file - lock may be poisoned");
    }
    
    if let Ok(mut previous) = state.previous_content.lock() {
        *previous = Some(previous_content);
    } else {
        log::error!("Failed to acquire lock for previous_content - lock may be poisoned");
    }
}

// DesktopWrapper implementation moved to desktop_wrapper.rs

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;
    use tokio::time::timeout;

    #[test]
    fn test_keyboard_shortcuts_default() {
        let shortcuts = KeyboardShortcuts::default();
        
        #[cfg(target_os = "macos")]
        {
            assert_eq!(shortcuts.agent_mode_toggle, "Option+D");
            assert_eq!(shortcuts.dictation_input, "Option+Space");
            assert_eq!(shortcuts.open_settings, "Cmd+,");
        }
        
        #[cfg(not(target_os = "macos"))]
        {
            assert_eq!(shortcuts.agent_mode_toggle, "Alt+D");
            assert_eq!(shortcuts.dictation_input, "Alt+Space");
            assert_eq!(shortcuts.open_settings, "Ctrl+,");
        }
        
        assert_eq!(shortcuts.stop_current_task, "Escape");
    }

    #[test]
    fn test_keyboard_shortcuts_serialization() {
        let shortcuts = KeyboardShortcuts::default();
        let serialized = serde_json::to_string(&shortcuts).unwrap();
        let deserialized: KeyboardShortcuts = serde_json::from_str(&serialized).unwrap();
        
        assert_eq!(shortcuts.agent_mode_toggle, deserialized.agent_mode_toggle);
        assert_eq!(shortcuts.dictation_input, deserialized.dictation_input);
        assert_eq!(shortcuts.stop_current_task, deserialized.stop_current_task);
        assert_eq!(shortcuts.open_settings, deserialized.open_settings);
    }

    #[test]
    fn test_timestamp_tracker_new() {
        let tracker = TimestampTracker::new();
        assert!(tracker.last_timestamp_shown.is_none());
        assert_eq!(tracker.events_since_last_timestamp, 0);
    }

    #[test]
    fn test_timestamp_tracker_record_event() {
        let mut tracker = TimestampTracker::new();
        
        // Record event with timestamp shown
        tracker.record_event(1000, true);
        assert_eq!(tracker.last_timestamp_shown, Some(1000));
        assert_eq!(tracker.events_since_last_timestamp, 0);
        
        // Record event without timestamp shown
        tracker.record_event(2000, false);
        assert_eq!(tracker.last_timestamp_shown, Some(1000)); // Should remain unchanged
        assert_eq!(tracker.events_since_last_timestamp, 1);
        
        // Record another event without timestamp
        tracker.record_event(3000, false);
        assert_eq!(tracker.events_since_last_timestamp, 2);
        
        // Record event with timestamp shown again
        tracker.record_event(4000, true);
        assert_eq!(tracker.last_timestamp_shown, Some(4000));
        assert_eq!(tracker.events_since_last_timestamp, 0);
    }

    #[tokio::test]
    async fn test_app_state_creation() {
        let state = AppState::new(None);
        
        // Test initial state values
        assert!(!state.is_agent_executing());
        assert!(state.get_current_agent_execution_id().is_none());
        assert!(!state.are_permissions_checked());
        assert!(!state.is_cloud_enabled());
        
        // Test Arc-wrapped values
        {
            let tts_provider = state.tts_provider.lock().unwrap();
            assert_eq!(*tts_provider, "system");
        }
        
        {
            let dictation_active = state.dictation_active.lock().unwrap();
            assert!(!*dictation_active);
        }
        
        {
            let sound_enabled = state.sound_enabled.lock().unwrap();
            assert!(*sound_enabled);
        }
        
        {
            let always_listening = state.always_listening_active.lock().unwrap();
            assert!(!*always_listening);
        }
    }

    #[tokio::test]
    async fn test_agent_execution_tracking() {
        let state = AppState::new(None);
        let execution_id = "test-execution-123".to_string();
        
        // Initially not executing
        assert!(!state.is_agent_executing());
        assert!(state.get_current_agent_execution_id().is_none());
        
        // Mark as started
        state.mark_agent_execution_started(execution_id.clone());
        assert!(state.is_agent_executing());
        assert_eq!(state.get_current_agent_execution_id(), Some(execution_id));
        
        // Mark as finished
        state.mark_agent_execution_finished();
        assert!(!state.is_agent_executing());
        assert!(state.get_current_agent_execution_id().is_none());
    }

    #[tokio::test]
    async fn test_cancellation_signaling() {
        let state = AppState::new(None);
        
        // Initially not cancelled
        assert!(!*state.cancel_rx.borrow());
        
        // Signal cancellation
        state.signal_cancel();
        
        // Should be cancelled now
        assert!(*state.cancel_rx.borrow());
        
        // Reset cancellation
        state.reset_cancel();
        
        // Should not be cancelled anymore
        assert!(!*state.cancel_rx.borrow());
    }

    #[tokio::test]
    async fn test_cancel_receiver_watch() {
        let state = AppState::new(None);
        let mut cancel_rx = state.cancel_rx.clone();
        
        // Test that we can watch for changes
        let initial_value = *cancel_rx.borrow();
        assert!(!initial_value);
        
        // Signal cancellation in a separate task
        let state_clone = state.clone();
        tokio::spawn(async move {
            tokio::time::sleep(Duration::from_millis(10)).await;
            state_clone.signal_cancel();
        });
        
        // Wait for the change
        let changed = timeout(Duration::from_millis(100), cancel_rx.changed()).await;
        assert!(changed.is_ok());
        
        // Value should now be true
        assert!(*cancel_rx.borrow());
    }

    #[tokio::test]
    async fn test_memory_manager_access() {
        let state = AppState::new(None);
        
        let memory_manager = state.get_memory_manager().await;
        assert!(memory_manager.try_lock().is_ok());
    }

    #[test]
    fn test_permissions_tracking() {
        let state = AppState::new(None);
        
        // Initially not checked
        assert!(!state.are_permissions_checked());
        
        // Mark as checked
        state.mark_permissions_checked();
        assert!(state.are_permissions_checked());
    }

    #[tokio::test]
    async fn test_permissions_state_update() {
        let state = AppState::new(None);
        
        // Initially no permissions state
        assert!(state.get_permissions_state().await.is_none());
        
        // Create a mock permissions state with correct structure
        use crate::commands::permissions::{PermissionsState, PermissionStatus};
        let mock_permissions = vec![
            PermissionStatus {
                permission_type: permission_types::ACCESSIBILITY.to_string(),
                granted: true,
                required: true,
                description: "Accessibility permission is granted".to_string(),
                instructions: "No action needed".to_string(),
            },
            PermissionStatus {
                permission_type: permission_types::SCREEN_RECORDING.to_string(),
                granted: false,
                required: true,
                description: "Screen recording permission is denied".to_string(),
                instructions: "Grant in System Preferences".to_string(),
            },
            PermissionStatus {
                permission_type: permission_types::MICROPHONE.to_string(),
                granted: true,
                required: false,
                description: "Microphone permission not determined".to_string(),
                instructions: "Will prompt when needed".to_string(),
            },
            PermissionStatus {
                permission_type: permission_types::INPUT_MONITORING.to_string(),
                granted: true,
                required: true,
                description: "Input monitoring permission is granted".to_string(),
                instructions: "No action needed".to_string(),
            },
        ];
        
        let permissions_state = PermissionsState {
            accessibility: mock_permissions[0].clone(),
            screen_recording: mock_permissions[1].clone(),
            microphone: mock_permissions[2].clone(),
            input_monitoring: mock_permissions[3].clone(),
            all_granted: false,
            app_name: "TestApp".to_string(),
        };
        
        // Update permissions state
        state.update_permissions_state(permissions_state.clone()).await;
        
        // Should now have the permissions state
        let retrieved_state = state.get_permissions_state().await;
        assert!(retrieved_state.is_some());
        let retrieved = retrieved_state.unwrap();
        assert_eq!(retrieved.accessibility.granted, true);
        assert_eq!(retrieved.screen_recording.granted, false);
        assert_eq!(retrieved.all_granted, false);
        assert_eq!(retrieved.app_name, "TestApp");
    }

    #[test]
    fn test_desktop_availability() {
        let state = AppState::new(None);
        
        // With no desktop instance provided, should not be available
        assert!(!state.is_desktop_available());
        assert!(state.try_get_desktop().is_none());
        assert!(state.get_desktop().is_err());
    }

    #[test]
    fn test_state_components_storage() {
        let state = AppState::new(None);
        
        // Test inserting and retrieving a component
        #[derive(Clone, Debug, PartialEq)]
        struct TestComponent {
            value: String,
        };
        
        let test_component = TestComponent {
            value: "test_value".to_string(),
        };
        
        // Insert component
        state.insert(test_component.clone());
        
        // Retrieve component
        let retrieved = state.get::<TestComponent>();
        assert!(retrieved.is_some());
        assert_eq!(retrieved.unwrap().value, "test_value");
        
        // Try to retrieve non-existent component
        #[derive(Clone)]
        struct OtherComponent;
        
        let other = state.get::<OtherComponent>();
        assert!(other.is_none());
    }

    #[test]
    fn test_tts_provider_management() {
        let state = AppState::new(None);
        
        // Check initial value
        {
            let tts_provider = state.tts_provider.lock().unwrap();
            assert_eq!(*tts_provider, "system");
        }
        
        // Update TTS provider
        {
            let mut tts_provider = state.tts_provider.lock().unwrap();
            *tts_provider = "openai".to_string();
        }
        
        // Verify update
        {
            let tts_provider = state.tts_provider.lock().unwrap();
            assert_eq!(*tts_provider, "openai");
        }
    }

    #[test]
    fn test_dictation_state_management() {
        let state = AppState::new(None);
        
        // Check initial state
        {
            let dictation_active = state.dictation_active.lock().unwrap();
            assert!(!*dictation_active);
        }
        
        {
            let clipboard_enabled = state.dictation_clipboard_enabled.lock().unwrap();
            assert!(*clipboard_enabled);
        }
        
        // Update dictation state
        {
            let mut dictation_active = state.dictation_active.lock().unwrap();
            *dictation_active = true;
        }
        
        {
            let mut clipboard_enabled = state.dictation_clipboard_enabled.lock().unwrap();
            *clipboard_enabled = false;
        }
        
        // Verify updates
        {
            let dictation_active = state.dictation_active.lock().unwrap();
            assert!(*dictation_active);
        }
        
        {
            let clipboard_enabled = state.dictation_clipboard_enabled.lock().unwrap();
            assert!(!*clipboard_enabled);
        }
    }

    #[test]
    fn test_always_listening_configuration() {
        let state = AppState::new(None);
        
        // Check initial values
        {
            let active = state.always_listening_active.lock().unwrap();
            assert!(!*active);
        }
        
        {
            let sensitivity = state.always_listening_sensitivity.lock().unwrap();
            assert_eq!(*sensitivity, 0.5);
        }
        
        {
            let wake_words = state.always_listening_wake_words.lock().unwrap();
            assert_eq!(wake_words.len(), 2);
            assert!(wake_words.contains(&app_identity::DEFAULT_WAKE_WORDS[0].to_string()));
            assert!(wake_words.contains(&app_identity::DEFAULT_WAKE_WORDS[1].to_string()));
        }
        
        // Update configuration
        {
            let mut active = state.always_listening_active.lock().unwrap();
            *active = true;
        }
        
        {
            let mut sensitivity = state.always_listening_sensitivity.lock().unwrap();
            *sensitivity = 0.8;
        }
        
        {
            let mut wake_words = state.always_listening_wake_words.lock().unwrap();
            wake_words.push("assistant".to_string());
        }
        
        // Verify updates
        {
            let active = state.always_listening_active.lock().unwrap();
            assert!(*active);
        }
        
        {
            let sensitivity = state.always_listening_sensitivity.lock().unwrap();
            assert_eq!(*sensitivity, 0.8);
        }
        
        {
            let wake_words = state.always_listening_wake_words.lock().unwrap();
            assert_eq!(wake_words.len(), 3);
            assert!(wake_words.contains(&"assistant".to_string()));
        }
    }

    #[test]
    fn test_app_state_clone() {
        let state1 = AppState::new(None);
        let state2 = state1.clone();
        
        // Both should track the same agent execution state
        state1.mark_agent_execution_started("test-123".to_string());
        assert!(state2.is_agent_executing());
        assert_eq!(state2.get_current_agent_execution_id(), Some("test-123".to_string()));
        
        // Both should respond to cancellation signals
        state1.signal_cancel();
        assert!(*state2.cancel_rx.borrow());
    }

    #[tokio::test]
    async fn test_browser_controller_lazy_initialization() {
        let state = AppState::new(None);
        
        // Initially no browser controller
        {
            let controller_guard = state.browser_controller.lock().await;
            assert!(controller_guard.is_none());
        }
        
        // get_or_init_browser_controller should fail without Playwright
        // but should not panic
        let result = state.get_or_init_browser_controller().await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_cloud_configuration() {
        let state = AppState::new(None);
        
        // Initially cloud should be disabled
        assert!(!state.is_cloud_enabled());
        
        // Get initial cloud config
        let config = state.get_cloud_config().await;
        assert!(!config.enabled);
    }
}

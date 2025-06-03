use computer_use_ai_sdk::Desktop;
use std::sync::Arc;
use std::path::PathBuf;
use std::collections::HashMap;
use std::any::{Any, TypeId};
use crate::commands::shell::ShellSessions;
use tokio::sync::{watch, Mutex as TokioMutex};
use log;
use playwright::Playwright; // Import Playwright
use std::sync::Mutex; // Added for tts_provider

// Import the BrowserController for persistent storage
use crate::agent::tools::browser_controller::BrowserController;
// Import the memory manager for persistent conversation state
use crate::agent::implementations::memory_manager::SimpleMemoryManager;

// Define a type alias for the cancellation sender for clarity
type CancelSender = watch::Sender<bool>;
// Define a type alias for the cancellation receiver for clarity
pub type CancelReceiver = watch::Receiver<bool>;

// Application state structure
#[derive(Clone)] // AppState needs to be Clone
pub struct AppState {
    pub desktop: Arc<Desktop>,
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
    pub spacebar_dictation_active: Arc<Mutex<bool>>, // Track if spacebar dictation is active
}

impl AppState {
    pub fn new(desktop: Arc<Desktop>) -> Self {
        let (cancel_tx, cancel_rx) = watch::channel(false); // Initial state: not cancelled
        Self {
            desktop,
            shell_sessions: ShellSessions::default(),
            cancel_tx: Arc::new(cancel_tx),
            cancel_rx,
            last_edited_file: Arc::new(std::sync::Mutex::new(None)),
            previous_content: Arc::new(std::sync::Mutex::new(None)),
            playwright_driver: Arc::new(TokioMutex::new(None)),
            browser_controller: Arc::new(TokioMutex::new(None)),
            memory_manager: Arc::new(TokioMutex::new(SimpleMemoryManager::new())), // Initialize persistent memory
            state_components: Arc::new(std::sync::Mutex::new(HashMap::new())),
            tts_provider: Arc::new(Mutex::new("off".to_string())), // Initialize TTS provider to "off"
            bar_ui_state: Arc::new(Mutex::new("default".to_string())), // Initialize bar UI state
            spacebar_dictation_active: Arc::new(Mutex::new(false)), // Initialize spacebar dictation as inactive
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
}

// Helper function to update undo state
#[allow(dead_code)] // Keep allowing dead code as it might be conditionally used
pub(crate) fn update_undo_state(state: &AppState, file_path: PathBuf, previous_content: Option<String>) {
    *state.last_edited_file.lock().unwrap() = Some(file_path);
    *state.previous_content.lock().unwrap() = Some(previous_content);
}

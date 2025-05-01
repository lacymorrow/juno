use computer_use_ai_sdk::Desktop;
use std::sync::{Arc, Mutex};
use std::path::PathBuf;
use std::collections::HashMap;
use std::any::{Any, TypeId};
use crate::commands::shell::ShellSessions;
use tokio::sync::watch;
use log;

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
    pub last_edited_file: Arc<Mutex<Option<PathBuf>>>,
    pub previous_content: Arc<Mutex<Option<Option<String>>>>,
    // Dynamic storage for other state components - Wrapped in Arc
    state_components: Arc<Mutex<HashMap<TypeId, Box<dyn Any + Send + Sync>>>>,
}

impl AppState {
    pub fn new(desktop: Arc<Desktop>) -> Self {
        let (cancel_tx, cancel_rx) = watch::channel(false); // Initial state: not cancelled
        Self {
            desktop,
            shell_sessions: ShellSessions::default(),
            cancel_tx: Arc::new(cancel_tx),
            cancel_rx,
            last_edited_file: Arc::new(Mutex::new(None)),
            previous_content: Arc::new(Mutex::new(None)),
            state_components: Arc::new(Mutex::new(HashMap::new())),
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

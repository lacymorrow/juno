use computer_use_ai_sdk::Desktop;
use std::sync::{Arc, Mutex};
use std::path::PathBuf;
use std::collections::HashMap;
use std::any::{Any, TypeId};

// Application state structure
#[allow(dead_code)] // Allow dead code for potentially unused fields
pub struct AppState {
    pub desktop: Arc<Desktop>,
    // State for text_editor_undo_edit
    pub last_edited_file: Mutex<Option<PathBuf>>,
    pub previous_content: Mutex<Option<Option<String>>>,
    // Dynamic storage for other state components
    state_components: Mutex<HashMap<TypeId, Box<dyn Any + Send + Sync>>>,
}

impl AppState {
    pub fn new(desktop: Arc<Desktop>) -> Self {
        Self {
            desktop,
            last_edited_file: Mutex::new(None),
            previous_content: Mutex::new(None),
            state_components: Mutex::new(HashMap::new()),
        }
    }

    // Insert a component into the state
    pub fn insert<T: 'static + Send + Sync>(&self, component: T) {
        let type_id = TypeId::of::<T>();
        let mut state_components = self.state_components.lock().unwrap();
        state_components.insert(type_id, Box::new(component));
    }

    // Get a reference to a component from the state
    pub fn get<T: 'static + Send + Sync + Clone>(&self) -> Option<Arc<T>> {
        let type_id = TypeId::of::<T>();
        let guard = self.state_components.lock().unwrap();

        guard.get(&type_id).and_then(|boxed| {
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

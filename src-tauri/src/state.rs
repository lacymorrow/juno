use computer_use_ai_sdk::Desktop;
use std::sync::{Arc, Mutex};
use std::path::PathBuf;

// Application state structure
#[allow(dead_code)] // Allow dead code for potentially unused fields
pub struct AppState {
    pub desktop: Arc<Desktop>,
    // State for text_editor_undo_edit
    pub last_edited_file: Mutex<Option<PathBuf>>,
    pub previous_content: Mutex<Option<Option<String>>>,
}

impl AppState {
    pub fn new(desktop: Arc<Desktop>) -> Self {
        Self {
            desktop,
            last_edited_file: Mutex::new(None),
            previous_content: Mutex::new(None),
        }
    }
}

// Helper function to update undo state
#[allow(dead_code)] // Keep allowing dead code as it might be conditionally used
pub(crate) fn update_undo_state(state: &AppState, file_path: PathBuf, previous_content: Option<String>) {
    *state.last_edited_file.lock().unwrap() = Some(file_path);
    *state.previous_content.lock().unwrap() = Some(previous_content);
}

use crate::element::UIElement;
use crate::{AutomationError, Selector, ElementTreeNode};
use anyhow::Result;
use std::any::Any;
use serde_json::Value as JsonValue;

/// The common trait that all platform-specific engines must implement
pub trait AccessibilityEngine: Send + Sync + Any {
    /// Get the root UI element
    fn get_root_element(&self) -> UIElement;

    #[cfg(target_os = "windows")]
    fn get_element_by_id(&self, _id: &str) -> Result<UIElement, AutomationError>;
    /// Get the currently focused element
    fn get_focused_element(&self) -> Result<UIElement, AutomationError>;

    /// Get all running applications
    fn get_applications(&self) -> Result<Vec<UIElement>, AutomationError>;

    /// Get application by name
    fn get_application_by_name(&self, name: &str) -> Result<UIElement, AutomationError>;

    /// Find elements using a selector
    fn find_element(
        &self,
        selector: &Selector,
        root: Option<&UIElement>,
    ) -> Result<UIElement, AutomationError>;

    /// Find all elements matching a selector
    /// Default implementation returns an UnsupportedOperation error,
    /// allowing platform-specific implementations to override as needed
    fn find_elements(
        &self,
        selector: &Selector,
        root: Option<&UIElement>,
    ) -> Result<Vec<UIElement>, AutomationError>;

    /// Open an application by name
    fn open_application(&self, app_name: &str) -> Result<UIElement, AutomationError>;

    /// Open a URL in a specified browser (or default if None)
    fn open_url(&self, url: &str, browser: Option<&str>) -> Result<UIElement, AutomationError>;

    /// Convert to Any for downcasting
    fn as_any(&self) -> &dyn Any;

    //Scroll at a specific position on screen
    fn scroll_at_position(
        &self,
        x: f64,
        y: f64,
        direction: &str,
        amount: f64,
    ) -> Result<(), AutomationError>;

    // Scroll at the current mouse position
    fn scroll_at_current_position(
        &self,
        direction: &str,
        amount: f64,
    ) -> Result<(), AutomationError>;

    /// Type text
    fn type_text(&self, text: &str) -> Result<(), AutomationError>;

    /// Get clipboard content
    fn get_clipboard_content(&self) -> Result<String, AutomationError>;

    /// Set clipboard content
    fn set_clipboard_content(&self, content: &str) -> Result<(), AutomationError>;

    /// Hold down a modifier key
    fn hold_key(&self, key: &str) -> Result<(), AutomationError>;

    /// Release a modifier key
    fn release_key(&self, key: &str) -> Result<(), AutomationError>;

    /// Wait for a specified duration
    fn wait(&self, duration_ms: u64) -> Result<(), AutomationError>;

    /// Press a single key with an optional modifier
    fn press_key(&self, key_name: &str, modifier: Option<&str>) -> Result<(), AutomationError>;

    /// Get the UI tree starting from a specific app or the focused one
    fn get_ui_tree(&self, app_name: Option<&str>) -> Result<JsonValue, AutomationError>;

    /// Get the current mouse cursor position.
    fn cursor_position(&self) -> Result<(f64, f64), AutomationError>;

    /// Move the mouse cursor to the specified coordinates.
    fn mouse_move(&self, x: f64, y: f64) -> Result<(), AutomationError>;

    /// Simulate pressing the left mouse button down at the specified coordinates.
    fn left_mouse_down(&self, x: f64, y: f64) -> Result<(), AutomationError>;

    /// Simulate releasing the left mouse button at the specified coordinates.
    fn left_mouse_up(&self, x: f64, y: f64) -> Result<(), AutomationError>;

    /// Simulate a standard left click (down + up) at specified coordinates.
    fn left_click(&self, x: f64, y: f64) -> Result<(), AutomationError>;

    /// Simulate a right click (down + up) at specified coordinates.
    fn right_click(&self, x: f64, y: f64) -> Result<(), AutomationError>;

    /// Simulate a middle click (down + up) at specified coordinates.
    fn middle_click(&self, x: f64, y: f64) -> Result<(), AutomationError>;

    /// Simulate a double left click at the specified coordinates.
    fn double_click(&self, x: f64, y: f64) -> Result<(), AutomationError>;

    /// Simulate a triple left click at the specified coordinates.
    fn triple_click(&self, x: f64, y: f64) -> Result<(), AutomationError>;

    /// Simulate dragging with the left mouse button from a start point to an end point.
    fn left_click_drag(
        &self,
        start_x: f64,
        start_y: f64,
        end_x: f64,
        end_y: f64,
    ) -> Result<(), AutomationError>;

    /// Get the title of the currently focused window.
    fn get_window_title(&self) -> Result<String, AutomationError>;

    /// Get a list of all open window elements (could be filtered by app in implementation).
    fn list_windows(&self) -> Result<Vec<UIElement>, AutomationError>;

    /// Close the currently focused window.
    fn close_window(&self) -> Result<(), AutomationError>;

    /// Maximize the currently focused window.
    fn maximize_window(&self) -> Result<(), AutomationError>;

    /// Minimize the currently focused window.
    fn minimize_window(&self) -> Result<(), AutomationError>;

    /// Resize the currently focused window.
    fn resize_window(&self, width: f64, height: f64) -> Result<(), AutomationError>;

    /// Move the currently focused window.
    fn move_window(&self, x: f64, y: f64) -> Result<(), AutomationError>;

    /// Get the element tree starting from a specific element
    fn get_element_tree(&self, element: &UIElement) -> Result<ElementTreeNode, AutomationError>;
}

#[cfg(target_os = "linux")]
mod linux;
#[cfg(target_os = "macos")]
pub mod macos;
#[cfg(target_os = "macos")]
pub mod tree_search;
#[cfg(target_os = "windows")]
mod windows;

/// Create the appropriate engine for the current platform
pub fn create_engine(
    use_background_apps: bool,
    activate_app: bool,
) -> Result<Box<dyn AccessibilityEngine>, AutomationError> {
    #[cfg(target_os = "macos")]
    {
        return Ok(Box::new(macos::MacOSEngine::new(
            use_background_apps,
            activate_app,
        )?));
    }
    #[cfg(target_os = "windows")]
    {
        return Err(AutomationError::UnsupportedPlatform(
            "Windows not yet supported".to_string(),
        ));
    }
    #[cfg(target_os = "linux")]
    {
        return Err(AutomationError::UnsupportedPlatform(
            "Linux not yet supported".to_string(),
        ));
    }
    #[cfg(not(any(target_os = "macos", target_os = "windows", target_os = "linux")))]
    {
        Err(AutomationError::UnsupportedPlatform(
            "Unsupported operating system".to_string(),
        ))
    }
}

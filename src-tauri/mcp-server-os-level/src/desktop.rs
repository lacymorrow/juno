//! Desktop UI automation through accessibility APIs
//!
//! This module provides a cross-platform API for automating desktop applications
//! through accessibility APIs, inspired by Playwright's web automation model.

use std::sync::Arc;

mod element;
mod errors;
mod locator;
pub mod platforms;
mod selector;
#[cfg(test)]
mod tests;


pub use element::{UIElement, UIElementAttributes};
pub use errors::AutomationError;
pub use locator::Locator;
pub use selector::Selector;

// Define a new struct to hold click result information - move to module level
pub struct ClickResult {
    pub method: String,
    pub coordinates: Option<(f64, f64)>,
    pub details: String,
}

/// The main entry point for UI automation
pub struct Desktop {
    engine: Arc<dyn platforms::AccessibilityEngine>,
}

impl Desktop {
    /// Create a new instance with the default platform-specific implementation
    pub fn new(use_background_apps: bool, activate_app: bool) -> Result<Self, AutomationError> {
        let boxed_engine = platforms::create_engine(use_background_apps, activate_app)?;
        // Move the boxed engine into an Arc
        let engine = Arc::from(boxed_engine);
        Ok(Self { engine })
    }

    /// Get the root UI element representing the entire desktop
    pub fn root(&self) -> UIElement {
        self.engine.get_root_element()
    }

    /// Create a locator to find elements matching the given selector
    pub fn locator(&self, selector: impl Into<Selector>) -> Locator {
        Locator::new(Arc::clone(&self.engine), selector.into())
    }

    /// Returns the accessibility element at the given screen coordinates, if any.
    /// Uses native platform hit-testing (~1-5ms on macOS).
    pub fn element_at_position(&self, x: f64, y: f64) -> Option<UIElement> {
        self.engine.element_at_position(x, y)
    }

    /// Get the currently focused element
    pub fn focused_element(&self) -> Result<UIElement, AutomationError> {
        self.engine.get_focused_element()
    }

    /// List all running applications
    pub fn applications(&self) -> Result<Vec<UIElement>, AutomationError> {
        self.engine.get_applications()
    }

    /// Find an application by name
    pub fn application(&self, name: &str) -> Result<UIElement, AutomationError> {
        self.engine.get_application_by_name(name)
    }

    /// Open an application by name
    pub fn open_application(&self, app_name: &str) -> Result<UIElement, AutomationError> {
        self.engine.open_application(app_name)
    }

    /// Open a URL in a specified browser (or default browser if None)
    pub fn open_url(&self, url: &str, browser: Option<&str>) -> Result<UIElement, AutomationError> {
        self.engine.open_url(url, browser)
    }

    /// Scroll at a specific position on the screen
    pub fn scroll_at_position(&self, x: f64, y: f64, direction: &str, amount: f64) -> Result<(), AutomationError> {
        self.engine.scroll_at_position(x, y, direction, amount)
    }

    /// Scroll at the current mouse position
    pub fn scroll_at_current_position(&self, direction: &str, amount: f64) -> Result<(), AutomationError> {
        self.engine.scroll_at_current_position(direction, amount)
    }

    /// Get the current clipboard content
    pub fn get_clipboard_content(&self) -> Result<String, AutomationError> {
        self.engine.get_clipboard_content()
    }

    /// Set the clipboard content
    pub fn set_clipboard_content(&self, content: &str) -> Result<(), AutomationError> {
        self.engine.set_clipboard_content(content)
    }

    /// Hold down a modifier key, optionally for a specific duration
    pub fn hold_key(&self, key: &str, duration_ms: Option<u64>) -> Result<(), AutomationError> {
        self.engine.hold_key(key, duration_ms)
    }

    /// Release a modifier key
    pub fn release_key(&self, key: &str) -> Result<(), AutomationError> {
        self.engine.release_key(key)
    }

    /// Simulate a standard left click (down + up) at specified coordinates.
    pub fn left_click(&self, x: f64, y: f64, modifiers: Option<&str>) -> Result<(), AutomationError> {
        self.engine.left_click(x, y, modifiers)
    }

    /// Click without warping the system cursor — tiered: SkyLight → CGEventPostToPid → HID-restore.
    pub fn left_click_no_warp(&self, x: f64, y: f64, modifiers: Option<&str>) -> Result<&'static str, AutomationError> {
        self.engine.left_click_no_warp(x, y, modifiers)
    }

    /// Right-click without warping the cursor.
    pub fn right_click_no_warp(&self, x: f64, y: f64) -> Result<&'static str, AutomationError> {
        self.engine.right_click_no_warp(x, y)
    }

    /// Double-click without warping the cursor.
    pub fn double_click_no_warp(&self, x: f64, y: f64, modifiers: Option<&str>) -> Result<&'static str, AutomationError> {
        self.engine.double_click_no_warp(x, y, modifiers)
    }

    /// Simulate a right click (down + up) at specified coordinates.
    pub fn right_click(&self, x: f64, y: f64, modifiers: Option<&str>) -> Result<(), AutomationError> {
        self.engine.right_click(x, y, modifiers)
    }

    /// Simulate a middle click (down + up) at specified coordinates.
    pub fn middle_click(&self, x: f64, y: f64, modifiers: Option<&str>) -> Result<(), AutomationError> {
        self.engine.middle_click(x, y, modifiers)
    }

    /// Simulate a double left click at the specified coordinates.
    pub fn double_click(&self, x: f64, y: f64, modifiers: Option<&str>) -> Result<(), AutomationError> {
        self.engine.double_click(x, y, modifiers)
    }

    /// Simulate a triple left click at the specified coordinates.
    pub fn triple_click(&self, x: f64, y: f64, modifiers: Option<&str>) -> Result<(), AutomationError> {
        self.engine.triple_click(x, y, modifiers)
    }
}

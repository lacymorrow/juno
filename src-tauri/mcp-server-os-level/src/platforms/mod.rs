use crate::element::UIElement;
use crate::input_tier::InputOutcome;
use crate::{AutomationError, ElementTreeNode, Selector};
use anyhow::Result;
use serde_json::Value as JsonValue;
use std::any::Any;

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
    fn hold_key(&self, key: &str, duration_ms: Option<u64>) -> Result<(), AutomationError>;

    /// Release a modifier key
    fn release_key(&self, key: &str) -> Result<(), AutomationError>;

    /// Wait for a specified duration
    fn wait(&self, duration_ms: u64) -> Result<(), AutomationError>;

    /// Press a single key with an optional modifier
    fn press_key(&self, key_name: &str, modifier: Option<&str>) -> Result<(), AutomationError>;

    /// Get the UI tree starting from a specific app or the focused one
    fn get_ui_tree(&self, app_name: Option<&str>) -> Result<JsonValue, AutomationError>;

    /// Returns the accessibility element at the given screen coordinates, if any.
    /// Uses platform-native hit-testing (e.g. AXUIElementCopyElementAtPosition on macOS).
    /// Default returns None — platforms that support AX hit-testing override this.
    fn element_at_position(&self, _x: f64, _y: f64) -> Option<UIElement> {
        None
    }

    /// Get the current mouse cursor position.
    fn cursor_position(&self) -> Result<(f64, f64), AutomationError>;

    /// Move the mouse cursor to the specified coordinates.
    fn mouse_move(&self, x: f64, y: f64) -> Result<(), AutomationError>;

    /// Simulate pressing the left mouse button down at the specified coordinates.
    fn left_mouse_down(&self, x: f64, y: f64) -> Result<(), AutomationError>;

    /// Simulate releasing the left mouse button at the specified coordinates.
    fn left_mouse_up(&self, x: f64, y: f64) -> Result<(), AutomationError>;

    /// Simulate a standard left click (down + up) at specified coordinates.
    fn left_click(&self, x: f64, y: f64, modifiers: Option<&str>) -> Result<(), AutomationError>;

    /// Click without warping the system cursor — tiered: SkyLight → CGEventPostToPid → HID-restore.
    ///
    /// `allow_physical` gates the last tier, the one that moves the pointer the
    /// user is holding. With it false, `Ok(None)` means "this step can only be
    /// done with the physical cursor", which is the caller's cue to ask.
    /// Default impl falls back to `left_click` for platforms that don't support process-targeted events.
    fn left_click_no_warp(
        &self,
        x: f64,
        y: f64,
        modifiers: Option<&str>,
        allow_physical: bool,
    ) -> Result<Option<InputOutcome>, AutomationError> {
        if !allow_physical {
            return Ok(None);
        }
        self.left_click(x, y, modifiers)?;
        Ok(Some(InputOutcome::physical_cursor("HID-default")))
    }

    /// Right-click without warping the cursor. Default falls back to `right_click`.
    fn right_click_no_warp(
        &self,
        x: f64,
        y: f64,
        allow_physical: bool,
    ) -> Result<Option<InputOutcome>, AutomationError> {
        if !allow_physical {
            return Ok(None);
        }
        self.right_click(x, y, None)?;
        Ok(Some(InputOutcome::physical_cursor("HID-default")))
    }

    /// Middle-click without warping the cursor. Default falls back to `middle_click`.
    fn middle_click_no_warp(
        &self,
        x: f64,
        y: f64,
        modifiers: Option<&str>,
        allow_physical: bool,
    ) -> Result<Option<InputOutcome>, AutomationError> {
        if !allow_physical {
            return Ok(None);
        }
        self.middle_click(x, y, modifiers)?;
        Ok(Some(InputOutcome::physical_cursor("HID-default")))
    }

    /// Double-click without warping the cursor. Default falls back to `double_click`.
    fn double_click_no_warp(
        &self,
        x: f64,
        y: f64,
        modifiers: Option<&str>,
        allow_physical: bool,
    ) -> Result<Option<InputOutcome>, AutomationError> {
        if !allow_physical {
            return Ok(None);
        }
        self.double_click(x, y, modifiers)?;
        Ok(Some(InputOutcome::physical_cursor("HID-default")))
    }

    /// Triple-click without warping the cursor. Default falls back to `triple_click`.
    fn triple_click_no_warp(
        &self,
        x: f64,
        y: f64,
        modifiers: Option<&str>,
        allow_physical: bool,
    ) -> Result<Option<InputOutcome>, AutomationError> {
        if !allow_physical {
            return Ok(None);
        }
        self.triple_click(x, y, modifiers)?;
        Ok(Some(InputOutcome::physical_cursor("HID-default")))
    }

    /// Press the left button without warping the cursor.
    fn left_mouse_down_no_warp(
        &self,
        x: f64,
        y: f64,
        allow_physical: bool,
    ) -> Result<Option<InputOutcome>, AutomationError> {
        if !allow_physical {
            return Ok(None);
        }
        self.left_mouse_down(x, y)?;
        Ok(Some(InputOutcome::physical_cursor("HID-default")))
    }

    /// Release the left button without warping the cursor.
    fn left_mouse_up_no_warp(
        &self,
        x: f64,
        y: f64,
        allow_physical: bool,
    ) -> Result<Option<InputOutcome>, AutomationError> {
        if !allow_physical {
            return Ok(None);
        }
        self.left_mouse_up(x, y)?;
        Ok(Some(InputOutcome::physical_cursor("HID-default")))
    }

    /// Drag without warping the cursor. Default falls back to `left_click_drag`.
    fn left_click_drag_no_warp(
        &self,
        start_x: f64,
        start_y: f64,
        end_x: f64,
        end_y: f64,
        allow_physical: bool,
    ) -> Result<Option<InputOutcome>, AutomationError> {
        if !allow_physical {
            return Ok(None);
        }
        self.left_click_drag(start_x, start_y, end_x, end_y)?;
        Ok(Some(InputOutcome::physical_cursor("HID-default")))
    }

    /// Scroll at a point without moving the real cursor there first.
    fn scroll_no_warp(
        &self,
        x: f64,
        y: f64,
        direction: &str,
        amount: f64,
        modifiers: Option<&str>,
        allow_physical: bool,
    ) -> Result<Option<InputOutcome>, AutomationError> {
        let _ = modifiers;
        if !allow_physical {
            return Ok(None);
        }
        self.scroll_at_position(x, y, direction, amount)?;
        Ok(Some(InputOutcome::physical_cursor("HID-default")))
    }

    /// Press a key against the process the agent is working on, without changing
    /// which application is frontmost.
    fn press_key_no_warp(
        &self,
        key_name: &str,
        modifier: Option<&str>,
        allow_physical: bool,
    ) -> Result<Option<InputOutcome>, AutomationError> {
        if !allow_physical {
            return Ok(None);
        }
        self.press_key(key_name, modifier)?;
        Ok(Some(InputOutcome::physical_cursor("HID-default")))
    }

    /// Hold a key against the background target.
    fn hold_key_no_warp(
        &self,
        key: &str,
        duration_ms: Option<u64>,
        allow_physical: bool,
    ) -> Result<Option<InputOutcome>, AutomationError> {
        if !allow_physical {
            return Ok(None);
        }
        self.hold_key(key, duration_ms)?;
        Ok(Some(InputOutcome::physical_cursor("HID-default")))
    }

    /// Type text into the process the agent is working on, rather than into
    /// whatever application happens to be frontmost.
    fn type_text_no_warp(
        &self,
        text: &str,
        allow_physical: bool,
    ) -> Result<Option<InputOutcome>, AutomationError> {
        if !allow_physical {
            return Ok(None);
        }
        self.type_text(text)?;
        Ok(Some(InputOutcome::physical_cursor("HID-default")))
    }

    /// Release a key against the background target.
    fn release_key_no_warp(
        &self,
        key: &str,
        allow_physical: bool,
    ) -> Result<Option<InputOutcome>, AutomationError> {
        if !allow_physical {
            return Ok(None);
        }
        self.release_key(key)?;
        Ok(Some(InputOutcome::physical_cursor("HID-default")))
    }

    /// Post a mouse event directly to a process by PID without moving the cursor.
    /// Default is a no-op (only meaningful on macOS).
    fn post_mouse_event_to_pid(
        &self,
        _pid: i32,
        _event_type_str: &str,
        _x: f64,
        _y: f64,
    ) -> Result<(), AutomationError> {
        Ok(())
    }

    /// Post a key event directly to a process by PID without affecting focus.
    fn post_key_event_to_pid(
        &self,
        _pid: i32,
        _keycode: u16,
        _key_down: bool,
    ) -> Result<(), AutomationError> {
        Ok(())
    }

    /// Simulate a right click (down + up) at specified coordinates.
    fn right_click(&self, x: f64, y: f64, modifiers: Option<&str>) -> Result<(), AutomationError>;

    /// Simulate a middle click (down + up) at specified coordinates.
    fn middle_click(&self, x: f64, y: f64, modifiers: Option<&str>) -> Result<(), AutomationError>;

    /// Simulate a double left click at the specified coordinates.
    fn double_click(&self, x: f64, y: f64, modifiers: Option<&str>) -> Result<(), AutomationError>;

    /// Simulate a triple left click at the specified coordinates.
    fn triple_click(&self, x: f64, y: f64, modifiers: Option<&str>) -> Result<(), AutomationError>;

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
        Ok(Box::new(macos::MacOSEngine::new(
            use_background_apps,
            activate_app,
        )?))
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

/// Create the appropriate engine for the current platform with auto-redirect permission handling
pub fn create_engine_with_auto_redirect(
    use_background_apps: bool,
    activate_app: bool,
    auto_open_settings: bool,
) -> Result<Box<dyn AccessibilityEngine>, AutomationError> {
    #[cfg(target_os = "macos")]
    {
        Ok(Box::new(macos::MacOSEngine::new_with_auto_redirect(
            use_background_apps,
            activate_app,
            auto_open_settings,
        )?))
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

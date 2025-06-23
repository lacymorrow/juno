use computer_use_ai_sdk::Desktop;
use std::sync::Arc;

#[derive(Clone)]
pub struct DesktopWrapper {
    desktop: Option<Arc<Desktop>>,
}

impl DesktopWrapper {
    pub fn new(desktop: Option<Arc<Desktop>>) -> Self {
        Self { desktop }
    }

    pub fn applications(&self) -> Result<Vec<computer_use_ai_sdk::UIElement>, String> {
        match &self.desktop {
            Some(desktop) => desktop.applications().map_err(|e| e.to_string()),
            None => Err("Desktop automation is not available. Please grant accessibility permissions and restart the app.".to_string()),
        }
    }

    pub fn focused_element(&self) -> Result<computer_use_ai_sdk::UIElement, String> {
        match &self.desktop {
            Some(desktop) => desktop.focused_element().map_err(|e| e.to_string()),
            None => Err("Desktop automation is not available. Please grant accessibility permissions and restart the app.".to_string()),
        }
    }

    pub fn locator(&self, selector: impl Into<computer_use_ai_sdk::Selector>) -> Result<computer_use_ai_sdk::Locator, String> {
        match &self.desktop {
            Some(desktop) => Ok(desktop.locator(selector)),
            None => Err("Desktop automation is not available. Please grant accessibility permissions and restart the app.".to_string()),
        }
    }

    pub fn open_application(&self, app_name: &str) -> Result<computer_use_ai_sdk::UIElement, String> {
        match &self.desktop {
            Some(desktop) => desktop.open_application(app_name).map_err(|e| e.to_string()),
            None => Err("Desktop automation is not available. Please grant accessibility permissions and restart the app.".to_string()),
        }
    }

    pub fn open_url(&self, url: &str, browser: Option<&str>) -> Result<computer_use_ai_sdk::UIElement, String> {
        match &self.desktop {
            Some(desktop) => desktop.open_url(url, browser).map_err(|e| e.to_string()),
            None => Err("Desktop automation is not available. Please grant accessibility permissions and restart the app.".to_string()),
        }
    }

    pub fn type_text(&self, text: &str) -> Result<(), String> {
        match &self.desktop {
            Some(desktop) => desktop.type_text(text).map_err(|e| e.to_string()),
            None => Err("Desktop automation is not available. Please grant accessibility permissions and restart the app.".to_string()),
        }
    }

    pub fn press_key(&self, key_name: &str, modifier: Option<&str>) -> Result<(), String> {
        match &self.desktop {
            Some(desktop) => desktop.press_key(key_name, modifier).map_err(|e| e.to_string()),
            None => Err("Desktop automation is not available. Please grant accessibility permissions and restart the app.".to_string()),
        }
    }

    pub fn wait(&self, duration_ms: u64) -> Result<(), String> {
        match &self.desktop {
            Some(desktop) => desktop.wait(duration_ms).map_err(|e| e.to_string()),
            None => Err("Desktop automation is not available. Please grant accessibility permissions and restart the app.".to_string()),
        }
    }

    pub fn get_clipboard_content(&self) -> Result<String, String> {
        match &self.desktop {
            Some(desktop) => desktop.get_clipboard_content().map_err(|e| e.to_string()),
            None => Err("Desktop automation is not available. Please grant accessibility permissions and restart the app.".to_string()),
        }
    }

    pub fn set_clipboard_content(&self, content: &str) -> Result<(), String> {
        match &self.desktop {
            Some(desktop) => desktop.set_clipboard_content(content).map_err(|e| e.to_string()),
            None => Err("Desktop automation is not available. Please grant accessibility permissions and restart the app.".to_string()),
        }
    }

    pub fn hold_key(&self, key: &str, duration_ms: Option<u64>) -> Result<(), String> {
        match &self.desktop {
            Some(desktop) => desktop.hold_key(key, duration_ms).map_err(|e| e.to_string()),
            None => Err("Desktop automation is not available. Please grant accessibility permissions and restart the app.".to_string()),
        }
    }

    pub fn release_key(&self, key: &str) -> Result<(), String> {
        match &self.desktop {
            Some(desktop) => desktop.release_key(key).map_err(|e| e.to_string()),
            None => Err("Desktop automation is not available. Please grant accessibility permissions and restart the app.".to_string()),
        }
    }

    pub fn mouse_move(&self, x: f64, y: f64) -> Result<(), String> {
        match &self.desktop {
            Some(desktop) => desktop.mouse_move(x, y).map_err(|e| e.to_string()),
            None => Err("Desktop automation is not available. Please grant accessibility permissions and restart the app.".to_string()),
        }
    }

    pub fn left_click(&self, x: f64, y: f64, modifiers: Option<&str>) -> Result<(), String> {
        match &self.desktop {
            Some(desktop) => desktop.left_click(x, y, modifiers).map_err(|e| e.to_string()),
            None => Err("Desktop automation is not available. Please grant accessibility permissions and restart the app.".to_string()),
        }
    }

    pub fn right_click(&self, x: f64, y: f64, modifiers: Option<&str>) -> Result<(), String> {
        match &self.desktop {
            Some(desktop) => desktop.right_click(x, y, modifiers).map_err(|e| e.to_string()),
            None => Err("Desktop automation is not available. Please grant accessibility permissions and restart the app.".to_string()),
        }
    }

    pub fn middle_click(&self, x: f64, y: f64, modifiers: Option<&str>) -> Result<(), String> {
        match &self.desktop {
            Some(desktop) => desktop.middle_click(x, y, modifiers).map_err(|e| e.to_string()),
            None => Err("Desktop automation is not available. Please grant accessibility permissions and restart the app.".to_string()),
        }
    }

    pub fn double_click(&self, x: f64, y: f64, modifiers: Option<&str>) -> Result<(), String> {
        match &self.desktop {
            Some(desktop) => desktop.double_click(x, y, modifiers).map_err(|e| e.to_string()),
            None => Err("Desktop automation is not available. Please grant accessibility permissions and restart the app.".to_string()),
        }
    }

    pub fn triple_click(&self, x: f64, y: f64, modifiers: Option<&str>) -> Result<(), String> {
        match &self.desktop {
            Some(desktop) => desktop.triple_click(x, y, modifiers).map_err(|e| e.to_string()),
            None => Err("Desktop automation is not available. Please grant accessibility permissions and restart the app.".to_string()),
        }
    }

    pub fn left_mouse_down(&self, x: f64, y: f64) -> Result<(), String> {
        match &self.desktop {
            Some(desktop) => desktop.left_mouse_down(x, y).map_err(|e| e.to_string()),
            None => Err("Desktop automation is not available. Please grant accessibility permissions and restart the app.".to_string()),
        }
    }

    pub fn left_mouse_up(&self, x: f64, y: f64) -> Result<(), String> {
        match &self.desktop {
            Some(desktop) => desktop.left_mouse_up(x, y).map_err(|e| e.to_string()),
            None => Err("Desktop automation is not available. Please grant accessibility permissions and restart the app.".to_string()),
        }
    }

    pub fn left_click_drag(&self, start_x: f64, start_y: f64, end_x: f64, end_y: f64) -> Result<(), String> {
        match &self.desktop {
            Some(desktop) => desktop.left_click_drag(start_x, start_y, end_x, end_y).map_err(|e| e.to_string()),
            None => Err("Desktop automation is not available. Please grant accessibility permissions and restart the app.".to_string()),
        }
    }

    pub fn cursor_position(&self) -> Result<(f64, f64), String> {
        match &self.desktop {
            Some(desktop) => desktop.cursor_position().map_err(|e| e.to_string()),
            None => Err("Desktop automation is not available. Please grant accessibility permissions and restart the app.".to_string()),
        }
    }

    pub fn scroll_at_position(&self, x: f64, y: f64, direction: &str, amount: f64) -> Result<(), String> {
        match &self.desktop {
            Some(desktop) => desktop.scroll_at_position(x, y, direction, amount).map_err(|e| e.to_string()),
            None => Err("Desktop automation is not available. Please grant accessibility permissions and restart the app.".to_string()),
        }
    }

    pub fn scroll_at_current_position(&self, direction: &str, amount: f64) -> Result<(), String> {
        match &self.desktop {
            Some(desktop) => desktop.scroll_at_current_position(direction, amount).map_err(|e| e.to_string()),
            None => Err("Desktop automation is not available. Please grant accessibility permissions and restart the app.".to_string()),
        }
    }

    pub fn list_windows(&self) -> Result<Vec<computer_use_ai_sdk::UIElement>, String> {
        match &self.desktop {
            Some(desktop) => desktop.list_windows().map_err(|e| e.to_string()),
            None => Err("Desktop automation is not available. Please grant accessibility permissions and restart the app.".to_string()),
        }
    }

    pub fn is_available(&self) -> bool {
        self.desktop.is_some()
    }

    // Helper methods for accessing the inner Desktop instance
    pub fn get_desktop(&self) -> Result<&Arc<Desktop>, String> {
        match &self.desktop {
            Some(desktop) => Ok(desktop),
            None => Err("Desktop automation is not available. Please grant accessibility permissions and restart the app.".to_string()),
        }
    }

    pub fn try_get_desktop(&self) -> Option<&Arc<Desktop>> {
        self.desktop.as_ref()
    }

    pub fn window_relative_click(
        &self,
        window_id: &str,
        x: f64,
        y: f64,
        click_type: Option<&str>,
        modifier: Option<&str>,
    ) -> Result<(), String> {
        match &self.desktop {
            Some(desktop) => {
                // Find the window by ID
                let windows = desktop.list_windows().map_err(|e| format!("Failed to list windows: {}", e))?;

                let target_window = windows
                    .into_iter()
                    .find(|window| {
                        window.id().map_or(false, |id| id == window_id)
                    })
                    .ok_or_else(|| format!("Window with ID '{}' not found", window_id))?;

                // Convert window-relative coordinates to global coordinates
                let (window_x, window_y, _width, _height) = target_window.bounds()
                    .map_err(|e| format!("Failed to get window bounds: {}", e))?;
                let global_x = window_x + x;
                let global_y = window_y + y;

                // Perform the click using existing functionality
                match click_type.unwrap_or("left") {
                    "left" => self.left_click(global_x, global_y, modifier),
                    "right" => self.right_click(global_x, global_y, modifier),
                    "double" => self.double_click(global_x, global_y, modifier),
                    "middle" => self.middle_click(global_x, global_y, modifier),
                    "triple" => self.triple_click(global_x, global_y, modifier),
                    unknown => Err(format!("Unsupported click type: {}", unknown)),
                }
            }
            None => Err("Desktop automation is not available. Please grant accessibility permissions and restart the app.".to_string()),
        }
    }

    pub fn focused_window_relative_click(
        &self,
        x: f64,
        y: f64,
        click_type: Option<&str>,
        modifier: Option<&str>,
    ) -> Result<(), String> {
        match &self.desktop {
            Some(desktop) => {
                // Get the focused element first
                let focused_element = desktop.focused_element()
                    .map_err(|e| format!("Failed to get focused element: {}", e))?;

                // Check if the focused element is a window, if not try to get its window
                let window_element = {
                    let attrs = focused_element.attributes();
                    if attrs.role == "AXWindow" {
                        focused_element
                    } else {
                        // Try to traverse up to find the window
                        let mut current = focused_element;
                        loop {
                            match current.parent() {
                                Ok(Some(parent)) => {
                                    let parent_attrs = parent.attributes();
                                    if parent_attrs.role == "AXWindow" {
                                        current = parent;
                                        break;
                                    }
                                    current = parent;
                                }
                                Ok(None) => {
                                    return Err("No window found in element hierarchy".to_string());
                                }
                                Err(e) => {
                                    return Err(format!("Error traversing element hierarchy: {}", e));
                                }
                            }
                        }
                        current
                    }
                };

                // Convert window-relative coordinates to global coordinates
                let (window_x, window_y, _width, _height) = window_element.bounds()
                    .map_err(|e| format!("Failed to get window bounds: {}", e))?;
                let global_x = window_x + x;
                let global_y = window_y + y;

                // Perform the click using existing functionality
                match click_type.unwrap_or("left") {
                    "left" => self.left_click(global_x, global_y, modifier),
                    "right" => self.right_click(global_x, global_y, modifier),
                    "double" => self.double_click(global_x, global_y, modifier),
                    "middle" => self.middle_click(global_x, global_y, modifier),
                    "triple" => self.triple_click(global_x, global_y, modifier),
                    unknown => Err(format!("Unsupported click type: {}", unknown)),
                }
            }
            None => Err("Desktop automation is not available. Please grant accessibility permissions and restart the app.".to_string()),
        }
    }
}

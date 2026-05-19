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

    pub fn element_at_position(&self, x: f64, y: f64) -> Option<computer_use_ai_sdk::UIElement> {
        self.desktop.as_ref().and_then(|d| d.element_at_position(x, y))
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

    /// Click without warping the system cursor — tiered: SkyLight → CGEventPostToPid → HID-restore.
    pub fn left_click_no_warp(&self, x: f64, y: f64, modifiers: Option<&str>) -> Result<&'static str, String> {
        match &self.desktop {
            Some(desktop) => desktop.left_click_no_warp(x, y, modifiers).map_err(|e| e.to_string()),
            None => Err("Desktop automation is not available. Please grant accessibility permissions and restart the app.".to_string()),
        }
    }

    /// Right-click without warping the cursor.
    pub fn right_click_no_warp(&self, x: f64, y: f64) -> Result<&'static str, String> {
        match &self.desktop {
            Some(desktop) => desktop.right_click_no_warp(x, y).map_err(|e| e.to_string()),
            None => Err("Desktop automation is not available. Please grant accessibility permissions and restart the app.".to_string()),
        }
    }

    /// Double-click without warping the cursor.
    pub fn double_click_no_warp(&self, x: f64, y: f64, modifiers: Option<&str>) -> Result<&'static str, String> {
        match &self.desktop {
            Some(desktop) => desktop.double_click_no_warp(x, y, modifiers).map_err(|e| e.to_string()),
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
}

use computer_use_ai_sdk::{Desktop, InputOutcome};
use std::sync::Arc;

/// Shown whenever the accessibility engine never came up. Kept in one place so
/// the guidance the user reads is identical from every entry point.
const DESKTOP_UNAVAILABLE: &str = "Desktop automation is not available. Please grant accessibility permissions and restart the app.";

#[derive(Clone)]
pub struct DesktopWrapper {
    desktop: Option<Arc<Desktop>>,
}

impl DesktopWrapper {
    pub fn new(desktop: Option<Arc<Desktop>>) -> Self {
        Self { desktop }
    }

    pub fn element_at_position(&self, x: f64, y: f64) -> Option<computer_use_ai_sdk::UIElement> {
        self.desktop
            .as_ref()
            .and_then(|d| d.element_at_position(x, y))
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

    pub fn locator(
        &self,
        selector: impl Into<computer_use_ai_sdk::Selector>,
    ) -> Result<computer_use_ai_sdk::Locator, String> {
        match &self.desktop {
            Some(desktop) => Ok(desktop.locator(selector)),
            None => Err("Desktop automation is not available. Please grant accessibility permissions and restart the app.".to_string()),
        }
    }

    pub fn open_application(
        &self,
        app_name: &str,
    ) -> Result<computer_use_ai_sdk::UIElement, String> {
        match &self.desktop {
            Some(desktop) => desktop.open_application(app_name).map_err(|e| e.to_string()),
            None => Err("Desktop automation is not available. Please grant accessibility permissions and restart the app.".to_string()),
        }
    }

    pub fn open_url(
        &self,
        url: &str,
        browser: Option<&str>,
    ) -> Result<computer_use_ai_sdk::UIElement, String> {
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

    /// Sleep for `duration_ms` milliseconds without blocking the async runtime.
    ///
    /// Deliberately not a call through to `Desktop::wait`: that one is a
    /// `std::thread::sleep`, so every caller inside an async context would park
    /// a tokio worker thread for the whole duration. The availability check is
    /// kept so this behaves like every other wrapper method.
    pub async fn wait(&self, duration_ms: u64) -> Result<(), String> {
        if self.desktop.is_none() {
            return Err(DESKTOP_UNAVAILABLE.to_string());
        }
        tokio::time::sleep(std::time::Duration::from_millis(duration_ms)).await;
        Ok(())
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
    ///
    /// Every `*_no_warp` method here returns `Ok(None)` when the step can only be
    /// done by driving the physical cursor and `allow_physical` said not to.
    pub fn left_click_no_warp(
        &self,
        x: f64,
        y: f64,
        modifiers: Option<&str>,
        allow_physical: bool,
    ) -> Result<Option<InputOutcome>, String> {
        match &self.desktop {
            Some(desktop) => desktop
                .left_click_no_warp(x, y, modifiers, allow_physical)
                .map_err(|e| e.to_string()),
            None => Err(DESKTOP_UNAVAILABLE.to_string()),
        }
    }

    /// Right-click without warping the cursor.
    pub fn right_click_no_warp(
        &self,
        x: f64,
        y: f64,
        allow_physical: bool,
    ) -> Result<Option<InputOutcome>, String> {
        match &self.desktop {
            Some(desktop) => desktop
                .right_click_no_warp(x, y, allow_physical)
                .map_err(|e| e.to_string()),
            None => Err(DESKTOP_UNAVAILABLE.to_string()),
        }
    }

    /// Middle-click without warping the cursor.
    pub fn middle_click_no_warp(
        &self,
        x: f64,
        y: f64,
        modifiers: Option<&str>,
        allow_physical: bool,
    ) -> Result<Option<InputOutcome>, String> {
        match &self.desktop {
            Some(desktop) => desktop
                .middle_click_no_warp(x, y, modifiers, allow_physical)
                .map_err(|e| e.to_string()),
            None => Err(DESKTOP_UNAVAILABLE.to_string()),
        }
    }

    /// Double-click without warping the cursor.
    pub fn double_click_no_warp(
        &self,
        x: f64,
        y: f64,
        modifiers: Option<&str>,
        allow_physical: bool,
    ) -> Result<Option<InputOutcome>, String> {
        match &self.desktop {
            Some(desktop) => desktop
                .double_click_no_warp(x, y, modifiers, allow_physical)
                .map_err(|e| e.to_string()),
            None => Err(DESKTOP_UNAVAILABLE.to_string()),
        }
    }

    /// Triple-click without warping the cursor.
    pub fn triple_click_no_warp(
        &self,
        x: f64,
        y: f64,
        modifiers: Option<&str>,
        allow_physical: bool,
    ) -> Result<Option<InputOutcome>, String> {
        match &self.desktop {
            Some(desktop) => desktop
                .triple_click_no_warp(x, y, modifiers, allow_physical)
                .map_err(|e| e.to_string()),
            None => Err(DESKTOP_UNAVAILABLE.to_string()),
        }
    }

    /// Press the left button without warping the cursor.
    pub fn left_mouse_down_no_warp(
        &self,
        x: f64,
        y: f64,
        allow_physical: bool,
    ) -> Result<Option<InputOutcome>, String> {
        match &self.desktop {
            Some(desktop) => desktop
                .left_mouse_down_no_warp(x, y, allow_physical)
                .map_err(|e| e.to_string()),
            None => Err(DESKTOP_UNAVAILABLE.to_string()),
        }
    }

    /// Release the left button without warping the cursor.
    pub fn left_mouse_up_no_warp(
        &self,
        x: f64,
        y: f64,
        allow_physical: bool,
    ) -> Result<Option<InputOutcome>, String> {
        match &self.desktop {
            Some(desktop) => desktop
                .left_mouse_up_no_warp(x, y, allow_physical)
                .map_err(|e| e.to_string()),
            None => Err(DESKTOP_UNAVAILABLE.to_string()),
        }
    }

    /// Drag without warping the cursor.
    pub fn left_click_drag_no_warp(
        &self,
        start_x: f64,
        start_y: f64,
        end_x: f64,
        end_y: f64,
        allow_physical: bool,
    ) -> Result<Option<InputOutcome>, String> {
        match &self.desktop {
            Some(desktop) => desktop
                .left_click_drag_no_warp(start_x, start_y, end_x, end_y, allow_physical)
                .map_err(|e| e.to_string()),
            None => Err(DESKTOP_UNAVAILABLE.to_string()),
        }
    }

    /// Scroll at a point without moving the real cursor there first.
    pub fn scroll_no_warp(
        &self,
        x: f64,
        y: f64,
        direction: &str,
        amount: f64,
        modifiers: Option<&str>,
        allow_physical: bool,
    ) -> Result<Option<InputOutcome>, String> {
        match &self.desktop {
            Some(desktop) => desktop
                .scroll_no_warp(x, y, direction, amount, modifiers, allow_physical)
                .map_err(|e| e.to_string()),
            None => Err(DESKTOP_UNAVAILABLE.to_string()),
        }
    }

    /// Press a key against the process the agent is working on.
    pub fn press_key_no_warp(
        &self,
        key_name: &str,
        modifier: Option<&str>,
        allow_physical: bool,
    ) -> Result<Option<InputOutcome>, String> {
        match &self.desktop {
            Some(desktop) => desktop
                .press_key_no_warp(key_name, modifier, allow_physical)
                .map_err(|e| e.to_string()),
            None => Err(DESKTOP_UNAVAILABLE.to_string()),
        }
    }

    /// Hold a key against the background target.
    pub fn hold_key_no_warp(
        &self,
        key: &str,
        duration_ms: Option<u64>,
        allow_physical: bool,
    ) -> Result<Option<InputOutcome>, String> {
        match &self.desktop {
            Some(desktop) => desktop
                .hold_key_no_warp(key, duration_ms, allow_physical)
                .map_err(|e| e.to_string()),
            None => Err(DESKTOP_UNAVAILABLE.to_string()),
        }
    }

    /// Type text into the process the agent is working on.
    pub fn type_text_no_warp(
        &self,
        text: &str,
        allow_physical: bool,
    ) -> Result<Option<InputOutcome>, String> {
        match &self.desktop {
            Some(desktop) => desktop
                .type_text_no_warp(text, allow_physical)
                .map_err(|e| e.to_string()),
            None => Err(DESKTOP_UNAVAILABLE.to_string()),
        }
    }

    /// Release a key against the background target.
    pub fn release_key_no_warp(
        &self,
        key: &str,
        allow_physical: bool,
    ) -> Result<Option<InputOutcome>, String> {
        match &self.desktop {
            Some(desktop) => desktop
                .release_key_no_warp(key, allow_physical)
                .map_err(|e| e.to_string()),
            None => Err(DESKTOP_UNAVAILABLE.to_string()),
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

    pub fn left_click_drag(
        &self,
        start_x: f64,
        start_y: f64,
        end_x: f64,
        end_y: f64,
    ) -> Result<(), String> {
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

    pub fn scroll_at_position(
        &self,
        x: f64,
        y: f64,
        direction: &str,
        amount: f64,
    ) -> Result<(), String> {
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

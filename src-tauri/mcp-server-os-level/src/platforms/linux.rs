use crate::element::{UIElement, UIElementImpl, UIElementAttributes};
use crate::platforms::AccessibilityEngine;
use crate::{AutomationError, Selector, ClickResult, Locator, ElementTreeNode};
use std::fmt::Debug;
use std::any::Any;
use serde_json::Value as JsonValue;

pub struct LinuxEngine;

impl LinuxEngine {
    pub fn new(_use_background_apps: bool, _activate_app: bool) -> Result<Self, AutomationError> {
        Err(AutomationError::UnsupportedPlatform(
            "Linux implementation is not yet available".to_string(),
        ))
    }
}

impl AccessibilityEngine for LinuxEngine {
    fn get_root_element(&self) -> UIElement {
        panic!("Linux implementation is not yet available")
    }

    fn get_focused_element(&self) -> Result<UIElement, AutomationError> {
        Err(AutomationError::UnsupportedPlatform(
            "Linux implementation is not yet available".to_string(),
        ))
    }

    fn get_applications(&self) -> Result<Vec<UIElement>, AutomationError> {
        Err(AutomationError::UnsupportedPlatform(
            "Linux implementation is not yet available".to_string(),
        ))
    }

    fn get_application_by_name(&self, _name: &str) -> Result<UIElement, AutomationError> {
        Err(AutomationError::UnsupportedPlatform(
            "Linux implementation is not yet available".to_string(),
        ))
    }

    fn find_element(
        &self,
        _selector: &Selector,
        _root: Option<&UIElement>,
    ) -> Result<UIElement, AutomationError> {
        Err(AutomationError::UnsupportedPlatform(
            "Linux implementation is not yet available".to_string(),
        ))
    }

    fn find_elements(
        &self,
        _selector: &Selector,
        _root: Option<&UIElement>,
    ) -> Result<Vec<UIElement>, AutomationError> {
        Err(AutomationError::UnsupportedPlatform(
            "Linux implementation is not yet available".to_string(),
        ))
    }

    fn open_application(&self, _app_name: &str) -> Result<UIElement, AutomationError> {
        Err(AutomationError::UnsupportedPlatform(
            "Linux implementation is not yet available".to_string(),
        ))
    }

    fn open_url(&self, _url: &str, _browser: Option<&str>) -> Result<UIElement, AutomationError> {
        Err(AutomationError::UnsupportedPlatform(
            "Linux implementation is not yet available".to_string(),
        ))
    }

    fn as_any(&self) -> &dyn Any {
        self
    }

    fn scroll_at_position(&self, _x: f64, _y: f64, _direction: &str, _amount: f64) -> Result<(), AutomationError> {
        Err(AutomationError::UnsupportedPlatform("Linux implementation is not yet available".to_string()))
    }

    fn scroll_at_current_position(&self, _direction: &str, _amount: f64) -> Result<(), AutomationError> {
        Err(AutomationError::UnsupportedPlatform("Linux implementation is not yet available".to_string()))
    }

    fn type_text(&self, _text: &str) -> Result<(), AutomationError> {
        Err(AutomationError::UnsupportedPlatform("Linux implementation is not yet available".to_string()))
    }

    fn get_clipboard_content(&self) -> Result<String, AutomationError> {
        Err(AutomationError::UnsupportedPlatform("Linux implementation is not yet available".to_string()))
    }

    fn set_clipboard_content(&self, _content: &str) -> Result<(), AutomationError> {
        Err(AutomationError::UnsupportedPlatform("Linux implementation is not yet available".to_string()))
    }

    fn hold_key(&self, _key: &str, _duration_ms: Option<u64>) -> Result<(), AutomationError> {
        Err(AutomationError::UnsupportedPlatform("Linux implementation is not yet available".to_string()))
    }

    fn release_key(&self, _key: &str) -> Result<(), AutomationError> {
        Err(AutomationError::UnsupportedPlatform("Linux implementation is not yet available".to_string()))
    }

    fn wait(&self, _duration_ms: u64) -> Result<(), AutomationError> {
        Err(AutomationError::UnsupportedPlatform("Linux implementation is not yet available".to_string()))
    }

    fn press_key(&self, _key_name: &str, _modifier: Option<&str>) -> Result<(), AutomationError> {
        Err(AutomationError::UnsupportedPlatform("Linux implementation is not yet available".to_string()))
    }

    fn get_ui_tree(&self, _app_name: Option<&str>) -> Result<JsonValue, AutomationError> {
        Err(AutomationError::UnsupportedPlatform("Linux implementation is not yet available".to_string()))
    }

    fn cursor_position(&self) -> Result<(f64, f64), AutomationError> {
        Err(AutomationError::UnsupportedPlatform("Linux implementation is not yet available".to_string()))
    }

    fn mouse_move(&self, _x: f64, _y: f64) -> Result<(), AutomationError> {
        Err(AutomationError::UnsupportedPlatform("Linux implementation is not yet available".to_string()))
    }

    fn left_mouse_down(&self, _x: f64, _y: f64) -> Result<(), AutomationError> {
        Err(AutomationError::UnsupportedPlatform("Linux implementation is not yet available".to_string()))
    }

    fn left_mouse_up(&self, _x: f64, _y: f64) -> Result<(), AutomationError> {
        Err(AutomationError::UnsupportedPlatform("Linux implementation is not yet available".to_string()))
    }

    fn left_click(&self, _x: f64, _y: f64, _modifiers: Option<&str>) -> Result<(), AutomationError> {
        Err(AutomationError::UnsupportedPlatform("Linux implementation is not yet available".to_string()))
    }

    fn right_click(&self, _x: f64, _y: f64, _modifiers: Option<&str>) -> Result<(), AutomationError> {
        Err(AutomationError::UnsupportedPlatform("Linux implementation is not yet available".to_string()))
    }

    fn middle_click(&self, _x: f64, _y: f64, _modifiers: Option<&str>) -> Result<(), AutomationError> {
        Err(AutomationError::UnsupportedPlatform("Linux implementation is not yet available".to_string()))
    }

    fn double_click(&self, _x: f64, _y: f64, _modifiers: Option<&str>) -> Result<(), AutomationError> {
        Err(AutomationError::UnsupportedPlatform("Linux implementation is not yet available".to_string()))
    }

    fn triple_click(&self, _x: f64, _y: f64, _modifiers: Option<&str>) -> Result<(), AutomationError> {
        Err(AutomationError::UnsupportedPlatform("Linux implementation is not yet available".to_string()))
    }

    fn left_click_drag(&self, _start_x: f64, _start_y: f64, _end_x: f64, _end_y: f64) -> Result<(), AutomationError> {
        Err(AutomationError::UnsupportedPlatform("Linux implementation is not yet available".to_string()))
    }

    fn get_window_title(&self) -> Result<String, AutomationError> {
        Err(AutomationError::UnsupportedPlatform("Linux implementation is not yet available".to_string()))
    }

    fn list_windows(&self) -> Result<Vec<UIElement>, AutomationError> {
        Err(AutomationError::UnsupportedPlatform("Linux implementation is not yet available".to_string()))
    }

    fn close_window(&self) -> Result<(), AutomationError> {
        Err(AutomationError::UnsupportedPlatform("Linux implementation is not yet available".to_string()))
    }

    fn maximize_window(&self) -> Result<(), AutomationError> {
        Err(AutomationError::UnsupportedPlatform("Linux implementation is not yet available".to_string()))
    }

    fn minimize_window(&self) -> Result<(), AutomationError> {
        Err(AutomationError::UnsupportedPlatform("Linux implementation is not yet available".to_string()))
    }

    fn resize_window(&self, _width: f64, _height: f64) -> Result<(), AutomationError> {
        Err(AutomationError::UnsupportedPlatform("Linux implementation is not yet available".to_string()))
    }

    fn move_window(&self, _x: f64, _y: f64) -> Result<(), AutomationError> {
        Err(AutomationError::UnsupportedPlatform("Linux implementation is not yet available".to_string()))
    }

    fn get_element_tree(&self, _element: &UIElement) -> Result<ElementTreeNode, AutomationError> {
        Err(AutomationError::UnsupportedPlatform("Linux implementation is not yet available".to_string()))
    }
}

// Placeholder LinuxUIElement that implements UIElementImpl
pub struct LinuxUIElement;

impl Debug for LinuxUIElement {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("LinuxUIElement").finish()
    }
}

impl UIElementImpl for LinuxUIElement {
    fn object_id(&self) -> usize {
        0
    }

    fn id(&self) -> Option<String> {
        None
    }

    fn role(&self) -> String {
        "".to_string()
    }

    fn attributes(&self) -> UIElementAttributes {
        UIElementAttributes {
            role: "".to_string(),
            label: None,
            value: None,
            description: None,
            properties: std::collections::HashMap::new(),
        }
    }

    fn children(&self) -> Result<Vec<UIElement>, AutomationError> {
        Err(AutomationError::UnsupportedPlatform(
            "Linux implementation is not yet available".to_string(),
        ))
    }

    fn parent(&self) -> Result<Option<UIElement>, AutomationError> {
        Err(AutomationError::UnsupportedPlatform(
            "Linux implementation is not yet available".to_string(),
        ))
    }

    fn bounds(&self) -> Result<(f64, f64, f64, f64), AutomationError> {
        Err(AutomationError::UnsupportedPlatform(
            "Linux implementation is not yet available".to_string(),
        ))
    }

    fn click(&self) -> Result<ClickResult, AutomationError> {
        Err(AutomationError::UnsupportedPlatform(
            "Linux implementation is not yet available".to_string(),
        ))
    }

    fn double_click(&self) -> Result<ClickResult, AutomationError> {
        Err(AutomationError::UnsupportedPlatform(
            "Linux implementation is not yet available".to_string(),
        ))
    }

    fn right_click(&self) -> Result<(), AutomationError> {
        Err(AutomationError::UnsupportedPlatform(
            "Linux implementation is not yet available".to_string(),
        ))
    }

    fn hover(&self) -> Result<(), AutomationError> {
        Err(AutomationError::UnsupportedPlatform(
            "Linux implementation is not yet available".to_string(),
        ))
    }

    fn focus(&self) -> Result<(), AutomationError> {
        Err(AutomationError::UnsupportedPlatform(
            "Linux implementation is not yet available".to_string(),
        ))
    }

    fn type_text(&self, _text: &str) -> Result<(), AutomationError> {
        Err(AutomationError::UnsupportedPlatform(
            "Linux implementation is not yet available".to_string(),
        ))
    }

    fn press_key(&self, _key: &str) -> Result<(), AutomationError> {
        Err(AutomationError::UnsupportedPlatform(
            "Linux implementation is not yet available".to_string(),
        ))
    }

    fn get_text(&self, _max_depth: usize) -> Result<String, AutomationError> {
        Err(AutomationError::UnsupportedPlatform(
            "Linux implementation is not yet available".to_string(),
        ))
    }

    fn set_value(&self, _value: &str) -> Result<(), AutomationError> {
        Err(AutomationError::UnsupportedPlatform(
            "Linux implementation is not yet available".to_string(),
        ))
    }

    fn is_enabled(&self) -> Result<bool, AutomationError> {
        Err(AutomationError::UnsupportedPlatform(
            "Linux implementation is not yet available".to_string(),
        ))
    }

    fn is_visible(&self) -> Result<bool, AutomationError> {
        Err(AutomationError::UnsupportedPlatform(
            "Linux implementation is not yet available".to_string(),
        ))
    }

    fn is_focused(&self) -> Result<bool, AutomationError> {
        Err(AutomationError::UnsupportedPlatform(
            "Linux implementation is not yet available".to_string(),
        ))
    }

    fn perform_action(&self, _action: &str) -> Result<(), AutomationError> {
        Err(AutomationError::UnsupportedPlatform(
            "Linux implementation is not yet available".to_string(),
        ))
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn create_locator(&self, _selector: Selector) -> Result<Locator, AutomationError> {
        Err(AutomationError::UnsupportedPlatform(
            "Linux implementation is not yet available".to_string(),
        ))
    }

    fn clone_box(&self) -> Box<dyn UIElementImpl> {
        Box::new(LinuxUIElement)
    }

    fn scroll(&self, _direction: &str, _amount: f64) -> Result<(), AutomationError> {
        Err(AutomationError::UnsupportedPlatform(
            "Linux implementation is not yet available".to_string(),
        ))
    }

    fn screenshot(&self) -> Result<String, AutomationError> {
        Err(AutomationError::UnsupportedPlatform(
            "Linux implementation is not yet available".to_string(),
        ))
    }

    fn select_text(&self) -> Result<(), AutomationError> {
        Err(AutomationError::UnsupportedPlatform(
            "Linux implementation is not yet available".to_string(),
        ))
    }

    fn get_all_attributes(&self) -> Result<UIElementAttributes, AutomationError> {
        Err(AutomationError::UnsupportedPlatform(
            "Linux implementation is not yet available".to_string(),
        ))
    }

    fn get_tree(&self) -> Result<ElementTreeNode, AutomationError> {
        Err(AutomationError::UnsupportedPlatform(
            "Linux implementation is not yet available".to_string(),
        ))
    }
}

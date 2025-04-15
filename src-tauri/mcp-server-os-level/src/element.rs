use crate::errors::AutomationError;
use crate::selector::Selector;
use std::collections::HashMap;
use std::fmt::Debug;
use serde::Serialize;

use super::{ClickResult, Locator};

/// Represents a UI element in a desktop application
#[derive(Debug)]
pub struct UIElement {
    inner: Box<dyn UIElementImpl>,
}

/// Attributes associated with a UI element
#[derive(Debug, Serialize)]
pub struct UIElementAttributes {
    pub role: String,
    pub label: Option<String>,
    pub value: Option<String>,
    pub description: Option<String>,
    pub properties: HashMap<String, Option<serde_json::Value>>,
}

/// Interface for platform-specific element implementations
pub(crate) trait UIElementImpl: Send + Sync + Debug + AsAny {
    fn object_id(&self) -> usize;
    fn id(&self) -> Option<String>;
    fn role(&self) -> String;
    fn attributes(&self) -> UIElementAttributes;
    fn children(&self) -> Result<Vec<UIElement>, AutomationError>;
    fn parent(&self) -> Result<Option<UIElement>, AutomationError>;
    fn bounds(&self) -> Result<(f64, f64, f64, f64), AutomationError>; // x, y, width, height
    fn click(&self) -> Result<ClickResult, AutomationError>;
    fn double_click(&self) -> Result<ClickResult, AutomationError>;
    fn right_click(&self) -> Result<(), AutomationError>;
    fn hover(&self) -> Result<(), AutomationError>;
    fn focus(&self) -> Result<(), AutomationError>;
    fn type_text(&self, text: &str) -> Result<(), AutomationError>;
    fn press_key(&self, key: &str) -> Result<(), AutomationError>;
    fn get_text(&self, max_depth: usize) -> Result<String, AutomationError>;
    fn set_value(&self, value: &str) -> Result<(), AutomationError>;
    fn is_enabled(&self) -> Result<bool, AutomationError>;
    fn is_visible(&self) -> Result<bool, AutomationError>;
    fn is_focused(&self) -> Result<bool, AutomationError>;
    fn perform_action(&self, action: &str) -> Result<(), AutomationError>;
    fn create_locator(&self, selector: Selector) -> Result<Locator, AutomationError>;
    fn scroll(&self, direction: &str, amount: f64) -> Result<(), AutomationError>;
    fn screenshot(&self) -> Result<String, AutomationError>;
    fn select_text(&self) -> Result<(), AutomationError>;
    fn get_all_attributes(&self) -> Result<UIElementAttributes, AutomationError>;

    // Add a method to clone the box
    fn clone_box(&self) -> Box<dyn UIElementImpl>;

    fn as_any(&self) -> &dyn std::any::Any;
}

// Add a public method to access the underlying Any implementation
pub trait AsAny {
    fn as_any(&self) -> &dyn std::any::Any;
}

// Generic implementation for types that implement UIElementImpl
impl<T: UIElementImpl + ?Sized + 'static> AsAny for T {
    fn as_any(&self) -> &dyn std::any::Any {
        // Delegate to the implementation provided by UIElementImpl
        UIElementImpl::as_any(self)
    }
}

impl UIElement {
    /// Create a new UI element from a platform-specific implementation
    pub(crate) fn new(impl_: Box<dyn UIElementImpl>) -> Self {
        Self { inner: impl_ }
    }

    /// Get the element's ID
    pub fn id(&self) -> Option<String> {
        self.inner.id()
    }

    /// Get the element's role (e.g., "button", "textfield")
    pub fn role(&self) -> String {
        self.inner.role()
    }

    /// Get all attributes of the element
    pub fn attributes(&self) -> UIElementAttributes {
        self.inner.attributes()
    }

    /// Get child elements
    pub fn children(&self) -> Result<Vec<UIElement>, AutomationError> {
        self.inner.children()
    }

    /// Get parent element
    pub fn parent(&self) -> Result<Option<UIElement>, AutomationError> {
        self.inner.parent()
    }

    /// Get element bounds (x, y, width, height)
    pub fn bounds(&self) -> Result<(f64, f64, f64, f64), AutomationError> {
        self.inner.bounds()
    }

    /// Click on this element
    pub fn click(&self) -> Result<ClickResult, AutomationError> {
        self.inner.click()
    }

    /// Double-click on this element
    pub fn double_click(&self) -> Result<ClickResult, AutomationError> {
        self.inner.double_click()
    }

    /// Right-click on this element
    pub fn right_click(&self) -> Result<(), AutomationError> {
        self.inner.right_click()
    }

    /// Hover over this element
    pub fn hover(&self) -> Result<(), AutomationError> {
        self.inner.hover()
    }

    /// Focus this element
    pub fn focus(&self) -> Result<(), AutomationError> {
        self.inner.focus()
    }

    /// Type text into this element
    pub fn type_text(&self, text: &str) -> Result<(), AutomationError> {
        self.inner.type_text(text)
    }

    /// Press a key while this element is focused
    pub fn press_key(&self, key: &str) -> Result<(), AutomationError> {
        self.inner.press_key(key)
    }

    /// Get text content of this element
    pub fn text(&self, max_depth: usize) -> Result<String, AutomationError> {
        self.inner.get_text(max_depth)
    }

    /// Set value of this element
    pub fn set_value(&self, value: &str) -> Result<(), AutomationError> {
        self.inner.set_value(value)
    }

    /// Check if element is enabled
    pub fn is_enabled(&self) -> Result<bool, AutomationError> {
        self.inner.is_enabled()
    }

    /// Check if element is visible
    pub fn is_visible(&self) -> Result<bool, AutomationError> {
        self.inner.is_visible()
    }

    /// Check if element is focused
    pub fn is_focused(&self) -> Result<bool, AutomationError> {
        self.inner.is_focused()
    }

    /// Perform a named action on this element
    pub fn perform_action(&self, action: &str) -> Result<(), AutomationError> {
        self.inner.perform_action(action)
    }

    /// Public method to get as Any, using the AsAny trait
    pub fn as_any(&self) -> &dyn std::any::Any {
        AsAny::as_any(self.inner.as_ref()) // Call trait method on the boxed inner value
    }

    /// Find elements matching the selector within this element
    pub fn locator(&self, selector: impl Into<Selector>) -> Result<Locator, AutomationError> {
        let selector = selector.into();
        self.inner.create_locator(selector)
    }

    /// Scroll the element in a given direction
    pub fn scroll(&self, direction: &str, amount: f64) -> Result<(), AutomationError> {
        self.inner.scroll(direction, amount)
    }

    /// Get a screenshot of this element
    pub fn screenshot(&self) -> Result<String, AutomationError> {
        self.inner.screenshot()
    }

    /// Select text within this element
    pub fn select_text(&self) -> Result<(), AutomationError> {
        self.inner.select_text()
    }

    /// Get all attributes of the element
    pub fn get_all_attributes(&self) -> Result<UIElementAttributes, AutomationError> {
        self.inner.get_all_attributes()
    }
}

impl PartialEq for UIElement {
    fn eq(&self, other: &Self) -> bool {
        self.inner.object_id() == other.inner.object_id()
    }
}

impl Eq for UIElement {}

impl std::hash::Hash for UIElement {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.inner.object_id().hash(state);
    }
}

impl Clone for UIElement {
    fn clone(&self) -> Self {
        // We can't directly clone the inner Box<dyn UIElementImpl>,
        // but we can create a new UIElement with the same identity
        // that will behave the same way
        Self {
            inner: self.inner.clone_box(),
        }
    }
}

// Implement AsAny for the UIElement wrapper struct
impl AsAny for UIElement {
    fn as_any(&self) -> &dyn std::any::Any {
         AsAny::as_any(self.inner.as_ref())
    }
}

// Implement AsAny for the boxed trait object as well
impl AsAny for Box<dyn UIElementImpl> {
    fn as_any(&self) -> &dyn std::any::Any {
        // Disambiguate: Call the as_any method from the UIElementImpl trait
        UIElementImpl::as_any(self.as_ref())
    }
}

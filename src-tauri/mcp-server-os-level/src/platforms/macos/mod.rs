pub mod actions;
pub mod constants;
pub mod element;
pub mod engine;
pub mod ffi;
pub mod permissions;
pub mod utils;
pub mod wrappers;

use crate::platforms::AccessibilityEngine;
use crate::element::UIElementImpl;
use crate::{
    AutomationError,
    ClickResult,
    UIElement,
    UIElementAttributes,
    Locator,
    Selector,
};

// Re-export key types publicly
pub use engine::MacOSEngine;
pub use element::MacOSUIElement;

// The rest of the original file content has been moved to the respective modules.

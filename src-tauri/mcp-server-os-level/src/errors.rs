use thiserror::Error;
use serde::{Deserialize, Serialize};

#[derive(Error, Debug, Serialize, Deserialize)]
pub enum AutomationError {
    #[error("Element not found: {0}")]
    ElementNotFound(String),

    #[error("Operation timed out: {0}")]
    Timeout(String),

    #[error("Permission denied: {0}")]
    PermissionDenied(String),

    #[error("Platform-specific error: {0}")]
    PlatformError(String),

    #[error("Unsupported operation: {0}")]
    UnsupportedOperation(String),

    #[error("Unsupported platform: {0}")]
    UnsupportedPlatform(String),

    #[error("Invalid argument: {0}")]
    InvalidArgument(String),

    #[error("Internal error: {0}")]
    Internal(String),

    #[error("No focused element found: {0}")]
    NoFocusedElement(String),

    #[error(
        "Element has zero or negative dimensions and cannot be used for visual operations. Role: '{role}', Label: '{label}', Bounds: ({x}, {y}, {width}, {height})"
    )]
    ZeroElementDimensions {
        role: String,
        label: String,
        x: f64,
        y: f64,
        width: f64,
        height: f64,
    },

    #[error("Initialization failed: {0}")]
    InitializationError(String),

    #[error("Unsupported selector: {0}")]
    UnsupportedSelector(String),

    #[error("Tool not found: {0}")]
    ToolNotFound(String),
}

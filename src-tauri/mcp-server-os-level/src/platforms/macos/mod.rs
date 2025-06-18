pub mod attributes;
pub mod constants;
pub mod display;
pub mod element;
pub mod engine;
pub mod ffi;
pub mod interaction;
pub mod input;
pub mod permissions;
pub mod utils;
pub mod wrappers;

// No platform-level imports needed here after refactoring

// Re-export key types publicly
pub use element::MacOSUIElement;
pub use engine::MacOSEngine;

// The rest of the original file content has been moved to the respective modules.

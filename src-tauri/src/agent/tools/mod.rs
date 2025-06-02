pub mod desktop_tools;

pub mod browser_tools;
pub mod browser_controller;
pub mod basic_tools; // Ensure basic_tools is declared


pub use browser_tools::get_browser_tool_definitions;
pub use browser_controller::BrowserController;
pub use basic_tools::*; // Export functions from basic_tools

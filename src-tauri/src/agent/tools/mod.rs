pub mod desktop_tools;
pub mod browser_tools;
pub mod browser_controller;
pub mod basic_tools; // Ensure basic_tools is declared
pub mod anthropic_computer_use; // Add the new Anthropic Computer Use tools

pub use browser_tools::get_browser_tool_definitions;
pub use browser_controller::BrowserController;
pub use basic_tools::*; // Export functions from basic_tools

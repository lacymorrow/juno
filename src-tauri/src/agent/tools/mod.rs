//! # Agent Tools Module
//!
//! This module contains all the tool implementations for the Juno AI Computer Use Agent.
//!
//! ## Module Structure:
//! - `desktop_tools`: Cross-platform desktop automation tools (screenshot, mouse, keyboard, UI elements)
//! - `browser_tools`: Web browser automation and interaction tools
//! - `browser_controller`: Low-level browser control and management
//! - `basic_tools`: Core system tools (file operations, terminal commands)
//! - `anthropic_computer_use`: Full Anthropic Computer Use API implementation with 17 actions
//! - `timer_tools`: Scheduling and monitoring tools for delayed agent execution
//! - `tool_config`: Tool configuration and category management system
//! - `enhanced_coding_tools`: Advanced development and coding assistance tools
//! - `cursor_integration`: Integration with Cursor IDE for development workflows
//! - `mcp_integration`: Model Context Protocol (MCP) server integration for extensibility
//! - `self_awareness_tools`: Self-building and introspection capabilities (debug mode only)
//! - `tool_mapping`: Tool mapping service for centralized tool categorization
//! - `ui_token_selector`: UI-Guided Visual Token Selection for 33% computational cost reduction
//!
//! ## Usage
//! Tools are registered with the `LocalToolProvider` and made available to AI agents.
//! Each module exports registration functions and relevant types.

pub mod desktop_tools;
pub mod browser_tools;
pub mod browser_controller;
pub mod basic_tools; // Ensure basic_tools is declared
pub mod anthropic_computer_use; // Add the new Anthropic Computer Use tools
pub mod timer_tools; // Add timer tools for agent scheduling
pub mod tool_config; // Add tool configuration management
pub mod enhanced_coding_tools;
pub mod cursor_integration;
pub mod mcp_integration;
pub mod self_awareness_tools; // Self-building and introspection capabilities
pub mod tool_mapping; // Add tool mapping service
pub mod ui_token_selector; // UI-Guided Visual Token Selection system

pub use browser_tools::get_browser_tool_definitions;
pub use browser_controller::BrowserController;
pub use basic_tools::*; // Export functions from basic_tools
pub use timer_tools::{register_timer_tools, TimerManager, TimerTask}; // Export timer functions and types
pub use tool_config::{ToolConfig, ToolConfigManager, ToolCategory}; // Export tool configuration types
pub use tool_mapping::ToolMappingService; // Export centralized tool mapping service
pub use mcp_integration::{MCPManager, MCPServerConfig, MCPServerStatus, MCPToolInfo};
pub use self_awareness_tools::register_self_awareness_tools; // Export self-awareness tool registration

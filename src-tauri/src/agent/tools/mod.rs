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
//! - `display_info_tools`: Screen resolution and display information without screenshots

//! - `tool_mapping`: Tool mapping service for centralized tool categorization
//! - `ui_token_selector`: UI-Guided Visual Token Selection for 33% computational cost reduction
//!
//! ## Usage
//! Tools are registered with the `LocalToolProvider` and made available to AI agents.
//! Each module exports registration functions and relevant types.

pub mod accessibility_tools; // Native macOS accessibility tools for element-level interaction
pub mod anthropic_computer_use; // Add the new Anthropic Computer Use tools
pub mod basic_tools; // Ensure basic_tools is declared
pub mod browser_controller;
pub mod browser_tools;
pub mod coordinator; // TARS Integration: Event-driven tool coordinator
pub mod engines; // TARS Phase 2: Tool call engines for different LLM providers
pub mod event_executor; // TARS Phase 1.7: Event-driven tool executor
pub mod collaborative_ai; // Advanced Collaborative AI System Design from ComfyBench research
pub mod cursor_integration;
pub mod desktop_tools;
pub mod display_info_tools; // Screen resolution and display information tools
pub mod enhanced_coding_tools;
pub mod enhanced_visual_reasoning;
pub mod exploration_reasoning; // Exploration-Then-Reasoning Paradigm from GUI-Xplore research
pub mod mcp_integration;
pub mod safari_tools; // Native Safari DOM automation with AppleScript injection
pub mod self_awareness_tools; // Self-building and introspection capabilities
// pub mod self_improvement; // Research-backed autonomous code generation system - TODO: Fix module not found

pub mod timer_tools; // Add timer tools for agent scheduling
pub mod tool_config; // Configuration and category management for all tools
pub mod tool_mapping; // Add tool mapping service
pub mod tool_versioning; // API versioning and compatibility management
pub mod ui_token_selector; // UI-Guided Visual Token Selection system
pub mod universal_block_parser; // Universal Block Parsing (UBP) system from SpiritSight Agent research // Enhanced Visual Reasoning System from CVPR 2025 research

pub use accessibility_tools::{AccessibilityElement, AccessibilityTools}; // Export accessibility tools
pub use basic_tools::*; // Export functions from basic_tools
pub use browser_controller::BrowserController;
pub use browser_tools::get_browser_tool_definitions;
pub use coordinator::ToolCoordinator; // TARS Integration: Event-driven tool coordinator
pub use engines::{ToolCallEngine, ToolCallEngineType, get_engine_for_provider}; // TARS Phase 2: Tool call engines
pub use event_executor::EventDrivenToolExecutor; // TARS Phase 1.7: Event-driven tool executor
pub use collaborative_ai::{
    CollaborativeAIDesigner, ComplexityLevel, SystemRequirements, WorkflowDesignResult,
}; // Export collaborative AI components
pub use enhanced_visual_reasoning::{
    ReasoningContext, SceneUnderstanding, VisualReasoningEngine, VisualReasoningResult,
};
pub use exploration_reasoning::{ExplorationConfig, ExplorationEngine, ExplorationResult}; // Export exploration-reasoning components
pub use mcp_integration::{MCPManager, MCPServerConfig, MCPServerStatus, MCPToolInfo};
pub use safari_tools::{get_safari_tool_definitions, get_safari_tools, SafariTools}; // Export Safari tools
pub use self_awareness_tools::register_self_awareness_tools; // Export self-awareness tool registration
// pub use self_improvement::*; // Export self-improvement types and functions - TODO: Fix module not found

pub use timer_tools::{register_timer_tools, TimerManager, TimerTask}; // Export timer functions and types
pub use tool_config::{ToolCategory, ToolConfig, ToolConfigManager}; // Export tool configuration types
pub use tool_mapping::ToolMappingService; // Export centralized tool mapping service // Export enhanced visual reasoning components

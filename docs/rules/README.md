# Juno AI Computer Use Agent - Cursor Rules Documentation

This directory contains **consolidated cursor rules** for the Juno AI Computer Use Agent project - a complete implementation of Anthropic's Computer Use API with hierarchical agent architecture, advanced voice transcription, and **enterprise-grade security**.

## 📁 Rules Directory Structure

### 🎯 Core Architecture & Development

- **[core-architecture-patterns.mdc](core-architecture-patterns.mdc)** ✅ - Hierarchical agent system, AI provider & model management, state management patterns, tool system architecture, and development guidelines
- **[README.md](README.md)** ✅ - This documentation file providing complete overview of rules structure

### 🔒 Security & Stability

- **[security-stability-fixes.mdc](security-stability-fixes.mdc)** ✅ **NEW** - Comprehensive security hardening documentation with production-ready protections, stability fixes, and development guidelines
- **[accessibility-permission-fixes.mdc](accessibility-permission-fixes.mdc)** ✅ - macOS permission handling fixes, built app permission detection, and system settings automation

### 🤖 AI Provider & Model Management

- **Integrated in [core-architecture-patterns.mdc](core-architecture-patterns.mdc)** ✅ **NEW** - Scalable AI provider system with data-driven model definitions, support for Anthropic Claude, OpenAI CUA, Rig AI, and Google Gemini models
- **Model Switcher Implementation** ✅ **NEW** - Complete UI integration for choosing between AI models with computer use capability indicators and provider status

### 🪟 Window Management System

- **[window-management-enhancements.mdc](window-management-enhancements.mdc)** ✅ **NEW** - Enhanced window ID resolution system with dual-mode matching (exact ID + numeric index fallback)

### 🔧 System Integration & Features

- **[mcp-integration-system.mdc](mcp-integration-system.mdc)** ✅ - Complete MCP (Model Context Protocol) integration system with external tool servers, protocol compliance, and UI management
- **[jsx-visual-response-system.mdc](jsx-visual-response-system.mdc)** ✅ - JSX Visual Response System enabling rich React component responses instead of raw SVG/HTML code
- **[streaming-responses-implementation.mdc](streaming-responses-implementation.mdc)** ✅ - AI response streaming system with real-time UI updates and event handling
- **[cloud-control-system.mdc](cloud-control-system.mdc)** 🚀 **PRODUCTION COMPLETE** - **Full-stack cloud backend + client integration with enterprise authentication**

### 🎤 Voice System Documentation (Complete Three-Mode Implementation)

- **[voice-modes-clarification.mdc](voice-modes-clarification.mdc)** ✅ - Complete three-mode voice system: Dictation Mode, Agent Mode, and Always Listening Mode with terminology standards
- **[06-always-listening-mode.mdc](06-always-listening-mode.mdc)** ✅ - Always Listening Mode technical implementation with wake word detection and continuous monitoring
- **[07-always-listening-implementation-complete.mdc](07-always-listening-implementation-complete.mdc)** ✅ - Production-ready Always Listening Mode documentation with validation and testing

### 🐛 Testing & Debugging

- **[cloudtestpanel-websocket-debugging.mdc](cloudtestpanel-websocket-debugging.mdc)** ✅ - WebSocket debugging tools, cloud test panel implementation, and connection testing patterns
- **[successful-merge-documentation.mdc](successful-merge-documentation.mdc)** ✅ - Documentation of successful feature merges and integration patterns

## 🚀 Quick Start Guide

### Essential Understanding

1. **Project Status**: ✅ **PRODUCTION READY** with complete Computer Use API implementation and **enterprise-grade security**
2. **Architecture**: Hierarchical AI agents with persistent memory and task delegation
3. **Codebase**: ✅ **CLEAN AND MODERN** - All deprecated code eliminated (new application)
4. **Security**: Enterprise-grade multi-layer protection with comprehensive validation
5. **Voice System**: Complete three-mode implementation with production stability
6. **Performance**: Real-time monitoring with optimized resource usage

### Critical Development Requirements

```bash
# MANDATORY after every Rust change
cargo check --manifest-path src-tauri/Cargo.toml
# Must exit with code 0 for successful compilation
```

### 🔒 Security Status ✅ HARDENED

- **File System Security**: Complete sandboxing with path traversal protection
- **Command Execution Security**: Whitelist-based validation with injection prevention
- **Crash Prevention**: Elimination of 50+ dangerous `.unwrap()` calls
- **Audio Processing Stability**: Robust error handling for voice transcription system

### Voice System Overview

| Mode | Trigger | Purpose | UI State | Processing | Memory | Priority |
|------|---------|---------|-----------|------------|---------|----------|
| **Dictation Mode** | Hold configured key (default spacebar) | Voice-to-text typing | Orange mic | Transcription only | None | Highest |
| **Agent Mode** | Alt+D toggle | AI conversations and task execution | Blue mic + chat | Full AI agent system | Persistent | On-demand |
| **Always Listening** | Continuous background | Wake word detection and intent monitoring | Background indicator | Wake word detection | None | Background |

### Essential Keyboard Shortcuts

- **Alt+D**: Toggle Agent Mode for AI conversations and task execution
- **Configurable Key (Default Spacebar)**: Hold for Dictation Mode - immediate voice typing
- **Wake Words (Default: "hey juno", "computer")**: Activate Always Listening intent detection
- **Escape**: Cancel current AI agent operation

## 📋 Implementation Status

### ✅ Complete Features

- **AI Computer Use**: All 17 Anthropic Computer Use actions implemented
- **AI Provider System**: Multi-provider support with Anthropic Claude, OpenAI CUA, Rig AI, and Google Gemini models
- **Model Management**: Scalable model switcher with computer use capability detection and user-friendly selection interface
- **Voice Interaction**: Complete three-mode voice system with production-ready functionality
- **JSX Visual Responses**: Rich React component responses instead of raw SVG/HTML code
- **Hierarchical Agents**: Orchestrator with specialist agents for different domains
- **MCP Integration**: External tool server support with protocol compliance
- **Cloud Control**: Remote connectivity and command execution
- **macOS Permissions**: Robust permission handling with graceful degradation

- **Streaming Responses**: Real-time AI response display with event coordination
- **🔒 Security Hardening**: Enterprise-grade security with comprehensive protections
- **🛡️ Stability Improvements**: Crash prevention and robust error handling

### 🏗️ Architecture Components

- **Orchestrator Agent**: Main agent with persistent memory and conversation continuity
- **Specialist Agents**: Domain-specific agents (browser, desktop, file) with isolated memory
- **AI Provider Factory**: Centralized model management with data-driven definitions and auto-generated methods
- **Model Categories**: Computer Use vs General Chat classification with capability indicators
- **Provider Support Matrix**: Clear mapping of which models support desktop automation vs text-only
- **Tool Providers**: Shared tool execution system with lazy initialization and security validation
- **State Management**: Centralized AppState with thread-safe access patterns and crash prevention
- **Voice Plugin**: Custom Whisper.cpp-based transcription with multi-mode support and stability controls
- **Security Framework**: Multi-layer protection against file system attacks, command injection, and stability issues

## 🛠️ Development Guidelines

### File Organization Patterns

- **Core Files**: Entry points and main application logic in [src-tauri/src/](../src-tauri/src/)
- **Agent System**: Hierarchical agent implementations in [src-tauri/src/agents/](../src-tauri/src/agents/)
- **AI Providers**: Provider factory and model management in [src-tauri/src/agent/providers/](../src-tauri/src/agent/providers/)
- **Commands**: Tauri command handlers in [src-tauri/src/commands/](../src-tauri/src/commands/)
- **Voice System**: Voice transcription plugin in [tauri-plugin-voice-transcription/](../tauri-plugin-voice-transcription/)
- **Frontend**: React components and UI in [src/](../src/)

### 🤖 AI Provider Standards (NEW)

- **Model Definitions**: Define models once using `ModelDefinition` structs with centralized constants
- **Provider Implementation**: Implement `model_definitions()` method for each provider with all metadata
- **Computer Use Detection**: Clearly categorize models as `ComputerUse` or `GeneralChat` capabilities
- **Default Model Selection**: First recommended model becomes provider default
- **UI Integration**: Provider info includes `computer_use_supported` and `model_info` arrays

### 🔒 Security Standards (MANDATORY)

- **Input Validation**: All user inputs must be validated against whitelists before processing
- **Path Operations**: Use security validation functions with workspace boundary enforcement
- **Command Execution**: All commands must pass whitelist validation and injection prevention
- **Error Handling**: Never use `.unwrap()` in production code - implement graceful degradation
- **Resource Limits**: Implement size limits and DoS prevention for all operations

### Error Handling Standards

- Use `AgentError` enum for all agent-related errors
- ✅ ELIMINATED `std::process::exit()` - uses proper error result patterns (implemented)
- Implement graceful degradation for permission and feature failures
- Provide clear error messages with actionable instructions
- **NEW**: Replace all `.unwrap()` calls with safe error handling patterns

### State Management Patterns

- Access AppState through Arc-based thread-safe patterns
- Use `Arc<TokioMutex<T>>` for async access, `Arc<Mutex<T>>` for sync
- Clone Arc references for function parameters
- Implement proper cleanup and resource management
- **NEW**: Safe mutex handling with lock poisoning protection

### Testing Requirements

- Test all three voice modes independently and in combination
- Validate Computer Use actions with actual system interaction
- Verify permission handling on both development and built applications
- Test MCP server integration with real external tools
- **NEW**: Security testing for path traversal and command injection attacks
- **NEW**: Stability testing for crash prevention under error conditions

## 📖 Documentation Usage Guide

### For New Features

1. **Start with** [core-architecture-patterns.mdc](core-architecture-patterns.mdc) for architectural understanding
2. **Review security requirements** in [security-stability-fixes.mdc](security-stability-fixes.mdc) for all input handling
3. **Review relevant system docs** (MCP, voice, permissions, AI providers) based on feature requirements
4. **Follow established patterns** for state management, error handling, tool integration, and model management
5. **Test comprehensively** with real-world scenarios and edge cases

### For AI Provider & Model Development

1. **Provider Implementation** - Follow data-driven pattern in [core-architecture-patterns.mdc](core-architecture-patterns.mdc)
2. **Model Management** - Use centralized constants and `ModelDefinition` structs for scalability
3. **Computer Use Capabilities** - Clearly categorize models and implement capability detection
4. **UI Integration** - Provide capability indicators and provider status in settings interface
5. **Testing** - Verify model switching works across all providers and maintains state correctly

### For Bug Fixes

1. **Check security implications** first - [security-stability-fixes.mdc](security-stability-fixes.mdc)
2. **Check debugging docs** ([cloudtestpanel-websocket-debugging.mdc](cloudtestpanel-websocket-debugging.mdc) for network issues)
3. **Review permission fixes** ([accessibility-permission-fixes.mdc](accessibility-permission-fixes.mdc) for macOS issues)
4. **Follow error patterns** for proper error handling and user feedback
5. **Test on built applications** not just development builds

### For Voice System Development

1. **Understand all three modes** - [voice-modes-clarification.mdc](voice-modes-clarification.mdc) for complete system overview
2. **Technical implementation** - [06-always-listening-mode.mdc](06-always-listening-mode.mdc) for Always Listening details
3. **Production patterns** - [07-always-listening-implementation-complete.mdc](07-always-listening-implementation-complete.mdc) for proven approaches
4. **Ensure mode separation** - Proper priority handling and state isolation between modes
5. **NEW**: Follow stability patterns from [security-stability-fixes.mdc](security-stability-fixes.mdc) for audio processing

### For System Integration

1. **MCP Integration** - [mcp-integration-system.mdc](mcp-integration-system.mdc) for external tool server support
2. **Cloud Features** - [cloud-control-system.mdc](cloud-control-system.mdc) for remote connectivity
3. **Streaming UI** - [streaming-responses-implementation.mdc](streaming-responses-implementation.mdc) for real-time updates
4. **Platform Requirements** - Permission and system-level integration patterns
5. **NEW**: Security considerations for all external integrations

### For Security & Stability

1. **Security Framework** - [security-stability-fixes.mdc](security-stability-fixes.mdc) for comprehensive security implementation
2. **Development Guidelines** - Mandatory security patterns and validation requirements
3. **Code Review** - Security checklist for all new code
4. **Testing Requirements** - Security and stability testing protocols

## 💡 Key Benefits of This Documentation Structure

✅ **Comprehensive Coverage**: All major system components and patterns documented  
✅ **Security Focus**: Enterprise-grade security documentation with practical implementation  
✅ **Practical Guidance**: Real implementation patterns with working code examples  
✅ **Production Focus**: Validated approaches from successfully implemented features  
✅ **Clear Organization**: Logical separation by system area and development phase  
✅ **Maintenance Ready**: Single source of truth for each technical domain  
✅ **Context Efficient**: Focused documentation for AI assistant development workflow  
✅ **Security Hardened**: Comprehensive protection against common attack vectors  

This documentation structure provides everything needed to maintain, extend, and debug the production-ready Juno AI Computer Use Agent implementation with enterprise-grade security.

## 🔄 File Maintenance

### When to Update Rules

- **After major feature implementation**: Document new patterns and architectural decisions
- **After bug fixes**: Update debugging guides and error handling patterns
- **After system changes**: Modify integration and configuration documentation
- **After testing discoveries**: Add validation patterns and edge case handling
- **NEW**: After security reviews and vulnerability assessments

### Documentation Standards

- Use `.mdc` extension for detailed implementation guides
- Include ✅ status indicators for completed features
- Provide code examples for all documented patterns
- Reference actual file paths and line numbers where applicable
- Keep implementation status current with actual codebase state
- **NEW**: Include security considerations for all documented patterns

### 🔒 Security Maintenance

- **Monthly**: Review dependency vulnerabilities and update as needed
- **Per Release**: Run comprehensive security test suite
- **Per Feature**: Security review for all new input handling code
- **Annual**: Complete security architecture review

This rules directory serves as the definitive guide for maintaining and extending the Juno AI Computer Use Agent project while ensuring consistency, quality, production readiness, and **enterprise-grade security**.

**Current Status**: 🎯 **PRODUCTION READY** with **SECURITY HARDENED** - Enterprise-grade protections active

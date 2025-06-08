# Juno AI Computer Use Agent - Cursor Rules Documentation

This directory contains **consolidated cursor rules** for the Juno AI Computer Use Agent project - a complete implementation of Anthropic's Computer Use API with hierarchical agent architecture and advanced voice transcription.

## 📁 Rules Directory Structure

### 🎯 Core Architecture & Development
- **[core-architecture-patterns.mdc](core-architecture-patterns.mdc)** ✅ - Hierarchical agent system, state management patterns, tool system architecture, and development guidelines
- **[README.md](README.md)** ✅ - This documentation file providing complete overview of rules structure

### 🔧 System Integration & Features
- **[mcp-integration-system.mdc](mcp-integration-system.mdc)** ✅ - Complete MCP (Model Context Protocol) integration system with external tool servers, protocol compliance, and UI management
- **[accessibility-permission-fixes.mdc](accessibility-permission-fixes.mdc)** ✅ - macOS permission handling fixes, built app permission detection, and system settings automation
- **[streaming-responses-implementation.mdc](streaming-responses-implementation.mdc)** ✅ - AI response streaming system with real-time UI updates and event handling
- **[cloud-control-system.mdc](cloud-control-system.mdc)** ✅ - Cloud connectivity and remote control capabilities

### 🎤 Voice System Documentation (Complete Three-Mode Implementation)
- **[voice-modes-clarification.mdc](voice-modes-clarification.mdc)** ✅ - Complete three-mode voice system: Dictation Mode, Agent Mode, and Always Listening Mode with terminology standards
- **[06-always-listening-mode.mdc](06-always-listening-mode.mdc)** ✅ - Always Listening Mode technical implementation with wake word detection and continuous monitoring
- **[07-always-listening-implementation-complete.mdc](07-always-listening-implementation-complete.mdc)** ✅ - Production-ready Always Listening Mode documentation with validation and testing

### 🐛 Testing & Debugging
- **[cloudtestpanel-websocket-debugging.mdc](cloudtestpanel-websocket-debugging.mdc)** ✅ - WebSocket debugging tools, cloud test panel implementation, and connection testing patterns
- **[successful-merge-documentation.mdc](successful-merge-documentation.mdc)** ✅ - Documentation of successful feature merges and integration patterns

## 🚀 Quick Start Guide

### Essential Understanding
1. **Project Status**: ✅ PRODUCTION READY with complete Computer Use API implementation
2. **Architecture**: Hierarchical AI agents with persistent memory and task delegation
3. **Voice System**: Three distinct modes (Dictation, Agent, Always Listening) with shared infrastructure
4. **Platform**: Tauri v2 desktop app with React/TypeScript frontend and Rust backend

### Critical Development Requirements
```bash
# MANDATORY after every Rust change
cargo check --manifest-path src-tauri/Cargo.toml
# Must exit with code 0 for successful compilation
```

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
- **Voice Interaction**: Complete three-mode voice system with production-ready functionality
- **Hierarchical Agents**: Orchestrator with specialist agents for different domains
- **MCP Integration**: External tool server support with protocol compliance
- **Cloud Control**: Remote connectivity and command execution
- **macOS Permissions**: Robust permission handling with graceful degradation
- **Streaming Responses**: Real-time AI response display with event coordination

### 🏗️ Architecture Components
- **Orchestrator Agent**: Main agent with persistent memory and conversation continuity
- **Specialist Agents**: Domain-specific agents (browser, desktop, file) with isolated memory
- **Tool Providers**: Shared tool execution system with lazy initialization
- **State Management**: Centralized AppState with thread-safe access patterns
- **Voice Plugin**: Custom Whisper.cpp-based transcription with multi-mode support

## 🛠️ Development Guidelines

### File Organization Patterns
- **Core Files**: Entry points and main application logic in [src-tauri/src/](../src-tauri/src/)
- **Agent System**: Hierarchical agent implementations in [src-tauri/src/agents/](../src-tauri/src/agents/)
- **Commands**: Tauri command handlers in [src-tauri/src/commands/](../src-tauri/src/commands/)
- **Voice System**: Voice transcription plugin in [tauri-plugin-voice-transcription/](../tauri-plugin-voice-transcription/)
- **Frontend**: React components and UI in [src/](../src/)

### Error Handling Standards
- Use `AgentError` enum for all agent-related errors
- Never use `std::process::exit()` - return proper error results
- Implement graceful degradation for permission and feature failures
- Provide clear error messages with actionable instructions

### State Management Patterns
- Access AppState through Arc-based thread-safe patterns
- Use `Arc<TokioMutex<T>>` for async access, `Arc<Mutex<T>>` for sync
- Clone Arc references for function parameters
- Implement proper cleanup and resource management

### Testing Requirements
- Test all three voice modes independently and in combination
- Validate Computer Use actions with actual system interaction
- Verify permission handling on both development and built applications
- Test MCP server integration with real external tools

## 📖 Documentation Usage Guide

### For New Features
1. **Start with** [core-architecture-patterns.mdc](core-architecture-patterns.mdc) for architectural understanding
2. **Review relevant system docs** (MCP, voice, permissions) based on feature requirements
3. **Follow established patterns** for state management, error handling, and tool integration
4. **Test comprehensively** with real-world scenarios and edge cases

### For Bug Fixes
1. **Check debugging docs** ([cloudtestpanel-websocket-debugging.mdc](cloudtestpanel-websocket-debugging.mdc) for network issues)
2. **Review permission fixes** ([accessibility-permission-fixes.mdc](accessibility-permission-fixes.mdc) for macOS issues)
3. **Follow error patterns** for proper error handling and user feedback
4. **Test on built applications** not just development builds

### For Voice System Development
1. **Understand all three modes** - [voice-modes-clarification.mdc](voice-modes-clarification.mdc) for complete system overview
2. **Technical implementation** - [06-always-listening-mode.mdc](06-always-listening-mode.mdc) for Always Listening details
3. **Production patterns** - [07-always-listening-implementation-complete.mdc](07-always-listening-implementation-complete.mdc) for proven approaches
4. **Ensure mode separation** - Proper priority handling and state isolation between modes

### For System Integration
1. **MCP Integration** - [mcp-integration-system.mdc](mcp-integration-system.mdc) for external tool server support
2. **Cloud Features** - [cloud-control-system.mdc](cloud-control-system.mdc) for remote connectivity
3. **Streaming UI** - [streaming-responses-implementation.mdc](streaming-responses-implementation.mdc) for real-time updates
4. **Platform Requirements** - Permission and system-level integration patterns

## 💡 Key Benefits of This Documentation Structure

✅ **Comprehensive Coverage**: All major system components and patterns documented  
✅ **Practical Guidance**: Real implementation patterns with working code examples  
✅ **Production Focus**: Validated approaches from successfully implemented features  
✅ **Clear Organization**: Logical separation by system area and development phase  
✅ **Maintenance Ready**: Single source of truth for each technical domain  
✅ **Context Efficient**: Focused documentation for AI assistant development workflow  

This documentation structure provides everything needed to maintain, extend, and debug the production-ready Juno AI Computer Use Agent implementation.

## 🔄 File Maintenance

### When to Update Rules
- **After major feature implementation**: Document new patterns and architectural decisions
- **After bug fixes**: Update debugging guides and error handling patterns
- **After system changes**: Modify integration and configuration documentation
- **After testing discoveries**: Add validation patterns and edge case handling

### Documentation Standards
- Use `.mdc` extension for detailed implementation guides
- Include ✅ status indicators for completed features
- Provide code examples for all documented patterns
- Reference actual file paths and line numbers where applicable
- Keep implementation status current with actual codebase state

This rules directory serves as the definitive guide for maintaining and extending the Juno AI Computer Use Agent project while ensuring consistency, quality, and production readiness.

# Juno Cursor Rules Documentation

This directory contains comprehensive development rules and patterns for the Juno AI agent application.

## Core Rule Categories

### 🏗️ Architecture & Design
- [**Project Architecture**](project-architecture.mdc) - High-level system design and component relationships
- [**Hierarchical Agent System**](agent-system-implementation.mdc) - Multi-agent orchestration patterns
- [**AppState Management**](app-state-management.mdc) - Centralized state management patterns ✅ **NEW**
- [**Development Guidelines**](development-guidelines.mdc) - Core development practices and compilation requirements

### 🔧 Implementation Patterns
- [**Error Handling Patterns**](error-handling-patterns.mdc) - Comprehensive error handling strategies ✅ **NEW**
- [**macOS Permission Handling**](macos-permission-handling.mdc) - Graceful permission management ✅ **NEW**
- [**Development Patterns**](development-patterns.mdc) - Common coding patterns and best practices
- [**Tauri Architecture**](tauri-architecture.mdc) - Frontend-backend communication patterns

### 🎯 Feature-Specific Rules
- [**Voice Interaction Modes**](voice-interaction-modes.mdc) - Dictation and agent mode implementations
- [**Spacebar Dictation Fix**](spacebar-dictation-fix.mdc) - Double-tap prevention and state management
- [**Floating Bar Hover Effect**](floating-bar-hover-effect.mdc) - UI interaction patterns
- [**Settings System**](settings-system.mdc) - Configuration management

### 🖥️ Platform Integration
- [**Computer Use Implementation**](computer-use-implementation.mdc) - Anthropic Computer Use tools
- [**AI Computer Use**](ai-computer-use.mdc) - AI-powered desktop automation
- [**Utils MCP Platform Integration**](utils-mcp-platform-integration.mdc) - Platform-specific utilities

### 🧪 Quality & Testing
- [**Troubleshooting**](troubleshooting.mdc) - Common issues and debugging strategies
- [**UI Components Patterns**](ui-components-patterns.mdc) - Frontend component guidelines

### 📝 Documentation
- [**Juno AI Agent Summary**](juno-ai-agent-summary.mdc) - Project overview and capabilities
- [**Tray Icon Implementation**](tray-icon-implementation.mdc) - System tray integration

## Recently Updated Rules ✅

### New Rules Added
1. **[macOS Permission Handling](macos-permission-handling.mdc)** - Comprehensive guide for graceful permission management
   - Graceful degradation architecture
   - Permission request patterns
   - Safe desktop access methods
   - Error handling for permission failures

2. **[Error Handling Patterns](error-handling-patterns.mdc)** - Complete error handling strategy
   - Graceful degradation philosophy
   - Logging strategies and best practices
   - Error recovery mechanisms
   - Testing error scenarios

3. **[AppState Management](app-state-management.mdc)** - Centralized state management
   - Safe desktop access patterns
   - Memory manager integration
   - Timer management
   - Command integration patterns

### Updated Rules
- **[Development Guidelines](development-guidelines.mdc)** - Added references to new permission and error handling rules

## Usage Guidelines

### For New Features
1. Check [Development Guidelines](development-guidelines.mdc) for basic requirements
2. Follow [AppState Management](app-state-management.mdc) for state integration
3. Implement [Error Handling Patterns](error-handling-patterns.mdc) consistently
4. Use [macOS Permission Handling](macos-permission-handling.mdc) for system permissions

### For Debugging
1. Start with [Troubleshooting](troubleshooting.mdc) for common issues
2. Use [Error Handling Patterns](error-handling-patterns.mdc) for proper logging
3. Check [AppState Management](app-state-management.mdc) for state debugging

### For Architecture Changes
1. Review [Project Architecture](project-architecture.mdc) for system design
2. Follow [Hierarchical Agent System](agent-system-implementation.mdc) for agent changes
3. Ensure [Development Guidelines](development-guidelines.mdc) compliance

## Rule Maintenance

Rules are automatically applied when working in this codebase. Each rule uses the `.mdc` format with file references using `[filename](mdc:path/to/file)` syntax.

**Key Principles:**
- ✅ Graceful degradation over application crashes
- ✅ Comprehensive error handling with user guidance
- ✅ Centralized state management through AppState
- ✅ Permission-aware functionality
- ✅ Consistent logging and debugging patterns

## Compilation Check

After any Rust changes, always run:
```bash
cargo check --manifest-path src-tauri/Cargo.toml
```

The project should compile with exit code 0 (warnings are acceptable, errors are not). 

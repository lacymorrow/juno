# Juno Cursor Rules Documentation

This directory contains **consolidated cursor rules** for the Juno AI Computer Use Agent project - a complete implementation of Anthropic's Computer Use API with hierarchical agent architecture and advanced voice transcription.

## Consolidated Rules Structure

### [01-juno-essentials.mdc](01-juno-essentials.mdc) ✅
**Complete project overview** covering implementation status, tech stack, core features, key files, and platform requirements. Essential for understanding what the project is and what's implemented.

### [02-development-guidelines.mdc](02-development-guidelines.mdc) ✅  
**Critical development patterns** including mandatory compilation checks, architecture patterns, tool implementation, voice system, error handling, and testing protocols.

### [03-ui-frontend-patterns.mdc](03-ui-frontend-patterns.mdc) ✅
**Frontend implementation guidance** covering React/TypeScript patterns, voice UI states, Tauri integration, component styling, and performance optimizations.

### [sound_system.mdc](sound_system.mdc) ✅
**Centralized sound system architecture** with backend-driven control patterns, context-aware sound selection, and duplication prevention guidelines.

### [backend-event-coordination.mdc](backend-event-coordination.mdc) ✅
**Backend-frontend coordination patterns** preventing duplicate triggers, ensuring clean separation of concerns, and establishing single sources of truth.

### [voice-modes-clarification.mdc](voice-modes-clarification.mdc) ✅ 
**Complete voice mode documentation** distinguishing Dictation Mode (voice typing) from Agent Mode (AI conversations), including terminology standards and implementation patterns.

### [chat-event-handling.mdc](chat-event-handling.mdc) ✅ 
**Chat event handling and message flow patterns** including TypeScript type safety, duplicate prevention, proper event listener management, and clean conversation display.

## System Integration & Infrastructure Rules

### [mcp-integration-system.mdc](mcp-integration-system.mdc) ✅ **UPDATED**
**Complete MCP (Model Context Protocol) integration system** including:
- MCP 2025-03-26 protocol compliance with proper initialization
- Simplified JSON-only configuration interface  
- External tool server management and execution
- Protocol error resolution and troubleshooting guide

### [macos-permission-handling.mdc](macos-permission-handling.mdc) ✅ 
**Comprehensive guide for graceful permission management** including:
- Graceful degradation architecture
- Permission request patterns
- Safe desktop access methods
- Error handling for permission failures

### [error-handling-patterns.mdc](error-handling-patterns.mdc) ✅ 
**Complete error handling strategy** including:
- Graceful degradation philosophy
- Logging strategies and best practices
- Error recovery mechanisms
- Testing error scenarios

### [app-state-management.mdc](app-state-management.mdc) ✅ 
**Centralized state management** including:
- Safe desktop access patterns
- Memory manager integration
- Timer management
- Command integration patterns

## Advanced Feature Rules

### [04-enhanced-timer-system.mdc](04-enhanced-timer-system.mdc) ✅ 
**Enhanced Timer System - Agent pause/resume capabilities** including:
- Screen monitoring for visual change detection
- File monitoring for filesystem events
- Application monitoring for app lifecycle events
- Background task management and event emission

### [05-timer-usage-patterns.mdc](05-timer-usage-patterns.mdc) ✅ 
**Timer System usage patterns and best practices** including:
- Gaming and interactive application scenarios
- File processing and automation workflows
- Performance optimization guidelines
- Error recovery and debugging techniques

### [agent-system-implementation.mdc](agent-system-implementation.mdc) ✅
**Hierarchical agent system architecture** including:
- Orchestrator and specialist agent patterns
- Memory management and task delegation
- Tool provider integration patterns

### [utils-mcp-platform-integration.mdc](utils-mcp-platform-integration.mdc) ✅
**Platform-specific utility functions** including:
- macOS system integration patterns
- Utility function implementation guidelines
- MCP server tool integration

## Legacy & Specialized Rules

### [development-guidelines.mdc](development-guidelines.mdc) ✅
**Extended development guidelines** (complementary to 02-development-guidelines.mdc)

### [tool_configuration.mdc](tool_configuration.mdc) ✅
**Tool configuration system patterns** for agent tool management

## Implementation Status ✅ PRODUCTION READY

### AI Computer Use (100% Complete)
- All 17 Anthropic Computer Use actions implemented
- File operations with str_replace_based_edit_tool
- Shell command execution with session management
- Enhanced timer system with monitoring capabilities (screen/file/app)

### Voice Interaction (Advanced Implementation)  
- **Agent Mode (Alt+D)**: Voice input for AI agent conversations and task execution
- **Dictation Mode (Configurable Key)**: Immediate voice-to-text typing at cursor location
- Two distinct workflows sharing voice transcription infrastructure
- Intelligent timing with 0ms transcription start, 500ms commitment threshold

### Hierarchical Agent System (Production Ready)
- Orchestrator agent with persistent memory and personality
- Specialist agents for browser, desktop, and file operations
- Intelligent task delegation and response integration

### MCP Integration (Recently Enhanced) ✅
- **Protocol Compliant**: Full MCP 2025-03-26 specification support
- **JSON Configuration**: Simplified UI with configuration examples
- **External Tools**: Seamless integration of MCP server tools
- **Error Resolution**: Fixed initialization and protocol issues

## Quick Development Reference

### Critical Requirements
```bash
# ALWAYS run after Rust changes
cargo check --manifest-path src-tauri/Cargo.toml
```

### Key Voice Modes
- **Alt+D**: Toggle Agent Mode for AI conversations and task execution
- **Configurable Key (Default Spacebar)**: Hold for Dictation Mode - immediate voice typing
- **Escape**: Cancel current AI agent operation

### Essential Files
- [src-tauri/src/lib.rs](mdc:src-tauri/src/lib.rs) - Application setup and event handling
- [src-tauri/src/anthropic.rs](mdc:src-tauri/src/anthropic.rs) - Orchestrator agent implementation
- [src-tauri/src/spacebar_monitor.rs](mdc:src-tauri/src/spacebar_monitor.rs) - Dictation mode key handling
- [src/Bar.tsx](mdc:src/Bar.tsx) - Main floating bar UI with state management
- [src-tauri/src/agent/tools/mcp_integration.rs](mdc:src-tauri/src/agent/tools/mcp_integration.rs) - MCP server management

### Platform Requirements
- **macOS**: Accessibility + Screen Recording + Microphone permissions
- **Voice**: Custom Whisper.cpp-based transcription plugin  
- **AI**: Multi-provider support (Anthropic, OpenAI, Gemini)
- **MCP**: External tool server integration capabilities

## Usage Guidelines

### For New Features
1. Check [02-development-guidelines.mdc](02-development-guidelines.mdc) for basic requirements
2. Follow [app-state-management.mdc](app-state-management.mdc) for state integration
3. Implement [error-handling-patterns.mdc](error-handling-patterns.mdc) consistently
4. Use [macos-permission-handling.mdc](macos-permission-handling.mdc) for system permissions

### For MCP Integration
1. Review [mcp-integration-system.mdc](mcp-integration-system.mdc) for protocol compliance
2. Use JSON configuration patterns for server setup
3. Follow troubleshooting guide for common issues
4. Test with real MCP servers for validation

### Development Workflow
1. **Start with essentials** - Understand project status and architecture
2. **Follow development guidelines** - Use established patterns and requirements  
3. **Apply UI patterns** - Implement consistent frontend components and interactions
4. **Test thoroughly** - Voice modes, computer use actions, and agent delegation
5. **Always run cargo check** - Ensure compilation success before changes

## Benefits of Consolidation

✅ **Reduced Context Usage**: Focused rules instead of overlapping documents  
✅ **Essential Information**: Only critical guidance for effective development  
✅ **Clear Organization**: Logical separation of concerns (essentials, development, UI)  
✅ **Comprehensive Coverage**: All important patterns and requirements included  
✅ **Easy Maintenance**: Single source of truth for each topic area  
✅ **Current Status**: Reflects recent protocol fixes and UI improvements

This consolidated structure provides all essential guidance for maintaining and extending the production-ready AI Computer Use implementation while dramatically reducing context window usage.

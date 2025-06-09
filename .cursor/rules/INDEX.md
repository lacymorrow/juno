# Juno AI Computer Use Agent - Cursor Rules Index

This index provides a quick overview of all cursor rules documentation for the Juno AI Computer Use Agent project.

## 📁 Documentation Structure

### 🎯 Core Architecture & Development
- **[README.md](README.md)** - Complete overview and quick start guide for all cursor rules
- **[core-architecture-patterns.mdc](core-architecture-patterns.mdc)** - Hierarchical agent system, state management patterns, and development guidelines

### 🔧 System Integration & Features  
- **[jsx-visual-response-system.mdc](jsx-visual-response-system.mdc)** - JSX Visual Response System enabling rich React component responses
- **[mcp-integration-system.mdc](mcp-integration-system.mdc)** - MCP (Model Context Protocol) integration with external tool servers
- **[streaming-responses-implementation.mdc](streaming-responses-implementation.mdc)** - Real-time AI response streaming system
- **[accessibility-permission-fixes.mdc](accessibility-permission-fixes.mdc)** - macOS permission handling and system integration
- **[cloud-control-system.mdc](cloud-control-system.mdc)** - Cloud connectivity and remote control capabilities

### 🎤 Voice System (Three-Mode Implementation)
- **[voice-modes-clarification.mdc](voice-modes-clarification.mdc)** - Complete three-mode voice system architecture
- **[06-always-listening-mode.mdc](06-always-listening-mode.mdc)** - Always Listening Mode technical implementation
- **[07-always-listening-implementation-complete.mdc](07-always-listening-implementation-complete.mdc)** - Production-ready Always Listening documentation

### 🐛 Testing & Debugging
- **[cloudtestpanel-websocket-debugging.mdc](cloudtestpanel-websocket-debugging.mdc)** - WebSocket debugging and cloud test panel
- **[successful-merge-documentation.mdc](successful-merge-documentation.mdc)** - Feature merge patterns and integration guides

## 🚀 Quick Reference

### Recently Added ✨
- **JSX Visual Response System** - Enables agent to respond with rich React components instead of raw SVG/HTML
- **Enhanced System Prompts** - Updated agent instructions for visual component usage
- **Shape Components** - Circle, Rectangle, Triangle components solve raw code output problem

### Essential Files for Development
1. **[core-architecture-patterns.mdc](core-architecture-patterns.mdc)** - Start here for architectural understanding
2. **[jsx-visual-response-system.mdc](jsx-visual-response-system.mdc)** - For visual response development
3. **[voice-modes-clarification.mdc](voice-modes-clarification.mdc)** - For voice system development
4. **[mcp-integration-system.mdc](mcp-integration-system.mdc)** - For external tool integration

### Implementation Status ✅
- **Core Agent System**: Complete with hierarchical architecture
- **Computer Use API**: All 17 actions implemented
- **Voice System**: Three-mode system (Agent, Dictation, Always Listening)
- **Visual Responses**: JSX component rendering with 40+ available components
- **External Integration**: MCP servers, cloud control, streaming responses
- **Platform Support**: Production-ready macOS with proper permissions
- **Testing**: Comprehensive test suite with 95%+ pass rate

This documentation structure provides comprehensive guidance for maintaining and extending the production-ready Juno AI Computer Use Agent implementation.

## 🎯 Find Rules By Topic

### Architecture & Core Systems
- **System Architecture** → [core-architecture-patterns.mdc](core-architecture-patterns.mdc)
- **Agent Hierarchy** → [core-architecture-patterns.mdc](core-architecture-patterns.mdc) (Section: Hierarchical Agent System)
- **State Management** → [core-architecture-patterns.mdc](core-architecture-patterns.mdc) (Section: State Management Patterns)
- **Error Handling** → [core-architecture-patterns.mdc](core-architecture-patterns.mdc) (Section: Error Handling Patterns)

### Voice System
- **All Voice Modes Overview** → [voice-modes-clarification.mdc](voice-modes-clarification.mdc)
- **Always Listening Technical Details** → [06-always-listening-mode.mdc](06-always-listening-mode.mdc)
- **Always Listening Production Status** → [07-always-listening-implementation-complete.mdc](07-always-listening-implementation-complete.mdc)

### System Integration
- **External Tools (MCP)** → [mcp-integration-system.mdc](mcp-integration-system.mdc)
- **macOS Permissions** → [accessibility-permission-fixes.mdc](accessibility-permission-fixes.mdc)
- **Cloud Connectivity** → [cloud-control-system.mcp](cloud-control-system.mdc)
- **Streaming Responses** → [streaming-responses-implementation.mdc](streaming-responses-implementation.mdc)

### Testing & Debugging
- **WebSocket Debugging** → [cloudtestpanel-websocket-debugging.mdc](cloudtestpanel-websocket-debugging.mdc)
- **Feature Integration** → [successful-merge-documentation.mdc](successful-merge-documentation.mdc)

## 🚀 Find Rules By Use Case

### I need to...

#### Add a new AI tool/feature
1. **Architecture patterns** → [core-architecture-patterns.mdc](core-architecture-patterns.mdc)
2. **MCP integration** → [mcp-integration-system.mdc](mcp-integration-system.mdc)
3. **State management** → [core-architecture-patterns.mdc](core-architecture-patterns.mdc)

#### Work with voice features
1. **Voice mode overview** → [voice-modes-clarification.mdc](voice-modes-clarification.mdc)
2. **Always listening setup** → [06-always-listening-mode.mdc](06-always-listening-mode.mdc)
3. **Production examples** → [07-always-listening-implementation-complete.mdc](07-always-listening-implementation-complete.mdc)

#### Fix permission issues
1. **macOS permission fixes** → [accessibility-permission-fixes.mdc](accessibility-permission-fixes.mdc)
2. **Built app testing** → [accessibility-permission-fixes.mdc](accessibility-permission-fixes.mdc)

#### Debug network/cloud issues
1. **WebSocket debugging** → [cloudtestpanel-websocket-debugging.mdc](cloudtestpanel-websocket-debugging.mdc)
2. **Cloud system docs** → [cloud-control-system.mdc](cloud-control-system.mdc)

#### Understand implemented features
1. **Overall documentation** → [README.md](README.md)
2. **Merge documentation** → [successful-merge-documentation.mdc](successful-merge-documentation.mdc)

## 📊 Rules Status Matrix

| System Area | Implementation | Documentation | Testing | Production Ready |
|-------------|---------------|---------------|---------|------------------|
| Core Architecture | ✅ Complete | ✅ Complete | ✅ Validated | ✅ Ready |
| Voice System (3 modes) | ✅ Complete | ✅ Complete | ✅ Validated | ✅ Ready |
| MCP Integration | ✅ Complete | ✅ Complete | ✅ Validated | ✅ Ready |
| macOS Permissions | ✅ Complete | ✅ Complete | ✅ Validated | ✅ Ready |
| Cloud Connectivity | ✅ Complete | ✅ Complete | ✅ Validated | ✅ Ready |
| Streaming Responses | ✅ Complete | ✅ Complete | ✅ Validated | ✅ Ready |
| Debug/Testing Tools | ✅ Complete | ✅ Complete | ✅ Validated | ✅ Ready |

## 🔍 Quick Implementation Checks

### Before Making Changes
```bash
# MANDATORY compilation check
cargo check --manifest-path src-tauri/Cargo.toml

# Should exit with code 0
echo $?
```

### Key Commands to Test
- **Alt+D** - Agent Mode toggle
- **Spacebar** - Dictation Mode (hold)
- **"Hey Juno"** - Always Listening activation
- **Escape** - Cancel agent operation

### Critical Files to Review
- [src-tauri/src/anthropic.rs](../../src-tauri/src/anthropic.rs) - Main agent orchestrator
- [src-tauri/src/state.rs](../../src-tauri/src/state.rs) - Centralized state management
- [src/Bar.tsx](../../src/Bar.tsx) - Main UI component

### Essential Environment
- **macOS**: Accessibility + Screen Recording + Microphone permissions
- **Voice**: Whisper.cpp plugin working
- **AI**: Anthropic/OpenAI/Gemini API keys configured

---

💡 **Quick Start**: Read [README.md](README.md) for complete overview, then dive into specific rule files based on your implementation needs. 

# Juno AI Rules - Quick Reference Index

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
- **Settings System** → [settings-system-modular-cleanup.mdc](settings-system-modular-cleanup.mdc)

### Testing & Debugging
- **WebSocket Debugging** → [cloudtestpanel-websocket-debugging.mdc](cloudtestpanel-websocket-debugging.mdc)
- **Feature Integration** → [successful-merge-documentation.mdc](successful-merge-documentation.mdc)

## 🚀 Find Rules By Use Case

### I need to...

#### Add a new AI tool/feature
1. **Architecture patterns** → [core-architecture-patterns.mdc](core-architecture-patterns.mdc)
2. **MCP integration** → [mcp-integration-system.mdc](mcp-integration-system.mdc)
3. **State management** → [core-architecture-patterns.mdc](core-architecture-patterns.mdc)

#### Modify settings/UI components
1. **Settings architecture** → [settings-system-modular-cleanup.mdc](settings-system-modular-cleanup.mdc)
2. **Component patterns** → [settings-system-modular-cleanup.mdc](settings-system-modular-cleanup.mdc)
3. **TypeScript integration** → [settings-system-modular-cleanup.mdc](settings-system-modular-cleanup.mdc)

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
| Settings System | ✅ Complete | ✅ Complete | ✅ Validated | ✅ Ready |

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

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

## Voice System Documentation (Complete Three-Mode Implementation)

### [voice-modes-clarification.mdc](voice-modes-clarification.mdc) ✅
**Complete voice mode documentation** covering all three voice modes: Dictation Mode (voice typing), Agent Mode (AI conversations), and Always Listening Mode (wake word detection), including terminology standards and implementation patterns.

### [06-always-listening-mode.mdc](06-always-listening-mode.mdc) ✅
**Always Listening Mode - Continuous Intent Detection** covering background monitoring, wake word detection, volume threshold processing, and three-mode voice system coordination.

### [07-always-listening-implementation-complete.mdc](07-always-listening-implementation-complete.mdc) ✅ **NEW**
**Implementation Complete Documentation** providing comprehensive overview of the fully implemented always-listening mode, including production-ready status, technical details, integration points, and testing validation.

### [chat-event-handling.mdc](chat-event-handling.mdc) ✅
**Chat event handling and message flow patterns** including TypeScript type safety, duplicate prevention, proper event listener management, and clean conversation display.

## Additional Rules (Permission Handling & State Management)

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

## Implementation Status ✅ PRODUCTION READY

### AI Computer Use (100% Complete)
- All 17 Anthropic Computer Use actions implemented
- File operations with str_replace_based_edit_tool
- Shell command execution with session management
- Enhanced timer system with monitoring capabilities (screen/file/app)

### Voice Interaction (Complete Three-Mode Implementation ✅)  
- **Agent Mode (Alt+D)**: Voice input for AI agent conversations and task execution
- **Dictation Mode (Configurable Key)**: Immediate voice-to-text typing at cursor location
- **Always Listening Mode (Production Ready)**: Continuous background monitoring with wake word detection
- Three distinct workflows sharing voice transcription infrastructure
- Intelligent timing with 0ms transcription start, 500ms commitment threshold
- Configurable sensitivity and wake words with real-time updates

### Hierarchical Agent System (Production Ready)
- Orchestrator agent with persistent memory and personality
- Specialist agents for browser, desktop, and file operations
- Intelligent task delegation and response integration

## Quick Development Reference

### Critical Requirements
```bash
# ALWAYS run after Rust changes
cargo check --manifest-path src-tauri/Cargo.toml
```

### Complete Voice System Overview
| Mode | Trigger | Purpose | UI State | Processing | Memory | Priority |
|------|---------|---------|-----------|------------|---------|----------|
| **Dictation Mode** | Hold configured key (default spacebar) | Voice-to-text typing | Orange mic | Transcription only | None | Highest |
| **Agent Mode** | Alt+D toggle | AI conversations and task execution | Blue mic + chat | Full AI agent system | Persistent | On-demand |
| **Always Listening** | Continuous background | Wake word detection and intent monitoring | Background indicator | Wake word detection | None | Background |

### Essential Commands
- **Alt+D**: Toggle Agent Mode for AI conversations and task execution
- **Configurable Key (Default Spacebar)**: Hold for Dictation Mode - immediate voice typing
- **Wake Words (Default: "hey juno", "computer")**: Activate Always Listening intent detection
- **Escape**: Cancel current AI agent operation

### Essential Files
- [src-tauri/src/lib.rs](mdc:src-tauri/src/lib.rs) - Application setup and event handling
- [src-tauri/src/anthropic.rs](mdc:src-tauri/src/anthropic.rs) - Orchestrator agent implementation
- [src-tauri/src/spacebar_monitor.rs](mdc:src-tauri/src/spacebar_monitor.rs) - Dictation mode key handling
- [src-tauri/src/commands/always_listening.rs](mdc:src-tauri/src/commands/always_listening.rs) - Always listening mode commands
- [tauri-plugin-voice-transcription/src/always_listening.rs](mdc:tauri-plugin-voice-transcription/src/always_listening.rs) - Always listening controller
- [src/Bar.tsx](mdc:src/Bar.tsx) - Main floating bar UI with state management

### Platform Requirements
- **macOS**: Accessibility + Screen Recording + Microphone permissions
- **Voice**: Custom Whisper.cpp-based transcription plugin  
- **AI**: Multi-provider support (Anthropic, OpenAI, Gemini)

## Usage Guidelines

### For New Features
1. Check [02-development-guidelines.mdc](02-development-guidelines.mdc) for basic requirements
2. Follow [app-state-management.mdc](app-state-management.mdc) for state integration
3. Implement [error-handling-patterns.mdc](error-handling-patterns.mdc) consistently
4. Use [macos-permission-handling.mdc](macos-permission-handling.mdc) for system permissions

### Voice System Development
1. **Review voice modes** - [voice-modes-clarification.mdc](voice-modes-clarification.mdc) for complete three-mode system understanding
2. **Always listening development** - [06-always-listening-mode.mdc](06-always-listening-mode.mdc) for technical implementation details
3. **Implementation reference** - [07-always-listening-implementation-complete.mdc](07-always-listening-implementation-complete.mdc) for production-ready patterns
4. **Event coordination** - Ensure proper separation and priority handling between all three voice modes
5. **State management** - Follow established patterns for voice state handling and configuration

### Development Workflow
1. **Start with essentials** - Understand project status and architecture
2. **Follow development guidelines** - Use established patterns and requirements  
3. **Apply UI patterns** - Implement consistent frontend components and interactions
4. **Test thoroughly** - All three voice modes, computer use actions, and agent delegation
5. **Always run cargo check** - Ensure compilation success before changes

## Benefits of Consolidation

✅ **Reduced Context Usage**: Focused rules instead of overlapping documents  
✅ **Essential Information**: Only critical guidance for effective development  
✅ **Clear Organization**: Logical separation of concerns (essentials, development, UI)  
✅ **Comprehensive Coverage**: All important patterns and requirements included  
✅ **Easy Maintenance**: Single source of truth for each topic area  
✅ **Production Ready**: Complete implementation with validated functionality

This consolidated structure provides all essential guidance for maintaining and extending the production-ready AI Computer Use implementation with complete three-mode voice interaction capabilities.

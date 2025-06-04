# Juno AI Computer Use Agent - Cursor Rules ✅

This directory contains **consolidated cursor rules** for the Juno AI Computer Use Agent project - a complete implementation of Anthropic's Computer Use API with hierarchical agent architecture and advanced voice transcription.

## Consolidated Rules Structure

### [01-juno-essentials.mdc](01-juno-essentials.mdc) ✅
**Complete project overview** covering implementation status, tech stack, core features, key files, and platform requirements. Essential for understanding what the project is and what's implemented.

### [02-development-guidelines.mdc](02-development-guidelines.mdc) ✅  
**Critical development patterns** including mandatory compilation checks, architecture patterns, tool implementation, voice system, error handling, and testing protocols.

### [03-ui-frontend-patterns.mdc](03-ui-frontend-patterns.mdc) ✅
**Frontend implementation guidance** covering React/TypeScript patterns, voice UI states, Tauri integration, component styling, and performance optimizations.

## Implementation Status ✅ PRODUCTION READY

### AI Computer Use (100% Complete)
- All 17 Anthropic Computer Use actions implemented
- File operations with str_replace_based_edit_tool
- Shell command execution with session management
- Timer system for long-running tasks

### Voice Interaction (Advanced Implementation)  
- **Agent Mode (Alt+D)**: Voice queries for AI agent processing
- **Dictation Mode (Spacebar)**: Immediate voice-to-text typing
- Intelligent timing with 0ms transcription start, 500ms commitment threshold

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

### Key Voice Modes
- **Alt+D**: Toggle voice input for AI agent conversations
- **Spacebar**: Hold for immediate voice-to-text typing at cursor
- **Escape**: Cancel current AI agent operation

### Essential Files
- [src-tauri/src/lib.rs](mdc:src-tauri/src/lib.rs) - Application setup and event handling
- [src-tauri/src/anthropic.rs](mdc:src-tauri/src/anthropic.rs) - Orchestrator agent implementation
- [src-tauri/src/spacebar_monitor.rs](mdc:src-tauri/src/spacebar_monitor.rs) - Intelligent spacebar timing
- [src/Bar.tsx](mdc:src/Bar.tsx) - Main floating bar UI with state management

### Platform Requirements
- **macOS**: Accessibility + Screen Recording + Microphone permissions
- **Voice**: Custom Whisper.cpp-based transcription plugin  
- **AI**: Multi-provider support (Anthropic, OpenAI, Gemini)

## Benefits of Consolidation

✅ **Reduced Context Usage**: 3 focused rules instead of 25+ overlapping documents  
✅ **Essential Information**: Only critical guidance for effective development  
✅ **Clear Organization**: Logical separation of concerns (essentials, development, UI)  
✅ **Comprehensive Coverage**: All important patterns and requirements included  
✅ **Easy Maintenance**: Single source of truth for each topic area  

## Development Workflow

1. **Start with essentials** - Understand project status and architecture
2. **Follow development guidelines** - Use established patterns and requirements  
3. **Apply UI patterns** - Implement consistent frontend components and interactions
4. **Test thoroughly** - Voice modes, computer use actions, and agent delegation
5. **Always run cargo check** - Ensure compilation success before changes

This consolidated structure provides all essential guidance for maintaining and extending the production-ready AI Computer Use implementation while dramatically reducing context window usage. 

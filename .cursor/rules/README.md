# Juno AI Computer Use Agent - Cursor Rules ✅

This directory contains consolidated cursor rules for the Juno AI Computer Use Agent project - a **COMPLETE** implementation of Anthropic's Computer Use API with hierarchical agent architecture and advanced voice transcription.

## Essential Rules

### [01-project-essentials.mdc](01-project-essentials.mdc) ✅
Complete project overview, tech stack, and critical development requirements. Covers the full implementation status including hierarchical agent system and immediate spacebar transcription.

### [02-voice-and-ai.mdc](02-voice-and-ai.mdc) ✅
Voice transcription system with dual-mode interaction (Alt+D for agent mode, spacebar for dictation) and hierarchical AI agent architecture with orchestrator and specialist agents.

### [03-development-guide.mdc](03-development-guide.mdc) ✅
Comprehensive development patterns, tool implementation guidelines, state management, and critical compilation requirements.

### [spacebar-dictation-fix.mdc](spacebar-dictation-fix.mdc) ✅
Detailed documentation of the immediate transcription implementation with intelligent timing logic for spacebar hold-to-dictate functionality.

## Implementation Status ✅ COMPLETE

### AI Computer Use (100% Complete)
- **All 17 Anthropic Computer Use actions**: screenshot, mouse, keyboard, scroll, wait
- **File Operations**: str_replace_based_edit_tool with full CRUD capabilities  
- **Shell Commands**: bash execution with session management
- **Timer System**: Long-running task management with context resumption

### Voice Interaction (Advanced Implementation)
- **Agent Mode**: Alt+D toggles voice input for AI agent queries
- **Dictation Mode**: Hold spacebar for immediate voice-to-text typing
  - Immediate transcription start (0ms delay)
  - 500ms threshold for commitment vs cancellation
  - Smart space passthrough for brief presses

### Hierarchical Agent System (Production Ready)
- **Orchestrator Agent**: Maintains personality and conversation memory
- **Specialist Agents**: Browser, Desktop, File domain experts
- **Tool Delegation**: Intelligent routing based on task analysis
- **Memory Separation**: Persistent orchestrator memory, isolated specialist memory

## Quick Reference

### Critical Development Requirements
- **Always run**: `cargo check --manifest-path src-tauri/Cargo.toml` after Rust changes
- **Tech Stack**: Tauri v2, Rust backend, React/TypeScript frontend  
- **Voice Modes**: Alt+D (agent), spacebar (dictation) with immediate transcription
- **Agent System**: Configurable single or multi-agent with specialized capabilities

### Key Implementation Files
- [src-tauri/src/lib.rs](mdc:src-tauri/src/lib.rs) - Application setup and event handling
- [src-tauri/src/anthropic.rs](mdc:src-tauri/src/anthropic.rs) - Orchestrator agent
- [src-tauri/src/spacebar_monitor.rs](mdc:src-tauri/src/spacebar_monitor.rs) - Intelligent spacebar handling
- [src-tauri/src/agent/tools/anthropic_computer_use.rs](mdc:src-tauri/src/agent/tools/anthropic_computer_use.rs) - Official Computer Use tools
- [src/Bar.tsx](mdc:src/Bar.tsx) - Main floating bar UI with state management

### Platform Requirements
- **macOS**: Accessibility + Screen Recording + Microphone permissions
- **Voice**: Custom Whisper.cpp-based transcription plugin
- **AI Providers**: Multi-provider support (Anthropic, OpenAI, Gemini)

## Development Workflow

1. Make changes file-by-file with proper imports (`crate::`)
2. Run cargo check after Rust modifications (exit code 0 required)
3. Test voice dictation: Alt+D → agent mode, spacebar → immediate typing
4. Use DevToolsPanel for manual testing of computer use functions
5. Keep files under 700 lines when possible

## Architecture Highlights

### Event-Driven Design
- Tauri events for backend→frontend communication
- Immediate visual feedback with smart state management
- Clean separation between voice modes and agent processing

### State Management
- Centralized AppState with Arc<TokioMutex<T>> for shared state
- Proper memory separation between orchestrator and specialists
- Event coordination for voice transcription and agent modes

### Performance Optimizations
- 50ms monitoring intervals for responsive spacebar handling
- Efficient static storage with proper cleanup
- Async patterns with cancellation support

This consolidated structure provides essential guidance for maintaining and extending the production-ready AI Computer Use implementation while eliminating redundancy and maintaining comprehensive coverage of all system components. 

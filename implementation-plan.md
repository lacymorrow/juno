# Juno Implementation Plan

## Project Overview
Juno is a Voice Controlled Computer Use Agent with Multi-Agent Architecture designed to automate computer interactions through voice commands and intelligent task delegation.

## Phase 1: Core Architecture & Voice Integration

### Task 1.1: Project Initialization ✅ DONE
Created Tauri v2 project structure with React frontend and established basic window management.

### Task 1.2: Voice Transcription Engine ✅ DONE
- Implemented Whisper integration for voice-to-text
- Set up audio capture and real-time transcription
- Created voice control module with start/stop functionality
- Integrated with macOS audio APIs

### Task 1.3: Voice Transcription Plugin Integration ✅ DONE
- Extracted voice transcription functionality into a reusable Tauri plugin (tauri-plugin-voice-transcription)
- Created comprehensive plugin API with TypeScript bindings (tauri-plugin-voice-transcription-api)
- Integrated the plugin into the main Juno app, replacing the embedded voice control implementation
- Maintained backward compatibility by rebroadcasting plugin events (dictation-started, partial-result, dictation-finished) as app events
- Added global shortcut handler (Alt+D) for Agent Mode that emits toggle-dictation-request event for frontend handling
- Fixed missing import of `toggleDictation` function in App.tsx from the plugin API package

### Task 1.4: Anthropic Claude Integration ✅ DONE
- Created Anthropic service module with Claude API integration
- Implemented streaming responses and proper error handling
- Set up message formatting for computer use capabilities
- Added browser automation with Playwright integration

### Task 1.5: TTS (Text-to-Speech) Engine ✅ DONE
- Implemented modular TTS system with provider abstraction
- Added system TTS provider using macOS `say` command
- Integrated ElevenLabs API provider with voice selection
- Added Replicate provider for alternative TTS options
- Created unified interface for seamless provider switching

### Task 1.6: Computer Use Tools ✅ DONE
- **Screenshot Tool**: Captures full screen or specific regions
- **Click Tool**: Simulates mouse clicks at coordinates
- **Type Tool**: Types text with proper keyboard simulation
- **Scroll Tool**: Scrolls windows and elements
- **Key Tool**: Simulates keyboard shortcuts and key presses
- **Cursor Position Tool**: Gets current mouse position
- **Window Management**: List, focus, and manipulate windows
- **Application Control**: Open/close applications
- **Clipboard Operations**: Read/write clipboard content
- **File Operations**: Basic file read/write capabilities
- All tools integrated with coordinate transformation for accurate interactions

### Task 1.7: Provider System Implementation ✅ DONE
- Created flexible provider system for AI model selection
- Implemented Anthropic provider with Claude 3.5 Sonnet
- Added Ollama provider for local model support
- Integrated xAI provider with Grok models
- Implemented OpenRouter provider for multiple model access
- Created settings management with API key storage
- Added provider configuration UI in DevToolsPanel

### Task 1.8: MCP (Message Control Protocol) Server Setup
- Set up two MCP servers:
  1. **MCP OS-Level Server** (Port 3000): Handles system-level operations for Anthropic's computer use
  2. **Screen Capture MCP Server** (Port 5173): Provides screenshot capabilities

### Task 1.9: Multi-Agent Architecture Implementation ✅ DONE
**Core Agent Infrastructure:**
- Added core agent traits (`AgentBrain`, `MemoryManager`, `ToolProvider`, `AgentRunnable`)
- Implemented comprehensive error handling with `AgentError` enum
- Added state management with `AgentState` and `AgentAction` enums

**Specialized Agent Framework:**
- `src-tauri/src/agents/base_agent.rs` - Abstract base for all agents ✅
- `src-tauri/src/agents/agent_factory.rs` - Factory for creating agents ✅
- Added SpecializedAgent trait with capabilities and confidence scoring
- Implemented AgentType enum (Browser, Desktop, System, Orchestrator)
- Created Task and TaskResult structures for task delegation
- Built AgentRegistry with intelligent agent selection

**Specialized Agents:**
- **Browser Agent** (`browser_agent.rs`): Web navigation, content extraction, automation ✅
- **Desktop Agent** (`desktop_agent.rs`): Native app interaction, window management, input automation ✅  
- **System Agent** (`system_agent.rs`): Shell commands, file operations, process management ✅
- **Orchestrator** (`orchestrator.rs`): Central coordinator for task delegation ✅

## Phase 2: User Interface Development

### Task 2.1: Floating Command Bar ✅ DONE
- Implemented expanding/collapsing floating bar with animations
- Added click-through functionality when collapsed
- Integrated voice input indicator with real-time feedback
- Added visual states for listening, processing, and speaking
- Implemented drag functionality and window management

### Task 2.2: Settings Window
- Design and implement settings UI
- Add voice model selection
- Configure TTS preferences
- Manage API keys securely
- Theme and appearance options

### Task 2.3: System Tray Integration ✅ DONE
- Added system tray icon with menu
- Implemented quick access to main functions
- Added toggle for floating bar visibility
- Created quit application option

### Task 2.4: Chat Interface ✅ DONE
- Created main window with conversation display
- Implemented message rendering with role-based styling
- Added screenshot display in conversation
- Integrated tool usage visualization
- Added thinking process display
- Created resizable developer panel

## Phase 3: Advanced Features

### Task 3.1: Context Awareness
- Implement screen content analysis
- Add application context detection
- Create smart command suggestions
- Improve command interpretation

### Task 3.2: Macro Recording
- Add ability to record action sequences
- Create macro playback system
- Implement macro editing interface
- Add conditional logic support

### Task 3.3: Plugin System Enhancement
- Design enhanced plugin architecture
- Create plugin API documentation
- Implement plugin loader
- Add example plugins

### Task 3.4: Multi-Monitor Support
- Detect multiple displays
- Add monitor selection in tools
- Coordinate transformation for different screens
- Window movement across monitors

## Phase 4: Testing and Optimization

### Task 4.1: Comprehensive Testing ✅ DONE
- Created extensive unit tests for tools
- Implemented integration tests for commands
- Added QA test commands for voice transcription
- Created test scripts for TTS providers
- Implemented automated test runner

### Task 4.2: Performance Optimization
- Optimize Whisper model loading
- Implement audio processing improvements
- Add response caching
- Reduce memory footprint

### Task 4.3: Error Handling ✅ DONE
- Implemented comprehensive error handling
- Added graceful degradation
- Created error reporting system
- Added retry mechanisms for API calls

### Task 4.4: Documentation
- Write user guide
- Create API documentation
- Add inline code documentation
- Create video tutorials

## Phase 5: Deployment

### Task 5.1: Build Pipeline
- Set up GitHub Actions
- Configure code signing
- Create release automation
- Add update mechanism

### Task 5.2: Distribution
- Prepare macOS app bundle
- Set up auto-updater
- Create installer
- Submit to distribution platforms

### Task 5.3: User Feedback
- Implement analytics (privacy-respecting)
- Add crash reporting
- Create feedback mechanism
- Set up user support system

## Current Status
- Phase 1: 95% Complete (Task 1.8 remaining)
- Phase 2: 75% Complete (Task 2.2 remaining)
- Phase 3: 0% Complete
- Phase 4: 50% Complete
- Phase 5: 0% Complete

## Next Steps
1. Complete MCP server setup (Task 1.8)
2. Create settings window UI (Task 2.2)
3. Begin Phase 3 advanced features
4. Continue performance optimization

## Technical Decisions Made
1. **Tauri v2**: Chosen for native performance and security
2. **Whisper**: Selected for offline voice transcription
3. **Anthropic Claude**: Primary AI model for computer use
4. **Playwright**: Browser automation for web interactions
5. **React + TypeScript**: Frontend framework for UI
6. **Rust**: Backend language for system interactions
7. **Voice Transcription Plugin**: Modular architecture for reusability
8. **Multi-Agent Architecture**: Specialized agents for different task categories

## Architectural Highlights

### Multi-Agent System
- **Orchestrator**: Intelligently delegates tasks to specialized agents
- **Browser Agent**: Handles web automation and content extraction
- **Desktop Agent**: Manages native application interactions
- **System Agent**: Executes shell commands and file operations
- **Agent Factory**: Centralized agent creation and management

### Voice Integration
- **Plugin Architecture**: Reusable voice transcription plugin
- **Real-time Processing**: Streaming audio capture and transcription
- **Event System**: Plugin events rebroadcast for compatibility
- **Global Shortcuts**: Alt+D for Agent Mode activation, configurable key for Dictation Mode

## Known Limitations
1. Currently macOS only (Windows/Linux support planned)
2. Requires local Whisper model download
3. Some tools require accessibility permissions
4. Browser automation requires separate browser instance 

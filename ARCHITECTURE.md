# Architecture Overview

## System Design

Juno is a production-ready Tauri v2 desktop application implementing a complete AI Computer Use agent with hierarchical architecture and advanced voice integration.

```
┌─────────────────┐    ┌─────────────────┐    ┌─────────────────┐
│   Frontend      │    │   Backend       │    │   Platform      │
│   React/TS      │◄──►│   Rust/Tauri    │◄──►│   macOS APIs    │
│   - Floating UI │    │   - Agents      │    │   - Automation  │
│   - Chat        │    │   - Tools       │    │   - Voice       │
│   - Settings    │    │   - State       │    │   - Browser     │
└─────────────────┘    └─────────────────┘    └─────────────────┘
```

## Hierarchical Agent System ✅

### Architecture Overview
```
┌─────────────────────────────────────────────────────────────┐
│                    Orchestrator Agent                      │
│  - Personality & Memory Management                         │
│  - Task Analysis & Delegation                             │
│  - Uses: delegate_to_*_agent tools                        │
└─────────────────┬───────────────────────────────────────────┘
                  │
    ┌─────────────┼─────────────┬─────────────────────────────┐
    │             │             │                             │
┌───▼────┐   ┌───▼────┐   ┌────▼────┐   ┌─────────────────────▼─┐
│Browser │   │Desktop │   │  File   │   │    Tool Providers     │
│ Agent  │   │ Agent  │   │ Agent   │   │  - Shared Resources   │
│        │   │        │   │         │   │  - Lazy Init          │
│ Web    │   │ UI     │   │ Code    │   │  - Browser Controller │
│ Auto   │   │ Auto   │   │ Ops     │   │  - System APIs        │
└────────┘   └────────┘   └─────────┘   └───────────────────────┘
```

### Core Components

#### Orchestrator (`src-tauri/src/anthropic.rs`)
- **Role**: Central coordinator with personality and conversational memory
- **Memory**: Persistent AppState memory manager for conversation history
- **Tools**: Delegation tools only (`delegate_to_browser_agent`, `delegate_to_desktop_agent`, `delegate_to_file_agent`)
- **Flow**: Analyzes tasks → Delegates to specialists → Coordinates responses

#### Specialist Agents
- **Browser Agent**: Web navigation, content extraction, page interaction
- **Desktop Agent**: Native app interaction, window management, input automation
- **File Agent**: Code editing, terminal operations, file system management
- **Memory**: Isolated `SimpleMemoryManager::new()` for task-specific context
- **Tools**: Domain-specific tool suites optimized for their expertise

#### Tool Providers
- **Shared Resources**: Browser controller, AI providers, system APIs
- **Lazy Initialization**: Expensive resources loaded on first use
- **Thread Safety**: Arc-based sharing for concurrent access

## Data Flow

### User Interaction Flow
```
User Input → Frontend → Tauri Command → Agent Selection → Tool Execution → Response
     ↓
┌─ Voice Mode (Alt+D): Voice → Transcription → Agent Processing
└─ Dictation Mode: Voice → Transcription → Direct Text Insertion
```

### Agent Execution Flow
```
1. submit_query() called
2. Escape key registered for cancellation
3. Memory manager cloned (Arc-based)
4. Agent brain initialized via BrainFactory
5. Tools registered with LocalToolProvider
6. Agent runs with max 15 iterations
7. Results returned, escape key unregistered
```

### Tool Execution Loop
```
Think Phase: AI analyzes context → Plans next action
  ↓
Act Phase: Tool selected → Parameters validated → Execution
  ↓
Memory Update: Tool results added to conversation history
  ↓
Iteration Check: Continue (< 15) or Complete
```

## State Management

### AppState Structure (`src-tauri/src/state.rs`)
```rust
pub struct AppState {
    memory_manager: Arc<TokioMutex<SimpleMemoryManager>>,
    browser_controller: Arc<TokioMutex<Option<BrowserController>>>,
    cancellation_token: Arc<TokioMutex<Option<CancellationToken>>>,
    agent_mode: Arc<TokioMutex<AgentMode>>,
    // ... other shared state
}
```

### Memory Architecture
- **Orchestrator Memory**: Persistent conversation history in AppState
- **Specialist Memory**: Fresh memory managers for task isolation
- **Thread Safety**: Arc<TokioMutex<T>> for shared mutable state
- **Cleanup**: Automatic memory pruning for context window management

## Voice System Architecture

### Dual Mode Design
```
┌─────────────────┐    ┌─────────────────┐
│   Agent Mode    │    │ Dictation Mode  │
│   Alt+D         │    │ Configurable    │
│                 │    │ Key (spacebar)  │
│ Voice → AI      │    │ Voice → Text    │
│ Processing →    │    │ Insertion       │
│ Computer Actions│    │                 │
└─────────────────┘    └─────────────────┘
```

### Voice Pipeline
```
Audio Capture → Whisper.cpp → Transcription → Mode Routing
                                    ↓
                        ┌───────────┴───────────┐
                        │                       │
                  Agent Mode              Dictation Mode
                (AI Processing)         (Direct Insertion)
```

### Plugin Architecture
- **Core**: `tauri-plugin-voice-transcription/` - Whisper.cpp integration
- **API**: TypeScript bindings for frontend integration
- **Events**: Plugin events rebroadcast for app compatibility
- **Global Shortcuts**: Dynamic registration/unregistration

## Frontend Architecture

### Component Structure
```
App.tsx (Main Window)
├── components/
│   └── FloatingBar.tsx (Floating Interface)
├── Chat Interface
│   ├── Message Display
│   ├── Screenshot Rendering
│   └── Tool Visualization
└── Settings Panel
    ├── Provider Configuration
    ├── Voice Mode Settings
    └── API Key Management
```

### Event System
- **Backend → Frontend**: Tauri events for real-time updates
- **Debouncing**: 100ms for rapid event sequences
- **Cleanup**: Proper event listener cleanup in useEffect
- **State Separation**: UI state independent from backend state

## Tool System

### Tool Categories
```
Desktop Tools (commands/*.rs)
├── Screenshot: capture_screenshot, capture_element_screenshot
├── Mouse: click, drag, move, position detection
├── Keyboard: type, key combinations, hold/release
├── Window: management, focus, application control
└── Advanced: scrolling, clipboard, accessibility

Browser Tools (tools/browser_tools.rs)
├── Navigation: browser_navigate, browser_screenshot
├── Content: browser_extract_content, element interaction
├── State: URL tracking, session management
└── Automation: form filling, clicking, scrolling

Voice Tools
├── TTS: Multiple providers (ElevenLabs, system, Replicate)
├── Transcription: Real-time Whisper.cpp processing
└── Events: Plugin events for compatibility

Timer Tools (agent/tools/timer_tools.rs)
├── Context Preservation: screen, file, app monitoring
├── Resume Capability: automatic context restoration
└── Long-running Tasks: background execution with monitoring
```

### Tool Registration Pattern
```rust
// Tool definition with AI-readable schema
ToolDefinition {
    name: "tool_name".to_string(),
    description: "Clear description for AI understanding".to_string(),
    input_schema: JsonSchema
}

// Async executor implementation
let executor = move |input: Value| async move {
    // Implementation logic
};

// Registration with provider
tool_provider.register_async_tool(definition, executor).await;
```

## Platform Integration

### macOS APIs
- **Accessibility**: Full accessibility tree navigation
- **Screen Capture**: High-resolution screenshot capabilities
- **Input Simulation**: Mouse and keyboard automation
- **Application Control**: Launch, focus, window management
- **System Context**: Current app, window, and screen information

### Browser Integration
- **Engine**: Playwright-based browser automation
- **Lazy Loading**: Browser controller initialized on first use
- **Session Management**: Persistent browser sessions
- **Content Extraction**: Text, images, and structure parsing

## Security & Permissions

### macOS Permission Requirements
- **Accessibility**: Required for UI automation and input simulation
- **Screen Recording**: Required for screenshot capabilities
- **Microphone**: Required for voice transcription
- **Full Disk Access**: Optional for enhanced file operations

### Security Patterns
- **Graceful Degradation**: Continue operation with reduced capabilities
- **Permission Checking**: Validate permissions before operations
- **Error Handling**: Never terminate app on permission failures
- **User Guidance**: Provide clear instructions for permission setup

## Error Handling Architecture

### Error Hierarchy
```rust
pub enum AgentError {
    Terminated,                // User cancellation via escape key
    MaxStepsReached,          // Iteration limit reached
    ToolNotFound,             // Invalid tool request
    ProviderError(String),    // AI provider failure
    ToolExecutionError(String), // Tool execution failure
}
```

### Recovery Strategies
- **Tool Failures**: Retry with error context
- **Provider Failures**: Fallback to alternative providers
- **Resource Issues**: Cleanup and reinitialize
- **User Cancellation**: Graceful termination with cleanup

## Performance Architecture

### Optimization Strategies
- **Lazy Initialization**: Load expensive resources on demand
- **Arc-based Sharing**: Efficient memory management
- **Context Pruning**: Automatic conversation history cleanup
- **Event Debouncing**: Prevent UI event flooding
- **Background Execution**: Long-running tasks with progress updates

### Monitoring & Observability
- **Structured Logging**: tracing crate with appropriate levels
- **Performance Metrics**: Tool execution times and memory usage
- **Health Checks**: System capability validation
- **Debug Information**: Comprehensive debug logging for development

This architecture provides a robust, scalable foundation for AI-driven computer automation with advanced voice integration and intelligent task orchestration.
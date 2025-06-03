# Architecture Overview

## System Design

```
┌─────────────────┐    ┌─────────────────┐    ┌─────────────────┐
│   React UI      │    │   Tauri Core    │    │  Agent System   │
│  - Floating Bar │◄──►│  - Commands     │◄──►│  - AI Brain     │
│  - Main Window  │    │  - State Mgmt   │    │  - Tools        │
│  - Voice Input  │    │  - Event System │    │  - Memory       │
└─────────────────┘    └─────────────────┘    └─────────────────┘
                              │
                              ▼
                    ┌─────────────────┐
                    │   macOS APIs    │
                    │  - Accessibility│
                    │  - Screenshots  │
                    │  - Input Events │
                    └─────────────────┘
```

## Core Layers

### 1. Frontend (React/TypeScript)
- **Location**: `src/`
- **Purpose**: User interface and interaction
- **Key Files**:
  - `src/components/ui/kibo-ui/ai/` - AI interface components
  - `src/App.tsx` - Main application component

### 2. Tauri Bridge (Rust)
- **Location**: `src-tauri/src/`
- **Purpose**: Cross-platform bridge and command handling
- **Key Modules**:
  - `lib.rs` - App initialization and global shortcuts
  - `commands/` - Tauri command handlers
  - `state.rs` - Global application state

### 3. Agent System (Rust)
- **Location**: `src-tauri/src/agent/`
- **Purpose**: AI-driven automation engine
- **Components**:
  - `providers/` - AI provider implementations (Anthropic, OpenAI, etc.)
  - `implementations/` - Agent runners and memory managers
  - `tools/` - Tool definitions and executors

### 4. Platform Integration (Rust)
- **Location**: `src-tauri/src/commands/`, `src-tauri/src/tools/`
- **Purpose**: macOS system integration
- **Capabilities**: Desktop automation, browser control, file operations

## Data Flow

### Agent Execution
1. **Input**: User query via React UI
2. **Routing**: Tauri command (`submit_query` or `submit_orchestrated_query`)
3. **Processing**: Agent brain processes with tool access
4. **Execution**: Tools perform system actions
5. **Response**: Results emitted back to frontend

### State Management
- **AppState**: Singleton managing global state
- **Cancellation**: Signal-based execution control
- **Memory**: Conversation history and context preservation
- **Browser State**: Lazy-initialized browser controller

### Event System
- **Tauri Events**: Bidirectional communication between frontend/backend
- **Global Shortcuts**: System-wide key bindings (Escape, Alt+D)
- **Voice Events**: Speech recognition state changes

## Key Design Patterns

### 1. Factory Pattern
- **BrainFactory**: Creates AI provider instances
- **AgentFactory**: Initializes agent components
- **ToolProvider**: Manages tool registration

### 2. Command Pattern
- **Tauri Commands**: Structured API endpoints
- **Tool Executors**: Async function wrappers
- **Agent Actions**: Discrete automation steps

### 3. Observer Pattern
- **Event Emitters**: State change notifications
- **Cancellation Signals**: Execution control
- **Progress Updates**: Real-time feedback

## Security Model

### Sandboxing
- **Tauri Security**: Capability-based permissions
- **API Isolation**: Controlled system access
- **Input Validation**: Parameter sanitization

### Resource Management
- **Browser Lifecycle**: Controlled creation/cleanup
- **File System**: Restricted access patterns
- **Network**: API key protection

## Extension Points

### Tool Development
- **Tool Trait**: Unified interface for new capabilities
- **Registration**: Dynamic tool addition
- **Async Execution**: Non-blocking operation support

### AI Providers
- **Provider Interface**: Pluggable AI backends
- **Configuration**: Runtime provider switching
- **Fallback**: Multi-provider resilience

### Platform Support
- **Abstraction Layer**: Cross-platform compatibility
- **Feature Detection**: Platform-specific capabilities
- **Graceful Degradation**: Unsupported feature handling 

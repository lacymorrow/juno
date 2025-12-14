# Juno Architecture Overview

## Executive Summary

Juno is a production-ready AI Computer Use Agent that enables autonomous desktop automation through natural language commands. Built on Tauri v2 with Rust backend and React frontend, it provides native macOS integration while maintaining cross-platform capabilities.

**Key Differentiators:**
- **Native Desktop Integration**: Direct OS-level automation without VMs or containers
- **Multi-Model AI Support**: 100+ AI models through unified interface
- **Advanced Security**: Process-level sandboxing with granular permissions
- **Enterprise-Ready**: Multi-user sessions, audit logging, reproducible environments

## System Architecture

```
┌─────────────────────────────────────────────────────────────┐
│                     Frontend (React/TypeScript)              │
│  ┌──────────────┐ ┌──────────────┐ ┌──────────────┐        │
│  │ Chat UI      │ │ Settings     │ │ Dashboard    │        │
│  └──────────────┘ └──────────────┘ └──────────────┘        │
└─────────────────────────┬───────────────────────────────────┘
                          │ Tauri IPC Bridge
┌─────────────────────────▼───────────────────────────────────┐
│                     Backend (Rust/Tauri v2)                  │
│  ┌──────────────────────────────────────────────────────┐  │
│  │              Orchestrator Layer                       │  │
│  │  ┌──────────┐ ┌──────────┐ ┌──────────────┐        │  │
│  │  │ Anthropic│ │ Agent    │ │ Memory       │        │  │
│  │  │ Handler  │ │ Runner   │ │ Manager      │        │  │
│  │  └──────────┘ └──────────┘ └──────────────┘        │  │
│  └──────────────────────────────────────────────────────┘  │
│  ┌──────────────────────────────────────────────────────┐  │
│  │              Model Zoo (AI Providers)                 │  │
│  │  ┌──────────┐ ┌──────────┐ ┌──────────┐ ┌────────┐ │  │
│  │  │Anthropic │ │ OpenAI   │ │ Google   │ │ Local  │ │  │
│  │  │ Claude   │ │ GPT-4o   │ │ Gemini   │ │ Ollama │ │  │
│  │  └──────────┘ └──────────┘ └──────────┘ └────────┘ │  │
│  └──────────────────────────────────────────────────────┘  │
│  ┌──────────────────────────────────────────────────────┐  │
│  │           Computer Use Commands                       │  │
│  │  ┌──────────┐ ┌──────────┐ ┌──────────┐ ┌────────┐ │  │
│  │  │ Mouse    │ │ Keyboard │ │ Screen   │ │ Window │ │  │
│  │  │ Control  │ │ Input    │ │ Capture  │ │ Mgmt   │ │  │
│  │  └──────────┘ └──────────┘ └──────────┘ └────────┘ │  │
│  └──────────────────────────────────────────────────────┘  │
│  ┌──────────────────────────────────────────────────────┐  │
│  │              Security & Sandboxing                    │  │
│  │  ┌──────────┐ ┌──────────┐ ┌──────────────┐        │  │
│  │  │ Session  │ │ Sandbox  │ │ Unrestricted │        │  │
│  │  │ Manager  │ │ Engine   │ │ Mode         │        │  │
│  │  └──────────┘ └──────────┘ └──────────────┘        │  │
│  └──────────────────────────────────────────────────────┘  │
└─────────────────────────┬───────────────────────────────────┘
                          │
┌─────────────────────────▼───────────────────────────────────┐
│                  Operating System (macOS)                    │
│  ┌──────────────┐ ┌──────────────┐ ┌──────────────┐        │
│  │ Accessibility│ │ Core Graphics│ │ Apple Events │        │
│  │ API          │ │ API          │ │ API          │        │
│  └──────────────┘ └──────────────┘ └──────────────┘        │
└──────────────────────────────────────────────────────────────┘
```

## Core Components

### 1. Frontend Layer (`/src`)

**Technology Stack:** React, TypeScript, Tailwind CSS, Vite

**Key Components:**
- `App.tsx`: Main application entry point
- `components/chat/`: Chat interface for AI interactions
- `components/settings/`: Configuration management UI
- `components/dashboard/`: System monitoring and controls
- `lib/hooks/`: Custom React hooks for state management
- `lib/services/`: API service layer for backend communication

### 2. Backend Layer (`/src-tauri`)

**Technology Stack:** Rust, Tauri v2, Tokio, computer-use-ai-sdk

**Core Modules:**

#### 2.1 Orchestrator System (`src/anthropic.rs`)
- **Purpose:** Central command processing and agent coordination
- **Key Functions:**
  - `submit_query()`: Main entry point for AI queries
  - `submit_orchestrated_query()`: Multi-agent task delegation
  - Memory persistence across sessions
  - Tool registration and execution

#### 2.2 Agent System (`src/agent/`)
```
agent/
├── implementations/       # Core agent logic
│   ├── brain.rs          # AI model integration
│   ├── runner.rs         # Execution engine
│   └── memory.rs         # Context management
├── providers/            # AI provider implementations
│   ├── anthropic.rs
│   ├── openai.rs
│   └── local.rs
├── tools/               # Tool definitions
│   └── registry.rs      # Tool registration system
└── model_zoo/           # Multi-model support (NEW)
    ├── providers/       # Provider implementations
    │   ├── anthropic.rs
    │   ├── openai.rs
    │   └── google.rs
    ├── local_models/    # Local model support
    │   ├── ollama.rs
    │   └── huggingface.rs
    └── composed_agents/ # UI grounding + planning
```

#### 2.3 Computer Use Commands (`src/commands/`)
```
commands/
├── computer.rs          # Unified computer API (NEW)
├── mouse.rs            # Mouse automation
├── keyboard.rs         # Keyboard input
├── core.rs            # Screenshot capture
├── window.rs          # Window management
├── filesystem.rs      # File operations
├── shell.rs           # Shell command execution
└── unrestricted.rs    # Unrestricted mode (NEW)
```

#### 2.4 Security Framework
```
├── session/           # Multi-user sessions (NEW)
│   ├── mod.rs        # Session management
│   ├── user.rs       # User authentication
│   └── permissions.rs # Role-based access
├── sandbox/          # Process isolation (NEW)
│   ├── mod.rs       # Sandbox orchestration
│   ├── workspace.rs # Isolated workspaces
│   └── process_isolation.rs
└── state.rs         # Application state with security
```

### 3. Platform Integration

#### macOS-Specific APIs
- **Accessibility API**: UI element detection and manipulation
- **Core Graphics**: Screen capture and mouse control
- **Apple Events**: Application scripting
- **App Sandbox**: Security entitlements

#### Cross-Platform Support
- Windows: Win32 API, UI Automation
- Linux: X11/Wayland, AT-SPI

## Key Features Implementation

### 1. Multi-Model AI Support (Model Zoo)

**Architecture:**
```rust
pub trait ModelInterface {
    async fn generate(prompt, images) -> Result<String>;
    async fn stream_generate(prompt) -> Result<Receiver<String>>;
    fn supports_vision() -> bool;
    fn supports_tools() -> bool;
    fn get_context_window() -> usize;
}
```

**Supported Providers:**
- **Anthropic**: Claude 3.5 Sonnet, Haiku, Opus
- **OpenAI**: GPT-4o, GPT-4 Turbo, o1 models
- **Google**: Gemini 2.0 Flash, Gemini 1.5 Pro
- **Local**: Ollama (Llama, Qwen, Mistral), HuggingFace models

**Composed Agents:**
- OmniParser + Claude: Visual UI understanding with reasoning
- UI-TARS + GPT-4o: Specialized UI automation
- Moondream + Gemini: Lightweight visual grounding

### 2. Computer Use API

**Unified Interface:**
```rust
pub struct ComputerInput {
    pub action: String,        // screenshot, click, type, scroll, etc.
    pub coordinate: Option<Vec<f64>>,
    pub text: Option<String>,
    pub scroll_count: Option<i32>,
    pub duration: Option<u64>,
}
```

**Action Types:**
- **Visual**: screenshot, screen recording
- **Mouse**: left/right/middle click, double/triple click, drag, move
- **Keyboard**: type text, press keys, key combinations
- **Window**: scroll, resize, focus, minimize/maximize
- **System**: wait, execute commands (unrestricted mode)

### 3. Security & Sandboxing

**Isolation Levels:**
```rust
pub enum IsolationLevel {
    None,           // No isolation (dev mode)
    Basic,          // Process isolation
    Strict,         // Limited permissions
    Educational,    // Safe mode for training
}
```

**Platform-Specific Implementation:**
- **macOS**: App Sandbox profiles, entitlements
- **Windows**: AppContainers, integrity levels
- **Linux**: Namespaces, seccomp-bpf

**Unrestricted Mode:**
- Bypass all rate limiting
- Direct system access
- Admin command execution
- Full filesystem access
- Audit logging for compliance

### 4. Session Management

**Multi-User Support:**
```rust
pub struct Session {
    pub id: String,
    pub user_id: String,
    pub workspace_id: String,
    pub permissions: SessionPermissions,
    pub isolation: IsolationConfig,
}
```

**Features:**
- Concurrent user sessions
- Role-based access control
- Isolated workspaces
- Session persistence
- Activity tracking

### 5. Visual Grounding (SOM)

**Set-of-Mark System:**
```rust
pub struct SOMResult {
    pub elements: Vec<UIElement>,
    pub text_regions: Vec<TextRegion>,
    pub annotated_image: Option<Vec<u8>>,
}
```

**Components:**
- YOLO-based UI element detection
- OCR for text extraction
- Accessibility API integration
- Element relationship mapping

## Data Flow

### 1. Command Processing Flow
```
User Input → Frontend → Tauri IPC → Orchestrator
    ↓
Agent Selection → Tool Registration → Model Selection
    ↓
Tool Execution → OS APIs → Result Processing
    ↓
Response → Frontend → User Display
```

### 2. Security Check Flow
```
Command Request → Permission Check → Rate Limiting
    ↓
Sandbox Validation → Workspace Boundary → Audit Log
    ↓
Execute or Reject → Log Result
```

## Configuration

### Environment Variables
```bash
ANTHROPIC_API_KEY      # Claude API access
OPENAI_API_KEY         # OpenAI models
GOOGLE_API_KEY         # Gemini models
UNRESTRICTED_MODE      # Enable full access
SESSION_TIMEOUT        # Session duration
```

### Security Policies (`security_config.toml`)
```toml
[sandbox]
level = "strict"
filesystem_access = ["~/Documents", "/tmp"]
network_access = false

[rate_limits]
screenshots_per_minute = 10
commands_per_minute = 30
```

## Performance Considerations

### Optimization Strategies
1. **Lazy Loading**: Models loaded on-demand
2. **Connection Pooling**: Reuse HTTP clients
3. **Arc-based Memory**: Efficient sharing across threads
4. **Streaming Responses**: Reduce latency for long outputs
5. **Caching**: Screenshot and model response caching

### Resource Limits
- Max memory per session: 2GB
- Max CPU per process: 50%
- Max concurrent sessions: 10
- Screenshot cache: 100MB

## Development Workflow

### Building
```bash
# Development
bun run tauri dev

# Production
bun run tauri build

# Testing
cargo test --manifest-path src-tauri/Cargo.toml
```

### Adding New Features

1. **New AI Model:**
   - Implement `ModelInterface` trait
   - Add to `ModelFactory::create()`
   - Register in `ModelZoo`

2. **New Computer Action:**
   - Add to `ComputerInput` enum
   - Implement in `execute_computer_action()`
   - Add security checks

3. **New Tool:**
   - Define in `agent/tools/`
   - Register in tool provider
   - Add to agent capabilities

## Security Considerations

### Threat Model
- **Malicious Commands**: Validated through whitelisting
- **Path Traversal**: Canonical path validation
- **Resource Exhaustion**: Rate limiting and quotas
- **Privilege Escalation**: Sandbox boundaries
- **Data Exfiltration**: Network isolation options

### Best Practices
1. Never run in unrestricted mode in production
2. Implement audit logging for all actions
3. Use session timeouts
4. Regular security updates
5. Principle of least privilege

## Future Roadmap

### Planned Features
- [ ] Cloud sync for multi-device support
- [ ] Plugin system for custom tools
- [ ] Advanced visual debugging
- [ ] Workflow recording and playback
- [ ] Team collaboration features

### Technical Debt
- [ ] Complete YOLO/OCR implementation
- [ ] Optimize memory usage for large models
- [ ] Implement distributed execution
- [ ] Add comprehensive test coverage

## Appendix

### File Structure
```
dotdot/
├── src/                 # Frontend code
├── src-tauri/          # Backend code
│   ├── src/           # Rust source
│   ├── Cargo.toml     # Dependencies
│   └── CLAUDE.md      # AI guidance
├── docs/              # Documentation
├── tests/             # Test suites
└── ARCHITECTURE.md    # This file
```

### Key Dependencies
- **Tauri v2**: Desktop app framework
- **computer-use-ai-sdk**: OS automation
- **Tokio**: Async runtime
- **Reqwest**: HTTP client
- **Serde**: Serialization

### References
- [Tauri Documentation](https://v2.tauri.app)
- [Anthropic Computer Use](https://docs.anthropic.com/computer-use)
- [Rust Async Book](https://rust-lang.github.io/async-book)
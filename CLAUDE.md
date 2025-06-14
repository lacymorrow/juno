# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

Juno is a production-ready Tauri v2 desktop application implementing Anthropic's Computer Use Bot for macOS automation. It features AI-powered desktop automation with voice control, multi-agent orchestration, and comprehensive system integration.

## Development Commands

### Build and Test Commands
```bash
bun install                              # Install dependencies
cargo check --manifest-path src-tauri/Cargo.toml  # CRITICAL: Run after every Rust change
bun run tauri dev                        # Development mode
bun run build                            # Build frontend
bun run tauri build                      # Build app
./run-all-tests.sh                       # Full test suite (frontend + backend)
npm test                                 # Frontend tests only (Vitest)
npm run test:watch                       # Frontend tests in watch mode
cargo test --manifest-path src-tauri/Cargo.toml  # Rust tests only
```

### Development Utilities
```bash
bun run tauri:dev:multi                  # Multi-instance development
./scripts/bump-version.sh <version>      # Automated version bumping
./test-rust-units.sh                     # Rust unit tests
./test-qa.sh                             # QA functional tests
```

## Architecture

### Hierarchical Agent System

```
Orchestrator (src-tauri/src/anthropic.rs)
├── Desktop Agent (screen interaction, accessibility)
├── Browser Agent (web automation) 
├── File Agent (filesystem operations)
└── Shared Tool Providers (lazy initialization)
```

**Key Principles:**
- **Orchestrator**: Uses persistent AppState memory, gets delegation tools only
- **Specialists**: Use fresh SimpleMemoryManager instances, get domain-specific tools
- **Memory**: Arc-based cloning for thread safety, automatic context pruning
- **Tools**: Lazy initialization for expensive resources (browser controller, AI providers)

### Core Components

- **Frontend**: React/TypeScript with Tauri v2 (`src/App.tsx`, `src/components/`)
- **Backend**: Rust with async/await patterns (`src-tauri/src/`)
- **Agent System**: Multi-agent orchestration (`src-tauri/src/agent/`)
- **Voice System**: Custom Whisper-based plugin (`tauri-plugin-voice-transcription/`)
- **Platform Integration**: macOS APIs (`src-tauri/mcp-server-os-level/src/platforms/macos/`)
- **State Management**: AppState with Arc<TokioMutex<T>> patterns (`src-tauri/src/state.rs`)

### Voice Modes

- **Agent Mode**: Alt+D → Voice → AI Processing → Computer Actions
- **Dictation Mode**: Configurable key → Voice → Direct text insertion

## Development Guidelines

### Critical Requirements

1. **Compilation Check**: Always run `cargo check --manifest-path src-tauri/Cargo.toml` after Rust changes
2. **Error Handling**: Use `AgentError` enum, never `std::process::exit()`
3. **Memory Management**: Clone memory managers safely (Arc-based), use proper async patterns
4. **Security**: All tools implement security validation (see `src-tauri/src/agent/tools/basic_tools.rs`)
5. **Permissions**: Never terminate app on macOS permission failures, implement graceful degradation

### Tool Development Pattern

```rust
// 1. Define tool with clear AI-readable description
ToolDefinition {
    name: "tool_name".to_string(),
    description: "Clear description for AI understanding".to_string(),
    input_schema: serde_json::json!({
        "type": "object",
        "properties": { "param": {"type": "string"} },
        "required": ["param"]
    })
}

// 2. Implement async executor with proper error handling
let executor = move |input: Value| async move {
    let param = input["param"].as_str()
        .ok_or("Missing required parameter")?;
    let result = perform_action(param).await?;
    Ok(serde_json::json!({"success": true, "result": result}))
};

// 3. Register with tool provider
tool_provider.register_async_tool(definition, executor).await;
```

### Testing Strategy

- **Frontend**: Vitest with Testing Library, jsdom environment, comprehensive mocking
- **Backend**: Cargo test with tokio-test, async patterns, serde validation
- **Integration**: Full end-to-end testing with `./run-all-tests.sh`
- **Coverage**: >90% for utilities, >80% for components, 100% for critical paths

### Security Framework

**Production vs Development Modes:**
- Development: Relaxed security for workflow (`cfg!(debug_assertions)`)
- Production: Strict validation, path sandboxing, command whitelisting

**File Operations Security:**
- Path traversal prevention, workspace-only access
- File extension validation (txt, md, rs, js, ts, json, etc.)
- Size limits: 10MB production, 50MB development

**Command Execution Security:**
- Whitelist enforcement for development tools only
- Dangerous pattern detection and blocking
- Comprehensive audit logging

## Project Structure

### Key Directories

- `src/` - React/TypeScript frontend
- `src-tauri/src/` - Rust backend implementation
- `src-tauri/src/agent/` - Multi-agent system core
- `src-tauri/src/commands/` - Tauri command handlers
- `src-tauri/mcp-server-os-level/` - macOS platform integration
- `tauri-plugin-voice-transcription/` - Voice transcription plugin
- `docs/` - Comprehensive documentation
- `scripts/` - Development utilities

### Important Files

- `src-tauri/src/anthropic.rs` - Main agent orchestrator entry point
- `src-tauri/src/state.rs` - Application state management
- `src-tauri/src/agent/implementations/` - Agent implementations
- `src-tauri/src/agent/tools/` - Tool system implementations
- `src/App.tsx` - Main frontend component
- `LLMs.txt` - Comprehensive AI agent instructions
- `.cursorrules` - Cursor IDE rules and guidelines

## API Keys Required

```env
ANTHROPIC_API_KEY=your_key_here    # Primary AI provider
OPENAI_API_KEY=your_key_here       # Alternative provider  
ELEVENLABS_API_KEY=your_key_here   # Text-to-speech (optional)
```

## Platform Requirements

- macOS with accessibility permissions
- Node.js 18+ and Rust 1.70+
- Microphone access for voice features
- Screen recording permissions for screenshots

## Common Development Patterns

1. **State Access**: Use AppState getters, never direct field access
2. **Async Operations**: Proper Result<T, String> handling throughout
3. **Event System**: Tauri events for backend→frontend communication with debouncing
4. **Resource Cleanup**: Proper cleanup in useEffect returns and Drop implementations
5. **Escape Key**: Dynamic registration only during agent execution
6. **Tool Registration**: Use shared tool providers with lazy initialization
7. **Memory Management**: Arc-based sharing with automatic pruning for context limits

## Debugging

Enable debug logging: `RUST_LOG=debug bun run tauri dev`

Critical debug points:
- Memory operations (add/retrieve messages)
- API request/response payloads
- Tool execution flow and errors
- State synchronization
- Event emission and handling
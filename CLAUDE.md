# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

Juno is a production-ready Tauri v2 desktop application implementing Anthropic's Computer Use Bot for macOS automation. It features AI-powered desktop automation with voice control, multi-agent orchestration, and comprehensive system integration.

## Development Commands

### Build and Test Commands
```bash
bun install                              # Install dependencies
cargo check --manifest-path src-tauri/Cargo.toml  # CRITICAL: Run after every Rust change (NOTE: requires 15m timeout)
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

1. **Compilation Check**: Always run `cargo check --manifest-path src-tauri/Cargo.toml` after Rust changes (NOTE: requires 15m timeout)
2. **Error Handling**: Use `AgentError` enum, never `std::process::exit()`
3. **Memory Management**: Clone memory managers safely (Arc-based), use proper async patterns
4. **Security**: All tools implement security validation (see `src-tauri/src/agent/tools/basic_tools.rs`)
5. **Permissions**: Never terminate app on macOS permission failures, implement graceful degradation
6. **Memory Safety**: NEVER use `.unwrap()` in production code - use proper error handling patterns

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

## Rate Limiting System

### Overview

Juno implements a comprehensive token bucket-based rate limiting system to prevent abuse and ensure system stability. The rate limiter protects against:
- API abuse (expensive AI operations)
- Resource exhaustion attacks
- Shell command injection attempts
- Screenshot flooding
- Browser automation abuse

### Default Rate Limits

```rust
// src-tauri/src/utils/rate_limiter.rs
GlobalRateLimiters {
    ai_operations: 20/minute,      // Expensive API calls
    file_operations: 100/second,    // File system operations
    shell_commands: 10/second,      // Security sensitive
    screenshots: 5/second,          // Resource intensive
    browser_operations: 30/minute   // Browser automation
}
```

### Usage in Commands

All Tauri commands should check rate limits before executing operations:

```rust
#[tauri::command]
pub async fn some_command(state: State<'_, AppState>) -> Result<String, String> {
    // Check rate limit
    if let Err(e) = state.rate_limiters.ai_operations.check("user_id").await {
        return Err(e.to_user_message());
    }
    
    // Execute operation
    perform_operation().await
}
```

### Rate Limiter Initialization

**IMPORTANT**: The rate limiter cleanup task must be initialized after the Tokio runtime is ready:

```rust
// In src-tauri/src/lib.rs setup()
tauri::async_runtime::spawn(async move {
    let app_state = app_handle.state::<AppState>();
    app_state.initialize_rate_limiter_cleanup().await;
});
```

### Configuration (Future Enhancement)

Currently, rate limits are hardcoded. Future versions will support:
- Configuration via settings.json
- Per-user rate limit overrides
- Environment-specific limits (dev vs prod)
- Distributed rate limiting for multi-instance deployments

### Best Practices

1. **Always check rate limits** before expensive operations
2. **Use appropriate limiter** for each operation type
3. **Return user-friendly errors** with retry-after information
4. **Monitor rate limit violations** for security analysis
5. **Consider burst allowances** for legitimate power users

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
8. **Safe Unwrapping**: Use match expressions, `?` operator, or `unwrap_or_else()` instead of `.unwrap()`
9. **Time Operations**: Always use `.unwrap_or_else(|_| Duration::from_secs(0))` for SystemTime
10. **Mutex Locking**: Use `match lock() { Ok(guard) => ..., Err(e) => ... }` pattern
11. **Regex Compilation**: Handle `Regex::new()` errors with proper fallbacks

## Memory Safety Best Practices

### Avoiding Panics in Production

**NEVER use `.unwrap()` in production code.** Instead:

```rust
// ❌ BAD: Can panic
let value = some_option.unwrap();
let result = some_result.unwrap();

// ✅ GOOD: Safe error handling
let value = some_option.ok_or("Error message")?;
let result = some_result.map_err(|e| format!("Failed: {}", e))?;

// ✅ GOOD: With defaults
let value = some_option.unwrap_or_default();
let value = some_option.unwrap_or_else(|| compute_default());
```

### Common Patterns

1. **SystemTime Operations**:
```rust
// Safe timestamp handling
let timestamp = SystemTime::now()
    .duration_since(UNIX_EPOCH)
    .unwrap_or_else(|_| Duration::from_secs(0))
    .as_secs();
```

2. **Mutex Locking**:
```rust
// Safe mutex access
match some_mutex.lock() {
    Ok(guard) => {
        // Use guard
    }
    Err(e) => {
        tracing::error!("Failed to acquire lock: {}", e);
        return Err("Lock poisoned".to_string());
    }
}
```

3. **Regex Compilation**:
```rust
// Safe regex compilation
match Regex::new(pattern_str) {
    Ok(regex) => {
        // Use regex
    }
    Err(e) => {
        tracing::warn!("Invalid regex pattern: {}", e);
        // Use fallback logic
    }
}
```

4. **Option Chaining**:
```rust
// Instead of checking is_some() then unwrap()
if some_option.is_some() {
    let value = some_option.unwrap(); // ❌ BAD
}

// Use if-let or match
if let Some(value) = some_option { // ✅ GOOD
    // Use value
}
```

### Resource Management

1. **RAII Pattern**: Resources are automatically cleaned up when dropped
2. **Arc<TokioMutex<T>>**: For thread-safe shared state
3. **Weak References**: To prevent circular dependencies
4. **Drop Implementations**: For custom cleanup logic

### Tokio Runtime Safety

**CRITICAL**: Never use `tokio::spawn` or async operations outside of Tokio runtime context:

```rust
// ❌ NEVER do this in Drop implementations
impl Drop for MyStruct {
    fn drop(&mut self) {
        tokio::spawn(async { /* cleanup */ }); // WILL PANIC!
    }
}

// ✅ CORRECT: Check for runtime existence
impl Drop for MyStruct {
    fn drop(&mut self) {
        if let Ok(handle) = tokio::runtime::Handle::try_current() {
            handle.spawn(async { /* cleanup */ });
        } else {
            // Perform synchronous cleanup or log warning
            log::warn!("Dropped outside Tokio runtime - async cleanup skipped");
        }
    }
}
```

**Common Pitfalls to Avoid:**
1. **tokio::spawn in constructors** - Defer to async initialization methods
2. **Async operations in Drop** - Use runtime handle check
3. **Static/lazy initialization** - Avoid Tokio operations
4. **Non-async functions** - Make async or check runtime exists

### Race Condition Prevention

1. **Atomic Operations**: Use `AtomicBool`, `AtomicUsize` for simple flags
2. **Semaphores**: For limiting concurrent operations
3. **RwLock**: For multiple readers, single writer patterns
4. **Channels**: For message passing between threads

## Debugging

Enable debug logging: `RUST_LOG=debug bun run tauri dev`

Critical debug points:
- Memory operations (add/retrieve messages)
- API request/response payloads
- Tool execution flow and errors
- State synchronization
- Event emission and handling
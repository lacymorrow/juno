# CLAUDE.md - Backend

This file provides guidance to Claude Code when working with the Rust/Tauri backend in this repository.

## CRITICAL: The Backend Owns ALL Business Logic

The Rust backend is the entire application. The frontend is a display layer. The backend must function independently — headless, CLI, or with any UI.

### What the backend owns
- **Agent execution**: All AI provider calls, tool execution, orchestration
- **Audio**: Microphone recording (voice-transcription plugin), TTS playback (`tts/`, `say` command)
- **Input**: Global keyboard shortcuts, escape key management, hotkey registration
- **I/O**: File system, shell commands, network requests, WebSocket connections
- **State**: All persistent state (Tauri Store), all shared state (`AppState`)
- **Control flow**: Agent loops, cancellation tokens, retries, delegation

### Communication with frontend
- **Backend → Frontend**: Emit Tauri events (`app_handle.emit("event-name", payload)`)
- **Frontend → Backend**: Frontend calls `invoke("command_name", { params })`
- **Never**: Frontend does not initiate I/O, audio, or network independently

### Why this matters
Juno can run as a CLI or headless process. The frontend is an optional UI skin. All logic, I/O, and state management live here so the application works without any frontend at all.

---

## Backend Overview

Rust-based Tauri v2 backend implementing a sophisticated multi-agent AI system with Computer Use capabilities for macOS automation. Features hierarchical agent architecture, comprehensive tool system, and advanced security framework.

## Development Commands

```bash
cargo fmt --manifest-path src-tauri/Cargo.toml --all  # Only cargo command to run locally
gh pr checks --watch                                  # CI runs fmt + clippy + test on the PR
scripts/juno-build.sh                                 # CI builds this branch, installs + opens it
```

Compiling happens in CI, not on this Mac. See "Rust: CI Compiles, Not Your
Mac" in the root `CLAUDE.md`.

**In a fresh clone or a new git worktree, build the frontend before any cargo
command:**

```bash
bun install && npm run build    # creates dist/, which cargo needs
```

`tauri::generate_context!()` reads `frontendDist` (`../dist`) while the macro
expands, so without it the Rust side does not compile. It surfaces as a
proc-macro panic pointing at `lib.rs`, which looks like a code fault and is
not:

```
error: proc macro panicked
  --> src-tauri/src/lib.rs:65:5
   = help: message: The `frontendDist` configuration is set to `"../dist"`
           but this path doesn't exist
```

## Architecture

### Hierarchical Agent System

```
Orchestrator (src/anthropic.rs)
├── Desktop Agent - UI automation, accessibility
├── Browser Agent - Web automation, content extraction  
├── File Agent - Filesystem operations, code editing
└── Tool Providers - Shared resources (browser, AI providers)
```

**Critical Principles:**
- **Orchestrator**: Uses persistent AppState memory, delegation tools only
- **Specialists**: Fresh SimpleMemoryManager, domain-specific tools
- **Memory**: Arc-based cloning for thread safety
- **Tools**: Lazy initialization for expensive resources

### Core Components

```
src/
├── main.rs                    # Application entry point
├── lib.rs                     # Library root
├── anthropic.rs               # Main orchestrator (submit_query)
├── state.rs                   # Application state management
├── commands/                  # Tauri command handlers
├── agent/                     # Multi-agent system
│   ├── implementations/       # Agent implementations
│   ├── providers/            # AI provider integrations
│   ├── tools/                # Tool system
│   └── prompts/              # Prompt management
├── cloud/                     # Cloud connector system
├── tts/                       # Text-to-speech providers
└── utils/                     # Utilities and helpers
```

## State Management

### AppState Pattern

```rust
// All persistent state in AppState
pub struct AppState {
    memory_manager: Arc<TokioMutex<SimpleMemoryManager>>,
    browser_controller: Arc<TokioMutex<Option<BrowserController>>>,
    cancellation_token: Arc<TokioMutex<Option<CancellationToken>>>,
    // ... other shared state
}

// Access via getters
impl AppState {
    pub async fn get_memory_manager(&self) -> Arc<TokioMutex<SimpleMemoryManager>> {
        self.memory_manager.clone()
    }
}
```

### Memory Management

```rust
// Orchestrator uses persistent memory
let memory_manager = app_state.get_memory_manager().await;

// Specialists use fresh memory  
let specialist_memory = Arc::new(TokioMutex::new(SimpleMemoryManager::new()));

// Safe cloning (Arc-based)
let memory_clone = memory_manager.clone();
```

## Agent Development

### Agent Implementation Pattern

```rust
// Agent brain creation
let brain = BrainFactory::create_brain(
    provider_type,
    model_name,
    personality_prompt,
    memory_manager.clone(),
).await?;

// Tool provider setup
let mut tool_provider = LocalToolProvider::new();
register_tools(&mut tool_provider, &app_state).await;

// Agent execution with iteration limit
let agent_runner = AgentRunner::new(brain, tool_provider);
let result = agent_runner.run_with_limit(query, 15).await?;
```

### Tool Registration

```rust
// Tool definition with AI-readable schema
let tool_def = ToolDefinition {
    name: "tool_name".to_string(),
    description: "Clear description for AI understanding".to_string(),
    input_schema: serde_json::json!({
        "type": "object",
        "properties": {
            "param": {"type": "string", "description": "Parameter description"}
        },
        "required": ["param"]
    })
};

// Async executor implementation
let executor = move |input: Value| {
    let app_state = app_state.clone();
    async move {
        // Parse parameters
        let param = input["param"].as_str()
            .ok_or("Missing required parameter")?;
            
        // Execute tool logic with security validation
        validate_security(&param)?;
        let result = perform_operation(param).await?;
        
        // Return structured response
        Ok(serde_json::json!({
            "success": true,
            "result": result
        }))
    }
};

// Register with provider
tool_provider.register_async_tool(tool_def, executor).await;
```

## Error Handling

### AgentError Enum

Defined in `src/agent/core.rs`:
```rust
pub enum AgentError {
    LlmError(String),             // LLM communication failure
    ToolError(String),            // Tool execution failure
    MemoryError(String),          // Memory management failure
    ConfigurationError(String),   // Configuration/setup failure (also for HTTP client init)
    StateError(String),           // Invalid state transition
    MaxStepsReached,              // Iteration limit reached
    LoopError(String),            // Agent loop failure
    InputError(String),           // Input validation failure
    OutputError(String),          // Output processing failure
    InvalidOutput(String),        // Invalid output format
}
```

### Tauri Command Pattern

```rust
#[tauri::command]
pub async fn command_name(
    param: String,
    app_state: State<'_, AppState>
) -> Result<String, String> {
    // Input validation
    if param.is_empty() {
        return Err("Parameter cannot be empty".to_string());
    }
    
    // Operation with proper error handling
    match perform_operation(&param, &app_state).await {
        Ok(result) => Ok(result),
        Err(e) => {
            tracing::error!("Command failed: {}", e);
            Err(e.to_string())
        }
    }
}
```

## Security Framework

### Security Configuration

```rust
// Security modes based on build type
let security_config = if cfg!(debug_assertions) {
    SecurityConfig::development_mode()
} else {
    SecurityConfig::default() // Production mode
};

// Path validation
fn validate_file_path(path: &str, config: &SecurityConfig) -> Result<PathBuf, String> {
    let path = Path::new(path);
    
    // Prevent path traversal
    if path.to_string_lossy().contains("../") {
        return Err("Path traversal not allowed".to_string());
    }
    
    // Workspace boundary enforcement
    let canonical = path.canonicalize()
        .map_err(|_| "Invalid path".to_string())?;
        
    // Additional security checks...
    Ok(canonical)
}
```

### Command Security

This section used to show a `const ALLOWED_COMMANDS` whitelist that has never
existed anywhere in Juno, and called the shell gate "command whitelist
validation". It is a **blocklist**. Two sections of invented code describing a
control that works the opposite way is how Juno came to look like it had two
competing shell allowlists. What it actually has:

**One parser.** `src/shell_command.rs` is the only place that understands a
shell command string: normalisation, tokenisation, the inert-character
whitelist, the command word, and the recursive-forced-`rm` tokeniser. Every
gate reads it and none of them parses for itself. Two parsers of the same
syntax drift, silently, and they did.

**Two gates, asking different questions.** They are not duplicates, and
merging them into one list would weaken both.

| Question | Where | Shape |
|---|---|---|
| Does Juno stop and ask the person? | `agent/tools/risk_classifier.rs` → `agent/tools/permission_policy.rs::requires_approval` | Risk level plus the person's mode. A closed set of 20 inert commands behind a character whitelist never asks |
| Does Juno run it at all? | `commands/shell.rs::refuse_forbidden_command` | Blocklist. Privilege escalation, catastrophic literals, recursive forced `rm` of `/`, redirection into system directories, path traversal in a redirection |
| Does Juno ask before it sends? (Claude CLI path) | `agent/providers/cli_approval.rs` | Consequence at the tool boundary: read-only connector calls and drafts run, sends and deletes ask |

**One construction, so a question can go away.** `src/trash.rs` writes the
`rm` the shell session runs: deletes go to the macOS Trash, so they are
reversible and need no dialog. It is a shim on the session's `PATH`, not a
rewrite of the command text, because bash has already expanded globs, quoting
and variables by the time the shim sees a path. When the Trash cannot take
something, the shim refuses and does not delete.

**The direction of travel.** Gate by consequence, not by command: inferring
intent from shell syntax is undecidable, so substring classification shrinks
rather than grows, and new gates go where intent is structurally known. Read
`docs/plans/permissions-by-consequence.md` before changing any of the three.

## Platform Integration

### macOS APIs

```rust
// Accessibility API usage
use computer_use_ai_sdk::prelude::*;

// Screen capture
pub async fn capture_screenshot() -> Result<String, String> {
    let desktop = Desktop::new()?;
    let screenshot = desktop.capture_screenshot().await?;
    
    // Convert to base64
    let base64_data = base64::engine::general_purpose::STANDARD
        .encode(&screenshot);
        
    Ok(format!("data:image/png;base64,{}", base64_data))
}

// Mouse automation
pub async fn click_at_position(x: i32, y: i32) -> Result<(), String> {
    let desktop = Desktop::new()?;
    desktop.click(x, y).await
        .map_err(|e| e.to_string())
}
```

### Permission Handling

```rust
// Never terminate on permission failures
pub async fn check_permissions() -> PermissionStatus {
    match check_accessibility_permission().await {
        Ok(has_permission) => {
            if has_permission {
                PermissionStatus::Granted
            } else {
                PermissionStatus::Denied
            }
        }
        Err(e) => {
            tracing::warn!("Permission check failed: {}", e);
            PermissionStatus::Unknown
        }
    }
}

// Graceful degradation
pub async fn operation_with_permissions() -> Result<String, String> {
    match check_permissions().await {
        PermissionStatus::Granted => {
            // Full functionality
            perform_full_operation().await
        }
        _ => {
            // Limited functionality with user guidance
            Ok("Limited functionality - please grant permissions".to_string())
        }
    }
}
```

## Testing Patterns

### Async Testing

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use tokio_test;
    
    #[tokio::test]
    async fn test_async_operation() {
        let result = async_operation().await;
        assert!(result.is_ok());
    }
    
    #[test]
    fn test_serialization() {
        let data = TestStruct { field: "value".to_string() };
        let json = serde_json::to_string(&data).unwrap();
        let deserialized: TestStruct = serde_json::from_str(&json).unwrap();
        assert_eq!(data, deserialized);
    }
}
```

### Mock Patterns

```rust
// Mock tool for testing
struct MockTool;

impl Tool for MockTool {
    async fn execute(&self, input: Value) -> Result<Value, String> {
        Ok(serde_json::json!({"mocked": true}))
    }
}

// Test with mock
#[tokio::test]
async fn test_with_mock() {
    let mut provider = LocalToolProvider::new();
    provider.register_tool("mock_tool", Box::new(MockTool));
    
    let result = provider.execute_tool("mock_tool", json!({})).await;
    assert!(result.is_ok());
}
```

## Critical Development Rules

### Compilation Check
**MANDATORY**: every Rust change must pass CI (`fmt`, `clippy -D warnings`, `test`) before merge. Push a draft PR and let CI compile it; don't run cargo builds locally.

### macOS: main-thread-only APIs

Some macOS APIs kill the process when called off the main thread. Not an
error, not a panic — `dispatch_assert_queue(main)` inside the framework, then
`__builtin_trap()`. SIGTRAP. No unwind, so `catch_unwind` never sees it, and
the log just stops mid-run with no message.

This is a **third threading axis**, separate from the two rules above it.
`tokio::task::spawn_blocking` is the correct tool for blocking work and stays
correct; it simply does not run on the main thread. Nothing in the type
system, in clippy, or in a function signature will warn you. Juno shipped
exactly this bug for weeks: dictation's paste path read the keyboard layout
through Text Input Services from a blocking worker and took the whole app
down on the first paste of every run (#603).

Treat these as main-thread-only:

| API family | Examples |
|---|---|
| Text Input Services | `TISCopyCurrentKeyboardLayoutInputSource`, `TISGetInputSourceProperty` |
| HIToolbox generally | `TSMGetInputSourceProperty`, input-source and TSM calls |
| AppKit UI | window, view, menu, `NSApp` anything |
| `NSWorkspace` mutations | launching, activating, opening |

The pattern, when a worker needs a value only the main thread can read:

```rust
static CACHED: AtomicU16 = AtomicU16::new(UNRESOLVED);

/// Main thread only. Call once during setup.
pub fn prime_cached() {
    if !is_main_thread() { warn!("prime called off-main; ignoring"); return; }
    CACHED.store(resolve_via_framework(), Ordering::Relaxed);
}

pub fn cached() -> u16 {
    if is_main_thread() {
        let v = resolve_via_framework();     // safe here, keeps it fresh
        CACHED.store(v, Ordering::Relaxed);
        return v;
    }
    match CACHED.load(Ordering::Relaxed) {
        UNRESOLVED => SENSIBLE_DEFAULT,      // degrade, never trap
        v => v,
    }
}
```

Resolve on main, cache atomically, degrade off-main. Prime from
`apply_macos_setup`, which already runs on the main thread.

**Do not** `dispatch_sync` to the main queue to "fix" this. It deadlocks
whenever the main thread is waiting on the task you are calling from, which
is exactly the shape of a Tauri command awaiting `spawn_blocking`.

Check the thread with `NSThread.isMainThread` via `objc`.
`dispatch::Queue::main().is_current()` does not exist in our bindings.

Regression tests come free: Rust's test harness runs every test on a spawned
thread, so calling the function from a test is already off-main. Before the
fix, the test binary dies and reports nothing.

### Memory Management
- Clone memory managers safely using Arc-based patterns
- Use `SimpleMemoryManager::new()` for specialists
- Access AppState memory via getters, never direct field access

### Error Handling
- Use `AgentError` enum for agent-related errors
- Use `Result<T, String>` for Tauri commands
- Never use `std::process::exit()` — use `app_handle.exit(0)` for Tauri-managed shutdown
- Never use `std::env::set_var()` — unsafe in multithreaded Rust; use Tauri Store instead
- Log errors at appropriate levels (error, warn, info, debug)
- **NEVER use `.unwrap()` or `.expect()` in production code** — always handle errors properly
- Never byte-slice strings (`&s[..n]`) — panics on multi-byte UTF-8; use `s.chars().take(n).collect::<String>()`

### Async Runtime
- **Always** use `tauri::async_runtime::spawn()` instead of `tokio::spawn()` / `tokio::task::spawn()`
- Use `tauri::async_runtime::JoinHandle` (not `tokio::task::JoinHandle`) for spawn return types
- Use `tokio::task::spawn_blocking()` for blocking operations (shell commands, sync file I/O)

### Escape Key Management
- Register escape key ONLY during agent execution
- Register at start of `submit_query`/`submit_orchestrated_query`
- Always unregister on **every** exit path — completion, error, cancellation, AND early returns

### Deadlock Prevention
- Never hold an async mutex while calling a function that acquires another mutex
- Use check-init-recheck pattern for lazy initialization (see `state.rs:get_or_init_browser_controller`)
- Never acquire the same `std::sync::Mutex` twice in one call chain

### HTTP Client Initialization
AI providers must set timeouts — default `Client::new()` has no timeout:
```rust
let client = Client::builder()
    .timeout(std::time::Duration::from_secs(120))
    .build()
    .map_err(|e| AgentError::ConfigurationError(format!("Failed to create HTTP client: {}", e)))?;
```

### Security Requirements
- All file operations must use security validation
- Shell commands pass `commands::shell::refuse_forbidden_command`, which is a
  blocklist, not a whitelist. Parse through `shell_command`, never by hand
- Implement different security levels for development vs production
- Add comprehensive audit logging for security events
- See `docs/audits/security-audit-2026-02-08.md` for 32 tracked vulnerabilities (2026-02-08, with 2026-09-11 status annotations)

### Shared Settings: Read-Modify-Write
`ToolSettings` in `settings/mod.rs` is written by multiple subsystems (ToolConfigManager, mouse settings UI, etc.). When saving, each subsystem MUST:
1. Read the existing `ToolSettings` from the store first
2. Update only the fields it owns
3. Write back the merged result

Never construct a fresh `ToolSettings` and save it — this clobbers fields owned by other subsystems. See `ToolConfigManager::save_to_centralized_settings()` for the correct pattern.

### Code Deduplication
- `parse_shortcut_string` lives in `src/shortcuts.rs` — do not duplicate in `lib.rs`
- `format_error` helper is `pub` in `lib.rs` — use `crate::format_error` instead of local copies
- Event listeners: one handler per event — check `setup_agent_control_listeners` before adding

## Key Files Reference

- `src/main.rs` - Application entry and setup
- `src/anthropic.rs` - Main orchestrator (`submit_query` entry point)
- `src/state.rs` - Application state management
- `src/agent/implementations/` - Agent brain and runner implementations
- `src/agent/tools/` - Complete tool system
- `src/commands/` - All Tauri command handlers
- `src/cloud/connector.rs` - Cloud system with hardware monitoring
- `mcp-server-os-level/` - macOS platform integration
- `Cargo.toml` - Dependencies and build configuration

## Common Patterns

### Async Command Handler
```rust
#[tauri::command]
pub async fn async_command(
    param: String,
    app_state: State<'_, AppState>
) -> Result<ResponseType, String> {
    // Validation, execution, error handling
}
```

### Tool Provider Access
```rust
let tool_provider = app_state.get_tool_provider().await;
let result = tool_provider.execute_tool("tool_name", params).await?;
```

### Event Emission
```rust
let app_handle = app_state.get_app_handle();
app_handle.emit("event_name", payload)?;
```

## Memory Safety Guidelines

### Safe Unwrapping Patterns

**Production Code Rule: ZERO `.unwrap()` or `.expect()` calls allowed**

```rust
// ❌ NEVER DO THIS IN PRODUCTION
let value = some_option.unwrap();
let value = some_option.expect("msg");
let timestamp = SystemTime::now().duration_since(UNIX_EPOCH).unwrap();
let guard = mutex.lock().unwrap();

// ✅ ALWAYS USE SAFE PATTERNS
// Option handling
let value = some_option.ok_or("Error: value not found")?;
let value = some_option.unwrap_or_default();
let value = some_option.unwrap_or_else(|| compute_default());

// Result handling
let result = operation().map_err(|e| format!("Operation failed: {}", e))?;
let result = operation().unwrap_or_else(|e| {
    tracing::error!("Operation failed: {}", e);
    default_value
});

// SystemTime (ALWAYS use this pattern)
let timestamp = SystemTime::now()
    .duration_since(UNIX_EPOCH)
    .unwrap_or_else(|_| Duration::from_secs(0))
    .as_secs();

// Mutex locking
match mutex.lock() {
    Ok(guard) => {
        // Use guard
    }
    Err(e) => {
        tracing::error!("Mutex poisoned: {}", e);
        return Err("Failed to acquire lock".to_string());
    }
}

// Regex compilation
match Regex::new(pattern) {
    Ok(regex) => {
        // Use regex
    }
    Err(e) => {
        tracing::warn!("Invalid regex pattern '{}': {}", pattern, e);
        // Fallback logic
    }
}
```

### Common Anti-Patterns to Avoid

```rust
// ❌ Checking then unwrapping
if option.is_some() {
    let value = option.unwrap(); // Still dangerous!
}

// ✅ Use if-let instead
if let Some(value) = option {
    // Use value safely
}

// ❌ Multiple unwraps in chain
let result = some_map.get("key").unwrap().field.unwrap();

// ✅ Use ? operator or and_then
let result = some_map.get("key")
    .and_then(|v| v.field.as_ref())
    .ok_or("Value not found")?;
```

### Race Condition Prevention

1. **Use Atomic Types**: 
   - `AtomicBool` for flags
   - `AtomicUsize` for counters
   - `Arc<TokioMutex<T>>` for complex shared state

2. **Semaphore Pattern**:
```rust
let semaphore = Arc::new(Semaphore::new(1));
let permit = semaphore.clone().try_acquire_owned()
    .map_err(|_| "Resource busy")?;
```

3. **RAII Resource Management**:
```rust
pub struct ManagedResource<T> {
    resource: Option<T>,
    cleanup: Option<Box<dyn FnOnce(T) + Send + 'static>>,
}

impl<T> Drop for ManagedResource<T> {
    fn drop(&mut self) {
        if let (Some(resource), Some(cleanup)) = 
            (self.resource.take(), self.cleanup.take()) {
            cleanup(resource);
        }
    }
}
```

### Testing Best Practices

- `.unwrap()` is acceptable in test code with clear context
- Use `expect()` with descriptive messages in tests
- Always test error paths, not just happy paths
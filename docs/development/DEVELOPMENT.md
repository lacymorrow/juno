# Development Guide (Canonical)

This is the canonical location for the development guide. For a minimal entry point, see `../SIMPLE_DOCS.md`.

---

# Development Guide

## Essential Requirements

### Mandatory Compilation Check ⚠️

```bash
cargo check --manifest-path src-tauri/Cargo.toml
```

**CRITICAL**: Run after EVERY Rust file modification. Project MUST compile with exit code 0.

### File Organization

- Keep files under 700 lines when possible
- Check for existing files before creating new ones
- Follow modular structure in `src-tauri/src/`
- Use conditional compilation `#[cfg(target_os = "macos")]` for platform-specific code

## Hierarchical Agent Architecture ✅

### Architecture Rules

- **Orchestrator Only**: Gets `delegate_to_*_agent` tools, uses persistent AppState memory
- **Specialists Only**: Get domain-specific tool suites, use fresh `SimpleMemoryManager::new()`
- **Memory Separation**: Never mix orchestrator and specialist memory managers
- **Structured Responses**: All specialists return JSON with success/error status

### Core Components

- **Orchestrator**: `src-tauri/src/anthropic.rs` - `submit_query` function
- **Agent Implementations**: `src-tauri/src/agent/implementations/`
- **Agent Providers**: `src-tauri/src/agent/providers/`
- **Agent Tools**: `src-tauri/src/agent/tools/`
- **Platform Integration**: `src-tauri/mcp-server-os-level/src/platforms/macos/`

### Memory Management Guidelines

- **Orchestrator**: Uses persistent memory manager from AppState
- **Specialists**: Use fresh `SimpleMemoryManager::new()` for task isolation
- Clone memory manager safely before passing to agent runners (uses Arc internally)
- Use debug logging to track memory operations
- Never create duplicate instances of singleton components

## State Management

### AppState Patterns

- All persistent state in `AppState` (`src-tauri/src/state.rs`)
- Use `Arc<TokioMutex<T>>` for shared mutable state
- Access state via provided getter methods
- Never create duplicate instances of singleton components

### Resource Management

- Lazy initialization for expensive resources (browser controller, AI providers)
- Clean up resources properly in useEffect returns
- Use proper async/await patterns throughout

## Error Handling Patterns

### Standard Error Handling

- Use `AgentError` enum for all agent-related errors
- Use `Result<T, String>` consistently for Tauri commands
- Log errors at appropriate levels: error, warn, info, debug
- Handle async errors with proper Result types
- **NEVER** use `std::process::exit()` on permission failures

### Permission Management (macOS)

- Implement graceful degradation for missing permissions
- Provide helpful error messages to guide users
- Never terminate the app on permission failures
- Check permissions before attempting operations

## Tool Development

### Tool Creation Pattern

```rust
// 1. Define tool structure
ToolDefinition {
    name: "tool_name".to_string(),
    description: "Clear description for AI understanding".to_string(),
    input_schema: serde_json::json!({
        "type": "object",
        "properties": {
            "param": {"type": "string", "description": "Parameter description"}
        },
        "required": ["param"]
    })
}

// 2. Implement async executor
let executor = move |input: Value| async move {
    // Parse input parameters
    let param = input["param"].as_str()
        .ok_or("Missing required parameter")?;
    
    // Execute tool logic
    let result = perform_action(param).await?;
    
    // Return structured response
    Ok(serde_json::json!({
        "success": true,
        "result": result
    }))
};

// 3. Register with provider
tool_provider.register_async_tool(definition, executor).await;
```

### Tool Best Practices

- All tools should be async for non-blocking operation
- Validate and sanitize all input parameters
- Convert errors to appropriate formats for agent consumption
- Provide clear descriptions for AI understanding
- Handle resource cleanup after tool execution

## UI Development

### Event-Driven Architecture

- Use Tauri events for backend→frontend communication
- Debounce rapid event updates (100ms recommended)
- Handle event cleanup in useEffect returns
- Maintain separate UI state from backend state

### Component Patterns

- Main app in `src/App.tsx`
- Use shadcn/ui components for consistency
- Implement proper loading and error states
- Handle audio playbook cleanup properly

### Voice Integration

- **Agent Mode**: Alt+D toggles voice input for AI agent conversations
- **Dictation Mode**: Configurable key (default spacebar) for immediate voice-to-text
- Event rebroadcasting for plugin compatibility
- Global shortcut management with dynamic registration

## API Integration

### Anthropic API Handling

- All API communication in `src-tauri/src/agent/providers/anthropic.rs`
- Tool results must be properly formatted for Anthropic API spec
- Handle tool_use and end_turn stop reasons correctly
- Log request payloads at debug level only

### Provider Management

- Multi-provider support (Anthropic, OpenAI, Gemini)
- Factory pattern for provider creation
- Runtime provider switching capability
- Fallback mechanisms for provider failures

## Version Management

### Automated Version Bumping

Use the automated script to bump versions across both Rust and Node.js components:

```bash
./scripts/bump-version.sh <new-version>
```

**Examples:**

```bash
./scripts/bump-version.sh 0.2.4        # Patch release
./scripts/bump-version.sh 1.0.0        # Major release  
./scripts/bump-version.sh 2.1.0-beta.1 # Pre-release
```

**What the script does:**

1. **Rust workspace**: Updates all `Cargo.toml` files via `cargo set-version --workspace`
2. **Node.js packages**: Updates `package.json` files via Changesets CLI
3. **Verification**: Provides commands to verify all versions updated correctly

**Single source of truth**: `src-tauri/Cargo.toml` version field drives all other versions. Tauri automatically inherits from `CARGO_PKG_VERSION`.

**Documentation**: See `.cursor/rules/version-bumping-workflow.mdc` for comprehensive workflow details.

## Testing Strategy

### Test Commands

```bash
./run-all-tests.sh           # Comprehensive test suite
./test-rust-units.sh         # Rust unit tests
npm test                     # Frontend tests (TypeScript/React)
npm run test:watch           # Watch mode for development
cargo check                  # REQUIRED compilation check
cargo test --manifest-path src-tauri/Cargo.toml  # Rust tests directly
```

### Frontend Testing (TypeScript/React)

... (content unchanged below) ...



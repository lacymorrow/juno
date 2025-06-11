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

- All API communication in `src-tauri/src/agent/implementations/agent_brain.rs`
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

**Technology Stack:**

- **Vitest**: Test runner and framework with TypeScript support
- **Testing Library**: React component testing with user-centric queries
- **jsdom**: Browser environment simulation for DOM testing
- **vi (Vitest)**: Mocking and assertion utilities

**Configuration:**

- `vitest.config.ts`: Path alias resolution (`@` → `./src`) matching main vite config
- `src/test/setup.ts`: Test environment setup with mocks for browser APIs

**Test Structure:**

```
src/
├── components/__tests__/
│   ├── VoiceStatusIndicator.test.tsx
│   └── DevToolsPanel.test.tsx
└── lib/__tests__/
    └── utils.test.ts
```

**Component Testing Patterns:**

- **Tauri API Mocking**: Complete `@tauri-apps/api/core` and `@tauri-apps/api/event` mocks
- **Plugin Mocking**: Voice transcription and system-level plugins
- **Icon Mocking**: Lucide React icon components for UI testing
- **Async Testing**: Proper handling of async operations with waitFor
- **User Interactions**: FireEvent for button clicks and user input simulation

**Coverage Areas:**

- Component rendering and state management
- Event handling and user interactions
- Tauri API integration and error handling
- Utility function validation and edge cases
- Async operations and promise handling

### Backend Testing (Rust)

**Technology Stack:**

- **Cargo Test**: Native Rust test framework
- **tokio-test**: Async runtime testing utilities
- **serde_json**: JSON serialization/deserialization validation
- **mockall**: Mock object framework for complex dependencies

**Test Categories:**

#### 1. Core Data Structures (`src-tauri/src/agent/structs.rs`)

- **Agent Errors**: Complete `AgentError` enum testing with error message validation
- **Message Types**: User/Assistant message serialization and deserialization
- **Tool Results**: Tool execution result handling and formatting
- **Error Handling**: Edge cases and error propagation patterns

#### 2. State Management (`src-tauri/src/state.rs`)

- **AppState**: Initialization, configuration updates, and getter methods
- **KeyboardShortcuts**: Platform-specific defaults and customization
- **Permissions State**: Permission status tracking and updates
- **Memory Management**: Arc-based state sharing and thread safety
- **Async Operations**: Tokio mutex handling and deadlock prevention

#### 3. Configuration System (`src-tauri/src/constants.rs`)

- **Event Constants**: String value validation for all event types
- **Default Values**: Platform-specific configuration defaults
- **Invariants**: Ensuring critical constants remain unchanged

**Testing Patterns:**

- **Async Testing**: `#[tokio::test]` for async function testing
- **Serialization Testing**: JSON roundtrip validation with serde
- **Error Propagation**: Comprehensive error handling validation
- **Thread Safety**: Multi-threaded access patterns with Arc/Mutex
- **Timeout Handling**: Async operations with proper timeout controls

### Development Testing

**Pre-commit Checklist:**

- [ ] `cargo check --manifest-path src-tauri/Cargo.toml` passes
- [ ] `npm test` passes with no failing tests
- [ ] No compilation warnings or errors
- [ ] New code includes appropriate test coverage

**Testing During Development:**

- Monitor debug logs during development
- Test multi-turn conversations for memory persistence
- Verify tool execution with real desktop interactions
- Test voice integration end-to-end
- Validate event flow with browser dev tools
- Test permission scenarios across different states

**Mock Strategy:**

- **Frontend**: Mock all Tauri APIs and external dependencies
- **Backend**: Mock external services and system APIs only when necessary
- **Integration Points**: Focus on API boundaries and data flow
- **Async Operations**: Use proper async testing patterns with tokio-test

### Test Organization

**File Naming Conventions:**

- Frontend: `*.test.tsx` for React components, `*.test.ts` for utilities
- Backend: `#[cfg(test)]` modules at the end of source files
- Integration: `tests/` directory for cross-component testing

**Coverage Goals:**

- **Frontend**: >90% line coverage for utilities, >80% for components
- **Backend**: >95% for core logic, >80% for integration points
- **Critical Paths**: 100% coverage for error handling and state management

**Continuous Integration:**

- All tests must pass on the target platform (macOS)
- Cross-platform compatibility testing where applicable
- Performance regression testing for critical paths
- Memory leak detection for long-running operations

### Platform Considerations

**macOS Specific Testing:**

- Permission state changes require actual macOS testing
- Accessibility API integration needs system-level validation
- Voice integration requires microphone access for full testing
- Screen recording features need proper system permissions

**Development vs Production:**

- Built app testing for permission validation
- Bundle identifier differences between dev and production
- Entitlements and Info.plist inclusion verification
- Code signing impact on accessibility features

## Performance Considerations

### Memory Optimization

- Clone memory managers efficiently (Arc-based)
- Limit conversation history if needed
- Clean up audio resources properly
- Use lazy initialization for expensive components

### Tool Execution

- Handle long-running tools with timer system
- Emit progress events for user feedback
- Implement proper cancellation handling via escape key
- Use background execution for appropriate tools

## Platform-Specific Implementation

### macOS Integration

- **Application Detection**: Use NSWorkspace APIs via objc messaging
- **Screen Resolution**: Use existing screenshot capture and image parsing
- **System Context**: Gather comprehensive context (window, app, screen info)
- **Accessibility**: Full accessibility tree navigation support

### Dependencies Management

- Avoid adding new dependencies when existing ones suffice
- Leverage existing crates: `objc`, `base64::Engine`, `image`
- Always check `mcp-server-os-level/` for existing functionality
- Use established patterns from `platforms/macos/`

## Escape Key Management

### Dynamic Registration Pattern

- **ONLY** register escape key during agent execution
- Register at start of `submit_query`/`submit_orchestrated_query`
- Unregister on completion/error/cancellation
- Provides graceful cancellation with proper cleanup
- Never leave escape key handlers registered permanently

## Debugging Best Practices

### Logging Strategy

- Use appropriate log levels consistently
- Include context in log messages (message counts, IDs, etc.)
- Log both success and failure paths
- Enable debug logging for memory and API operations

### Common Debug Points

1. Memory operations (add/retrieve messages)
2. API request/response payloads (debug level only)
3. Event emission and handling
4. Tool execution flow
5. State synchronization issues
6. Escape key registration/unregistration

### Sound System Integration

- Multiple provider support (ElevenLabs, system, Replicate)
- Proper audio resource cleanup
- Volume control and playback management
- Event coordination for voice feedback

## Voice Modes Implementation

### Agent Mode Details

- Alt+D global shortcut activation
- Voice → Transcription → AI Agent Processing → Agent Response/Actions
- Full hierarchical agent system with orchestrator and specialists
- No direct text pasting - AI responses through chat interface
- Persistent conversation memory

### Dictation Mode Details

- Configurable key activation (default spacebar)
- Voice → Transcription → Text insertion at cursor location
- Pure transcription service with no AI processing
- Immediate transcription start (0ms delay)
- 500ms threshold for commitment vs cancellation
- Smart cancellation with key passthrough

This guide consolidates all development patterns and ensures consistent implementation across the Juno codebase.

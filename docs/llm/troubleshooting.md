# 🔍 Troubleshooting Guide

## Common Issues and Solutions

This guide helps diagnose and fix common problems in Juno development.

## 🚨 Compilation Errors

### Issue: "no reactor running" panic
**Symptom**: Runtime panic with message about no Tokio reactor
```
thread 'main' panicked at 'there is no reactor running'
```

**Solution**: Use `tauri::async_runtime::spawn()` instead of `tokio::spawn()`
```rust
// ❌ WRONG
tokio::spawn(async move { /* ... */ });

// ✅ CORRECT
tauri::async_runtime::spawn(async move { /* ... */ });
```

### Issue: Type mismatch in commands
**Symptom**: Compilation error about type mismatch in Tauri commands

**Solution**: Ensure command returns `Result<T, String>`
```rust
// ❌ WRONG
#[tauri::command]
pub async fn my_command() -> Result<String, MyError> {
    // ...
}

// ✅ CORRECT
#[tauri::command]
pub async fn my_command() -> Result<String, String> {
    // Convert errors to String
    do_something().map_err(|e| e.to_string())
}
```

### Issue: Lifetime errors with async
**Symptom**: Complex lifetime errors in async functions

**Solution**: Use `Arc` and clone before moving into async blocks
```rust
// ❌ WRONG
let data = &self.data;
tokio::spawn(async move {
    use_data(data);  // Lifetime error
});

// ✅ CORRECT
let data = Arc::clone(&self.data);
tokio::spawn(async move {
    use_data(&data);  // Works
});
```

## 🎙️ Voice System Issues

### Issue: Microphone not working
**Symptom**: Voice commands not responding, no transcription

**Diagnostic Steps**:
1. Check permissions: `System Preferences → Security & Privacy → Microphone`
2. Verify Juno is listed and checked
3. Test with debug logging:
```bash
RUST_LOG=tauri_plugin_voice_transcription=debug bun run tauri dev
```

**Solution**: 
- Grant microphone permission
- Restart the app after permission change
- Check audio input device in system settings

### Issue: Wake words not detected
**Symptom**: Always listening mode active but not responding to wake words

**Solution**:
1. Check wake word configuration:
```typescript
await invoke('get_always_listening_wake_words');
```

2. Adjust sensitivity:
```typescript
await invoke('set_always_listening_sensitivity', { 
    sensitivity: 0.3  // Lower = more sensitive
});
```

3. Verify wake words are lowercase and simple

## 🖱️ Permission Issues

### Issue: "Accessibility permission required"
**Symptom**: Click/type commands fail with permission error

**Solution**:
1. Open System Preferences → Security & Privacy → Privacy → Accessibility
2. Add Juno to the list (may need to unlock with admin password)
3. Ensure checkbox is checked
4. **Important**: Test with BUILT app, not just dev mode

### Issue: Screenshot permission denied
**Symptom**: Screenshot commands fail

**Solution**:
1. System Preferences → Security & Privacy → Privacy → Screen Recording
2. Add and enable Juno
3. Restart the application

## 🔌 Frontend Issues

### Issue: Commands not found
**Symptom**: `invoke` returns "command not found" error

**Diagnostic**:
```typescript
console.error('Command failed:', error);
// Check exact command name
```

**Solution**:
1. Verify command is registered in `registry.rs`
2. Check command name matches exactly (case-sensitive)
3. Ensure command module is included in `mod.rs`

### Issue: State not updating
**Symptom**: UI doesn't reflect backend changes

**Solution**: Use events for state synchronization
```typescript
// Listen for backend updates
useEffect(() => {
    const unlisten = listen('state-updated', (event) => {
        setState(event.payload);
    });
    return () => { unlisten.then(fn => fn()); };
}, []);
```

### Issue: Modal not closing
**Symptom**: Modal stays open after action

**Solution**: Ensure proper state management
```typescript
// Reset modal state after action
const handleAction = async () => {
    try {
        await performAction();
        setModalOpen(false);  // Don't forget this
    } catch (error) {
        // Handle error
    }
};
```

## 🤖 Agent Issues

### Issue: Agent not responding
**Symptom**: Commands sent but no response

**Diagnostic**:
1. Check API keys are set correctly
2. Enable debug logging:
```bash
RUST_LOG=juno::agent=debug bun run tauri dev
```

**Solution**:
- Verify API keys in settings
- Check network connectivity
- Review agent logs for errors

### Issue: Tools not available
**Symptom**: "Tool not found" errors

**Solution**:
1. Check tool is registered in agent
2. Verify tool name matches exactly
3. Ensure tool category is enabled:
```typescript
await invoke('get_tool_config');
```

### Issue: Memory overflow
**Symptom**: Agent becomes slow or unresponsive

**Solution**: Clear conversation memory
```typescript
await invoke('clear_conversation_memory', { 
    agentId: 'orchestrator' 
});
```

## 🔒 Security Issues

### Issue: File access denied
**Symptom**: File operations fail with security error

**Solution**:
1. Check if in production mode (strict security)
2. Verify file is in allowed directory
3. Check file extension is permitted
4. For development, use debug mode:
```bash
RUST_LOG=debug bun run tauri dev
```

### Issue: Command blocked
**Symptom**: Shell commands fail with "not allowed"

**Solution**:
- Check command whitelist in security config
- Use only allowed commands in production
- For development, ensure debug mode is active

## 🏗️ Build Issues

### Issue: Build fails on macOS
**Symptom**: Build errors related to permissions or signing

**Solution**:
1. Ensure Xcode command line tools installed:
```bash
xcode-select --install
```

2. Check entitlements file exists:
```bash
ls src-tauri/juno.entitlements
```

3. Verify Info.plist is included in build

### Issue: Missing dependencies
**Symptom**: Build fails with missing crate errors

**Solution**:
```bash
cd src-tauri
cargo clean
cargo update
cargo build
```

## 🧪 Testing Issues

### Issue: Tests timeout
**Symptom**: Async tests hang indefinitely

**Solution**: Add timeout to async tests
```rust
#[tokio::test]
#[timeout(Duration::from_secs(5))]
async fn test_with_timeout() {
    // Test code
}
```

### Issue: Mock state not working
**Symptom**: Tests fail with state-related errors

**Solution**: Create proper test state
```rust
fn create_test_state() -> AppState {
    AppState {
        config: Arc::new(TokioMutex::new(Config::default())),
        // Initialize all required fields
    }
}
```

## 📊 Performance Issues

### Issue: UI freezing
**Symptom**: Interface becomes unresponsive

**Diagnostic**:
1. Check browser console for errors
2. Look for synchronous operations
3. Monitor event frequency

**Solution**:
- Debounce rapid events
- Use async operations
- Implement virtual scrolling for long lists

### Issue: High memory usage
**Symptom**: App consumes excessive memory

**Solution**:
1. Clear unused agent memory
2. Limit conversation history
3. Check for memory leaks in event listeners

## 🔧 Debug Techniques

### Enable Comprehensive Logging
```bash
# Maximum verbosity
RUST_LOG=trace bun run tauri dev

# Specific module
RUST_LOG=juno::agent::tools=debug bun run tauri dev

# Multiple modules
RUST_LOG=juno::agent=debug,tauri_plugin_voice_transcription=debug bun run tauri dev
```

### Check Debug Files
```bash
# Agent requests saved automatically in debug mode
ls -la ./debug/agent_request_*.json

# View latest request
cat ./debug/agent_request_* | jq . | less
```

### Frontend Debugging
```javascript
// Add to any component
console.log('Component state:', { 
    props, 
    state, 
    timestamp: new Date().toISOString() 
});

// Trace command execution
const result = await invoke('command_name', args);
console.log('Command result:', { command: 'command_name', args, result });
```

### Rust Debugging
```rust
// Add debug derives
#[derive(Debug)]
pub struct YourStruct { /* ... */ }

// Use debug printing
dbg!(&variable);
println!("Debug: {:?}", variable);
tracing::debug!(?variable, "Debug info");
```

## 🆘 Getting Help

1. **Check logs first** - Most issues are visible in debug logs
2. **Search existing issues** - Someone may have encountered it
3. **Isolate the problem** - Create minimal reproduction
4. **Provide context** - Include OS, Rust version, error messages

### Information to Include
- Operating system and version
- Rust version: `rustc --version`
- Node version: `node --version`
- Full error message
- Steps to reproduce
- Relevant code snippets

---

*For additional help, check the project's issue tracker or documentation.*
# Event Listener & Resource Accumulation Fixes

## 🔥 Critical Issues Identified

### 1. MCP Server Listener Accumulation

**Problem**: MaxListenersExceededWarning with 11 drain listeners on MCP servers
**Root Cause**: Multiple MCP server initializations without cleanup

### 2. Frontend Event Listener Duplication  

**Problem**: Voice events have listeners in both VoiceContext AND App.tsx
**Root Cause**: Incomplete migration to centralized VoiceContext

### 3. Hot Reload Resource Leaks

**Problem**: Vite reloads don't clean up backend resources
**Root Cause**: Missing cleanup handlers for development mode

## ✅ Fix #1: MCP Server Singleton Pattern

**File**: `src-tauri/src/state.rs`

```rust
// Add cleanup method to AppState
impl AppState {
    /// Cleanup all MCP servers and resources
    pub async fn cleanup_mcp_resources(&self) -> Result<(), String> {
        info!("🧹 Cleaning up MCP resources...");
        
        let mcp_manager = self.get_mcp_manager().await;
        let manager_guard = mcp_manager.lock().await;
        
        // Stop all servers
        let configs = manager_guard.get_server_configs().await;
        for config in configs {
            if let Err(e) = manager_guard.stop_server(&config.id).await {
                warn!("Failed to stop MCP server '{}': {}", config.name, e);
            }
        }
        
        drop(manager_guard);
        info!("✅ MCP resources cleaned up");
        Ok(())
    }
    
    /// Initialize MCP servers with deduplication
    pub async fn initialize_mcp_servers_once(&self) -> Result<(), String> {
        static INITIALIZED: std::sync::Once = std::sync::Once::new();
        static mut INITIALIZATION_RESULT: Option<Result<(), String>> = None;
        
        unsafe {
            INITIALIZED.call_once(|| {
                // Run async initialization in blocking context
                let rt = tokio::runtime::Handle::current();
                INITIALIZATION_RESULT = Some(rt.block_on(async {
                    self.initialize_mcp_servers().await
                }));
            });
            
            INITIALIZATION_RESULT.as_ref().unwrap().clone()
        }
    }
}
```

## ✅ Fix #2: Remove Duplicate Voice Listeners

**File**: `src/App.tsx`

```typescript
// REMOVE these duplicate listeners from App.tsx:

// ❌ DELETE - Already in VoiceContext
useEffect(() => {
  const unlisten = listen<{ query?: string | null; error?: string | null }>(
    "app-dictation-finished",
    (event) => {
      // This is duplicate - VoiceContext handles this
    }
  );
  return () => unlisten.then((unlistenFn) => unlistenFn());
}, [submitQuery, voiceSounds, sound]);

// ❌ DELETE - Already in VoiceContext  
useEffect(() => {
  const unlisten = listen("agent-active", async (event) => {
    // This is duplicate - VoiceContext handles this
  });
  return () => unlisten.then((unlistenFn) => unlistenFn());
}, []);

// ✅ KEEP - Use VoiceContext instead:
const { voiceState, agentState } = useVoice();

// React to voice state changes instead of listening to events
useEffect(() => {
  if (voiceState.transcriptionText) {
    submitQuery(voiceState.transcriptionText, true);
  }
}, [voiceState.transcriptionText, submitQuery]);
```

## ✅ Fix #3: Hot Reload Cleanup Handler

**File**: `src-tauri/src/lib.rs`

```rust
// Add development cleanup handler
pub fn setup_development_cleanup(app: &tauri::App) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    #[cfg(debug_assertions)]
    {
        // Listen for frontend reloads and cleanup resources
        let app_handle = app.handle().clone();
        app.listen("frontend-reload", move |_event| {
            info!("🔄 Frontend reload detected - cleaning up resources...");
            
            let app_handle_clone = app_handle.clone();
            tauri::async_runtime::spawn(async move {
                // Cleanup MCP servers
                if let Ok(state) = app_handle_clone.try_state::<AppState>() {
                    if let Err(e) = state.cleanup_mcp_resources().await {
                        error!("Failed to cleanup MCP resources: {}", e);
                    }
                }
                
                info!("✅ Development cleanup completed");
            });
        });
        
        info!("🛠️ Development mode cleanup handlers installed");
    }
    
    Ok(())
}
```

## ✅ Fix #4: Prevent Multiple Tool Provider Registration

**File**: `src-tauri/src/agent/providers/factory.rs`

```rust
pub async fn register_computer_use_tools(
    provider: &mut LocalToolProvider,
    app_handle: tauri::AppHandle,
) -> Result<(), String> {
    info!("🔧 Registering Computer Use tools (race-condition safe)...");

    // Use a global mutex to ensure that only one thread can register tools at a time
    // This prevents race conditions where multiple threads try to register the same tools simultaneously
    use std::sync::Arc;
    use tokio::sync::Mutex;

    lazy_static::lazy_static! {
        static ref TOOL_REGISTRATION_MUTEX: Arc<Mutex<()>> = Arc::new(Mutex::new(()));
    }

    // Acquire the lock to ensure exclusive access to tool registration
    let _lock = TOOL_REGISTRATION_MUTEX.lock().await;

    info!("🔧 Acquired tool registration lock, proceeding with registration...");

    // Only register tools once per provider instance
    if provider.get_tool_count() > 0 {
        info!("🔧 Tools already registered for this provider instance, skipping duplicate registration");
        return Ok(());
    }
    
    // Get the app state for MCP manager integration
    let state_manager = app_handle.state::<AppState>();

    // Set up MCP manager in the tool provider (per-provider instance)
    let mcp_manager = state_manager.get_mcp_manager().await;
    provider.set_mcp_manager(mcp_manager);

    // Register the official Anthropic Computer Use tools (per-provider instance)
    register_anthropic_computer_use_tools(provider, app_handle.clone()).await?;

    // Register additional desktop automation tools (per-provider instance)
    crate::agent::tools::desktop_tools::register_desktop_tools(provider, state_manager.clone(), app_handle.clone()).await;

    // Register timer tools for agent task scheduling and resumption (per-provider instance)
    crate::agent::tools::timer_tools::register_timer_tools(provider, app_handle.clone()).await;

    // Register self-awareness and introspection tools (per-provider instance, development mode only)
    crate::agent::tools::register_self_awareness_tools(provider).await;

    // Simply refresh MCP tools from cache (fast operation if already loaded)
    if let Err(e) = provider.refresh_mcp_tools().await {
        warn!("Failed to refresh MCP tools from cache: {}", e);
    } else {
        info!("MCP tools refreshed from cache (no network calls)");
    }

    info!("✅ Computer Use tools registered successfully for provider instance");

    // Lock is automatically released when _lock goes out of scope
    Ok(())
}
```

## ✅ Fix #5: MCP Server Process Monitoring

**File**: `src-tauri/src/agent/tools/mcp_integration.rs`

```rust
impl MCPServerConnection {
    /// Enhanced cleanup with process termination
    pub async fn disconnect(&mut self) {
        info!("🔌 Disconnecting MCP server: {}", self.config.name);
        
        // Close communication channels first
        self.stdin_writer = None;
        self.stdout_reader = None;
        self.stderr_reader = None;
        
        // Terminate process
        if let Some(mut process) = self.process.take() {
            // Try graceful shutdown first
            if let Err(e) = process.terminate().await {
                warn!("Failed to terminate MCP server process gracefully: {}", e);
                
                // Force kill if necessary
                if let Err(e) = process.kill().await {
                    error!("Failed to force kill MCP server process: {}", e);
                }
            }
            
            // Wait for process to exit
            match tokio::time::timeout(Duration::from_secs(5), process.wait()).await {
                Ok(Ok(exit_status)) => {
                    info!("MCP server process exited: {}", exit_status);
                }
                Ok(Err(e)) => {
                    error!("Error waiting for MCP server process: {}", e);
                }
                Err(_) => {
                    error!("Timeout waiting for MCP server process to exit");
                }
            }
        }
        
        self.status = MCPServerStatus::Disconnected;
        info!("✅ MCP server disconnected: {}", self.config.name);
    }
}
```

## ✅ Fix #6: Development Mode Detection Script

**File**: `scripts/check-listener-accumulation.sh`

```bash
#!/bin/bash

# Check for signs of listener accumulation
echo "🔍 Checking for listener accumulation issues..."

# Check for MaxListenersExceededWarning in logs
if grep -r "MaxListenersExceeded" . --include="*.log" 2>/dev/null; then
    echo "❌ MaxListenersExceededWarning found in logs"
else
    echo "✅ No MaxListenersExceeded warnings found"
fi

# Check for duplicate voice event listeners
DUPLICATE_LISTENERS=$(grep -r "listen.*dictation-active\|listen.*agent-active" src/ --include="*.ts" --include="*.tsx" | wc -l)

if [ $DUPLICATE_LISTENERS -gt 2 ]; then
    echo "❌ Potential duplicate voice listeners found ($DUPLICATE_LISTENERS instances)"
    echo "Expected: 1 in VoiceContext + 1 check = 2 total"
else
    echo "✅ Voice listeners look good ($DUPLICATE_LISTENERS instances)"
fi

# Check for multiple MCP initialization points
MCP_INITS=$(grep -r "initialize_mcp_servers\|init.*mcp.*server" src-tauri/ --include="*.rs" | wc -l)
echo "📊 MCP initialization points found: $MCP_INITS"

if [ $MCP_INITS -gt 3 ]; then
    echo "⚠️  Consider consolidating MCP initialization"
fi

echo "✅ Listener accumulation check complete"
```

## 🎯 Implementation Order

1. **Immediate**: Remove duplicate voice listeners from App.tsx
2. **High Priority**: Add MCP server cleanup and singleton initialization  
3. **Medium Priority**: Add hot reload cleanup handlers
4. **Low Priority**: Add monitoring scripts

## 🧪 Testing

After implementing fixes:

```bash
# Run the detection script
./scripts/check-listener-accumulation.sh

# Monitor logs for warnings
tail -f your-app.log | grep -i "MaxListeners\|duplicate\|warning"

# Test hot reload behavior
# 1. Start app in dev mode
# 2. Make frontend changes to trigger reload
# 3. Check logs for proper cleanup messages
```

## 📊 Expected Results

- **No more MaxListenersExceededWarning**
- **Single initialization sequences in logs**
- **Proper cleanup messages on reload**  
- **Reduced memory usage over time**
- **No duplicate permission checks**

---

**Priority**: 🔥 **CRITICAL** - These fixes prevent memory leaks and performance degradation.

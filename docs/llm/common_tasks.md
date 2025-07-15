# 🎯 Common Tasks for LLMs

## Quick Task Reference

This guide provides step-by-step instructions for common development tasks in the Juno project.

## 📋 Task Categories

### 1. Adding a New Command

**Task**: Add a new Tauri command that can be called from the frontend

**Steps**:
```rust
// 1. Create command in src-tauri/src/commands/your_module.rs
use tauri::State;
use crate::state::AppState;

#[tauri::command]
pub async fn your_new_command(
    state: State<'_, AppState>,
    param1: String,
    param2: Option<i32>
) -> Result<String, String> {
    // Access app state if needed
    let some_state = state.some_field.lock().await;
    
    // Your logic here
    Ok(format!("Processed: {} with {:?}", param1, param2))
}

// 2. Add to src-tauri/src/commands/mod.rs
pub mod your_module;

// 3. Register in src-tauri/src/commands/registry.rs
pub fn register_commands() -> impl Fn(tauri::Builder<tauri::Wry>) -> tauri::Builder<tauri::Wry> {
    |builder| {
        builder.invoke_handler(tauri::generate_handler![
            // ... existing commands ...
            your_module::your_new_command, // Add here
        ])
    }
}

// 4. Use from frontend in TypeScript
import { invoke } from "@tauri-apps/api/core";

const result = await invoke('your_new_command', {
    param1: "hello",
    param2: 42
});
```

### 2. Adding a New AI Tool

**Task**: Add a new tool that the AI agent can use

**Steps**:
```rust
// 1. Create tool in src-tauri/src/agent/tools/your_tool.rs
use rig::tool::{Tool, ToolDefinition};
use serde_json::{json, Value};

pub struct YourTool;

impl YourTool {
    pub fn definition() -> ToolDefinition {
        ToolDefinition {
            name: "your_tool_name".to_string(),
            description: "Clear description of what this tool does".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "param1": {
                        "type": "string",
                        "description": "Description of param1"
                    }
                },
                "required": ["param1"]
            }),
        }
    }
    
    pub async fn execute(params: Value) -> Result<Value, Box<dyn std::error::Error>> {
        let param1 = params["param1"].as_str().unwrap_or_default();
        
        // Tool implementation
        
        Ok(json!({
            "success": true,
            "result": "Your result here"
        }))
    }
}

// 2. Register in the appropriate agent file
// For orchestrator: src-tauri/src/anthropic.rs
// For desktop agent: src-tauri/src/agents/desktop_agent.rs
// In the tool registration section:
tool_provider.register_async_tool(
    YourTool::definition(),
    move |params| {
        let params = params.clone();
        async move { YourTool::execute(params).await }
    }
);
```

### 3. Adding a Settings Option

**Task**: Add a new configurable setting

**Steps**:
```typescript
// 1. Add to settings type in src/types/settings.ts
export interface Settings {
    // ... existing fields ...
    yourNewSetting: boolean;
    yourNewValue: string;
}

// 2. Add UI in src/components/settings/sections/GeneralSettings.tsx
<div className="flex items-center justify-between">
    <div className="space-y-0.5">
        <Label>Your New Setting</Label>
        <div className="text-sm text-muted-foreground">
            Description of what this does
        </div>
    </div>
    <Switch
        checked={settings.yourNewSetting}
        onCheckedChange={(checked) => 
            updateSetting('yourNewSetting', checked)
        }
    />
</div>

// 3. Handle in backend src-tauri/src/commands/settings.rs
#[derive(Serialize, Deserialize)]
pub struct Settings {
    // ... existing fields ...
    pub your_new_setting: bool,
    pub your_new_value: String,
}

// 4. Add default in Default implementation
impl Default for Settings {
    fn default() -> Self {
        Self {
            // ... existing defaults ...
            your_new_setting: false,
            your_new_value: "default".to_string(),
        }
    }
}
```

### 4. Adding a Frontend Component

**Task**: Create a new React component

**Steps**:
```typescript
// 1. Create component in src/components/YourComponent.tsx
import React, { useState, useEffect } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { Button } from '@/components/ui/button';

interface YourComponentProps {
    title: string;
    onAction?: () => void;
}

export const YourComponent: React.FC<YourComponentProps> = ({ 
    title, 
    onAction 
}) => {
    const [data, setData] = useState<string>('');
    const [loading, setLoading] = useState(false);
    
    const handleClick = async () => {
        setLoading(true);
        try {
            const result = await invoke('your_command', { 
                param: title 
            });
            setData(result as string);
            onAction?.();
        } catch (error) {
            console.error('Error:', error);
        } finally {
            setLoading(false);
        }
    };
    
    return (
        <div className="p-4 border rounded">
            <h3 className="text-lg font-semibold">{title}</h3>
            <p className="text-sm text-gray-600">{data}</p>
            <Button 
                onClick={handleClick}
                disabled={loading}
                className="mt-2"
            >
                {loading ? 'Loading...' : 'Click Me'}
            </Button>
        </div>
    );
};

// 2. Use in parent component
import { YourComponent } from '@/components/YourComponent';

function ParentComponent() {
    return (
        <YourComponent 
            title="My Component"
            onAction={() => console.log('Action performed')}
        />
    );
}
```

### 5. Handling Errors Properly

**Task**: Implement proper error handling

**Steps**:
```rust
// 1. Define custom error in src-tauri/src/agent/core.rs
#[derive(Debug, thiserror::Error)]
pub enum YourError {
    #[error("Network error: {0}")]
    Network(String),
    
    #[error("Invalid input: {0}")]
    InvalidInput(String),
    
    #[error("Tool execution failed: {0}")]
    ToolError(String),
}

// 2. Use in your code
pub async fn your_function() -> Result<String, YourError> {
    // Validate input
    if input.is_empty() {
        return Err(YourError::InvalidInput("Input cannot be empty".into()));
    }
    
    // Handle potential failures
    let result = risky_operation()
        .await
        .map_err(|e| YourError::Network(e.to_string()))?;
    
    Ok(result)
}

// 3. Convert for Tauri commands
impl From<YourError> for String {
    fn from(err: YourError) -> Self {
        err.to_string()
    }
}

// 4. Handle in frontend
try {
    const result = await invoke('your_command');
} catch (error) {
    // Error will be a string with the error message
    console.error('Command failed:', error);
    // Show user-friendly error
    showNotification({
        title: 'Error',
        description: error as string,
        type: 'error'
    });
}
```

### 6. Working with Voice System

**Task**: Add voice-related functionality

**Steps**:
```rust
// 1. For dictation mode
use tauri_plugin_voice_transcription::TranscriptionExt;

#[tauri::command]
pub async fn start_voice_input(app: AppHandle) -> Result<(), String> {
    app.transcription()
        .start_dictation()
        .await
        .map_err(|e| e.to_string())?;
    Ok(())
}

// 2. For always listening
#[tauri::command]
pub async fn configure_wake_words(
    app: AppHandle,
    wake_words: Vec<String>
) -> Result<(), String> {
    app.transcription()
        .set_always_listening_wake_words(wake_words)
        .await
        .map_err(|e| e.to_string())?;
    Ok(())
}

// 3. Listen for transcription events in frontend
import { listen } from '@tauri-apps/api/event';

useEffect(() => {
    const unlisten = listen('transcription', (event) => {
        const { text, is_final } = event.payload as any;
        if (is_final) {
            console.log('Final transcription:', text);
            // Process the transcription
        }
    });
    
    return () => {
        unlisten.then(fn => fn());
    };
}, []);
```

### 7. Testing Your Changes

**Task**: Run tests and validate changes

**Steps**:
```bash
# 1. Check Rust compilation (MANDATORY after any Rust change)
cargo check --manifest-path src-tauri/Cargo.toml

# 2. Run all tests
./run-all-tests.sh

# 3. Run specific Rust tests
cargo test your_test_name --manifest-path src-tauri/Cargo.toml

# 4. Run frontend tests
npm test

# 5. Test in development mode
bun run tauri dev

# 6. Test with debug logging
RUST_LOG=debug bun run tauri dev

# 7. Build and test production version
bun tauri build
# Test the built app in target/release/bundle/
```

### 8. Debugging Issues

**Task**: Debug problems in the application

**Steps**:
```rust
// 1. Add debug logging
use tracing::{debug, info, warn, error};

debug!("Variable value: {:?}", some_variable);
info!("Operation completed successfully");
warn!("Potential issue: {}", warning_message);
error!("Error occurred: {}", error);

// 2. Enable debug mode
RUST_LOG=debug bun run tauri dev

// 3. Check specific module logs
RUST_LOG=juno::agent=debug bun run tauri dev

// 4. Inspect frontend console
// Open browser dev tools with Cmd+Option+I

// 5. Check Tauri dev tools
// Right-click app window → Inspect Element

// 6. Save debug requests (automatic in debug mode)
// Check ./debug/agent_request_*.json files
```

## 🚀 Quick Patterns

### State Access Pattern
```rust
// In Tauri commands
pub async fn command(state: State<'_, AppState>) -> Result<T, String> {
    let data = state.some_field.lock().await;
    // Use data
}
```

### Event Emission Pattern
```rust
// Backend
app_handle.emit("event-name", payload)?;

// Frontend
const unlisten = await listen('event-name', (event) => {
    console.log(event.payload);
});
```

### Async Command Pattern
```rust
#[tauri::command]
pub async fn async_command() -> Result<String, String> {
    // Async operations
    tokio::time::sleep(Duration::from_secs(1)).await;
    Ok("Done".to_string())
}
```

### Tool Registration Pattern
```rust
tool_provider.register_async_tool(
    definition,
    move |params| {
        let params = params.clone();
        async move { 
            // Tool implementation
            Ok(json!({"result": "success"}))
        }
    }
);
```

## ⚠️ Common Pitfalls

1. **Forgetting `cargo check`** - Always run after Rust changes
2. **Using wrong spawn** - Use `tauri::async_runtime::spawn()` not `tokio::spawn()`
3. **Direct file I/O** - Use Tauri store instead
4. **Missing error handling** - Always handle Result types
5. **Blocking async code** - Don't use blocking operations in async functions

---

*For more detailed information, refer to [LLM_GUIDE.md](../../LLM_GUIDE.md) and [LLMs.txt](../../LLMs.txt)*
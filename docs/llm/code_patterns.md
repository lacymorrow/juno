# 🔧 Code Patterns Reference

## Essential Code Patterns for Juno Development

This reference provides copy-paste ready code patterns for common scenarios.

## 🎯 Tauri Command Patterns

### Basic Command
```rust
#[tauri::command]
pub fn simple_command(name: String) -> String {
    format!("Hello, {}!", name)
}
```

### Async Command with State
```rust
#[tauri::command]
pub async fn stateful_command(
    state: State<'_, AppState>,
    input: String,
) -> Result<String, String> {
    let mut data = state.some_data.lock().await;
    data.push(input.clone());
    Ok(format!("Stored: {}", input))
}
```

### Command with Error Handling
```rust
#[tauri::command]
pub async fn fallible_command(
    path: String,
) -> Result<Vec<String>, String> {
    std::fs::read_dir(&path)
        .map_err(|e| format!("Failed to read directory: {}", e))?
        .filter_map(|entry| {
            entry.ok().and_then(|e| {
                e.file_name().to_str().map(String::from)
            })
        })
        .collect::<Vec<_>>()
        .into()
}
```

## 🤖 Agent Tool Patterns

### Basic Tool Definition
```rust
use rig::tool::ToolDefinition;
use serde_json::{json, Value};

pub fn create_tool() -> (ToolDefinition, impl Fn(Value) -> BoxFuture<'static, Result<Value, Box<dyn Error + Send + Sync>>>) {
    let definition = ToolDefinition {
        name: "my_tool".to_string(),
        description: "Does something useful".to_string(),
        input_schema: json!({
            "type": "object",
            "properties": {
                "input": {
                    "type": "string",
                    "description": "The input to process"
                }
            },
            "required": ["input"]
        }),
    };
    
    let executor = move |params: Value| {
        Box::pin(async move {
            let input = params["input"].as_str().unwrap_or_default();
            
            // Tool logic here
            
            Ok(json!({
                "result": format!("Processed: {}", input)
            }))
        }) as BoxFuture<'static, Result<Value, Box<dyn Error + Send + Sync>>>
    };
    
    (definition, executor)
}
```

### Tool with External Dependencies
```rust
pub fn create_browser_tool(
    browser: Arc<Mutex<Option<BrowserController>>>,
) -> (ToolDefinition, impl Fn(Value) -> BoxFuture<'static, Result<Value, Box<dyn Error + Send + Sync>>>) {
    let definition = ToolDefinition {
        name: "browser_navigate".to_string(),
        description: "Navigate browser to URL".to_string(),
        input_schema: json!({
            "type": "object",
            "properties": {
                "url": {
                    "type": "string",
                    "description": "URL to navigate to"
                }
            },
            "required": ["url"]
        }),
    };
    
    let executor = move |params: Value| {
        let browser = browser.clone();
        Box::pin(async move {
            let url = params["url"].as_str()
                .ok_or("URL is required")?;
            
            let mut browser_guard = browser.lock().await;
            if let Some(controller) = browser_guard.as_mut() {
                controller.navigate(url).await?;
                Ok(json!({ "success": true }))
            } else {
                Err("Browser not initialized".into())
            }
        }) as BoxFuture<'static, Result<Value, Box<dyn Error + Send + Sync>>>
    };
    
    (definition, executor)
}
```

## 🔄 State Management Patterns

### App State Definition
```rust
use std::sync::Arc;
use tokio::sync::Mutex as TokioMutex;

#[derive(Clone)]
pub struct AppState {
    pub config: Arc<TokioMutex<Config>>,
    pub memory_manager: Arc<TokioMutex<MemoryManager>>,
    pub active_agents: Arc<TokioMutex<Vec<String>>>,
}

impl AppState {
    pub fn new() -> Self {
        Self {
            config: Arc::new(TokioMutex::new(Config::default())),
            memory_manager: Arc::new(TokioMutex::new(MemoryManager::new())),
            active_agents: Arc::new(TokioMutex::new(Vec::new())),
        }
    }
}
```

### Safe State Access
```rust
// Avoid deadlocks with proper lock ordering
pub async fn safe_state_operation(state: &AppState) -> Result<(), String> {
    // Always acquire locks in the same order
    let config = state.config.lock().await;
    let memory = state.memory_manager.lock().await;
    
    // Perform operations
    
    // Locks are automatically released when dropped
    Ok(())
}
```

## 📡 Event Patterns

### Backend Event Emission
```rust
use tauri::Manager;

// Emit to all windows
app_handle.emit("event-name", json!({
    "message": "Hello from backend",
    "timestamp": SystemTime::now()
}))?;

// Emit to specific window
if let Some(window) = app_handle.get_webview_window("main") {
    window.emit("window-event", payload)?;
}
```

### Frontend Event Listening
```typescript
import { listen } from '@tauri-apps/api/event';
import { useEffect } from 'react';

// In a React component
useEffect(() => {
    const setupListeners = async () => {
        const unlisten = await listen<PayloadType>('event-name', (event) => {
            console.log('Received:', event.payload);
            // Handle event
        });
        
        return unlisten;
    };
    
    const unlistenPromise = setupListeners();
    
    return () => {
        unlistenPromise.then(unlisten => unlisten());
    };
}, []);
```

### Event with Acknowledgment
```rust
// Backend - Listen for frontend response
let (tx, rx) = tokio::sync::oneshot::channel();
let event_id = uuid::Uuid::new_v4().to_string();

app_handle.emit("request-event", json!({
    "id": event_id,
    "data": "some data"
}))?;

// Store tx with event_id for later response handling
```

## 🛡️ Error Handling Patterns

### Custom Error Types
```rust
use thiserror::Error;

#[derive(Error, Debug)]
pub enum AppError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    
    #[error("Network error: {0}")]
    Network(String),
    
    #[error("Invalid configuration: {0}")]
    Config(String),
    
    #[error("Agent error: {0}")]
    Agent(#[from] AgentError),
}

// Convert to String for Tauri
impl From<AppError> for String {
    fn from(err: AppError) -> Self {
        err.to_string()
    }
}
```

### Result Chain Pattern
```rust
pub async fn complex_operation() -> Result<String, AppError> {
    let config = load_config()
        .map_err(|e| AppError::Config(e.to_string()))?;
    
    let data = fetch_data(&config.url).await?;  // Auto-converts network errors
    
    let processed = process_data(data)
        .ok_or_else(|| AppError::Config("Invalid data format".into()))?;
    
    Ok(processed)
}
```

## 🔐 Security Patterns

### Path Validation
```rust
use std::path::{Path, PathBuf};

pub fn validate_file_path(path: &str) -> Result<PathBuf, String> {
    let path = Path::new(path);
    
    // Get canonical path to resolve .. and symlinks
    let canonical = path.canonicalize()
        .map_err(|e| format!("Invalid path: {}", e))?;
    
    // Ensure it's within allowed directory
    let workspace = std::env::current_dir()
        .map_err(|e| format!("Failed to get workspace: {}", e))?;
    
    if !canonical.starts_with(&workspace) {
        return Err("Path is outside workspace".into());
    }
    
    Ok(canonical)
}
```

### Command Validation
```rust
const ALLOWED_COMMANDS: &[&str] = &["ls", "cat", "grep", "find"];

pub fn validate_command(cmd: &str) -> Result<(), String> {
    let parts: Vec<&str> = cmd.split_whitespace().collect();
    let command = parts.first().ok_or("Empty command")?;
    
    if !ALLOWED_COMMANDS.contains(command) {
        return Err(format!("Command '{}' not allowed", command));
    }
    
    // Check for dangerous patterns
    if cmd.contains("rm -rf") || cmd.contains("sudo") {
        return Err("Dangerous command pattern detected".into());
    }
    
    Ok(())
}
```

## ⚡ Performance Patterns

### Lazy Initialization
```rust
use once_cell::sync::Lazy;

static EXPENSIVE_RESOURCE: Lazy<ExpensiveResource> = Lazy::new(|| {
    // This only runs once, on first access
    ExpensiveResource::initialize()
});

pub fn use_resource() {
    EXPENSIVE_RESOURCE.do_something();
}
```

### Concurrent Operations
```rust
use futures::future::join_all;

pub async fn parallel_operations(urls: Vec<String>) -> Vec<Result<String, Error>> {
    let futures = urls.into_iter().map(|url| {
        async move {
            fetch_url(&url).await
        }
    });
    
    join_all(futures).await
}
```

## 🧪 Testing Patterns

### Unit Test with Mock
```rust
#[cfg(test)]
mod tests {
    use super::*;
    
    #[tokio::test]
    async fn test_async_function() {
        let mock_state = AppState::new();
        
        let result = your_function(&mock_state).await;
        
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "expected value");
    }
    
    #[test]
    fn test_sync_function() {
        let input = "test input";
        let output = process_input(input);
        
        assert_eq!(output, "expected output");
    }
}
```

### Integration Test
```rust
// In tests/integration_test.rs
use tauri::test::{mock_builder, MockRuntime};

#[test]
fn test_command() {
    let app = mock_builder()
        .invoke_handler(tauri::generate_handler![commands::my_command])
        .build(tauri::test::mock_context())
        .expect("Failed to build app");
    
    // Test command invocation
}
```

## 📝 Frontend Patterns

### Custom Hook Pattern
```typescript
import { useState, useEffect } from 'react';
import { invoke } from '@tauri-apps/api/core';

export function useBackendData<T>(command: string, args?: any) {
    const [data, setData] = useState<T | null>(null);
    const [loading, setLoading] = useState(true);
    const [error, setError] = useState<string | null>(null);
    
    useEffect(() => {
        const fetchData = async () => {
            try {
                setLoading(true);
                const result = await invoke<T>(command, args);
                setData(result);
            } catch (err) {
                setError(err as string);
            } finally {
                setLoading(false);
            }
        };
        
        fetchData();
    }, [command, JSON.stringify(args)]);
    
    return { data, loading, error };
}
```

### Component with Tauri Integration
```typescript
export const TauriComponent: React.FC = () => {
    const { data, loading, error } = useBackendData<string[]>('get_items');
    
    if (loading) return <div>Loading...</div>;
    if (error) return <div>Error: {error}</div>;
    
    return (
        <ul>
            {data?.map((item, index) => (
                <li key={index}>{item}</li>
            ))}
        </ul>
    );
};
```

## 🔄 Store Pattern

### Tauri Store Usage
```rust
use tauri_plugin_store::StoreExt;

pub async fn save_to_store(
    app: AppHandle,
    key: &str,
    value: serde_json::Value,
) -> Result<(), String> {
    let store = app.store("settings.json")
        .map_err(|e| e.to_string())?;
    
    store.set(key.to_string(), value);
    store.save().map_err(|e| e.to_string())?;
    
    Ok(())
}

pub async fn load_from_store(
    app: AppHandle,
    key: &str,
) -> Result<serde_json::Value, String> {
    let store = app.store("settings.json")
        .map_err(|e| e.to_string())?;
    
    store.get(key)
        .cloned()
        .ok_or_else(|| format!("Key '{}' not found", key))
}
```

---

*These patterns are used throughout the Juno codebase. For complete examples, see the actual implementation files.*
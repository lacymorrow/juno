# API Reference

## Agent Commands

### submit_query
**Purpose**: Execute AI agent with single-agent mode  
**Signature**: `submit_query(query: String) -> Result<(), String>`  
**Location**: `src-tauri/src/anthropic.rs`  
**Flow**: Registers escape key → Runs agent → Unregisters escape key  

### submit_orchestrated_query
**Purpose**: Execute AI agent with multi-agent orchestration  
**Signature**: `submit_orchestrated_query(query: String, use_orchestrator: bool) -> Result<String, String>`  
**Location**: `src-tauri/src/commands/orchestrator.rs`  
**Fallback**: Uses `submit_query` when `use_orchestrator = false`

### cleanup_browser
**Purpose**: Clean up browser resources  
**Signature**: `cleanup_browser() -> Result<(), String>`  
**Location**: `src-tauri/src/anthropic.rs`

## Desktop Automation Commands

### Screenshot Commands
```rust
capture_screenshot_command() -> Result<String, String>  // Returns base64 image
capture_element_screenshot_command(selector: String) -> Result<String, String>
```

### Mouse Commands
```rust
dev_left_click(x: i32, y: i32) -> Result<(), String>
dev_right_click(x: i32, y: i32) -> Result<(), String>
dev_double_click(x: i32, y: i32) -> Result<(), String>
dev_middle_click(x: i32, y: i32) -> Result<(), String>
dev_mouse_move(x: i32, y: i32) -> Result<(), String>
dev_left_mouse_down(x: i32, y: i32) -> Result<(), String>
dev_left_mouse_up(x: i32, y: i32) -> Result<(), String>
dev_left_click_drag(start_x: i32, start_y: i32, end_x: i32, end_y: i32) -> Result<(), String>
dev_get_cursor_position() -> Result<(i32, i32), String>
dev_triple_click(x: i32, y: i32) -> Result<(), String>
```

### Keyboard Commands
```rust
dev_type_text(text: String) -> Result<(), String>
dev_press_key(key: String) -> Result<(), String>
dev_hold_key(key: String) -> Result<(), String>
dev_release_key(key: String) -> Result<(), String>
dev_global_type_text(text: String) -> Result<(), String>
```

### Element Commands
```rust
dev_get_focused_element_info() -> Result<String, String>  // Returns JSON
dev_click_focused_element() -> Result<(), String>
dev_find_element_by_selector(selector: String) -> Result<String, String>
dev_click_element_by_selector(selector: String) -> Result<(), String>
dev_get_selected_text() -> Result<String, String>
```

### Window Commands
```rust
dev_get_window_list() -> Result<String, String>  // Returns JSON array
dev_get_window_info() -> Result<String, String>  // Returns JSON
dev_focus_window(window_id: u32) -> Result<(), String>
dev_scroll_window(direction: String, amount: i32) -> Result<(), String>
```

### Application Commands
```rust
dev_open_application(app_name: String) -> Result<(), String>
dev_open_url(url: String) -> Result<(), String>
list_apps() -> Result<Vec<String>, String>
```

### Clipboard Commands
```rust
dev_get_clipboard() -> Result<String, String>
dev_set_clipboard(content: String) -> Result<(), String>
```

### Wait Command
```rust
dev_wait(milliseconds: u64) -> Result<(), String>
```

## File System Commands

```rust
dev_list_files(path: String) -> Result<String, String>  // Returns JSON
dev_get_file_content(path: String) -> Result<String, String>
dev_set_file_content(path: String, content: String) -> Result<(), String>
```

## Shell Commands

```rust
dev_bash_command(command: String) -> Result<String, String>  // Returns output
```

## Text Editor Commands

```rust
dev_text_editor_view(file_path: String) -> Result<String, String>
dev_text_editor_create(file_path: String, content: String) -> Result<(), String>
dev_text_editor_str_replace(file_path: String, old: String, new: String) -> Result<(), String>
dev_text_editor_insert(file_path: String, line: usize, content: String) -> Result<(), String>
dev_text_editor_undo_edit(file_path: String) -> Result<(), String>
```

## Provider Management Commands

```rust
get_providers() -> Result<Vec<String>, String>
get_active_provider() -> Result<String, String>
set_active_provider(provider: String) -> Result<(), String>
get_provider_settings(provider: String) -> Result<String, String>  // Returns JSON
update_provider_api_key(provider: String, api_key: String) -> Result<(), String>
update_provider_model(provider: String, model: String) -> Result<(), String>
update_provider_max_tokens(provider: String, max_tokens: u32) -> Result<(), String>
update_provider_temperature(provider: String, temperature: f32) -> Result<(), String>
update_provider_system_prompt(provider: String, prompt: String) -> Result<(), String>
```

## TTS Commands

```rust
invoke_tts(text: String, voice_id: Option<String>) -> Result<String, String>  // Returns base64 audio
set_tts_provider_command(provider: String) -> Result<(), String>
get_tts_provider_command() -> Result<String, String>
```

## Orchestrator Commands

```rust
get_orchestrator_status() -> Result<OrchestratorStatusReport, String>
configure_orchestrator(config: OrchestratorConfigDTO) -> Result<(), String>
get_task_history() -> Result<Vec<TaskResult>, String>
get_active_tasks() -> Result<Vec<Task>, String>
get_agent_capabilities() -> Result<HashMap<String, Vec<AgentCapability>>, String>
```

## QA Test Commands

```rust
qa_test_click(x: i32, y: i32) -> Result<String, String>
qa_test_click_series(clicks: Vec<(i32, i32)>) -> Result<String, String>
qa_test_coordinate_transformation(x: i32, y: i32) -> Result<String, String>
qa_test_click_visualization(x: i32, y: i32) -> Result<(), String>
qa_test_select_text(start_x: i32, start_y: i32, end_x: i32, end_y: i32) -> Result<String, String>
qa_test_scroll(direction: String, amount: i32) -> Result<String, String>
```

## Status Commands

```rust
check_server_status() -> Result<String, String>
dev_test_click_visualization(x: i32, y: i32) -> Result<(), String>
```

## Data Types

### Common Structures
```rust
// Error types
Result<T, String>  // Standard return type for all commands

// Coordinate types
(i32, i32)  // Screen coordinates

// Window info JSON structure
{
  "id": number,
  "title": string,
  "app_name": string,
  "bounds": { "x": number, "y": number, "width": number, "height": number }
}

// Element info JSON structure
{
  "role": string,
  "title": string,
  "value": string,
  "position": { "x": number, "y": number },
  "size": { "width": number, "height": number },
  "enabled": boolean,
  "focused": boolean
}
```

### Orchestrator Types
```rust
OrchestratorConfigDTO {
  max_parallel_tasks: usize,
  task_timeout_seconds: u64,
  enable_task_splitting: bool,
  enable_fallback_agents: bool,
  min_confidence_threshold: f32
}

OrchestratorStatusReport {
  orchestrator_available: bool,
  current_tasks: usize,
  total_tasks_delegated: usize,
  success_rate: f32,
  agent_statuses: Vec<AgentStatus>,
  active_task_count: usize
}
```

## Global Events

### Frontend → Backend
- `bar-state-changed` - Floating bar state updates
- `toggle-dictation-request` - Manual dictation toggle

### Backend → Frontend  
- `backend-response` - Agent execution results
- `agent-stopping` - Agent cancellation notification
- `mouse-entered-window` / `mouse-left-window` - Window hover events
- `app-dictation-started/finished/stopped` - Voice transcription events 

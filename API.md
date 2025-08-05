# API Reference

## Tauri Commands

### Agent Execution
```rust
submit_query(query: String) -> Result<String, String>
// Main orchestrator agent with hierarchical delegation

submit_orchestrated_query(query: String) -> Result<String, String>  
// Multi-agent coordination with specialized agents
```

### Desktop Automation
```rust
capture_screenshot() -> Result<String, String>
// Full screen screenshot, returns base64 image

capture_element_screenshot(x: i32, y: i32, width: i32, height: i32) -> Result<String, String>
// Element screenshot with coordinates

dev_click(x: f64, y: f64) -> Result<String, String>
// Mouse click at coordinates

dev_type(text: String) -> Result<String, String>
// Type text with keyboard simulation

dev_key(key: String) -> Result<String, String>
// Send key press (supports modifiers: cmd+c, alt+tab, etc.)

dev_scroll(x: i32, y: i32, direction: String) -> Result<String, String>
// Scroll in direction: up, down, left, right
```

### Browser Control
```rust
browser_navigate(url: String) -> Result<String, String>
// Navigate to URL, lazy initializes browser

browser_extract_content() -> Result<String, String>
// Extract page content as text

browser_screenshot() -> Result<String, String>
// Screenshot current browser page

browser_interact(selector: String, action: String) -> Result<String, String>
// Interact with page elements (click, type, etc.)
```

### Voice & TTS
```rust
play_tts(text: String, voice: String) -> Result<(), String>
// Text-to-speech with provider selection

stop_audio() -> Result<(), String>
// Stop current audio playback
```

### System Control
```rust
get_focused_window() -> Result<String, String>
// Get current focused window info

get_running_applications() -> Result<Vec<String>, String>
// List all running applications

focus_application(app_name: String) -> Result<String, String>
// Focus specific application
```

### Configuration
```rust
get_agent_mode() -> Result<String, String>
// Get current agent mode (Single/Multi)

set_agent_mode(mode: String) -> Result<(), String>
// Set agent mode configuration
```

## Computer Use Tools

### Official Anthropic Tools
- **computer_20250124**: All 17 actions (screenshot, mouse, keyboard, scroll, wait)
- **str_replace_based_edit_tool**: File operations (view, create, edit, insert)
- **bash_20250124**: Shell command execution

### Enhanced Tools
- **Timer Tools**: pause_agent, resume_agent with context monitoring
- **Delegation Tools**: delegate_to_browser_agent, delegate_to_desktop_agent, delegate_to_file_agent

## Voice Plugin API

### TypeScript Bindings
```typescript
import { startTranscription, stopTranscription } from 'tauri-plugin-voice-transcription-api'

// Start voice transcription
await startTranscription()

// Stop transcription
await stopTranscription()

// Listen to events
await listen('transcription-result', (event) => {
  console.log('Transcribed:', event.payload.text)
})
```

### Global Shortcuts
- **Alt+D**: Toggle Agent Mode voice input
- **Configurable Key** (default spacebar): Dictation Mode activation

## Error Responses

### Standard Error Format
```json
{
  "error": "ErrorType",
  "message": "Human readable description",
  "details": "Additional context"
}
```

### Common Error Types
- `PermissionDenied`: Missing macOS permissions
- `ToolNotFound`: Invalid tool request
- `ProviderError`: AI provider failure
- `ExecutionTimeout`: Operation exceeded time limit
- `ResourceNotAvailable`: Required resource unavailable

## Event System

### Backend → Frontend Events
```typescript
// Agent execution status
'agent-status-update': { status: 'running' | 'thinking' | 'completed' | 'error' }

// Voice transcription events
'dictation-started': {}
'partial-result': { text: string }
'dictation-finished': { text: string }

// Tool execution progress
'tool-execution': { tool: string, status: 'started' | 'completed' | 'error' }
```

## Rate Limiting

All API endpoints are protected by rate limiting to ensure system stability and prevent abuse. Rate limits are enforced per operation type:

### Rate Limits by Category

| Operation Type | Rate Limit | Description |
|----------------|------------|-------------|
| AI Operations | 20/minute | Expensive API calls (submit_query, etc.) |
| Shell Commands | 10/second | Security-sensitive operations |
| Screenshots | 5/second | Resource-intensive operations |
| File Operations | 100/second | Filesystem access |
| Browser Operations | 30/minute | Web automation tasks |

### Rate Limit Errors

When a rate limit is exceeded, commands return an error with:
- Error message indicating rate limit exceeded
- Retry-after duration in seconds
- User-friendly explanation

```typescript
// Example error response
{
  error: "Rate limit exceeded for AI operations. Please retry after 30 seconds."
}
```

### Implementation

Rate limiting uses a token bucket algorithm with:
- Automatic token refill based on time
- Burst capacity for occasional spikes
- Per-user tracking (currently using default user)
- Automatic cleanup of stale buckets

## Configuration Environment

### Required API Keys
```env
ANTHROPIC_API_KEY=your_key_here      # Primary AI provider
OPENAI_API_KEY=your_key_here         # Alternative provider  
ELEVENLABS_API_KEY=your_key_here     # TTS provider (optional)
```

### System Requirements
- macOS with accessibility permissions
- Screen recording permissions for screenshots
- Microphone access for voice features
- Node.js 18+ and Rust 1.70+ for development

All commands return Results with proper error handling and support async execution patterns.
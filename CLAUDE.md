# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

Juno is a Tauri v2 desktop application implementing Anthropic's Computer Use for macOS. It combines a React/TypeScript frontend with a Rust backend to provide AI-powered desktop automation with voice control, multi-agent orchestration, and MCP integration.

## Development Commands

```bash
# Setup
bun install && cp .env.example .env

# Full app development (Tauri + Vite)
bun run tauri:dev

# Frontend only (Vite dev server on port 1420)
bun run dev

# Build
bun run build                    # Frontend build (tsc + vite)
bun tauri build                  # Full production app
bun run build:universal          # Universal macOS binary

# Testing
npm test                         # Vitest (frontend)
npm run test:watch               # Watch mode
cargo test --manifest-path src-tauri/Cargo.toml   # Rust tests

# Rust compilation check (MANDATORY after Rust changes, ~15min)
cargo check --manifest-path src-tauri/Cargo.toml --message-format=short 2>&1 | tee cargo-check-results.log

# Linting
cargo clippy --manifest-path src-tauri/Cargo.toml

# Debug mode with self-awareness tools
RUST_LOG=debug bun run tauri dev

# Multi-instance development
bun run tauri:dev:multi
```

## Architectural Boundary: Backend Owns Logic, Frontend is Display-Only

This is the most critical architectural principle in Juno. Violating it creates bugs, breaks the CLI, and couples logic to the UI.

### The Rule

**Rust backend** = ALL business logic, I/O, state, control flow
**TypeScript frontend** = Display layer. Renders backend state. Sends user interactions via `invoke()`.

### What Lives Where

| Concern | Where | NEVER in |
|---------|-------|----------|
| Keyboard shortcuts | Rust (`shortcuts.rs`, global hotkeys) | TypeScript |
| Microphone recording | Rust (`tauri-plugin-voice-transcription`) | TypeScript (`getUserMedia`) |
| Audio playback / TTS | Rust (`tts/`, `say` command, ElevenLabs API) | TypeScript (`Web Audio API`) |
| Agent execution | Rust (`anthropic.rs`, agent system) | TypeScript |
| File system operations | Rust (agent tools, commands) | TypeScript |
| Shell commands | Rust (`commands/shell.rs`) | TypeScript |
| WebSocket connections | Rust (`cloud/connector.rs`, `tokio-tungstenite`) | TypeScript (`@tauri-apps/plugin-websocket`) |
| Settings persistence | Rust (Tauri Store) | TypeScript (localStorage) |
| Rendering chat messages | TypeScript (React components) | Rust |
| Styling / layout | TypeScript (Tailwind, shadcn/ui) | Rust |
| Animations / transitions | TypeScript (CSS, React) | Rust |
| User click → action routing | TypeScript calls `invoke()` | TypeScript runs the action directly |

### Why This Matters

1. **CLI independence**: Juno can run headlessly. The backend must function without any frontend.
2. **No browser APIs for native work**: `getUserMedia()`, `Web Audio API`, `WebSocket` in JS are wrong — we have native Rust equivalents with better performance and permissions.
3. **Single source of truth**: Backend emits events → frontend renders. Never the reverse.
4. **Third-party web libraries**: Libraries like ElevenLabs React SDK assume a web app. We use only their rendering/layout components (Conversation, Message, Response). Any component that calls `getUserMedia()`, `AudioContext`, or browser networking is off-limits.

### Frontend's Allowed Operations

The frontend may ONLY:
- Call `invoke('command_name', { params })` to request backend actions
- Listen for Tauri events (`useEventListener`) to receive state updates
- Read from Tauri Store for cached settings display
- Render UI based on data received from the backend
- Manage local UI state (modals open/closed, scroll position, animations)

---

## Architecture

### Workspace Structure

Cargo workspace with three members:
- `src-tauri/` — Main Tauri application (Rust backend)
- `src-tauri/mcp-server-os-level/` — macOS platform integration library
- `tauri-plugin-voice-transcription/` — Custom Whisper-based voice plugin

### Hierarchical Agent System

```
Orchestrator (src-tauri/src/anthropic.rs — submit_query entry point)
├── Desktop Agent — UI automation via macOS accessibility APIs
├── Browser Agent — Web automation, content extraction
├── File Agent — Filesystem operations with security controls
└── Tool Providers — Shared resources (browser, AI providers)
```

- **Orchestrator**: Uses persistent AppState memory (Arc-based), delegation tools only
- **Specialists**: Fresh `SimpleMemoryManager` instances (isolated per task)
- All memory managers use `Arc<TokioMutex<T>>` for thread safety

### Frontend → Backend Communication

**Tauri Commands** (frontend calls backend):
```typescript
import { invoke } from '@tauri-apps/api/core';
const result = await invoke<string>('submit_query', { query });
```

**Tauri Events** (backend pushes to frontend):
```rust
app_handle.emit("agent-text-stream", payload)?;
```
```typescript
// Preferred: useEventListener hook (handles cleanup + race conditions automatically)
import { useEventListener } from '@/hooks/useEventListener';
useEventListener<{ chunk: string }>('agent-text-stream', (payload) => { ... });

// Manual: must use mounted flag — listen() is async, cleanup is sync
// See useEventListener.ts for the canonical implementation
```

Key events: `agent-text-stream`, `agent-stream-start/end`, `provider_settings_changed`, `cloud-command-received`, `bar-state-update`

### Frontend Stack

- React 18 + TypeScript, Vite 6, Tailwind CSS 4
- shadcn/ui (Radix UI primitives) — 52 components in `src/components/ui/`
- Path aliases: `@/*` → `./src/*`, `~/*` → `./*`
- State: Tauri Store for persistence, React Context (`VoiceContext`), local state
- Multiple windows: main, floating-panel, floating-bar, onboarding, settings, desktop-cursor-overlay

### Backend Key Files

| File | Purpose |
|------|---------|
| `src-tauri/src/anthropic.rs` | Main orchestrator, `submit_query()` entry point |
| `src-tauri/src/state.rs` | Central `AppState` with all shared state |
| `src-tauri/src/commands/` | 50+ Tauri command handlers (organized by domain) |
| `src-tauri/src/agent/tools/` | 24 tool modules (computer use, browser, desktop, safari, MCP) |
| `src-tauri/src/agent/providers/` | AI provider integrations |
| `src-tauri/src/agent/prompts/` | Prompt management with `{{variable}}` substitution |
| `src-tauri/src/cloud/connector.rs` | WebSocket cloud connector with hardware monitoring |
| `src-tauri/src/menu/tray_menu.rs` | Dynamic system tray |

### Package Manager

Bun (uses `bun.lock`).

## Critical Development Rules

### Rust: Mandatory Compilation Check
After every substantial Rust change, the project MUST compile:
```bash
cargo check --manifest-path src-tauri/Cargo.toml
```

### Rust: No `.unwrap()` or `.expect()` in Production Code
```rust
// BANNED
value.unwrap();
value.expect("msg");

// USE INSTEAD
value.ok_or("error message")?;
value.unwrap_or_default();
value.unwrap_or_else(|| default);

// SystemTime pattern
SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_else(|_| Duration::from_secs(0));
```

### Rust: Always Use `tauri::async_runtime::spawn()`
`tokio::spawn()` causes "no reactor running" panics in Tauri context — not just event listeners, but **anywhere** in the Tauri app:
```rust
// WRONG — panics at runtime
tokio::spawn(async { ... });
tokio::task::spawn(async { ... });

// CORRECT
tauri::async_runtime::spawn(async { ... });

// For blocking operations (e.g., shell commands), use:
tokio::task::spawn_blocking(|| { std::process::Command::new("say").output() });
```

### Rust: Error Handling
- Use `AgentError` enum for agent errors, `Result<T, String>` for Tauri commands
- Never use `std::process::exit()` — use `app_handle.exit(0)` for Tauri-managed shutdown
- No string-based error detection (`.contains("timeout")`) — use structured error types
- Never use `std::env::set_var()` — it is unsafe in multithreaded programs
- Run `./scripts/detect-string-error-patterns.sh` to check

### Rust: String Safety
Never byte-slice strings — panics on multi-byte UTF-8:
```rust
// WRONG — panics if char boundary falls in multi-byte sequence
format!("{}...", &content[..50]);

// CORRECT
format!("{}...", content.chars().take(50).collect::<String>());
```

### Rust: Escape Key Management
Register escape key ONLY during agent execution (`submit_query`/`submit_orchestrated_query`). Always unregister on **every** exit path — including early returns, errors, and cancellation.

### Rust: Deadlock Prevention
Never hold an async mutex while calling a function that acquires another (or the same) mutex. Use check-init-recheck for lazy initialization:
```rust
// WRONG — deadlock if get_or_init_playwright also locks browser_controller
let guard = self.browser_controller.lock().await;
if guard.is_none() {
    let driver = self.get_or_init_playwright_driver().await?; // deadlocks
}

// CORRECT — release lock before expensive init, recheck after
{
    let guard = self.browser_controller.lock().await;
    if guard.is_some() { return Ok(guard.clone()); }
} // lock released
let new_controller = init_expensive_resource().await?;
let mut guard = self.browser_controller.lock().await; // reacquire
if guard.is_some() { return Ok(guard.clone()); } // double-check
*guard = Some(new_controller);
```

### Persistence: Tauri Store Pattern
All configuration MUST use Tauri store (`tauri_plugin_store::StoreExt`), not `std::env::set_var` or direct file I/O:
```rust
use tauri_plugin_store::StoreExt;
let store = app_handle.store("config_name.json").map_err(|e| format!("Failed: {}", e))?;
store.set("key", value);
store.save().map_err(|e| format!("Failed: {}", e))?;
```

### Cloud/WebSocket Architecture
- Backend: Native Rust WebSocket via `tokio-tungstenite` to `wss://juno-cloud-backend.fly.dev/ws`
- Frontend: Listens for events only — NEVER imports `@tauri-apps/plugin-websocket` (causes build failure)
- Authentication: HMAC-signed messages with device-specific API keys

### macOS Permissions
Always test **built apps** (not dev builds) for permission issues — they have different bundle identifiers. Required files: `src-tauri/juno.entitlements`, `src-tauri/Info.plist`, `src-tauri/tauri.conf.json` bundle config.

## Testing

Frontend tests mock Tauri APIs:
```typescript
vi.mock('@tauri-apps/api/core', () => ({ invoke: vi.fn() }));
vi.mock('@tauri-apps/api/event', () => ({
  listen: vi.fn(() => Promise.resolve(() => {})),
  emit: vi.fn()
}));
```

Test files: `src/components/__tests__/`, `src/test/setup.ts`. Rust tests use inline `#[cfg(test)]` modules with `#[tokio::test]`.

## Concurrency Patterns

- `Arc<TokioMutex<T>>` for shared async state
- `AtomicBool`/`AtomicUsize` for simple flags and counters
- Semaphores for limiting concurrent operations
- RAII patterns for resource cleanup
- Never hold multiple async locks simultaneously — release before acquiring another
- Use `tokio::task::spawn_blocking()` for blocking operations (shell commands, sync I/O)

## Security

- `SECURITY_AUDIT.md` — Tracked security vulnerabilities from 2026-02-08 audit (32 issues)
- See audit before making changes to: cloud/, agent/tools/, commands/shell.rs, browser_controller.rs

## Additional References

- `LLMs.txt` — Comprehensive AI agent instructions (1200+ lines)
- `src-tauri/CLAUDE.md` — Backend-specific guidance
- `src/CLAUDE.md` — Frontend-specific guidance
- `docs/rules/` — Development rules (13 files)
- `docs/` — Full documentation tree

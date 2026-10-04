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

# Build and check Rust: in CI, not locally (see "Rust: CI Compiles, Not Your Mac")
scripts/juno-build.sh            # Build this branch in CI, install + open it
scripts/juno-build.sh promote    # Ship the newest prerelease to users
gh pr checks --watch             # fmt + clippy + cargo test run on the PR

# Frontend (cheap, fine locally)
bun run build                    # Frontend build (tsc + vite)
bun run test                     # Vitest

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
| Window dragging | TypeScript (`useDragWindow` hook → `startDragging()`) | `data-tauri-drag-region` attribute |
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

## Model Policy

- **Default model: `claude-opus-5-5`** (Anthropic provider), the owner's choice (2026-10-04). Juno defaults to a model in Anthropic's *current* lineup that Juno can also drive the desktop with; never default to a model the docs list under "Legacy models (still available)". Existing saved model choices are untouched; new installs and Reset get Opus.
- **Opus 5.5 takes computer use only through `computer_toolset_20260801`** (a `computer_20251124` tool returns a 400, [what's new in Opus 5.5](https://platform.claude.com/docs/en/models/opus-5-5/whats-new-opus-5-5#computer-20251124-is-not-supported)). The toolset ships 17 member tool definitions and costs roughly **2x the input-token overhead on every request** (4,530 vs 2,152 input tokens on an identical call, measured 2026-09-22). That cost is accepted for the default.
- **Which computer-use shape Juno sends is a per-model decision, and the toolset is the exception, not the upgrade.** A model gets `computer_toolset_20260801` only when it accepts nothing earlier — today that is Opus 5.5 alone. Every other toolset-GA model (Fable 5.1, Fable 5, Opus 5, Sonnet 5, Opus 4.8) is wire-verified to accept `computer_20251124` and gets that. `toolset_ga` records which models *could* take the toolset; `computer_use` records what Juno actually sends, and a test asserts Juno never sends the toolset to a model that is not toolset-GA. The reverse is intentionally not asserted.
- **The toolset is a different request shape, not a version bump**: no `name` field on the tool entry, no display dimensions, **no beta header at all** (it is GA), 17 member tools instead of one `computer` tool with an `action` field, `toolset_name: "computer"` echoed on results, and batches that halt on the first failure. Both paths are live; changing one must not touch the other.
- The Anthropic provider (`src-tauri/src/agent/providers/anthropic.rs`) sends `thinking: {type: "adaptive", display: "summarized"}` on 4.6+ models, `fallbacks: "default"` on Fable/Opus 5 tier (server-side retry on a safety refusal), and replays the previous turn's thinking blocks on tool-use turns (the API rejects tool-use turns whose thinking blocks were dropped).
- Model IDs and capabilities live in **one table**: `Provider::model_definitions()` in `src-tauri/src/agent/providers/types.rs`. Each model declares its computer-use tool version, whether the provider lists it as Current or Legacy, whether it is GA on `computer_toolset_20260801`, its image tier, adaptive thinking, and server-side fallbacks. Every call site derives from that table — do not add a `&[&str]` model list anywhere, which is the drift this replaced. Verify new IDs and capabilities against the live provider docs, never from memory (LAC-3106).
- **Browser use is the same question with the opposite answer: Juno does NOT send `browser_toolset_20260801`, and that is deliberate.** Anthropic's browser toolset is GA, takes no beta header, and is client-side, so Juno's chromiumoxide/CDP stack is the right substrate in principle. It is ruled out on cost, not architecture: it costs **3.8x the input-token overhead of Juno's own browser tools** (6,613 vs 1,755 input tokens on an identical `claude-opus-5-5` call, measured against the live API 2026-09-22), or 10,849 stacked with the computer toolset Opus 5.5 already requires. Nothing forces it the way Opus 5.5 forces the computer toolset: Juno's browser tools are plain custom tools and every catalog model accepts them. The wire-verified contract, the measurements, and what would change the decision are in `docs/plans/browser-toolset-20260801.md`. **Do not add a browser toolset path without re-measuring and re-reading that file.**
- `availability` (the provider's lifecycle label) and `toolset_ga` (computer-use toolset support) are **separate axes**. Opus 4.8 is toolset-GA and legacy; Haiku 4.5 is current and not toolset-GA. Conflating them is what made a legacy model the default in #580.

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
| `src-tauri/src/agent/providers/` | AI provider integrations (Anthropic, OpenAI, Gemini, Claude CLI) |
| `src-tauri/src/agent/providers/claude_cli.rs` | Claude CLI subprocess provider — no API key needed |
| `src-tauri/src/agent/prompts/` | Prompt management with `{{variable}}` substitution |
| `src-tauri/src/cloud/connector.rs` | WebSocket cloud connector with hardware monitoring |
| `src-tauri/src/menu/tray_menu.rs` | Dynamic system tray |

### Package Manager

Bun (uses `bun.lock`).

### Claude CLI Provider

Alternative to direct API keys — uses the locally installed `claude` binary (Claude Code) as a subprocess. Users with a Claude Max/Pro subscription can use Juno through their existing CLI authentication.

**How it works**: Spawns `claude -p --output-format=stream-json --model <model> --dangerously-skip-permissions "query"` per query. Parses NDJSON output and emits the same Tauri streaming events as the Anthropic API provider.

**Key flags**:
- `-p` — Print mode (non-interactive, pipe-friendly)
- `--include-partial-messages` — Raw streaming events, not just finished messages. This is what makes reasoning, tool names and tool arguments visible *while* Claude works; without it the first visible text lands only at the end of the turn. A `claude` too old to accept it exits non-zero with empty stdout, so `run_streaming` retries once with the flag stripped and latches `PARTIAL_MESSAGES_UNSUPPORTED` for the session (degraded, never failed).
- `--effort <low|medium|high|xhigh|max>` — Hidden advanced setting (`providers[].effort` in the settings store, no UI), default `high`. An unrecognised value is dropped rather than forwarded.
- `--strict-mcp-config` — Passed only when the "Load account MCP connectors" setting (advanced, `providers[].load_account_mcp`, default on) is **off**: then only MCP servers from `--mcp-config` load. By default the flag is omitted, so the person's claude.ai connectors (Slack, Gmail, Drive) and user-level servers load alongside Juno's tool server (LAC-4056)
- `--mcp-config <path>` — Points the CLI at Juno's **own** computer tool, served from inside the running app (`agent/providers/juno_mcp.rs`): streamable HTTP on loopback, a bearer token minted per app run, one `computer` tool backed by the same `run_computer_action` the API provider calls. Written to a pid-scoped temp file so the token never appears in `ps`.
- `--append-system-prompt` — Added alongside `--mcp-config`: steers the model toward the MCP tool instead of `cliclick`/`screencapture` via Bash
- `--dangerously-skip-permissions` — Required because stdin is null; CLI can't prompt for tool permissions (MCP tools also run without prompting)

**Persistent session (beta, default OFF)**: `cli_persistent_session_enabled` in the settings store, surfaced as a beta toggle under Settings → Advanced. Keeps one `claude` process alive per conversation (`--input-format stream-json`, messages over stdin), removing the 1.6–3.1s per-follow-up spawn overhead. Escape sends a `control_request`/`interrupt` instead of killing the process. Every user message carries a `uuid` the CLI echoes as `command_uuid` on `command_lifecycle` frames; a turn is rendered only if such a frame opened it, which is what makes turns the CLI starts on its own (observed: a finishing background Bash task) invisible rather than mistaken for answers. Any turn the persistent path does not complete kills the process and falls back to the one-shot `--resume` path with no context loss. `shutdown_all()` runs on `RunEvent::Exit`. Module: `src-tauri/src/agent/providers/claude_cli_session.rs`; spike with measurements: `docs/plans/cli-persistent-session-spike.md`.

**Auth**: Checked once per session via `claude auth status --json`, cached with `AtomicBool`. Uses OAuth/keychain (not API key).

**Models**: `opus`, `sonnet`, `haiku` (CLI aliases — resolves to latest versions automatically)

**Computer use is not degraded on this path, and the note that used to say it was is wrong.** Desktop automation used to be delegated to the separate `juno-cua` binary, which meant the mouse moved without Juno knowing: the smooth-movement setting went unread and the cursor overlay went untold. It is now served in-process, so the cursor overlay (`systemPink`, identity `claude-cli`), the smooth-movement setting, and AX click verification all apply exactly as they do on the API provider. **`juno-cua` is not involved, and does not ship in the bundle** — `tauri.conf.json` declares no `externalBin` and its `resources` do not include it. Bash, Read and Edit remain the CLI's own tools, deliberately: they need no desktop, and routing them through Juno would add a hop for nothing.

**Limitations**: Without an `AppHandle` (headless and test paths) no tool server is started and the CLI runs toolless. Escape cancels a running CLI query by killing the subprocess (LAC-3697).

**Default provider**: When nobody has picked a provider, the provider that would otherwise run has no credential, and the CLI is installed *and* signed in, Juno selects the Claude CLI at launch rather than asking for an API key — someone on Claude Max already pays for this. The rule, and its reverse (Juno gives the CLI up if it made the choice and the CLI later disappears or signs out), lives in `agent/providers/default_selection.rs` as a pure function; the I/O around it is `agent/providers/startup_default.rs`, spawned unawaited so launch never waits on a subprocess. A choice made in Settings or in setup sets `ProviderSettings::provider_chosen_by_user` and is never overruled.

## Critical Development Rules

### Rust: CI Compiles, Not Your Mac
Do not run `cargo check`, `cargo clippy`, `cargo test` or `tauri build` locally. Zero is a 16 GB M1 shared by every agent, and one Juno build fills it. GitHub's macOS runners do it for free (public repo).

1. `cargo fmt --manifest-path src-tauri/Cargo.toml --all` (formatting only, compiles nothing).
2. Commit, push, open a draft PR against `main`. `ci.yml` runs fmt, clippy `-D warnings` and tests.
3. `gh pr checks --watch`, then `gh run view <id> --log-failed` on a red check. Clippy lists every warning in one pass, so fix them all before pushing again.
4. To run the app: `scripts/juno-build.sh` builds the current branch in CI, installs it in /Applications and opens it. `scripts/juno-build.sh v0.8.12` installs a published release or prerelease.

Local cargo is the exception: only when the task is a build-system change that CI cannot show you, and say so in the PR.

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
Register escape key ONLY during agent execution (`submit_query`, the single entry point; `submit_orchestrated_query` and the other thirteen orchestrator commands were removed with the ungated executor behind them). Always unregister on **every** exit path — including early returns, errors, and cancellation. The stop key is *observed* with a passive NSEvent monitor (`platform/stop_key_monitor.rs`) that never consumes the key — never register a bare Escape as an exclusive global hotkey (LAC-3746).

### Rust: Deadlock Prevention
Never hold an async mutex while calling a function that acquires another (or the same) mutex. Use check-init-recheck for lazy initialization:
```rust
// WRONG — deadlock if the init path also locks browser_controller
let guard = self.browser_controller.lock().await;
if guard.is_none() {
    let controller = BrowserController::new().await?; // deadlocks
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

## Auto-Update and Release Channels

Juno keeps itself current. The policy lives in one file, `src-tauri/src/updater.rs`, and the UI decides nothing: it renders an `UpdateStatus` and calls three commands.

**The schedule.** `updater::spawn_schedule` runs one check 20s after launch, then every 6 hours. It re-reads the channel and the on/off flag from the store on every tick, so a change in Settings lands at the next check rather than the next launch. Anything found is downloaded and installed in the background. **Nothing ever relaunches on its own** — on macOS the bundle is swapped on disk while the running process carries on, so the new version starts the next time Juno does. Juno is a dictation tool; a self-chosen relaunch would land mid-sentence.

**Two channels, because there are two audiences.** `release-every-merge.yml` cuts a version on every merge and publishes it as a GitHub *prerelease*. GitHub's `/releases/latest` excludes prereleases, so:

| Channel | Feed | Sees |
|---|---|---|
| `stable` | `releases/latest/download/latest.json` | Only releases promoted with `juno-build promote` |
| `prerelease` (default) | `releases/download/canary/latest.json` | Every build, within the hour |

The canary feed is a **fixed tag whose `latest.json` every release build overwrites** (the "Publish the canary feed" step in `release-tauri.yml`), precisely because `/releases/latest` is the thing that cannot see prereleases. That step has a version guard: release builds finish out of order, and an older build overwriting the manifest would hand everyone a downgrade.

The default is `prerelease` while Juno's own team are the only testers. Flip `defaults::UPDATE_CHANNEL` to `"stable"` before there are users who did not sign up to find the bugs.

**The endpoint is chosen at runtime** via `UpdaterExt::updater_builder().endpoints(...)`, not from `tauri.conf.json`. The `pubkey` there still applies to both feeds: the channel decides which manifest is read, never whether the signature is checked. Rotating the signing key still breaks auto-update silently for everyone on an older build, because a failed signature check is indistinguishable from "no update available".

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

- `docs/audits/security-audit-2026-02-08.md`: tracked security vulnerabilities from the 2026-02-08 audit (32 issues, with 2026-09-11 status annotations)
- See audit before making changes to: cloud/, agent/tools/, commands/shell.rs, browser_controller.rs

## PR feature media

UI PRs attach evidence (see the `insanely-great` demo test). Capture only the new or changed feature in this PR, never unchanged screens. Commit it into the PR at `docs/changelog/media/<PR number>/` with descriptive names (`trigger-sentences-add.png`, `double-tap.mp4`), embed it in the PR body, and keep recordings small (under 5 MB, mp4/webm, no audio unless the feature is audio). Nothing lives only in `/tmp`. No private data. Fleet rule: `~/repo/paperclip-company/knowledge/agent-common.md` (PR feature media).

## Additional References

- `LLMs.txt` — Short pointer for AI agents (this file is canonical; the old 1,200-line version is in `docs/legacy/`)
- `src-tauri/CLAUDE.md` — Backend-specific guidance
- `src/CLAUDE.md` — Frontend-specific guidance
- `docs/rules/` — Development rules (13 files)
- `docs/` — Full documentation tree

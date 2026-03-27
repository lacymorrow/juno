# External Agent Integration Plan

**Goal**: Allow any CLI coding agent (Claude Code, Codex, OpenCode, Gemini CLI, etc.) to discover and use Juno's capabilities — maximizing adoption across the agent ecosystem.

**Date**: 2026-03-27
**Status**: Planning

---

## Core Insight

There are **three integration layers**, ordered by friction:

1. **CLI** (zero-dependency) — Any agent with shell access calls `juno` directly
2. **MCP Server** (structured) — Agents connect via standard MCP protocol
3. **Skill/Discovery** (onboarding) — Agents learn Juno exists and how to use it

All three should exist. The CLI is the foundation; MCP adds structure; skills add discoverability.

---

## What Juno Already Has

| Asset | Status | Notes |
|-------|--------|-------|
| CLI (13+ subcommands, headless mode) | Working | `juno query`, `juno computer`, `juno voice`, etc. |
| `mcp-server-os-level/` library | Working | 20+ desktop tools, `call_tool()` + `list_tools()` |
| 336 Tauri commands | Working | Full capability surface |
| Headless runtime (76KB) | Working | Executes without UI |
| Cloud WebSocket | Working | Proprietary protocol |
| Axum dependency | Unused | In Cargo.toml, never wired up |
| Homebrew tap | Exists | `lacymorrow/tap` repo |

---

## Phase 1: CLI-First Integration (Lowest Friction)

**Why CLI first**: Every agent (Claude Code, Codex, OpenCode, Gemini CLI, Goose) has a Bash/shell tool. Zero configuration needed. If Juno is on PATH, agents can use it immediately.

### 1A. Ensure CLI is solid for agent consumption

- [ ] `juno query "..."` — submit query, get text response on stdout
- [ ] `juno query --output json "..."` — structured JSON output for programmatic use
- [ ] `juno computer screenshot` — capture screenshot, return base64 or file path
- [ ] `juno computer click --x 100 --y 200` — individual computer use actions
- [ ] `juno system info` — system context (screen size, running apps, permissions)
- [ ] `juno system check` — verify permissions are granted
- [ ] Exit codes: 0 = success, 1 = error, 2 = permissions missing
- [ ] All commands work without the Juno GUI running (headless)

### 1B. Machine-readable output

All commands should support `--output json` for structured responses:
```bash
$ juno computer screenshot --output json
{"type": "screenshot", "path": "/tmp/juno-screenshot-1234.png", "width": 2560, "height": 1600}

$ juno query --output json "what's on screen?"
{"response": "I can see a terminal window with...", "tools_used": ["screenshot"], "duration_ms": 3200}
```

### 1C. `juno --help-for-agents` or `juno capabilities`

A special command that outputs a concise tool catalog designed for LLM consumption:
```bash
$ juno capabilities
# Juno Desktop Automation - Available Commands
## Computer Use
- juno computer screenshot — Capture the screen (returns file path)
- juno computer click --x <X> --y <Y> — Click at coordinates
- juno computer type --text "hello" — Type text
...
```

This is what a skill/prompt would include so agents know what's available.

---

## Phase 2: Skill for Agent Ecosystems (Discovery)

**Publish to skills.sh** so any of the 17+ supported agents can install with:
```bash
npx skills add lacymorrow/juno
```

### Skill structure

```
.claude/skills/juno/SKILL.md   (or published as GitHub repo)
```

The SKILL.md teaches agents:
1. How to detect if Juno is installed
2. What capabilities are available
3. When to use Juno vs built-in tools
4. Usage patterns and examples

### Key content areas

- **Detection**: `which juno` or `juno system check`
- **Installation**: `brew install lacymorrow/tap/juno`
- **CLI usage**: Direct `juno query`, `juno computer` commands
- **MCP setup**: For agents that support MCP (bonus, not required)
- **Decision framework**: "Use Juno when you need to interact with GUI apps, take screenshots, automate browser, or run multi-step desktop workflows"

### Cross-agent compatibility

The skill works with: Claude Code, Codex, OpenCode, Gemini CLI, Cursor, Goose, GitHub Copilot, Windsurf, and 9+ others via the skills ecosystem.

---

## Phase 3: MCP Server (Structured Tool Access)

**Why MCP in addition to CLI**: MCP provides typed tool schemas, streaming results, and native integration with agent tool-calling loops. Agents call MCP tools the same way they call built-in tools — no shell parsing needed.

### 3A. `juno serve-mcp` subcommand

New CLI subcommand that starts Juno as an MCP server:

```bash
# stdio mode (most compatible — works in .mcp.json)
juno serve-mcp

# HTTP mode (for network/remote access)
juno serve-mcp --transport http --port 7867
```

**Agent config** (`.mcp.json`):
```json
{
  "mcpServers": {
    "juno": {
      "command": "juno",
      "args": ["serve-mcp"]
    }
  }
}
```

### 3B. Two operating modes

**Standalone mode** (default): Loads `mcp-server-os-level` library directly.
- Computer use (screenshot, click, type, scroll, key)
- Desktop automation (find element, click element, list windows)
- Clipboard, mouse, keyboard
- No orchestrator, no browser controller, no voice

**Full mode** (`--connect-app`): Connects to running Juno app via IPC.
- Everything in standalone mode
- `query` tool — full multi-agent orchestrator
- Browser automation (Playwright)
- Voice/TTS
- Safari automation
- Custom UI rendering

### 3C. Tool catalog for MCP

Priority tools to expose:

| Tool | Category | Description |
|------|----------|-------------|
| `screenshot` | computer | Capture screen, return image |
| `click` | computer | Click at x,y coordinates |
| `type_text` | computer | Type text at cursor |
| `press_key` | computer | Press keyboard key/combo |
| `scroll` | computer | Scroll at position |
| `mouse_move` | computer | Move cursor |
| `find_element` | desktop | Find UI element by selector |
| `click_element` | desktop | Click element by selector |
| `list_windows` | desktop | List open windows |
| `open_application` | desktop | Launch application |
| `get_clipboard` | system | Read clipboard |
| `set_clipboard` | system | Write clipboard |
| `bash` | system | Execute shell command |
| `query` | agent | Submit to Juno orchestrator (full mode) |
| `browser_navigate` | browser | Navigate browser (full mode) |
| `browser_extract` | browser | Extract page content (full mode) |

### 3D. Technical implementation

- Use `rmcp` or `mcp-rs` crate for MCP protocol handling
- stdin/stdout JSON-RPC for stdio transport
- Axum (already in Cargo.toml) for HTTP transport
- Tool registry maps MCP tool names → existing headless runtime functions

---

## Phase 4: Distribution & Discoverability

### 4A. Homebrew formula
```bash
brew install lacymorrow/tap/juno
```
Update existing `homebrew-tap/` repo with current build.

### 4B. MCP Registry
```bash
mcp-publisher init
mcp-publisher publish
```
Publish to `registry.modelcontextprotocol.io` — discoverable by GitHub Copilot, Cursor, etc.

### 4C. `npx @juno/setup` (zero-friction onboarding)

Thin npm package that:
1. Detects installed agents (Claude Code, Codex, Cursor, etc.)
2. Checks if Juno is installed, suggests `brew install` if not
3. Adds Juno to each agent's MCP config (`.mcp.json`, etc.)
4. Installs the skill
5. Tests the connection

### 4D. GitHub README / website
- Badge: "Works with Claude Code, Codex, OpenCode, ..."
- One-line setup instructions
- Demo GIF showing an agent using Juno

---

## Phase 5: Plugin Package (Claude Code Specific)

Bundle skill + MCP server + hooks as a Claude Code plugin for the official marketplace.

---

## Strategic Moat: The `query` Tool

Most MCP servers offer simple tools (read file, search, etc.). Juno's `query` tool is unique:

```
Agent → juno query "find the login button and click it"
        └→ Juno orchestrator
           ├→ Screenshot
           ├→ Visual analysis
           ├→ Element detection
           └→ Click action
        ← "Done. Clicked the login button at (450, 320)."
```

**No other MCP server offers a full multi-agent orchestrator as a single tool call.** This is what makes Juno sticky — external agents can delegate complex desktop workflows to Juno without managing the multi-step loop themselves.

---

## Implementation Priority

| # | Phase | Effort | Impact | Dependency |
|---|-------|--------|--------|------------|
| 1 | CLI polish (1A-1C) | Small | High | None |
| 2 | Skill on skills.sh | Small | High | Phase 1 |
| 3 | Homebrew formula | Small | Medium | Phase 1 |
| 4 | `juno serve-mcp` (standalone) | Medium | High | Phase 1 |
| 5 | `npx @juno/setup` | Small | Medium | Phase 3-4 |
| 6 | MCP Registry | Small | Medium | Phase 4 |
| 7 | Full MCP mode (connect to app) | Medium | Very High | Phase 4 |
| 8 | HTTP transport | Medium | Medium | Phase 4 |
| 9 | Claude Code plugin | Small | Medium | Phase 2+4 |

---

## Success Metrics

- Number of `npx skills add` installs (skills.sh telemetry)
- MCP server connections per day
- CLI invocations from non-Juno processes
- Homebrew install count
- GitHub stars / forks on the skill repo

---

## Open Questions

- Should standalone MCP mode require the full Juno binary, or should we ship a separate lightweight `juno-mcp` binary?
- Should the skill auto-configure `.mcp.json`, or just provide instructions?
- Rate limiting / security for MCP server (local-only by default, auth for HTTP?)
- Should `juno serve-mcp` work on Linux (subset of tools) or macOS only?

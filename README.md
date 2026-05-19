<p align="center">
  <img src="public/juno5.png" alt="Juno" width="128" height="128" />
</p>

<h1 align="center">Juno</h1>

<p align="center">
  <strong>AI that controls your Mac.</strong><br/>
  Describe a task. Watch it happen.
</p>

<p align="center">
  <a href="https://junebug.ai">Website</a> &middot;
  <a href="#install">Install</a> &middot;
  <a href="#what-can-juno-do">Use Cases</a> &middot;
  <a href="#for-ai-agents">Agent Integration</a> &middot;
  <a href="#development">Development</a>
</p>

<p align="center">
  <img alt="GitHub stars" src="https://img.shields.io/github/stars/lacymorrow/juno?style=flat-square" />
  <img alt="Version" src="https://img.shields.io/github/v/release/lacymorrow/juno?style=flat-square&color=blue" />
  <img alt="License" src="https://img.shields.io/badge/license-FSL--1.1--MIT-green?style=flat-square" />
  <img alt="macOS" src="https://img.shields.io/badge/macOS-14%2B-black?style=flat-square&logo=apple" />
  <img alt="npm" src="https://img.shields.io/npm/v/juno-cua?style=flat-square&color=red" />
</p>

<!-- TODO: Replace with demo GIF/video when available -->
<!-- <p align="center"><img src="docs/demo.gif" alt="Juno demo" width="600" /></p> -->

---

Juno is a native macOS desktop app that gives AI full control of your computer. It uses Anthropic's Computer Use to see your screen, move the mouse, type on the keyboard, and complete tasks autonomously.

You talk (or type). Juno does the rest.

## What can Juno do?

Anything you can do with a mouse and keyboard:

- **Fill out forms** — "Apply to this job posting with my resume"
- **Research and summarize** — "Find the 5 best-reviewed coffee shops near me and put them in a spreadsheet"
- **Automate workflows** — "Download all invoices from my email and organize them by date"
- **Navigate apps** — "Open Figma, export this frame as PNG, and upload it to Slack"
- **Browse the web** — "Go to HN, find posts about Rust, and bookmark the top 3"
- **Manage files** — "Sort my Downloads folder — move images to Photos, PDFs to Documents"

Juno sees your screen, understands what's on it, and takes action. No scripting. No macros. Just describe what you want.

## Key Features

**Native macOS app** — Built with Tauri and Rust. Fast, lightweight, no Electron. Uses real macOS accessibility APIs for precise interaction.

**Voice control** — Say "Hey Juno" (or set a custom wake word) and speak your task. Always-on listening with local Whisper transcription. 99+ languages.

**Multi-agent orchestration** — Complex tasks get split across specialized agents (Desktop, Browser, File) coordinated by an orchestrator. They work in parallel.

**MCP integration** — Extend Juno with any Model Context Protocol server. Add custom tools, connect to APIs, integrate with your existing stack.

**Floating UI** — A minimal, always-on-top bar that stays out of your way. Expand to chat, collapse to a sliver, or hide completely.

**CLI for AI agents** — `juno-cua` gives Claude Code, Cursor, Codex, and other AI coding agents the ability to see and control your desktop.

## Install

### Desktop App (macOS 14+)

Download the latest `.dmg` from [junebug.ai](https://junebug.ai) or [GitHub Releases](https://github.com/lacymorrow/juno/releases).

### Homebrew

```bash
brew install lacymorrow/tap/juno-cua
```

### npm (CLI / Agent Integration)

```bash
npx juno-cua
```

This installs the `juno-cua` tool that gives AI coding agents (Claude Code, Cursor, Codex, Gemini CLI) desktop automation capabilities via MCP.

## For AI Agents

Juno bridges the gap between AI coding agents and the real desktop. With `juno-cua`, your agent can:

- Take screenshots and analyze what's on screen
- Click, type, scroll, and drag
- Open apps and URLs
- Read the accessibility tree for precise element targeting
- Chain actions: screenshot → analyze → click → type

```bash
# Give Claude Code desktop superpowers
npx juno-cua
```

Works with Claude Code, Cursor, Codex, Gemini CLI, and any agent that supports MCP tools.

## How It Works

```
You (voice or text)
  → Orchestrator (routes to specialist agents)
    → Desktop Agent (screen, mouse, keyboard)
    → Browser Agent (web automation, scraping)
    → File Agent (filesystem operations)
  → Task complete
```

Juno uses Anthropic's Computer Use API to give Claude vision and control of your screen. The orchestrator breaks complex tasks into subtasks and delegates to specialist agents that run in parallel. All processing happens locally — your screen data stays on your machine.

## Architecture

| Layer | Tech | Purpose |
|-------|------|---------|
| Frontend | React, TypeScript, Vite, Tailwind | Display layer, chat UI, floating bar |
| Backend | Rust, Tauri v2 | All business logic, agent execution, I/O |
| Voice | Custom Whisper plugin | Local speech-to-text, always-on listening |
| AI | Anthropic Claude (Computer Use) | Vision, reasoning, tool calling |
| Platform | macOS Accessibility APIs | Screen capture, UI automation |
| Extensions | MCP (Model Context Protocol) | Custom tool servers |

The backend owns everything — the frontend is a thin display layer. Juno can run headlessly as a CLI without any UI.

## Development

```bash
# Prerequisites: Rust, Bun, macOS 14+

# Setup
bun install && cp .env.example .env

# Run the full app (Tauri + Vite)
bun run tauri:dev

# Frontend only (Vite dev server on :1420)
bun run dev

# Run tests
bun test              # Frontend (Vitest)
cargo test --manifest-path src-tauri/Cargo.toml  # Backend

# Build
bun tauri build       # Production app
```

See [CLAUDE.md](CLAUDE.md) for detailed development rules and architecture docs.

## Project Structure

```
juno/
├── src/                    # React frontend
├── src-tauri/              # Rust backend (Tauri v2)
│   ├── src/
│   │   ├── anthropic.rs    # Main orchestrator
│   │   ├── agent/          # Multi-agent system (tools, providers, prompts)
│   │   ├── commands/       # 50+ Tauri command handlers
│   │   └── cloud/          # Cloud sync & device management
│   └── mcp-server-os-level/  # macOS platform library
├── tauri-plugin-voice-transcription/  # Custom Whisper plugin
└── packages/juno-cua/      # CLI & MCP server for AI agents
```

## Security

Juno includes enterprise-grade security controls:

- Tiered permission system (5 levels) for agent actions
- File path validation with workspace boundary enforcement
- Command execution whitelist
- Tool approval system for sensitive operations
- Full audit logging

See [SECURITY_AUDIT.md](SECURITY_AUDIT.md) for details.

## License

[FSL-1.1-MIT](LICENSE) — Source-available. Free to use, modify, and create derivative works. Converts to MIT after 2 years.

## Community

- [junebug.ai](https://junebug.ai) — Website & download
- [Discussions](https://github.com/lacymorrow/juno/discussions) — Questions, use cases, ideas
- [GitHub Issues](https://github.com/lacymorrow/juno/issues) — Bug reports & feature requests
- [Blog](https://junebug.ai/blog) — Technical articles about building Juno
- [@junebug_ai](https://x.com/junebug_ai) — Product updates
- [@lacybuilds](https://x.com/lacybuilds) — Dev updates from the builder

Contributions welcome. See [CLAUDE.md](CLAUDE.md) for architecture docs and development rules.

---

<p align="center">
  Built by <a href="https://github.com/lacymorrow">Lacy Morrow</a>
</p>

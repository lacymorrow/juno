# Reddit Launch Posts

---

## r/artificial

### Title
I built a Mac app where AI controls your computer -- it sees your screen and uses the mouse and keyboard to complete tasks

### Body
I've been building Juno for the past few months. It's a native macOS desktop app that uses Anthropic's Computer Use API to give AI full control of your computer.

You describe a task in natural language (or speak it), and the AI takes over -- it sees your screen, moves the mouse, types on the keyboard, and works through the task step by step.

**Example tasks:**
- "Fill out this job application with my resume"
- "Find the cheapest flights to NYC next weekend and screenshot the results"
- "Go through my Downloads folder and organize everything by file type"
- "Open Slack, find the #engineering channel, and summarize what I missed today"

**How it's different from browser-only AI agents:**
Juno works with your entire desktop, not just a browser tab. It can interact with Finder, Figma, Slack, Terminal, Xcode -- any app on your Mac. It uses real macOS Accessibility APIs so it can target specific UI elements precisely.

**Voice control:** Say "Hey Juno" and speak your task. Local Whisper transcription -- nothing leaves your machine.

**Multi-agent:** Complex tasks get split across specialist agents (Desktop, Browser, File) that run in parallel.

**For AI coding agents:** I also built `juno-cua` which gives Claude Code, Cursor, and Codex the ability to control your desktop via MCP tools.

Tech: Tauri v2 + Rust (not Electron), ~15MB.

Website: https://junebug.ai
GitHub: https://github.com/lacymorrow/juno

---

## r/MacApps

### Title
Juno -- AI-powered desktop automation for macOS. Describe a task, watch it happen.

### Body
Hey r/MacApps -- I built Juno, a native Mac app (Tauri + Rust, not Electron) that uses AI to automate anything on your desktop.

Instead of writing scripts or recording macros, you just describe what you want in natural language. The AI sees your screen and uses the mouse and keyboard to get it done.

**Some things I use it for:**
- Organizing files across folders
- Filling out repetitive forms
- Researching and comparing options across multiple tabs
- Navigating complex app workflows

It also has voice control -- say "Hey Juno" and describe what you need. Transcription happens locally via Whisper.

**Native macOS:**
- Uses Accessibility APIs for precise interaction
- Lightweight (~15MB, Rust backend)
- Floating bar UI that stays out of your way
- Works with any app on your Mac

Requires macOS 14+ and Accessibility permissions.

Download: https://junebug.ai
Source: https://github.com/lacymorrow/juno

---

## r/Anthropic

### Title
Built a macOS app on top of Computer Use -- AI agent that controls your desktop

### Body
I've been building with the Computer Use API since it launched and wanted to share what I've made: Juno, a macOS desktop app that turns Claude into a full desktop automation agent.

**Architecture (for the technically curious):**
- Hierarchical agent system: Orchestrator delegates to specialist agents (Desktop, Browser, File)
- Each specialist has a fresh memory context (isolated per task)
- Orchestrator has persistent memory across tasks
- Screenshots are JPEG-compressed with quality 85 (PNG for zoom operations)
- Action cooldown (500ms between UI interactions) to prevent race conditions
- AX (Accessibility) verification on all 14 UI action types
- Prompt caching for cost reduction

**Voice integration:**
Custom Whisper plugin for local speech-to-text. Say "Hey Juno" + your task. Transcription never leaves your machine.

**MCP support:**
You can extend Juno with custom MCP tool servers. Also ships as an MCP server itself (`juno-cua`) so other agents like Claude Code can use it.

**Security:**
5-level permission system, command whitelisting, tool approval for sensitive actions, audit logging. Wrote up the full audit: SECURITY_AUDIT.md in the repo.

The Computer Use API is incredible to build on. Happy to answer questions about the implementation.

GitHub: https://github.com/lacymorrow/juno
Website: https://junebug.ai
CLI: `npx juno-cua`

---

## Posting Notes

- **r/artificial:** Post first (largest audience, most general). Emphasize the wow factor.
- **r/MacApps:** Post 24 hours later. Emphasize native, lightweight, not Electron. This audience cares about app quality.
- **r/Anthropic:** Post 24-48 hours later. Go deep on technical details. This audience understands Computer Use.
- **r/LocalLLaMA:** Skip for now -- Juno uses Anthropic's API, not local models. Could revisit if local model support is added.
- **General rules:** Don't self-promote aggressively. Frame as "built this, sharing with you" not "buy my product." Answer every question. Be honest about limitations (macOS only, requires Anthropic API key, etc.).

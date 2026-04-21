---
title: "How I Built a Desktop AI Agent with Anthropic Computer Use and Tauri"
published: false
description: "Building a native macOS app where AI sees your screen and controls your computer. Lessons from shipping a multi-agent system with Rust, Tauri v2, and Claude Computer Use."
tags: rust, ai, tauri, anthropic
cover_image: # TODO: Add hero image
---

# How I Built a Desktop AI Agent with Anthropic Computer Use and Tauri

When Anthropic released the Computer Use API, I saw every demo go viral. People couldn't stop sharing videos of Claude using a computer. But every implementation was the same: a Docker container, a VNC session, a virtualized desktop.

I wanted to build something different. A real desktop app. Native macOS. No virtualization. The AI controls *your* actual computer.

That's Juno.

## The idea

The core concept is simple: you describe a task, and AI completes it by controlling your desktop like a human would -- seeing the screen, moving the mouse, clicking, typing.

```
"Fill out this job application with my resume"
"Find the cheapest flight to NYC next weekend"
"Organize my Downloads folder by file type"
```

No scripting. No macros. No browser extensions. The AI works with any app on your Mac.

## Why Tauri + Rust (not Electron)

I chose Tauri v2 for a few reasons:

1. **Native macOS APIs.** Juno needs Accessibility APIs for UI automation, ScreenCaptureKit for screenshots, and low-level input simulation. Rust gives direct access to all of this via `objc2` and system frameworks.

2. **Performance.** The app takes screenshots, compresses them, sends them to an API, processes the response, and executes actions -- in a loop. Every millisecond of overhead matters. The final binary is ~15MB.

3. **Security boundary.** The Rust backend owns ALL business logic. The TypeScript frontend is purely a display layer. This means Juno can run headlessly as a CLI -- the UI is optional.

The strict backend/frontend separation was the best architectural decision I made. It forced clean APIs, made testing straightforward, and gave me a CLI for free.

## Multi-agent architecture

A single AI agent struggles with complex tasks. It loses context, gets confused, makes mistakes. So I built a hierarchical system:

```
Orchestrator
├── Desktop Agent (screen, mouse, keyboard)
├── Browser Agent (web automation, extraction)
└── File Agent (filesystem operations)
```

The **orchestrator** receives the user's task, breaks it into subtasks, and delegates to specialists. Each specialist has:

- Its own fresh memory context (isolated per task)
- Domain-specific tools (the Desktop Agent can't access file tools)
- An iteration limit (prevents infinite loops)

The orchestrator has persistent memory across tasks, so it learns about your desktop over time.

### Why this matters

When you say "Research the top 5 coffee shops near me and put them in a spreadsheet," the orchestrator:

1. Delegates "search for coffee shops" to the Browser Agent
2. Delegates "create spreadsheet" to the File Agent
3. Coordinates results between them

Each agent runs independently. The Browser Agent navigates and extracts data. The File Agent creates the file. They don't interfere with each other.

## Voice control

Juno has always-on voice control via a custom Whisper plugin built in Rust:

- Say "Hey Juno" (or any custom wake word) to activate
- Transcription runs locally -- audio never leaves your machine
- Supports 99+ languages
- Partial transcription results (you see words appearing in real-time)

The voice plugin is a separate Tauri plugin (`tauri-plugin-voice-transcription`) that manages the audio pipeline independently from the main app.

## The Computer Use loop

Here's what happens when you give Juno a task:

1. **Screenshot** -- Capture the screen via ScreenCaptureKit (JPEG, quality 85)
2. **Send to Claude** -- The screenshot + task description go to the Computer Use API
3. **Parse response** -- Claude returns a tool call: `click(x, y)`, `type("text")`, `scroll(direction)`, etc.
4. **Execute action** -- Juno performs the action via macOS Accessibility APIs
5. **Verify** -- AX (Accessibility) tree verification confirms the action landed
6. **Repeat** -- Take another screenshot, send it back, continue until done

Key implementation details:

- **Action cooldown** (500ms between actions) prevents race conditions with app animations
- **AX verification** on all 14 action types catches failures immediately
- **JPEG compression** reduces API costs by ~60% vs PNG (except zoom operations, which need precision)
- **Screenshot history limiting** keeps only the 3 most recent screenshots in context
- **Prompt caching** (Anthropic's ephemeral caching) reduces cost further on long conversations

## Extending with MCP

Juno supports the Model Context Protocol for extensibility. You can add custom tool servers that give the AI new capabilities:

- Connect to your company's internal APIs
- Add tools for specific workflows
- Integrate with databases, CRMs, project management tools

Juno also ships *as* an MCP server (`juno-cua`), so other AI agents like Claude Code and Cursor can use it to control the desktop.

```bash
npx juno-cua  # Installs MCP tools for your AI coding agent
```

This creates a feedback loop: your coding agent writes code, then uses Juno to test it visually in the browser.

## Security

Giving AI control of your computer is a security-sensitive operation. Juno has:

- **5-level permission system** -- from "read-only" to "full autonomy"
- **Command whitelisting** -- only allowed shell commands can execute
- **File path validation** -- workspace boundary enforcement prevents path traversal
- **Tool approval** -- optional user confirmation before sensitive actions
- **Audit logging** -- every action is logged

I did a full security audit early on and documented 32 issues in `SECURITY_AUDIT.md`. Transparency matters when your app controls a computer.

## Lessons learned

**1. The backend must own everything.** I started with some logic in the frontend (audio capture via `getUserMedia`, WebSocket connections). Every time, it broke. Moving everything to Rust solved stability issues and gave me the CLI for free.

**2. Multi-agent is worth the complexity.** A single agent hitting context limits and losing track of tasks was the #1 user complaint. Splitting into specialists with isolated memory fixed it.

**3. Accessibility APIs are underrated.** Instead of only relying on screenshots + coordinate clicking (fragile), I use macOS Accessibility APIs to identify and target specific UI elements. This makes automation dramatically more reliable.

**4. Tauri v2 is production-ready.** The ecosystem is mature. The Rust ↔ TypeScript bridge works well. The plugin system is flexible. I'd choose it again.

**5. Demo videos are your best marketing.** The visual of AI controlling a desktop is inherently viral. Every other marketing channel pales in comparison.

## Try it

- **Desktop app:** [junebug.ai](https://junebug.ai)
- **GitHub:** [github.com/lacymorrow/juno](https://github.com/lacymorrow/juno)
- **CLI for AI agents:** `npx juno-cua`
- **Homebrew:** `brew install lacymorrow/tap/juno-cua`

macOS 14+ required. Windows and Linux are on the roadmap.

If you have questions about Computer Use, Tauri, multi-agent systems, or desktop automation, I'm happy to answer in the comments.

---

## Publishing Notes

- **Cross-post to:** Dev.to, Hashnode, personal blog (if exists)
- **Tags:** #rust #ai #tauri #anthropic (Dev.to), tauri, rust, ai-agents, anthropic (Hashnode)
- **Timing:** 1-2 days after Show HN, while momentum is building
- **Hero image:** Screenshot or GIF of Juno in action (the demo video frame)
- **Canonical URL:** Set to the primary publishing platform

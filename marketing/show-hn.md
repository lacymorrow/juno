# Show HN Draft

## Title

Show HN: Juno -- AI agent that controls your Mac desktop (Anthropic Computer Use)

## Body

Hi HN, I built Juno, a native macOS app that gives AI full control of your computer.

You describe a task in natural language (or speak it), and Juno's AI agent takes over -- it sees your screen, moves the mouse, types on the keyboard, and completes the task autonomously.

**What it actually does:**

- You say "Fill out this job application with my resume" and watch the AI navigate to the form, fill in every field, and submit
- You say "Find the cheapest flight to NYC next weekend" and it opens your browser, searches, compares prices, and reports back
- You type "Sort my Downloads folder -- images to Photos, PDFs to Documents" and it moves every file

**How it works:**

Juno uses Anthropic's Computer Use API to give Claude vision and control of the screen. Under the hood there's a multi-agent system: an orchestrator splits complex tasks into subtasks and delegates to specialist agents (Desktop, Browser, File) that can run in parallel.

**Tech stack:**

- Tauri v2 + Rust backend (not Electron -- ~15MB, native performance)
- Custom Whisper plugin for local voice transcription (99+ languages)
- macOS Accessibility APIs for precise UI element targeting
- MCP (Model Context Protocol) support for extending with custom tools
- The backend runs everything -- the frontend is just a display layer. Juno works headlessly as a CLI too.

**For other AI agents:**

I also built `juno-cua` (npm/Homebrew) which gives AI coding agents like Claude Code, Cursor, and Codex the ability to see and interact with the desktop via MCP tools. Your coding agent can now take screenshots, click buttons, and navigate apps.

**Security:**

Tiered permission system (5 levels), command whitelisting, file path validation, tool approval for sensitive actions, full audit logging. Detailed in SECURITY_AUDIT.md.

The license is FSL-1.1-MIT (source-available, converts to MIT after 2 years).

Website: https://junebug.ai
GitHub: https://github.com/lacymorrow/juno
CLI: `npx juno-cua`

Happy to answer any questions about the architecture, the Computer Use API, or building desktop apps with Tauri + Rust.

---

## Posting Notes

- **When:** Tuesday or Wednesday, 9-10am ET
- **Strategy:** Respond to EVERY comment within 30 min for first 6 hours
- **Key angles to emphasize in comments:**
  - Native macOS (not Electron) -- this resonates hard on HN
  - Rust backend, ~15MB binary
  - Voice control with local Whisper (privacy angle)
  - The AI agent integration angle (juno-cua) is unique
  - Security-first approach with audit trail
- **Avoid:** Don't oversell. HN prefers understated, technical honesty.
- **If asked about Anthropic shipping their own:** "Juno is source-available and extensible via MCP. It works with the desktop, not just a browser tab."

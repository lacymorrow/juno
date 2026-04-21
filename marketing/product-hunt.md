# Product Hunt Launch Plan

> Reference: `marketing/growth-strategy.md`
> Goal: Top 5 Product of the Day

---

## Timing

**Launch day:** 1 week after Show HN, on a Tuesday or Wednesday.
**Why:** Capitalize on HN momentum (GitHub stars, Twitter buzz) for social proof. Tuesday/Wednesday have the best PH engagement. Avoid Mondays (high competition) and Fridays (low traffic).

**Post time:** 12:01 AM PT (PH daily reset). The listing accumulates upvotes over 24 hours.

---

## Listing

### Tagline (60 chars max)
AI that controls your Mac — describe a task, watch it happen

### Description

Juno is a native macOS app that gives AI full control of your computer. Describe a task in natural language (or speak it), and Juno's AI sees your screen, moves the mouse, types, clicks, and completes the task.

**How it works:**
- Describe any task: "Fill out this form," "Research flights," "Organize my files"
- Juno's AI agent sees your screen and takes action
- Multi-agent system splits complex tasks across specialists running in parallel
- Voice control with local Whisper transcription (privacy-first, 99+ languages)

**Why Juno is different:**
- Native Mac app (Tauri + Rust, ~15MB) — not Electron, not a Docker container
- Works with ANY app on your Mac — not just browser tabs
- MCP integration — extend with custom tools, or use Juno as a tool for other AI agents
- Source-available (FSL-1.1-MIT)

**For developers:**
`npx juno-cua` gives Claude Code, Cursor, and Codex the ability to see and control the desktop.

### Topics/Categories
- Artificial Intelligence
- Mac
- Developer Tools
- Productivity
- Open Source

### Maker Comment (post immediately after launch)

Hey Product Hunt! I'm Lacy, the maker of Juno.

I built Juno because every Computer Use demo was a Docker container with a virtualized desktop. I wanted AI that controls my *actual* computer — my real apps, my real files, my real browser sessions.

The result: a native macOS app (~15MB, Tauri + Rust) where you describe a task and watch AI complete it. It uses Anthropic's Computer Use API under the hood, with a multi-agent system that splits complex tasks across specialized agents.

Some things I use it for daily:
- Filing expense reports (it fills out every field)
- Researching and comparing options across multiple tabs
- Organizing files that pile up in Downloads
- Testing UI flows in apps I'm building

The voice control is my favorite feature — say "Hey Juno" and speak your task. Transcription is local via Whisper, so audio never leaves your machine.

I also built `juno-cua` which gives AI coding agents (Claude Code, Cursor, Codex) the ability to see and control the desktop via MCP. Your coding agent can now click buttons, fill forms, and test UIs.

Happy to answer any questions about the architecture, Computer Use API, or building desktop apps with Tauri!

---

## Media Assets (Required)

### Gallery Images (5 recommended)
1. **Hero:** Screenshot of Juno's floating bar UI + a task being completed (AI moving mouse)
2. **Voice:** Screenshot showing voice control in action (waveform + transcription)
3. **Multi-agent:** Diagram or screenshot of orchestrator splitting a task
4. **CLI:** Terminal screenshot showing `npx juno-cua` in action with Claude Code
5. **Architecture:** Clean diagram of Tauri + Rust + multi-agent system

### Video (strongly recommended — PH algorithm favors video)
- 60-90 second demo video
- Show a real task from start to finish
- Include voice control demo
- End with "Try it: junebug.ai"
- No music over voice — clean screen recording with optional narration

### Logo
- Juno icon (the one from `public/juno5.png`)
- Ensure it's square, at least 240x240px

---

## Launch Day Playbook

### Pre-launch (Day before)
- [ ] Draft personal messages to 10-20 people asking for support (NOT asking for upvotes — PH rules)
- [ ] Prepare 5-8 thoughtful responses to common questions
- [ ] Queue up Twitter/X post: "We're live on Product Hunt today"
- [ ] Email waitlist: "Juno is on Product Hunt — check it out"
- [ ] Ensure junebug.ai is fast, polished, download link works

### Launch Day Schedule (PT)
- **12:01 AM** — Listing goes live automatically
- **6:00 AM** — Post maker comment
- **7:00 AM** — Tweet "We're live on Product Hunt" with link
- **7:00 AM** — Send waitlist email
- **8:00 AM - 6:00 PM** — Monitor and respond to every comment within 30 min
- **12:00 PM** — Mid-day Twitter update with current rank
- **6:00 PM** — Evening Twitter update thanking supporters
- **11:59 PM** — PH day ends, final rank determined

### Comment Response Templates

**"How does this compare to X?"**
Great question! [Specific comparison]. The key difference is Juno works with your entire desktop, not just the browser. And it's source-available so you can see exactly what it does. Happy to go deeper on any specific comparison.

**"Privacy concerns with screenshots?"**
Totally valid concern. Screenshots are sent to Anthropic's API for processing — same as using Claude directly. Voice control is fully local (Whisper runs on-device). We have a 5-level permission system so you control what the AI can access. Full security audit is public: github.com/lacymorrow/juno/blob/main/SECURITY_AUDIT.md

**"macOS only?"**
For now, yes. The Rust backend is mostly cross-platform — the macOS-specific parts are Accessibility APIs and ScreenCaptureKit. Windows and Linux are on the roadmap. In the meantime, `juno-cua` (the CLI) gives some cross-platform functionality for agent integration.

**"What about cost / API pricing?"**
Juno itself is free. You bring your own Anthropic API key. Cost depends on usage — each task involves screenshots + API calls. A typical task costs $0.05-0.50 depending on complexity. We're working on a usage estimator for the website.

**"Is this safe?"**
Security is a first-class concern. Juno has a 5-level permission system, command whitelisting, file path validation, tool approval for sensitive actions, and audit logging. The full security audit (32 documented issues and mitigations) is public in the repo.

---

## Success Metrics

| Metric | Minimum | Good | Great |
|--------|---------|------|-------|
| Upvotes | 200 | 400 | 700+ |
| Rank | Top 10 | Top 5 | #1 of the day |
| Comments | 20 | 50 | 100+ |
| junebug.ai traffic (day of) | 2,000 | 5,000 | 10,000+ |
| GitHub stars (day of) | +100 | +300 | +500+ |
| Downloads (day of) | 50 | 150 | 300+ |

---

## Post-Launch

- Write a "Lessons from our Product Hunt launch" Twitter thread
- Add "Featured on Product Hunt" badge to junebug.ai and README
- Follow up with everyone who commented — DM makers of complementary products
- If top 5: submit to Product Hunt newsletters and "best of" collections
- Repurpose PH comments into FAQ content for junebug.ai

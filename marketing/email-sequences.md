# Email Lifecycle Sequences

> Platform: Resend (already integrated in junebug.ai)
> Audience: RESEND_AUDIENCE_ID (waitlist subscribers)

---

## Sequence 1: Waitlist Welcome (Immediate)

**Trigger:** User signs up on junebug.ai waitlist
**Timing:** Immediate

### Email: Welcome to the Juno Waitlist

**Subject:** You're on the list
**Preview:** Juno — AI that controls your Mac

---

Hey {first_name},

You're on the Juno waitlist. Here's what you signed up for:

**Juno is a native Mac app where you describe a task and AI does it.** It sees your screen, moves the mouse, types, clicks, and completes tasks across any app on your Mac.

A few things while you wait:

- **Star us on GitHub** — [github.com/lacymorrow/juno](https://github.com/lacymorrow/juno)
- **Try the CLI now** — If you use Claude Code or Cursor, run `npx juno-cua` to give your AI agent desktop control today
- **Follow for updates** — [@lacymorrow on Twitter/X](https://twitter.com/lacymorrow)

I'll email you when Juno is ready for you.

— Lacy

---

## Sequence 2: Launch Announcement

**Trigger:** Manual send on launch day
**Timing:** Launch day, 7:00 AM PT

### Email: Juno is live

**Subject:** Juno is live — AI that controls your Mac
**Preview:** Download now. Describe a task. Watch it happen.

---

Hey {first_name},

Juno is live. You can download it now.

**[Download Juno](https://junebug.ai)** (macOS 14+)

Here's what you're getting:

- Describe any task in natural language → AI completes it
- Voice control: say "Hey Juno" + your task (local Whisper, audio never leaves your Mac)
- Multi-agent system: complex tasks get split across specialists running in parallel
- Works with every app on your Mac — not just the browser
- Native Tauri + Rust app, ~15MB

**Quick start:**
1. Download and open Juno
2. Grant Accessibility permissions when prompted
3. Add your Anthropic API key in Settings
4. Type or say your first task

If you're a developer, `npx juno-cua` gives Claude Code / Cursor / Codex the ability to control your desktop.

We're also [live on Product Hunt](https://producthunt.com/posts/juno) today if you want to show support.

Questions? Reply to this email — I read every one.

— Lacy

---

## Sequence 3: Onboarding Drip (Post-download)

**Trigger:** User downloads Juno (tracked via analytics event or manual segment)
**Timing:** Days 1, 3, 7 after download

### Email 1 (Day 1): Your first task

**Subject:** Try this with Juno
**Preview:** 3 tasks that work great on day one

---

Hey {first_name},

Welcome to Juno. Here are 3 tasks to try right now:

**1. Organize your Downloads folder**
> "Sort my Downloads — move images to Photos, PDFs to Documents, delete anything older than 30 days"

**2. Quick research**
> "Find the top 5 coffee shops near [your neighborhood] and compare their ratings"

**3. Fill out a form**
> Open any web form and say: "Fill this out with my info — [your name], [your email]"

**Pro tip:** Try voice control. Say "Hey Juno" and speak your task naturally. It works in 99+ languages.

If something doesn't work as expected, I want to know. Reply to this email or [open a GitHub issue](https://github.com/lacymorrow/juno/issues).

— Lacy

---

### Email 2 (Day 3): Power features

**Subject:** 3 things most people miss in Juno
**Preview:** Voice control, multi-agent tasks, and MCP tools

---

Hey {first_name},

You've had Juno for a few days. Here are features most people discover later than they should:

**1. Voice control is always listening**
Say "Hey Juno" anytime — even when the app is minimized. You can change the wake word in Settings. Audio is processed locally via Whisper and never leaves your machine.

**2. Complex tasks use multiple agents**
When you give Juno a complex task, the orchestrator splits it across specialist agents (Desktop, Browser, File) that run in parallel. Try: "Research the top 3 project management tools and put them in a spreadsheet" — the Browser Agent searches while the File Agent preps the document.

**3. MCP extends what Juno can do**
If you have MCP tool servers, Juno can use them. Connect to databases, APIs, internal tools. Settings → MCP to configure.

**For developers:** `npx juno-cua` turns Juno into an MCP server that your AI coding agent can use. Claude Code can now take screenshots, click buttons, and test UIs.

What's the most useful thing you've done with Juno so far? Reply and tell me — I feature the best ones.

— Lacy

---

### Email 3 (Day 7): Feedback ask

**Subject:** How's Juno going?
**Preview:** Quick question (takes 10 seconds)

---

Hey {first_name},

You've had Juno for a week. Quick question:

**What's the one thing you wish Juno did better?**

Just reply to this email. One sentence is fine. I read every response and it directly shapes what I build next.

If Juno has been useful, here are two things that help a lot:
- **Star the repo:** [github.com/lacymorrow/juno](https://github.com/lacymorrow/juno)
- **Tell one person** who'd find it useful

Thanks for being an early user.

— Lacy

---

## Sequence 4: Monthly Newsletter

**Trigger:** Manual send, 2x/month
**Timing:** 1st and 15th of each month, 10:00 AM ET

### Template

**Subject:** Juno Update — [headline feature or milestone]
**Preview:** [One-line summary]

---

Hey {first_name},

**What's new in Juno:**

- **[Feature 1]** — [1-2 sentence description]
- **[Feature 2]** — [1-2 sentence description]
- **[Bug fix / improvement]** — [1 sentence]

**From the community:**
- [User quote or interesting use case]
- [GitHub discussion or feature request highlight]

**What's next:**
- [Upcoming feature or focus area]
- [Any call for feedback or testing]

**Links:**
- [Latest release](https://github.com/lacymorrow/juno/releases)
- [Full changelog](https://github.com/lacymorrow/juno/blob/main/CHANGELOG.md)

— Lacy

---

## Sequence 5: Win-back (Inactive Users)

**Trigger:** User hasn't opened Juno in 30 days (requires analytics tracking)
**Timing:** Day 30 after last active session

### Email: Still there?

**Subject:** Juno got better since you left
**Preview:** Here's what changed

---

Hey {first_name},

Haven't seen you in Juno lately. Here's what's new since your last visit:

- **[Top 3 features/improvements released since they were last active]**

If something wasn't working right or you hit a friction point, I'd love to hear about it. Just reply — I'll personally look into it.

**[Re-download Juno](https://junebug.ai)** | **[What's new](https://github.com/lacymorrow/juno/releases)**

— Lacy

---

## Email Principles

1. **From "Lacy" not "Juno Team"** — personal, founder-led tone
2. **Plain text feel** — minimal formatting, no heavy HTML templates
3. **Every email has one clear action** — don't overwhelm
4. **Always allow reply** — every reply goes to Lacy's inbox
5. **No emoji in subject lines** — clean, professional
6. **Unsubscribe link** in footer (Resend handles this)
7. **Segment by behavior** — downloads get onboarding, waitlist-only gets launch announcement
8. **Frequency cap** — never more than 1 email per week to any single person

---

## Technical Implementation Notes

The junebug.ai waitlist already uses Resend with `RESEND_API_KEY` and `RESEND_AUDIENCE_ID`. To implement these sequences:

1. **Sequence 1** (welcome): Add to the Resend audience webhook or use Resend's automation feature
2. **Sequences 2-3** (launch, onboarding): Manual sends via Resend API or dashboard, segmented by tag
3. **Sequence 4** (newsletter): Manual via Resend broadcast
4. **Sequence 5** (win-back): Requires analytics integration to track active users; implement later

**Tags to add in Resend:**
- `waitlist` — signed up but hasn't downloaded
- `downloaded` — has downloaded the app
- `active` — used in last 30 days
- `inactive` — no activity in 30+ days

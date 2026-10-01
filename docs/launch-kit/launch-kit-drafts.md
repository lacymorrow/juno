# Juno Launch Kit Drafts

Status: DRAFT. Lacy approves every post individually. Herald publishes after approval.
No launch date set. Depends on step 1 (demo clip) and step 2 (download path QA pass).

Note: humanizer skill was not available in this session. Humanizer rules applied manually
from ~/.agents/skills/humanizer/SKILL.md. Herald should re-run through humanizer before publishing.

---

## 1. X Launch Thread (@junebug_ai, 5 posts)

Each post verified under 280 characters.

### Post 1 (clip + hook)

[Demo clip attached]

Juno is out. A computer use agent for your Mac, controlled by voice.

It reads your screen and drives your apps until the job is done, working in the background so your cursor stays yours.

Free, open source. junebug.ai

### Post 2 (what makes it different)

Cloud computer use agents work in a sandbox. Desktop ones take over your mouse.

Juno sends actions to apps through macOS accessibility APIs in the background. You keep your cursor and your frontmost window. The agent works alongside you.

### Post 3 (technical moat)

What makes this reliable on Mac: AX-grounded clicking.

Before every click, Juno hit-tests the macOS accessibility tree. If there is a button at that coordinate, it fires AXPress instead of a raw pixel click. 1-5ms. Falls back silently.

No Docker, no VNC. Native macOS.

### Post 4 (no API key needed)

If you have Claude Max or Pro, you already paid for this.

Juno runs through the Claude CLI. Sign in once, the agent works on your Mac at zero additional cost.

Direct API keys work too.

### Post 5 (CTA)

40k+ lines of Rust, all open source. Built on Tauri v2 with React.

github.com/lacymorrow/juno
junebug.ai

---

## 2. Show HN Post

### Title

Show HN: Juno, voice-controlled computer use for macOS (Tauri/Rust, open source)

### Lacy's first comment

I built Juno because I wanted voice computer use on my Mac without handing my desktop to a cloud VM or watching my cursor get hijacked.

It is a Tauri v2 app. The backend is about 40k lines of Rust. Everything except the chat UI lives there: audio, screenshots, networking, accessibility, agent orchestration. The frontend is React + TypeScript, and it only renders what the backend tells it.

How it works: you speak (local Whisper, nothing uploaded) or type. Juno screenshots your display, sends the image to Claude, and executes the response. Click, type, scroll, open apps. It loops until the task is done or you press Escape.

The part I am most proud of is AX-grounded clicking. Before every click, Juno calls AXUIElementCopyElementAtPosition on the macOS accessibility tree. If there is a button at that coordinate, it fires a semantic AXPress instead of a raw CGEvent click. This eliminates the 1-5px targeting errors that coordinate-based agents hit when UI reflows or animates between the screenshot and the click. The hit-test takes 1-5ms and falls back silently when no interactive element is found.

Background mode is on by default. The agent acts through accessibility APIs rather than the physical cursor, so your mouse stays yours and your frontmost window stays in front. Driving the real cursor is a separate opt-in.

Runs on your Claude Max or Pro subscription through the Claude CLI. No separate billing. Direct API keys work too.

Tech stack:
- Tauri v2 (Rust backend, Vite + React frontend)
- ScreenCaptureKit for screenshots
- whisper.cpp for local voice transcription
- macOS accessibility APIs for AX-grounded clicking and background mode
- tokio + async Rust for the concurrency layer
- Multi-agent orchestration with memory isolation between specialists

macOS 14+. Free. Source: https://github.com/lacymorrow/juno

Happy to answer questions about the accessibility API work or the architecture.

---

## 3. Product Hunt Listing

### Tagline

Voice-controlled computer use for your Mac. Your cursor stays yours.

### Description

Juno is a desktop agent that uses your Mac the way you do. You talk or type. It reads your screen, clicks buttons, types text, and keeps going until the job is done.

What sets it apart from cloud-based computer use: Juno runs natively on macOS and acts through accessibility APIs in the background. Your cursor stays under your hand and your frontmost window stays in front.

Before every click, Juno checks the macOS accessibility tree at the target coordinate. If it finds a button, it clicks the element semantically instead of guessing at pixels. This is AX-grounded clicking, and it is the reason coordinate-based targeting drift does not bite here.

Voice input runs locally through Whisper. No audio leaves your machine. If you have a Claude Max or Pro subscription, you do not need a separate API key.

Built with Tauri v2 (Rust + React). Open source.

### Gallery list (suggested screenshots/assets)

1. Hero: Juno bar + orb on the desktop with a voice command being spoken
2. AX grounding: Juno clicking a button in Figma, overlay showing the element it found
3. Voice: local transcription in progress, waveform visible
4. Background mode: user's cursor in one app while Juno works in another app
5. Settings: provider selection panel (Claude CLI login vs API key)
6. GitHub: the repo card, star count, language breakdown

### Maker comment (from Lacy)

I started building Juno because I wanted to say something to my Mac and have my apps respond. The existing computer use tools wanted me to type into a chat window or hand my desktop to a cloud VM, and that felt backward.

The hard technical problem is clicking accuracy. Every computer use agent screenshots the display, picks a coordinate, and clicks. Between the screenshot and the click, UI can reflow, animate, or shift. That pipeline drifts 1-5 pixels, and coordinate-based agents miss their target constantly.

Juno fixes this with AX-grounded clicking. Before every click, we hit-test the macOS accessibility tree at the coordinate. If there is an interactive element (button, link, text field), we fire a semantic AXPress action instead of a raw CGEvent coordinate click. The accessibility system routes that press to the right element regardless of pixel position. The hit-test takes 1-5ms, and when there is no element, we fall back to the coordinate click silently.

Your cursor stays yours. Background mode is on by default. Juno acts through accessibility APIs, not your physical mouse. You can keep working while it runs.

40k+ lines of Rust, open source. Built on Tauri v2 with whisper.cpp for voice and ScreenCaptureKit for screenshots. Free if you already have Claude Max or Pro.

github.com/lacymorrow/juno

# Juno Launch Kit Drafts

Status: DRAFT. Lacy approves every post individually. Herald publishes after approval.
No launch date set. Depends on step 1 (demo clip) and step 2 (download path QA pass).

Note: humanizer skill was not available when these drafts were written. Humanizer rules
applied manually from writing checklist. Herald should re-run through humanizer before publishing.

Hook: "answers you can press" (ops lead, 2026-10-07). OpenAI shipped Intelligent UI;
Juno's interactive response cards have been live for months. Lead every piece with this angle.
Pricing URL: junebug.ai/pricing (LAC-4126). Verify preview is live before publishing.
X thread: moved to LAC-4175 (under LAC-4173, due Oct 9).

---

## 1. X Launch Thread

Moved to LAC-4175 (child of LAC-4173, due Oct 9). Original drafts in git history (b0e36abb).
LAC-4175 owns the final thread with the "answers you can press" hook.

---

## 2. Show HN Post

### Title

Show HN: Juno, voice-controlled computer use for macOS (Tauri/Rust, open source)

### Lacy's first comment

I built Juno because I wanted to say something to my Mac and have my apps respond. OpenAI just shipped "Intelligent UI" in ChatGPT, where answers come back as interactive components you can tap. Juno has been shipping this for months. The difference: instead of charts inside a chat window, Juno streams response cards with action buttons that press real controls in your apps through macOS accessibility APIs. You say "reschedule my 3pm," a card shows the event, and the Reschedule button fires an AXPress on Calendar.

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

macOS 14+. Free. Pricing and founding presale: junebug.ai/pricing. Source: https://github.com/lacymorrow/juno

Happy to answer questions about the accessibility API work or the architecture.

---

## 3. Product Hunt Listing

### Tagline

Answers you can press. Voice-controlled computer use for Mac.

### Description

Most AI agents answer with text. Juno answers with buttons that work.

You talk or type. Juno reads your screen, streams a response card with action buttons, and presses real controls in your apps through macOS accessibility APIs. "Reschedule my 3pm" returns a card with the event and a Reschedule button that fires an AXPress on Calendar. The response is the action.

Before every click, Juno checks the macOS accessibility tree at the target coordinate. If it finds a button, it clicks the element semantically instead of guessing at pixels. This is AX-grounded clicking, and it is the reason coordinate-based targeting drift does not bite here.

Background mode is on by default. Your cursor stays under your hand. Your frontmost window stays in front. The agent works alongside you.

Voice input runs locally through Whisper. No audio leaves your machine. If you have a Claude Max or Pro subscription, you do not need a separate API key. Pricing: junebug.ai/pricing

Built with Tauri v2 (Rust + React). Open source.

### Gallery list (suggested screenshots/assets)

1. Hero: Juno streaming a response card with action buttons on the desktop
2. AX grounding: Juno clicking a button in Figma, overlay showing the element it found
3. Voice: local transcription in progress, waveform visible
4. Background mode: user's cursor in one app while Juno works in another app
5. Settings: provider selection panel (Claude CLI login vs API key)
6. GitHub: the repo card, star count, language breakdown

### Maker comment (from Lacy)

I started building Juno because I wanted answers I could press. Say something to your Mac and have your apps respond with buttons that drive real controls on your desktop.

OpenAI just showed this idea with Intelligent UI in ChatGPT. Juno has been shipping it for months. The difference is where the buttons point: theirs render inside a chat window. Ours fire AXPress actions on real macOS UI elements through the accessibility tree.

The hard technical problem is clicking accuracy. Every computer use agent screenshots the display, picks a coordinate, and clicks. Between the screenshot and the click, UI can reflow, animate, or shift. That pipeline drifts 1-5 pixels, and coordinate-based agents miss their target constantly.

Juno fixes this with AX-grounded clicking. Before every click, we hit-test the macOS accessibility tree at the coordinate. If there is an interactive element (button, link, text field), we fire a semantic AXPress action instead of a raw CGEvent coordinate click. The accessibility system routes that press to the right element regardless of pixel position. The hit-test takes 1-5ms, and when there is no element, we fall back to the coordinate click silently.

Your cursor stays yours. Background mode is on by default. Juno acts through accessibility APIs, not your physical mouse. You can keep working while it runs.

40k+ lines of Rust, open source. Built on Tauri v2 with whisper.cpp for voice and ScreenCaptureKit for screenshots. Free if you already have Claude Max or Pro. Pricing and founding presale: junebug.ai/pricing

github.com/lacymorrow/juno

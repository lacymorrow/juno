# Juno Launch Kit Drafts

Status: DRAFT. Lacy approves every post individually. Herald publishes after approval.
No launch date set. Depends on step 1 (demo clip) and step 2 (download path QA pass).

Hook: "answers you can press" (ops lead, 2026-10-07). OpenAI shipped Intelligent UI;
Juno's interactive response cards have been live for months. Lead every piece with this angle.

Pricing: junebug.ai/pricing is live as of 2026-10-21 (LAC-4176 and LAC-4174 both done).
Verified against the live page: Free tier is the full app with your own key, Pro is $12/mo
or $100/yr, founding ladder starts at $49 for the first 100 seats. Lacy chose "wait for
pricing" on 2026-10-19, so these links are now real and the copy quotes the live numbers.
Herald re-checks the seat count before publishing, since the first-100 line goes stale.

License: the repo is FSL-1.1-MIT, which is source-available and converts to MIT after two
years. Earlier drafts of this kit said "open source" in three places, including the Show HN
title. That is wrong and HN would have caught it. Every claim now says source-available.

Humanizer: run on 2026-10-21 against ~/.agents/skills/humanizer/SKILL.md. Earlier drafts
were written when that skill was unreachable, so the rules were approximated by hand.

Line count: earlier drafts said 40k lines of Rust. Measured 2026-10-21, src-tauri/src is
140,973 lines and the whole Rust workspace is 166,120. The copy now says about 140k, which
is what `find src-tauri/src -name '*.rs' | xargs wc -l` returns if a reader checks.

X thread: moved to LAC-4175 (under LAC-4173).

---

## 1. X Launch Thread

Moved to LAC-4175 (child of LAC-4173). Original drafts in git history (b0e36abb).
LAC-4175 owns the final thread with the "answers you can press" hook.

---

## 2. Show HN Post

### Title

Show HN: Juno, voice-controlled computer use for macOS (Rust, source available)

### Lacy's first comment

I built Juno because I wanted to say something to my Mac and have my apps respond. OpenAI just shipped "Intelligent UI" in ChatGPT, where answers come back as components you can tap. Juno's been doing that for months, with the buttons pointed somewhere else. Theirs render inside a chat window. Juno's cards have buttons that press the real controls in your apps, through the macOS accessibility APIs. Say "reschedule my 3pm" and you get a card with the event on it, and the Reschedule button fires an AXPress on Calendar.

It's a Tauri v2 app. The backend is about 140k lines of Rust, and everything except the chat UI lives there: audio, screenshots, networking, accessibility, agent orchestration. The React frontend only renders what the backend hands it.

You speak (local Whisper, nothing uploaded) or you type. Juno screenshots your display, sends the image to Claude, and runs what comes back. Click, type, scroll, open apps. It loops until the task is done or you hit Escape.

The part I'm proudest of is AX-grounded clicking. Before every click, Juno calls AXUIElementCopyElementAtPosition on the accessibility tree. If there's a button at that coordinate, it fires a semantic AXPress instead of a raw CGEvent click. That kills the 1-5px targeting errors every coordinate-based agent hits when the UI reflows or animates between the screenshot and the click. The hit-test costs 1-5ms, and when it finds nothing interactive there it falls back to the plain click without comment.

Background mode is on by default, so the agent works through accessibility APIs rather than your physical cursor. Your mouse stays yours and your frontmost window stays in front. Driving the real cursor is a separate opt-in.

It runs on your Claude Max or Pro subscription through the Claude CLI, so there's no second bill. Direct API keys work too.

Tech stack:
- Tauri v2 (Rust backend, Vite + React frontend)
- ScreenCaptureKit for screenshots
- whisper.cpp for local voice transcription
- macOS accessibility APIs for AX-grounded clicking and background mode
- tokio and async Rust for the concurrency layer
- Multi-agent orchestration with memory isolation between specialists

macOS 14+, and it's alpha. Free with your own API key, and that's the whole app rather than a trial. If you'd rather not manage a key, hosted usage is $12/mo, and the first 100 founding licenses are $49 once: https://junebug.ai/pricing

Source is FSL-1.1-MIT. Source-available now, MIT in two years: https://github.com/lacymorrow/juno

Happy to answer questions about the accessibility API work or the architecture.

---

## 3. Product Hunt Listing

### Tagline

Answers you can press. Voice computer use for Mac.

### Short description (for the 260-character listing field)

Juno answers with buttons that work. Talk or type, and it presses the real controls in your apps through macOS accessibility APIs. Your cursor stays yours. Voice runs on-device. Free with your own API key, or $12/mo and we handle the AI.

### Description (long form, for the launch page body and the first comment)

Most AI agents answer with text. Juno answers with buttons that work.

You talk or you type. Juno reads your screen and streams back a response card, and the buttons on that card press the real controls in your apps through the macOS accessibility APIs. Ask it to reschedule your 3pm and the card comes back with the event on it. The Reschedule button fires an AXPress on Calendar. The answer is the action.

Before every click, Juno checks the accessibility tree at the target coordinate. If there's a button there, Juno presses that element instead of guessing at pixels. That's AX-grounded clicking, and it's why the usual coordinate drift doesn't bite here.

Background mode is on by default, so your cursor stays under your hand and whatever window you're in stays in front. You keep working while Juno does.

Voice runs locally through Whisper, so no audio leaves your machine. The app is free with your own API key, and if you already pay for Claude Max or Pro you don't need a key at all. Hosted usage is $12/mo if you'd rather we handle it, and the first 100 founding licenses are $49 once: junebug.ai/pricing

Built on Tauri v2, Rust underneath and React on top. Source-available on GitHub under FSL-1.1-MIT.

### Gallery list (suggested screenshots/assets)

1. Hero: Juno streaming a response card with action buttons on the desktop
2. AX grounding: Juno clicking a button in Figma, overlay showing the element it found
3. Voice: local transcription in progress, waveform visible
4. Background mode: user's cursor in one app while Juno works in another app
5. Settings: provider selection panel (Claude CLI login vs API key)
6. GitHub: the repo card, star count, language breakdown

### Maker comment (from Lacy)

I started building Juno because I wanted answers I could press. Say something to your Mac, and your apps answer with buttons that drive real controls on your desktop.

OpenAI just showed this idea off as Intelligent UI in ChatGPT. Juno's been shipping it for months. What's different is where the buttons point. Theirs render inside a chat window. Ours fire AXPress actions on real macOS UI elements through the accessibility tree.

The hard problem is clicking accuracy. Every computer use agent screenshots the display, picks a coordinate, and clicks it. In between, the UI can reflow, animate, or shift a few pixels. Coordinate-based agents miss, constantly.

Juno hit-tests the accessibility tree at that coordinate first. If there's an interactive element sitting there, a button or a text field, it fires a semantic AXPress and the accessibility system routes the press to the right element no matter where the pixels went. 1-5ms. When there's nothing there, it falls back to the coordinate click.

Your cursor stays yours, too. Background mode is on by default, so Juno acts through accessibility APIs rather than your physical mouse, and you can keep working while it runs.

140k lines of Rust, source-available under FSL-1.1-MIT and MIT in two years. Built on Tauri v2, with whisper.cpp doing voice transcription on-device. Free with your own key, free with no key at all if you already pay for Claude Max or Pro, and the first 100 founding licenses are $49 once: junebug.ai/pricing

github.com/lacymorrow/juno

# Changelog

All notable changes to Juno are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added

- The floating bar has a React Orb appearance, built on ogl, with per-state hue, intensity, and audio reactivity (#570)
- The three.js orb, now named ElevenLabs Orb, carries matched colors and motion for every bar state (#570)
- Settings can show or hide the menu bar tray icon and the glowing bar border (#570)
- Dictation can stream provisional text while you speak, behind an advanced setting. The audio thread keeps a ten second sliding window and emits cumulative partials about every 600ms, drawn dimmed until the final decode replaces them. Partials are display only and never typed.
- A Models settings pane holds the dictation model and the assistant model in one place: Fast, Balanced, and Most accurate tier cards over Whisper tiny.en, large-v3-turbo, and large-v3, with inline download state and an arm64-gated on-device Parakeet option
- Whisper large-v3 joins the model catalog

### Changed

- The orb's perlin-noise texture is vendored into the bundle, so the orb renders offline (#570)
- A thin white separation halo replaces the black depth halo behind the bar, sized to stay inside the bar window (#570)
- Your speech to text provider persists to the central store and is honored at startup

### Fixed

- The flame border stops biting inward when melt is zero, so the bar keeps a clean outline (#570)
- The main window's voice state receives the transcript again. The voice plugin emits `{ text }` and both listeners were reading the payload as a plain string.

## [0.7.3] - 2026-09-22

### Added

- A static depth halo sits behind the floating bar and chat pane, so a black bar stays visible on a dark wallpaper (#569)

### Fixed

- Transcripts rewrite "Juneau" back to "Juno", since Whisper knows the Alaska capital and not the assistant (#569)
- The chat stays pinned to the bottom when you submit. The self-scroll guard widened from a single flag to a short time window, so a burst of layout writes no longer reads as you scrolling away. (#569)

## [0.7.2] - 2026-09-22

### Added

- Juno mutes the always-listening microphone while she speaks, so her own text to speech cannot wake her (#567)
- A wake chime plays the instant a wake phrase lands, before the request is captured (#567)
- The screen edges glow in system blue while Juno captures a spoken request (#567)
- You can quit Juno by voice with "quit", "exit", "goodbye", or "shut down". A bare "stop" or "cancel" still only disarms listening, so quitting is never a surprise. (#567)
- The bar's border lights with a flame that holds a color per state: blue for Juno listening or working, green for your dictation, red for an error (#568)
- Builds sign and notarize locally, and a build that cannot sign fails rather than producing an unsigned app (#560)
- Demo builds are built, signed, and notarized in CI from a Demo build workflow, with the DMG attached to the run and optionally pushed to the private blob store (#564)
- Juno introduces herself by voice once when onboarding ends, naming the activation key that was actually bound (#561)
- An unlinked floating-bar state harness at `/__bar-harness` exercises every bar state without the backend (#565)

### Changed

- The updater signing key is regenerated without a password. Installs on the old key will not auto-update across this change. (#563)

### Removed

- The dead second copy of the shortcut dispatcher is gone, along with the legacy store it read from (#561)

### Fixed

- The Claude CLI session resumes per conversation, so a follow-up message continues the conversation instead of starting a new agent every turn (#562)
- Whole-utterance sound annotations such as "(upbeat music)" are dropped instead of typed into the document (#562)
- Trigger conflicts check against the triggers store, so a row no longer reports a conflict with its own binding and block its own rebinding (#561)
- Mouse control and notification settings accept the argument names the frontend actually sends, which had made both toggles fail silently (#562)
- The settings window comes back after you turn the Dock icon off, which used to close the window holding the switch (#562)
- Sending a message scrolls to it (#559)
- The bar's gravity wells stay in place while the bar resizes (#566)
- The always-listening agent path runs the same one-second dedup as every other submit path, so a stray re-fire cannot stack a second agent run (#567)
- Juno waits until the bar is on screen before she speaks her greeting (#562)

## [0.7.1] - 2026-09-20

0.7.1 is about Juno working without taking the machine away from you. The agent reaches other apps through accessibility actions and events posted to the target process, so you can keep typing while she works, and she asks before she touches the physical mouse. The release also brings demo builds that carry their own key, a waveform menu bar icon, the native share sheet for chats, and about thirty fixes from the first week of hardware testing.

### Added

- The agent works in the background through accessibility actions and events posted to the target process, so you can keep typing while Juno works (#547)
- Juno asks before taking the physical mouse, and says so on screen while she has it (#547)
- You can hide Juno's Dock icon and run her from the menu bar alone (#547)
- The menu bar icon is a five-bar waveform whose state reads from its shape rather than its color, with frames for idle, armed, recording, transcribing, agent, paused, and error. The first tray menu row and the tooltip name the state in words. (#549)
- The Claude CLI provider reaches Juno's computer-use tools through the juno-cua MCP server, instead of falling back to cliclick over Bash (#496)
- Demo copies of Juno carry their own compiled-in Anthropic key, bundle under a separate identifier so they install beside a normal Juno, and tell the recipient when the demo budget ends (#550, #551)
- Your own key takes precedence over the demo key, so adding a key to a demo copy stops it spending the demo budget (#550)
- Settings shows a Demo access row naming the cohort, so a build in the wild traces back to the key that can revoke it (#550)
- Holding Fn is a push-to-talk binding (#551)
- The floating bar has a composer, and the bar grows as the text grows (#551, #553)
- You can share or export a chat through the native macOS share sheet, with Save as Markdown and Save as HTML as Juno services inside it (#545)
- An export has the same shape every time: front matter, a title taken from your question, and Asked, Spoken, Written, and Generated sections (#545)
- Copy flips to a check for a second and a half and writes through the native pasteboard, since WebKit refuses `navigator.clipboard` in a window that is never key (#545)
- Copy takes the reply as read, meaning spoken lines, prose, and the words inside generated components, and never the `<TTS>` markers or the JSX (#545)
- The chat pane header reaches Open in window and Settings, and the hover pill offers Reopen chat once you have closed a conversation (#545)
- A ring drawn around the cursor replaces pointer scaling while the agent has control (#551)
- Cmd+W closes a window. The menu had no Close item, so it was inert everywhere. (#558)
- A permissions diagnostic reports what macOS actually thinks, in five states rather than a boolean, and refuses to read Juno's own cached answer (#558)
- You can reset a single permission from the debug toggle (#558)

### Changed

- Voice is one trigger method alongside key and mouse bindings, rather than a subsystem of its own (#555)
- Each macOS permission is asked for at the point it is needed instead of all at once during setup (#550)
- One event fires per agent phase, so nothing has to infer what "agent active" means (#557)
- The X means one thing everywhere: stop what is happening and go back to rest. The composer, the voice row, wake capture, driving, and working all call the same function, and the arrow still commits, so dictation is still typed. (#555)
- A voice session has an identity. There is one microphone, so there is at most one session: every start registers one, every stop claims it, and a claim that does not match is refused rather than acted on. (#556)
- Fn is a keyboard binding rather than its own kind of trigger, and older settings migrate on read (#558)
- Wake word matching is phonetic, so "juneau" reaches "juno" without anyone spelling homophones by hand. Vowels are kept on purpose, because dropping them the way Soundex does collapses juno, june, john, and jane onto one key. (#558)
- Each window is described once, in `tauri.conf.json`, so a window Juno rebuilds is indistinguishable from one Tauri made at startup (#558)
- shiki carries a curated grammar set, which cuts the frontend bundle (#541)

### Removed

- The abandoned settings redesign left behind in the codebase is gone (#548)
- The keyboard shortcuts pane is retired. Escape and Cmd+Comma keep their behavior as constants, normalized on read, so an older build's custom value cannot outlive the decision. (#555)
- The global voice activation shortcut is deleted outright, so its combination is claimable again (#555)
- The silent save to Downloads is gone, replaced by the share sheet's save panel (#545)
- The second text to speech implementation is deleted. Rust plays audio through afplay. (#558)

### Fixed

- `<TTS>` blocks are spoken from every provider; previously only the Anthropic API path spoke them (#544)
- A dictation transcript is typed at the cursor instead of being handed to the agent (#553)
- Stored voice triggers re-register at startup, so a saved wake phrase still listens after a restart (#555)
- A wake phrase survives the speech to text model loading late. A ready watcher re-applies the triggers, and it is armed before the first attempt so the engine cannot become ready in the gap. (#555)
- Cancel cancels. Both cancel paths called `stop_dictation`, which finalises and types what you had just abandoned. (#555)
- Releasing a held key no longer resurrects a cancelled session and submits it (#555)
- A coordinated stop re-arms the wake phrase from the stored triggers instead of revoking it until the next launch (#555)
- A trigger with no binding can never fire, so it no longer persists claiming to be on (#555)
- A trigger switches back on after you record a key, instead of staying off forever and reading as unbindable (#558)
- The tray icon is re-asserted as a template image after every icon change, so it stops rendering flat black on a dark menu bar (#553)
- Adding a trigger that already exists reports the collision instead of dropping the row and returning success (#553)
- Onboarding auto-grant registers the right row and checks real Input Monitoring state (#543)
- The bar's snap wells keep their place across displays, and a wide screen offers five stops (#546)
- The bar opens where you left it. Rust restores the stored well while the window is still hidden, instead of letting macOS cascade the window to mid-screen. (#558)
- The composer opens one row tall instead of several (#558)
- The pill stays where it is on hover. Well anchoring pinned a screen edge while the pill is centred in its window, so every hover teleported it 38px sideways and every collapse teleported it back. (#558)
- A quickly spoken "juno" is heard. Detection needed a full second of continuous speech, and the word takes about 450ms. (#558)
- The settings window keeps its transparency, shadow, and title bar style after a rebuild, instead of coming back opaque (#558)
- A typed agent run lights the tray. The single start site announced nothing, and every typed run goes through it. (#556)
- A spoken media command no longer animates the tray for an agent that was never asked to run (#556)
- A transcript with no owner is dropped with a warning instead of going to whichever session happened to be current when the text landed (#556)
- A transcription error stops recovering by committing whatever the failing engine managed to hear (#556)
- The composer mic targets the agent and says so. It was labelled "switch to dictation" while dictation types into whatever has focus, which is useless from inside Juno's own composer. (#558)
- The build script looks in the workspace target directory. The guard meant to stop an Anthropic key shipping inside a public build had been reporting success without reading a byte of the binary, and the artifact naming never ran. (#554)
- A build no longer reports success when tauri really failed, or failure when tauri exited non-zero after writing both bundles (#554)

## [0.7.0] - 2026-09-12

0.7.0 is the release where the floating bar became Juno's whole interface. The bar hosts the chat, shrinks to a pill when idle, snaps into magnetic wells, and follows your cursor across displays. Around it sit a unified trigger model that replaced fixed shortcuts, a Settings window rebuilt to match macOS System Settings, a native setup assistant, parallel agent sessions, conversation history that survives a restart, and local engines for both speech to text and text to speech. A security audit ran against the whole app and closed before the tag.

### Added

- The floating bar hosts the chat pane: a pill until you ask something, then the response streams in below it (#508)
- The idle bar is a small pill that grows on hover into mic and type buttons, and drags from anywhere (#510)
- The bar drags on the first click, even when Juno is not the active app (#509)
- The bar drags freely and snaps into magnetic wells on release, with a drop-well indicator that dims the screen and cuts clear holes where the bar can land, and a dock-aware pane (#513, #517, #523)
- A fresh launch parks the bar in the top-right well, and a saved position re-snaps to the nearest current well, so a remembered spot survives a resolution or monitor change (#523)
- The bar follows your cursor across displays (#528)
- You can reopen the chat pane from the tray, close it with a global Escape, and keep the history across opens (#526)
- The response view scrolls to the bottom as new messages stream in (#478)
- A unified trigger model replaces fixed shortcuts, pairing a method (push-to-talk, toggle, voice) with a target (agent or dictation) (#520, #521)
- A voice trigger listens for a wake phrase with an optional "hey" prefix, so "juno" reaches the agent and "transcribe" reaches dictation (#520)
- A global voice activation shortcut, Option+Shift+V, starts recording from anywhere without Juno focused (LAC-1420)
- Conversation history persists across launches, with a browser and reload (#527)
- The Settings window matches macOS System Settings, with sidebar vibrancy, inset traffic lights, row-level search that deep-links to the matching row, and an advanced toggle that hides the rest by default (#512, #505, #517)
- Onboarding is a native setup assistant with permission auto-grant, live shortcut demos, hand-drawn macOS glyphs, and a system palette (#529, #531)
- Onboarding polls macOS permissions once a second and advances the moment one is granted, and the state stays live on every step rather than only the permissions step (LAC-1862, LAC-1880)
- A permission dialog is asked for once per launch. Later clicks open System Settings instead of asking again. (#458)
- Several agent sessions run at once, with a session registry, an input arbiter, per-session cursors in an eight-slot identity palette, a roster strip, and a session switcher (LAC-1432)
- A background session posts a macOS notification when it finishes or fails, and Escape cancels only the focused session (LAC-1432)
- Automations run on a schedule, gated behind a high-risk confirmation because they re-execute unattended with full tool access (#476)
- Skill names complete as you type, fuzzy matched, in the bar and the main chat input (#480, #481)
- Agent memory persists across sessions (LAC-1429)
- Juno asks for human confirmation before a risky action (LAC-1427)
- Companion mode observes without acting, with a hard tool boundary (LAC-1411)
- Clicks go through the macOS accessibility tree by default and fall back to coordinates, and events post straight to the target process (LAC-1484, LAC-1487)
- A full-screen cursor overlay draws a Juno cursor sprite with per-state animations (#435)
- Juno highlights the element she is about to act on before a computer-use action (#475)
- Juno can point at something while she talks. A `[POINT:x,y:label:screenN]` tag in her reply flies a labelled cursor there on a bezier arc, and the tag is stripped from the text you read. (LAC-1418)
- Speech to text sits behind a provider interface, with Parakeet as an engine (#448)
- Kokoro-82M, Chatterbox, and Supertonic are local text to speech providers, and Whisper v3-turbo becomes the transcription default (LAC-1508, LAC-2050, LAC-1503)
- A live audio waveform draws while you record (#422)
- Agent replies render live React components, starting with a NowPlayingCard that reads real Spotify and Apple Music state and drives the player, plus a collapsed `<Why>` rationale block (#499, #503, #506)
- Playback commands answer locally. "Pause Spotify", "skip this song", and "what's playing?" run one AppleScript call inside `submit_query` rather than a model round trip. (#503)
- Claude Opus 5 becomes the default Anthropic model, then Fable 5.1, alongside Claude Sonnet 5 and Opus 4.8 (#492, #498, #472)
- Thinking models get an adaptive thinking summary on 4.6 and newer, and a server-side fallback so a safety-classifier refusal is retried instead of surfacing as an error (#498)
- TLS sessions warm before Anthropic API calls, which cuts first-request latency (#428)
- Onboarding asks about your role and work type, and offers a Claude CLI connect card as the primary way in (LAC-1428, LAC-1953)
- `browser_extract_content` takes a `property` option (LAC-3055)
- The macOS permissions check is cached for five seconds (#440)

### Changed

- chromiumoxide over the Chrome DevTools Protocol replaces the abandoned `playwright` crate
- Every query entry point routes through one submission pipeline (#497)
- Orchestrated queries register in the parallel-session registry, so the roster shows what a specialist agent is doing (#490)
- Escape is watched with a passive NSEvent monitor instead of being claimed as an exclusive hotkey, so other apps still receive it (#507)
- The Anthropic default `max_tokens` rises from 4096 to 16384, since component-rich replies were truncating mid-card (#498)
- The cloud connector is off by default. The hosted backend is down, and the app reconnect-looped at every launch. (#498)
- Sound cues play on the key edge rather than after audio setup or after Whisper finalizes, and resolved sound paths are cached, so starting and stopping dictation feels immediate (#519)
- Releases are Apple Silicon only
- Settings state is shared through one React context instead of each mount making its own IPC round trip
- The Rust toolchain is pinned, and rustfmt and clippy are gated in CI
- The repository is scanned with CodeQL on every push (#437)
- Verbose init logging drops from info to debug (#444)

### Removed

- The chat-based onboarding system the setup assistant replaced is gone (#533)
- Retired Claude models are gone from the provider definitions (#493)
- 14MB of unreferenced marketing PNGs leave the bundle (#530)

### Fixed

- Escape cancels the Claude CLI subprocess, now that the session cancellation token reaches the provider (#495)
- Multi-step tasks work again on thinking models. Thinking and redacted_thinking blocks are captured and replayed verbatim on the assistant turn that issued the tool calls, which the API rejects when they are dropped. (#498)
- Finishing a browser session no longer closes your browser. Cleanup and `Drop` called `browser.close()` over an attach, which terminates every Chrome window and tab you had open.
- A dropped clone of the browser controller no longer tears down the browser that every surviving clone is still using, and only pages Juno opened are closed
- MCP servers that fail to start are removed instead of left disabled, and that decision persists through the race that used to lose it (#413, #414, #409)
- ScreenCaptureKit captures exclude Juno's own windows (#418)
- Screenshots are sized to the model in use across the whole capture pipeline (#425)
- Onboarding state stops flapping when the frontend mounts, and chat input unblocks on complete or skip (#441, LAC-2072)
- The system cursor is restored on quit, and the reference counting that left it scaled is fixed (LAC-1078)
- The floating bar toggles from the menu, now that the event reaches the window manager (#491)
- The settings sidebar keeps a fixed width across panes (#469)
- Shortcut chips stop overlapping their descriptions (#504)
- Any JSX component tag in a reply is detected, instead of matching an allowlist that had been stripping NowPlayingCard (#501)
- NowPlayingCard shows Spotify artwork, now that the webview CSP allows the image host, and falls back to a music icon rather than an empty box (#502)
- The compact and hover bar sizes apply in one `setFrame`, so the pill no longer snaps sideways for a frame during the transition (#525)
- The status dot stops jumping when the pill grows on hover, and the window keeps one height and anchor across both idle layouts (#511)
- The bar leaves its listening state the moment you stop from the keyboard, and shows the transcribing state while speech to text finalizes (#524)
- Per-button hover works while Juno is unfocused (#524)
- Sound cues stop blocking the async runtime for the length of the clip (#518)
- Adding a trigger no longer flashes back to the previous list. A trigger with no binding yet persists as unconfigured. (#522)
- A toggle agent trigger stops and submits on the second tap (#522)
- Overlays stay visible above full-screen apps and never pull keyboard focus from the app you are using (LAC-1413)
- The `tool_choice` and screenshot token settings persist, and the bar reads real text to speech state instead of fabricated voice fields (#538)
- Juno's Info.plist merges correctly. The bundle had been shipping with no CFBundleIdentifier, which reset your permission grants on every update. (#532)
- The onboarding window stops flashing on launch (#532)
- An ActionButton command that does not exist routes through the agent instead of failing (LAC-2461)

### Security

- Cloud auth fails closed, spawning an MCP server needs your approval, and the entitlements are cleaned up (#534)
- A shell injection path in the agent is closed, and the file-write workspace boundary is enforced (#535)
- The browser runs behind sandbox flags, selector injection is closed, URL schemes are restricted, and the Gemini key no longer rides in the header it was leaking through (#537)
- Safari JavaScript execution goes through the approval flow (#539, #540)
- Strings truncate on character boundaries, and the panicking `expect` calls are gone from the Rust backend (#536)
- vite, react-router, and @babel/core are upgraded past their open advisories (#487)

## [0.6.0] - 2026-05-15

0.6.0 gives the agent a way to watch without touching anything, and teaches it to look before it acts. Companion mode is enforced at the tool boundary, window titles reach the agent's system context, and the system cursor grows while the agent has control. Releases move onto ShipEx.

### Added

- Companion mode watches without touching anything, enforced at the tool boundary
- A `list_visible_windows` tool reports open windows, and window titles reach the agent's system context (LAC-1199, LAC-1198)
- The system prompt tells the agent to observe before acting (#407)
- The system cursor grows while the agent has control

### Changed

- The screenshot is captured in parallel with speech-to-text finalization when you release push-to-talk (#424)
- ShipEx replaces the custom release script

### Removed

- macOS window vibrancy and the remote-control GitHub OAuth flow from 0.5.3 are reverted

### Fixed

- MCP servers that were configured by default but never existed are dropped (#409)

## [0.5.3] - 2026-05-11

### Added

- In-app updates ship through tauri-plugin-updater and a GitHub Actions release pipeline
- macOS windows gain vibrancy, and a GitHub OAuth flow covers remote control (#389, #390)

## [0.5.1] - 2026-04-30

### Added

- The agent runs through the Claude CLI as a provider, so a Claude subscription works without an API key (#398)
- Every click hit-tests the macOS accessibility tree and presses the element semantically when one is found, falling back to a coordinate click otherwise (#402)
- The project is licensed under FSL-1.1-MIT

### Fixed

- Microphone permission is asked for through one code path, so macOS stops showing the dialog repeatedly (#401)
- A running agent cancels reliably, the text-to-speech process is tracked by PID, and the browser is cleaned up on exit (#401)

## [0.4.13] - 2026-03-27

### Added

- One script cuts the Tauri, juno-cua, npm, and Homebrew releases

### Fixed

- A build script links the Swift runtime path for juno-cua
- Transcription markers filter through one shared `filter_transcription_text()` rather than several copies

## [0.4.11] - 2026-03-25

### Added

- juno-cua is a headless CLI that exposes computer-use tools as subcommands with JSON output: screenshot, click, type, key press, scroll, app and URL opening, accessibility tree reads, element search, and clipboard access (#387)
- Onboarding asks for an API key as a step (#386)

### Changed

- The floating bar and capabilities screens are interactive onboarding steps (#392)

### Removed

- Stale files are gone: `actions.rs`, the websocket test harness, and unused configs (#391)

### Fixed

- screencapturekit moves to 1.5.4 so x86_64 cross-compilation succeeds (#393)

## [0.4.9] - 2026-02-16

### Fixed

- Escape stops the agent reliably, where it had been blocking other apps without ending the run

## [0.4.7] - 2026-02-12

### Added

- Juno runs headlessly, with a test harness and CLI stubs
- Agent responses render through tri-modal UI components
- The bar appearance switches from General settings (#374)
- MCP settings open the MCP config directory, and diagnostics report a broken configuration
- The permissions screen sits on a shared permissions service (#383)

### Fixed

- Roughly 40 Rust backend bugs are fixed and the clippy warnings cleared across the app, the OS-level MCP server, and the voice transcription plugin
- Roughly 68 frontend TypeScript bugs are fixed across six audit passes
- Tool calls that arrive with an `end_turn` stop reason execute instead of being printed into the chat
- Computer-use tasks force single-agent mode, and `submit_query` runs the agent directly again

### Security

- A Content Security Policy is enabled in `tauri.conf.json`

## [0.4.1.2] - 2025-06-29

### Fixed

- Text-to-speech requests run asynchronously, closing a race that dropped or doubled speech
- Base64 decoding of text-to-speech audio is fixed

## [0.4.1.1] - 2025-06-29

### Changed

- Screenshots scale to the standard resolutions the Anthropic Computer Use API expects

## [0.4.1] - 2025-06-28

### Changed

- Configuration and error message templates live in one centralized module, replacing hardcoded strings across providers, settings, and the Anthropic path

### Fixed

- Agent memory trims aggressively, so a long run stops overflowing the model's context window

## [0.4.0] - 2025-06-24

### Added

- Debug command handlers and shared frontend modules cover formatting, validation, and development helpers

### Changed

- The agent runner loop is rewritten, and the mouse, filesystem, text editor, element, window, and shell command handlers validate their arguments

## [0.3.8] - 2025-06-23

### Added

- A desktop cursor overlay shows where the agent is pointing
- The developer tools gain a self-improvement panel and a click QA test panel

### Fixed

- Concurrent text-to-speech requests stop, which had been burning tokens on speech nobody heard

## [0.3.2] - 2025-06-21

### Added

- A UI token selector analyzes the screenshot and drops what the model does not need, which cuts screenshot tokens
- Settings persist through a settings manager with a typed store
- Escape and stop requests coordinate through dedicated coordinators, so one press ends everything that is running
- Text-to-speech shows what Juno is saying, with metadata, while it plays
- Dictation and always-on listening share one Whisper instance

## [0.3.0] - 2025-06-19

### Added

- The voice AI bar arrives, with a live audio visualizer
- macOS permissions are checked natively rather than inferred
- Linux and Windows have platform setup paths alongside macOS

### Fixed

- Microphone permission is detected without triggering repeated macOS authorization prompts

## [0.2.8] - 2025-06-18

### Added

- A menu bar tray menu and an application menu
- Dictation state lives in the backend, in a dedicated state manager
- Frontend constants generate from the Rust constants modules, so event names, timeouts, ports, and UI values have one source

### Changed

- Platform-specific code sits behind a platform module, with a shared error handling layer

## [0.2.4] - 2025-06-12

### Added

- The always-on-top floating bar and a transparent floating panel
- An onboarding window
- Settings screens for shortcuts, notifications, tools, and network
- A shortcut recorder for binding global keys
- A key press overlay and a command overlay while the agent types
- Juno launches at login
- A running agent stops from the tray or a shortcut

## [0.1.1] - 2025-06-09

### Added

- The Settings window, with sections for the AI provider, voice, and general behavior
- An onboarding flow
- JSX components render inside agent messages
- Agent execution progress shows while a task runs
- An empty chat suggests example prompts
- Agent memory stores through dedicated commands
- The agent recovers from errors instead of ending the run

## [0.1.0] - 2025-06-08

### Added

- Juno is a native macOS app built on Tauri v2 that gives Claude control of the desktop through Anthropic's Computer Use
- Work is orchestrated across desktop, browser, and system agents
- Backend tools control the mouse, keyboard, screen, filesystem, and shell
- A dedicated browser controller drives a browser
- Model Context Protocol servers extend the agent
- You can talk to Juno with local Whisper transcription, always-on listening, and dictation
- Anthropic, OpenAI, and Gemini are available as providers
- Juno runs from a CLI entry point without the UI

[Unreleased]: https://github.com/lacymorrow/juno/compare/v0.7.3...HEAD
[0.7.3]: https://github.com/lacymorrow/juno/compare/v0.7.2...v0.7.3
[0.7.2]: https://github.com/lacymorrow/juno/compare/v0.7.1...v0.7.2
[0.7.1]: https://github.com/lacymorrow/juno/compare/v0.7.0...v0.7.1
[0.7.0]: https://github.com/lacymorrow/juno/compare/v0.6.0...v0.7.0
[0.6.0]: https://github.com/lacymorrow/juno/compare/v0.5.3...v0.6.0
[0.5.3]: https://github.com/lacymorrow/juno/compare/v0.5.1...v0.5.3
[0.5.1]: https://github.com/lacymorrow/juno/compare/v0.4.13...v0.5.1
[0.4.13]: https://github.com/lacymorrow/juno/compare/v0.4.11...v0.4.13
[0.4.11]: https://github.com/lacymorrow/juno/compare/v0.4.9...v0.4.11
[0.4.9]: https://github.com/lacymorrow/juno/compare/v0.4.7...v0.4.9
[0.4.7]: https://github.com/lacymorrow/juno/compare/v0.4.1.2...v0.4.7
[0.4.1.2]: https://github.com/lacymorrow/juno/compare/v0.4.1.1...v0.4.1.2
[0.4.1.1]: https://github.com/lacymorrow/juno/compare/v0.4.1...v0.4.1.1
[0.4.1]: https://github.com/lacymorrow/juno/compare/v0.4.0...v0.4.1
[0.4.0]: https://github.com/lacymorrow/juno/compare/v0.3.8...v0.4.0
[0.3.8]: https://github.com/lacymorrow/juno/compare/0.3.2...v0.3.8
[0.3.2]: https://github.com/lacymorrow/juno/compare/0.3.0...0.3.2
[0.3.0]: https://github.com/lacymorrow/juno/compare/0.2.8...0.3.0
[0.2.8]: https://github.com/lacymorrow/juno/compare/0.2.4...0.2.8
[0.2.4]: https://github.com/lacymorrow/juno/compare/0.1.1...0.2.4
[0.1.1]: https://github.com/lacymorrow/juno/compare/0.1.0...0.1.1
[0.1.0]: https://github.com/lacymorrow/juno/releases/tag/0.1.0

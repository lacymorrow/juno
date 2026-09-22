# Headless Juno: the juno-cua CLI plan

**Goal:** run Juno almost completely without the window, through one binary, so people can build a Raycast extension, a different UI, or a shell script on top of it.

**Date:** 2026-09-22
**Status:** final plan, not started. Lives on its own branch (`headless-cli`) when it starts. Tracked in LAC-3994.
**Source:** FluidVoice teardown (`docs/audits/fluidvoice-teardown-2026-09-21.md`, sections 9 and 10) and the 2026-09-22 CLI audit below.
**Supersedes:** the CLI parts of `docs/plans/external-agent-integration.md`, which counted declared subcommands as working. Most voice ones are stubs.

---

## Decisions

1. **CLI is the product. MCP is a wrapper.** `juno-cua` already has `list-tools` and a generic `call`; `serve-mcp` reads the same table. Every new capability is added once, as a command in that table, and MCP gets it for free. No MCP-only features, ever. Lacy's instinct is right: for this use case MCP adds a protocol without adding a capability. The wrapper stays because the Claude CLI provider needs it (LAC-3696).
2. **One binary.** Everything below goes into `crates/juno-cua`, not the Tauri app's `juno` CLI mode. The Tauri CLI stays for `juno query` (the agent stack is too big to move) and is otherwise left alone.
3. **The running app is the server. The daemon is the app without a window, for developers only.** A small `juno-control` library implements a unix-socket server (NDJSON request/response plus a `subscribe` verb for events). The desktop app hosts it whenever it is running. `juno-cua daemon` hosts the same server headless, and refuses to start when the app is running ("Juno is running, commands will use it"). Every CLI command resolves in this order: app socket, daemon socket, one-shot in-process. A normal user never runs the daemon; it exists so someone building on the CLI on a machine without the app, or in CI, gets the warm engine, the mic, the triggers, and the event bus. This also settles hotkey ownership: whichever process hosts the server owns the triggers, and there is never more than one.
4. **Voice engines move to a Tauri-free crate.** `tauri-plugin-voice-transcription/src/{engine,engine_whisper,engine_parakeet,engine_manager,config,wake_word,mic_permissions,shared_whisper}.rs` have zero Tauri references today. `controller.rs` and `always_listening.rs` touch Tauri only to emit events, and `utils.rs` only to resolve resource paths. Extract `crates/juno-voice` with an `EventSink` trait; the Tauri plugin and the CLI both implement it. This is the enabler for everything in tier 1 and it is small.
5. **Output contract from day one.** Every command: JSON on stdout, human text on stderr, exit 0/1/2 (ok / failed / bad args), `--format json|pretty|quiet` as today. Long-running commands stream NDJSON lines with a `type` field. This is what a Raycast extension consumes.
6. **Settings are files.** `app_settings.json`, `conversation-<id>.json`, `conversations-index.json` are Tauri store JSON in the app data dir. The CLI reads and writes them directly. The app picks up changes on next launch; a file watcher in the app is a later item, not a blocker.

---

## Ranked plan

Ranking is usefulness divided by effort. Each tier ships whole before the next starts. Effort is a rough size: S under a day, M a few days, L a week or more.

### Tier 0: enablers (do first, nothing user-visible)

| # | Item | Effort | Why first |
|---|---|---|---|
| 0.1 | Extract `crates/juno-voice` (engines, config, model paths, wake word, mic permission check) with an `EventSink` trait replacing `AppHandle::emit`. Tauri plugin becomes a thin adapter. | M | Unblocks every voice command. Engines are already Tauri-free. |
| 0.2 | Move the Whisper model catalog and downloader out of `src-tauri/src/commands/whisper_model.rs` into `juno-voice`. Add the Parakeet CTC files to the catalog. | S | `models` and `transcribe` need it. Also the first step to making Parakeet reachable at all. |
| 0.3 | Output contract: NDJSON event type, error type, exit codes, documented in `crates/juno-cua/README.md`. | S | Everything after this depends on a stable shape. |
| 0.4 | Command table becomes the single registry: name, args schema, description, handler. `list-tools`, `capabilities`, `call`, and `serve-mcp` all read it. | S | Mostly already true; make it explicit so new commands cannot skip MCP. |

### Tier 1: low-hanging fruit (high use, small effort, no daemon needed)

| # | Command | Effort | Notes |
|---|---|---|---|
| 1.1 | `transcribe <file|->` | S after 0.1 | wav/m4a/mp3 via `hound` + decode, or 16 kHz PCM on stdin. Output: text, confidence, duration_ms, engine, model. The FluidVoice route worth matching first. |
| 1.2 | `models list|download <id>|use <id>|warm` | S after 0.2 | Sizes, paths, installed flag. `warm` runs the silent decode and prints ms. |
| 1.3 | `insert-text <text|->` with `--mode clipboard-free|paste` and `--copy` | S after LAC-3993 | Same code as the dictation insert path. The single most useful command for a Raycast extension. |
| 1.4 | `frontmost-app`, `selected-text`, `text-before-cursor` | S | All three already exist in `src-tauri/src/utils` for the agent; expose them. `text-before-cursor` is also what the cleanup layer needs. |
| 1.5 | `windows list|focus|move|resize` and `drag` | S | Gaps in the computer-use set. AX and CGEvent code exists in `mcp-server-os-level`. |
| 1.6 | `wait-for --selector <ax> [--timeout]` | S | Polls `find-elements`. Turns flaky scripts into reliable ones. |
| 1.7 | `settings get <key>|set <key> <value>|show|export|import|reset` | S | Direct store file access, dotted keys, validated against the Rust schema by deserializing `AppSettings`. |
| 1.8 | `conversations list|show <id>|export <id>|delete <id>` | S | Read the store files. Read-only first; delete is the only write. |
| 1.9 | `permissions status|request <accessibility|microphone|screen|input>` | S | Status exists in the app; request is one API call each. Machine-readable, no prose. |
| 1.10 | `speak <text> [--voice]` | S | System `say` first. Other TTS providers later, they need API keys and the app's settings. |
| 1.11 | `screenshot --window <id>|--region x,y,w,h` | S | Extends the existing screenshot. |

Definition of done for tier 1: a Raycast extension can transcribe an audio file, read the selected text, insert text into the focused app, and change one setting, using only `juno-cua`, with a recorded demo.

### Tier 2: the control socket, hosted by the app (the big lift)

| # | Item | Effort | Notes |
|---|---|---|---|
| 2.1 | `crates/juno-control`: unix-socket server at `~/Library/Application Support/Juno/control.sock`, NDJSON request/response, one connection per command, a `subscribe` verb for events. Socket file mode 0600. The desktop app hosts it at launch. | M | The app already has the warm engine, the mic, the triggers and the events. This exposes them; it does not move them. |
| 2.2 | Every CLI command resolves app socket, then daemon socket, then one-shot in-process, and says on stderr which it used. | S | Users never think about it. |
| 2.2b | `daemon start|stop|status|restart` in `juno-cua`: the same server, headless, developer-only. Refuses to start while the app is running. `--foreground` for debugging. Holds the warm engine, prepared mic (pre-roll and readiness gate from the latency plan), trigger state, event bus. | M | Only for building on the CLI without the app, or CI. Not installed by default, not mentioned in onboarding. |
| 2.3 | `record [--seconds N|--until-silence] [-o file]` | S after 2.1 | Mic to file or stdout PCM. |
| 2.4 | `listen [--until-silence|--seconds N]` | S after 2.1 | Mic to text in one shot. Streams partials as NDJSON when the engine supports it. |
| 2.5 | `dictate start|stop|cancel|status` | M after 2.1 | The full hotkey path without the hotkey: record, transcribe, clean up if configured, insert. `status` reports the same states the bar shows. |
| 2.6 | `events tail [--type ...]` | S after 2.1 | Streams bar-state, partial and final transcripts, audio level, agent stream events. This is how someone builds their own bar. |
| 2.7 | `triggers list|add|remove|enable|disable|press <id>` | M after 2.1 | Whichever process hosts the control socket owns the triggers (`triggers/mod.rs`), so there is never a conflict. `press` fires an edge for scripting and tests. |
| 2.8 | `wake start|stop|status|set-phrases|set-sensitivity` | S after 2.7 | Always-listening loop from `juno-voice`. |
| 2.9 | launchd plist install/uninstall via `daemon install`. | S | Developer convenience only. Never installed by the app. |

Definition of done for tier 2: with the app running, `juno-cua dictate start` records through the app's warm engine and `juno-cua events tail` shows every bar state. Then quit the app, start the daemon, and the same two commands work unchanged. Recorded.

### Tier 3: gated on other Juno work (build inside the shared crate so the CLI is free)

| # | Command | Blocked on |
|---|---|---|
| 3.1 | `cleanup <text|->` with `--rules-only` | Cleanup layer (teardown list D) |
| 3.2 | `dictionary list|add|remove|import|export` | Custom dictionary (D3) |
| 3.3 | `history list|show|search|export|clear` | Dictation history (E1) |
| 3.4 | `tts` with the app's providers (Kokoro, ElevenLabs, ...) | Provider config access from the shared crate |

### Tier 4: app-state surfaces, low urgency

`tools list|enable|disable|approval`, `skills list|run`, `automations list|create|delete|run`, `mcp list|remove|test`, `notifications test`, `sounds play`, `onboarding reset|status`. All are store-file reads and writes or one Tauri command each. Do them when someone asks.

---

## Example integrations

These are the proof that the CLI is a platform, and the source of the demo recordings. Each is a separate small repo or folder under `examples/`, built only on `juno-cua`, never on internal crates. Build them in this order; each one is the acceptance test for the tier it depends on.

| # | Integration | Uses | Depends on | Effort |
|---|---|---|---|---|
| E1 | **Shell recipes** in `examples/shell/`: voice note to a dated Markdown file, transcribe every new file in a watched folder, "read the selected text aloud", insert a canned snippet into the focused app. | `transcribe`, `insert-text`, `selected-text`, `speak` | Tier 1 | S |
| E2 | **Raycast extension** (`juno-raycast`, published to the Raycast store): commands for Transcribe File (file picker, result copied and shown), Dictate (starts through the app socket, shows partials, inserts on stop), Insert Snippet, Toggle Setting, Recent Transcriptions once history exists. | `transcribe`, `dictate`, `events tail`, `insert-text`, `settings`, `history` | Tier 1 for the first three, tier 2 for Dictate | M |
| E3 | **Alternative bar** (`examples/mini-bar/`, a SwiftUI or Tauri window under 300 lines): subscribes to `events tail` and renders listening, transcribing, refining, done, with the live audio level. Proves "build your own UI". | `events tail`, `dictate` | Tier 2 | S |
| E4 | **Claude Code / OpenClaw skill update**: extend the existing `juno` skill (`~/.claude/skills/juno`) so agents know the voice, text-context and settings commands, with the JSON shapes. | everything in tier 1 | Tier 1 | S |
| E5 | **Stream Deck or Hammerspoon binding**: one button that runs `dictate start` on press and `dictate stop` on release, with the button state driven by `events tail`. | `dictate`, `events tail`, `triggers press` | Tier 2 | S |

Cut from examples: a VS Code extension (Raycast covers the same audience with less surface), an Obsidian plugin (a shell recipe writes the note), and a web UI (the socket is local-only by design).

---

## Cut

- Running the daemon while the desktop app is running. The app hosts the socket; the daemon is for developers without the app.
- Moving `juno query` and the agent stack into juno-cua. Too big, no user asked for it, `juno query` already works headless.
- Cloud connector commands. Cloud is being reconsidered separately.
- Performance monitoring command. The benchmark lines from the latency plan cover it.
- An HTTP server like FluidVoice's. The socket plus the CLI is the same capability with auth for free (file permissions).
- MCP registry listing.

---

## Demo test

- Ten seconds: `juno-cua transcribe meeting.m4a` prints text. `juno-cua insert-text "hello"` types it where the cursor is.
- Removed: see Cut. Every command has one job and a JSON shape.
- One primary action per command; no interactive prompts anywhere.
- Defaults: no daemon needed for tier 1. With the app running, every command uses the app's socket without configuration. The daemon is a developer tool and is never needed alongside the app.
- Error states: no engine installed (says which command installs it), no permission (says which one and how), daemon not running (falls back silently, says so on stderr).
- Instant: computer-use commands unchanged. Voice commands warm through the daemon.
- Seams: the CLI must use the same state names as the bar and the settings UI. One vocabulary.
- Evidence: a recording of the shell recipes and the Raycast extension for tier 1, and for tier 2 the mini-bar rendering live states from `events tail`, once with the app running and once against the daemon.
- DRI: Lacy, until the LAC issue assigns it.

# FluidVoice Teardown

**Date:** 2026-09-21
**Author:** Claude Code session, requested by Lacy
**Subject:** FluidVoice 1.6.10-beta.2 (altic-dev/FluidVoice, commit c391411f, 2026-09-20), ~93k lines of Swift
**Question:** what should Juno borrow, especially the local cleanup stage, without making dictation slower

**License note.** FluidVoice is GPLv3 as of 2026-02-23. Juno is FSL 1.1. Nothing below may be copied as code. Every item is an idea to re-implement in Rust from the description. The on-device model ("Fluid Intelligence", `fluid-1` pico/mini) is closed and not in the repo at all; the open-source build ships a pass-through shim.

---

## 1. The one thing that matters

FluidVoice's local cleanup does not feel slow because the LLM work is hidden inside time the user is already spending, and because the pipeline refuses to wait on it.

The mechanism, in order of impact:

1. **The model is warmed while the user is still talking.** The first PCM buffer triggers a prewarm that loads the local model and primes a KV cache for a fixed prompt prefix. The prewarm is deliberately not cancelled on stop, only on abort or on a new recording, so the post-stop cleanup call hits a warm cache. (`ContentView.swift:4056-4114`, `:3945-3951`)
2. **The prompt prefix is constant and tiny.** On the private path there is no system prompt at all. The cleanup instructions are baked into a small purpose-tuned model. That is what makes the prefix cache stable and prefill near zero. Any custom prompt disqualifies the private route and falls back to the OpenAI-shaped path. (`ContentView.swift:2407-2409`)
3. **Rules run first and last; the LLM only sees the middle.** Filler-word strip, custom dictionary regexes and spoken punctuation run before the model. Slash-command/mention literalization, lowercase-first, trailing-period, chaining space and smart caps run after it, so user preferences always beat the model. (`ContentView.swift:2777-2880`)
4. **Fast paths never flash a spinner.** "Transcribing" appears only after 100 ms, "Refining" only after 500 ms, and both are cancelled the instant a result lands. (`ASRService.swift:393`, `ContentView.swift:234`)
5. **Any failure types the raw transcript.** Throw, timeout, unverified provider, over-budget input: the user still gets text, plus a non-blocking failure card with a retry. (`ContentView.swift:2833-2845`)
6. **Long inputs skip the model instead of being truncated.** A token-budget estimate (words/0.75, dense segments charged bytes/2) must leave headroom for output; at the 4096 default context that is about 800 words. (`SettingsStore.swift:21-107`)
7. **Streaming stays off the main thread and off the target app.** Partial cleanup goes only to the overlay, at 30 fps, and only after 0.5 s so short cleanups never render a partial. Nothing is speculatively typed. (`ContentView.swift:79-145`, `LLMClient.swift:59-63`)
8. **Stop cue plays when the mic stops, not when text arrives.** (`ASRService.swift:2906-2915`)
9. **Insertion is dispatched and the overlay retired in the same main-actor turn**, so dismissal never queues behind history writes or a render. (`ContentView.swift:3005-3050`)
10. **No racing timeout task.** They rely on URLSession timeouts because a task-group timeout wrapper can keep the caller suspended for the full timeout. (`LLMClient.swift:193-196`)

What they do not do, and where Juno can beat them: there is no wall-clock budget on the cleanup call (it inherits a 30 s network timeout, which is absurd for dictation), no speculative cleanup on partials, and no cleanup caching.

---

## 2. Where Juno stands today

From the audit of `~/repo/juno` on the same day:

| Area | Juno now | FluidVoice |
|---|---|---|
| Cleanup after STT | One regex that drops `[BLANK_AUDIO]`-style artifacts (`tauri-plugin-voice-transcription/src/utils.rs:42-57`), Whisper path only | Rules + optional local or cloud LLM |
| Custom dictionary | None | Regex replacements + ASR vocabulary boost + acoustic pronunciation profiles + auto-learned corrections |
| Spoken punctuation | None (`--auto-punctuation` CLI flag is declared and errors) | Prefix-gated ("literal comma"), 39 symbols + 4 actions |
| Capitalization from context | None | Reads text before cursor at recording start |
| STT | Whisper large-v3-turbo q5 via whisper-rs, batch; Parakeet CTC exists but is unreachable from the UI | Parakeet TDT/Flash, Nemotron, Cohere, Apple Speech, Whisper; streaming |
| Mic start | Engine pre-warmed, mic opened fresh per press | Core Audio IOProc held prepared across idle; 2 s first-PCM readiness gate |
| Hotkeys | global-shortcut plugin + NSEvent monitor; Fn only as bare modifier; hold threshold 300 ms | CGEventTap; any modifier side as bare key; Right Option default; tap/hold/automatic at 0.4 s |
| Insertion | Clipboard + Cmd+V, 600 ms conditional restore | Unicode CGEvent chunks of 200 first, AX, then paste; full pasteboard snapshot restore guarded by changeCount |
| History | None for dictation | SQLite, raw + processed, audio optional, diff view, budget |
| Per-app | Frontmost app is read for the agent only | Prompt bindings per bundle ID, plus app-class heuristics |
| Overlay | Floating bar (6 looks), tray icon, screen-edge glow | Notch or bottom pill, 4 size presets |

The only written intent in Juno for cleanup is one line in `docs/plans/dictation-latency.md` (item 10): capture app/AX context in parallel with recording and run an opt-in cleanup pass with a "preserve exact wording" bypass. That line is still the right shape.

---

## 3. Adopt, in order

Each item is scoped so it ships whole. Latency cost is stated up front.

### 3.1 Deterministic layer (zero added latency)

Runs in the Rust final-result path before paste. All string work, well under 1 ms.

- **Artifact filter on both engines.** The Parakeet session skips `filter_transcription_text` today. Trivial fix.
- **Filler-word removal**, default on, list editable. Whole-word match on punctuation-trimmed tokens.
- **Custom dictionary.** Entries are `triggers[] -> replacement`. Compile once to case-insensitive regexes with word-boundary edges only when the trigger edge is a word char, sort longest pattern first, rebuild on settings change. Whitespace-only replacements eat surrounding spaces.
- **Prefix-gated spoken punctuation.** Modern models already emit punctuation, so "comma" must stay a word. Require a prefix word ("literal comma"). Fast-bail when the prefix is absent. Derive spacing from the symbol (period right-attached, "dot" no-space, "dash" spaced, "hyphen" tight, quotes toggle). Context guards for "dot" (only near TLD/host words), "slash" (only near path words) and bare "plus"/"equals".
- **Context-aware capitalization and chaining.** At recording start, read the text before the cursor via AX in parallel with mic start. On insert: capitalize after newline or sentence end, lowercase otherwise, add a leading space when the previous char is not whitespace. This is the single biggest quality win per line of code and it is free.
- **Autocomplete-safe trailing space.** If the output ends in `/cmd` and the target is a code/chat app, or `@name` in Slack-like apps, drop the trailing space so the app's popup survives. Keep the app-class list in a constant, not a setting.

### 3.2 Overlap everything with the recording (removes latency)

Juno's own plan items 6 and 7, plus one new one.

- Hold the cpal input stream prepared across idle with a small pre-roll ring buffer, so press-to-first-sample is near zero. Guard against the Bluetooth HFP trap: never instantiate the capture graph while idle on a BT input.
- Start the mic on the press event itself, not on the 50 ms poll tick.
- Gather AX context (frontmost bundle ID, text before cursor) on the press, concurrently, and hand it to the final-result handler.
- If a local model is enabled, load it and prime the prefix cache on first PCM. Do not cancel on normal stop.

### 3.3 Local LLM cleanup, opt-in, with a hard budget

Recommended stack for Juno: `llama-cpp-2` (llama.cpp bindings, Metal) in-process, GGUF, with the prompt prefix saved as a llama.cpp state so prefill is only the transcript. Ollama and LM Studio as optional external OpenAI-compatible providers for people who already run them. Candidate models to benchmark on M-series: Qwen3 0.6B and 1.7B, Gemma 3 1B, at Q4_K_M. Fine-tuning a sub-1B cleanup model on Juno's own history pairs is the long-run answer FluidVoice chose; do not start there.

Rules that keep it from feeling slow, all of which Juno should enforce and FluidVoice does not fully:

- **Wall-clock budget.** Set a per-utterance budget proportional to input length (start at 400 ms + 15 ms per word, cap 1.5 s). On expiry, type the rule-cleaned transcript and let the model result go to history only. Never make the user wait on the model.
- **Token budget.** Skip the model when estimated input plus 1.15x output does not fit the context with headroom. Skip, do not truncate.
- **Fixed prefix, temperature 0.2, no thinking.** One user turn, no system message, transcript at the end. The whole prompt is the FluidVoice base prompt in spirit: clean fillers and false starts, fix punctuation and casing, convert spoken numbers, apply "scratch that" corrections, never answer questions, output only the text.
- **User formatting rules run after the model.**
- **Deferred indicator.** Show "Refining" in the bar only after 500 ms. Stream partial tokens to the bar only, at most 30 fps, only after 500 ms.
- **Raw transcript on any error**, plus a small "Cleanup failed, raw text used" card with retry. Never a modal.
- **"Preserve exact wording" bypass**, either a second trigger or a spoken prefix, that skips the model and the spoken-punctuation layer. FluidVoice ships this as a per-shortcut prompt selection ("Fast, no cleanup"); Juno's trigger model already has the shape for it.

### 3.4 Dictation history with a raw/cleaned diff

Store raw and processed text, app, window title, durations, and whether the model ran. Show a token-level diff (word/whitespace/punct tokens, longest-common-subsequence, coalesced runs) with a hard cap of roughly 24 KB or 2k tokens beyond which it says so and shows both texts. This is also the evaluation harness for 3.3 and the training set for a future tuned model. Keep audio off by default, with a disk budget if it is ever on.

### 3.5 Hotkey and trigger upgrades

Juno's unified trigger model is already tighter than FluidVoice's. Borrow only these:

- **Any modifier key as a bare trigger, side-specific.** Right Option is FluidVoice's default and it is a good one. Map key codes 54/55/58/61/59/62/56/60 to sides. Since CGEvent flags only report the family, keep a set of previously-seen side key codes and fall back to `CGEventSource::key_state` to disambiguate. Requires an event tap or the existing NSEvent flags monitor.
- **"Automatic" activation mode.** Tap toggles, hold is push-to-talk. Threshold 0.4 s. Track per press whether the target was already active and whether this press started it, so a tap that started recording leaves it running. Juno's current 300 ms threshold cancels short presses; this replaces that with a better rule.
- **Clean-press detection.** A modifier-only trigger fires only if no other key or mouse button was pressed during the hold, so Right Option still chords normally. Arm only on the first press to avoid the re-arm bug (their issue #688).
- **Release-before-start.** If the key is released before the engine confirms start, register a pending stop and poll up to 3 s rather than dropping the utterance.
- **Ignore autorepeat** on any chord that waits for modifier release.
- **Conflict messages that name the other binding** and distinguish duplicate from overlap ("Overlaps Edit Mode, use a different modifier key").
- **Tag synthesized events** with an event-source user-data marker so Juno's own paste and Return keystrokes never re-enter its hotkey handling.

### 3.6 Insertion path

Juno pastes via clipboard first. FluidVoice's order is worth testing: unicode CGEvent chunks (200 UTF-16 units, split-safe on surrogate pairs) to the AX-focused PID first, AX insert second, paste third, per-character last. Clipboard-free is the default there and is what users notice. When paste is used, snapshot every pasteboard representation, mark the temp item with `org.nspasteboard.TransientType` so clipboard managers ignore it, restore off the critical path after 0.5 s, and only if `changeCount` is unchanged. Ghostty needs the paste path. Resolve the Cmd+V key code with `UCKeyTranslate` under the Command modifier and cache it per input source, since Dvorak-QWERTY-Command differs.

### 3.7 Small UX details worth copying

- Overlay goes on the screen under the mouse pointer, not the main screen.
- The activation-mode picker's subtitle is the selected mode's own description, so it re-explains itself.
- Sound preview on change, and on slider release only.
- Settings search hides results for controls that are currently unreachable (permission not granted, dependent toggle off).
- Onboarding tryout gives a real sentence in the user's language and renders the shortcut as a pressable key.
- Media pause with strict ownership: only resume what you paused, never let a stale session resume a newer one, never block first PCM on it.
- Analytics opt-out confirms before turning off, with a "what we collect" sheet. Off is fine too; the pattern is the point.

---

## 4. Do not copy

- **Five text-formatting toggles, all default off.** Lowercase first letter, remove trailing period, slash/@ formatting, space between dictations, smart capitalization. Two are splits of one legacy flag. Ship smart caps and chaining on by default with one "Formatting" preset, and put the terminal-literal behavior behind the app-class heuristic.
- **Seven settings sections for three cards' worth of content.** Notifications is two toggles, Experimental is two toggles, Audio is two controls. Juno's ten sections with advanced gating is already the better structure. Their Dictation section is a 500-line dumping ground with history, streaks, analytics and media pause in it.
- **The trigger matrix.** Six shortcut kinds plus per-prompt shortcuts plus mouse buttons plus three activation modes. Their conflict engine is excellent only because the surface is too big. Juno's six-row cap is right.
- **Stats, streaks, milestones, "time saved" cards.** Gamification is not Juno's flat macOS voice.
- **"Fluid SFX 0 through 4."** Five unnamed sounds, one with an end cue. Two named sounds or one.
- **Command mode.** Juno already has the agent; a second terminal-tool loop is a seam.
- **Local HTTP API.** Juno has MCP.
- **Pronunciation dictionary via encoder embeddings.** Clever, off by default there, gated to 15 s clips and 3+ enrollments. Not worth the surface until the plain dictionary exists.
- **The 30 s cleanup timeout.**

---

## 5. Settings layout, for reference

App sidebar (not settings): Configure (Voice Engine, AI Providers, Cleanup Styles, Custom Dictionary), Use (Command Mode, File Transcription), Activity (History, Stats), Help (Getting Started, Change logs, Feedback). Settings pushes over the sidebar in place with a back row and a search field; it remembers which app page you came from.

Settings sections in order: General, Dictation, Notifications, Audio, Overlay, Data & Diagnostics, Experimental. Defaults that matter: activation Toggle, primary hotkey Right Option, edit mode Option+R, cancel Escape, clipboard-free insert, copy-to-clipboard off, history on, audio off (4 GB budget when on), auto-learn corrections on and suggested after 1 correction, media pause off, overlay at bottom, medium size, live preview on, 150-char preview.

Onboarding is six steps: welcome, language, voice engine (download gates continue), permissions (live mic level), tryout (must dictate once), AI enhancement (skippable, primary button "Finish Setup").

---

## 6. STT note

FluidVoice's "insanely fast Parakeet" is a CoreML implementation in their FluidAudio fork, with Parakeet TDT v3 (25 languages) and a Flash EOU variant for live English. Juno's Parakeet is CTC 0.6B via ONNX and is unreachable from the UI (no permission TOMLs for the provider commands, no downloader, no settings). Juno's latency plan item 8 (streaming commit) still points at Parakeet TDT. Reviving the Parakeet path is a separate track from cleanup and should stay separate; cleanup at 3.1 works on whatever the engine returns.

---

## 7. Demo test

- Ten seconds: press the key, talk, release. Text appears with correct casing and no "um", within the same beat as today. Nothing else changes on screen.
- Removed: the stats, command mode, local API, per-prompt shortcuts, five formatting toggles, pronunciation profiles. See section 4.
- One primary action: dictate. Cleanup is a default, not a screen.
- Defaults: rules on, model off until the user opts in on first run.
- States: model failed (raw text typed, small card), model too slow (raw text typed, result in history), input too long (model skipped silently).
- Instant: 3.1 adds under 1 ms; 3.3 is bounded by the budget or it does not run.
- Seams: the bar's "Refining" label must use the same vocabulary as history and settings. One word, everywhere.
- Evidence needed before any of this is called done: a fixture of 20 recorded utterances with press-to-paste timings before and after, and a screenshot of the history diff.
- DRI: Lacy, until a LAC issue assigns it.

---

## 8. Command mode, in detail

Labelled "Alpha" in the UI. Off by default, hotkey unbound by default.

**Entry.** Press the command-mode hotkey, speak, press again to send. The transcript skips the entire dictation pipeline (no punctuation rules, no cleanup, no history) and becomes a user message in a chat. Follow-ups can be typed into the notch or the Command Mode window. Multiple chats are stored and switchable. (`ContentView.swift:3995-4020`, `CommandModeService.swift:303-355`)

**Model.** Any verified OpenAI-compatible chat provider, linked to the global provider by a "Sync" toggle or chosen separately. The local Fluid model is explicitly refused: "Fluid Intelligence for Command Mode is coming soon." Temperature 0.1. Streaming on by default with a 60 fps UI throttle; thinking tokens are shown in a collapsible "Thinking" block and never sent back to the API. (`SettingsStore+CommandMode.swift:52-85`, `CommandModeService.swift:845-900`)

**One tool.** `execute_terminal_command(command, workingDirectory?, purpose)`. It runs `/bin/zsh -c` with a 30 s timeout, cwd defaulting to home, PATH prefixed with Homebrew paths, and returns JSON `{success, command, output, error, exitCode, executionTimeMs}`. There is no other tool: no screenshots, no accessibility, no browser, no app-specific actions. Native apps are driven by osascript recipes baked into the system prompt (Reminders, Notes, Calendar, Messages). (`TerminalService.swift:20-63, 68-140`)

**Loop.** Up to 20 turns. The system prompt mandates check, execute, verify, and requires a `purpose` tag on every call. The UI step type (checking, executing, verifying) is derived from the purpose text and from command prefixes such as `ls`, `cat`, `test`, `which`. A turn with no tool call ends the run; it is marked a success if the text contains "complete", "done", "success" or "finished". (`CommandModeService.swift:372-500, 519-548`)

**Safety.** One toggle, "Confirm before execute", default on. A command is destructive when it starts with `rm`, `rmdir`, `mv`, `sudo`, `kill`, `pkill`, `killall`, `chmod`, `chown`, `chgrp`, `dd`, `mkfs`, `format`, `>`, `truncate` or `shred`, contains `rm -`, or has `rm`, `sudo` or `dd` after a pipe, `;`, `&&` or `xargs`. Those wait for a confirmation card in the Command Mode window; the notch only says confirmation is needed. Everything else auto-runs. No sandbox, no allowlist, no dry run, no audit trail beyond the chat. (`CommandModeService.swift:553-597`)

**Verdict for Juno.** Juno's agent already has roughly 25 tools, AX-grounded clicking, screenshots, a browser, MCP, per-tool approval and an audit trail. FluidVoice's command mode is a single-tool shell agent and is behind Juno on every axis. Two small things are worth keeping in mind:

- The `purpose` tag pattern (checking / executing / verifying) gives the overlay a plain-language step label for free. Juno's bar states could show the same three words instead of "Agent Working".
- The destructive-prefix list is a cheap default confirm gate for a shell tool. Juno's "Require Tool Approval" is broader; a prefix list would only matter if approval is turned off.

---

## 9. Local HTTP API

A loopback HTTP server on `127.0.0.1:47733`, off by default. There is no settings UI for it; it is enabled only by writing the `LocalAPIEnabled` defaults key (and optionally `LocalAPIPort`). Connections from anything but localhost are dropped. No auth token. Max request body 500 MB. (`LocalAPIModels.swift:4-24`, `LocalAPIServer.swift:96-105`)

| Route | Does |
|---|---|
| `GET /v1/health` | version and status |
| `GET /v1/history?limit=` | recent dictation entries, 1 to 1000 |
| `GET/POST /v1/dictionary/replacements` | read or write the instant-replacement dictionary (append or replace mode) |
| `GET/POST /v1/dictionary/custom-words` | read or write Parakeet vocabulary-boost terms |
| `POST /v1/transcribe` | transcribe a file path or base64 audio with the active local engine |
| `POST /v1/postprocess` | run the configured cleanup pipeline over `{text}` |

Purpose: let scripts, editor plugins and launcher tools use the app's local STT and cleanup as a service, and sync the dictionary from outside. It is the same idea as Juno's MCP server and headless CLI, in HTTP form. If Juno ever wants this, the native shape is two MCP tools (`transcribe_file`, `cleanup_text`) and a dictionary resource, not a second server.

---

## 10. Master improvements list for the issue

Grouped by area. Each line is one unit of work. Items marked [core] are the ones Lacy called out.

**A. Latency: overlap and pre-warm** [core]
- A1. Keep the Core Audio / cpal input prepared across idle (registration and buffers only, no running hardware, no mic indicator) so press-to-first-sample is near zero. Skip the idle pre-warm when the input is Bluetooth to avoid pinning HFP.
- A2. Start capture on the press event itself, not the 50 ms poll tick.
- A3. Pre-roll ring buffer of about 300 ms so the first syllable is never lost.
- A4. First-PCM readiness gate: arm per session and attempt, resolve on first buffer, 2 s timeout, and admit a same-device retry for Bluetooth within 5 s.
- A5. Keep the STT engine warm with a throwaway decode after load (Juno has this) and re-check readiness in a background task at recording start, so stop never waits on a model load.
- A6. Load the cleanup model and prime its prefix KV cache on first PCM. Do not cancel on normal stop; cancel only on abort or new recording.
- A7. Read AX context (frontmost bundle ID, text before cursor) on the press, in parallel with capture.
- A8. Play the stop cue when audio stops, not when text arrives (Juno already moved cues to key edges).
- A9. Deferred indicators: "Transcribing" after 100 ms, "Refining" after 500 ms, cancelled when a result lands.
- A10. Dispatch insertion and retire the overlay in the same event-loop turn; never queue dismissal behind history writes.
- A11. Silence gate: skip STT entirely for sub-threshold clips.
- A12. Benchmark log lines with a shared pipeline id (recording_start, first_pcm, stop_drained, stt_ms, cleanup_ms, insert_ms) and a 20-utterance fixture to measure press-to-paste.

**B. Hotkeys and triggers** [core]
- B1. Automatic activation mode: tap toggles, hold is push-to-talk, threshold 0.4 s. Track per press whether the target was already active and whether this press started it, so a short tap that started recording leaves it running.
- B2. Any modifier key, side-specific, as a bare trigger (Right Option as the suggested default). Keep a set of previously-seen side key codes and fall back to `CGEventSource::key_state` to disambiguate, since CGEvent flags only carry the family.
- B3. Clean-press rule: a modifier-only trigger fires only if no other key or mouse button was pressed during the hold, so the key still chords normally. Arm only on the first press.
- B4. Release-before-start: if the key comes up before capture has confirmed, register a pending stop and poll up to 3 s rather than dropping the utterance.
- B5. Ignore autorepeat events on any chord that waits for modifier release.
- B6. Tag synthesized keystrokes (paste, Return) with an event-source marker so Juno's own hotkey path ignores them.
- B7. Re-enable a disabled event tap inline in the callback, not on the next health check, so macOS never gets the key.
- B8. Conflict messages that name the other binding and distinguish duplicate from overlap.
- B9. Optional: mouse-button triggers, with unmodified left and right click rejected.

**C. Insertion** [core]
- C1. Settings picker "Text insertion": Clipboard-free (default) or Clipboard paste. Keep "Also copy to clipboard" as its own toggle.
- C2. Clipboard-free path: unicode CGEvent chunks of 200 UTF-16 units, surrogate-pair safe, posted to the AX-focused PID; fall back to AX insert, then paste, then per-character.
- C3. Paste path: snapshot every pasteboard representation, mark the temp item transient so clipboard managers ignore it, restore off the critical path after 0.5 s only if changeCount is unchanged.
- C4. Per-app forced paste list, starting with Ghostty.
- C5. Cache the Cmd+V key code per input source using UCKeyTranslate under the Command modifier.
- C6. Skip settle delays entirely when the target PID is known; skip focus restore when the exact element is already focused.

**D. Cleanup pipeline**
- D1. Artifact filter on the Parakeet path too.
- D2. Filler-word removal, default on, editable list.
- D3. Custom dictionary with cached longest-first regexes.
- D4. Prefix-gated spoken punctuation with symbol-derived spacing and context guards for dot, slash, plus.
- D5. Context-aware capitalization and chaining space from the text before the cursor.
- D6. Autocomplete-safe trailing space for `/cmd` and `@name` in known apps.
- D7. Local LLM cleanup, opt-in, wall-clock budget, token budget, fixed prefix, raw text on any failure, partials to the bar only.
- D8. User formatting rules applied after the model.
- D9. "Preserve exact wording" bypass.
- D10. Auto-learned corrections: watch the inserted range via AX for 30 s after insert, diff on value change, suggest a dictionary entry after N corrections. Later, after D3 exists.

**E. History and evaluation**
- E1. Dictation history: raw, cleaned, app, window title, durations, model flag.
- E2. Raw versus cleaned diff view with hard caps.
- E3. Copy raw / copy cleaned / delete / clear all. No audio by default.

**F. Overlay and feedback**
- F1. Overlay on the screen under the mouse pointer.
- F2. Mode-specific processing labels with one vocabulary shared by bar, history and settings.
- F3. Cleanup-failed card with retry, never a modal.
- F4. Media pause with strict ownership (optional, off by default).

**G. Settings and onboarding**
- G1. Settings search that hides unreachable controls.
- G2. Picker subtitles that show the selected option's description.
- G3. Sound preview on change and on slider release only.
- G4. Onboarding tryout with a real sentence and a pressable key; require one dictation before finishing.
- G5. Surface the STT engine picker and revive the Parakeet path (separate track).

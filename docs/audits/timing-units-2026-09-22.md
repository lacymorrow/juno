# Timing and units audit — 2026-09-22

Every duration, timeout, delay, interval, sleep, cooldown, deadline, TTL and
retry backoff in the Rust backend, the two workspace crates, and the
TypeScript frontend, checked against four questions:

1. **Unit correct** at every boundary it crosses (model, external API, config
   file, frontend)?
2. **Unit discoverable** without reading the implementation? The rule: either
   the name carries it (`duration_ms`, `timeout_secs`, `MAX_WAIT_SECONDS`) or
   the doc comment / schema description states it in words. A `Duration`-typed
   value is self-describing and satisfies this. A bare `duration: u64` does
   not, even when it currently behaves correctly.
3. **Bound sane and correctly expressed**? Comment, message and arithmetic must
   agree, in the unit they claim, and fractional or negative values must not
   slip past. `as_u64()` on a JSON number is the specific trap.
4. **Clock source right**? `Instant` for elapsed time, `SystemTime` only for
   wall-clock timestamps.

Scope: 136 files contain a timing construct (`Duration::from*`, `.elapsed()`,
`tokio::time::{sleep,interval,timeout}`, `setTimeout`, `setInterval`). Every
one was read or grepped. The clean rows are listed so the next person knows
this was a sweep, not a sample.

Branch: `fix/timing-units-audit`, built on `fix/batch-halt-on-failure`.
**Nothing in this document was compiled or tested.** A demo was being recorded;
`cargo` and `npm test` were off-limits for the whole session. Every test
described below is written but unrun.

---

## Defects, worst first

### D1 — Action cooldown measured on the wall clock — FIXED

`src-tauri/src/agent/tools/anthropic_computer_use.rs:291` (`enforce_action_cooldown`)

`ACTION_COOLDOWN_MS` (300ms) is the only pacing an AX-path click or a `type`
ever gets. `InputArbiter` measures its own 500ms cooldown with `Instant`; this
one measured elapsed time with `SystemTime` millis since the epoch. The two
disagreed about what a clock is, silently.

A **backward** clock step was survivable: `saturating_sub` floors the gap at
zero and the action sleeps a full 300ms it did not need. A **forward** step is
the failure. The measured gap looks enormous, the cooldown sleeps nothing, and
the pacing disappears — which is the "clicked too fast" failure the constant
exists to prevent. Nothing throws, nothing logs; the clicks just start missing.

**Fix**: the stamp is now milliseconds since a process-start `Instant`
baseline (`MONOTONIC_BASELINE` + `monotonic_now_ms`, line 32). Chosen over
`Mutex<Option<Instant>>` because this sits on the hot path of every click and
currently takes one relaxed atomic load plus one store — a mutex would add a
lock, and poisoning, where there is none. Readings are offset by one so `0`
stays free as the "no action recorded yet" sentinel. **Neither cooldown
constant's value changed.**

Tests: sentinel, monotonicity, read-only actions still skip the cooldown, two
consecutive UI actions are separated by it, and a regression guard asserting
the stored stamp is small (process-relative) rather than ~1.7e12 (epoch
millis).

### D2 — Timer tools accept any schedule a model invents — FIXED

`src-tauri/src/agent/tools/timer_tools.rs` — `set_timer`, `set_screen_monitor`,
`set_file_monitor`, all registered on every agent via
`agent/providers/factory.rs:527`.

Each schema declares `"minimum": 1` and no maximum. A JSON Schema `minimum` is
advisory; nothing on the request path enforced it. Four failures followed, all
silent:

| # | Site | Failure |
|---|------|---------|
| a | `now + delay_seconds` | Float→int casts saturate, so `delay_seconds: 1e30` became `u64::MAX`; the add panicked in debug, or wrapped in release to a `trigger_time` in the past, firing the timer at once. |
| b | `check_interval_seconds: 0` | Reached `tokio::time::interval`, which **panics** on a zero period. The monitoring task died with nothing in the log to say the monitor had stopped. |
| c | max-duration check in both monitors | `SystemTime` epoch-secs with a bare `-`. A backward clock step underflowed: panic in debug, wrap in release that read as a huge elapsed time and stopped the monitor immediately. Same clock-source defect as D1. |
| d | `check_interval_seconds` / `max_duration_seconds` read with `as_u64()` | Returns `None` for `2.5`, so a model asking for a 2.5-second interval silently got the default and nothing said so. |

**Fix**: one `schedule_seconds` helper (line 305) reads the value as f64,
rounds, and clamps into `1..=MAX_SCHEDULE_SECONDS` (30 days, line 288). `None`
still distinguishes "absent or non-numeric" from "asked for badly", which the
callers need. Both monitor loops now measure elapsed time with `Instant`; the
file monitor keeps a separate wall-clock stamp, renamed
`started_at_unix_seconds`, because that one is genuinely compared against a
file's mtime. Nine tests cover whole, fractional, zero, negative, absurd,
`u64::MAX`, `f64::MAX`, missing and non-numeric inputs, plus a check that the
cap equals the number of seconds its doc comment claims.

### D3 — Network clients with no timeout at all — FIXED

The project rule (`src-tauri/CLAUDE.md`, "HTTP Client Initialization") is that
a client must set a timeout, because `reqwest::Client::new()` has none: a
server that accepts the connection and then never answers leaves the caller
holding a future that never resolves.

| File:line | What | Verdict |
|---|---|---|
| `agent/providers/rig.rs:61` | AI provider, rebuilt on every `decide_next_action`. No timeout, no external wrapper. | **Was a rule violation.** Now uses `HTTP_REQUEST_TIMEOUT_SECONDS` / `HTTP_CONNECT_TIMEOUT_SECONDS`, same as the Anthropic brain. |
| `tts/elevenlabs.rs:52` | The TTS request itself. No timeout. | Fixed, same constants. |
| `tts/replicate.rs:91` | Initial POST. Its poll loop is bounded by `REPLICATE_TIMEOUT_SECONDS`, but that deadline is only armed once the POST returns. | Fixed, same constants. The sibling Chatterbox client at line ~390 already set 120s. |
| `agent/providers/gemini.rs:412` | `Default` impl: `.build().unwrap_or_else(\|_\| Client::new())` — asks for a timeout, then silently discards it. | Fallback kept (`Default` cannot fail) but now logs at error level. |
| `tts/supertonic.rs:21` | Same pattern in a `OnceLock`. | Same treatment. |
| `utils/network.rs:73` | `Client::new()` | **Clean.** Every call is wrapped in `tokio::time::timeout(3s)`. Left alone. |

### D4 — The `wait` label disagreed with the `wait` handler — FIXED

`anthropic_computer_use.rs`, `get_descriptive_tool_name`

The handler reads `seconds` first, then `duration`, both as f64. The label read
only `duration`, only via `as_u64()`. So `{"seconds": 5}` was logged and shown
as `computer/wait(1s)`, and so was `{"duration": 1.5}`. Cosmetic in effect, but
a label that disagrees with the action it describes is how a unit bug survives
a review — this is the same file where the 30000ms/30s `wait` cap hid. The
label now reads the same keys, in the same order, as the same type. Six tests.

### D5 — Bare time names a reader or an agent could misread — FIXED

| File:line | Name | Unit | Fix |
|---|---|---|---|
| `settings/mod.rs:145-147` | `CloudSettings.reconnect_interval` / `.heartbeat_interval` / `.command_timeout` | seconds | **Documented, not renamed.** These are persisted Tauri Store keys; renaming the field renames the key and orphans every setting already on disk. `CloudConfig` already carried `// seconds`; the struct the store actually writes now says it too. |
| `cli/mod.rs:200` | `juno voice query --duration` | seconds | Help text now says "in seconds", matching every other CLI time flag. |
| `cli/mod.rs:11` | `DEFAULT_VOICE_TIMEOUT` | seconds | Renamed `DEFAULT_VOICE_RECORDING_SECONDS`. Module-private, no external callers. |
| `tools/mod.rs:254` | shell tool schema, `timeout_seconds` described as "Optional timeout." | seconds | Description now says SECONDS. The name carried it; the sentence the model reads did not. |
| `prompts/templates.rs:826` | `<AnimatedNumber duration={1200} />` in the agent-facing prompt | ms | Prompt now states MILLISECONDS. The TS prop is documented as ms in `animated-components.tsx`, but the prompt is what the model reads, and a bare `duration` invites `duration={2}` for "two seconds" — the hold_key bug again, in the UI layer. |

**Frontend call sites touched: none.** The only `invoke` in `src/` carrying a
time value is `DevToolsPanel.tsx:173` → `wait` with `duration_sec`, already
unit-named and already correct (`durationMs / 1000.0`). Nothing in `src/` reads
the renamed CLI constant. `src/lib/constants.generated.ts:944` holds the
*string* `'heartbeat_interval'`, not the value, and is unaffected because the
field was documented rather than renamed.

---

## Reported, deliberately not changed

### R1 — Every input validator in `commands/` is debug-only (systemic)

`commands/debug_utils.rs:37-46` — `DebugConfig::production_mode()` sets
`validate_inputs: false`, and every `commands/*` validator is gated on it. In a
release build there is **no** input validation on these commands:

- `commands/core.rs:913` — `wait(duration_sec)` has no production bound at all.
  `duration_sec: 1e30` saturates to `u64::MAX` millis. The agent path is capped
  at 30s by `tool_provider.rs:1195`, so this is reachable only from the Tauri
  command (DevTools) or a future caller.
- `commands/shell.rs:741-758` — the `timeout == 0` and `timeout > 3600` checks
  never run in release. A `timeout_seconds: 0` becomes `Duration::from_secs(0)`
  and every command reports an instant timeout.
- `commands/keyboard.rs:237` — `reasonable_duration(duration_ms)` likewise.

Not changed: making validation unconditional alters production behaviour across
a large number of commands and is a security/robustness decision, not a units
fix. Worth its own issue.

### R2 — `commands::core::wait` blocks a tokio worker

`commands/core.rs:928` → `desktop.wait(duration_ms)` →
`platforms/macos/interaction.rs:2009` → `std::thread::sleep`. Called from an
`async` Tauri command, so a 30-second agent `wait` parks a tokio worker thread
for 30 seconds. The project rule says `tokio::task::spawn_blocking` for
blocking operations. Not changed: the desktop wrapper is synchronous by design
and the right fix (a `tokio::time::sleep` at the command layer, or
`spawn_blocking`) is a behaviour call, and this path is on the demo.

### R3 — Cloud backoff's declared cap is not the cap

`constants/api.rs:132` declares `MAX_RETRY_INTERVAL_MS = 300000` ("5 minutes").
Nothing reads it. The real backoff in `cloud/connector.rs:507-512` is
`2000ms * 2^min(retry, 5)`, maxing at 64s. Not a defect — the effective cap is
*under* the declared one — but a constant that names a bound nobody enforces is
the shape of the `wait` bug, and someone will eventually trust it.

### R4 — Two different defaults for the same cloud field

`cloud/config.rs:124` has `command_timeout: 600`; `settings/mod.rs:416` has
`command_timeout: 30`, and `cloud/config.rs:401` builds the config *from*
settings. So the effective value is 30 seconds, not the 600 the config's own
default advertises. Same story for the pair as a whole. Judgment call about
which is correct; not a unit error.

### R5 — Divergent hold_key caps

`resolve_hold_key_duration_ms` clamps at `MAX_HOLD_KEY_MS = 300_000`
(Anthropic's 300s), while `debug_utils.rs:253` `reasonable_duration` rejects
anything over `30_000`ms. Both are internally consistent and in their stated
units; they simply disagree with each other, and per R1 the second only runs in
debug.

### R6 — Accepted-and-ignored timeout knobs

- `agent/tools/browser_tools.rs:70, 106, 152` — three `"timeout"` schema
  properties, each correctly described as milliseconds, none of which the
  executor ever reads. The model is told it can set a navigation timeout; it
  cannot.
- `mcp-server-os-level/src/lib.rs:1495` — the `bash` tool's `timeout` parses
  and then logs "timeout parameter specified but not yet implemented."

Not a unit defect, but the same failure mode: the caller's instruction is
silently discarded.

### R7 — File monitor's `Modified` check compares against the wrong baseline

`timer_tools.rs` (file monitor, `FileMonitorType::Modified`) compares the
file's mtime against the *monitor's start time*, not the previous tick. It
therefore only ever fires for a file modified within `check_interval + 1`
seconds of the monitor starting. A logic bug, not a units bug; flagged here
because it lives inside the code D2 touched.

### R8 — `enforce_action_cooldown`'s load/store is not atomic

Two concurrent sessions can both read a stale `LAST_UI_ACTION_MS` and skip the
cooldown. In practice `InputArbiter` serializes all coordinate-based physical
input, so the window is narrow. Making it a CAS loop is a real change to
pacing behaviour under parallel sessions; left alone.

### R9 — Cosmetic

- `mcp_integration.rs:166` — `let _base_delay = Duration::from_millis(500);` is
  dead; the 500 is hardcoded in the formula two lines down. The comment, the
  arithmetic and the 30s cap all agree, so only the dead binding is wrong.
- `commands/notifications.rs:29` — `NotificationData.timeout: Option<u32>`,
  bare, with the comment "Override default duration". Read by nothing.
- `tauri-plugin-voice-transcription/src/controller.rs:546` —
  `Instant::now() - level_emit_interval` panics if the process starts within
  70ms of boot. Not reachable in practice.

---

## Everything checked and clean

### Constants

| File | Verdict |
|---|---|
| `constants/timeouts.rs` (140 constants) | **Exemplary.** Every constant carries `_MS` or `_SECONDS`. Grepped for cross-unit misuse (`from_millis(*_SECONDS)`, `from_secs(*_MS)`, stray `*1000` / `/1000`): zero hits anywhere in the workspace. |
| `constants/api.rs` `cloud_networking` | All `_MS` named. See R3 for the unused one. |
| `constants/settings.rs:123-124` | `MIN/MAX_HEARTBEAT_INTERVAL` with `// seconds`; matches every consumer. |

### Computer use / input

| Site | Unit | Named | Correct | Clock |
|---|---|---|---|---|
| `input_arbiter.rs` `DEFAULT_COOLDOWN` and all uses | `Duration` | yes (typed) | yes | `Instant` ✓ |
| `anthropic_computer_use.rs:1322` `resolve_hold_key_duration_ms` | ms out, s or ms in | yes | yes, clamped, rejects ≤0 and non-numeric | n/a |
| `anthropic_computer_use.rs:2661-2677` computer schema | ms / s | yes, both descriptions shout the unit | yes | n/a |
| `tool_provider.rs:1195` `MAX_WAIT_SECONDS` | seconds | yes | yes, f64, catches fractional and non-finite | n/a |
| `commands/computer.rs` `ComputerInput.duration_ms` | ms | yes, with `#[serde(alias="duration")]` | yes | n/a |
| `commands/keyboard.rs:222` `hold_key(duration_ms)` | ms | yes | see R1 | n/a |
| `commands/core.rs:896` `wait(duration_sec)` | seconds | yes | see R1 | n/a |
| `state/desktop_wrapper.rs:83,104` | ms | yes | yes | n/a |
| `platforms/macos/interaction.rs:1597,2009` | ms | yes | yes | see R2 |
| `platforms/mod.rs:75,81` trait signatures | ms | yes | yes | n/a |
| `mcp-server-os-level/src/lib.rs:558` computer schema `duration_ms` | ms | yes, description warns about Anthropic's bare `duration` | yes | n/a |
| `prompts/templates.rs:1217-1239` hold_key and wait | s / ms | yes, stated in words | cap matches `MAX_WAIT_SECONDS` | n/a |

### Agent loop

| Site | Verdict |
|---|---|
| `agent_runner.rs:696,765` approval poll | `timeout_seconds * 1000 / 50` against a 50ms sleep. Divisor and sleep agree. Default 60s matches the frontend's `?? 60`. Clean. |
| `agent_runner.rs:860` `wait_for_mouse_movement_completion` | All `_ms` named, `Instant` for the budget, exponential backoff capped by remaining time. Clean. |
| `anthropic.rs:95-100` cancel wait | `CANCEL_TIMEOUT_MS` named, `Instant`, 50ms poll. Clean. |
| `agent/error_recovery.rs:139-158` | `base_retry_delay` / `max_retry_delay` are `Duration`-typed (self-describing); every elapsed measurement is `Instant`. Clean. |
| `agent/implementations/memory_manager.rs` | `Instant` for every `.elapsed()`; `SystemTime` only for record timestamps. Clean. |

### Providers

| Site | Verdict |
|---|---|
| `providers/anthropic.rs:288-295` | `HTTP_REQUEST_TIMEOUT_SECONDS` + `HTTP_CONNECT_TIMEOUT_SECONDS`, `map_err` on failure. The reference implementation. |
| `providers/gemini.rs:127`, `providers/openai.rs:107` | 120s explicit. Clean (the `Default` fallback is D3). |
| `providers/claude_cli.rs:114` `CLI_TIMEOUT` | `Duration`-typed, 300s, used with `tokio::time::timeout`; cancellation tests use `Instant`. Clean. |
| `providers/claude_cli.rs:770` | Reads the CLI's own `duration_ms` and logs it as `ms`. Correct. |

### Cloud

| Site | Verdict |
|---|---|
| `cloud/connector.rs:466-519` | Backoff and check interval both from `_MS` constants; `Duration`-typed throughout. Clean (see R3). |
| `cloud/connector.rs:773, 937, 1088, 1149` | Auth timeout, connection timeout, heartbeat and status intervals all `Duration::from_secs`. Clean. |
| `cloud/connector.rs:58, 542, 952, 1258` | `connection_start_time` and latency measurement use `Instant`; `last_heartbeat` and message stamps use `SystemTime`. Correct split. |
| `cloud/client.rs:106, 265` | `reconnect_interval` and `heartbeat_interval * 3` as seconds. Consistent (naming was D5). |

### MCP

| Site | Verdict |
|---|---|
| `mcp_integration.rs:155, 791, 809` | `Duration::from_secs(timeout_seconds)`. Clean. |
| `mcp_integration.rs:165-183` backoff | Comment ("500ms, 1s, 2s … 30s capped"), arithmetic and cap all agree. `Instant` for `last_failure.elapsed()`. Clean apart from R9's dead binding. |
| `mcp_integration.rs:1566` batch timeout | `timeout_seconds * 2`, commented. Clean. |

### Voice, TTS, dictation

| Site | Verdict |
|---|---|
| `tauri-plugin-voice-transcription/src/always_listening.rs` (8 constants) | All `_MS` named; sample-count conversions are explicit `* rate / 1000`. `Instant` everywhere. Clean. |
| `tauri-plugin-voice-transcription/src/controller.rs:545` | `level_emit_interval` is `Duration`-typed, `Instant` for elapsed. Clean (see R9). |
| `tauri-plugin-voice-transcription/src/mic_permissions.rs:120,188` | `Instant` + `Duration`. Clean. |
| `dictation_monitor.rs:53-165` | Five thresholds, all `_MS` constants, all measured with `Instant`. Clean. |
| `tts/mod.rs:84-94, 812-861` | `Instant` for elapsed, `Duration` literals. Clean. |
| `tts/replicate.rs:190-211, 462-479` | `Instant` for the poll deadline against `REPLICATE_TIMEOUT_SECONDS`, 1s poll. Clean. |
| `tts/supertonic.rs:12` `REQUEST_TIMEOUT_SECS` | Named. Clean (fallback was D3). |

### Scheduler, permissions, startup, misc

| Site | Verdict |
|---|---|
| `scheduler.rs:111` `now_secs()` | `SystemTime` — **correct**, cron schedules are wall-clock events. `TICK_INTERVAL_SECS` named. Clean. |
| `commands/permissions.rs:76-140` | `Instant` in the cache tuple, TTL from `PERMISSIONS_CACHE_TTL_SECONDS`. Clean. |
| `permission_gate.rs:27, 104` | `ASK_AGAIN_AFTER` is `Duration`-typed; `LAST_ASKED` holds `Instant`. Clean. |
| `startup.rs:112-118` | `PERMISSION_CACHE` uses `Instant` + `Duration`. `DESKTOP_CACHE` uses `SystemTime` millis but with a documented `saturating_sub`, and a forward step only re-inits early — harmless. Clean. |
| `greeting.rs:142-192` | `Instant` deadline for `wait_for_the_bar`; `SystemTime` for uptime, compared against the kernel's wall-clock boot time with `saturating_sub`. Correct on both counts. |
| `input_control/mod.rs:28-395` | `CONSENT_TIMEOUT_SECONDS * 1000 / CONSENT_POLL_INTERVAL_MS` against a 50ms sleep. Divisor and sleep agree. Clean. |
| `commands/shell.rs:212-404` | `timeout_seconds` → `Duration::from_secs`; poll uses `start_time.elapsed()` (`Instant`). Validator's comment, message and arithmetic all agree on seconds. Clean apart from R1's gating. |
| `commands/memory.rs:175` `retention_seconds` → `screenshot_retention_seconds` | Named at both ends. Clean. |
| `debug_utils.rs:253, 271` | `reasonable_duration(duration_ms)` caps 30000 and says "ms"; `valid_duration_seconds(duration_sec)` caps 60 and says "seconds". Both self-consistent. Clean apart from R1 and R5. |

### Frontend (`src/`)

72 `setTimeout` / `setInterval` sites and every duration constant were read.

| Site | Verdict |
|---|---|
| `DevToolsPanel.tsx:165-173` | `durationMs / 1000.0` → `invoke("wait", { duration_sec })`. **The only `invoke` in `src/` carrying a time value.** Conversion correct, both names unit-carrying. |
| `commands/media.rs:168-176` ↔ `now-playing-card.tsx` | Spotify reports duration in **milliseconds**, Apple Music in seconds. The AppleScript divides Spotify's by 1000 at the source, the field is `duration_secs` at both ends, and there are unit tests for both players. **Exemplary** — this is the shape every cross-boundary unit should have. |
| `ChatMessageV2.tsx:291-322` | Approval countdown: `timeoutSeconds`, ticking `setInterval(..., 1000)`, decrementing by 1. Units agree. Default `?? 60` matches the backend. Clean. |
| `DesktopCursorOverlay.tsx`, `FloatingBar.tsx`, `SnapWellsOverlay.tsx`, `ListeningGlow.tsx`, `BarFlameBorder.tsx`, `bar/dynamic-bar.tsx`, `conversation.tsx`, `PermissionNotice.tsx`, `useConversation.ts`, `useSound.ts`, `now-playing-card.tsx` | Every duration constant is `*_MS`-suffixed; every `setTimeout` literal is milliseconds and plausible for its purpose. Clean. |
| `HistoryView.tsx:18`, `AutomationsSettings.tsx:51` | Both take Unix **seconds** and name the parameter `unixSecs` / `unixSeconds`. Clean. |
| `animated-components.tsx:73` | `duration` prop is bare, but documented as ms. The agent-facing prompt was the gap — see D5. |

---

## Not verified

- **Nothing here was compiled, type-checked, linted or tested.** `cargo check`,
  `cargo test`, `npx tsc` and `npm test` were all off-limits (a demo was being
  recorded). All eleven edited Rust files were run through
  `rustfmt --edition 2021 --check`, which parses them: all eleven parse and are
  already rustfmt-clean. That catches syntax, and nothing else. In particular
  these are unverified:
  `std::sync::LazyLock<std::time::Instant>` initialised with the `Instant::now`
  fn item (D1); the `pub(super)` visibility of `schedule_seconds` /
  `MAX_SCHEDULE_SECONDS` from the file-level test module (D2); and the f64
  `Display` formatting the new wait-label tests assert on (`5.0` → `"5"`,
  `1.5` → `"1.5"`).
- Whether Anthropic's live computer-tool schema still treats a bare `duration`
  as seconds. The prior commit on this branch verified it; not re-checked here.
- Whether a 120s request timeout is long enough for the longest real ElevenLabs
  or Replicate generation. It matches what every other client in the tree uses,
  and previously there was no bound at all, so this is strictly tighter — but
  the number is inherited, not measured.

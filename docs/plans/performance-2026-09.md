# Juno performance plan, September 2026

Status: six branches built, none compiled, none merged. Demo recording is today, so every item below lands
on its own branch in its own worktree and nothing merges to main until the demo
is recorded and a full `cargo check` / `clippy` / `tsc` / test pass has been run
for all of them together.

DRI: Lacy Morrow.

## The one sentence

Juno's computer use feels slow for two unrelated reasons, and roughly 80% of the
felt pain is dead air rather than compute: a measured 8.5s Claude CLI task shows
the user text only in its last 1.4s.

## Measured baseline (2026-09-22, this machine)

| # | Finding | Evidence | Cost |
|---|---------|----------|------|
| 1 | CLI spawns a fresh `claude` process per query | wall 3.49s vs CLI-reported `duration_ms: 1758` on a trivial prompt | ~1.4-1.7s per query |
| 2 | `--include-partial-messages` never passed | `claude_cli.rs:311` `build_args` | no token/thinking/tool deltas |
| 3 | NDJSON parser drops `stream_event`, `user`, `tool_use` | `claude_cli.rs:725-790`, `other => skipped` | ~7s dead air in an 8.5s task |
| 4 | `extract_text_from_message` returns only `text` blocks | `claude_cli.rs:895` | tool-only turns render nothing |
| 5 | `cache_control` on system + tools only, never on `messages` | `anthropic.rs:1645-1680` | full history reprocessed every turn |
| 6 | `limit_screenshot_history` rewrites an older message every turn | `anthropic.rs:474` | mutates the cache prefix continuously |
| 7 | Screenshot pipeline double-encodes | `commands/core.rs:48` | ~180ms/shot measured waste |
| 8 | ~~Two stacking action cooldowns~~ **WRONG, see correction below** | `anthropic_computer_use.rs:220/1299` + `input_arbiter.rs:69` | none |
| 9 | No `output_config.effort` / no fast mode anywhere | grep over `src-tauri/src` | unused latency lever |
| 10 | Tool type is `computer_20251124` | `constants/api.rs:40` | one generation behind GA `computer_toolset_20260801` |

Reference event timeline captured from the real CLI:

```
t+ 1.22s  system/init
t+ 2.60s  content_block_start/tool_use     <- Juno shows nothing
t+ 2.60s  input_json_delta  x17            <- Juno shows nothing
t+ 3.24s  assistant (tool-only)            <- extract_text returns ""
t+ 4.57s  user (tool_result)               <- Juno shows nothing
t+ 5.29s  content_block_start/thinking     <- Juno shows nothing
t+ 5.79s  thinking_delta                   <- Juno shows nothing
t+ 7.13s  text_delta x20                   <- first visible pixel
t+ 8.09s  result
```

## Correction: the cooldowns do not stack (2026-09-22)

Finding 8 above was wrong and is kept only so nobody re-derives it. The two
cooldowns OVERLAP. `enforce_action_cooldown` records `LAST_UI_ACTION_MS` after
its own sleep, and `InputArbiter::acquire` reads `last.elapsed()` at the moment
it runs, which is after that sleep. Sequential sleeps to absolute deadlines
finish at `max(T0+300, T+500)`, and since the guard is released after the floor
records, the arbiter deadline always dominates. The floor adds zero for any
guard-taking action. Real spacing is ~500ms for guard-taking actions and ~300ms
for AX-path clicks and typing. 800ms never occurs.

There is one genuine exception, documented but not fixed: the floor measures
with `SystemTime` (wall clock) while the arbiter uses `Instant` (monotonic). A
backward clock step makes the floor sleep a spurious 300ms; a FORWARD step makes
it sleep zero, removing the only pacing AX-path clicks and typing ever get,
which is the direction that actually causes "clicked too fast" failures. Fixing
it means moving the floor to `Instant`. Deferred until after the demo.

## Decisions already taken (do not re-litigate)

- The CLI keeps loading Bash/Read/Edit/WebFetch. They augment rather than remove
  functionality. Revisit later; do not add `--tools ""` now.
- Effort is a hidden advanced setting with a sane default. Not user-facing yet.
- Screenshot resolution tiers, capture, scaling, and coordinate math are OFF
  LIMITS. They were hard-won across multiple displays. No item here touches them.
- Native action batching must be proven to work with or cleanly replace the
  existing batching BEFORE any existing batching is removed.
- No agent runs `cargo`. All compilation happens in one pass at the end.

## Phase 1 - Make Juno show what Claude is doing

Branch: `perf/cli-streaming-feedback`. Files: `src-tauri/src/agent/providers/claude_cli.rs`,
settings plumbing, frontend reasoning surface if one already exists.

1. Add `--include-partial-messages` to `build_args`.
2. Handle `stream_event` in the NDJSON loop:
   - `content_block_start` type `thinking` -> begin a reasoning surface
   - `content_block_delta` type `thinking_delta` -> stream reasoning text
   - `content_block_start` type `tool_use` -> announce the tool by name
   - `content_block_delta` type `input_json_delta` -> accumulate and show the
     target (e.g. "Clicking (640, 60)") before the action happens
   - `content_block_delta` type `text_delta` -> replaces the current
     whole-message diffing as the text path
3. Handle `user` events so an arriving `tool_result` ends the pending tool
   indicator instead of leaving it hanging.
4. Keep the existing `assistant` handler as a fallback for when partial messages
   are unavailable, but stop it double-emitting text that `text_delta` already
   emitted.
5. `--effort` passthrough as a hidden advanced setting with a sane default.

Non-goals: no `--tools` restriction, no process lifecycle changes, no changes to
TTS tag parsing semantics.

## Phase 2 - Kill the per-query process boot

Branch: `perf/cli-persistent-session`. Risk: HIGH. Ships behind an
off-by-default flag.

Spike first, implement second. The open questions that must be answered against
the real CLI before any integration:

- Exact `--input-format stream-json` input message shape.
- How to cancel a running turn without killing the process. Escape currently
  kills the subprocess (LAC-3697); a persistent process needs a different path.
- Process death, restart, and leak handling across app suspend/resume.
- One process per conversation, or one global process multiplexing sessions.

Deliverable: a design note plus a self-contained module and a flag, defaulting
OFF. Do not restructure the existing parse loop; Phase 1 owns that code.

## Phase 3a - Prompt cache and pruning economics

Branch: `perf/anthropic-cache-breakpoints`. Files: `src-tauri/src/agent/providers/anthropic.rs`.

1. Add `cache_control` breakpoints on the last `tool_result` block of recent
   turns (up to 3 more beyond the existing system + tools breakpoint).
2. Convert `limit_screenshot_history` from per-turn pruning to budget-triggered
   batch pruning, so the message prefix stays append-only between prunes.

Explicitly unchanged: resolution, capture, scaling, coordinates,
`MAX_RECENT_SCREENSHOTS` semantics as seen by the model beyond the batching
change.

## Phase 3b - computer_toolset_20260801

Branch: `perf/computer-toolset-investigation`. INVESTIGATION AND PLAN ONLY, no
implementation.

Current GA toolset is `computer_toolset_20260801` (Opus 5, Sonnet 5, Opus 4.8,
Fable 5, Fable 5.1, Mythos 5/5.1). It replaces the single `computer` tool with
17 member tools, adds native batch actions, and rejects `name`,
`display_width_px`, `display_height_px`, `display_number`, and `enable_zoom`.

The question to answer before writing any code: does native batching work in
tandem with Juno's existing batching, conflict with it, or supersede it? Produce
a written recommendation with evidence.

Note for the model switcher work: Haiku 4.5 does NOT support computer use in any
tool version. There is no cheap fast Anthropic tier for this.

## Fix A - Screenshot double encode

Branch: `perf/screenshot-single-encode`. Files: `src-tauri/src/commands/core.rs`,
`src-tauri/mcp-server-os-level/src/platforms/macos/utils.rs`.

`capture_and_encode_screenshot()` captures, PNG-encodes, base64-encodes.
`capture_screenshot_command` then immediately base64-decodes, PNG-decodes,
resizes, JPEG-encodes, base64-encodes. Measured waste on a 1920x1080 grab:
PNG encode ~99ms + base64 ~5ms + decode ~5ms + PNG decode ~74ms = ~180ms per
screenshot producing an intermediate that is thrown away.

`capture_display_buffer()` (utils.rs:669) already returns the raw buffer.
Add a sibling that hands it back without encoding, and have
`capture_screenshot_command` resize and JPEG it directly.

Hard constraints: keep `capture_and_encode_screenshot()` working for existing
callers; do not change the resize target, filter, JPEG quality, or any
coordinate/scaling call.

## Fix B - DESCOPED to documentation only

Branch: `perf/action-cooldown-dedup`. Comments and tests only, zero production
control-flow or constant changes, because the premise below turned out to be
wrong (see the correction section above). Files:
`src-tauri/src/agent/tools/anthropic_computer_use.rs`,
`src-tauri/src/agent/input_arbiter.rs`.

Two sleeps stack for `key`, `scroll`, `mouse_move`, `hold_key`, and drag: the
300ms global cooldown then up to 500ms more inside `InputArbiter::acquire()`.
Worst case 800ms before one keystroke. They measure from different reference
points, which is why they double-count instead of overlapping.

In the one-action-per-turn loop both are usually no-ops because the model round
trip already exceeds 800ms. They bite inside batches, which is exactly what
Phase 3b is trying to make fast.

Fix: let the InputArbiter's 500ms be authoritative for actions that take it, and
skip the redundant 300ms for those. Do not lower either number; the
"clicked too fast" failures they prevent are real.

## Outcome (2026-09-22)

Merged into `integration/perf-2026-09-22`, branched from `origin/main` (not the
local `main`, which was 8 commits behind and included the React 19 / TS 7 /
Vite 8 upgrade the branches had not seen):

- `perf/computer-toolset-investigation` — migration plan, no code
- `perf/action-cooldown-dedup` — comments and regression tests only
- `fix/batch-halt-on-failure` — batch halt scoped to UI-mutating actions,
  `hold_key` seconds vs ms, the `wait` validator, unit naming across six files
- `perf/cli-streaming-feedback` — live reasoning and tool intent, old-CLI
  fallback, stream-surface drop guard
- `perf/screenshot-single-encode` — one encode instead of three

Held back deliberately:

- `perf/anthropic-cache-breakpoints` — merges last and alone. The only branch
  whose failure mode is a hard 400 rather than a degradation, so the first live
  request is its test: log `usage.cache_creation` and check that
  `ephemeral_1h_input_tokens` is non-zero only on anchor-advance turns.
- `perf/cli-persistent-session` — parked as a fast follow, flag off, tracked as
  LAC-4001.

One merge conflict, in `anthropic_computer_use.rs`: two branches each appended a
`#[cfg(test)]` module to the end of the file. Both kept. Worth recording that
this repo uses diff3 conflict style, so a resolution must also remove the
`||||||| base` marker — grepping only for `<<<<<<<`, `=======` and `>>>>>>>`
leaves a file that looks clean and does not parse.

Bugs found that were live on `main`, none of which threw or logged:

1. Batch loop kept executing after a failed click, typing into whatever window
   had focus
2. `previous_char_count` never reset between assistant messages, eating the
   opening of every message after the first in a tool loop
3. `hold_key` read Anthropic's seconds as milliseconds on the direct API path
4. The `wait` validator permitted 8.3 hours while its comment said 30 seconds
   and its message said milliseconds; fractional values skipped it entirely
5. The screenshot path encoded a PNG and discarded it, ~180ms per capture
6. 5-minute cache writes that expire before they are read, which cost 1.25x for
   nothing
7. Haiku 4.5 advertised as supporting computer use, which it does not at any
   tool version

Latent, needing particular hardware or models: the `ULTRA_HD` visual-token
overflow (LAC-4000), the high-res tier including Opus 4.5/4.6, and the
`SystemTime` vs `Instant` mismatch in the action-cooldown floor.

# Claude CLI persistent session — spike

Status: spike complete, **recommendation GO** (behind an off-by-default flag), with one
hard prerequisite and one new risk class that must be accepted before this is ever
turned on for a real user.

Date: 2026-09-22. CLI under test: `claude 2.1.278` (`/Users/lacy/.local/bin/claude`,
a Mach-O arm64 binary at `~/.local/share/claude/versions/2.1.278`).

Everything below was measured against that binary. Nothing here is from memory or
from the SDK docs.

---

## Why

`ClaudeCliBrain::run_streaming` spawns a fresh `claude` subprocess per user query and
adds `--resume <session-id>` on every follow-up. Each spawn pays for Node startup,
settings cascade, plugin and skill discovery, CLAUDE.md discovery, MCP wiring, and a
transcript read.

Measured, same model (`haiku`), same machine, same working directory:

Four turns of the same trivial prompt in one conversation, `haiku`, same machine, same
working directory. "Overhead" is wall clock minus the CLI's own `duration_ms`, i.e. the
part that is pure process cost.

**Today — one process per query (`--session-id`, then `--resume`):**

| turn | wall | `duration_ms` | overhead |
|---|---|---|---|
| 1 (cold) | 5.76 s | 1848 ms | 3.91 s |
| 2 (`--resume`) | 4.84 s | 1778 ms | 3.06 s |
| 3 (`--resume`) | 2.86 s | 915 ms | 1.94 s |
| 4 (`--resume`) | 2.50 s | 866 ms | 1.64 s |

**Persistent — one process for the conversation:**

| turn | wall | `duration_ms` | overhead |
|---|---|---|---|
| 1 (cold) | 4.60 s | 1705 ms | 2.89 s |
| 2 | 1.30 s | 1197 ms | **0.11 s** |
| 4 | 0.92 s | 911 ms | **0.005 s** |
| after a tool interrupt | 1.27 s | 1268 ms | ~0.00 s |

Steady state: **~1.6–3.1 s of overhead per follow-up turn becomes ~0.** That is the
largest single latency item on this provider, and it is paid on every message.

---

## Q1 — What JSON does `--input-format stream-json` expect on stdin?

Verified empirically. One JSON object per line (NDJSON), stdin left open.

A user turn:

```json
{"type":"user","uuid":"9253225b-01e8-44c5-8e1c-b48e83f03970","message":{"role":"user","content":[{"type":"text","text":"Reply with exactly: D_DONE"}]}}
```

- `type` and `message` are required; `message` is an Anthropic Messages-API user
  message (`role` plus a `content` array of blocks).
- **`uuid` is optional but Juno must always send it.** It is the only thing that makes
  a turn identifiable. See Q2a.
- No `initialize` control request is needed. The first message on stdin starts the
  first turn immediately.
- Messages sent back to back queue and run strictly in order — verified: `E1` then
  `E2` produced two results in order with no interleaving.
- `--input-format stream-json` requires `-p`. Pair it with
  `--output-format stream-json`.

Output frames observed, in the order they appear for a turn:

```
command_lifecycle  state=queued      command_uuid=<yours>
command_lifecycle  state=started     command_uuid=<yours>
system/init                          (re-emitted at the START OF EVERY TURN)
user                                 (the replay, with --replay-user-messages)
system/thinking_tokens               (0..n)
assistant                            (one per content block; cumulative per message)
rate_limit_event                     (occasionally)
result              subtype=success terminal_reason=completed result_index=N
command_lifecycle  state=completed   command_uuid=<yours>
```

Two things a one-shot parser gets away with and a persistent one does not:

1. **`system/init` is per turn, not per process.** The CLI's own doc string: *"Session
   metadata the CLI emits at the start of each turn, normally ahead of every other
   message of that turn."* Confirmed: one `init` frame in turn 1 and one in turn 2.
   Do not treat `init` as "a new session started".
2. **`assistant` frames carry the cumulative text of the current message**, so the
   `previous_char_count` delta cursor in `process_stream` must be reset at the start of
   every turn, not per process.

The first useful spawn flag set:

```
-p --input-format stream-json --output-format stream-json
--model <m> --strict-mcp-config --dangerously-skip-permissions
--session-id <uuid> --replay-user-messages
[--mcp-config <path> --append-system-prompt <guidance>]
```

---

## Q2 — Can a running turn be cancelled without killing the process?

**Yes. Cleanly. This is the blocker and it is not a blocker.**

The init frame advertises it:

```json
"capabilities":["interrupt_receipt_v1","interrupt_cancel_queued_v1","msg_lifecycle_v1"]
```

Send on stdin:

```json
{"type":"control_request","request_id":"juno_int_9e0aa8f9","request":{"subtype":"interrupt"}}
```

Get back, on stdout:

```json
{"type":"control_response","response":{"subtype":"success","request_id":"juno_int_9e0aa8f9","response":{"still_queued":[]}}}
```

then the running turn ends with:

```json
{"type":"result","subtype":"error_during_execution","terminal_reason":"aborted_streaming","is_error":true,"result":null,...}
```

Measured behaviour:

| case | result |
|---|---|
| interrupt mid text generation | `control_response` success in **0.035 s**, turn ends `aborted_streaming`, process alive (`poll()==None`) |
| interrupt mid **tool call** (a `Bash` `sleep 45`) | `control_response` success in **0.01 s**, a synthetic `user` frame `"[Request interrupted by user]"` is emitted, turn ends `aborted_streaming`, process alive |
| next turn after an interrupt | works, **1.27 s** wall, normal `success` result |
| interrupt while **idle** (no turn running) | `success` with `still_queued: []` — a harmless no-op |

`still_queued` lists messages that were queued behind the interrupted turn and were
**not** cancelled (the `interrupt_cancel_queued_v1` capability). Juno sends one message
at a time, so this stays empty, but the field must be read rather than assumed.

This replaces `child.kill()` on the escape path (LAC-3697) with a message. The
conversation survives the cancel, which is strictly better than today: today an escape
destroys the process and the next message pays a fresh boot plus a `--resume`.

Other control requests present in the binary and usable later if wanted:
`set_permission_mode`, `set_model`, `set_max_thinking_tokens`, `mcp_status`,
`initialize`, `can_use_tool`, `hook_callback`, `mcp_message`. `control_cancel_request`
also exists, for cancelling a long-running *client-originated* control request.

### Q2a — The real hazard: turns Juno never asked for

This is the finding that decides the design, and it does not exist in the one-shot
model at all.

A persistent process **starts turns on its own.** Reproduced: a `Bash` tool call with
`run_in_background: true` was left running; the turn was interrupted; the process then
sat idle; ~25 s later, with nothing on stdin, it emitted

```
system/background_tasks_changed
system/task_updated
system/task_notification
system/init
assistant (thinking)
assistant (text)
result subtype=success result_index=2 result="Background task completed—LATE_TASK has finished."
```

An unsolicited turn. It spent tokens, ran the model, and produced text no user asked
for. A naive reader that waits for "the next `result` frame" will pick that up and
render it as the answer to the user's next message. In an earlier run that is exactly
what happened: the reply to "Reply with exactly: ALIVE6" was a stale
`"The bash command completed. Output: DONE_SLEEP"`, returned in 0.00 s.

**The fix is exact and cheap.** Set `uuid` on the outgoing user message. The CLI
echoes it as `command_uuid` on three `command_lifecycle` frames:

```json
{"type":"command_lifecycle","command_uuid":"9253225b-...","state":"queued",...}
{"type":"command_lifecycle","command_uuid":"9253225b-...","state":"started",...}
{"type":"command_lifecycle","command_uuid":"9253225b-...","state":"completed",...}
```

The unsolicited turn in section C above produced **no `command_lifecycle` frames at
all**. So the rule is unambiguous:

> A turn belongs to Juno if and only if a `command_lifecycle` frame carrying Juno's own
> `command_uuid` opened it. Everything outside such a window is dropped.

Verified: omit `uuid` on the user message and you get no lifecycle frames, so this is
opt-in and Juno must opt in. `result.result_index` is a monotonic counter across the
whole process life (it counted the unsolicited turn as index 2) and is a useful
secondary sanity check, not a substitute.

**This is the hard prerequisite.** A persistent session implemented without lifecycle
correlation will silently show users answers to questions they did not ask. Do not
ship one.

The residual risk stays even with correct correlation: the process can still *run* a
turn nobody asked for, spending tokens and — with `--dangerously-skip-permissions`,
which this provider passes — potentially **taking desktop actions through Juno's own
MCP `computer` tool** with no user present. Correlation makes it invisible, not
impossible. Mitigations, in order of how much they cost:

- Keep the flag off by default (already required).
- Reap the process on idle (see Q4), which bounds the window.
- Consider `--tools` to drop `Bash` background execution on this path, or refuse to
  keep a session whose last turn left background tasks running.

---

## Q3 — `--resume` and session identity with a persistent process

`--session-id` is the right mechanism for a **new** session, and `--resume` for an
existing one. Getting that wrong is a hard failure, not a fallback:

```
$ claude -p --session-id <id-that-already-exists> "..."
Error: Session ID <id> is already in use.
```

So the rule is:

- **No CLI session for this conversation yet** → spawn with `--session-id <fresh uuid>`.
- **One already exists** (the one-shot path made it, or a previous persistent process
  did) → spawn with `--resume <that id>`. Verified: `--resume` works with
  `--input-format stream-json`, the session id stays pinned (no fork), prior context
  carries through, and `command_lifecycle` frames behave identically.
- Once pinned, every frame of every turn in that process reported the same
  `session_id`, matching what was passed in.
- **The transcript is still written to disk.** After the persistent process was killed
  with SIGTERM, a plain one-shot `claude -p --resume <that uuid>` returned rc=0 and
  resumed into the same session, with the prior turns' context intact — including the
  turns that ran inside the persistent process.

That last point is the safety property that makes this whole change defensible:

> A persistent session degrades to today's one-shot `--resume` path with **zero context
> loss**. If the process dies, is reaped, or the flag is turned off mid-conversation,
> the existing code path picks the conversation up exactly where it left off.

So the design is: Juno reuses the existing `CLI_SESSIONS` registry (which already maps
conversation id → CLI session id and is already consumed by `build_args`). A hit means
`--resume`; a miss means a fresh UUID and `--session-id`. Nothing else about the
existing resume logic changes.

Note: `--no-session-persistence` must **not** be passed on this path. It would remove
the fallback.

### The corollary that makes the fallback safe

Because a message written to a live process's stdin is *already queued inside it*, a
fallback to the one-shot path is only safe if that process cannot still run it. If it
did, the user would get two answers to one question, and be billed twice.

So: **whenever the persistent path does not carry a turn to completion, the process is
killed.** The message dies with it, and only then does the one-shot path take over.
That is the rule `run_turn` enforces on every non-`Completed` exit.

(Attempting to `--resume` a session a live process still holds is also refused by the
CLI — `session_held_by_background` is one of its documented startup-refusal reasons —
so this is not only about double-billing.)

---

## Q4 — Death, restart, suspend, orphans

Measured:

| event | behaviour |
|---|---|
| **SIGSTOP 6 s then SIGCONT** (app suspend) | survived. Next turn: 1.07 s wall, normal success. Frames buffered in the pipe and were read after resume. |
| **SIGTERM** | exits with 143. Clean. |
| **stdin closed** | exits 0 after 0.55 s. This is the graceful shutdown. |
| **75 s idle** | process still alive (`poll()==None`). No CLI-side idle timeout observed at that scale. Longer idles (hours, laptop sleep) were **not** tested. |
| **CLI crash / abnormal exit** | not directly induced. The reader task sees stdout EOF, which is the signal to evict the session. |

Handling required in Juno:

1. **Health check before every turn.** `child.try_wait()`; a process that has exited is
   evicted from the registry and the turn falls back to the one-shot path (which
   `--resume`s the same session id — no context lost).
2. **EOF on stdout means dead.** The reader task must mark the session dead on EOF, not
   just end quietly, or the next turn will block on a channel nobody will ever feed.
3. **Idle reaper.** A session with no turn for N minutes is killed. This bounds both the
   leak and the unsolicited-turn window of Q2a. Suggested N = 10 minutes.
4. **Bounded registry.** Cap concurrent sessions; evict (and kill) the least recently
   used past the cap. `MAX_TRACKED_SESSIONS = 64` is fine for *session ids*, but live
   *processes* need a far smaller cap — each one is a Node process holding an MCP
   connection. Suggested cap = 3.
5. **Orphans.** `kill_on_drop(true)` on the `Child` covers a drop on a live runtime. It
   does **not** cover Juno being SIGKILLed. On a hard kill of Juno the `claude` children
   are reparented to launchd and survive. They will exit on their own once their stdin
   pipe breaks — the write end dies with Juno — but this was not verified and should be
   before enabling by default. An explicit `shutdown_all()` from Juno's exit handler is
   required regardless.
6. Timeouts. The one-shot path's 300 s `CLI_TIMEOUT` must become a **per-turn** timeout,
   not a process timeout. A turn that times out should be interrupted (Q2), not have
   its process killed.

Not verified without running Juno itself: behaviour across a real macOS App Nap /
laptop-sleep cycle, and whether the CLI's own websocket-ish background machinery
(`messaging_socket_path` appears in the init frame) misbehaves after a long suspend.

---

## Q5 — One process per conversation, or one global process?

**One process per conversation.** Not close.

- The CLI session *is* the conversation. One process multiplexing several conversations
  would mean one transcript, so every conversation would see every other one's history.
  There is no per-message session routing in the stdin protocol.
- Messages queue and run strictly in order in one process (verified with E1/E2). A
  global process would serialize all of Juno's conversations behind each other, which
  is a direct regression against LAC-1432 parallel sessions.
- Interrupt is process-wide. `{"subtype":"interrupt"}` has no `command_uuid` argument,
  so a global process could not cancel one conversation without cancelling whichever
  turn was actually running. Escape would cancel the wrong chat.
- Cost of the choice: N Node processes instead of one. That is what the idle reaper and
  the concurrency cap in Q4 are for.

---

## Q6 — Does a persistent process break Juno's in-process MCP server?

**No.** This turned out to be the easiest question, because of how `juno_mcp` is
already built.

`juno_mcp::ensure_running` stores its `Endpoint { url, token }` in a
`static ENDPOINT: OnceLock<Endpoint>`. The token is minted once per **app run**, not
per query, and the URL is a loopback port bound once and served for the life of the
process. `write_mcp_config` writes to a path keyed on `std::process::id()`. So:

- The config file a persistent process was spawned with stays valid for as long as Juno
  is running, which is by construction at least as long as the child.
- The bearer token never rotates under the child.
- Juno's MCP server is **streamable HTTP**, not a spawned stdio server, so there is no
  per-child server process to keep alive and nothing to reconnect.

Empirically, with a stdio MCP server (the harder case — a spawned child):

- `system/init` reported `mcp_servers:[{"name":"spike","status":"connected","source":"dynamic"}]`
  on **every** turn.
- The MCP server process was spawned **once** (pid 97631) and the same pid served a
  `tools/call` nine seconds later on turn 2, and stayed connected through the interrupt,
  the SIGSTOP/SIGCONT, and the 75 s idle.

So a persistent process is strictly *better* for MCP than the one-shot path: today every
query re-handshakes the MCP server (`initialize`, `notifications/initialized`,
`tools/list`) before it can do anything.

One thing that does change: `--mcp-config`, `--append-system-prompt`, `--model` and
`--system-prompt` are **spawn-time** arguments. A persistent process keeps whatever it
was born with. If the user changes model or prompt mid-conversation, the live process is
stale. The control protocol has `set_model` and a system-prompt slot, but the simple and
obviously-correct answer is to record a spawn signature (model + system prompt +
mcp config path) on the session and tear down and respawn when it changes. Respawning
costs one cold boot and loses nothing, because of Q3.

---

## Recommendation

**GO**, with these conditions:

1. Ship behind a store-persisted flag, **default off**. Non-negotiable while the demo
   is live.
2. **Lifecycle correlation is mandatory, not an optimisation.** Send `uuid` on every
   user message; accept only frames inside a `command_lifecycle` window carrying that
   `command_uuid`; drop everything else. Without this the feature shows users answers
   to questions they did not ask.
3. Escape cancels with a `control_request` interrupt, then waits for the aborted
   `result`. Only if that does not arrive within a short bound (say 10 s) does it fall
   back to killing the process.
4. Never pass `--no-session-persistence`. The on-disk transcript is the fallback.
5. Keep the one-shot path as the default and as the failure path. Any setup failure —
   binary missing, spawn error, dead process, changed spawn signature — falls back to
   it, and because the session id is shared, nothing is lost.
6. Idle reaper + a small concurrency cap + an explicit shutdown on app exit.

The one thing that would turn this into a NO-GO is if condition 2 could not be met.
It can be met, exactly, with a documented CLI capability (`msg_lifecycle_v1`) that the
init frame advertises. That is why this is a GO.

---

## What was built on this branch

All of it additive. The one-shot path is untouched except for one early branch and one
visibility keyword, deliberately away from `build_args` and the NDJSON parse loop that
another agent is rewriting.

| file | change |
|---|---|
| `src-tauri/src/agent/providers/claude_cli_session.rs` | new. The whole persistent lifecycle. |
| `src-tauri/src/agent/providers/claude_cli.rs` | a ~50-line early branch in `run_streaming`, between the MCP-config resolution and `build_args`; and `extract_text_from_message` made `pub(super)`. |
| `src-tauri/src/agent/providers/mod.rs` | one `pub mod` line. |
| `src-tauri/src/constants/settings.rs` | one store key. |
| `src-tauri/src/lib.rs` | two command registrations. |

Shape of the module:

- **One process per conversation**, in a `OnceLock<TokioMutex<HashMap<..>>>` registry.
- **Exactly one async lock per session**: the inbox. Locking it both serializes turns
  and hands the turn its frames, so there is no second lock and no ordering to get
  wrong. A writer task owns stdin behind a channel, so sending never needs a lock.
  The `Child` sits behind a `std` mutex, touched only by non-blocking `try_wait` and
  `start_kill`, never across an await.
- **Correlation** by `command_uuid` on `command_lifecycle`, as condition 2 requires.
  Nothing reaches the UI before the `started` frame for Juno's own uuid, which is also
  what makes a fallback before that point invisible.
- **Cancel** sends the interrupt and waits out a 10 s grace for the aborted result;
  only if that never comes is the process killed.
- **Three deadlines**: 45 s for the lifecycle ack, 300 s for the turn (interrupted,
  not killed), 10 s for the interrupt grace.
- **Reaper** self-starts on first use, kills sessions idle over 10 minutes; at most 3
  live processes, LRU evicted.
- **Flag**: `cli_persistent_session_enabled` in the settings store, default false, with
  `get_/set_cli_persistent_session_enabled` commands. Turning it off kills every live
  process at once.

Commands to flip it, until there is UI:

```js
await invoke('set_cli_persistent_session_enabled', { enabled: true })
await invoke('get_cli_persistent_session_enabled')
```

### Still unverified (needs a build, or needs the real app)

Everything below was out of reach in this spike because compiling was not allowed.

- **The Rust module in this branch has never been compiled.** `cargo check` and
  `cargo clippy` are required before any of it is trusted. `rustfmt --check` parses it
  cleanly, which proves the syntax and nothing more — not types, not borrows, not
  trait bounds.
- Real macOS suspend / laptop sleep across a persistent child. SIGSTOP/SIGCONT for 6 s
  was fine; App Nap and a closed lid are a different thing.
- Orphan behaviour when Juno itself is SIGKILLed. `kill_on_drop` does not cover it, and
  `shutdown_all()` exists but is **not yet wired into an app exit handler** — that is a
  deliberate gap, since wiring it means touching app setup, and the feature is off.
- Interaction with LAC-1432 parallel sessions under real concurrent load.
- Whether Juno's cursor overlay and AX verification behave across a turn that was
  interrupted rather than killed. On the one-shot path the process dies and everything
  it held dies with it; here it does not.
- Token cost of unsolicited turns in practice.
- The `--mcp-config` path is keyed on Juno's pid and written per query by the one-shot
  path. A persistent process holds that path open across turns; it is stable for the
  app's life, but nothing was tested against a rewrite of that file mid-session.
- Two turns racing to open the *same cold conversation* would both spawn with the same
  fresh `--session-id`, and the loser's process would die with "already in use". The
  registry recheck kills the surplus process either way, and a loser that never
  acknowledged falls back cleanly, so both outcomes are safe — but turns within one
  conversation are sequential today, so this path is untested because it should not
  occur.

---

## Reproduction

The spike scripts are not checked in. They were three Python drivers against the real
binary:

1. persistent process, 5 turns, interrupt mid-generation, idle interrupt, clean shutdown;
2. `--session-id` pinning, a stdio MCP server across turns, interrupt mid-tool-call,
   SIGSTOP/SIGCONT, 75 s idle, SIGTERM, then a one-shot `--resume` of the dead session;
3. turn correlation: `command_lifecycle` frames, an unsolicited background-task turn,
   and back-to-back message queueing.

Each one is a `subprocess.Popen` with `stdin=PIPE` kept open, a reader thread parsing
NDJSON off stdout, and `json.dumps(msg) + "\n"` written to stdin per turn.

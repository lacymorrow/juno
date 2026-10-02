# Ambient awareness: a turn that something other than a person can start

**Status:** Plan only. Nothing built.
**Read `## What already exists` before writing any code.** Most of a wake-up path is already in the tree and already wired. The work is mostly replacing a dishonest signal with an honest one, not building a subsystem.
**DRI:** unassigned.

## The one-line problem

A Juno turn can only begin when a person speaks or types, so any task whose next step depends on the world changing has to be restarted by hand every time the world changes.

## The ask

Lacy, 2026-10-01:

> "The agent should get events/notifications or just be aware of what the user does. So if, for example, we decide to play a game of chess, the agent should continue watching the game and continue working and moving."

Chess is the test case and it is the whole specification. Two people are playing chess in a browser. Nobody should have to say "your turn" after every move. Juno notices the board changed, sees it is its move, moves, and goes back to watching. The turn boundary becomes an event in the world instead of a keystroke from the person. Waiting for a build, waiting for mail, watching a download, noticing the person switched apps and the task no longer makes sense: all the same machinery.

## How a turn begins and ends today

### It begins in exactly one function

Every source funnels into `anthropic::submit_query` (`src-tauri/src/anthropic.rs:343`):

| Source | Call site |
|---|---|
| Typed or spoken, through the bar and panes | `src-tauri/src/integration.rs:151`, `:215`, `:659` |
| Cloud (phone, web) | `src-tauri/src/cloud/commands.rs:890` |
| Headless CLI | `src-tauri/src/cli/headless.rs:1375` |
| Cron automations | `src-tauri/src/scheduler.rs:345` |
| Orchestrated query with the orchestrator off | `src-tauri/src/commands/orchestrator.rs:171` |
| **A timer or a monitor firing** | `src-tauri/src/events/timer_handlers.rs:421` |

`submit_query` rate-limits, trims, tries to resolve a pending send approval, announces the query to every surface, tries local intents, plans a route, then hands the query to `AgentExecutionQueue` (`anthropic.rs:457`, queue at `:173`), which serializes runs so only one agent runs at a time.

The run then: resets the cancel channel and registers the escape user `agent_execution` (`anthropic.rs:692`, `:701`), flips the execution flag and the bar through `handle_agent_execution_state_transition(active = true)` and `handle_agent_started` (`state_management.rs:507`, `commands/ui_commands.rs:661`), takes a session row via `begin_session_run` (`agents/session.rs:491`), and calls `DefaultAgentRunner::run` (`anthropic.rs:1115`).

### It ends because the model said so, and for no other reason

`DefaultAgentRunner::run` is one `loop` (`src-tauri/src/agent/implementations/agent_runner.rs:1259`) that calls `step()` and matches the action. Exactly one arm ends the turn normally: `AgentAction::Finish(text)` (`:1371`) transitions to `AgentState::Finished` and returns `Ok`. The loop keeps going on `Think`, `RespondToUser` and `ExecuteTool`. The only other exits are an error, the step cap (which asks the person for more steps, `:1270`), and the cancel receiver going true (`:1259`).

So nothing in the runner decides there is more to do. End of turn is the model emitting an end-of-turn. After the return, `anthropic.rs:1470` unregisters the escape user, `:1671` calls `handle_agent_stopped`, which puts the bar back to `Default` (`ui_commands.rs:671`), and the `SessionHandle` drop (`agents/session.rs:528`) removes the session row.

### Where a wake-up source attaches

`submit_query` is already the one door, and the timer path already walks through it. A watch uses the same door. **Nothing in `agent_runner.rs` changes for this feature.** The runner never needs to know a watch exists.

### `agent/` and `agents/` are two different things

`src-tauri/src/agent/` is the machinery of one run: core types, the runner, memory, brains (`providers/`), tools, prompts, routing. `src-tauri/src/agents/` is the fleet: specialist agent types (`desktop_agent.rs`, `browser_agent.rs`, `system_agent.rs`), the factory, the task orchestrator (`orchestrator.rs`, reached from `commands/orchestrator.rs` only when `use_orchestrator` is true), and the parallel-session registry (`session.rs`). This feature lives in `agent/tools/` plus `agents/session.rs`. The orchestrator is not involved and must not become involved.

## What already exists

### A wake-up path, built, wired, and wrong

Juno already ships a screen monitor and a file monitor whose stated purpose is this feature. `src-tauri/src/events/timer_handlers.rs:1-10` says so in its own module doc: "Supports use cases like chess games, page monitoring, and user interruption recovery."

The chain, end to end:

1. Tools `set_screen_monitor` and `set_file_monitor` are defined at `agent/tools/timer_tools.rs:476` and `:722`, executed at `:540` and `:778`, and registered on every provider instance at `agent/providers/factory.rs:612`.
2. Each spawns a `tauri::async_runtime::spawn` poll loop (`timer_tools.rs:610`, `:863`) and on a detected change emits `timer-expired` with the whole `TimerTask` (`:663`, `:934`), then breaks.
3. `events/handlers.rs:154` listens for `timer-expired` (registered from `lib.rs:1134`) and hands the payload to `TimerEventHandler::handle_timer_expired` (`events/timer_handlers.rs:217`).
4. That validates size, age and a pattern blocklist, reads agent state (`:367`), picks a strategy (`:382`), and calls `anthropic::submit_query` (`:421`) with `context["query"]`, falling back to `description`.

This is a working wake. What is wrong with it is specific, and listing it is most of the design:

1. **The change test is a string compare of two base64 PNGs** (`timer_tools.rs:650`). Any pixel anywhere changes it: a clock, the cursor, a blinking caret, video. On a full-screen capture it fires on the first tick, every time.
2. **`threshold` is accepted and never read** (`:561`). **`region` is accepted, stored in the `TimerTask`, and never used** (`:554`): the loop always captures the whole screen.
3. **It is one-shot.** The loop `break`s on first detection. A chess game needs to watch again after each move, and nothing re-arms it, so every move costs the model another tool call just to resume watching.
4. **There is no stop condition by default.** `max_duration_seconds` is optional and its absence sets `trigger_time` to `u64::MAX` (`:576`). An armed monitor captures the full screen every two seconds forever.
5. **Escape does not touch it.** `stop_coordinator::perform_coordinated_cleanup` (`commands/stop_coordinator.rs:161`) stops TTS, the focused agent session, dictation, and the input monitors. It never mentions `TimerManager`. Neither does `cleanup::cleanup_application` (`cleanup.rs:49`). A monitor survives Escape, survives the browser being closed, and survives everything short of quit.
6. **A screen-monitor wake that lands mid-run fires the global cancel.** `interrupt_and_restart` calls `app_state.signal_cancel()` (`timer_handlers.rs:471`), which kills every session. The rest of the app moved to per-session cancel (`agents/session.rs:208`, `stop_coordinator.rs:210`).
7. **Nothing shows a monitor is armed.** `list_timers` is a model tool, not a surface. There is no indication and no stop control.

### Things worth reusing rather than reinventing

- **The conversation survives a re-submission.** `AdvancedMemoryManager` (`agent/implementations/memory_manager.rs:151`) holds every field behind `Arc<RwLock<..>>`, and the runner is handed a clone (`anthropic.rs:1088`), so messages a woken turn adds land in the same log the previous turn used. A wake continues the game instead of starting a new conversation. This is the single most important existing fact and it is why this feature is small.
- **Runs are already serialized.** `AgentExecutionQueue` (`anthropic.rs:173`) means a wake that lands mid-run waits rather than races.
- **The session registry is the UI.** `agents/session.rs` gives per-row identity colour, status, current action, focus, per-session cancel, and a broadcast on every mutation. `src/hooks/useAgentSessions.ts` mirrors it and `src/components/AgentSessionRows.tsx` plus `src/components/AgentRosterStrip.tsx` render it. Commands `list_agent_sessions`, `focus_agent_session`, `cancel_agent_session` exist (`commands/agent_sessions.rs:60`, `:83`, `:121`). The cap is 12 (`state.rs:509`).
- **`platform/stop_key_monitor.rs` is the precedent for watching the person.** Two NSEvent monitors, the global one gated on Accessibility already being granted (because adding it untrusted raises Apple's own alert and delivers nothing anyway), handlers return the event untouched, AppKit calls made on the main thread via `run_on_main_thread`, and the monitor installed only while something needs it and removed when idle, ref-counted by a pure ledger in `commands/escape_key_coordinator.rs:43`. Any new observer of the person copies this shape, including the ledger.
- **`agent/app_observation.rs` is the precedent for not using screenshots.** Its module doc argues the case: one `osascript` to System Events is about 0.8s and a screenshot is the most expensive thing in a turn, so the cheap check runs after a launch and nowhere else. `agent_runner.rs:130` caps a liveness observation at 2s of staleness so a burst of actions pays for one check.
- **No notification observer exists for app or window state.** Every app question today is a poll (`utils/mod.rs:1119`, `commands/app_url.rs:22`). There is no `NSWorkspace` notification observer and no installed `AXObserver`; the private `_AXObserverAddNotificationAndCheckRemote` probe in `mcp-server-os-level/src/platforms/macos/interaction.rs:2111` classifies elements and does not receive notifications. Treat both as new code.
- **The browser is CDP already.** `chromiumoxide = "0.9"` (`src-tauri/Cargo.toml:72`). `BrowserController` (`agent/tools/browser_controller.rs:49`) holds a `Page` behind a mutex, pumps the CDP handler on a spawned task (`:37`), and lives lazily in `AppState` (`state.rs:366`, `get_or_init_browser_controller` at `state.rs:1257`). `extract_content` (`:1128`) already evaluates a JS function and JSON-decodes the result through a private `eval_json` helper. `chromiumoxide::page::Page::event_listener::<T>()` exists in 0.9.1 for real CDP event subscription.

## Signal sources, ranked

The ranking is by cost per unit of truth. The decisive number is not the cost of a capture, it is that **a model call is three to four orders of magnitude more expensive than a poll**, so the rule the design rests on is: a tick must never cost a model call.

| Rank | Signal | Permission | Latency | Battery and dollars | What it cannot see |
|---|---|---|---|---|---|
| 1 | CDP `evaluate` of a small JS expression on a page Juno already drives | None beyond what the browser already granted | One poll interval, 250ms to 2s by choice | Sub-millisecond of CPU per tick, zero tokens | Anything outside that tab: native apps, other tabs, canvas pixels it cannot read from the DOM, cross-origin frames |
| 2 | CDP events (`Page.frameNavigated`, `Page.lifecycleEvent`, `Network.webSocketFrameReceived`, `DOM.*`) | Same | Push, tens of milliseconds | Near zero when the event is rare, unbounded when it is not (a websocket chess server sends frames constantly) | Same as above, and DOM mutation events only fire for nodes the client has already requested, so they are not a drop-in for a MutationObserver |
| 3 | File mtime, size and existence poll | None | One poll interval | Three `stat` calls per tick | Anything not expressed as a file. Already built, see `timer_tools.rs:778` |
| 4 | `osascript` to System Events for app presence and window counts | Accessibility (already requested) for window counts, none for presence | Roughly 0.8s per call, nearly all process startup | Too expensive to poll faster than every few seconds | Window contents. Measured and documented in `agent/app_observation.rs` |
| 5 | `NSWorkspace` activation and window notifications | Accessibility for window-level detail | Push, immediate | Effectively free: the notification is delivered, nothing is polled | What is inside a window. New code, no precedent in the tree |
| 6 | Passive `NSEvent` monitors for the person's own input | Accessibility, and only add the global monitor once granted | Push, immediate | Effectively free | Intent. It sees keystrokes and clicks, not what they mean. Precedent: `platform/stop_key_monitor.rs` |
| 7 | Screen capture plus a model call per tick | Screen Recording (TCC), which Juno does not currently require | Seconds | The expensive one. Every tick is a full request carrying the whole conversation plus roughly 1,500 image tokens for a scaled full-screen shot. The repo has already measured the capture pipeline itself at about 180ms of pure waste per shot before the fix in `docs/plans/performance-2026-09.md:150`, and `limit_screenshot_history` rewriting an older message every turn breaks the prefix cache (`performance-2026-09.md:25`) | Nothing, which is exactly the problem: it sees everything, including the person's banking tab |

**Recommendation.** Rank 1 for everything the browser can answer. Rank 3 for files. Ranks 5 and 6 as a second slice, for "the person switched apps, this task is stale". Rank 7 only when a person explicitly asks Juno to watch something that is not in a browser and not a file, never as a default, and never on a timer without a hard wake budget.

**Rank 1 is not a compromise for the chess case, it is the better answer.** A board position read from the DOM is exact. The same position read from a screenshot is a model's guess at a picture of it, costs a request, and takes seconds.

## The design

### Where the watch lives and who owns it

Rust, entirely. A watch is a Rust task plus a row in the session registry. React displays the row and offers a stop button, exactly as it already does for background agent sessions.

New module: **`src-tauri/src/agent/tools/page_watch.rs`**. It owns the tool definition, the poll task, and a `PageWatchRegistry` that lives in `AppState`.

### The shape of a watch

```rust
struct PageWatch {
    id: String,                 // uuid
    description: String,        // what the model said it is watching, shown to the person
    expression: String,         // JS function body, returns a string or null: the digest
    until: Option<String>,      // JS function body, returns bool: stop watching
    poll: Duration,             // clamped 250ms..=30s
    baseline: String,           // last digest seen
    wakes_used: u32,
    max_wakes: u32,             // clamped 1..=200, default 20
    deadline: Instant,          // start + max_duration, default 30 min
    session: SessionHandle,     // identity, status, focus, cancel, and the UI row
}
```

The expression returns a **digest**, not a boolean. The model names what matters (`document.querySelectorAll('kwdb-move').length`, or the text of the last move), and Juno hands the new value to the woken turn. That is what makes the wake message factual: "the value you were watching changed from 23 to 24" rather than "something changed, go take a screenshot". For chess this is the whole feature.

### How it wakes a turn

Through `anthropic::submit_query`, the same door everything else uses. The woken turn's prompt is composed by Rust, not by the model:

```
[Juno is watching: <description>]
The value you were watching changed from <old> to <new>.
It is wake 3 of 20. Continue the task, then say whether to keep watching.
```

Because `AdvancedMemoryManager` is Arc-shared (`memory_manager.rs:151`) this lands in the same conversation, so the model has the whole game in context and does not need re-briefing.

The watch task stays alive across the wake. It re-baselines to the new digest before submitting, so a move made while the model is thinking is caught on the next tick instead of being lost or re-fired.

### How it terminates

Six ways, all required, none optional:

1. **`until` returns true.** The model's own stop condition, for example the game being over.
2. **`max_wakes` reached.** Hard cap, default 20. This is the spending answer: cost is bounded by the number of wakes, not by watch duration, and a tick costs nothing. On the last wake the message says so.
3. **`deadline` reached.** Default 30 minutes.
4. **The page is gone.** The `evaluate` call errors, the tab closed, or `location.origin` differs from the origin at arm time. Stop, do not re-arm, do not wake. A watch on a closed chess tab must die silently.
5. **Escape, and the session switcher.** The watch holds a session row, so `cancel_agent_session` already cancels it and `stop_coordinator::cancel_focused` (`stop_coordinator.rs:210`) reaches it. The loop selects on `session.cancel_receiver()`.
6. **App quit.** `RunEvent::Exit` (`lib.rs:1225`) aborts every watch task, next to the existing `claude_cli_session::shutdown_all()`.

Two more conditions that are easy to get wrong and must be written into the loop:

- **Sleep.** Use `MissedTickBehavior::Skip` on the interval so eight hours of sleep does not deliver 28,800 catch-up ticks in one burst. Separately, if the wall-clock gap since the last tick exceeds ten poll intervals, treat it as a suspension: silently re-baseline and do not wake. Waking after lunch to "the board changed" for a game that ended an hour ago is worse than missing it.
- **Escape priority.** A watch created before a run would hold registry focus (`agents/session.rs` auto-focuses the first session created), so Escape during a woken turn would cancel the watch instead of the turn. Fix it in the registry, not at the call site: add `SessionKind { Run, Watch }`, and make focus prefer a `Run`. Escape then stops the turn; Escape again, with nothing running, stops the watch. That is the same two-level behaviour Escape already has in `events/shortcuts.rs:164`, where Escape with nothing to stop dismisses the pane.

### Files and the exact change in each

1. **NEW `src-tauri/src/agent/tools/page_watch.rs`.** The `watch_page` and `stop_watching` tool definitions and executors, the `PageWatchRegistry` (`Arc<TokioMutex<HashMap<String, Arc<PageWatch>>>>` plus the task handles), the poll loop, the clamping helpers, and the wake-message builder. Clamping copies `timer_tools::schedule_seconds` (`timer_tools.rs:305`), which already treats every model-supplied number as adversarial. Reuse `BrowserController` through `state.get_or_init_browser_controller()` (`state.rs:1257`) and never hold the browser lock across an await that could re-enter it (CLAUDE.md deadlock rule).
2. **`src-tauri/src/agent/tools/mod.rs`** add `pub mod page_watch;` and re-export the registry type, beside the `timer_tools` line at `:71`.
3. **`src-tauri/src/agent/providers/factory.rs`** register the two tools at `:612`, next to `register_timer_tools`.
4. **`src-tauri/src/constants/agent.rs`** add `WATCH_PAGE` and `STOP_WATCHING` beside `SET_SCREEN_MONITOR` at `:113`.
5. **`src-tauri/src/agent/tools/tool_mapping.rs`** map both to `ToolCategory::Browser`, not `Timer`, beside `:156`.
6. **`src-tauri/src/agent/tool_logger.rs`** give the category an icon and verb so a running watch reads correctly in the chat tool row, as `ToolCategory::Timer` does at `:966`.
7. **`src-tauri/src/state.rs`** add `page_watches: Arc<PageWatchRegistry>` next to `browser_controller` at `:366`, construct it at `:450`.
8. **`src-tauri/src/agents/session.rs`** add `AgentSessionStatus::Watching` at `:101` with `is_terminal() == false`; add `SessionKind` and store it on `AgentSession`; make `create` prefer a `Run` for focus; add `begin_watch_session`.
9. **`src-tauri/src/commands/ui_commands.rs`** add `BarState::Watching` at `:65` and its `as_str` arm at `:86`; add an `armed_watches: usize` field to `UIManager` at `:175`; in `handle_agent_stopped` (`:671`) go to `Watching` instead of `Default` when `armed_watches > 0`; add a setter the registry calls on arm and disarm.
10. **`src-tauri/src/constants/ui.rs`** add `WATCHING: &str = "watching"` to `bar_states` at `:67`.
11. **`src-tauri/src/lib.rs`** register the two new commands (`list_page_watches`, `stop_page_watch`) in the handler list near `:343`, and abort watch tasks in the `RunEvent::Exit` arm at `:1225`.
12. **`src-tauri/src/cleanup.rs`** stop every watch in `cleanup_application` at `:49`, before the browser controller is torn down. A watch outliving its browser is the bug this line prevents.
13. **Run `bun run generate-constants`** so `src/lib/constants.generated.ts` picks up the new bar state and command names. Do not hand-edit that file.
14. **`src/hooks/useAgentSessions.ts`** add `"watching"` to the `AgentSessionStatus` union at `:7`.
15. **`src/components/AgentSessionRows.tsx`** add `watching: "Watching"` to `STATUS_LABELS` at `:11` and an `Eye` icon case in `StatusIcon` at `:21`.
16. **`src/components/bar/bar-state-mapper.ts`** add a `BAR_STATES_WATCHING` case to `getStatusLabel` returning "Watching", and to each appearance mapper so the new state is not silently rendered as idle.
17. **`src/components/bar/island/IslandBar.tsx` and the other appearances**: `watching` maps to the status posture. The island spec table (`docs/plans/island-appearance.md:73`) gains one row.

### What is deliberately not changed

`agent_runner.rs`, `agents/orchestrator.rs`, `commands/orchestrator.rs`, and the brains. A watch is outside a turn, and a turn that does not know it was woken by a watch is the correct design.

## The smallest complete slice worth shipping on its own

**One watched page, one JS digest expression, re-arming, bounded, visible, stoppable.** Items 1 through 16 above. No screen capture, no app observer, no input observer, no second watch.

It is the right slice for five reasons:

1. It makes the chess example literally work, which is the test Lacy named.
2. A tick costs no model call and no tokens, so the spending question has an answer that is not a prompt.
3. It has natural termination that does not depend on the person remembering: the tab closes and the watch dies.
4. It needs no new TCC permission. Screen Recording is not requested and Accessibility is already granted.
5. Every piece it leans on already exists and is already rendered: the session registry, the roster strip, the rows, the queue, the shared memory, the browser controller.

### Specified tightly enough to implement without asking

`watch_page` input schema:

| Field | Type | Required | Default | Clamp |
|---|---|---|---|---|
| `description` | string | yes | | 200 chars, shown to the person verbatim |
| `expression` | string | yes | | A JS function body returning a string, a number or null. Evaluated as `function() { <expression> }` through the same path `extract_content` uses (`browser_controller.rs:1200`). Result is JSON-stringified and compared as a string |
| `until` | string | no | none | Same shape, must return a boolean |
| `poll_ms` | number | no | 1000 | 250 to 30000 |
| `max_wakes` | number | no | 20 | 1 to 200 |
| `max_duration_seconds` | number | no | 1800 | 10 to 86400 |

Returns at once, without waiting for a change: `{ "watch_id": "...", "baseline": "<digest now>", "message": "Watching: <description>. Will wake up to 20 times or for 30 minutes." }`. If the baseline evaluation errors, return the error and arm nothing: a watch that could never have fired must not be reported as armed.

`stop_watching` takes `watch_id`, or no argument to stop all. Idempotent.

The loop, in order, every tick:

1. If the cancel receiver is true, remove the row and return.
2. If `Instant::now() >= deadline`, wake once with a timeout message, remove the row and return.
3. If the wall-clock gap since the previous tick is more than ten poll intervals, re-baseline and continue without waking.
4. Evaluate `expression`. On error, or if the page is gone, or if `location.origin` changed, remove the row and return without waking.
5. If `until` is present, evaluate it. If true, remove the row and return without waking.
6. If the digest equals `baseline`, continue.
7. Set `baseline` to the new digest. Increment `wakes_used`. If `wakes_used > max_wakes`, remove the row and return (the wake that hits the cap still fires, carrying a message that says it is the last one).
8. Submit the composed prompt through `anthropic::submit_query`. Do not await the run to completion inside the tick; the queue already serializes.

### Evidence the slice needs before it is called done

- A lichess game against Juno, four moves played by hand with no spoken prompt between them, recorded as mp4 and committed to `docs/changelog/media/<PR>/` per the repo rule.
- Rust unit tests for the pure parts: clamping, the suspension rule, the digest comparison, the terminate precedence order (cancel before deadline before page-gone before `until` before change), and the registry focus rule that makes a `Run` outrank a `Watch`.
- Frontend tests for the new status label, the new bar state in `bar-state-mapper`, and the stop control.
- `cargo fmt`, then CI for clippy and tests. Do not compile locally.

### What cannot be reasoned about and has to be run

1. **Whether `Instant` advances across system sleep on this build.** On macOS `Instant` is backed by a clock that does not advance while the machine is asleep, which would silently extend a 30-minute deadline across a lunch break. Close the lid with a watch armed, open it, and read the log. The suspension rule in step 3 uses wall clock on purpose; the deadline needs the same treatment if the test shows `Instant` stalls.
2. **Whether the person's own Chrome is reachable.** `try_connect_to_existing_browser` (`browser_controller.rs:210`) requires remote debugging to already be on (`is_remote_debugging_enabled`, `:262`); otherwise Juno launches its own profile. So the honest answer to "can Juno watch the chess tab I already have open" is probably no, and the watch runs in Juno's own window. Verify which window the chess game actually ends up in before writing any copy that implies otherwise.
3. **Poll cost at 250ms.** One `Runtime.evaluate` round trip per tick over a local websocket should be well under a millisecond of CPU, but measure it with `powermetrics` over ten minutes before defending the default.
4. **Whether a wake that lands while the previous woken turn is still running queues correctly** or stacks up a backlog of identical wakes. The queue serializes, but nothing currently collapses duplicates.

## What the person sees

A watch is not a backend feature. Three surfaces, all already built except the states:

1. **The bar gets a `watching` state.** The bar's other states are about the current turn, and a watch is not a turn, which is the argument for leaving it out. The argument for putting it in wins: when a watch is the only thing Juno is doing, `default` is a lie, and the bar is the only surface the person is definitely looking at. So: no run in flight and at least one watch armed shows `watching`. A wake takes over with the normal run states. When that turn ends and the watch is still armed, the bar returns to `watching` rather than `default` (`ui_commands.rs:671`). The dot breathes slowly, the line reads the watch description, truncated. Clicking it stops the watch.
2. **The roster strip and session rows,** unchanged code, one new status. The row reads the agent name as `Watching: <description>` and carries the existing per-row cancel. This is where more than one watch is legible and the bar is not.
3. **Escape.** Once to stop a running turn, again to stop the watch. No new key, no new gesture.

The arm and the disarm are both spoken, because the turn that arms a watch is still a turn and still speaks: "I will watch the board and move when you do" and "I have watched twenty moves, tell me to keep going". The second sentence is the budget made audible, which is better than a number in a settings pane nobody opens.

## Removed, and decided against

- **A continuous screen-capture loop.** This was the obvious reading of the ask and it is the wrong answer. It needs a TCC permission Juno does not currently ask for, it watches the person's whole desktop including tabs that are none of Juno's business, and at one model call per tick it spends real money for a worse signal than a DOM read. It stays available as the existing `set_screen_monitor` tool, which a person can ask for explicitly.
- **Fixing `set_screen_monitor` in this slice.** Its defects are listed above and they are all real, but fixing them means deciding on image diffing, regions and thresholds, which is a bigger and separate argument. What this slice must do is stop it being invisible and unstoppable: give it the same session row and the same Escape participation. See the follow-up list.
- **A new wake-up entry point.** Everything goes through `submit_query`. A second door would mean the rate limiter, the approval check, the surface announcement and the queue all get bypassed.
- **A `WatchState` in the agent runner.** Considered giving the runner a "do not finish, wait for an event" action so the turn itself never ends. Rejected: it holds a model context, an escape registration, a session row and a queue slot open for the whole wait, and it makes the step cap meaningless. A watch between turns costs nothing while waiting.
- **CDP DOM mutation events instead of a poll.** Rank 2 above. `DOM.childNodeInserted` only fires for nodes the client has already requested, so it is not a drop-in MutationObserver, and a chess site's websocket traffic makes `Network.webSocketFrameReceived` a firehose. A 1Hz evaluate is simpler, bounded, and indistinguishable to the person.
- **A settings pane for watch defaults.** Nothing to configure until someone complains. The caps are named constants in `page_watch.rs` so the next person can argue with the numbers.
- **Per-tick approval.** The standing position (`docs/plans/permissions-by-consequence.md:17`) is that the default allows, with sending and spending as the exceptions. A watch is a spending question, and the answer is the wake budget, which is construction, not a prompt. A prompt per tick would be the worst of both.

## Open questions for Lacy

1. **Does the bar get a `watching` state, or does a watch live only in the roster strip?** Recommendation: it gets one. Idle while watching is a lie, and the bar is the only surface the person reliably sees.
2. **Default `max_wakes` of 20.** Twenty moves is most of a casual chess game and about twenty turns of model cost. Recommendation: ship 20, and make the last wake say it is the last so the person can extend it by speaking.
3. **Should a watch survive the turn that armed it being cancelled by Escape?** Recommendation: no. Escape on the turn stops the turn; if the turn that armed the watch is cancelled before it finished, the watch goes too. A watch armed by a completed turn survives, and needs its own Escape.
4. **Should `set_screen_monitor` keep shipping as it is while this lands?** Recommendation: no. Give it the session row and the Escape participation in the same PR as the slice, because an invisible unstoppable full-screen capture loop is the one defect here that is a privacy problem rather than a quality problem. The rest of its fixes can wait.
5. **Native (non-browser) watching: which signal first?** Recommendation: rank 5, the `NSWorkspace` activation notification, as a second slice. It answers "the person switched apps, this task is stale", it is push rather than poll, and it needs no permission Juno does not already hold.

## Follow-ups, not in this slice

- `set_screen_monitor` and `set_file_monitor`: read `region`, honour `threshold` with a real image diff, re-arm instead of one-shot, default `max_duration_seconds` to something finite, and stop using the global cancel in `timer_handlers.rs:471`.
- `NSWorkspace` activation and window-change observer, in the `stop_key_monitor` shape, ref-counted by a ledger.
- A passive `NSEvent` observer for "the person is typing in the window Juno was about to click", which is the real fix for Juno fighting the person for the keyboard.
- Collapsing duplicate wakes that arrive while a woken turn is still running.
- The tray: a background watch belongs in the menu bar, which means a glyph and a "Stop watching" item in `menu/tray_menu.rs`.

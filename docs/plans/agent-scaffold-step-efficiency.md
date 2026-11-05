# Agent scaffold and step efficiency

**Juno already has the scaffold the benchmark credits for the big win. What it does not have is any idea whether that scaffold works, because step count and wall clock per task were never recorded. This is the measurement, and the ranked list of what to change once there is a baseline to move.**

LAC-4008. Written 2026-10-16. Sources re-verified against the live papers the same day.

## The ticket's numbers hold. Its conclusion does not.

LAC-4008 was filed on the claim that the same Claude Opus 4.6 scores 39.2% bare and 73.7% under a good scaffold on MacAgentBench, and that "Juno is the scaffold, so this is Juno's number to move." The Chief of Staff asked for a re-check before building against a month-old reading. The numbers are right. What produces them is not what the ticket assumed.

MacAgentBench (676 tasks, 25 macOS apps, ~60% needing both GUI and CLI), Claude Opus 4.6:

| Framework | Pass@1 | Avg steps | What it has |
|---|---|---|---|
| Baseline | 39.2% | 27.1 | Pure GUI agent. Screenshots and mouse/keyboard only. No CLI, no AppleScript, no skills |
| Agent-S3 | 66.9% | 10.0 | Framework and multi-agent decomposition, no skill library |
| OpenClaw | 73.7% | 8.6 | The above plus a pre-built skill library |

The gap splits in two, and the halves are not the same kind of work:

- **39.2 to 66.9 is +27.7 points and 2.7x fewer steps, from framework design alone.** What Agent-S3 adds over the pure-GUI baseline is CLI and scripting access, structured UI reads instead of pixel guessing, and decomposition across agents.
- **66.9 to 73.7 is +6.8 points, from the skill library.** The paper is blunt that this is where its headline advantage comes from, and that it does not generalise: "on tasks without skill coverage, however, this advantage largely disappears. For 5 out of 8 models, OpenClaw even underperforms the baseline."

**Juno is not the baseline.** It is already built the way the +27.7 half is built. `agent/prompts/templates.rs` makes AppleScript and keyboard shortcuts "TIER 0: FASTEST & MOST RELIABLE" and the screenshot-based `computer` tool an explicit "LAST RESORT". `describe_ui` reads the accessibility tree instead of a picture. Clicks are AX-grounded through `AXUIElementCopyElementAtPosition`. The orchestrator delegates to desktop, browser and file specialists with isolated memory.

So the honest restatement: the 34.5-point gap is not sitting unclaimed waiting for a scaffold to be built. Most of it is the architecture Juno already shipped. **What is unclaimed is proof.** Nobody can say whether Juno's loop behaves like a 10-step scaffold or a 27-step one, because Juno has never logged the step count of a task.

That reorders this ticket's own bullets. Instrumentation was listed first and it is not just first, it is the whole of this PR.

## The second finding is the one that still bites

OSWorld-Human (arXiv 2506.16042), on efficiency rather than success:

- The best agents take **2.7-4.3x more steps** than a human needs for the same task.
- **Planning and reflection model calls are 75% to 94% of end-to-end latency.** Not the tool execution. The thinking.
- **A step near the end of a task takes about 3x as long as one at the start**, because prompt tokens and therefore latency grow as history accumulates.

That last one is the shape of Juno's remaining problem, and it is why the log line splits model time from tool time and prints one line per step. An average would hide a growth curve, and the growth curve is the finding.

## What this PR ships

`src-tauri/src/task_timing.rs`. Two log lines, next to the `[TurnTiming]` lines that already measure the first second of a turn:

```text
[StepTiming] task=3 step=7 model=1840 tools=320 tool_calls=2 screenshots=1
[TaskTiming] task=3 outcome=finished steps=7 wall=18240 model=12900 tools=4100 overhead=1240 tool_calls=11 screenshots=4 first_step=980 last_step=2840 concurrent=0
```

`overhead` is wall clock minus model minus tools. That is Juno's own cost: memory locks, approval waits, app-liveness checks, speech. No benchmark measures it for us, and it is the part only we can fix.

Five hooks in `agent/implementations/agent_runner.rs`, all additive:

| Hook | Where | Records |
|---|---|---|
| `TaskRun::begin` | top of `run` | opens the task, closed by `Drop` |
| `begin_step` | the existing `current_step += 1` | opens a step, logs the one before it |
| `add_model` | after each brain await | planning and reflection time |
| `add_tools` | around `execute_tools_with_batching` | desktop time and call count |
| `add_screenshot` | `add_tool_result_to_memory` | any result carrying a `base64_image` |
| `note_outcome` | `transition_state` | terminal state, by enum variant |

Two deliberate choices worth naming:

**The task closes on `Drop`, not on each exit path.** The loop has six exits: finished, cancelled, three flavours of running out of steps, and a tool error. A task left open would swallow the next one into its totals. `Drop` also covers `?` propagation and a panic, and there is no seventh exit to forget. This is the same discipline the escape-key rule enforces, done by construction instead of by vigilance.

**The outcome comes from the state machine, matched on the variant.** Every one of those six exits transitions state before it returns, so `transition_state` is the one place that sees all of them. `AgentState::Failed` carries free text and is never parsed, per the no-string-based-error-detection rule.

A delegated specialist joins the task that delegated to it rather than opening its own, because a task is what the person waited for. Two genuinely parallel background sessions (LAC-1432) would sum into one line, so a `begin` from a different session sets `concurrent=1` and marks the line as not fit for a baseline rather than quietly lying.

### The first hypothesis it settles

`MAX_ITERATIONS` is 15, and the baseline scaffold in the benchmark averages 27.1 steps. If Juno's loop is anywhere near baseline efficiency, it is hitting the step wall routinely and firing the continuation prompt at people mid-task. If it is near 8.6, the cap is generous and invisible. Those are very different products and one `grep [TaskTiming]` tells them apart.

## The audit, against the live guidance

Checked against the computer-use tool docs, fetched 2026-10-16.

| Documented guidance | Juno today | Verdict |
|---|---|---|
| "place the instruction text *before* the screenshot image... improves click accuracy" | Images first, in both `convert_message_to_api` and computer tool results | **Violates.** The code comment argues the opposite case on purpose, so this is a decision to revisit with data, not a typo |
| Attach a screenshot to the last result of a batch that did not end in one, which "saves a round trip" | Batching and the halt rule are implemented. Nothing attaches a verification screenshot | **Gap.** A documented free round-trip, and round-trips are the thing this ticket is about |
| "After each step, take a screenshot and carefully evaluate if you have achieved the right outcome" | The prompt says the reverse: "AVOID screenshots for routine UI clicking and task verification", and the `screenshot` action is documented as "(but really, don't use this)" | **Deliberate divergence.** Juno substitutes deterministic AX verification and `app_observation`. Probably better than asking the model to self-check, and currently unproven |
| Prefer keyboard shortcuts for dropdowns and scrollbars | Tier 0 of the prompt is AppleScript and keyboard shortcuts, ahead of any clicking | **Already done**, and stronger than the guidance asks |
| 20 or fewer images per request | Batch prune high-water is 12, so at most 11 screenshots are retained | **Already satisfied** |
| Prompt caching with batched rather than per-turn pruning | Implemented thoroughly in PR #598, with the cache-prefix reasoning written down | **Already done** |
| Per-model thinking effort | The API path sends adaptive thinking with no effort level, and `effort` is absent from `Provider::model_definitions()` | **Gap, and it is LAC-4007's**, not this ticket's |

### Where the loop re-reasons without re-screenshotting

The ticket asked where Juno re-screenshots unnecessarily. The pruning work already answered that: at most 11 images are ever in flight and 8 of every 9 turns are full cache hits. The cost that remains is not images, it is that **every step re-sends the whole history and pays planning latency on it**, which is exactly the OSWorld-Human growth curve. Fewer steps is the only lever on it. That is why step count, not screenshot count, is the headline field.

## Ranked follow-ups

Each gets its own issue and its own measured before and after, now that before and after can be measured.

1. **Text before image, in both sites.** Smallest possible change, a documented accuracy claim behind it, and now falsifiable. If click accuracy does not move, revert it and keep the comment.
2. **Attach a verification screenshot to a batch that did not end in one.** Removes a round trip per batch by the doc's own account. Directly a step-count win.
3. **Settle the verification question with data.** Juno's AX verification against the documented screenshot-and-evaluate loop. Juno's version should win. It has never been checked.
4. **Per-model thinking effort** (LAC-4007). On OSWorld 2.0 the same model moves from 22.08 to 48.65 between `low` and `max`. It is the largest single-parameter effect on this list and it is already someone's ticket.
5. **Run MacAgentBench against Juno.** MIT licensed, Docker'd macOS VM. Only worth the setup cost after 1 through 3, and only to get a real number rather than an argument.

## On the stated dependency

The ticket says it "depends on LAC-4007 and LAC-4000 landing first, since both change the per-step economics this would measure." That is now inverted, for two reasons.

Prompt caching and batched screenshot pruning, LAC-4007's first two levers, **already landed** in PR #598 under a different number. LAC-4007 is still `todo` on the board; the code is in `main`. Only per-model effort remains, and it changes which effort is sent, not how steps are counted.

And the ticket's own first bullet says "nothing improves what is not measured." LAC-4007's remaining lever is an efficiency claim that needs a before and after, and this is the thing that produces one. Measurement first is the only ordering that lets either ticket close on its own merits.

## Insanely-great pass

Run on this design before building, per the ticket.

```
For: whoever is trying to find out why a Juno task felt slow, with only a log file
Verdict: CUT
Remove: the model/provider field (the [TurnTiming] line in the same log already
  carries it), token and cache counts (they belong to the request, not the step),
  per-session tracker keying (nothing needs it yet), and all four behaviour
  changes this ticket invites
Fails: 2: the first draft of the line carried eleven fields plus provider, model
  and effort -> cut to what the two published findings actually need
Fails: 8: "audit the loop, fix the loop, benchmark the loop" is one ticket doing
  four things at eighty percent -> ship the measurement whole, file the rest
Ship as: two log lines and the five hooks that feed them, nothing that changes
  what Juno does
Evidence needed: a [TaskTiming] line from a real task, in the PR body
```

The removals are the point. A behaviour change shipped in the same PR as the instrument that measures it cannot be evaluated, and four of them at once cannot be attributed. The loop is untouched here on purpose.

## How to take a baseline

```bash
# Run the same handful of tasks, then:
grep TaskTiming ~/Library/Logs/Juno/juno-$(date +%F).log
grep StepTiming ~/Library/Logs/Juno/juno-$(date +%F).log   # the per-step curve
```

For a before and after, hold the model, the effort and the task list fixed, and discard any line with `concurrent=1`.

## Sources

- MacAgentBench: <https://arxiv.org/abs/2606.22557>, <https://github.com/JetAstra/MacAgentBench>
- OSWorld-Human: <https://arxiv.org/abs/2506.16042>
- Computer use tool: <https://platform.claude.com/docs/en/agents-and-tools/tool-use/computer-use-tool>

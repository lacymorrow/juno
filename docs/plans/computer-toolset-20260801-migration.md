# Migrating Juno to `computer_toolset_20260801`

Status: **investigation only. No code changed.**
Branch: `perf/computer-toolset-investigation`
Date: 2026-09-22
Docs verified live per LAC-3106 (nothing here is from cached model knowledge).

Primary source, fetched live on 2026-09-22:
<https://platform.claude.com/docs/en/agents-and-tools/tool-use/computer-use-tool>
Secondary: <https://platform.claude.com/docs/en/agents-and-tools/tool-use/define-tools>,
<https://platform.claude.com/docs/en/build-with-claude/vision#evaluate-image-size>

---

## Top three

1. **The coordinate pipeline is safe.** Removing `display_width_px` changes nothing, because
   the declared pair is already identical to the actual image dimensions by construction
   (`core.rs:115-119` resizes with `resize_exact` to exactly the pair `anthropic.rs:1592`
   declares). `utils/coordinates.rs` and `capture_screenshot_command` need **zero** edits.
   What must survive the refactor is the skip-guard at `anthropic.rs:1602-1613`, §2.3(a).
2. **Two live bugs found that have nothing to do with this migration and should be fixed
   now**: `ULTRA_HD` 2576x1610 is 5336 visual tokens against a 4784 ceiling, and Opus 4.5 /
   4.6 are handed high-res screenshots that exceed their 1568px limit. Both cause rejected
   screenshots today. §2.3(b), Phase 1.
3. **Keep Juno's batching executor, delete the batching prompt fragment.** Native batching
   is a contract, not a runtime; Juno's executor is the runtime the contract requires. Two
   semantics need fixing: halt-on-first-failure, and grouping tool results into one user
   message. §3.

---

## 0. Answer to the question that triggered this

> "Does the native action batching conflict with the batching we already have, or work in tandem, before we strip ours out?"

**Neither replaces the other, because they are not the same layer.**

`computer_toolset_20260801` does not ship a batching *runtime*. It ships a *contract*
describing how the client must execute and answer an assistant turn that contains several
`tool_use` blocks. Juno's `execute_tools_with_batching` **is** that client runtime, and it
already does most of what the contract asks.

Juno's "batching" is really two separate things, and they need opposite decisions:

| Piece | Where | Verdict |
|---|---|---|
| **Executor**: run N tool calls in order, one approval, cancellation-aware | `agent_runner.rs:205`, `:279`, `:297` | **Keep.** It is the required client half. Fix its failure semantics. |
| **Prompt fragment**: tells the model to emit batches, with hand-written `{"name":"computer","input":{"action":...}}` examples | `prompts/templates.rs:736-782` | **Delete on cutover.** Batching becomes native model behaviour, and the examples teach a wire format the new toolset rejects. |

So: **run both, minus the prompt fragment.** Detail in section 3.

---

## 1. What actually changes (verified)

### 1.1 Declaration

Today (`anthropic.rs:1626-1633`, serialized from `ApiTool::BuiltIn` at `anthropic.rs:202-220`):

```json
{"type": "computer_20251124", "name": "computer",
 "display_width_px": 1680, "display_height_px": 1050, "enable_zoom": true}
```
plus header `anthropic-beta: computer-use-2025-11-24,...` (`anthropic.rs:1726`, value built at `:399`).

After:

```json
{"type": "computer_toolset_20260801",
 "configs": {"zoom": {"enabled": true}},
 "cache_control": {"type": "ephemeral"}}
```
with **no computer-use beta flag**. Docs: "`computer_toolset_20260801`: No beta header required (GA)."

Rejected outright, returning `invalid_request_error`: `name`, `display_width_px`,
`display_height_px`, `display_number`, `enable_zoom`. Every one of those except
`display_number` is currently emitted by Juno.

`configs` is per-member. Each member takes `enabled` (default `true`, **including `zoom`**)
and `defer_loading` (default `false`). Because all 17 default to enabled, the `configs`
object can be omitted entirely unless Juno wants to switch something off.

### 1.2 One tool becomes seventeen

Members: `screenshot`, `zoom`, `left_click`, `right_click`, `middle_click`, `double_click`,
`triple_click`, `left_click_drag`, `mouse_move`, `left_mouse_down`, `left_mouse_up`,
`cursor_position`, `scroll`, `type`, `key`, `hold_key`, `wait`.

That is exactly the 17-action enum Juno already implements
(`anthropic_computer_use.rs:2557`). The list matches one-for-one. What changes is the
**dispatch shape**: the action moves from `input.action` to the block's `name`, and the
block carries `"toolset_name": "computer"`.

```json
{"type": "tool_use", "id": "toolu_01...", "name": "left_click",
 "toolset_name": "computer", "input": {"coordinate": [512, 384]}}
```

Every answering block must carry it too:

```json
{"type": "tool_result", "tool_use_id": "toolu_01...",
 "toolset_name": "computer", "content": [...]}
```

Docs: "Every `tool_result` answering a computer member must include `"toolset_name": "computer"`.
Results omitting it or naming a different toolset are rejected."

Dispatch is on the **pair** `(toolset_name, name)`, not on `name` alone. This matters:
`type` and `key` are plausible names for a user-defined tool, so matching on name alone
would be a real collision risk once Juno adds tools.

### 1.3 Per-member input schemas differ from Juno's

These are not cosmetic. Four are behavioural mismatches:

| Member | Toolset spec | Juno today (`anthropic_computer_use.rs:2551-2599`) | Delta |
|---|---|---|---|
| `left_click_drag` | `start_coordinate` **and** `coordinate` | `coordinate` only, "drag starts from current cursor position" (`:2561`) | **Breaking.** Must honour an explicit start point. |
| `hold_key` | `text` + `duration` in **seconds**, max 300 | `duration_ms` / `duration` in **milliseconds** (`:2572-2579`) | **Breaking unit mismatch.** A 2s hold becomes a 2ms hold. |
| `left_mouse_down` / `left_mouse_up` | no input, acts at current cursor | takes coordinates | Behavioural. |
| `key` | `text` + optional `repeat` (1-100) | `key` or `text`, no `repeat` | Additive. |
| clicks | optional `text` = modifier keys (`"shift"`, `"ctrl+shift"`) | `text` is overloaded for typing | Needs disambiguation per member. |
| `wait` | `duration` seconds, max 300 | `seconds` or `duration` | Normalise. |
| `scroll` | `scroll_direction`, `scroll_amount`, optional `coordinate`, `text` | same | Match. |
| `zoom` | `region: [x0,y0,x1,y1]` | same (`:2592`) | Match. |

### 1.4 Native batch semantics

Docs, verbatim on the failure rule: "Run sequentially; stop at first failure." Unexecuted
blocks must still be answered, with this exact body:

```json
{"type": "tool_result", "tool_use_id": "...", "toolset_name": "computer",
 "is_error": true,
 "content": "Not executed: an earlier computer action in this turn failed."}
```

"A request leaving any `tool_use` block in a batch unanswered is rejected with
`invalid_request_error`."

No maximum batch size is stated. No opt-in: the docs describe batching as inherent model
behaviour ("Claude can plan a short sequence of actions ... and return them together in one
response"), not a flag. Juno gets batches whether or not it asks for them, which is already
true today.

### 1.5 Model support

Live table:

| Supports `computer_toolset_20260801` | In Juno's model list? |
|---|---|
| Claude Fable 5.1 | yes, and it is the default |
| Claude Fable 5 | yes |
| Claude Opus 5 | yes |
| Claude Sonnet 5 | yes |
| Claude Opus 4.8 | yes |
| Claude Mythos 5 | **no** |
| Claude Mythos 5.1 | **no** |

Stay on `computer_20251124` + beta header: Opus 4.7, Opus 4.6, Opus 4.5, Sonnet 4.6.
All four are in Juno's list (`types.rs:11,14,13,24`).

**Claude Haiku 4.5 does not support computer use in any tool version.** Verified live:
the docs table lists it as "Not supported / Computer use unavailable". Juno currently
advertises the opposite: `types.rs:282-287` sets `supports_computer_use: true` for
`claude-haiku-4-5-20251001`, and `types.rs:43` routes it to `computer_20250124`. That is a
live wrong claim in the model picker, independent of this migration.

---

## 2. The display-dimension question, answered

This was flagged as the top risk. **It is a simplification, not a breakage.** Here is why,
with the evidence.

### 2.1 What Juno does today

1. `capture_screenshot_command` (`commands/core.rs:48`) captures, picks a standard
   resolution for the display+model (`core.rs:87-98`), and resizes the image with
   **`resize_exact(standard_width, standard_height)`** (`core.rs:115-119`), then JPEG q85.
2. `anthropic.rs:1592-1600` reads back that same pair via
   `coordinates::get_current_standard_resolution()` and declares it as
   `display_width_px` / `display_height_px`.
3. Claude returns coordinates in that declared space.
4. `coordinates::transform_standard_to_screen_coordinates` (`utils/coordinates.rs:334`)
   divides by `display_to_standard_scale_{x,y}` and adds `display_origin_{x,y}` to reach
   global screen coordinates.

### 2.2 What changes

Under the new toolset, step 2 disappears. Claude infers the coordinate space from the image
it receives. That image is already **exactly** `standard_width x standard_height`, because
of `resize_exact`. The declared pair and the actual image pair are the same numbers today,
by construction and by an explicit comment at `core.rs:82-83`:

> "This MUST match the resolution used inside `update_standard_resolution_scaling_with_display`
> so the image Claude receives matches the declared `display_width_px`/`display_height_px`."

Therefore the coordinate space Claude emits is **bit-identical before and after**.

**`utils/coordinates.rs` needs zero changes. `capture_screenshot_command`'s scaling path
needs zero changes. `constants/ui.rs::standard_resolutions` needs no change for coordinate
reasons.** The declaration was always redundant with the image; the new toolset just stops
asking for the redundant copy.

Docs confirm the client keeps owning the mapping: "If you scale screenshots down before
returning them, scale Claude's coordinates back up before applying them to the real display."
Juno already does precisely that.

### 2.3 What is genuinely at risk in that area

Three things, and only one is caused by the migration.

**(a) The skip-guard is load-bearing and a naive port deletes it.**
`anthropic.rs:1594-1613` returns `None` (dropping the computer tool from the request
entirely) when `get_current_standard_resolution()` yields zeros or errors. Its stated
purpose is "skipping computer tool to prevent coordinate mismatch". Under the new
declaration there is no dimension field to fill, so the obvious refactor removes the guard
along with the fields. **It must be kept as an explicit gate**, because
`transform_standard_to_screen_coordinates` silently returns the identity when
`standard_width == 0` (`coordinates.rs:340-347`). Without the guard, an uninitialised
session would hand the model a full-resolution image, take back full-resolution
coordinates, apply an identity transform, and click in roughly the right place on the main
display and the wrong place on every secondary display. Silent, intermittent, and exactly
the class of bug the pipeline was hardened against.

**(b) Two pre-existing image-size violations that the new docs make fatal.**
Not caused by this migration, but the new page states the consequence in a way the old one
did not: "the toolset takes no display dimensions and the API doesn't downscale for you, so
an oversized `tool_result` image is rejected with a validation error."

Visual-token budget is `ceil(w/28) * ceil(h/28)`. Limits: 4784 tokens / 2576px long edge for
Opus 4.7 and later; 1568px / ~1.15MP for everything earlier.

| Constant (`constants/ui.rs`) | Pixels | Visual tokens | 4784 tier | 1568px tier |
|---|---|---|---|---|
| `XGA` :153 | 1024x768 | 1036 | ok | ok |
| `WXGA` :156 | 1280x800 | 1334 | ok | ok |
| `FWXGA` :159 | 1366x768 | 1372 | ok | ok |
| `HD_WXGA` :168 | 1680x1050 | 2280 | ok | **over** (1680 > 1568) |
| `HD_1080` :171 | 1920x1080 | 2691 | ok | **over** |
| `UW_1080` :179 | 2560x1080 | 3588 | ok | **over** |
| `ULTRA_HD` :174 | 2576x1610 | **5336** | **over by 552** | **over** |

Two findings:

- `ULTRA_HD` (2576x1610) is 4.15 MP against a ~3.75 MP ceiling and 5336 tokens against 4784.
  It satisfies the long-edge rule and violates the token rule. It is reachable: it is only
  filtered out when it does not fit inside the display (`ui.rs:240-244`), so a 16:10 display
  of 2576x1610 logical points or larger selects it (aspect tie with `HD_WXGA` at 1.600,
  broken toward the larger, `ui.rs:253-270`). A retina MacBook reports logical points
  (`core.rs:296-297` reads `display_info.bounds`, which is points, not pixels), so the
  common laptop case lands on `WXGA` and is safe. External 16:10 4K-class panels are not.
  Suggested replacement: **2400x1500** (4644 tokens, same 16:10 aspect, comfortably inside
  both limits). 2432x1520 lands on 4785, one token over, so do not cut it finer.
- `supports_high_res` (`ui.rs:195-198`) keys off `OPUS_4_5_PLUS_MODELS`, which includes
  Opus 4.5 and Opus 4.6. Per the live docs the 2576px tier begins at **Opus 4.7**. So Opus
  4.5 and 4.6 are handed `HD_WXGA`/`HD_1080`/`ULTRA_HD`/`UW_1080`, and **all four exceed
  their 1568px / 1.15MP limit**. The code comment at `ui.rs:162-165` contains the bug in
  plain sight: it correctly cites Opus 4.7, then applies the number to "all models using the
  computer_20251124 tool type."

**(c) Global scaling state versus batches and parallel sessions.**
`SCREENSHOT_SCALE` is a single process-global `RwLock` (`coordinates.rs:24`) and the target
display is re-chosen per capture from the **cursor position** (`core.rs:284-294`). A batch of
five actions is planned against one screenshot but executed over several seconds, during
which nothing pins the scaling state. If a capture on another parallel session (LAC-1432)
lands mid-batch, or the cursor crosses to another display, later coordinates in the same
batch are inverted with the wrong scale factor. This is latent today; leaning harder on
batching makes the window wider. Mitigation is cheap: snapshot `ScalingInfo` (it is `Copy`,
`coordinates.rs:46`) at the start of a batch and invert against the snapshot.

**None of (a), (b) or (c) requires editing the transform math.** (a) is a guard to preserve,
(b) is two constants and one model list, (c) is a snapshot. The hard-won screenshot and
coordinate pipeline stays as it is.

---

## 3. Juno's batching versus native batching, side by side

| Property | Juno today | `computer_toolset_20260801` | Verdict |
|---|---|---|---|
| Who produces the batch | prompt fragment `templates.rs:736` instructs the model | native model behaviour, no opt-in | delete the fragment |
| Order | sequential, index order, `agent_runner.rs:317` | "executed sequentially in order (not parallel)" | **match** |
| Parallelism | none | explicitly none | **match** |
| On a failing action | **continues to the next action** | **stops at first failure** | **MISMATCH, must fix** |
| Unexecuted actions | only on cancellation: `"Tool execution was cancelled"`, `agent_runner.rs:244-127` | `is_error: true` + exact string `"Not executed: an earlier computer action in this turn failed."` | **MISMATCH, must fix** |
| Every `tool_use` answered | yes, validated at `anthropic.rs:1564-1574` | required, else `invalid_request_error` | **match** |
| Results grouping | **one `user` message per result**, `anthropic.rs:1530-1545` | all results in **one** `user` message, nothing between it and the tool-use turn | **MISMATCH, must fix, see §3.2** |
| `toolset_name` on results | not emitted, field absent from `ApiContentBlock` (`anthropic.rs:126-147`) | mandatory | **must add** |
| Approval | one approval for the whole batch, riskiest member sets the level, `agent_runner.rs:438-511` | not specified | **Juno-only, keep** |
| Escape / cancellation | checked before each action and raced against each action, `agent_runner.rs:319-326`, `:352-361` | not specified | **Juno-only, keep** |
| Pacing between actions | 300ms + arbiter, §4 | not specified | **Juno-only, keep, but tune** |

### 3.1 The failure-semantics gap is the real bug

Juno does not currently halt on failure, and it is worse than it looks. The computer tool
almost never returns `Err`: failures come back as `Ok(create_anthropic_error_response(...))`
(`anthropic_computer_use.rs:1261`, `:1328`) and are only recognised afterwards by sniffing
the payload with `is_anthropic_error_response` (`agent_runner.rs:226`). The executor's loop
at `agent_runner.rs:317-428` inspects that only for *logging*. It then proceeds to the next
action regardless.

Concrete consequence today, before any migration: in a `click -> type -> key Return` batch,
if the click fails, Juno still types the text and still presses Return, into whatever
happens to be focused. The model is then shown three results and has no signal that the
sequence broke. The new spec names this as the thing not to do. **Fixing halt-on-failure is
worth doing on its own merits and should not wait for the toolset cutover.**

### 3.2 Tool results must be grouped into one user message

Confirmed live against
<https://platform.claude.com/docs/en/agents-and-tools/tool-use/parallel-tool-use> and
<https://platform.claude.com/docs/en/agents-and-tools/tool-use/handle-tool-calls>:

> "Whichever strategy you use, return one `tool_result` for each `tool_use` block, all
> together in the next user message. Match each result to its call with `tool_use_id`, and
> put every `tool_result` block before any text content in that message."

> "Tool result blocks must immediately follow their corresponding tool use blocks in the
> message history. You cannot include any messages between the assistant's tool use message
> and the user's tool result message."

> "In the user message containing tool results, the `tool_result` blocks must come FIRST in
> the content array."

Juno violates this shape for every multi-block turn. `anthropic.rs:1530-1545` pushes a
**separate** `ApiMessage { role: "user", content: Blocks(vec![one tool_result]) }` per
result, inside the per-message loop at `:1397`. A three-action batch produces three
consecutive user messages.

That this ships today suggests the API is merging consecutive same-role turns rather than
rejecting them. The `api/messages` reference is claimed to say "Consecutive `user` or
`assistant` turns in your request will be combined into a single turn", but that quote came
back from a summarising fetch and could not be confirmed against raw page text, so it is not
being relied on here. Either way the documented contract is explicit and Juno should match
it rather than lean on merge behaviour it cannot cite.

The fix is contained: accumulate results for one assistant turn and push a single
`ApiMessage` whose `content` is the full `Vec<ApiContentBlock>` of `tool_result`s, in
`tool_use` order. The `pending_tool_calls` bookkeeping at `:1349`, `:1408`, `:1438` already
tracks exactly the set that needs grouping.

The docs also confirm the `is_error` skip shape used for halted batches, and give the
parallel-tool-use variant of the sentence:

```json
{"type": "tool_result", "tool_use_id": "toolu_02", "is_error": true,
 "content": "Not executed: the preceding write_file call failed."}
```

The computer-use page's sentence is the one to emit for computer members:
`"Not executed: an earlier computer action in this turn failed."`

And it confirms `toolset_name` is conditionally required, not optional:

> "A `tool_result` that answers a computer use or browser use member block must also echo
> the same `toolset_name` value as the `tool_use` block; a member result that omits it is
> rejected."

### 3.3 Recommendation

**Keep Juno's executor. Do not replace it with "native batching" - there is nothing to
replace it with.** Change three things inside it, delete one prompt fragment.

1. `execute_sequential_batch` halts on the first failing action, where "failing" means
   `Err(_)` **or** `is_anthropic_error_response(&result.output)`.
2. Remaining actions in the halted batch get a `tool_result` with `is_error: true` and the
   exact sentence from the docs. Reuse the existing `handle_batch_cancellation` shape
   (`agent_runner.rs:244`), which already walks `tool_results_cache` for `None` entries;
   it needs a reason parameter rather than the hardcoded cancellation string.
3. `toolset_name` plumbed end to end (§5).
4. `PromptFragments::tool_batching_optimization` removed from the three templates that
   include it (`templates.rs:1747`, `:1778`, `:1993`). Its worked examples all use
   `{"name": "computer", "input": {"action": ...}}`, which is precisely the shape the new
   toolset rejects. Leaving it in actively mis-steers the model post-cutover.

Everything the executor adds on top of the spec (batch approval, Escape mid-batch,
cancellation fill, per-action logging, cursor overlay) is Juno's own product surface and the
spec says nothing that conflicts with it.

---

## 4. Cooldowns inside a batch

Two pacing mechanisms sit on the action path.

- `enforce_action_cooldown` (`anthropic_computer_use.rs:220`, `ACTION_COOLDOWN_MS = 300`
  at `:25`). Sleeps until 300ms have passed since the **start** of the previous UI-modifying
  action, tracked in a process-global `AtomicU64` (`:28`). Applies to the 14 UI-modifying
  actions listed at `:200-216`.
- `InputArbiter::acquire` (`input_arbiter.rs:68-87`, `DEFAULT_COOLDOWN = 500ms` at `:26`,
  wired in at `state.rs:478`). Sleeps until 500ms have passed since the previous guard was
  **dropped**, i.e. since the previous action completed. Acquired only for the nine
  `always_physical` actions (`anthropic_computer_use.rs:1307-1323`) plus the physical
  fallback paths inside AX-grounded click and type.

**Correction to the "up to 800ms" framing.** These do not simply add. Both are
"time since the previous action", not "sleep this long", so they overlap. For an action of
duration `T`, spacing from one action's start to the next is roughly
`max(300, T + 500)` ms, not 800. The 300ms cooldown is almost entirely absorbed by the
arbiter's 500ms whenever the arbiter is taken at all. Where it is *not* absorbed is the
common case: AX-grounded clicks and typing skip the arbiter (that is the documented
parallelism moat, `input_arbiter.rs:16-19`), so those pay only 300ms.

Cost of a 5-action batch:

| Batch shape | Pacing paid |
|---|---|
| 5 AX-grounded clicks | ~1.2s (4 x 300ms) |
| 5 physical `key`/`scroll` | ~2.0s + action time (4 x ~500ms) |
| mixed, typical | ~1.2 to 2.0s |

In a one-action-per-turn loop this is free: a Fable 5.1 round trip with adaptive thinking
exceeds 500ms comfortably, so the cooldown always expires during the model call. Inside a
batch there is no round trip to hide behind, so it becomes the dominant cost and eats a
meaningful share of the latency batching was meant to buy.

Note for the agent fixing the double-counting: after the 300ms path is removed or folded in,
**the remaining 500ms is still unconditional and still un-hidden inside a batch.** Two
options, in order of preference:

1. Reuse the mechanism already proven in this repo: `wait_for_mouse_movement_completion`
   (`agent_runner.rs:216-219`) replaced a hardcoded 350ms delay with event-driven
   completion detection. The same trick generalises: after a click or keystroke, wait on an
   AX "element settled" signal with the 500ms as a ceiling rather than a floor.
2. Failing that, make the cooldown positional: full cooldown for the first action of a batch
   and for any action that changes the frontmost app (already detected by
   `get_frontmost_bundle_id`, `anthropic_computer_use.rs:294`), reduced between same-app
   actions inside one batch.

Do not simply lower `DEFAULT_COOLDOWN` globally. It exists because macOS drops events fired
faster than it delivers them, and it also serialises parallel sessions against a single
hardware pointer.

---

## 5. Exhaustive list of call sites

### 5.1 Tool declaration and wire format

| File:line | What | Change |
|---|---|---|
| `src-tauri/src/constants/api.rs:40` | `COMPUTER_20251124` | keep, add `COMPUTER_TOOLSET_20260801 = "computer_toolset_20260801"` |
| `src-tauri/src/constants/api.rs:63-64` | `COMPUTER_USE_2025_11_24` beta flag | keep for legacy tier; the new toolset must **not** send it |
| `src-tauri/src/constants/api.rs:88` | `COMPUTER_USE_2025_11_24_TOOLS` group | add a `_20260801` group |
| `src-tauri/src/agent/providers/anthropic.rs:202-220` | `ApiTool::BuiltIn` | add a third variant `Toolset { tool_type, configs, cache_control }` with no `name`/`display_*`/`enable_zoom`. Docs: a client toolset is "a single entry with no `name`". |
| `src-tauri/src/agent/providers/anthropic.rs:1592-1600` | reads standard resolution, declares `display_width_px`/`display_height_px` | remove the fields, **keep the gate**, see §2.3(a) |
| `src-tauri/src/agent/providers/anthropic.rs:1602-1613` | `return None` skip-guard | preserve as an explicit gate |
| `src-tauri/src/agent/providers/anthropic.rs:1621-1626` | `enable_zoom = Some(true)` when type contains `20251124` | becomes `configs: {"zoom": {"enabled": true}}`, or omit entirely since the default is already `true` |
| `src-tauri/src/agent/providers/anthropic.rs:1645-1660` | `cache_control` applied to the last tool | toolset entries accept `cache_control`; verify the new variant is covered by the `match` |
| `src-tauri/src/agent/providers/anthropic.rs:399-408` | `beta_header_value` | must drop the computer-use flag for toolset models, keep `prompt-caching-2024-07-31` and the fallback flag |
| `src-tauri/src/agent/providers/types.rs:163-177` | `computer_use_beta_flag` | return `""` for toolset-tier models |
| `src-tauri/src/agent/providers/types.rs:185-210` | `resolve_tool_type` | add the toolset tier above the Opus 4.5+ tier |

### 5.2 `toolset_name` plumbing

| File:line | What | Change |
|---|---|---|
| `src-tauri/src/agent/providers/anthropic.rs:126-147` | `ApiContentBlock` | add `#[serde(skip_serializing_if = "Option::is_none")] toolset_name: Option<String>` |
| `src-tauri/src/agent/providers/anthropic.rs:1254-1266` | tool_use replay on assistant turns | set `toolset_name` from the call. The API rejects tool-use turns whose blocks were mangled, same class of failure as the thinking-block replay rule already documented in CLAUDE.md |
| `src-tauri/src/agent/providers/anthropic.rs:1530-1545` | tool_result construction, one `ApiMessage` per result | set `toolset_name: Some("computer")` for computer members, **and** group all results for one assistant turn into a single `ApiMessage`, §3.2 |
| `src-tauri/src/agent/providers/anthropic.rs:1349`, `:1408`, `:1438` | `pending_tool_calls` bookkeeping | already tracks the exact set that needs grouping, reuse it |
| `src-tauri/src/agent/providers/anthropic.rs:1889-1903` | non-streaming `tool_use` parse | read `toolset_name` off the block |
| `src-tauri/src/agent/providers/anthropic.rs:672-690` | streaming `content_block_start` parse | read `toolset_name` off `content_block` |
| `src-tauri/src/agent/core.rs:119-124` | `ToolCall` | add `toolset_name: Option<String>`. This is the type the whole executor passes around |
| `src-tauri/src/agent/core.rs:127-132` | `ToolResult` | carry it back for the result builder |
| `src-tauri/src/agent/core.rs:136-147` | `ToolDefinition` | `api_type` is currently the only versioning hook; a toolset has no per-member definition, so registration needs a "this is a toolset member" marker |

### 5.3 Name-based dispatch that breaks when `name` becomes the member

Every one of these compares `tool_call.name == "computer"`. After the cutover the name is
`left_click`, `type`, `screenshot`, and so on. All must become a `(toolset_name, name)` check.

| File:line | Context |
|---|---|
| `src-tauri/src/agent/implementations/agent_runner.rs:174` | `is_mouse_movement_tool`, reads `input["action"]` |
| `src-tauri/src/agent/implementations/agent_runner.rs:338` | skip runner-level request logging |
| `src-tauri/src/agent/implementations/agent_runner.rs:372` | skip runner-level result logging |
| `src-tauri/src/agent/implementations/agent_runner.rs:702` | preserve full JSON so the provider can extract `base64_image` |
| `src-tauri/src/agent/providers/anthropic.rs:1452` | `tool_name == "computer"` gate for screenshot image blocks. `tool_name` comes from `Message.name` (`:1442`), so the memory layer needs the toolset too |
| `src-tauri/src/agents/desktop_agent.rs:60-63` | routes `"computer"` to `execute_computer_tool` |
| `src-tauri/src/agent/providers/juno_mcp.rs:197`, `:211` | juno-cua MCP server filters to the single `computer` tool. MCP has no toolsets, so **this path keeps the old `action`-style shape** and must be explicitly insulated from the change |
| `src-tauri/src/cli/headless.rs:881`, `:1198`, `:1206` | headless CLI tool-name matching |
| `src-tauri/src/agent/implementations/tool_provider.rs:1183` | "Computer tool text parameter too long" validation |

### 5.4 Action dispatch and per-member schemas

| File:line | Change |
|---|---|
| `anthropic_computer_use.rs:1258-1265` | `execute_computer_tool` reads `input["action"]`. New toolset sends no `action`. Needs an adapter that synthesises `{"action": <member>, ..input}` so the 1400-line body below it is untouched. This is the cheapest possible migration shape and is strongly preferred |
| `anthropic_computer_use.rs:2525-2600` | `create_versioned_tools`, the `computer` `ToolDefinition`. For toolset models this is not sent at all, but the executor still needs the registration to route calls |
| `anthropic_computer_use.rs:2693-2790` | `register_anthropic_computer_use_tools_with_version`, matches on `tool.name == "computer"`. Must register all 17 member names, or register one entry plus an alias table |
| `anthropic_computer_use.rs:2572-2579` | `hold_key` duration in ms vs the spec's seconds. **Unit bug, fix in the adapter** |
| `anthropic_computer_use.rs:2559-2563` | `left_click_drag` lacks `start_coordinate` |
| `anthropic_computer_use.rs:200-216` | `is_ui_modifying_action` takes an action string, keeps working via the adapter |
| `anthropic_computer_use.rs:1307-1318` | `always_physical` match, same |
| `run_computer_action` at `anthropic_computer_use.rs:66-95` | reads `input["action"]` at `:77` for cursor-overlay state, same adapter fixes it |

### 5.5 Model gating

| File:line | Change |
|---|---|
| `types.rs:29-39` | `OPUS_4_5_PLUS_MODELS` is doing two unrelated jobs: computer-tool tier **and** screenshot-resolution tier (via `ui.rs:197`). Split into `COMPUTER_TOOLSET_20260801_MODELS` (Fable 5.1, Fable 5, Opus 5, Sonnet 5, Opus 4.8) and `HIGH_RES_2576_MODELS` (Opus 4.7 and later, **not** 4.5 or 4.6) |
| `types.rs:282-287` | Haiku 4.5 `supports_computer_use: true` is wrong, see §1.5 |
| `types.rs:545-548` | test asserting Haiku 4.5 maps to `computer_20250124` encodes the same wrong assumption |
| `constants/ui.rs:174` | `ULTRA_HD` 2576x1610 exceeds the visual-token budget, see §2.3(b) |
| `constants/ui.rs:191-198` | `supports_high_res` keys off the wrong list |
| `constants/ui.rs:161-165` | comment cites Opus 4.7 then applies it to the whole `computer_20251124` tier |
| Model list | Mythos 5 and Mythos 5.1 absent entirely. Adding them is optional and out of scope, but note it |

### 5.6 Executor

| File:line | Change |
|---|---|
| `agent_runner.rs:317-428` | halt on first failure, treating `is_anthropic_error_response` as failure |
| `agent_runner.rs:244-271` | `handle_batch_cancellation` gains a reason parameter so it can emit the exact "Not executed" sentence as well as the cancellation string |
| `agent_runner.rs:205-240` | `execute_tools_with_batching`, snapshot `ScalingInfo` for the batch, see §2.3(c) |
| `prompts/templates.rs:736-782` | delete `tool_batching_optimization` |
| `prompts/templates.rs:1747`, `:1778`, `:1993` | remove the three includes |
| `prompts/templates.rs:2002`, `:2005` | drop the stale "tool batching optimization" description and tag |

### 5.7 Versioning module

`agent/tools/tool_versioning.rs` models versioning as `ApiVersion -> (api_type, beta_flag)`
per tool name. A toolset has no per-tool name and no beta flag, so the enum needs a fourth
variant `ComputerToolset20260801` whose `beta_flag()` is empty and whose `get_tool_api_type`
returns the toolset type for a synthetic key. Affected: `:14-21`, `:25-31`, `:34-40`,
`:43-49`, `:103-125`, `:128-134`, `:248-300` (tests).

---

## 6. Risk register

Ranked. #1 is the coordinate question because it was flagged as off limits.

| # | Risk | Severity | Likelihood | Mitigation |
|---|---|---|---|---|
| 1 | **Coordinate/display pipeline disturbed by removing `display_*_px`.** | **Low, and this is the finding.** The declared pair is already identical to the image's actual pair by construction (`core.rs:82-83`, `:115-119`). `utils/coordinates.rs` and `capture_screenshot_command` need **no change**. | n/a | Change nothing in the transform math. Add a regression test asserting `get_current_standard_resolution()` equals the decoded JPEG dimensions. |
| 2 | **The skip-guard at `anthropic.rs:1602-1613` is deleted along with the fields it guarded.** Silent wrong-place clicks on secondary displays, because the transform degrades to identity at `coordinates.rs:340-347` instead of erroring. | **High** | High, it is the obvious refactor | Keep the gate as an explicit precondition. Better: make `transform_standard_to_screen_coordinates` return `Result` instead of silently passing through. |
| 3 | **`ULTRA_HD` 2576x1610 = 5336 visual tokens, over the 4784 ceiling.** Docs: oversized `tool_result` image "is rejected with a validation error". Every screenshot fails on affected displays. | **High** | Medium, needs a 16:10 display of 2576x1610 points or larger | Replace with 2400x1500 (4644 tokens). Add a compile-time or unit-test assertion that every entry in `ALL_RESOLUTIONS` is inside its tier's budget. Pre-existing, fix independently. |
| 4 | **Opus 4.5 and 4.6 get 2576-tier resolutions they cannot accept.** All four high-res constants exceed their 1568px / 1.15MP limit. | **High** | High whenever those models are selected | Split the model lists, §5.5. Pre-existing. |
| 5 | **`hold_key` seconds vs milliseconds.** A 2-second hold becomes 2ms. Silent no-op, not an error. | Medium | Certain once the member schema is live | Convert in the adapter, add a unit test. |
| 6 | **`toolset_name` missing on a `tool_result`.** Docs: "Results omitting it or naming a different toolset are rejected." Hard failure of every agent turn. | High | Low, it fails immediately and loudly | Plumb it, §5.2. Fails fast, so it will not ship silently. |
| 7 | **Results grouping: Juno emits one `user` message per `tool_result` (`anthropic.rs:1530`); the docs require all results "all together in the next user message" with nothing between.** Confirmed mismatch, §3.2. Ships today only because the API appears to merge consecutive same-role turns, which is behaviour Juno cannot cite. | **High** | Certain for every multi-block turn | Accumulate per assistant turn, push one `ApiMessage` with all `tool_result` blocks in `tool_use` order, results first in the content array. Contained to `:1530-1545` plus the `pending_tool_calls` bookkeeping. |
| 8 | **`left_click_drag` `start_coordinate` ignored.** Drags start from wherever the cursor happens to be. | Medium | Certain | Honour the field. |
| 9 | **Global `SCREENSHOT_SCALE` changes mid-batch** (cursor crosses displays, or a parallel session captures). Later actions in the batch invert with the wrong scale. | Medium | Low per batch, rises with batch length and session count | Snapshot `ScalingInfo` per batch, §2.3(c). |
| 10 | **Stale prompt fragment teaches the rejected `{"action": ...}` shape** post-cutover. | Medium | Certain if not removed | Delete it in the same commit as the cutover, not before. |
| 11 | **No halt-on-failure**: a failed click is followed by typing into whatever is focused. | Medium | Already happening today | Fix independently of the migration, §3.1. |
| 12 | **Haiku 4.5 advertised as computer-use capable** when it supports it at no version. | Low | Only if a user selects it | Flip the flag, `types.rs:282-287`. |
| 13 | **Cooldowns eat the latency win.** ~1.2 to 2.0s of sleeping per 5-action batch. | Low, a performance ceiling not a bug | Certain | §4. Do not lower `DEFAULT_COOLDOWN` globally. |
| 14 | **Claude CLI provider and juno-cua MCP diverge.** `juno_mcp.rs:197-211` offers a single `computer` tool over MCP; MCP has no toolsets. | Low | n/a | Explicitly leave the MCP path on the `action` shape. The adapter in §5.4 makes both shapes converge on one implementation. |

---

## 7. What could not be verified

Stated plainly, as requested.

1. **Whether the API silently tolerates Juno's split `tool_result` messages, or is merging
   consecutive same-role turns behind the scenes.** The requirement itself is now confirmed
   (§3.2): results go "all together in the next user message", with nothing between. What is
   not confirmed is why Juno's split form has not been failing. The `api/messages` reference
   is claimed to say "Consecutive `user` or `assistant` turns in your request will be combined
   into a single turn", but that came back from a summarising fetch and could not be checked
   against raw page text. This does not change the recommendation, since Juno should match the
   documented contract either way, but it does mean the grouping fix may be invisible in
   testing rather than obviously fixing a failure.
2. **Whether `computer_toolset_20260801` tolerates a `zoom` region larger than the visual-token
   budget**, and whether a zoom image counts against the same ceiling as a screenshot. The
   docs say zoom "capture[s] a region at full resolution" and that coordinates stay in full
   screenshot space, but give no separate size rule for zoom output. Juno's zoom returns a
   region at native resolution, which on a retina display is 2x the logical region and could
   exceed the ceiling for a large region.
3. **Whether the model's spatial accuracy changes** when it is no longer told the display
   dimensions. The coordinate space is provably identical (§2.2), but "identical wire format"
   is not "identical model behaviour". Needs an empirical click-accuracy comparison.
4. **Anything requiring a compile or a run.** Per the constraint for this task, no `cargo`
   command of any kind was executed. Every `file:line` here is from reading source, not from
   a build. Line numbers are against `main` at `c0cafed6`.
5. **Whether `defer_loading` on members is useful to Juno.** The docs point at the tool-search
   feature rather than explaining it in the computer-use context. Not needed for this migration.
6. **Mythos 5 / Mythos 5.1 behaviour**, since Juno does not configure them.

---

## 8. Phased sequence

Each phase ships on its own and leaves the app working. Phases 1 and 2 are independent of
the toolset and are worth landing first regardless of whether the migration proceeds.

**Phase 0 - probe (no app changes).**
One-off script against the live API confirming a `computer_toolset_20260801` declaration with
no `configs` is accepted on `claude-fable-5-1`, that a member `tool_result` without
`toolset_name` is rejected as documented, and how a split-vs-grouped `tool_result` pair is
handled (§7.1). Gate: all three answered.

**Phase 1 - fix the pre-existing image-size bugs.** No toolset involvement.
Split `OPUS_4_5_PLUS_MODELS` into a computer-tier list and a resolution-tier list
(`types.rs:29`, `ui.rs:195`). Replace `ULTRA_HD` with 2400x1500 (`ui.rs:174`). Add a unit
test asserting every `ALL_RESOLUTIONS` entry fits its tier's `ceil(w/28)*ceil(h/28)` budget.
Flip Haiku 4.5's `supports_computer_use` (`types.rs:282`). Ships alone, fixes real rejections.

**Phase 2 - halt-on-failure.** No toolset involvement.
`agent_runner.rs:317-428` stops at the first failing action, `is_anthropic_error_response`
counted as failure. `handle_batch_cancellation` gains a reason parameter. Use the exact
"Not executed" sentence now, since it is valid under both regimes. Ships alone, fixes the
click-fails-then-types-anyway bug described in §3.1.

**Phase 3 - the adapter, behind the old wire format.**
Introduce a member-name to action-name adapter in `anthropic_computer_use.rs` so that
`(toolset_name="computer", name="left_click", input={...})` and
`(name="computer", input={"action":"left_click", ...})` both reach one implementation.
Fix `hold_key` units and `left_click_drag` `start_coordinate` here. Still declaring
`computer_20251124`, so nothing on the wire changes. Fully testable with unit tests, no API
calls.

**Phase 3b - group tool results into one user message.** No toolset involvement.
`anthropic.rs:1530-1545` accumulates per assistant turn instead of pushing per result, §3.2.
Valid and correct under the old format too, so it ships alone.

**Phase 4 - `toolset_name` plumbing, still on the old wire format.**
Add the optional field to `ApiContentBlock`, `ToolCall`, `ToolResult` and the four parse and
build sites. It serialises only when `Some`, so with the old declaration nothing changes on
the wire. This de-risks the cutover: the field is already threaded and tested before it
matters.

**Phase 5 - the cutover, behind a flag.**
New `ApiTool::Toolset` variant, new model list, beta header suppressed for toolset models,
skip-guard preserved as an explicit gate. Put it behind
`NEXT_PUBLIC_FEATURE_COMPUTER_TOOLSET_ENABLED` or the Tauri-store equivalent, defaulting
off, so a bad turn is one setting away from being reverted without a rebuild. Delete
`tool_batching_optimization` in this same commit, not earlier: it is still correct under
the old format.

**Phase 6 - batch hardening.**
Snapshot `ScalingInfo` per batch. Then the cooldown work from §4, coordinated with whoever
is fixing the double-counting.

**Phase 7 - flip the default, remove the flag.**
After a click-accuracy comparison against the pre-migration baseline (§7.3).

Before each phase: `cargo check --manifest-path src-tauri/Cargo.toml`, per CLAUDE.md.
None was run for this document.

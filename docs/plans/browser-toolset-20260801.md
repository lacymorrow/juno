# `browser_toolset_20260801`: contract, cost, and why Juno does not send it

**Decision (2026-09-22): Juno keeps its own browser tools. It does not send `browser_toolset_20260801` to any model.**

This is the same decision rule #582 applied to computer use, with the opposite outcome. Opus 5.5 rejects `computer_20251124` outright, so the computer toolset is forced. Nothing forces the browser toolset: Juno's browser tools are plain custom tools, and every model in the catalog accepts them. The toolset costs 3.8x the input-token overhead to buy a capability upgrade no model requires.

Everything below was verified against `api.anthropic.com` on 2026-09-22, six requests at `max_tokens: 1`. Where the docs and the wire disagree the wire wins; here they agreed.

## The measurements

All on `claude-opus-5-5`, one `"hi"` user turn, nothing else in the request.

| Request tools | `usage.input_tokens` | vs Juno today |
| --- | --- | --- |
| Juno's 5 browser tools (`browser_navigate`, `browser_extract_content`, `browser_interact`, `browser_get_current_url`, `browser_screenshot`) | 1,755 | baseline |
| `[{"type": "browser_toolset_20260801"}]` | 6,613 | **+4,858 (3.77x)** |
| Same, with the 4 optional members enabled | 7,496 | +5,741 (4.27x) |
| `computer_toolset_20260801` + `browser_toolset_20260801` | 10,849 | the stacked cost on Opus 5.5 |

For scale, #582 measured the *computer* toolset at 4,530 vs 2,152 input tokens, roughly 2x, and that was judged expensive enough that Juno sends it only where nothing earlier works. The browser toolset is worse on the same axis.

The cost is unavoidable per request. Tools are declared on every call, and changing the tool list mid-conversation invalidates the prompt cache, so this cannot be scoped to browser-only turns. On Opus 5.5 an agentic loop would carry 10,849 input tokens of tool overhead before any content, on every turn, whether or not a browser is involved.

## The contract, as established

Source: [Browser use tool](https://platform.claude.com/docs/en/agents-and-tools/tool-use/browser-use-tool), cross-checked against [Computer use tool](https://platform.claude.com/docs/en/agents-and-tools/tool-use/computer-use-tool#combine-with-other-tools) and [Tool use overview](https://platform.claude.com/docs/en/agents-and-tools/tool-use/overview).

**Type and gating.** `browser_toolset_20260801`, GA on the Claude API, **no beta header**. Declared bare:

```json
{ "type": "browser_toolset_20260801" }
```

A `name` field is a hard 400. Wire-verified:

> `tools.0.browser_toolset_20260801: name is not accepted on a toolset entry: the member names are fixed by the dated type, and the served tool keeps the family's bare name`

**Supported models** (docs): `claude-fable-5-1`, `claude-mythos-5-1`, `claude-fable-5`, `claude-mythos-5`, `claude-opus-5-5`, `claude-opus-5`, `claude-sonnet-5`, `claude-opus-4-8`. Gating is enforced per model. Wire-verified on a model outside that list:

> `'claude-haiku-4-5-20251001' does not support tool types: browser_toolset_20260801. Did you mean one of bash_20250124, ...`

Note the enumeration in that error lists no browser tag at all for Haiku, which is how `browser_toolset_20260801` was first spotted. The same supported-model list as `computer_toolset_20260801`, so Juno's existing `toolset_ga` column in `Provider::model_definitions()` would cover both axes if this is ever adopted.

**It is client-side.** Nothing runs on Anthropic's side. The application supplies the browser, dispatches every member call, owns tab identity, captures screenshots, and builds page reads. That makes Juno's chromiumoxide/CDP stack the right substrate in principle. This was the decisive question and the answer is favorable; cost is what rules it out, not architecture.

**31 member tools**, 27 enabled by default plus 4 off by default (`javascript_exec`, `file_upload`, `read_console`, `read_network`), toggled through `configs` keyed by member name.

- Navigation and capture: `navigate`, `screenshot`, `zoom`
- Pointer: `left_click`, `right_click`, `middle_click`, `double_click`, `triple_click`, `hover`, `left_click_drag`, `left_mouse_down`, `left_mouse_up`, `mouse_move`, `scroll`, `scroll_to`
- Keyboard and timing: `type`, `key`, `hold_key`, `wait`
- Page reading: `read_page`, `find`, `get_page_text`
- Forms and files: `form_input`, `file_upload`
- Diagnostics and scripting: `read_console`, `read_network`, `javascript_exec`
- Tabs: `new_tab`, `list_tabs`, `switch_tab`, `close_tab`

Every member takes an optional `tab_id`. Targets are one of `{"type": "coordinate", "x": <int>, "y": <int>}` or `{"type": "ref", "ref": "ref_2"}`; coordinates are viewport pixels, a separate frame from the computer toolset's desktop-screenshot pixels.

**Results.** Every member result echoes `"toolset_name": "browser"`, the same rule `unroute_toolset_call` exists for on the computer side. A result may carry only `text`, `image` (base64 PNG, `screenshot` and `zoom` only, not downscaled by the API), and a `browser_state` block carrying the full tab inventory plus optional `state_changes`. `browser_state` is never sent on an error result, at most one per result, exactly one tab marked `"active": true`.

**Batching.** One assistant turn may carry several member calls. They run sequentially, halt at the first failure, and every remaining call is answered with `is_error: true` and this exact text:

> `Not executed: an earlier action in this turn failed.`

## Why the wire-boundary trick from #582 does not carry over

#582 routes 17 computer member names into Juno's single internal `computer` tool, which already dispatches on an `action` field. That is a rename at the wire boundary, which is why it could land without touching the risk classifier, approval flow, cooldown, cursor overlay, or batch halt.

The browser toolset has no such shape to land on. Juno exposes 5 CSS-selector tools over a single `Page`; the toolset assumes an accessibility tree with a stable element-reference registry, a multi-tab inventory with application-assigned stable IDs, viewport-coordinate input dispatch, and a `browser_state` channel. Roughly two-thirds of the 31 members (`read_page`, `find`, `scroll_to`, `form_input`, `zoom`, `hover`, the mouse up/down pair, `hold_key`, and all four tab members) have no internal equivalent to route to. Adopting the toolset means building that executor first. It is a new subsystem, not a port.

## What would change the decision

Any one of these, in rough order of likelihood:

1. A model Juno wants to ship on stops accepting custom browser tools, the way Opus 5.5 stopped accepting `computer_20251124`. That is the forcing function the computer toolset had and this one does not.
2. The per-request overhead drops materially, or the tool block becomes separately cacheable so the cost is paid once rather than per turn.
3. Measured task success on browser work is bad enough that ref-based targeting is worth 4,858 tokens a turn. Juno's selector-based `browser_interact` is the weakest part of the current surface, and that is the honest case for adoption, but nobody has measured it.

If (3) is the motive, the cheaper experiment first is `configs`-trimming: the 4 optional members measured at roughly 220 tokens each, so a toolset cut down to `navigate`, `read_page`, `find`, `left_click`, `type`, `key`, `screenshot` would plausibly land near Juno's current 1,755. That is untested. It would still require the ref registry and the accessibility-tree read, which is most of the executor work.

## Filed, not fixed

Mapping the existing surface for this decision turned up three defects in Juno's browser tools. None are touched by this document and none are regressions; they are filed here so they are not lost.

1. **Five phantom tool names.** `browser_click`, `browser_type`, `browser_scroll`, `browser_get_content`, and `browser_form` exist in `src-tauri/src/constants/agent.rs`, in `tool_config.rs`'s default registrations, and in the generated `src/lib/constants.generated.ts`, with no `ToolDefinition` behind any of them.
2. **Three real tools are unreachable from the tool-config UI.** `browser_interact`, `browser_extract_content`, and `browser_get_current_url` are absent from `tool_config.rs`, so they cannot be toggled.
3. **Form fills escape the risk classifier.** `risk_classifier.rs` routes `browser_fill` / `browser_type` to `classify_form_fill_risk`, but the real shape is `browser_interact` with `action: "type"`, which falls through to `RiskLevel::Low`. Typing into a page form does not currently reach the approval gate that the classifier was written to enforce.

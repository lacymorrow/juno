# CLI providers spike results (LAC-7610, child 1 of LAC-7608)

Measured 2026-11-10 on zero (macOS, arm64) against `@github/copilot` **1.0.109** (installed from npm into a scratch prefix, `copilot --acp`) and the live Juno 73086 `juno_mcp` endpoint (HTTP, bearer header, loopback). Plan: `cli-providers.md`, section "The spike".

## Deviations from the brief, stated up front

- **The harness is a throwaway Node ACP driver (about 60 lines), not a Rust harness.** Rust compiles in CI only (root CLAUDE.md), and the questions are about the wire, which a JSON-RPC driver measures just as well. No Rust harness exists, so nothing was compiled and no dependency was added. The driver lives in `/tmp`, not in the repo.
- **`agy` was not measured.** There is no `agy` binary on this Mac or in the npm registry under that name. `Antigravity.app` is installed but is the IDE; it ships no `agy acp` entry point. Row 2 (agy) is **unmeasured**, not no-go. Needs either the install command from Lacy or the row dropped.
- **Signed-out behaviour was not measurable.** Copilot is authenticated on this account (probably from the `gh` keyring; `~/.copilot` logs from the first launch said "no authenticated GitHub host", yet prompts succeed). A signed-out `session/new` needs a machine with no GitHub credential. Note: `session/new` is not where it would fail on a signed-in box, and `authMethods` offers one method, `copilot-login`, with `_meta.terminal-auth` = `copilot login`.
- **Privacy note.** Measurement 2 sent one real screen capture request through GitHub Copilot's model. The model chose `cursor_position` and answered "Ghostty"; no screenshot image was returned to it.
- `agent-client-protocol` pin: crates.io serves **5.1.0** (published 2026-11-09; previous 5.0.0 on 2026-11-02). The plan's reading is confirmed as of today. Nothing was added to `Cargo.toml`.

## Copilot (ACP) per measurement

| # | Question | Result |
|---|---|---|
| 1 | `initialize` | `protocolVersion: 1`; `mcpCapabilities.http: **true**` (and `sse: true`); `promptCapabilities.image: **true**` (`audio: false`, `embeddedContext: true`); `loadSession: true`; `sessionCapabilities`: `close`, `list`; `authMethods`: `[copilot-login]`. `agentInfo` Copilot 1.0.109. **Not a no-go.** `initialize` took 0.8 to 1.5 s. |
| 2 | HTTP MCP + bearer, `computer` round trip | **Yes.** `session/new` with `{type:"http", name:"juno", url, headers:[{name:"Authorization", value}]}` is accepted (0.8 to 1.0 s). Copilot surfaces the tool as `juno-computer`; the call reached `run_computer_action` and returned `Cursor at (1198, 542).`. First `tool_call` 3.0 s after `session/prompt` (this was a skill load, `agent-browser`, not Juno's tool); first call to `juno-computer` 7.1 s of run clock (about 4.7 s after the prompt); first `agent_message_chunk` 3.0 s (plain text prompt) to 4.4 s (tool prompt) after the prompt. The model picked `cursor_position` and not `screenshot`; an image-return round trip was not forced and is the one gap here. |
| 3 | `session/request_permission` | **Yes, and it asks on everything, reads included.** It asked before `computer` (a read-shaped call) and before a shell `touch`. Options: `allow_once`, `allow_always`, `reject_once`. Replying `reject_once` produced `tool_call_update` `status: failed`, `{"message":"The user rejected this tool call.","code":"rejected"}`, the file was **not** created, and the model's next text was "Info: Operation cancelled by user". So the denial reaches the model, as text, but the wording is Copilot's, not Juno's `cli_approval.rs` text. `cancelled` outcome was not separately tested. Note Juno's `permissions_first` gate runs inside `juno_mcp`, so Juno would be asked twice per tool unless the client auto-allows `juno-*` on the ACP side (the client owns that choice). |
| 4 | `session/cancel` latency | **10, 14, 22 ms** (n=3, cancelled 0.8 s into a streaming reply; response `stopReason: "cancelled"`). Bar is under 200 ms. **Pass.** |
| 5 | Model selection | Launch flag: `--model <id>` (`auto` lets Copilot pick). A bad id fails fast: `Execution failed: Model "bogus-x" is not available.`. `session/new` returns `configOptions` for `mode` (agent/plan/autopilot) and `allow_all` (on/off) and **no model list**; `/model` is interactive only and was not captured. So the Copilot model list in `types.rs` cannot be generated from ACP; it needs a hand-kept list or an `auto`-only row. `--reasoning-effort` exists (none to max). Signed-out error: not measured (see deviations). |
| 6 | Cost, native vs ACP | See below. |

### Not gating

- **Cold start to first text**, ACP, simple prompt: about 1.0 s `initialize` + 0.85 s `session/new` + 3.2 to 4.4 s to first chunk = **about 5.3 to 6.3 s**. Compare Claude 3 to 5 s and Codex 2.5 to 4.3 s: Copilot is the slowest of the three cold. With a warm spare (init and `session/new` already paid) the figure is about 3.2 to 4.4 s, which is the same range as a warm Claude. First chunk time is dominated by Copilot's model call (it did a 1.3 s reasoning step before text on the plain prompt).
- **Token overhead.** An empty prompt ("say hi", no MCP server) costs **14,892 input tokens** (1,280 cached) on Copilot's own system prompt and tools. The same account with Juno mounted and the screenshot prompt cost **66,906 input tokens**, of which about 26 k is the `agent-browser` skill Copilot pulled in from the user's `~/.agents/skills`. The Juno tool schema's own share was not isolated; a clean run needs `HOME` pointed at an empty config. The user's skills and MCP config leak into the ACP session by default, which Juno should decide on deliberately (Claude path has `load_account_mcp`, default on).

## Native Copilot path, measured for the comparison

`copilot -p "<prompt>" --output-format json --additional-mcp-config @<file> --allow-tool=juno --no-ask-user` also mounts Juno's server and completed the same screenshot prompt in **16.1 s** wall (spawn to exit). The JSONL stream carries **25 distinct event types** in that one run (`session.start`, `assistant.message_delta`, `tool.execution_start`, `tool.execution_complete`, `result`, `session.usage_checkpoint`, and others). The plan's "six event types wire-verified" is the subset a provider needs, not the stream's size. `@file` keeps the bearer token out of argv, as `juno_mcp::mcp_config` already does for Claude.

## Measurement 6: cost, two numbers and a recommendation

Engineer's estimate (the CTO was paused when this was filed; CTO may overwrite). Both counted as the nine shared registration files plus the provider module.

| | Provider module | Shared files | Dependency | Security stage | Total |
|---|---|---|---|---|---|
| Native `-p --output-format json` | about 1 day (spawn, parse 6 needed events, `--resume` for follow-ups, kill on Escape) | 9 files, about 1 day | none | none | **about 2 days (S to M)** |
| ACP client (`agent-client-protocol` 5.1.0) | about 3 days (persistent process, reader/writer tasks, `session/update` routing, permission round trip, cancel, auth-required mapping) + a fake-agent test harness, about 1 day | 9 files, about 1 day | adds a crate | yes, reads `Cargo.toml` | **about 5 days (M to L)** |

What the extra 3 days buy: sub-25 ms cancel (native cancels by killing the process), a permission round trip through the approval sheet (native is `--allow-tool` or nothing; Juno's own `juno_mcp` gate still prompts), a process that stays warm across follow-ups (native pays about 16 s wall or at least a full spawn per turn unless `--resume`), and reuse for row 2 and beyond.

**Recommendation: ACP is not clearly more expensive than native by enough to say no to it, and the measurements do not say yes either.** The honest call:

- If Copilot is the only row Lacy will ever ship, **native is clearly cheaper** (2 days vs 5, no dependency, no Security review) and loses nothing a user feels except cancel speed.
- The plan's payback case rests on rows 2 and later reusing `acp_cli.rs`. Those rows (agy, Cursor) are parked on LAC-7609 and agy is unmeasured here.
- So: **recommend native for Copilot now; ACP returns with row 2**, exactly the CEO's amendment on LAC-7609. Measurement 1 and 4 say ACP would work well when it does return (HTTP MCP true, cancel 10 to 22 ms), so nothing here closes that door.

## Go / no-go per row

| Row | Verdict |
|---|---|
| Copilot via ACP | **Go** technically (http true, round trip works, cancel 10 to 22 ms). |
| Copilot via native `-p` | **Go**, and recommended first because it is cheaper (measurement 6). |
| agy via `agy acp` | **Unmeasured.** No `agy` binary installable here; needs an install source. Not a no-go. |

## Gaps to close before child issue 2 starts

1. agy: where does `agy` come from? Lacy or the CTO names the install.
2. Signed-out `session/new` error: run on a machine or `HOME` with no GitHub credential and no `GH_TOKEN`.
3. Force a real `screenshot` call so an image content block round trip through ACP is observed.
4. Isolate Juno's tool-schema token cost with `HOME` pointed at an empty config.
5. A Copilot model list for `types.rs` (interactive `/model`).

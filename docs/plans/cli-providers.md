# CLI providers: one ACP adapter, Copilot first, Antigravity second

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Juno runs on the coding-agent CLI a person already pays for, with no API key typed into Juno, starting with GitHub Copilot CLI.

**Architecture:** One new provider, `Provider::Acp`-shaped but exposed as one `Provider` variant per CLI, that drives any Agent Client Protocol (ACP) agent as a subprocess over JSON-RPC on stdio. Juno's loopback MCP computer tool (`juno_mcp.rs`) is passed at `session/new` as an HTTP MCP server, exactly as the Claude CLI and Codex providers mount it today. Per-CLI code shrinks to one table row: binary name, launch arguments, sign-in probe, model list. Claude Code and Codex stay on their shipped native paths.

**Tech Stack:** Rust (Tauri backend), `agent-client-protocol` crate 5.1.0 (Zed, Apache-2.0, 6.1M downloads, updated 2026-11-09), existing `juno_mcp.rs`, existing `codex_session.rs` process pattern.

**Status:** scoping plan, 2026-11-10. Tracked in LAC-7608. Written by the Product Manager; CTO owns the build. Nothing below is started.

---

## Decision, in five lines

1. **Build one ACP client, not one parser per CLI.** Copilot, Antigravity, Gemini CLI, Cursor, OpenCode, Qwen Code and Kiro all ship an ACP server mode. Codex and Claude reach ACP only through third-party adapter binaries, and both already have native Juno providers, so they stay native.
2. **Row 1 is GitHub Copilot CLI** (`copilot --acp`). Largest installed base of any qualifying CLI, wire-checked signed in on this Mac today.
3. **Row 2 is Antigravity CLI** (`agy acp`), **not Gemini CLI.** Google moved every free, Google AI Pro and Google AI Ultra account off `gemini` to `agy` on 2026-06-18. `gemini` now serves Gemini Code Assist Standard and Enterprise licences only. For a person on a Mac, the Google subscription CLI is `agy`.
4. **"Azure AI Foundry" is zero Juno code.** It is Copilot CLI's bring-your-own-key feature, configured inside Copilot (`/model`, saved to `~/.copilot/providers.json`). Juno spawns `copilot`, Copilot reads its own config. Juno adds no field, no toggle, no provider.
5. **A spike comes first and can say no.** Two to three days against `copilot --acp` and `agy acp`, measuring the five things in "The spike". If Copilot's ACP preview cannot mount an HTTP MCP server, Copilot ships on its native `-p --output-format json` path instead and the ACP decision is revisited with the measurement in hand.

---

## The three criteria, and who meets them

The CEO's rule (LAC-7608, 2026-11-10): a CLI qualifies only with all three of
(a) a non-interactive mode with streamed structured output,
(b) MCP client support, so Juno's computer tool can be mounted,
(c) auth from the person's existing subscription, no API key typed into Juno.

Everything in the table was read from live vendor docs on 2026-11-10, never from memory (LAC-3106). "Wire" means run on this Mac today; "docs" means the flag is documented but was not run here.

| CLI (binary) | (a) streamed structured output | (b) MCP client, HTTP + headers | (c) subscription sign-in | ACP server | Installed base | Verdict |
|---|---|---|---|---|---|---|
| **GitHub Copilot CLI** (`copilot`, 1.0.109) | **Wire.** `copilot -p "<q>" --output-format json` emits JSONL: `session.start`, `assistant.message_delta` (`deltaContent`), `assistant.message` (`content`, `toolRequests[]`), `assistant.turn_end`, `session.usage_checkpoint` (`premiumRequests`), `result` (`exitCode`, `usage`). 25 lines for a one-word answer, stderr empty, exit 0. | **Docs.** `--additional-mcp-config <json or @file>` adds a server for one session. Remote server fields: `type` (`http`, `sse`), `url`, `tools` (required, `["*"]` or a list), `headers` (optional, variable expansion). `--allow-tool juno` pre-approves the server. Workspace `.mcp.json` is deliberately ignored in `-p` mode. | **Wire.** `copilot login` (browser OAuth, token in the system credential store). "Available with all Copilot plans"; hosted models "need a Copilot subscription". This Mac answered without a prompt. | `copilot --acp` (docs, "public preview"). Registry entry `github-copilot-cli` 1.0.109, args `--acp`. | 4.7M paid subscribers (Microsoft, 2026-01-28); ~50M cumulative users (FY26 Q4 call, secondary). | **Build. Row 1.** |
| **Antigravity CLI** (`agy`) | **Docs.** `agy -p --output-format stream-json`: `init`, `step_update` (`text_delta`, `tool_name`, `usage`), `result`. `--input-format stream-json` keeps one process for many turns, the shape Juno's persistent Claude path already speaks. `--dangerously-skip-permissions`, `--model`, `--continue`, `--print-timeout 0`. Exit `authentication required` when signed out. | **Docs.** Remote servers "must use the Streamable HTTP transport": `serverUrl` (or `url`) plus `headers`, in `~/.gemini/config/mcp_config.json` or `<cwd>/.agents/mcp_config.json`. Juno spawns in a scratch cwd holding that file. | **Docs.** Sign in with a Google account: free tier, Google AI Pro, Google AI Ultra. Tokens in Apple Keychain. `agy acp --login` for headless sign-in. Business accounts cannot buy overage credits. | `agy acp` (docs). Registry entry `antigravity-acp` 1.4.5, author Google LLC, binary `agy_acp_server.par`. | Unpublished. The audience is every Google AI Pro/Ultra holder plus the free tier, all moved here from Gemini CLI on 2026-06-18. | **Build. Row 2.** Replaces Gemini CLI in the CEO's order. |
| Gemini CLI (`gemini`, 0.67.0; 0.46.0 on this Mac, brew-deprecated, LAC-5355) | Wire (0.46.0 help): `-p`, `--output-format stream-json` (`init`, `message`, `tool_use`, `tool_result`, `error`, `result`). | Docs: `httpUrl` + `headers` in `settings.json`; `--allowed-mcp-server-names`; `gemini mcp add -t http -H`. | **Fails for Juno's audience.** README (main, 2026-11-10): "No free tier or Google AI plans ... moved to Antigravity CLI on June 18, 2026". Enterprise Code Assist licences only. | `gemini --acp` (wire, 0.46.0 help) | Enterprise seats only. | **Drop.** Not a consumer CLI any more. |
| Cursor CLI (`agent`) | Docs: `agent -p --output-format stream-json` (NDJSON; `system`/`init` carries `apiKeySource: env\|flag\|login`, `session_id`, `model`). | Docs: `~/.cursor/mcp.json` or `.cursor/mcp.json`, `url` + `headers` (Streamable HTTP), `--approve-mcps`. | Docs: `agent login` (browser, Cursor account); `agent status` is a clean probe. `CURSOR_API_KEY` exists and is not for Juno. | `agent acp` (docs, "advanced, hidden command"). Registry entry `cursor` 2026.11.05. | No published user count; "millions of developers"; ~$4B annualized revenue (Forbes, 2026-06-08, secondary). | **Row 3, after rows 1 and 2 ship** and the per-row cost is measured. |
| OpenCode (`opencode`, 1.18.34 here) | Wire: `opencode run --format json`. | Docs: `type: remote`, `url`, `headers`, `oauth: false` in `opencode.json`. | Docs: signs in with ChatGPT Plus/Pro, GitHub Copilot (device OAuth, Copilot Free accepted), Anthropic. It resells subscriptions Juno reaches directly. | `opencode acp` (wire, help). Registry entry `opencode` 2.2.7. | ~200k GitHub stars (2026-08, secondary); no user count. Named argument: `lash` is Lacy's OpenCode fork. | **Defer.** A table row later if a named installed base appears. |
| Qwen Code (`qwen`) | Docs: `qwen -p`. | Docs: ACP. | Qwen OAuth free tier (not verified here). | `qwen --acp` (registry 0.27.1) | Unknown. | Defer. |
| Kiro CLI (`kiro-cli`) | Headless with an API key (docs). | Docs: `mcpServers`. | Kiro Pro subscription (Amazon); headless path documented with a key, which fails (c) as read. | `kiro-cli acp` (registry 3.0.0) | Unknown. | Defer; re-read (c) when a row is cheap. |
| Goose, Aider, Amp | Yes | Yes | **Fail (c).** API key or pay-as-you-go. | Goose yes | n/a | Out. Non-goal: new API-key providers. |
| Codex CLI (`codex`) | Shipped (`codex exec --json`, `codex app-server`). | Shipped (`-c mcp_servers.juno.*`). | Shipped (ChatGPT plan). | Only via `@agentclientprotocol/codex-acp` (third-party adapter). | Shipped. | **Keep native.** |
| Claude Code (`claude`) | Shipped. | Shipped. | Shipped. | Only via `@agentclientprotocol/claude-agent-acp` (adapter). | Shipped. | **Keep native.** |

---

## Why one ACP adapter (the CEO's question)

The CEO asked: before per-CLI code, does one adapter over a shared protocol cover two or more at once? **Yes, and the two it covers first are the two at the top of the order.**

What ACP v1 gives Juno, verified against the protocol docs on 2026-11-10:

| Need | Today (per CLI) | ACP (once) |
|---|---|---|
| Streamed text and thinking | A parser per CLI's event vocabulary (`claude_cli.rs` 4,047 lines, `codex_cli.rs` + `codex_session.rs` 2,440 lines) | `session/update` with `agent_message_chunk`, `agent_thought_chunk`, `tool_call`, `tool_call_update`, `usage_update`; `stopReason` `end_turn` or `cancelled` |
| Mount Juno's computer tool | `--mcp-config` (Claude), `-c mcp_servers.juno.*` (Codex), a config file in a scratch cwd (agy), `--additional-mcp-config` (Copilot) | `session/new` `mcpServers: [{type: "http", name, url, headers: [{name, value}]}]`. Agent advertises `mcpCapabilities.http` at `initialize`. The bearer token travels inside JSON on stdin, never on argv. |
| Ask the person before a consequential tool | `--permission-prompt-tool mcp__juno__approve` (Claude only, `cli_approval.rs` 1,801 lines); `approvalPolicy: never` (Codex) | `session/request_permission` from agent to client. One handler, routed to the same `tool-approval-request` sheet. |
| Stop | kill, `control_request/interrupt` (Claude), `turn/interrupt` (Codex) | `session/cancel` |
| Warm process, many turns | Bespoke in both shipped providers | One process, `session/new` per conversation, `session/prompt` per turn |
| Signed-out detection | `claude auth status --json`; read `~/.codex/auth.json` | `initialize` returns `authMethods`; a `session/new` that needs sign-in fails with the auth-required error. Same probe for every row. |

What it costs:

- **One new dependency**, `agent-client-protocol` 5.1.0. New dependency means a Security review stage (CLAUDE.md, "new deps"). The crate is the one Zed ships in production and the registry's reference implementation.
- **Model selection is not in ACP v1.** `session/set_mode` switches modes, not models; a model-config option is an open RFD. Each row therefore names its model at launch (`copilot --acp --model <id>`, `agy acp --model <id>` are the candidates the spike confirms) and a model change restarts the process. Copilot's `auto` is its own default and is the recommended entry.
- **ACP v2 is a draft** (2026-07-20). v1 peers "will remain common for some time"; the crate speaks v1 and the plan pins v1.
- **Maturity varies by agent.** Copilot's ACP is "public preview"; Cursor's is "advanced, hidden". The spike measures Copilot; Cursor waits.
- **A stdio-only agent cannot mount Juno's tool.** Every ACP agent must support stdio MCP; HTTP is optional. Juno has no stdio MCP shim in the bundle (`juno-cua` does not ship). An agent whose `initialize` reports `mcpCapabilities.http: false` is out until it does. The spike reads this field first.

The fallback, written down now so it is not improvised later: if Copilot's ACP cannot pass the spike's five measurements, Copilot ships on its native path (`copilot -p --output-format json --additional-mcp-config @<file> --allow-tool juno --no-ask-user -s`, one process per turn, `--resume <sessionId>` for follow-ups), using the JSONL vocabulary wire-checked above, and Antigravity ships on its native stream-json path. That is two more parsers, which is what the ACP adapter exists to avoid, so the spike result is a real decision point and the CTO makes it.

---

## Azure AI Foundry: what it is, what Juno does

Copilot CLI's bring-your-own-key (BYOK) lets a Copilot user route the agent to a model they host: OpenAI, Azure OpenAI, Microsoft Foundry, Anthropic, Ollama, vLLM. Verified from `copilot help providers`, `copilot help environment` and the Copilot SDK BYOK doc on 2026-11-10:

- **The current path is inside Copilot.** `/model` adds a provider and saves it to `~/.copilot/providers.json` (`COPILOT_PROVIDERS_CONFIG`). "GitHub doesn't charge for custom models from your enterprise or models you bring yourself."
- **The environment path is legacy** (`copilot help providers-env`): `COPILOT_PROVIDER_TYPE` (`openai` | `azure` | `anthropic`), `COPILOT_PROVIDER_BASE_URL`, `COPILOT_PROVIDER_API_KEY`, `COPILOT_PROVIDER_BEARER_TOKEN`, `COPILOT_PROVIDER_WIRE_API`, `COPILOT_PROVIDER_AZURE_API_VERSION`, `COPILOT_PROVIDER_HEADERS`. Foundry `/openai/v1/` endpoints use `type: openai`; native `*.openai.azure.com` endpoints use `type: azure`.
- **Juno does nothing.** The Copilot subprocess reads the person's own Copilot config. No Juno setting, no field, no provider id. A person who configured Foundry in Copilot yesterday is on Foundry in Juno today.
- **One caveat Juno must say out loud.** BYOK models are text-only unless `COPILOT_PROVIDER_VISION=true`. Juno's computer tool returns screenshots. A Foundry deployment that cannot see the screen cannot drive the desktop. The error state below covers it.

---

## What the person sees

The pattern is the Codex card in Settings → Providers, which is the Claude card without the e-mail line. Nothing new is invented.

**Provider list.** A CLI provider is listed only when its binary is installed (the Codex rule, LAC-4125; Claude keeps its always-listed history). Signed out greys it with the badge words below. Binaries are looked up on `PATH` plus `~/.local/bin`, because the `agy` installer puts it there and Juno launched from the Dock does not have the shell's `PATH`.

| State | Copilot row | Antigravity row |
|---|---|---|
| Installed, signed in | Card title "No API key needed". Body: "Juno is using your GitHub Copilot plan through Copilot CLI. Nothing else to set up." Footer: "Nothing to enter. Juno talks to Copilot on this Mac." | "Juno is using your Google account through Antigravity CLI. Nothing else to set up." Footer: "Nothing to enter. Juno talks to Antigravity on this Mac." |
| Installed, signed out | Badge "Sign in to Copilot". Card title "Copilot CLI is not signed in". Body: "Run `copilot login` in your terminal, then come back to this window." | Badge "Sign in to Antigravity". Body: "Run `agy` once in your terminal and sign in, then come back to this window." |
| Not installed | Not listed. | Not listed. |

**Model picker.** Shared `AssistantModelPicker`. Copilot's entries: `auto` (recommended; Copilot picks within the plan) plus the plan's named models the spike reads from `/model`. Antigravity's entries: the models `agy --model` accepts, read in the spike. All `Price::Subscription`, `ComputerUse::FunctionTools`, logo id `github` and `google` in `model-selector.tsx`.

**Default provider.** `default_selection.rs` already walks an ordered list and picks the first signed-in CLI when nobody chose and nothing has a credential. The list becomes Claude, Codex, Copilot, Antigravity. A person with only Copilot signed in launches Juno and is on Copilot without touching Settings. A choice they make themselves is never overruled.

**Setup (onboarding).** Unchanged. It offers "Use your Claude subscription" or a key. The default-selection rule covers Copilot and Antigravity with no new row.

**Advanced.** Max tokens and temperature stay hidden for CLI providers, as they are for Claude and Codex.

**The first ten seconds.** Someone with Copilot CLI signed in and no Anthropic key opens Juno. The bar is ready. They say "open Mail and start a new message". Juno's cursor moves. Settings → Providers says "Juno is using your GitHub Copilot plan through Copilot CLI." They typed nothing.

**Loading.** The active row's ACP process starts at launch and again after each new conversation adopts it, the warm-spare rule the Claude path already uses, so the first turn does not pay the 3 to 5 s cold start the shipped CLIs show. Until first text arrives the bar shows its "Working…" placeholder, as it does for every provider. The spike measures the cold start so the warm start is a number, not a hope.

**Other MCP servers.** Copilot's built-in GitHub MCP server is off for Juno's rows (`--disable-builtin-mcps`): it loads repository tools into a desktop assistant and took 2 s of the 6 s wire-checked session. The person's own configured servers follow the existing "Load account MCP connectors" advanced setting, default on, exactly as on the Claude path.

**Empty state.** No CLI installed and no key: unchanged, `NO_PROVIDER_MESSAGE` ("Juno has no AI to talk to yet. Sign in to Claude Code, or add an API key in Settings > Providers."). Extend the sentence to name the CLIs that are installed but signed out, if any, nothing more.

**Error states, each one sentence in the bar and the log line that explains it.**
- Signed out mid-session: "Copilot signed out. Run `copilot login`, then try again." Provider marked `needs_sign_in`, the badge updates.
- The ACP agent does not advertise `mcpCapabilities.http`: "This version of Copilot can't use Juno's tools yet. Update Copilot, or pick another provider." Provider stays listed, unavailable, with the reason.
- The model refuses images (BYOK without vision): "This model can't see the screen. Pick another model, or run `copilot` and `/model`." The turn ends; nothing retries blindly.
- Quota exhausted: the agent's own text is surfaced verbatim, the way LAC-6453 taught us a spent window hangs rather than errors on Claude, so the idle timeout (`CLI_IDLE_TIMEOUT`, bounds silence not length) applies unchanged.

---

## What we remove

Written down, per the Jobs Standard.

- **Gemini CLI as a provider.** Enterprise-only since 2026-06-18. Antigravity takes its place.
- **Any Azure AI Foundry UI in Juno.** No endpoint field, no key field, no "Azure" provider. It is Copilot's config.
- **Per-CLI event parsers for new CLIs.** ACP's `session/update` is the one vocabulary. The two shipped parsers stay for the two shipped providers.
- **Per-CLI permission plumbing.** `session/request_permission` replaces `--permission-prompt-tool`, `--allow-tool`, `--dangerously-skip-permissions` for every ACP row.
- **Setup-screen rows per CLI.** Default selection does the work silently.
- **Providers listed when their binary is absent.** The list does not grow with every CLI in the world.
- **A model-parameters pane for CLI providers.** Already hidden; stays hidden.
- **Trial-credit, cost and telemetry changes.** `billing_model_id` gets `copilot-cli:<model>` and `antigravity-cli:<model>` keys and `prices.json` gets `$0.00` subscription rows, nothing else.

---

## The spike (child issue 1, go/no-go)

A Rust test harness in a branch, no UI, no shipped code. It answers five questions against `copilot --acp` (1.0.109) and `agy acp`, on this Mac, with Juno's real `juno_mcp` endpoint. Each answer is a number or a yes/no in `docs/plans/cli-providers-spike.md`.

1. `initialize` response: `protocolVersion`, `agentCapabilities.mcpCapabilities.http`, `promptCapabilities.image`, `authMethods`. **No-go for a row if `http` is false.**
2. `session/new` with Juno's HTTP MCP server and bearer header: does the agent list the `computer` tool, and does one `session/prompt` ("take a screenshot and tell me the frontmost app") round-trip through `run_computer_action`? Time to first `agent_message_chunk`, and to the first `tool_call`.
3. `session/request_permission`: does the agent ask before a write-shaped tool, and does a `cancelled`/denied answer reach the model as text, as `cli_approval.rs` guarantees today?
4. `session/cancel` latency (Codex measured 8 ms on `turn/interrupt`; the bar is "under 200 ms").
5. Model selection: which launch flag pins the model, and what `/model` lists for this account (Copilot) and `--model` accepts (agy). Signed-out behaviour: the exact error on `session/new`.

Also measured, not gating: cold start to first text versus the shipped Claude (3 to 5 s) and Codex (2.5 to 4.3 s) paths; token overhead of Juno's tool schema as the agent sees it.

Effort: S, two to three days including CI. Output: the spike doc and a one-line go/no-go per row posted on LAC-7608.

---

## Build sequence (child issues 2 to 4)

Each ships whole before the next starts. Effort: S under a day, M a few days, L a week or more.

### Child issue 2: `acp_cli.rs` + the Copilot row (M to L)

**Files**
- Create: `src-tauri/src/agent/providers/acp_cli.rs` (the client: process, writer and reader tasks, `initialize`, `session/new`, `session/prompt`, `session/update` routing to Juno's stream events, `session/request_permission` to the approval sheet, `session/cancel` on Escape, auth-required mapping)
- Create: `src-tauri/src/agent/providers/acp_rows.rs` (the table: one `AcpRow` per CLI with `provider`, `binary`, `launch_args(model)`, `sign_in_hint`, `display_name`, `description`; Copilot is the first row)
- Modify: `src-tauri/Cargo.toml` (add `agent-client-protocol = "5.1"`; Security review reads this line)
- Modify: `src-tauri/src/agent/providers/types.rs` (`Provider::CopilotCli`; `from_str` accepts `copilot_cli`, `copilot-cli`, `copilotcli`; `id` `"copilot_cli"`; `display_name` `"Copilot CLI"`; `description` `"Use your local Copilot CLI installation with your GitHub Copilot plan, no API key required"`; `model_definitions` with `auto` recommended and the spike's list; `default_model` `"auto"`; the four tests that enumerate providers gain the variant)
- Modify: `src-tauri/src/agent/providers/factory.rs` (list the provider only when `acp_rows::copilot().installed()`; `is_available` and `needs_sign_in` from the row's last known sign-in, the Codex pattern; both `create_brain` arms)
- Modify: `src-tauri/src/agent/providers/default_selection.rs`, `startup_default.rs`, `settings/reset.rs` (append the `CliCandidate` after Codex; the module doc's "Claude first, Codex second" sentence gains "Copilot third")
- Modify: `src-tauri/src/agent/providers/config.rs` (default `ProviderConfig` entry), `commands/providers.rs`, `providers/mod.rs`
- Modify: `src-tauri/src/cost_log.rs` (`billing_model_id` → `copilot-cli:{model}`; the test at line 652 gains a case), `scripts/pricing/prices.json` (`copilot-cli:auto` at 0.00 with the subscription note)
- Modify: `src-tauri/src/connectivity.rs` (`Answerer::Copilot`, name `"Copilot"`), `src-tauri/src/agent/router.rs` (both provider lists in tests)
- Modify: `src/components/settings/sections/AIProviderSettings.tsx` (the card copy above; the footer ternary), `src/lib/provider-status.ts` (`"Sign in to Copilot"`), `src/components/ai-elements/model-selector.tsx` (`copilot_cli` → `github`), `src/types/settings.ts` (the `needs_sign_in` comment stops saying "Only the Claude CLI")
- Test: `src-tauri/src/agent/providers/acp_cli.rs` `#[cfg(test)]` (a fake ACP agent on stdio: asserts `initialize` params, `mcpServers` shape with the bearer header, update routing, permission round-trip, cancel), `acp_rows.rs` tests (every row's `launch_args` contains its ACP flag and never contains the token), the enumeration tests above, `src/components/settings/__tests__/` for the card copy per state
- Media: `docs/changelog/media/<PR>/providers-copilot-signed-in.png`, `providers-copilot-signed-out.png`

**Steps** (expanded to bite size after the spike fixes the launch flag and model list; the shape is the Codex PR #698):
- [ ] Failing test: fake agent, `initialize` → assert `protocolVersion: 1` and `clientCapabilities` sent.
- [ ] Implement `AcpClient::spawn(row, model)` and `initialize`.
- [ ] Failing test: `session/new` carries `mcpServers[0] == {type: http, name: juno, url, headers: [Authorization]}` and `cwd` is the home directory (LAC-4057 reasoning applies).
- [ ] Implement `new_session`.
- [ ] Failing test: three `agent_message_chunk`s become three `agent-text-stream` emits in order; one `tool_call` becomes a tool event with the tool name; `stopReason: end_turn` ends the turn.
- [ ] Implement `prompt` and the update router; reuse `ClaudeCliBrain`'s TTS funnel the way `codex_cli.rs` does.
- [ ] Failing test: `session/request_permission` with a send-shaped tool opens `ask_person`; a denial answers `cancelled`.
- [ ] Implement the permission handler on top of `cli_approval::verdict_for` and `ask_person`.
- [ ] Failing test: cancel token → `session/cancel` sent → `stopReason: cancelled` → turn ends under 200 ms in the fake.
- [ ] Register `Provider::CopilotCli` across the files above; `cargo fmt`; push a draft PR; `gh pr checks --watch`.
- [ ] Settings copy and badge; screenshots into `docs/changelog/media/<PR>/`.
- [ ] `scripts/juno-build.sh`, run the first-ten-seconds script on this Mac, attach the recording.

**Gates:** `Security review: (new dependency `agent-client-protocol`, a subprocess holding a loopback bearer token, a sign-in probe)`, `Code review:`, `QA:` with Lacy's approval stage, because the Settings copy is customer-read copy (Rule 32, judgment). The last gate owns the merge (Rule 33).

### Child issue 3: the Antigravity row (S to M)

**Files**
- Modify: `acp_rows.rs` (row: binary `agy`, lookup includes `~/.local/bin`, args `["acp", "--model", <model>]` pending the spike), `types.rs` (`Provider::AntigravityCli`, id `antigravity_cli`, display `"Antigravity CLI"`, models from the spike), every enumeration site from child issue 2, `cost_log.rs` (`antigravity-cli:{model}`), `prices.json`, `connectivity.rs` (`Answerer::Google` is the right name; "Gemini" already exists for the API provider), `AIProviderSettings.tsx`, `provider-status.ts` (`"Sign in to Antigravity"`), `model-selector.tsx` (`google`)
- Test: the row test; the enumeration tests; card copy test
- Media: `providers-antigravity-signed-in.png`

**Steps**
- [ ] Failing row test; add the row; enumeration tests red → green; fmt; draft PR; CI.
- [ ] Copy and badge; screenshot; `juno-build.sh`; run the first ten seconds with `agy` signed in.

**Gates:** `Code review:`, `QA:` (verification only, QA signs and merges; the copy pattern was approved in child issue 2).

### Child issue 4: the Cursor row (S)

Only after rows 1 and 2 are live and the per-row cost is known. `agent acp` is a hidden command; the issue opens with a one-day re-check of the Cursor docs. Same files as child issue 3. **Gates:** `Code review:`.

---

## Open questions the spike settles

1. Does `copilot --acp` honour `--model`, and does the ACP session report the resolved model (`session.model_change` does on the JSONL path)?
2. Does Copilot's ACP mode read `~/.copilot/mcp-config.json`, so the "Load account MCP connectors" setting means something for that row? If not, the setting is hidden for Copilot rather than shown and ignored.
3. Does `agy acp` need the scratch-cwd `.agents/mcp_config.json`, or does `session/new mcpServers` suffice? ACP should suffice; the file is the fallback.
4. Where does `agy` keep its sign-in marker (Apple Keychain per docs), and is "run `agy` once" the right instruction or does `agy acp --login` open the browser on its own?
5. Does the ACP `tool_call` for Juno's `computer` tool carry the `toolset_name`-style echo Juno's own streams expect, or does the router map it?

---

## The insanely-great gate

Run 2026-11-10 on this document: SHIP after two fixes, a loading state for the process cold start and a decision on Copilot's built-in MCP server, both applied above. Run it again on each child's PR before it merges. Demo test items: first ten seconds written above; removal list above; one primary action per screen (the Settings card has none to press when signed in, one link when not); empty, loading and error states above; screenshot or recording attached to each PR; DRI: CTO builds, Product Manager owns this plan, Lacy approves the Settings copy.

---

## Sources (read 2026-11-10)

- GitHub Copilot CLI programmatic reference: https://docs.github.com/en/copilot/reference/copilot-cli-reference/cli-programmatic-reference
- GitHub Copilot CLI command reference (MCP server fields, `--acp`, `--additional-mcp-config`): https://docs.github.com/en/copilot/reference/cli-command-reference
- About Copilot CLI ("available with all Copilot plans"): https://docs.github.com/copilot/concepts/agents/about-copilot-cli
- Copilot SDK BYOK: https://github.com/github/copilot-sdk/blob/main/docs/auth/byok.md
- `copilot help providers`, `copilot help environment`, `copilot login --help`, and `copilot -p ... --output-format json` run on this Mac, Copilot CLI 1.0.109
- Gemini CLI README status notice: https://github.com/google-gemini/gemini-cli
- Gemini CLI transition post: https://developers.googleblog.com/an-important-update-transitioning-gemini-cli-to-antigravity-cli/
- Gemini CLI headless reference: https://github.com/google-gemini/gemini-cli/blob/main/docs/cli/headless.md
- Antigravity CLI headless: https://antigravity.google/docs/cli/headless/
- Antigravity CLI ACP: https://antigravity.google/docs/cli/acp/
- Antigravity CLI MCP: https://antigravity.google/docs/mcp (Streamable HTTP only, `serverUrl` + `headers`)
- Antigravity CLI reference and troubleshooting: https://antigravity.google/docs/cli/reference/, https://antigravity.google/docs/cli/troubleshooting/
- ACP agents, initialization, session setup, prompt turn, authentication, Rust library: https://agentclientprotocol.com/get-started/agents, /protocol/initialization, /protocol/session-setup, /protocol/prompt-turn, /protocol/v1/authentication, /libraries/rust
- ACP v2 draft announcement (2026-07-20): https://agentclientprotocol.com/announcements/acp-v2-draft
- ACP registry: https://cdn.agentclientprotocol.com/registry/v1/latest/registry.json
- `agent-client-protocol` crate: https://crates.io/crates/agent-client-protocol
- Cursor CLI: https://cursor.com/docs/cli/overview, /docs/cli/reference/parameters, /docs/cli/reference/output-format, /docs/cli/reference/authentication, /docs/context/mcp
- OpenCode: https://opencode.ai/docs/cli/, /docs/mcp-servers/, /docs/providers/
- Copilot subscriber counts: Microsoft FY26 Q2 earnings (4.7M paid, 2026-01-28), TechCrunch 2025-07-30 (20M cumulative)
- Cursor revenue: Forbes 2026-06-08 via secondary reporting

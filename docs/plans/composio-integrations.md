# Composio integrations for Juno

Status: Phase 1 filed as LAC-4210 (2026-10-08). Pricing approved by Lacy 2026-10-08 (recorded on LAC-4130). Launch on Composio's Google app approved 2026-10-08. Composio org for Phase 2 = LAC-4212 (Lacy, later). DRI: Lacy. Date: 2026-10-04, updated 2026-10-08.

Shareable version: https://claude.ai/artifact/5zgJB2Cfw9EYdvDAzn3Pu1

**One sentence: attach Composio Connect as a single remote MCP server, ship Gmail and Google Calendar only, and spend the real work on the approval gate, because Juno's risk classifier currently rates every unknown tool `Low` and would let `GMAIL_SEND_EMAIL` run unasked.**

## The first ten seconds

Person holds the dictation key and says "what's on my calendar this afternoon". Juno answers out loud. No setup screen, no JSON, no tool list. The one time it cannot answer, it says "I need your Google Calendar" and a browser tab opens to Google's own consent screen. They click allow, the tab says done, Juno is already talking.

That is the product. Everything below exists to serve it.

## What Composio is

An integration layer that holds OAuth for a person's SaaS accounts and exposes those APIs to an agent as tools. As of 2026 it advertises 1,000+ toolkits and 20,000+ individual tools (Gmail, Slack, Notion, Linear, GitHub, Stripe, and so on). It owns the parts nobody wants to write: OAuth apps per vendor, token refresh, rate limits, retries, schema drift.

Mechanics that matter here:

- **Hosted MCP endpoint.** `https://connect.composio.dev/mcp`, streamable HTTP. One URL, no per-app plumbing.
- **Seven meta-tools, not 20,000 tools.** `COMPOSIO_SEARCH_TOOLS`, `COMPOSIO_GET_TOOL_SCHEMAS`, `COMPOSIO_MULTI_EXECUTE_TOOL`, `COMPOSIO_MANAGE_CONNECTIONS`, `COMPOSIO_WAIT_FOR_CONNECTIONS`, `COMPOSIO_REMOTE_WORKBENCH`, `COMPOSIO_REMOTE_BASH_TOOL`. The agent searches the catalog at runtime and executes by slug, so context stays small. This is the whole reason the approach is viable for a local assistant.
- **Auth.** Claude Desktop and Cursor authenticate to the endpoint by OAuth at setup. Other clients pass an `x-consumer-api-key` header. Per-user scoping is a `user_id` on the session; connections persist under that id across sessions.
- **OAuth completion needs no callback of ours.** Composio generates a hosted consent link, the person approves in their browser, Composio redirects to its own success page. Juno polls. No Juno backend, no `juno://` URL scheme, no hosted redirect to maintain.
- **SDKs are Python and TypeScript only.** No Rust SDK. There is a REST API at `https://backend.composio.dev/api/v3.1` if we ever want to skip MCP.
- **Open source is the SDK, not the platform.** MIT on GitHub (~30k stars), but the catalog, the OAuth apps and the token vault are their cloud. There is no self-host escape.

## What the competitors actually do

Clicky (YC Spring 2026) is the one Lacy named. Its marketing says the opposite of integrations: "anything on your screen. if you can see it, heyclicky can see it. no plugins or integrations needed." Composio appears nowhere on the site. Users found it anyway, and the open-source clone `jasonkneen/openclicky` ships the real pattern in its bundled skills:

- Composio is attached to the agent runtime as an MCP server literally named `composio`.
- Apps are connected **per toolkit** in `Settings -> Integrations`: slugs `gmail`, `googlecalendar`, `googledrive`, `googledocs`, `googlesheets`. The skill explicitly forbids asking a person to connect "Google Workspace" as one thing.
- **The agent never runs OAuth.** On missing or expired auth it stops and tells the person to connect that one app in Settings. It is also forbidden from using computer-use to drive the app's own settings window.
- Screen control is the fallback, not the path. "Do not silently switch to browser automation."
- Their safety line, which matches Lacy's existing rule: reads and ordinary writes execute directly because the request is the approval. Deleting, archiving, overwriting what was not asked for, and spending money need confirmation. Gmail sending needs explicit approval of recipients, subject, body, attachments.
- They do not trust success booleans. After a write they read back and verify, because a Docs create can return success and leave a title-only document.

Everyone else in the category (Wispr Flow, Raycast, Highlight) either stays a keyboard and never integrates, or builds extensions one at a time. Composio is how a small team gets the long tail without a connector engineer.

## Options

| Path | Work | Cost to Lacy | Verdict |
| --- | --- | --- | --- |
| Composio Connect as one remote MCP server | days | metered per call | **pick this** |
| Composio REST v3.1 from Rust directly | weeks, reimplements the meta-tool loop | same | no |
| Per-app MCP servers (gmail-mcp, slack-mcp, ...) | weeks per app, every token on us, every refresh bug on us | none | no |
| Build our own OAuth broker | months | hosting | no |

Arcade.dev is the one real alternative worth a second look, it leads on per-user scoped permissions and just-in-time grants, which is exactly the axis where Composio is weakest for us. Nango is the open-source option. Neither has Composio's catalog. Revisit only if the approval work below proves impossible on top of meta-tools.

## What Juno already has

Juno is further along than it looks. It already is an MCP client.

- `~/repo/juno/src-tauri/src/agent/tools/mcp_integration.rs` speaks MCP over stdio **and** HTTP JSON-RPC (`is_http_transport`, `ensure_http_url` at line 139, `send_http_request` at line 806).
- `MCPServerConfig` (line 20) carries an `approved` flag so an arbitrary command is never spawned without consent.
- `~/repo/juno/src-tauri/src/agent/tools/tool_config.rs` has a `ToolCategory::MCP` already wired into settings and enablement.
- `~/repo/juno/src-tauri/src/settings/mod.rs:347` persists `mcp_servers`.
- Risk and approval already live in one place: `risk_classifier.rs` then `permission_policy.rs::requires_approval` (line 84).

So the integration is small. The gaps are specific.

### Gap 1, the approval gate. This is the whole job.

**Deferred 2026-10-08 (Lacy): out of Phase 1, parked as LAC-4213. Must land before integrations leave the Advanced beta toggle.**

`classify_risk` in `~/repo/juno/src-tauri/src/agent/tools/risk_classifier.rs:82` ends in `_ => RiskLevel::Low`. Every tool it does not name by hand is Low. Today that is fine, Juno registers a known set. The moment Composio is attached, `GMAIL_SEND_EMAIL`, `GOOGLEDRIVE_DELETE_FILE` and `STRIPE_CREATE_REFUND` all arrive as unnamed tools and land on Low, which `requires_approval` lets through in every mode except AskFirst.

Worse, with meta-tools the tool name is not even the action. Every call arrives as `COMPOSIO_MULTI_EXECUTE_TOOL` with the real slug buried in its arguments, up to 50 of them per call. A name-based classifier cannot see it.

This is the dead-control pattern Juno keeps producing: a safeguard that no longer governs what it is named after. The fix has to be structural, not a list:

1. Add an arm for `COMPOSIO_MULTI_EXECUTE_TOOL` that reads the inner slugs out of the arguments and returns the **highest** risk across them.
2. Classify by slug verb, not by allowlist: `_SEND`, `_DELETE`, `_ARCHIVE`, `_REFUND`, `_PAY`, `_TRANSFER`, `_CHARGE`, `_REMOVE`, `_REVOKE`, `_SHARE` are High. Reads (`_GET_`, `_LIST_`, `_SEARCH_`, `_FETCH_`) are Low. Unknown write-shaped slugs default to **Medium, not Low**.
3. Flip the default for externally supplied tools. Juno's own tools can keep an allowlist default because we wrote them. A tool whose name came off the network is unknown by definition and must not be Low.
4. Pin it with a test, the way `alias_names_are_still_reachable` pins the browser arm. A test that asserts `GMAIL_SEND_EMAIL` nested inside a multi-execute call returns High, and that an unrecognised `*_DELETE_*` slug does not return Low.
5. `describe_action` (line 158) needs to render the inner action in English, "Send an email to katie@... " not "Run COMPOSIO_MULTI_EXECUTE_TOOL". The prompt is useless otherwise.

Do this before the first connection ships, not after.

### Gap 2, the HTTP transport is not spec-complete

`send_http_request` sets only `Accept` and parses the body with `serde_json::from_str`. Streamable HTTP needs three things it does not do:

- **Custom headers.** `MCPServerConfig` has no `headers` map, so `x-consumer-api-key` cannot be sent. Add one, and store its value in the keychain rather than in the settings JSON.
- **SSE frame parsing.** A `text/event-stream` response arrives as `data: {...}` lines. Today that fails to parse. Branch on the response content type.
- **Session id.** Capture `Mcp-Session-Id` from the initialize response and echo it on subsequent requests. Also send `MCP-Protocol-Version`.

Roughly 200 to 300 lines in one file, worth doing on its own merits since it unlocks every hosted MCP server, not just this one.

### Gap 3, how connecting looks (decided 2026-10-08: don't feature it)

Lacy: most people won't use integrations, so they don't get a section. What competitors do: Clicky has a Settings, Integrations screen (per its bundled skills); openclicky reduced it to one off-by-default "Composio connected apps" toggle under System & Logs; Codex shows nothing, Composio is one `[mcp_servers.composio]` line in config.toml.

Juno:
- No top-level Integrations section, no catalog, no curated rows. Today's paste-JSON MCP UI in `~/repo/juno/src/components/settings/sections/NetworkSettings.tsx:125` stays as the power-user path.
- Connect is just-in-time: a request that needs an unconnected app gets a reply card with one button, "Connect Google Calendar". The button runs the app's connect flow (browser consent) and the request retries. The agent never runs OAuth itself.
- A "Connected apps" group in the existing Tools section renders only once something is connected: app, account, Disconnect.
- "Composio" appears once, as a disclosure line on the first connect card.
- Advanced: one toggle, "Use my own Composio account".

### Gap 4, who holds the API key

Three ways, pick one deliberately.

- **Ship a key in the app.** Extractable from the binary in minutes, and every user's calls bill to us. No.
- **Each person brings their own Composio key.** Free for us, and it fails the first ten seconds badly. No, except as the advanced-settings escape hatch.
- **OAuth to the MCP endpoint, the way Claude Desktop does.** No key in the binary, each person's Composio account is their own, usage is theirs. **Answered 2026-10-08: yes.** `connect.composio.dev` publishes MCP OAuth metadata with dynamic client registration (`login.composio.dev/oauth2/register`), PKCE S256 and public clients (`token_endpoint_auth_method: none`), and DCR accepted a `http://127.0.0.1:<port>/callback` loopback redirect. The browser consent step was not run. But this route signs the person into their own Composio account, so it is the Advanced and internal-beta path only. Paying users go through Juno's gateway (LAC-4128) holding our Composio key server-side, with the Juno account id as Composio `user_id`.

## Money

Published pricing on 2026-10-04: Hobby is free with 100,000 tool calls and 50,000 trigger events a month. Pro is $29/mo including $29 of usage, then $0.0003 per tool call, $0.003 per trigger, $3.75 per million LLM tokens, $0.50 per GB-hour of sandbox.

At $0.0003 a call, 500 people doing 20 calls a day is about 300k calls a month, roughly $90. That is affordable.

The risk is not the rate, it is the volatility. A competitor's teardown (Scalekit, so read it with that in mind) claims that in August 2026 the $29 tier dropped from 200k to 50k calls, the $229 tier was replaced by a $599 one, overage went from $0.25 to $4 per 1,000, and self-managed credentials moved behind $599. Those numbers do not match the public pricing page today. Either way the lesson holds: this is a young vendor repricing aggressively, and we would be putting a user-facing capability on top of it. If we ever hold the key, get the rate in writing and put a monthly cap in the backend.

## Pricing (approved by Lacy 2026-10-08)

Fits the LAC-4128 price list, no new tier. One entitlement key, `integrations`, added to the LAC-4130 plan_features launch keys. Composio calls debit the same dollar credits ledger at 2x cost (one rate-table row).

- Free BYO-plan, no account: asking for an app prompts a free Juno sign-in, which starts the trial.
- Trial, Pro ($12/mo, $100/yr), founding seats: integrations included, debited from credits. Founding liability is capped by the grant balance.
- Advanced: the person's own Composio account via OAuth, free to us (`byok_unlocks_all`).

Assumes ~3 billed calls per spoken request (Phase 1 measures it). At $0.0003/call a request debits ~$0.0018; a regular user (20/day) costs us $0.54/mo and is debited $1.08. If the claimed $4/1k repricing is real, the same user costs $7.20 and the rate row moves; the ledger means we never lose money per call. Composio's free tier covers the first 100k calls/mo org-wide (~55 regular users).

Google: launch on Composio's verified Google app (consent screen says Composio). Juno-branded Google app needs CASA Tier 2, ~$540-1k/yr and 2-8 weeks, yearly. Revisit at ~50 paying users.

## Privacy, stated plainly

Connecting Gmail through Composio means a third party holds a refresh token for that person's mail. Juno's posture is permissive by default, and this is still a real change in who is trusted. It does not need a scary modal. It needs one honest line on the Integrations screen and in the connect flow, naming Composio, with a link. Say it once, do not nag.

## Scope

**Phase 1, the demo (LAC-4210).** HTTP transport fixed, generic MCP OAuth client, risk classification by inner slug plus the pinning tests, just-in-time connect card plus a Connected apps group that appears only after a connection, tested on Gmail and Google Calendar, behind an Advanced beta toggle, connecting via the person's own Composio account. Ship when "what's on my calendar" and "read me my unread mail" work by voice and "send Katie an email" stops and asks.

**Phase 2, the paid product.** Blocked on LAC-4128/4130/4134. Gateway proxies Composio with our key and the account id, checks `integrations`, debits the ledger per billed call.

**Phase 3.** Verify Slack, Notion, Linear end to end (no new UI, they connect the same way) and add skill-level notes about verifying writes.

**Later, only if asked for.** A searchable catalog beyond the curated rows.

## Cut from this proposal

- An Integrations section, a 1,000-app gallery, or curated app rows. Asking is the catalog.
- `COMPOSIO_REMOTE_WORKBENCH` and `COMPOSIO_REMOTE_BASH_TOOL`. Juno already has a local shell. A second shell in someone else's cloud is a seam and a billing line.
- Triggers and webhooks, 50k a month sitting there unused. Juno has its own triggers.
- Team and shared connections.
- Any API key field on the Integrations screen.
- "Google Workspace" as one connect button. Clicky learned this the hard way, it is five separate toolkits.
- Writing our own OAuth broker.

## Open questions

1. ~~Does `connect.composio.dev/mcp` accept OAuth from a third-party client?~~ Yes, see gap 4.
2. Are per-toolkit scopes reducible, can Gmail be connected read-only first and upgraded to send on demand?
3. Does a connection survive our `user_id` changing, and what is `user_id` for a desktop install with no accounts?

## Demo test

Unfilled until Phase 1 is built. Needed: screen recording of the voice ask working and of the send approval prompt, committed under `~/repo/juno/docs/changelog/media/<PR>/`.

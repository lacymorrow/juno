# Accounts, usage gateway, credits (LAC-4128)

Status: **CTO-approved 2026-10-04** (plan gate; CTO owns this gate, not Lacy).
Stack approved by Lacy 2026-10-04: Better Auth + Polar.
Security Engineer must review before any part of this holds a real model key or takes real money.

## What this replaces

Today a "demo build" carries an Anthropic key compiled in at build time
(`src-tauri/src/demo.rs`, `option_env!("JUNO_DEMO_ANTHROPIC_KEY")`). The app sends it
straight to `api.anthropic.com` with an `x-api-key` header
(`agent/providers/anthropic.rs`). Anyone with the binary can `strings` the key out;
the only protection is a spend-capped console workspace.

The gateway inverts the trust model: **the model keys never ship in the binary.** The
app authenticates as a *person* (session token), and our server holds the Anthropic/OpenAI
keys, meters spend, and enforces a credit balance. `demo.rs` stays only as the fallback
for the "no account, no key" path during migration, then retires.

## Decision 1 — Where it runs: split plane

Two services, because they have opposite runtime shapes.

| Plane | Runs on | Why |
|---|---|---|
| **Control plane** — Better Auth, Polar webhooks, credits ledger, account API, Settings data | `juno-www` on **Vercel** | Short request/response, DB-backed. Polar's Better Auth plugin is a TS library built for exactly a Next.js app. juno-www is already Next 16 on Vercel with `app/api/*` routes and a Postgres-ready stack. |
| **Streaming gateway** — the model proxy itself | **Hetzner `178.156.130.157`** (persistent process), *not* Vercel | Computer-use runs stream for many minutes with screenshots. Vercel functions have a hard execution ceiling; a long SSE proxy is the wrong fit. A persistent VM has no per-request timeout. |

The gateway validates the Better Auth session (shared Postgres session table, or a short-lived
signed gateway token minted by the control plane), meters on the usage the model API actually
reports, and decrements the **same** ledger the control plane grants into.

Rejected: putting the proxy on Vercel (duration ceiling kills long runs). Reconsider the existing
Fly backend (`juno-cloud-backend.fly.dev`, currently off) only if we want to retire Hetzner later;
Fly machines also have no request ceiling, so it is a valid alternate home, not a blocker.

## Decision 2 — Database

**Postgres, Drizzle ORM** — the shipkit house standard (root `CLAUDE.md`), and both Better Auth
and Polar's plugin support it first-class. Hosted serverless Postgres (Neon or Supabase) so both
planes reach the same DB: Vercel over the pooled connection, Hetzner over a direct connection.
One database, two readers. The credits ledger is ours and lives here, never in Polar.

## Decision 3 — API shape

**Auth (control plane, juno-www):**
- `ALL /api/auth/*` — Better Auth handler. Providers: Apple, Google, email magic link. No passwords.
- Desktop flow: app opens the system browser to `/sign-in?desktop=1` → person signs in → callback
  redirects to `juno://auth/callback?token=<one-time code>` → app exchanges the code for a session
  → session stored in the **macOS Keychain** (new; no keyring crate in the Rust tree yet).
- Sign-in is prompted **only** when the person reaches for trial / Pro / founding activation.
  Bring-your-own-key users never see it.

**Account (control plane):**
- `GET /api/account` — plan, trial status, credit balance. Powers the Settings row and the
  out-of-credits / signed-out states.
- `POST /api/polar/webhook` — verify Polar signature, grant entitlements/credits idempotently.

**Gateway (streaming plane, Hetzner):**
- `POST /v1/messages` — Anthropic-shaped passthrough; `Authorization: Bearer <gateway/session token>`.
  Returns the upstream SSE stream unchanged so `anthropic.rs` needs only a base-URL + auth swap, not
  a new parser.
- `POST /v1/chat/completions` — OpenAI-shaped, same contract, for the OpenAI path.
- Balance exhausted → a **structured, non-streaming JSON** the app designs for (a clean
  "out of credits" state), not a raw upstream 400. Decided before the first upstream byte: the
  gateway checks balance, forwards, then meters from the usage block the stream reports.

## Decision 4 — Credits ledger (ours)

Append-only, double-entry, server-authoritative. Grants are rows; spend is rows; balance is a sum
(with a cached running total for reads).

- **Grants:** trial (one-time, ~$2 model cost — final size lands after LAC-4124), Pro monthly
  allowance, founding one-time $20, top-up packs.
- **Metering:** convert the model API's reported input/output/cache tokens to our credit unit using
  a per-model rate table. Hosted default = Sonnet; pricier models burn more credit. **Never trust a
  client-reported count** — meter only on the upstream usage block.
- **One trial per account+device:** bind the trial grant to `(account_id, device_id)` where
  `device_id` reuses the existing `cloud/auth.rs` device identity. A second account on the same
  device gets no fresh trial.
- **Idempotency:** every Polar webhook carries an event id; granting is keyed on it so a replayed
  webhook grants once. Every spend debit is keyed on the upstream request id.

## Decision 5 — Polar

- Products: Pro monthly **$12** / yearly **$100**; top-up packs; founding lifetime license in
  batches (#1-100 $49, #101-300 $99, #301-500 $149, then $199).
- Polar's Better Auth plugin + webhooks grant entitlements/credits into *our* ledger.
- Founding = a license key activated against the account (not a Polar-side entitlement we read live).
- **Test mode until Lacy flips live.** Live keys are a Security-gated, Lacy-gated switch.

## Threat model (Security Engineer reviews before live)

1. **Model keys** live only on the gateway host env, never in the binary, never in git, never in the
   client. This is the whole point; a regression here is a critical.
2. **Open-proxy abuse:** the gateway forwards nothing without a valid session *and* a positive
   balance checked first. Per-user rate limit. No anonymous path.
3. **Session token theft:** short-lived, revocable, Keychain-stored (not a file, not localStorage).
   Deep-link callback uses a one-time code exchanged over TLS, not the session token in the URL.
4. **Metering integrity:** debit from the upstream usage block only; a client cannot under-report.
5. **Trial abuse:** account+device binding (above); watch for disposable-email magic-link farming
   (Apple/Google raise the cost; magic link is the soft spot — rate-limit grant creation per IP/device).
6. **Webhook forgery/replay:** verify Polar signature; idempotent grant keyed on event id.
7. **Ledger tampering:** append-only, server-only writes; the app reads balance, never sets it.

## App side (Jobs standard — the Design child ticket owns the demo test)

The person-facing surface is small and must pass the demo test before it ships:
- One Account/plan row in Settings; trial and credit balance visible.
- **Signed-out state** and **out-of-credits state** are designed, not errors — one primary action each
  ("Sign in to start your trial" / "Top up" or "Upgrade").
- Plugs into the default-provider rule (LAC-4125) as the fallback *after* Claude/ChatGPT logins:
  a person already signed into Claude Max keeps using that; the gateway is the path for everyone else.

## Rollout

Test mode end to end (Polar sandbox, a non-production credit pool) → Security review → Lacy flips
Polar live and we point the app's provider base URL at the gateway. `demo.rs` stays as the pre-account
fallback until the gateway is the default, then retires.

## Child tickets (sliced from this plan)

Created as children of LAC-4128. Build order roughly top-down; the gateway and ledger are the spine.

1. **Control plane: Better Auth on juno-www** — Apple/Google/magic-link, no passwords, Postgres+Drizzle
   schema, `/api/auth/*`, desktop `juno://` deep-link code exchange. (Founding Eng; Security review child.)
2. **Credits ledger + metering** — schema, grant/debit API, per-model rate table, idempotency keys.
   (Founding Eng; Security review child.)
3. **Streaming usage gateway on Hetzner** — `/v1/messages` + `/v1/chat/completions` SSE proxy, balance
   gate, meter-from-usage, structured out-of-credits response, holds the model keys. (Founding/DevOps;
   Security review child — this one is the key-holder.)
4. **Polar integration** — products, Better Auth plugin, webhook grant into ledger, founding license
   activation, test mode. (Founding Eng; Security review child.)
5. **Juno client: session + Keychain + provider swap** — Keychain storage, deep-link handler, point the
   Anthropic/OpenAI providers at the gateway with the session token, prompt sign-in only on
   trial/Pro/founding. (Founding Eng, Rust; CI compiles.)
6. **App-side Settings + states (Design-led)** — account row, balance, signed-out + out-of-credits
   states, Jobs demo test. (Design child first, then Founding Eng.)

Each child gets Security review before its piece goes live (the three money/key-holding tickets are
non-negotiable). QA on the client-facing pieces. Merge & deploy per the usual gate.

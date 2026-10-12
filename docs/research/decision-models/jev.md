# Jev (TypeSafe AI) for Juno: feasibility research (2026-10-08)

Tags: [V] verified against primary source (docs.typesafe.ai or OpenRouter docs, fetched today). [U] unverified (secondary or third-party).

## Summary verdict
Feasible and easy to try. Plain HTTPS JSON, one POST, bearer auth, no SDK needed from Rust. [V] Fits all four Juno uses (route, settings destination, component pick, mid-run input) as Choice questions with an explicit "none" option, several questions per call in parallel. [V]
Not proven for our accuracy bar: TypeSafe publishes no benchmarks and does not claim calibration. [V] The docs warn about option-order bias toward the first option and about injected text. [V] Latency is plausible (about 150 to 300 ms p50 on third-party telemetry) [U] but region is undocumented, so measure from Charlotte. Use the 0.9 threshold plus fallback to Claude as already planned. Not a safety gate.

## What it is
"System One" model: state in, typed answers out, no generated text. Primitives: Choice (choice, probabilities summing to 1, confidence 0-1), Score (score, legend, probabilities, confidence), Noul (yes/no, number 0-1, no confidence field). [V] Questions in one request are evaluated in parallel and independently against the same state. [V] Flagship model jev-1.13.0, aliases jev-latest and jev-preview (no preview build exists). [V] Text only, English primary, other languages lower accuracy. [V] Weights and architecture unpublished. [U] Company: TypeSafe AI, San Francisco, founded 2024, CEO Diogo Almeida (ex-OpenAI), $40M seed led by DCVC announced about 2026-09-15. [U]

## Access & cost
- Direct: sign in at console.typesafe.ai, keys page, set TYPESAFE_API_KEY. [V, quickstart]
- Waitlist: conflicting. Docs do not mention a waitlist. One third party says GA since 2026-09-21 with the waitlist removed, and $5 starting credit (about 120M input tokens) on new accounts [U]. Another still says early-access waitlist [U]. Homepage reportedly still says early access [U]. Try signup to settle it.
- Price: $0.042 per M input tokens, output free. [V, models page] Max 64k-token call is under 0.3 cents [U math, consistent]. Juno estimate: a request with 30 settings options plus 4 questions is roughly 1k to 2k tokens, about $0.00008 per call. Negligible.
- Hosts: TypeSafe direct `POST https://api.typesafe.ai/v1/systemone` [V]; OpenRouter `POST https://openrouter.ai/api/alpha/decisions` and `/api/v1/systemone`, model `typesafe/jev-1.13` or `~typesafe/jev-latest`, OpenRouter key, usage.cost in response [V, OpenRouter guide]; AIMLAPI `/v1/decisions` model `typesafe/jev` about $0.058/M [U]; Portkey `/v1/decisions`, no streaming [U]; OrcaRouter `/v1/systemone` [U]; Cloudflare Workers AI [U]; ZenMux [U].
- Rate limits (direct): 100K tokens/s and 80 req/s, dynamic, can change without notice; 429 on excess; higher on enterprise. [V] Generous for one user. Partial-transcript use (maybe 5 calls/s) is fine.
- Official SDKs: Python (`pip install typesafe-sdk`, 3.10+) and JavaScript/TypeScript. No Rust SDK; docs say call HTTP from any language. [V] OpenAPI spec: mentioned by a third party, not found in docs [U]. Agent skill: `typesafe-ai/skills`. [V]

## API shape
Request (direct, from docs; Juno-shaped example is mine):
```json
POST https://api.typesafe.ai/v1/systemone
Authorization: Bearer $TYPESAFE_API_KEY
{
  "model": "jev-1.13.0",
  "state": {"transcript": "open bluetooth settings", "run_active": false},
  "questions": {
    "route": {"type": "choice",
      "instructions": "How should Juno handle this request?",
      "criteria": {"chat": "plain conversation, no tools", "tools": "needs tools or files",
                   "computer_use": "needs to operate the screen", "escalate": "hard, needs the strongest model"}},
    "settings_dest": {"type": "choice",
      "instructions": "Which settings page does the person want opened, if any?",
      "criteria": {"bluetooth": "Mac Bluetooth settings", "juno_voice": "Juno voice settings", "none": "no settings page requested"}},
    "is_for_juno": {"type": "noul", "instructions": "Is this addressed to the assistant?"}
  }
}
```
Response:
```json
{"model": "jev-1.13.0",
 "answers": {
   "route": {"type": "choice", "choice": "tools", "confidence": 0.82,
             "probabilities": {"chat": 0.05, "tools": 0.9, "computer_use": 0.03, "escalate": 0.02}},
   "settings_dest": {"type": "choice", "choice": "bluetooth", "confidence": 0.97, "probabilities": {"...": 0}},
   "is_for_juno": {"type": "noul", "noul": 0.96}},
 "usage": {"input_tokens": 412, "output_tokens": 0}}
```
Field names [V]: request state (string/object/array), model, questions map (keys not sent to model), per question type, instructions (string/object/array), criteria (choice: map option to description or null; score: ordered array; noul: optional {true,false}). Response model, answers, usage. Errors: 401, 422 (names field), 429, 529 (overloaded, back off). [V] Exact example numbers above are illustrative, not captured. Host schemas differ slightly (OpenRouter alpha path); use the direct host for the benchmark. No streaming documented on direct API; Portkey says decisions do not stream [U]. Not needed, output is tiny.

## Limits
- Choice: up to 255 options per question. [V] 30 destinations plus none is fine. Docs advise listing the full set, not a shortlist. [V]
- Score: 2 to 10 levels. [V]
- Questions per request: no maximum stated. [V (absence)] Each costs tokens; parallel so latency barely grows. [V claim, vendor]
- Context: 64k tokens total (state plus all questions); 32k for state plus the single longest question. [V] OpenRouter lists 32k. [V]
- No "none" semantics built in: add an explicit "none"/"other" option. [V] One third-party test: 0 of 30 out-of-scope messages flagged, each at 0.99 confidence, when no escape option existed [U].
- Confidence: computed from distribution shape (how far top probability sits above uniform). Docs explicitly do NOT claim calibration. [V] Third-party study: calibration error about 4.4x noise floor on unfamiliar synthetic data, fine on public benchmarks [U]. Noul has no confidence field; use distance from 0.5. [V]
- Benchmarks: none published in docs (jaggedness page: "does not report any benchmarks"). [V] Third parties: internal dashboard 67.8% vs 74.1% best comparator [U]; a 78-case classifier suite scored 0.974 [U]; a Classmethod routing test found lower latency/cost, no broad accuracy [U].
- Known failure modes (docs, reviewed 2026-10-02) [V]: literal reading; math/counting; date/time comparison; double negatives and multi-hop; accuracy drops with large irrelevant state; adversarial or injected content shifts answers; contradictory criteria; Choice option order matters and it leans to the first option (mitigate by shuffling and checking consistency); cannot generate text; weaker on slow-reasoning tasks. Slot values ("10 minutes") need extraction elsewhere.
- Docs recommend: atomic questions, batch in one call (smart-home demo uses 4 questions upfront and calls sequential rounds slower), act on confidence bands, high threshold (above 0.9) for destructive actions. [V]
- No caching/state reuse across requests documented. [V (absence)] Re-sending the long criteria each partial-transcript call is the cost, still tiny.

## Latency
- Vendor: 70 to 500 ms end to end; homepage demo 0.114 s. No methodology. [U]
- OpenRouter telemetry via Firecrawl: p50 210, p95 340, p99 750 ms (2026-10-01). [U, from existing notes]
- OrcaRouter 7-day: p50 151 ms, p95 247 ms, error rate 0.49%. [U]
- Ably Pong demo via Vercel AI Gateway: avg 227 ms, p95 400 ms. Vendor-adjacent. [U]
- A 78-case suite: about 302 ms per case hosted. [U]
- Regions: not documented anywhere in docs. [V (absence)] Also served via Cloudflare Workers AI, which suggests edge presence. [U] Charlotte RTT is unknown; test direct, OpenRouter and Cloudflare. Keep a warm keep-alive HTTPS connection.
- Implication for Juno: p50 around 150 to 300 ms from a warm connection is inside the 1.2 s budget even serial; on partial transcripts it is ready at key release. No independent lab latency benchmark exists. [U]

## Privacy
- Docs legal page: Privacy Policy contains a commitment not to train on user data; DPA covers retention; zero data retention available for enterprise via sales@typesafe.ai. [V, summary page only] Full documents at typesafe.ai/legal were not read. Subprocessors not listed. Retention period for default accounts: unknown.
- Via OpenRouter: separate OpenRouter data policy applies, shown on the model page; not read. Via Cloudflare, different terms again.
- Juno transcripts are private. Needs Lacy's privacy call. Default retention unknown, ZDR only on enterprise.

## Rust integration path
Plain `reqwest` POST with a `serde_json` body; no SDK. Struct for `answers` as `HashMap<String, Answer>` with an untagged enum for choice/noul/score. Reuse one `reqwest::Client` with keep-alive and an 800 ms connect, about 400 ms total timeout, so a miss falls through to Claude. Retry on 429/529 with backoff only if time budget allows. Pin model to `jev-1.13.0` (aliases move on new releases). Could slot behind the existing `Route` enum in `router.rs`. Run on partial Whisper transcripts with cancellation of stale calls; shuffle option order per call or keep the most likely option not first and check agreement across two orders for high-stakes picks. Remember rule: agents never run cargo locally, so a CI build is needed for any in-app version; the benchmark below can be Python/curl outside the app.

## Effort to first benchmark result
- Signup and key: 15 to 30 min (plus waitlist risk, then use OpenRouter key).
- curl smoke test: 15 min.
- Python harness (standalone, `requests` or typesafe-sdk) over a labelled set with 4 question groups, order-shuffle, latency timing from this Mac: 2 to 3 hours given labelled data exists (labelling 300 utterances is the bigger cost, 1 to 2 hours more).
- Total to first accuracy plus latency number: about 3 to 5 hours. Rust integration into the app: another 4 to 8 hours, CI-built.

## Ease-of-use score: 4 / 5
One endpoint, simple JSON, documented schema, per-option probabilities, bearer auth, tiny cost, good docs with an llms.txt. Loses a point for: access status and free credit unclear, no Rust SDK or OpenAPI link in docs, undocumented region, no benchmarks or calibration claim, option-order bias that must be handled in code.

## Risks / unknowns
1. Calibration and real accuracy on Juno utterances (settings near-misses, "not for Juno" detection): no evidence either way; must benchmark. Out-of-scope inputs may come back at high confidence.
2. Retention and training terms by host: only a summary was read; default-account retention unknown; transcripts are private.
3. Region and real Charlotte latency; documented rate limits are "adjusting dynamically" and can change without notice. [V]
4. Company maturity: seed stage (about Sept 2026), month-old GA, one model. Reported talks for a $1B raise at $10B valuation are unconfirmed [U]. "We can't prove it isn't subsidized" on pricing (TypeSafe quote via eesel) [U]. Aliases move; deprecation policy undocumented. [V (absence)]
5. Prompt injection from screen/transcript text shifts probabilities. [V]
6. No outage history found; error rate 0.49% on one router [U]. 529 overload is a documented status. [V]
7. Access conflict (waitlist vs GA).

## Publishable facts (for a public blog comparison)
- Output is probabilities only; docs say it is not trained to generate text. [V]
- $0.042 per M input tokens, free output; limits 80 req/s and 100K tokens/s. [V]
- Up to 255 options per Choice, 64k context. [V]
- TypeSafe's own jaggedness page lists option order bias and "leans toward the first option." [V] Good test: same question, shuffled options, how often does the answer flip?
- Docs state confidence is not claimed to be calibrated. [V] Good test: reliability diagram on Juno data.
- Vendor claims 70 to 500 ms; independent numbers about 150 to 300 ms p50, p95 250 to 400 ms. [U]
- Funding: $40M seed, DCVC, ~Sept 15, 2026. [U]
- Calibration study and 0/30 out-of-scope finding. [U]

## Sources
Primary (fetched):
- https://docs.typesafe.ai/llms.txt (index)
- https://docs.typesafe.ai/introduction.md
- https://docs.typesafe.ai/introduction/quickstart.md
- https://docs.typesafe.ai/api.md
- https://docs.typesafe.ai/models.md
- https://docs.typesafe.ai/primitives/choice.md
- https://docs.typesafe.ai/confidence.md
- https://docs.typesafe.ai/concepts/state.md
- https://docs.typesafe.ai/model-jaggedness/jev-1.13.md
- https://docs.typesafe.ai/legal.md
- https://docs.typesafe.ai/sdk.md
- https://docs.typesafe.ai/patterns/intent-routing.md
- https://docs.typesafe.ai/demos/smart-home.md
- https://openrouter.ai/docs/guides/community/jev.md
- https://openrouter.ai/typesafe
Secondary:
- https://www.orcarouter.ai/blog/jev-typesafe-system-one-what-we-know
- https://www.eesel.ai/blog/jev-ultrafast
- https://runtimewire.com/article/diogo-almeida-typesafe-jev-40m-seed-pong
- https://dealroom.co/news/151032-typesafe-exits-stealth-with-40m-seed-to-build-ai-for-software-not-people/
- https://cryptobriefing.com/typesafe-ai-billion-dollar-funding-jev-model/
- https://aimlapi.com/models/typesafe-jev
- https://portkey.ai/docs/integrations/llms/typesafe.md
- https://www.firecrawl.dev/blog/openai-decisions-api-vs-jev
- Prior notes: ~/repo/juno/.claude/worktrees/fast-orchestration-research/docs/research/fast-orchestration.md

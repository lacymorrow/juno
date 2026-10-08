# Fast orchestration research

Date: 2026-10-08. Goal: cut the time from "person finishes speaking" to "first spoken word" to about 1 s. Today the turn is push-to-talk, Whisper STT, Claude CLI (sonnet, persistent stream-json process, effort high), sentence split, macOS `say`. Measured: first Claude token 1.2 to 2.5 s warm, 4 to 5 s cold.

## Recommendation

The main finding: a router that runs before Claude cannot get under 1 s, because it adds its own latency to the front of the turn. Every option below that is a classifier helps only if it runs in parallel with Claude, or if it picks a faster responder. So the order is about removing serial work first, then adding a fast first-sentence path.

1. **Parallelize and pre-warm, no new vendor (saves 0.3 to 3 s, free).**
   Start the Claude CLI request the moment STT finalizes, never after any classifier. Keep the process warm (the 4 to 5 s cold turns are the biggest single loss) and drop effort from high to low or medium for chat turns. Start TTS on the first sentence boundary, and flush the first clause at a comma or 8 to 10 words instead of waiting for a full sentence. These are measurement-and-tuning changes, not research bets. Do them first and re-measure with the existing `[TurnTiming]` log.

2. **Instant spoken acknowledgment, local, 0 ms network (first word at about STT + 50 to 100 ms, free).**
   Pick from a small bank of pre-synthesized fillers ("Sure", "One sec", "Let me check") with a rule-based or tiny embedding match on the transcript (under 5 ms, see below). This hides the Claude gap rather than shrinking it, and it is the only thing here that reliably gets "first spoken word" under 1 s on the tool and computer-use paths. Risk: fillers on every turn feel robotic, so use them only when the predicted route is tools or computer use, and keep answers on the chit-chat path unfilled.

3. **Fast first-sentence model on a low-latency host, raced against Claude (first real sentence in about 300 to 600 ms, roughly $0.05 to 0.10 per M tokens, so a fraction of a cent per turn).**
   Send the transcript to a small model on Groq (or a Gemini Flash-Lite class model) with a prompt that says "answer in one short sentence or say you are working on it". Speak that sentence, then hand the rest to Claude, and tell Claude what was already said so it continues. Reported TTFT for small models on Groq is roughly 80 to 190 ms; independent numbers conflict, so measure from Charlotte before committing. Needs an API key and a privacy decision (transcripts leave the machine to a second vendor).

4. **Jev or OpenAI Decisions as the route/risk gate, only after 1 to 3 (cost: about 210 ms p50 if serial, near 0 if parallel; $0.042 per M input tokens).**
   Jev is the only one of the decision models you can call today. Use it where a typed, thresholdable answer matters: one request with several questions (route: chat/tools/computer-use; is this destructive; is this a follow-up). The existing `router.rs` classifier (an Anthropic API call, 1500 ms timeout, off by default, API provider only) could be swapped to Jev behind the same `Route` enum. Do not put it in front of the first word. Do not use it for a hard safety gate yet, see Jev caveats.

5. **Skip for now.** Apple Foundation Models on macOS 27 (no published TTFT, beta-era guided-generation slowdowns reported), MLX local LLMs (no measured numbers found), gpt-live-1 as a full replacement (the backend and tool use would leave Claude CLI, and API access status is unclear).

Expected result if 1 to 3 land: first spoken word around 100 to 600 ms after STT on all paths, with Claude's real answer arriving at the current 1.2 to 2.5 s warm. The "under 1 s total" target is met for the first word, not for the full answer.

## Comparison

| Option | Role | Latency (source) | Cost | Access | Fit for Juno |
|---|---|---|---|---|---|
| Existing `router.rs` classifier | route chat/tools/escalate | 1500 ms timeout cap; real value not measured here | Anthropic API tokens | Beta, off by default, API provider only | Serial, so it cannot help first-word time |
| Jev (TypeSafe) | typed route/risk decisions | P50 210 ms, P95 340 ms, P99 750 ms (OpenRouter telemetry, per a third-party write-up) | $0.042 per M input, $0 output | Reported GA since 2026-09-15 via TypeSafe, OpenRouter, AIMLAPI | Good gate, no text out, so cannot speak |
| OpenAI Decisions API | typed decisions | 150 ms vs 1.6 s for Luna (OpenAI slide, conditions unstated) | Not published | Limited preview; 403 for standard keys as of Oct 1 to 2 | Unusable today |
| Groq small model | first sentence + routing | about 80 to 190 ms TTFT reported, sources conflict | about $0.05 to 0.08 per M (Llama 8B class) | Public | Best first-sentence candidate |
| Cerebras small model | same | 170 to 440 ms TTFT reported, sources conflict | about $0.10 per M | Public | Throughput leader, not TTFT leader |
| Gemini Flash-Lite class | same | no ms figure found | 3.1 Flash-Lite $0.25 in / $1.50 out per M | Public | Measure before choosing |
| Claude Haiku | same, one vendor | Haiku 4.5: 0.58 to 0.96 s TTFT on 10k-token prompts (Artificial Analysis). No data found for Haiku 5.5 | Anthropic API | Public | Too slow for the first word |
| Apple Foundation Models (macOS 27) | on-device | No published TTFT; Xcode 27 instrument exists | Free | OS feature, 4096-token window | Unproven, revisit after measuring |
| Embedding router / rules | local intent match | under 5 ms to 100 ms claimed, vendor-affiliated | Free | Library | Good for filler selection |
| gpt-live-1 | full duplex voice | Turn-taking 0.798 s (Full Duplex Bench via secondary sources); 498 ms median measured by Agora, but on the ChatGPT app | $0.05 per min | Reported GA 2026-09-10; Agora says API was waitlist-only in July | Different architecture; see notes |

## Notes by option

### Jev (TypeSafe AI)

What it is: a hosted "System One" decision model. You send application state (text, JSON or array) plus typed questions and get back structured answers with a probability per option and a confidence value. It generates no text. Question types: Choice (pick an option, per-option probabilities), Noul (yes/no probability), Score (probability-weighted position on a rubric). Many questions in one request run in parallel, so latency barely grows with question count.

Latency: P50 0.21 s, P95 0.34 s, P99 0.75 s, from OpenRouter production telemetry as of 2026-10-01, reported by Firecrawl. That is a platform measurement, not an independent benchmark. I did not measure it.

Pricing: $0.042 per M input tokens, free output (Firecrawl, Infrabase, Beri). AIMLAPI lists about $0.058 per M input. Sources disagree, so check the provider page.

API shape: AIMLAPI documents `POST /v1/decisions` with model `typesafe/jev`, body has a `state` string and a `questions` object where each question has a `type` and `instructions`. OpenRouter documents `POST https://openrouter.ai/api/alpha/decisions` and model ids like `typesafe/jev-1.13`. Exact field names differ by host; I did not retrieve TypeSafe's own docs at docs.typesafe.ai, so the schema is unverified.

Access: the task brief said early access, API only. Third-party sources say generally available with no waitlist since 2026-09-15 through TypeSafe, OpenRouter and AIMLAPI. Dealroom still says early access. Unverified which is current. The lowest-friction route is an OpenRouter key.

Limits and caveats: context 32K on OpenRouter, 64K per TypeSafe; text only; accuracy drops as irrelevant state grows. One review lists failure modes (arithmetic, counting, date comparison, double negatives, vague rubrics) and warns that injected text from adversarial input shifts the probabilities. A Hacker News critic says confidence values move when answer order changes (reported, not verified). Gradually.ai says no unambiguously attributed benchmark results exist yet. Calibration evidence is thin.

Fit: reasonable as a route classifier (chat / tools / computer use) feeding the existing `Route` enum, since the output is a probability you can threshold and fall back to Tools on low confidence, which matches the current "if unsure pick tools" rule. Weak as a risk gate, because the transcript is untrusted user and screen text and prompt injection measurably moves it. If used for risk, treat it as advisory and keep the deterministic permission rules as the real gate. It cannot produce the first sentence, so it does nothing for the first-word goal by itself.

What I could not confirm: TypeSafe's own docs, the exact request schema, current access status, any independent latency number, any accuracy number on a routing task like ours.

### OpenAI Decisions API

Announced at DevDay 2026-09-29, built on a specialized GPT-6 Luna. OpenAI's slide shows 150 ms against 1.6 s for a Luna call; test conditions are unstated. Pricing, request format, confidence scores and accuracy are unpublished. A tester on Oct 1 to 2 got HTTP 403 "Decision API is not enabled for this user" from `POST /v1/decisions` and docs pages returned 404. I found no evidence of broad rollout as of Oct 8. Luna itself is $0.10 in / $0.50 out per M. Revisit when it ships; Jev gives the same shape today.

### Small fast LLMs (Groq, Cerebras, Gemini Flash-Lite, Haiku)

Published numbers conflict, mostly from aggregators. Groq is most often reported as lowest TTFT for small models (about 80 to 190 ms), Cerebras strongest on throughput. One source shows the opposite ordering on a 70B model with 400 to 1800 ms TTFTs, so conditions differ. I found no ms figure for Gemini 3.5 Flash-Lite and no data at all for Haiku 5.5; Haiku 4.5 on Artificial Analysis sits at 0.58 to 0.96 s with a 10k-token prompt, which is too slow for a first word. Action: run a 20-call TTFT test from the Mac against Groq and one Gemini Flash-Lite model with the real short Juno prompt before choosing. Streaming must be on and the connection kept warm (the router already sets an 800 ms connect timeout; a warm keep-alive connection matters more than the model).

### On-device

Apple Foundation Models: no published TTFT for macOS 27. Xcode 27 has a Foundation Models Instrument showing TTFT, tokens per second and total latency, so Juno can measure it itself. A developer forum thread reports guided generation slowing from seconds to minutes on macOS 27 betas 5 to 7 (one person's report, fix status unknown). Context window is 4096 tokens. MLX small models: searched, found no measured numbers for Apple silicon, so no claim. Note that Juno's rule is that agents never run cargo locally; any on-device experiment needs CI builds.

Embedding or rule routers: claims range from under 4 ms to 100 ms for local routing, mostly from vendor-affiliated posts. These handle a fixed intent set well and fail on out-of-distribution queries, which is fine for picking a filler phrase and wrong for deciding safety.

### OpenAI realtime and Live: only what is new

- Agora measured GPT-Live latency (published after the 2026-07-09 test): median about 498 ms from last speech frame to first audio, n=30, one iPhone, ChatGPT consumer app, not the API. Its main finding is jitter: standard deviation fell from 489 ms to 104 ms against Advanced Voice Mode, P90 only 104 ms above the median. Under 10% uplink loss the median rose 314 ms versus 2448 ms for Advanced. Agora also says the GPT-Live API was waitlist-only at that time, which conflicts with the 2026-09-10 GA date you already have; treat the GA claim as needing a check against OpenAI's docs.
- Secondary sources quote 0.798 s turn-taking latency for gpt-live-1 versus 1.41 s for gpt-realtime-2.1 and 1.63 s for gpt-realtime-2 on Full Duplex Bench. Not verified against the benchmark itself.
- OpenAI has not published a time-to-first-audio number for gpt-live-1. An Eden AI table says about 900 ms (model unspecified, unverified).
- Implication: even the best realtime voice path lands around 0.5 to 0.8 s. Option 2 above (local filler) and option 3 (fast first sentence) reach the same range without moving the backend off Claude CLI.

## Sources

- Jev overview and comparison with Decisions API: https://www.firecrawl.dev/blog/openai-decisions-api-vs-jev
- AIMLAPI Jev page: https://aimlapi.com/models/typesafe-jev
- OpenRouter TypeSafe: https://openrouter.ai/typesafe
- Gradually.ai Jev page: https://www.gradually.ai/en/ai-models/jev/
- Nexos Jev guide (could not fetch, 403): https://nexos.ai/blog/what-is-jev/
- Infrabase: https://infrabase.ai/inference-apis/typesafe-jev
- Beri review: https://www.beri.net/tools/typesafe-jev
- Dealroom: https://app.dealroom.co/companies/typesafe_ai_jev
- OpenAI Decisions API coverage: https://www.eesel.ai/blog/openai-decisions-api and https://www.eesel.ai/blog/openai-decisions-api-pricing and https://cryptobriefing.com/openai-decisions-api-gpt-6-luna/
- GPT-6 Luna on OpenRouter: https://openrouter.ai/openai/gpt-6-luna
- Groq vs Cerebras roundups: https://dev.to/gowtham21/groq-vs-cerebras-which-is-fastest-llm-inference-in-2026-29hj and https://costbench.com/best/fastest-llm-inference/ and https://deploybase.ai/articles/groq-vs-cerebras-pricing-speed-and-benchmark-comparison
- Haiku 4.5 providers (Artificial Analysis): https://artificialanalysis.ai/models/claude-4-5-haiku/providers
- Gemini 3.5 Flash-Lite comparison (Artificial Analysis): https://artificialanalysis.ai/pt/models/comparisons/gemini-3-6-flash-vs-gemini-3-5-flash-lite
- Apple Foundation Models forum thread: https://developer.apple.com/forums/thread/843310
- WWDC 2026 session 8121: https://developer.apple.com/videos/play/wwdc2026/8121/
- Agora GPT-Live latency: https://prod.agora.io/en/blog/openai-didnt-publish-gpt-lives-latency-so-we-measured-it
- GPT-Live-1 launch summary: https://aiweekly.co/alerts/openai-launches-gpt-live-1-in-the-api-at-005minute-cuts-turn-taking-latency-to
- Embedding router latency: https://dailyaiworld.com/blogs/semantic-router-ai-agents-2026 and https://tianpan.co/blog/2026/04/16/intent-classification-agent-routers

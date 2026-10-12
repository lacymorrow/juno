# Kev research (2026-10-08)

## Summary verdict
Kev exists, is real, actively maintained, Apache-2.0, and runs on Apple Silicon via MLX with a one-command server. It serves TypeSafe's `POST /v1/systemone` contract, so one harness can hit Jev and Kev by switching base URL. The 0.8B probe on this M1 (16 GB) answered 9 of 13 Juno-style decisions correctly zero-shot, at 110-270 ms per single-question call (after a 11 s first call), 713 ms when packing 4 questions with a new state. That is under 1 s but not by a wide margin on an M1, and zero-shot accuracy on the mid-run and escalate questions is not usable. Fine-tuning on our labels is a first-class, documented path. Verdict: worth benchmarking as the open, local candidate; ship only after fine-tuning. Jev-compat means the same harness works for both. Production shipping from Rust is the hard part (see Effort).

## Existence check
- Kev (verified): github.com/jaredpalmer/kev, publisher Jared Palmer. Apache-2.0. Created 2026-09-17, last push 2026-10-06, 8,716 stars (API, 2026-10-08), 38 open issues. Release `kev-1.0` (GitHub) with 0.8B/4B/9B checkpoints + SHA-256. Description: "Jev-like family of decision models built on top of Qwen3.5/3.8".
- HF (verified): huggingface.co/jaredpalmer/kev-0.8b (16k downloads, modified 2026-10-01), also kev-4b, kev-9b, kev-27b, plus older kev-0.5b/0.6b/8b. 0.8b files: adapter_model.safetensors 43 MB, head.pt 2.1 MB; weights = Qwen3.5-0.8B-Base + LoRA r16 + pointer head. HF Space demo and dataset `jaredpalmer/kev-suites`.
- Family: 0.8B, 4B, 9B (LoRA on Qwen3.5 base), 27B (full-weight on Qwen3.8). The Pinggy "0.8B/4B/9B" is incomplete (there is a 27B).
- jevlike (verified): github.com/vinnylarouge/jevlike, MIT, created 2026-09-16, last push 2026-09-16, 1,352 stars. Publisher is an individual (vinnylarouge). It is a recipe/trainer with a from-scratch byte encoder or a frozen HF encoder plus an option-attention head, not a pretrained model and no /v1/systemone server. Byte encoder truncates context to 192 bytes. Not on PyPI (404). Some HF community derivatives exist (e.g. Pradheep1647/jevlike-qwen3.5-2b-*, experimental, ~0 downloads). Not pursued further; for Juno it is a build-your-own path, not a Jev alternative today.
- Not Kev: PyPI `kev` is an unrelated key-value ORM. Kev is not on PyPI; install from the repo (`uv sync --extra serve`).
- Also found, related: iapp/OpenThai-SystemOne (+GGUF, Qwen3.5-0.8B-Base, Apache-2.0, Thai), deepanwadhwa/OpenDecision, others. Not evaluated.
- Searched: Pinggy article, GitHub API/search, HF API search (kev, jevlike, systemone), PyPI (kev, jevlike, typesafe-sdk), web search.

## Serving + macOS
- Own FastAPI/uvicorn server: `python -m kev.serve --run jaredpalmer/kev-0.8b --port 8009`. Needs Python 3.12/3.13 and uv. CUDA, ROCm, or MLX (auto on Apple Silicon). Not vLLM/llama.cpp (verified in README). KEV_DTYPE, KEV_API_KEY, KEV_TEMPERATURE env vars.
- Verified on this Mac: log line "serving ... on mps via mlx (bfloat16)". Venv 1.1 GB, HF cache 1.7 GB (base weights bf16 ~1.5 GB + adapter). Server RAM: model card says MLX peak 2.7 GB at 8k-token state, process footprint ~5.3 GB (M5, vendor-measured); I did not measure RSS reliably.
- Vendor latency (M5 32 GB, MLX): 8k-token state 1.4 s new / 74 ms cached. Pinggy quotes M3 Pro: 77 ms MLX fresh, 47 ms repeated, 213 ms PyTorch MPS, first call 3.2 s.
- No GGUF/ONNX/CoreML export from Kev itself. OpenThai-SystemOne-GGUF shows a llama.cpp path exists for the same 0.8B architecture but the pointer head needs custom code (unverified).

## API compatibility
- Verified from README: request `{state, model, questions:{id:{type: noul|choice|score, instructions, criteria}}}`; response `answers[id]` with `choice`, `probabilities`, `confidence`; `x-typesafe-request-id` header; bearer auth. Confidence formulas copied from TypeSafe's `system-one-adapter` 0.2.1. Official `typesafe-sdk` works with `base_url` changed (README example). Extra routes `/permute`, `/separate`.
- Verified in probe: plain curl-style JSON with `type: choice` worked unchanged; I did not run the typesafe-sdk itself nor call real Jev (no key). Jev-side behavior of the same body is unverified by me, but the README claims contract parity.
- Caveat: Kev answers only noul/choice/score (same as System One). Our four decision types are all `choice`.

## Evidence
All vendor-reported (unverified independently) unless noted.
- Kev-0.8B: new-sources accuracy 0.648 dev / 0.697 test; trained-sources 0.827; Brier 0.481 new. Kev-4B 0.817/0.838; 9B 0.820/0.852; 27B 0.851/0.889; Jev 0.857 (dev only). Decision Index: 0.8B 23.3, 4B 38.0, 9B 41.0, 27B 52.3, Jev 54.0. Jev numbers come from Kev's author's own reads of Jev, not a controlled comparison (README says so).
- Model card: "out of domain it is markedly less accurate than Kev-4B"; JevBench public items 0.636 (hard tier 0.360) for 0.8B.
- Pinggy's 0.822 vs 0.857 figure is Kev-9B, 4.0% vs 3.7% confident error.
- Latency: L4 GPU 22.7 ms for six questions short state (vendor).
- Our probe (verified, M1 16 GB, one run, 13 cases, tiny sample): see below.

## Fine-tuning
- Verified in README: JSONL, same shape as API request plus `label` per question (choice = option name). `python -m kev.train --data train.jsonl --base Qwen/Qwen3.5-0.8B-Base --init_from jaredpalmer/kev-0.8b ...`; `kev.benchmark`; then serve the run dir. Always use `--init_from` (from-base scored 0.33 vs 0.84 in a user test). A `kev-finetune` agent skill automates it on Modal; ~$1 for a 4B run on H100. Vendor example: 4B 67.7% -> 73.6% on 1,050 generated records; real complaints 0.804 -> 0.904. 400 records was inside noise.
- Training script options show `--device cuda` and `--dtype bf16`; whether training on Mac MPS is practical for 0.8B is unverified. Rent a GPU (Modal) is the documented route.
- Fits Juno: labels are our options; fits temperature on our held-out slice.

## Probe results
Setup: repo at 5e42a7a (2026-10-05), venv under `kev-probe/venv`, HF cache under `kev-probe/hf`, run with `python -I`, MLX bf16, kev-0.8b, Apple M1 16 GB. One `choice` question per call, option descriptions as criteria, no fine-tuning. Raw output: `kev-probe/probe.out`, script `kev-probe/probe.py`.

Accuracy 9/13:
- route: chat OK (0.61), tools OK (0.58), computer_use OK but marginal (0.34 vs chat 0.33), "research five CRM tools, write report" -> tools (0.62), wanted escalate. MISS.
- settings (21 options incl. none): microphone OK (0.43 vs audio 0.30), hotkeys OK (0.32 vs dictation 0.31), "what's the weather" -> none OK (0.77).
- component: timer OK (0.88), "tell me a joke" -> none OK (0.91).
- mid-run: stop OK (0.71); "also make it 4 o'clock" -> new (wanted add) MISS; "hey honey did you take the dog out" -> add (wanted not_for_juno) MISS; "weather in Denver" -> add (wanted new) MISS.
Reading: component and none-detection strong; routing okay-ish; mid-run (no real context about the running task given, which Juno would supply) weak; many top probabilities near 0.3 so thresholds/fallback would be needed.

Latency (M1, loopback HTTP, client-measured):
- First request after start: 11.4 s (warmup/compile). Must prewarm at launch.
- Single question, new short state: 118-198 ms (median of the 13 about 145 ms).
- Same state repeated (20 runs): median 228 ms (noisy; machine had other load).
- New state, 21-option settings question (20 runs): median 267 ms, max 377 ms.
- 4 questions packed in one request, new state (10 runs): median 714 ms.
Takeaway: ~150-270 ms per decision on M1; four-at-once is 0.7 s, so budget by calling only the needed question, not all four. M-series Pro/Max/M5 should be faster (vendor M5 / Pinggy M3 Pro numbers are 2-4x lower, unverified here).

## Effort
- To first benchmark result: low. Done here in about 15 minutes wall clock, mostly downloads. A real benchmark = write ~100-300 labeled Juno utterances in the JSONL shape, run `kev.benchmark` or loop the HTTP API: half a day.
- To a fine-tuned 0.8B: label data (1-2k records recommended), one Modal/H100 run (~$1 scale), eval. 1-2 days including labeling.
- Shipping: server is Python + MLX + PyTorch stack. 1.1 GB venv, ~1.5 GB base weights bf16 (adapter+head only 45 MB). Not embeddable in the Rust/ONNX app as is. Options: (a) optional download that runs a sidecar Python server (heavy, fragile, notarization pain); (b) port to Rust (mlx-rs/candle) or export to ONNX/CoreML: Qwen3.5 uses DeltaNet/hybrid layers and a custom pointer head, so a port is real engineering (unverified difficulty). Quantized 4-bit 0.8B would be ~0.5 GB (estimate, Kev ships bf16 only; accuracy impact unmeasured). Realistic story: optional local download, not in the base bundle.
- Because of API parity, the lowest-risk plan is: develop harness against /v1/systemone, benchmark Jev vs Kev through base_url, decide, then port only if Kev wins.

## Ease-of-use score: 4/5
Existence, license, docs, Mac support, API parity, and fine-tuning path are all strong and worked first try (one venv, one command). Loses a point because shipping inside a Rust Tauri app has no ready path (Python sidecar, 1.5 GB weights, 11 s cold start), and zero-shot 0.8B accuracy is mediocre on our mid-run questions.

## Risks/unknowns
- Project is 3 weeks old; fast churn (rapid releases, 38 open issues). Pin `@v1.0` revision.
- Single maintainer; vendor-run evals; Jev comparisons are not controlled.
- 0.8B drops sharply out of domain; needs fine-tuning and calibration for our labels.
- Our probe: 13 cases, single run, M1, no context about the running task for mid-run questions.
- Latency is per-question sensitive: packed questions cost more; first call 11 s.
- Memory 3-5 GB vendor footprint (M5) is big next to a voice assistant already running Parakeet.
- Rust integration cost unknown (ONNX/MLX port of hybrid Qwen3.5 + pointer head).
- Did not test typesafe-sdk or real Jev.

## Publishable facts (public blog comparison)
Verified by direct check on 2026-10-08:
- Kev is Apache-2.0, by Jared Palmer, github.com/jaredpalmer/kev, created 2026-09-17, 8.7k stars, release kev-1.0.
- Four sizes (0.8B, 4B, 9B, 27B) on HF under jaredpalmer; 0.8B adapter is 43 MB plus 2 MB head on a Qwen3.5-0.8B-Base.
- Implements `POST /v1/systemone`; README shows the TypeSafe SDK working by changing base_url.
- Serves via its own FastAPI server; MLX on Apple Silicon is automatic. We ran 0.8B on an M1 16 GB: 118-270 ms per single-question decision, 714 ms with four questions in one request, 11 s first call; 9/13 correct on our small zero-shot test.
- jevlike is MIT, a recipe not a model, by an individual, 1.3k stars.
Vendor-reported (attribute, do not state as fact): Kev-9B 0.822 vs Jev 0.857 on new sources; Kev-27B 0.851; 0.8B Index 23.3 vs Jev 54.0; M5 latencies; fine-tune gains.

## Sources
- https://pinggy.io/blog/best_open_source_jev_alternatives_self_hosted_decision_models/
- https://github.com/jaredpalmer/kev (README, docs/model-cards/kev-0.8b.md, API, releases/kev-1.0)
- https://huggingface.co/jaredpalmer/kev-0.8b (HF API for sizes)
- https://github.com/vinnylarouge/jevlike
- https://pypi.org/pypi/typesafe-sdk/json ; https://pypi.org/project/kev/ (unrelated)
- https://huggingface.co/api/models?search=jevlike ; ?search=systemone
- https://pyshine.com/Kev-Trainable-Decision-Models-On-Qwen-Source-Tour/ ; https://huggingface.co/tt-hous/kev-9b
- https://archerhume.com/posts/jevs-architecture-unmasked (cited by Kev README, not read)

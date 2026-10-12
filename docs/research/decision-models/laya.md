# Laya research for Juno (2026-10-08)

## Summary verdict
Laya exists, is real and active, Apache-2.0, open weights, but it is a base to fine-tune, not a zero-shot router. Zero-shot on our made-up Juno questions it scored 15/20 (English) and 5/20 (multilingual) on the 4-way route question, and 9/15 and 12/15 on a 10-option settings question. Speed is good once fine-tuned weights exist: ~40 ms per call on MPS for the 322M multilingual checkpoint, 150+ ms for the 421M English one (on a heavily loaded Mac). Recommendation: worth a fine-tune benchmark (ModernBERT-large or mmBERT-base on ~hundreds to a few thousand labeled Juno utterances), not worth shipping zero-shot. The Rust path is the open problem: no published ONNX, only an export script.

## What it is
- Publisher: Convai Innovations (HF org convaiinnovations), maintainer GitHub NandhaKishorM. Repo https://github.com/NandhaKishorM/laya. VERIFIED (GitHub API, HF API).
- License Apache-2.0 (GitHub API). VERIFIED. HF API license field was empty; README says Apache-2.0.
- Repo created 2026-09-18, last push 2026-10-07, latest release v0.4.0 (2026-10-07), ~30 releases in 3 weeks, 31,662 stars, 120 open issues. VERIFIED. (Press pieces quote 4.6k and 19.3k stars at earlier dates; stars growth is fast, treat as hype-adjacent.)
- Checkpoints, all VERIFIED on HF:
  - convaiinnovations/laya: ModernBERT-large, 421M, 512 ctx, model.safetensors 843 MB (fp32 file; the 421M count and 843 MB imply fp16/bf16 storage). English.
  - convaiinnovations/laya-multilingual: mmBERT-base, 322M, 644 MB, 1024 ctx (up to 8192), 100+ languages. HF also has it as subfolder multilingual/ in the main repo.
  - convaiinnovations/laya-typed-decisions: ModernBERT-large, fine-tuned on the typed-decisions benchmark.
  - HF downloads: laya 36k, multilingual 2.5k, typed-decisions 0.9k.
- No paper. Architecture write-up is a dev.to article and secondary press. VERIFIED absence (README links none). Training objective is called RLCD (proper-scoring-rule rewards, GRPO-style).

## Format
- VERIFIED from README. `Router().predict(state, questions)` or `laya.load(repo).predict(state, questions)`. State = string, dict, list (conversation). Questions = dict of id to {type, instructions, criteria}.
- Types: `choice` (criteria dict label to description), `score` (ordinal rubric list), `noul` (yes/no, returns P(true)).
- Multiple questions per pass: yes, "single forward pass for N questions". Published: 1 q 39.5 ms, 10 q 158 ms on T4, so cost still grows with question count (each question adds sequence-side work). Our probe: 2 questions in one call ~206 ms on MPS multilingual vs ~40 ms for one, so not free.
- Output: answers[qid]["choice"], probabilities, confidence, plus low_confidence flag via min_confidence. Calibration: one temperature per question type fit post-training; reported ECE 0.466 to 0.081 after refit (secondary source) and 0.246 Jev vs 0.081 Laya (README). Base checkpoint raw ECE 0.213. Probabilities are only calibrated after you fit temperatures on your own data.
- Option limit: options share a fixed head budget (head_max_len 192 English, 256 multilingual), roughly (head_max_len-16)/k tokens per option. 77 options: Banking77 0.425 vs Jev 0.870. Floor of 4 tokens per option; past ~head_max_len/4 (about 48-64) options the head overflows the cap and truncates the state. Workarounds: raise head_max_len to 384/512 (58-label MASSIVE test 24/58 to 34/58, 1.4x CPU time), predict_shortlist (embed then top-k), predict_tournament (groups of 16, 2 passes, BANKING77 0.43 to 0.61). Our 10 and ~30 settings options are inside the safe zone (~7+ tokens per option at 256 for 30; keep descriptions short), but verify.
- Known defects documented by the author (README Honest limits): negation unsafe in choice ("don't cancel" picks cancel); noul can follow label text, English noul with no criteria answers "no" always; multilingual has first-position bias on score; avoid yes/no/true/false as choice keys; choice order sensitivity (35.7% answers flip when options reversed on base, 6.5% after fine-tune with shuffle). VERIFIED as author claims, not independently tested by me.

## Evidence
All figures are the author's own, never third-party reproduced, except where noted.
- typed-decisions (2,000 decisions): base 0.362 (multilingual 0.352), random 0.318, majority 0.461; fine-tuned laya-typed-decisions 0.766; Jev 0.727 (published by TypeSafe). The fine-tuned model was trained on that benchmark's own training split.
- Routed Laya vs Jev (README, Jev numbers third-party): AG News 0.950 vs 0.910, DAIR Emotion 0.595 vs 0.480, Banking77 0.425 vs 0.870 (Jev leads above ~20 options), jabr 49-task benchmark 0.583 vs 0.966 (secondary summary at pinggy.io).
- Latency: 39.5 ms (English) and 32.8 ms (multilingual) for 1 question on a Tesla T4; 7.2 ms/q batched of 10. Jev 236-276 ms p50 (third-party, AbdelStark/jev-benchmarks, nibzard). Pinggy quotes the author's M3 Pro at 66 ms median for 3 questions, no GPU acceleration (not independently checked).
- Fine-tuning gain on 20-option MASSIVE intent (English): base 0.730 to 0.936-0.941 with 11.5k rows. VERIFIED in docs/finetune.md (author numbers).

## Running on macOS + Rust path
- PyTorch: `pip install laya` (v0.4.0) works on Apple Silicon, Python 3.10+, torch 2.14, transformers 5.x. Device auto-picks CUDA, MPS, CPU. VERIFIED by me (torch 2.14.1, MPS available). First MPS call has Metal compile warmup.
- ONNX: no prebuilt ONNX on HF. Repo ships `laya-ts/scripts/export_onnx.py` (split encoder.onnx + head.onnx, verified to 1e-4 vs torch) and a laya-java/laya-dotnet path with fused laya.onnx via prepare_checkpoint.py; `laya[onnx]` extra and `ONNXAgent` exist. VERIFIED to exist by reading source; I did not run the export. No int8 export or int8 size published (UNVERIFIED). Estimate: int8 of 421M ~ 420-450 MB, of 322M ~ 330 MB (arithmetic, not measured). Compare Parakeet already bundled.
- Rust: none. No candle or ort port. To use `ort` in Juno you must (a) reimplement tokenization and the Laya prompt layout (option markers, head_max_len budgeting, marker_pos/marker_mask, qtype inputs to head.onnx) in Rust, or (b) port from laya-ts / laya-java, which already do this against ONNX Runtime without Python. Those SDKs are the reference for a Rust port; the sequence-building logic (build_sequence in laya/common.py) is the real work. Calibration temperatures live in rl_agent_config.json.
- Universal binary: ONNX Runtime linked on aarch64 only (per brief); Intel would need the CPU ORT build or a gating like Parakeet.

## Fine-tuning
- Supported and the author's main recommendation. `laya-train --data file.csv|jsonl --base english|multilingual --out dir` on one device (CUDA, MPS, CPU); also Kaggle 2xT4 notebook and an MPS script for 16 GB MacBooks. Fits per-type temperatures on a held-out slice, writes questions.json. Options: --loss rlcd|soft-ce, --shuffle-options (recommended for many options), --freeze-encoder (head only, small gains). VERIFIED in docs/finetune.md.
- Data volume: author runs used 6,000 decisions (typed-decisions, one T4) and 11,514 rows (MASSIVE 20-way). No minimum stated; calibration warns if <10 held-out items. For Juno, expect a few hundred to a few thousand labeled utterances per question; the four Juno questions can be trained as separate choice questions in one JSONL. Full fine-tune of the 421M on a T4 is roughly hours for 30k questions (author: 4-5 h on 2xT4 for ~30k); our dataset would be much smaller. UNVERIFIED for our data.

## Probe results
Environment: this Mac (Darwin 25.6, Apple Silicon), venv under the report dir, `python -I`, laya 0.4.0, torch 2.14.1. IMPORTANT: machine was heavily loaded by other agents (load average 25-50), so latencies are inflated and noisy; a quiet machine would be faster. Median of 20 (route) or 15 (settings) calls after one warmup call. 4 route labels (chat, tools, computer_use, escalate) and 10 settings options, my own short descriptions, zero-shot, no tuning, one question per call. Script: laya-probe/laya_probe.py, raw outputs laya-probe/out_*.txt.

| checkpoint | device | load | route p50 | route acc | settings p50 | settings acc |
|---|---|---|---|---|---|---|
| laya 421M | CPU | 34 s | 414 ms | 15/20 | 218 ms | 9/15 |
| laya 421M | MPS | 15 s | 153 ms | 15/20 | 174 ms | 9/15 |
| laya-multilingual 322M | CPU | 28 s | 176 ms | 5/20 | 129 ms | 12/15 |
| laya-multilingual 322M | MPS | 9 s | 45 ms | 5/20 | 39 ms | 12/15 |

Two questions in one call: ~206 ms on MPS multilingual, ~2.7 s on first MPS English call (compile), 874 ms CPU English.
Sensible? English route misses: "tell me a joke" to tools, "weather in Charlotte" to chat, all three escalate examples to tools. Confidences on misses were low (0.07-0.23), so a confidence gate would catch most. English settings misses: five of six went to "none" (hotkey, voice, delete history, sounds, Spanish), one-line descriptions too thin; confidences 0.18-0.45. Multilingual route collapsed to computer_use for 14 of 20 inputs, often at confidence 0.9, which is the worst failure (confident and wrong); multilingual settings were decent (12/15). This matches the author's "near chance zero-shot" statement. Small n, my labels, my criteria wording; indicative only.
Not run: mid-run input question, component question, int8/ONNX, accuracy after fine-tuning, quiet-machine latency.

## Effort
- First benchmark result: about half a day. Label ~300-1000 utterances per question (can reuse Juno logs), write JSONL, run `laya-train` on MPS (or a rented T4), evaluate with `laya-evals`. No Rust needed for that step; accuracy decides go or no-go.
- Shipping in Juno: 1-2 weeks of Rust. Export ONNX (script exists), build tokenizer + prompt layout in Rust against laya-ts/laya-java as reference, validate against Python goldens (laya-dotnet has regen_golden.py), calibrate temperatures, add int8 if needed. Bundle size 330-840 MB so it should be an optional/first-run download (like the Parakeet model), not in the dmg. ORT on aarch64 only; Intel path needs a separate decision. Rust-side latency estimate 40-150 ms on M-series for one question (extrapolated from PyTorch MPS numbers, UNVERIFIED).

## Ease-of-use score: 3 / 5
Python side is a 5: pip install, three-line API, built-in trainer, MPS support, large docs. Dropped to 3 because zero-shot is weak (Juno needs a tuned model), no ONNX artifact and no Rust port, young code (3 weeks, releases almost daily, 120 open issues), documented sharp edges (negation, noul labels, option order), and per-call latency grows with number of questions.

## Risks/unknowns
- Brand-new project; API churn (0.3.x to 0.4.0 changed the routing default). Single maintainer.
- Benchmark numbers are mostly author-run; Jev numbers third-party. The 0.766 win is on the author's own benchmark after training on its split. Press claims ("3.9% more accurate", "7.8x faster") are selective; the broader jabr benchmark has Jev at 0.966 vs Laya 0.583.
- Confidently wrong outputs possible (multilingual route).
- Mid-run input ("stop / add / new / not for Juno") is short-text intent classification with negation-like phrasing ("don't stop"), which the README flags as a weakness. Test early.
- Intent classification with few labels and short text is also doable with a small embedding classifier (see sibling probes in this directory) at a fraction of the size; Laya's edge is the shared pass over arbitrary options and calibrated probabilities.
- Unclear Rust port cost until the head.onnx input layout is read in full.

## Publishable facts (public blog comparison)
Safe to state, with attribution:
- Laya is an Apache-2.0, open-weight, non-autoregressive decision model from Convai Innovations; 421M (ModernBERT-large, English) and 322M (mmBERT-base, 100+ languages) checkpoints, 843 MB and 644 MB on Hugging Face (verified).
- First released 2026-09-18; v0.4.0 on 2026-10-07; ~31.7k GitHub stars at 2026-10-08 (verified).
- Supports choice, score and yes/no question types and multiple questions per forward pass (verified in README).
- The author reports 39.5 ms (English) and 32.8 ms (multilingual) per question on a Tesla T4, and states zero-shot accuracy near chance on its own typed-decisions benchmark (0.362 vs 0.318 random) with 0.766 after fine-tuning. Say "author-reported".
- The author states it trails Jev on large option sets (Banking77 0.425 vs 0.870) and on the jabr benchmark.
- Our own measurement: on an Apple Silicon Mac under heavy load, zero-shot one-question calls took ~40 ms (multilingual, MPS) to ~150 ms (English, MPS) and ~130-410 ms on CPU; zero-shot accuracy on 20 hand-written assistant requests across 4 routes was 15/20 (English) and 5/20 (multilingual). Small sample, unoptimized prompts; label it as such.
Do not state: int8 sizes, Rust speed, or independent confirmation of the author's accuracy claims.

## Sources
- GitHub repo and README: https://github.com/NandhaKishorM/laya (README, docs/finetune.md, laya-ts/scripts/export_onnx.py, laya-java/MODELS.md, laya/onnx_agent.py), GitHub API for stars, releases, license.
- Hugging Face: https://huggingface.co/convaiinnovations/laya , /laya-multilingual , /laya-typed-decisions (file sizes via HF API; eval/results.md).
- https://www.datacamp.com/blog/top-open-source-jev-alternatives
- https://pinggy.io/blog/best_open_source_jev_alternatives_self_hosted_decision_models/ (Convai attribution, Apache-2.0, M3 Pro 66 ms, jabr 0.583 vs 0.966)
- Secondary: https://aiweekly.co/alerts/convai-ships-laya-a-421m-modernbert-decision-model-apache-20 , https://pub.towardsai.net/what-is-laya-laya-vs-jev-explained-simply-f7125dd3e582 , Jev third-party benchmarks https://github.com/AbdelStark/jev-benchmarks and https://github.com/nibzard/decision-model-benchmark (cited by the Laya README, not opened by me).
- Probe: laya-probe/laya_probe.py and out_en_cpu.txt, out_en_mps.txt, out_ml_cpu.txt, out_ml_mps.txt (kept locally, not committed).

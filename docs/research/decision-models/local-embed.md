# Local embedding + per-question heads as a Jev alternative for Juno

Date 2026-10-08. Research only, no repo changes. [V] = verified this session (page fetched or measured), [U] = unverified (from memory / not confirmed).

## 1. Summary verdict

Feasible and easy. Recommended: **Model2Vec `potion-base-8M` (7.5M params, MIT) via `model2vec-rs`, with one logistic-regression head per question exported as a plain f32 matrix.** No ONNX Runtime, so it works identically on aarch64 and x86_64, no native dependency, ~30 MB f32 (about 8 MB int8) [V size claim from README], embedding cost is microseconds (measured 0.07 ms/utterance in Python, 8k samples/s single thread claimed for Rust). The 20 ms budget is met by two orders of magnitude.

Why not a transformer encoder by default: the repo's ONNX Runtime story is aarch64-only. `~/repo/juno/tauri-plugin-voice-transcription/Cargo.toml` gates `parakeet-rs` (which pulls `ort-sys` 2.0.0-rc.13, per Cargo.lock) to aarch64 because ort-sys ships a prebuilt only for aarch64-apple-darwin and broke the x86_64 half of the universal build [V, from the file comment]. A web check indicates Microsoft stopped shipping x86_64 macOS ORT prebuilts after 1.23 and the workaround is `load-dynamic` plus a bundled dylib [U, single third-party source]. So an ort-based MiniLM/bge path would need either aarch64-only, or `candle`/`tract` on Intel, or a bundled x86_64 libonnxruntime. Static embeddings sidestep all of it.

Fallback if accuracy on real data is short: bge-small-en-v1.5 or MiniLM-L6 through `candle` (pure Rust, both arches) or ort on aarch64 + candle on Intel. Expect roughly 5 to 15 ms per utterance [U for Rust numbers; Python torch measured below].

Honest caveat: a static model is a bag of word vectors. It cannot tell "stop" the command from "stop" in "don't stop the music" as well as a transformer, and mid-run input (d) is the hardest question for it. Benchmark (d) first.

## 2. Candidates table

| Model | Params | Size | License | CPU latency | English quality |
|---|---|---|---|---|---|
| potion-base-8M | 7.56M [V] | ~30 MB f32, ~8 MB int8 [V README says ~8 MB smallest] | MIT [V] | 0.07 ms measured (Python); Rust 8k samples/s claimed [V] | MTEB classification 70.34, avg 51.08 [V] |
| potion-base-32M | 32.3M [V] | ~30 MB claimed for "best" [V, README inconsistent] | MIT repo [V]; model card license not checked | similar to 8M [U] | higher than 8M, number not fetched [U] |
| all-MiniLM-L6-v2 | 22.7M [U] | ~90 MB f32 ONNX, ~23 MB int8 [U] | Apache-2.0 [U] | 15.7 ms measured, torch CPU, M1, single utterance | MTEB classification ~63 [U] |
| bge-small-en-v1.5 | 33.4M [V] | ~130 MB f32, ~33 MB int8 [U] | MIT [V] | 46 ms measured in torch (noisy, likely thread overhead; ONNX would be faster) | MTEB classification avg 74.14 [V] |
| gte-small | 33M [U] | ~130 MB f32 [U] | MIT [U] | like bge-small [U] | classification ~72 [U] |
| EmbeddingGemma-300M | ~300M [V] | ~1.2 GB f32; QAT Q4/Q8 checkpoints exist [V] | Gemma terms, gated download [V] | too heavy for 20 ms CPU in a bundle [U] | MTEB Eng v2 mean 69.67; no classification split given [V]; Q8 69.49 [V] |
| nomic-embed-text-v1.5 | 137M [U] | ~550 MB f32 [U] | Apache-2.0 [U] | too heavy [U] | not checked |
| mxbai-embed-xsmall-v1 | ~22M [U] | ~90 MB [U] | Apache-2.0 [U] | MiniLM-like [U] | not checked |

Note: no embedding model reports a rigorous "published CPU latency" on the cards I could fetch; latencies above are my measurements or marked claims.

## 3. Rust integration path

| Runtime | aarch64 + x86_64 mac without extra native deps? | Notes |
|---|---|---|
| `model2vec-rs` | Yes, pure Rust + `tokenizers`; default `onig` backend is a small C lib compiled by cargo, `fancy-regex` feature avoids it [V features, U that onig builds cleanly in universal] | Loads safetensors f32/f16/i8 + tokenizer.json. Best fit. |
| `ort` 2.0.0-rc.13 | aarch64 yes (prebuilt, already linked via parakeet). x86_64 no prebuilt in this setup [V from Cargo.toml comment] | Intel needs `load-dynamic` + shipped dylib [U]. rc status, API churn. |
| `fastembed-rs` v7 | Built on ort + tokenizers [V], so inherits the Intel problem | Easiest API for transformer models (bge-small, MiniLM, EmbeddingGemma listed [V]). Its ort pin must match parakeet's rc.13 or Cargo will carry two `ort-sys` (links conflict) [U, check]. |
| `candle` | Yes, pure Rust, CPU, both arches [U] | BERT-family (MiniLM/bge/gte) supported; slower than ORT, fine for ~10 ms class [U]. |
| `tract` | Yes, pure Rust [U] | Runs ONNX; operator coverage for BERT is OK but fiddly [U]. |

Tokenizer: Hugging Face `tokenizers` crate (Cargo.lock already has 0.22.2 and 0.23.2 [V], so a third copy is avoidable by matching one). Model2Vec needs `tokenizer.json` only; BERT models need `tokenizer.json` too.

Integration shape (static path): load model once at startup (~30 MB mmap/read), `encode(utterance)` -> 256-d f32 vector, for each question do `softmax(W x + b) / T`, write results into the existing decision struct. Under 100 lines of Rust plus a build-time weights file. Models, heads and label lists ship as resources, not code.

## 4. Heads and calibration

- **Plain LR on frozen embeddings** is the right default. SetFit adds contrastive fine-tuning of the encoder; it helps most at 8 to 16 examples per class and matters little once you have 50+ per class [U, SetFit paper claims]. It also forces a fine-tuned encoder per question or shared, which breaks the "one embedding, many heads" economy. Skip it unless LR plateaus.
- kNN/centroid: zero training, good for (b) with ~30 destinations when examples are scarce, but worse calibrated. Useful as a baseline.
- Examples needed [U, rule of thumb]: 20 to 50 per class gives usable LR on clear intents; 100+ for confusable pairs (add-to-task vs new-request). Route (a) with 4 classes and (b) with 31 classes (30 + none) are different regimes: 30 x 40 = 1,200 examples for (b). Generate with an LLM, then hand-check.
- **Calibration**: fit temperature (single scalar T on held-out logits) or Platt/isotonic per head with sklearn `CalibratedClassifierCV`; multinomial LR is already near-calibrated with L2 regularization, so temperature scaling is enough. Use cross-validated logits, not training logits.
- **Export as weights**: `coef_` (K x D) and `intercept_` (K) as a flat f32 JSON or `.safetensors`/raw `.bin`, plus label list and T. Rust: `logits = W.x + b; p = softmax(logits / T)`. K x 256 floats per head; all four heads under 100 KB.

## 5. "None" / out-of-distribution

- Add an explicit **none** class trained with hard negatives (chat that mentions a setting word, "Wi-Fi is slow today", for (b); speech to someone else for (d) "not for Juno"). This is more reliable than thresholds alone.
- Then also a **max-prob threshold** after calibration, with abstain = none/escalate. Pick the threshold on a held-out set to a target precision.
- Static embeddings give flat OOD behavior: probe below shows OOD max-prob 0.43 to 0.75 for gibberish and side-talk, so a threshold alone would not reject "uh yeah no I was talking to my wife" (0.75 on potion). An explicit none class is required. Optionally add a distance-to-nearest-training-example gate.
- Route (a) already has "escalate" as the safe fallback; low confidence should map to escalate to the large model, so errors cost latency, not correctness. This is the main design lever.

## 6. Probe results (toy data, M1, CPU, Python 3.14 venv, `python -I`)

Setup: 60 hand-written utterances (20 chat, 20 tools, 20 computer-use), stratified 60/40 split, LR (C=10) on normalized embeddings, test n=24. Script: `probe.py` in this dir. Latency = mean wall clock for encoding a single utterance in a loop (Python overhead included).

| Model | Embed latency / utterance | Acc (split seed 0) | Acc mean of 5 splits | Dim | OOD max-prob (gibberish, side-talk, "numbers look fine") |
|---|---|---|---|---|---|
| potion-base-8M (model2vec 0.9.0) | 0.07 ms | 0.88 | 0.83 | 256 | 0.43, 0.75, 0.50 |
| all-MiniLM-L6-v2 (torch, 4 threads) | 15.7 ms | 0.79 | 0.81 | 384 | 0.47, 0.40, 0.50 |
| bge-small-en-v1.5 (torch, 4 threads) | 46 ms (noisy) | 1.00 | 0.89 | 384 | 0.49, 0.49, 0.38 |

Reading: toy data, 24 test items, so differences of a few points are noise (one item = 4 points). The meaningful results are latency and that all three reach ~0.8 to 0.9 with 12 training examples per class. Torch single-sentence latency is pessimistic for the transformers; an ORT/candle int8 build would likely be several times faster [U]. This is not evidence on mid-run input or the 30-way settings question.

## 7. Effort

- First real benchmark result: **about 1 day** for one person. Build a labelled set (LLM-generate ~1,000 to 2,000 utterances across the 4 questions plus negatives, spot check), run the script above across potion-base-8M/32M, bge-small, MiniLM with 5-fold CV, report accuracy, per-class P/R, calibration (ECE) and abstain curve. Pure Python, no cargo.
- Ship in Juno: **about 3 to 5 days** after the benchmark. Add `model2vec-rs`, resource bundling for model + 4 head files, a `local_decision` module returning probabilities in the same shape Jev returns, a confidence-to-escalate policy, a test pinning head label order to the Rust enums (see the dead-control pattern memory), and CI on both arch jobs. If a transformer is needed on Intel: add 2 to 4 days for candle path or dylib bundling.

## 8. Ease-of-use score: 4 / 5

Static path is a 4: no runtime or arch problems, tiny bundle, microsecond latency, heads are just matrices, retraining is a Python script. Loses one point because the labelled data and the harder questions (mid-run input, 30-way settings, none handling) are the real work, and a static model may cap accuracy. Transformer path would be a 3 given the Intel ONNX Runtime situation.

## 9. Risks / unknowns

- Accuracy of static embeddings on short, negation-heavy, ASR-noisy utterances (d). Unmeasured. Mitigate by escalating on low confidence.
- Train/serve skew: ASR transcripts differ from typed text; train on transcribed speech.
- ort double-linking if fastembed is added next to parakeet-rs (links conflict) [U].
- x86_64 ORT prebuilt status is from one third-party source [U].
- `onig` build in a universal build [U]; `fancy-regex` is the escape hatch.
- potion-base-32M license and score not verified here; EmbeddingGemma is license-gated and too large.
- Heads need retraining whenever Juno adds a settings destination or component; versioned label files mitigate.
- Jev quality comparison needs the same eval set run through Jev; not done.

## 10. Publishable facts (public blog comparison)

- potion-base-8M: 7.56M params, MIT, MTEB classification 70.34 (model card) [V]. bge-small-en-v1.5: 33.4M params, MIT, MTEB classification avg 74.14 [V]. EmbeddingGemma: ~300M, Gemma license, MTEB Eng v2 mean 69.67 [V].
- Model2Vec claims up to 500x CPU speedup vs the teacher model and up to 50x smaller; model2vec-rs about 8,000 samples/s single thread, 1.7x Python [V as vendor claims].
- Our measurement (Apple M1, single utterance, toy 60-utterance set): static 0.07 ms vs MiniLM-L6 torch 15.7 ms vs bge-small torch 46 ms; all 0.8 to 0.9 accuracy on a 3-way route. Label as toy, not a benchmark.
- Do not publish Jev comparisons until both are run on the same held-out set.

## 11. Sources

- ~/repo/juno/tauri-plugin-voice-transcription/Cargo.toml and ~/repo/juno/Cargo.lock (ort 2.0.0-rc.13, parakeet-rs 0.3.7, tokenizers 0.22.2 and 0.23.2)
- https://github.com/MinishLab/model2vec-rs
- https://github.com/MinishLab/model2vec
- https://huggingface.co/minishlab/potion-base-8M
- https://huggingface.co/BAAI/bge-small-en-v1.5
- https://huggingface.co/google/embeddinggemma-300m
- https://github.com/Anush008/fastembed-rs
- https://github.com/pykeio/ort
- https://raw.githubusercontent.com/samvallad33/vestige/main/docs/INSTALL-INTEL-MAC.md (x86_64 ORT prebuilt claim, unconfirmed)
- Probe: probe.py and venv in this directory

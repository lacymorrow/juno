# Voice Model Research — May 2026

Research audit for upgrading Juno's speech-to-text (STT) and text-to-speech (TTS) stack.

**Current stack:**
- STT: Whisper v1 `ggml-tiny.en` (~39MB, 39M params) via `whisper-rs 0.11.0`
- TTS: ElevenLabs API (primary), Replicate API (secondary), macOS `say` (offline fallback)

---

## Part 1: Speech-to-Text (STT)

### Current Performance Baseline

Whisper tiny.en: ~5.6% WER on LibriSpeech-clean, ~14.9% on LibriSpeech-other. Fast but lowest-accuracy Whisper variant. No streaming — chunked processing only.

### STT Model Comparison

| Model | Params / Size | WER (LS-clean / avg) | Latency | Local/Cloud | License | Rust Path | Streaming | Languages |
|-------|---------------|----------------------|---------|-------------|---------|-----------|-----------|-----------|
| **Whisper tiny.en (current)** | 39M / ~39MB | ~5.6% clean, ~14.9% other | 32x RT | Local | MIT | whisper-rs (direct) | Chunked | English |
| **Whisper large-v3-turbo** | 809M / ~1.6GB | ~7.75% avg | 129x RT (2.3x faster than v3) | Local | MIT | whisper-rs (direct) | Chunked | 99 langs |
| **Distil-Whisper large-v3** | 756M | Within 1% of large-v3 | 6.3x faster than v3 | Local | MIT | whisper-rs via GGML | Chunked | 99 langs |
| **NVIDIA Parakeet TDT 0.6B v2** | 600M | **1.69% clean, 3.19% other** | RTFx 3386 | Local | CC-BY-4.0 | **parakeet-rs crate** | **Yes (160ms)** | English |
| **NVIDIA Parakeet TDT 1.1B** | 1.1B | Better than 0.6B | RTFx ~1212 | Local | CC-BY-4.0 | parakeet-rs | **Yes** | English |
| **NVIDIA Canary Qwen 2.5B** | 2.5B | **5.63% avg (#1 HF leaderboard)** | Moderate | Local | NVIDIA license | ONNX via ort | No | Multilingual |
| **Moonshine Tiny** | 27M / ~26MB | 12.66% avg | **34ms latency** | Local | MIT | ONNX via ort | **Yes** | English |
| **Moonshine Medium** | 245M | **6.65% avg** (beats Whisper v3) | 107ms / 10s audio | Local | MIT | ONNX via ort | **Yes** | English |
| **Qwen3-ASR 0.6B** | 600M | Competitive SOTA | 2000x throughput | Local | Apache 2.0 | MLX / ONNX | Yes | **52 langs** |
| **Deepgram Nova-3** | N/A | 5.26% general | Sub-300ms | Cloud | Proprietary | HTTP | **Yes** | 36+ langs |
| **Groq Whisper** | large-v3 | Same as Whisper | **228x RT** | Cloud | Proprietary | HTTP | Batch only | 99 langs |

### STT Key Findings

1. **Sesame is TTS, not STT.** Sesame AI Labs makes a Conversational Speech Model (CSM) for speech generation. Not relevant for transcription.

2. **NVIDIA Parakeet TDT 0.6B v2 is the accuracy king.** 1.69% WER on clean speech — 70% better than our current tiny model, 78% better than Whisper large-v3. Has a native Rust crate (`parakeet-rs` on crates.io, v0.2.8) with ONNX Runtime backend. Also has `parakeet.cpp` with Metal GPU acceleration (~27ms for 10s audio on Apple Silicon).

3. **Moonshine Medium is the best size-to-accuracy tradeoff.** 245M params beats Whisper large-v3's 1.5B params at 6.65% avg WER. Native streaming, MIT license, ONNX models for Rust via `ort` crate.

4. **Whisper large-v3-turbo is the zero-effort upgrade.** Same `whisper-rs` crate, just swap the model file. No code changes. ~7.75% avg WER, 2.3x faster than large-v3.

5. **`parakeet.cpp` on Apple Silicon** achieves ~27ms encoder inference for 10s of audio using Metal GPU — 96x faster than CPU.

### STT Recommendation: Two-Phase Approach

**Phase 1 — Immediate (config change only):**
Upgrade default model from `ggml-tiny.en.bin` to `ggml-large-v3-turbo-q5_0.bin` (~600MB quantized). Zero code changes — just swap the model path in `config.rs`. Add settings UI for model size selection. This gives ~7.75% avg WER (massive accuracy improvement).

**Phase 2 — Medium-term (new engine):**
Migrate to `parakeet-rs` with NVIDIA Parakeet TDT 0.6B v2:
- 1.69% WER on clean speech (best-in-class)
- Native streaming (160ms chunks vs Whisper's batch-only)
- Speaker diarization built in
- `parakeet-rs` crate exists and is actively maintained
- ONNX Runtime works on macOS CPU and GPU (CoreML)

The `SharedWhisperManager` would need to be abstracted into a trait-based `TranscriptionEngine` that can back either whisper-rs or parakeet-rs, with engine selected at runtime via settings.

---

## Part 2: Text-to-Speech (TTS)

### TTS Model Comparison

| Model | Quality | Latency | Local/Cloud | Size | License | Voice Clone | Streaming | Rust Crate | Emotion Control |
|-------|---------|---------|-------------|------|---------|-------------|-----------|------------|-----------------|
| **ElevenLabs (current)** | Gold standard | 75-300ms | Cloud | N/A | Proprietary | Yes | Yes | No | Yes (v3) |
| **Kokoro-82M** | Very Good | **<300ms** | Local (CPU) | **82M** | Apache 2.0 | No (54 voices) | Yes | **Yes (4+ crates)** | No |
| **Orpheus TTS** | Excellent | ~130-200ms | Local (GPU) | 150M-3B | Apache 2.0 | Yes | Yes | No | **Yes (8 tags)** |
| **Chatterbox (Resemble)** | Excellent | <200ms | Local (GPU) | 350M+ | MIT | Yes | Yes | No | **Yes (slider)** |
| **Fish Speech S2** | Top-tier | Fast | Both | Moderate | Apache 2.0 | Yes | Yes | No | Yes (RL) |
| **Sesame CSM** | Exceptional | **Too slow for RT** | Local (GPU) | 1B | Apache 2.0 | Yes | Impractical | Yes (csm.rs) | Excellent |
| **Dia2 (Nari Labs)** | Excellent | Streaming | Local (GPU) | 1-2B | Apache 2.0 | Yes | **Yes (true)** | No | Yes (dialogue) |
| **Zonos (Zyphra)** | Excellent | 2x RT | Local (GPU) | 1.6B | Apache 2.0 | Yes | Limited | No | **Yes (5 params)** |
| **F5-TTS** | Very Good | Sub-7s | Local (GPU) | Small | MIT | Yes | No | No | Limited |
| **Piper** | Decent | Instant | Local (CPU) | 15-80MB | GPL | No | Yes | **Yes (3+ crates)** | No |
| **OpenAI TTS** | Good | ~500ms | Cloud | N/A | Proprietary | No | Yes | No | No |
| **Cartesia Sonic 3** | Good | **40ms** | Cloud | N/A | Proprietary | Yes | **Yes (WS)** | No | Yes |
| **Qwen3-TTS** | SOTA | Moderate | Local (GPU) | 1.7B | Apache 2.0 | Yes | Yes | **Yes (ONNX)** | Yes |
| **Bark (Suno)** | Good variety | **41s/3 sentences** | Local | Large | MIT | Limited | No | No | Non-verbal |
| **Tortoise** | Excellent | **~2 min/sentence** | Local | Large | Apache 2.0 | Yes | Impractical | No | Excellent |
| **Parler TTS** | Good | Moderate | Local | 880M-2.3B | Apache 2.0 | Text-described | Yes | No | Text-controlled |
| **Mars5 (Camb.ai)** | Decent | Moderate | Both | 1.2B | **AGPL** | Yes | No | No | Limited |
| **MetaVoice** | Good | <1.0 RTF | Both | 1.2B | Apache 2.0 | Yes (en) | Yes | No | Moderate |
| **StyleTTS 2** | Academic SOTA | Fast | Local | Moderate | MIT | Limited | Possible | No | Style transfer |
| **NVIDIA FastPitch** | Good | Fast | Local | Small | Apache 2.0 | Limited | Via Riva | No | Pitch control |

### TTS Key Findings

1. **Sesame CSM is impressive but too slow for real-time.** Even on H200 GPU, streaming latency is impractical for conversational use. The quality is exceptional — natural pauses, breath, chuckles — but it cannot serve as a real-time TTS engine today. Has a Rust implementation (`csm.rs`) but latency is the blocker.

2. **Kokoro-82M is the clear winner for local TTS.** 82M params, runs on Apple Silicon CPU, sub-300ms latency, Apache 2.0. Hit #1 on TTS Arena leaderboard. Has 4+ Rust crates: `kokoroxide`, `tts-rs`, `any-tts` (with Metal GPU acceleration and pure-Rust phonemizer), `kokorox`. No voice cloning, but 54 preset voices is sufficient for a desktop assistant.

3. **Orpheus TTS has the best expressiveness.** Supports emotion tags (`<laugh>`, `<chuckle>`, `<sigh>`, `<cough>`, `<gasp>`, etc.) that map well to Juno's `<TTS>` content filtering. 150M-3B variants. Apache 2.0. ~130ms TTFB. No Rust crate — needs Python subprocess or ONNX export.

4. **Chatterbox (Resemble AI) is a strong alternative.** MIT license, 63.75% preferred over ElevenLabs in blind tests, emotion exaggeration slider, neural watermarking. 350M turbo variant runs on 4GB GPU.

5. **Coqui TTS/XTTS: avoid.** Company shut down, CPML license is non-commercial for model weights. Legally risky.

6. **Mars5: avoid.** AGPL license — copyleft requires source disclosure. Problematic for proprietary desktop app.

7. **Bark and Tortoise: skip.** Far too slow for real-time (41s and 2min per output respectively).

8. **NVIDIA FastPitch: outdated.** Two-stage pipeline (spectrogram + vocoder) adds complexity. Superseded by newer models.

### TTS Recommendation: 3-Tier Architecture

**Tier 1 — Local Fast (default + offline fallback): Kokoro-82M**
- Replaces macOS `say` command
- Runs on Apple Silicon CPU, no GPU needed
- Sub-300ms latency, Apache 2.0, 54 voices
- Use `any-tts` Rust crate (Metal acceleration, pure-Rust phonemizer)
- Dramatically better quality than `say` with zero cloud dependency
- Fallback chain becomes: ElevenLabs -> Kokoro (instead of ElevenLabs -> `say`)

**Tier 2 — Cloud High-Quality (primary when online): Keep ElevenLabs**
- Remains quality leader (MOS 4.3)
- Consider Cartesia Sonic (40ms TTFA) if latency becomes critical
- Consider Fish Audio S2 as cost-effective alternative
- Replicate provider could be swapped for Fish Audio API

**Tier 3 — Future Local GPU: Orpheus-150M or Chatterbox Turbo**
- For users with capable GPU (Apple M-series with 16GB+ unified memory)
- Orpheus emotion tags integrate well with Juno's `<TTS>` system
- Chatterbox has MIT license and emotion exaggeration control
- Both need Python subprocess or ONNX conversion — less clean than Kokoro's native Rust path

---

## Part 3: Implementation Roadmap

### Quick Wins (config changes, no architecture changes)

1. **Upgrade Whisper model to large-v3-turbo** — swap `model_path` default in `tauri-plugin-voice-transcription/src/config.rs`. Add model download on first run. Add settings UI for model selection.

2. **Add Kokoro-82M as TTS provider** — add `any-tts` or `kokoroxide` to `src-tauri/Cargo.toml`. Implement `KokoroTts` alongside existing `ElevenLabsTts`, `ReplicateTts`, `SystemTts` in `src-tauri/src/tts/`. Update fallback chain.

### Medium-Term (architecture changes)

3. **Abstract transcription engine** — extract `SharedWhisperManager` into a `TranscriptionEngine` trait. Implement `WhisperEngine` and `ParakeetEngine`. Engine selection via settings.

4. **Integrate parakeet-rs** — add `parakeet-rs` dependency. Implement `ParakeetEngine` with streaming support. Enable 160ms chunk streaming for real-time feedback.

5. **Model management UI** — settings page for selecting STT model (Whisper tiny/small/medium/large-v3-turbo, Parakeet 0.6B/1.1B) and TTS provider (Kokoro local, ElevenLabs, Replicate, System). Show model download progress.

### Long-Term (future considerations)

6. **Orpheus/Chatterbox TTS for GPU users** — detect available GPU memory, offer high-quality local TTS when hardware permits.

7. **Sesame CSM monitoring** — quality is exceptional. If latency improves (hardware or model optimization), re-evaluate as local TTS option.

8. **Qwen3-ASR for multilingual** — when multilingual support becomes a priority, Qwen3-ASR (52 languages, Apache 2.0) is the strongest candidate.

---

## Part 4: Rust Integration Summary

| Integration | Crate | Backend | Effort | Notes |
|-------------|-------|---------|--------|-------|
| Whisper (current) | `whisper-rs` 0.11 | whisper.cpp | Zero (swap model) | Supports all Whisper GGML models |
| Parakeet STT | `parakeet-rs` 0.2.8 | ONNX Runtime | Medium | Streaming, diarization, multiple models |
| Parakeet.cpp STT | Custom FFI | C++/Metal | High | Fastest on Apple Silicon |
| Moonshine STT | `ort` crate | ONNX Runtime | Medium | Edge-optimized, true streaming |
| Kokoro TTS | `any-tts` / `kokoroxide` | ONNX Runtime | **Low** | Metal GPU, pure-Rust phonemizer |
| Piper TTS | `piper-rs` | ONNX Runtime | Low | Very lightweight, GPL license concern |
| Cloud STT (Deepgram/Groq) | `reqwest` | HTTP | Low | Cloud-dependent, adds cost |

---

## Sources

### STT
- [NVIDIA Parakeet TDT 0.6B v2](https://huggingface.co/nvidia/parakeet-tdt-0.6b-v2) — HuggingFace
- [parakeet-rs crate](https://crates.io/crates/parakeet-rs/0.2.8) — crates.io
- [parakeet.cpp](https://github.com/Frikallo/parakeet.cpp) — GitHub
- [Moonshine](https://github.com/moonshine-ai/moonshine) — GitHub
- [Whisper large-v3-turbo benchmark](https://whispernotes.app/blog/introducing-whisper-large-v3-turbo)
- [Open ASR Leaderboard](https://huggingface.co/spaces/hf-audio/open_asr_leaderboard) — HuggingFace
- [NVIDIA Canary Qwen 2.5B](https://huggingface.co/nvidia/canary-qwen-2.5b) — HuggingFace
- [Qwen3-ASR](https://github.com/QwenLM/Qwen3-ASR) — GitHub
- [Best open source STT 2026](https://northflank.com/blog/best-open-source-speech-to-text-stt-model-in-2026-benchmarks) — Northflank

### TTS
- [Sesame CSM](https://github.com/SesameAILabs/csm) — GitHub
- [csm.rs Rust implementation](https://github.com/cartesia-one/csm.rs) — GitHub
- [Kokoro-82M](https://huggingface.co/hexgrad/Kokoro-82M) — HuggingFace
- [any-tts Rust crate](https://crates.io/crates/any-tts) — crates.io
- [Orpheus TTS](https://github.com/canopyai/Orpheus-TTS) — GitHub
- [Chatterbox](https://github.com/resemble-ai/chatterbox) — GitHub
- [Fish Speech](https://github.com/fishaudio/fish-speech) — GitHub
- [Dia2](https://github.com/nari-labs/dia2) — GitHub
- [Zonos](https://github.com/Zyphra/Zonos) — GitHub
- [TTS Arena Leaderboard](https://tts-agi-tts-arena-v2.hf.space/leaderboard)
- [Cartesia Sonic](https://cartesia.ai/sonic) — Cartesia

# Intel (x86_64) support via a universal build

Status: implemented (LAC-4089). This file is now the record of what was done
and why, not a proposal. The sections that turned out to be wrong when the code
was read are marked.

## The one-line problem

`parakeet-rs` (the Parakeet speech-to-text engine, via `ort-sys`) ships a
prebuilt ONNX Runtime only for `aarch64` macOS. It was an unconditional
dependency, so `--target universal-apple-darwin` failed to link the x86_64
half, which is why the release was pinned to `aarch64-apple-darwin` from
65b4d20b (2026-09-12). The fix is not a build flag; it is making Parakeet
arm64-only and letting Intel fall back to Whisper.

Ground truth for the claim, rather than the commit message repeating it:
`ort-sys-2.0.0-rc.13/build/download/dist.tsv` lists the prebuilt targets. The
Apple entries are `aarch64-apple-darwin` (coreml and coreml,webgpu) and the two
iOS triples. There is no `x86_64-apple-darwin` row. Microsoft stopped shipping
`x64-osx` ONNX Runtime binaries; ort's static-link path still knows the name
(`static_link/mod.rs:106`) but there is nothing to download.

## Why this is safe to do

The default STT provider is already `Whisper` (`SttProvider::default`). Whisper
(`whisper-rs`, Metal feature) builds on both arches. So on Intel we lose only
the *option* of Parakeet, and the default experience is unchanged. Nobody who
never touched the setting notices.

`any-tts` (Kokoro TTS) was checked for the same problem and does not have it:
it is Candle-backed, not ONNX, so `ort` enters the graph through `parakeet-rs`
alone.

## The design decision

Keep the `SttProvider::Parakeet` enum variant present on every architecture.
Do NOT `cfg` the variant itself. A user who selected Parakeet on an Apple
Silicon machine has `"parakeet"` written to their Tauri Store; that value must
still deserialize on an Intel machine (or after a config sync) rather than
panic or fail the whole settings load. So the variant stays; only its
*implementation* is gated.

On x86_64, selecting Parakeet resolves to Whisper with a logged warning. The
setting is accepted, it just cannot be honoured, the same way an unavailable
model falls back rather than erroring.

**The gate runs along the dependency, not along the feature.** The earlier draft
of this plan proposed defining a second `ParakeetModelStatus` for x86_64. That
is unnecessary: the download manifest, the byte-size check and the status type
touch only the filesystem and have no `parakeet-rs` dependency at all. They are
now `src/parakeet_model.rs`, compiled everywhere; `src/engine_parakeet.rs` keeps
the loader and is compiled on aarch64 only. Nothing is duplicated.

An Intel Mac still needs to answer "is Parakeet downloaded?" — that is what the
Models pane is built from, and a pane has to describe a machine that cannot run
the engine as accurately as one that can.

## Files and the exact change in each

1. **`tauri-plugin-voice-transcription/Cargo.toml`**
   `parakeet-rs = "0.3"` moved from `[dependencies]` into
   `[target.'cfg(target_arch = "aarch64")'.dependencies]`. This is the change
   that unblocks the x86_64 link; everything else is making the code compile
   without the crate present.

2. **`tauri-plugin-voice-transcription/src/parakeet_model.rs`** (new, every arch)
   `PARAKEET_HF_REPO`, `PARAKEET_HF_REVISION`, `ParakeetFile`,
   `PARAKEET_MODEL_FILES`, `parakeet_total_bytes`, `parakeet_file_url`,
   `missing_parakeet_files`, `parakeet_model_files_present`,
   `ParakeetModelStatus` — all moved out of `engine_parakeet.rs`.
   `ParakeetModelStatus` gained a `supported` field so the two reasons a person
   cannot use Parakeet (not downloaded / not built for this Mac) are never
   reported as one.

3. **`tauri-plugin-voice-transcription/src/engine_parakeet.rs`** (aarch64 only)
   Reduced to the loader: `ParakeetEngine`, `ParakeetSession`, the optimised
   graph cache, and the live transcription test.
   `ParakeetEngine::model_files_present` is gone — it was a one-line wrapper
   over `missing_parakeet_files`, which callers now use directly.

4. **`tauri-plugin-voice-transcription/src/engine.rs`**
   New `parakeet_is_supported() -> bool` (`cfg!(target_arch = "aarch64")`), and
   `startup_provider` consults it. Without that, an Intel Mac with a synced
   settings store logs "STT provider: parakeet" and then runs Whisper.

5. **`tauri-plugin-voice-transcription/src/engine_manager.rs`**
   The Whisper construction is extracted into `build_whisper`, and
   `build_engine`'s `SttProvider::Parakeet` arm is split by arch: the existing
   loader on aarch64, `build_whisper` plus a warning on x86_64. Tests assert
   which engine a Parakeet request reaches on each arch.

6. **`tauri-plugin-voice-transcription/src/commands.rs`**
   `ParakeetModelStatus` now imported from `parakeet_model`. No `cfg` needed:
   `get_parakeet_model_status` compiles and answers on both arches.

7. **`tauri-plugin-voice-transcription/src/utils.rs`**
   `ParakeetEngine::model_files_present` → `parakeet_model_files_present`.

8. **`tauri-plugin-voice-transcription/src/config.rs`**, **`src-tauri/src/state_management.rs`**
   No change, as the earlier draft predicted.

9. **`src-tauri/src/commands/stt_models.rs`**
   The earlier draft listed "frontend (settings UI)" as outstanding work. It was
   already done: `ModelDef::arm64_only`, `catalog_for_arch`, `tier_for`,
   `recommended_id` and `current_arch` were written with this gate in mind, with
   tests asserting an Intel Mac never sees Parakeet and gets large-v3-turbo as
   its Balanced row. The frontend reads that status and needed nothing.

   What was missing is the gate on the paths that take a model *id* rather than
   reading the catalog. A new `arch_supports(def)` is checked first in
   `is_downloaded`, which stops `active_model_id` naming a model
   `catalog_for_arch` omits (the Models pane would render with no active row),
   and is checked in `start_download` (replacing an inline copy of the same
   rule) and in `use_stt_model` (so the error says "only available on Apple
   Silicon" rather than a misleading "not downloaded yet").

## CI

- **`.github/workflows/release-tauri.yml`**: `targets:` is
  `aarch64-apple-darwin,x86_64-apple-darwin` again and `args:` is
  `--target universal-apple-darwin`. The "Apple Silicon only" comment block is
  replaced with what is now true.
- **`.github/workflows/ci.yml`**: a new `rust-intel` job runs
  `cargo check --all-targets --target x86_64-apple-darwin` on both crates.
  **This is the part the earlier draft missed and the most important addition.**
  The existing `rust` job compiles the runner's own architecture, so before this
  job nothing checked x86_64 until a release build — the slowest and most
  expensive place to discover a missed `#[cfg]`. `check` rather than
  `clippy`/`test`: lints do not vary by architecture and the suite already runs
  on arm64; this job exists to prove the Intel half compiles.
- **`.github/workflows/build-branch.yml`**: a `target` input
  (`aarch64-apple-darwin` default, `universal-apple-darwin` available), both
  targets installed, and `lipo -info` printed after the build. Without this
  there is no way to get a *branch* onto an Intel Mac, which is what the
  verification step below needs.

## The updater manifest

No work. Tauri derives the manifest from the bundle, and a universal build
emits both platform keys. Verified against v0.6.0, the last universal release:

```
darwin-aarch64      → Juno_universal.app.tar.gz
darwin-x86_64       → Juno_universal.app.tar.gz
darwin-aarch64-app  → Juno_universal.app.tar.gz
darwin-x86_64-app   → Juno_universal.app.tar.gz
```

v0.8.15 (aarch64) has `darwin-aarch64` and `darwin-aarch64-app` only, which is
exactly why Intel installs stranded on v0.6.0 see no update. They start updating
again the first time a universal release publishes.

The asset *names* change with the target: `Juno_universal.app.tar.gz`, not
`Juno_aarch64.app.tar.gz`. `scripts/juno-build.sh` hardcoded the aarch64 name
and now matches `Juno_*.app.tar.gz`, so installing any release — before
65b4d20b, between, or after — keeps working.

## How to verify (the part that cannot be reasoned, only run)

1. CI's `rust-intel` job is the compile proof, and it runs on every PR. Do not
   run cargo locally (see "Rust: CI Compiles, Not Your Mac").
2. A universal branch build for an Intel Mac:
   ```
   gh workflow run build-branch.yml --ref <branch> -f target=universal-apple-darwin
   ```
   The run log's `lipo -info` line must read `x86_64 arm64`. Download the
   `juno-<sha>` artifact and copy it to graphite.
3. On **graphite** (an Intel Mac, `100.78.6.88`): dictation works on Whisper,
   Settings > Models lists four rows with Whisper large-v3-turbo as Balanced and
   no Parakeet row, and the log line
   `[VoicePlugin] STT provider: ... parakeet built in: false` is present. This is
   the real test; an arm64 host cannot prove the Intel path.
4. After merge, the first release build's `latest.json` must contain a
   `darwin-x86_64` key.

## Risk

The remaining unknown is transitive and only the build reveals it: other native
crates must also produce x86_64 objects. `whisper-rs` with the Metal feature and
`any-tts` with `candle-*/metal` are the two that matter; Metal exists on Intel
Macs, so both should compile, but "should" is what the `rust-intel` job is for.
Budget for a compile-fix loop on the x86_64 half rather than a clean first pass.

## Not in scope

Windows. It needs the computer-use MCP path rewritten (no `juno-cua`/CGEvent
equivalent), which is a separate and much larger effort.

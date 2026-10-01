# Intel (x86_64) support via a universal build

Status: implemented. Every build Juno produces is now
`--target universal-apple-darwin`. This doc is the record of why, what moved,
and the two things only a real release can confirm.

## The one-line problem

`parakeet-rs` (the Parakeet speech-to-text engine, via `ort-sys`) ships a
prebuilt ONNX Runtime only for `aarch64` macOS. It used to be an unconditional
dependency, so `--target universal-apple-darwin` built the arm64 half and
failed to link the x86_64 one, and the release was pinned to
`aarch64-apple-darwin` (commit `65b4d20b`) rather than let every release fail.

The fix was not a build flag. Parakeet is now arm64-only and Intel falls back
to Whisper.

## Why that is safe

The default STT provider is already Whisper (`SttProvider::default`), and
`whisper-rs` with the Metal feature builds on both architectures. On Intel we
lose only the *option* of Parakeet. Nobody who never touched the setting
notices.

## The design decision

The `SttProvider::Parakeet` enum variant exists on every architecture.
Deliberately. A person who chose Parakeet on an Apple Silicon machine has
`"parakeet"` written to their Tauri Store, and that value has to keep
deserializing on an Intel machine rather than fail the whole settings load. So
the variant stays and only the implementation is gated.

On x86_64, booting or switching to Parakeet resolves to Whisper with a logged
warning, the same way an unavailable model falls back rather than erroring.

## Where things live now

- **`tauri-plugin-voice-transcription/Cargo.toml`** has `parakeet-rs` under
  `[target.'cfg(target_arch = "aarch64")'.dependencies]`. This is the change
  that unblocks the link; `ort` and `ort-sys` are the only crates it pulls in,
  so the x86_64 dependency graph loses them entirely.
- **`src/parakeet_model.rs`** (new, compiled everywhere) holds everything that
  never touches the loader: the pinned HuggingFace manifest, the on-disk check,
  `ParakeetModelStatus`, and the arch gate itself as `PARAKEET_SUPPORTED`
  (`cfg!(target_arch = "aarch64")`) with `PARAKEET_UNSUPPORTED_REASON` for the
  UI to show.
- **`src/engine_parakeet.rs`** is now only the loader and the session, and is
  compiled on aarch64 alone.
- **`src/engine_manager.rs`** splits the `SttProvider::Parakeet` match arm by
  arch. Both arms, and the plain Whisper arm, build their Whisper engine
  through one `whisper_engine` helper, so the Intel fallback reports a missing
  model file exactly like an ordinary Whisper request.
- **`src/lib.rs`** asks `parakeet_model::parakeet_ready(dir)` at startup, which
  is `PARAKEET_SUPPORTED && every file on disk`. Asking only about the files
  would boot an engine this binary does not contain.
- **`src-tauri/src/commands/stt_models.rs`** already filtered Parakeet out of
  the catalog at runtime (`arm64_only` plus `catalog_for_arch`), so the Models
  pane on Intel simply has no Parakeet row. A test now pins that filter to
  `PARAKEET_SUPPORTED` so the catalog flag cannot drift from what the build
  links. The onboarding offer is no longer gated on `is_arm64`: it offers
  whatever the Balanced row is for this Mac, which is large-v3-turbo on Intel.

## CI

Every workflow that builds the app installs both Rust targets and builds
`--target universal-apple-darwin`:

- `.github/workflows/release-tauri.yml`
- `.github/workflows/demo-build.yml` (the junebug.ai demo)
- `.github/workflows/build-branch.yml` (what `juno-build` installs)

`release-cua.yml` was already building both halves and `lipo`ing them; it is
unchanged.

`scripts/tauri-build.sh` defaults to the universal target when no `--target` is
given, so `bun run tauri:build` is universal and the separate
`bun run build:universal` script is gone.

The Rust cache `shared-key` moved from `tauri-release-aarch64` to
`tauri-release-universal`. The first build after this lands is cold.

## The updater

`tauri-action` expands a universal `.app.tar.gz` into `darwin-aarch64`,
`darwin-x86_64`, `darwin-aarch64-app` and `darwin-x86_64-app` keys in
`latest.json`, all pointing at the same artifact
(`src/upload-version-json.ts`; verified present in the compiled `dist/index.js`
on the `v0` tag this repo pins).

That matters in both directions. `tauri-plugin-updater` 2.x looks up only
`{os}-{arch}-{installer}` then `{os}-{arch}`; it has no `darwin-universal`
fallback. A manifest with only a `darwin-universal` key would break updates for
*every* install, arm64 included. The expansion is what keeps that from
happening, and it is the single most important thing to eyeball on the first
real release.

The release asset name changes with the target, from `Juno_aarch64.app.tar.gz`
to `Juno_universal.app.tar.gz`. `scripts/juno-build.sh` now matches
`Juno_*.app.tar.gz` so it still installs the tags that already exist.

## What a real release still has to confirm

1. `latest.json` on the first universal release carries both `darwin-aarch64`
   and `darwin-x86_64`.
2. An existing arm64 install updates to it (the key it reads must not have
   moved).
3. `lipo -info` on the shipped binary shows `x86_64 arm64`.
4. Juno runs on graphite (Intel, `100.78.6.88`): dictation works on Whisper and
   the Models pane has no Parakeet row.

## Risk

The x86_64 half has never compiled. `parakeet-rs` was the known blocker, but
any other crate that links a prebuilt library could be a second one. The
likeliest candidate is `whisper-rs` with the `metal` feature: Metal exists on
Intel Macs and whisper.cpp builds its Metal backend there, but that is the kind
of thing only the build reveals. Budget for a compile-fix loop on the x86_64
half, not a clean first pass.

## Not in scope

Windows. It needs the computer-use MCP path rewritten (no `juno-cua`/CGEvent
equivalent), which is a separate and much larger effort.

# Intel (x86_64) support via a universal build

## The one-line problem

`parakeet-rs` (the Parakeet speech-to-text engine, via `ort-sys`) ships a
prebuilt ONNX Runtime only for `aarch64` macOS. It is an unconditional
dependency, so `--target universal-apple-darwin` fails to link the x86_64
half, which is why the release is pinned to `aarch64-apple-darwin` today. The
fix is not a build flag; it is making Parakeet arm64-only and letting Intel
fall back to Whisper.

## Why this is safe to do

The default STT provider is already `Whisper` (`SttProvider::default`). Whisper
(`whisper-rs`, Metal feature) builds on both arches. So on Intel we lose only
the *option* of Parakeet, and the default experience is unchanged. Nobody who
never touched the setting notices.

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

## Files and the exact change in each

Reference counts are from `grep -rn parakeet` at time of writing (76 lines, 7
files). Each below is the change, not just the location.

1. **`tauri-plugin-voice-transcription/Cargo.toml`**
   Move `parakeet-rs = "0.3"` out of `[dependencies]` and into:
   ```toml
   [target.'cfg(target_arch = "aarch64")'.dependencies]
   parakeet-rs = "0.3"
   ```
   This is the change that actually unblocks the x86_64 link. Everything else
   is making the code compile without the crate present.

2. **`tauri-plugin-voice-transcription/src/lib.rs`**
   Gate the module and its command:
   ```rust
   #[cfg(target_arch = "aarch64")]
   pub mod engine_parakeet;
   ```
   The `get_parakeet_model_status` command in the handler list stays (see
   commands.rs) so the frontend can always call it.
   In the async init block (~line 130-160) the `parakeet_model_dir` resolution
   is harmless to keep, but the `EngineManager::initialize(provider, ...,
   Some(&parakeet))` call must pass the provider through unchanged; the manager
   decides the fallback (item 4).

3. **`tauri-plugin-voice-transcription/src/engine_parakeet.rs`**
   No internal change. The module is compiled only on aarch64. Its public
   types (`ParakeetEngine`, `ParakeetModelStatus`) are referenced from
   `engine_manager.rs` and `commands.rs`, which must therefore also gate those
   references (items 4, 5).

4. **`tauri-plugin-voice-transcription/src/engine_manager.rs`**
   The `build_engine` match has `SttProvider::Parakeet => { ParakeetEngine... }`.
   Split it by arch:
   ```rust
   #[cfg(target_arch = "aarch64")]
   SttProvider::Parakeet => { /* existing ParakeetEngine construction */ }
   #[cfg(not(target_arch = "aarch64"))]
   SttProvider::Parakeet => {
       tracing::warn!("Parakeet is Apple Silicon only; using Whisper on this Mac");
       let ctx = SharedWhisperManager::initialize(whisper_model_path)?;
       Ok(Arc::new(WhisperEngine::new(ctx)))
   }
   ```
   Drop the top-of-file `use crate::engine_parakeet::ParakeetEngine;` behind
   `#[cfg(target_arch = "aarch64")]`.

5. **`tauri-plugin-voice-transcription/src/commands.rs`**
   - `use crate::engine_parakeet::ParakeetModelStatus;` → gate, and provide an
     x86_64 stand-in so the return type of `get_parakeet_model_status` exists on
     both arches. Simplest: define `ParakeetModelStatus` (a small serde struct
     with an `available: bool` / `reason`) in a place compiled on both arches,
     or return a shared status enum. On x86_64 the command returns
     `{ available: false, reason: "not supported on Intel Macs" }`.
   - The `"parakeet" => SttProvider::Parakeet` parse arm stays (the variant
     exists everywhere). The manager handles the fallback, so no cfg needed
     here.

6. **`tauri-plugin-voice-transcription/src/config.rs`**
   No change. `parakeet_model_dir` and the `SttProvider` field stay; default is
   already Whisper.

7. **`src-tauri/src/state_management.rs`**
   No change (the only reference is a comment).

8. **Frontend (settings UI)**
   Whatever pane lets the user choose Parakeet should reflect availability
   rather than offer a dead choice. Read `get_parakeet_model_status`: when it
   returns unavailable, hide or disable the Parakeet option with the reason.
   This is the one place a person sees the difference on Intel, so it gets the
   empty/disabled state designed, not a silently broken radio button.

## CI

`.github/workflows/release-tauri.yml` currently pins `args: --target
aarch64-apple-darwin` and installs only that rust target (line ~34). After the
gating:
- add `x86_64-apple-darwin` to the `targets:` line,
- change `args:` to `--target universal-apple-darwin`,
- delete the "Apple Silicon only" comment block (lines ~86-91) since it no
  longer holds,
- confirm the updater manifest gets a `darwin-x86_64` entry so Intel installs
  can auto-update (they currently get none by design).

## How to verify (the part that cannot be reasoned, only run)

1. `rustup target add x86_64-apple-darwin` (already done on zero).
2. Build both halves locally: `bun run build:universal` (already wired in
   package.json as `--target universal-apple-darwin`). The x86_64 half is the
   one that has never compiled; watch for `ort-sys` / `parakeet` link errors,
   which mean a reference was missed.
3. `lipo -info` the built binary shows `x86_64 arm64`.
4. Run it on **graphite** (an Intel Mac, `100.78.6.88`): dictation should work
   on Whisper, and the Parakeet option should be absent or disabled. This is
   the real test; the arm64 host cannot prove the Intel path.

## Risk

The unknown is transitive: other native crates (`whisper-rs` Metal,
`chromiumoxide`, anything linking a prebuilt lib) must also produce x86_64
objects. whisper's Metal feature compiles on Intel Macs (Metal exists there),
but this is exactly the kind of thing that only the build reveals. Budget for a
compile-fix loop on the x86_64 half, not a clean first pass.

## Not in scope

Windows. It needs the computer-use MCP path rewritten (no `juno-cua`/CGEvent
equivalent), which is a separate and much larger effort.

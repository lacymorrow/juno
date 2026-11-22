# Which PRs get a macOS runner

`ci.yml` decides per PR whether the macOS jobs (`rust-macos`, `rust-intel`, `homebrew-formula-macos`) run. The hosted macOS pool is small, so a PR that cannot reach the Rust suite does not ask for it.

- A `changes` job lists the PR's files. These paths skip macOS: `docs/`, `marketing/`, `media/`, root `*.md`, and `src/` except `src/components/settings/` (read by a Rust test in `src-tauri/src/settings/registry.rs`).
- Every other path runs Rust. A push to `main` always runs it.
- The required check `Rust (fmt + clippy + test)` is an ubuntu job that always reports, so a skipped macOS job never leaves the PR `BLOCKED`.

If you add a Rust test that reads a file outside `src-tauri/`, make sure that path is not in the skip list above.

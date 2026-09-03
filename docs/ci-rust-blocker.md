# Rust CI blocker: `src-tauri` cannot build from a clean checkout

**Status:** RESOLVED 2026-07-23 — option 3 (replace the dependency).
**Discovered:** 2026-07-23, while adding build CI (PR #477)

> `playwright` 0.0.20 was replaced with **chromiumoxide 0.9** on branch
> `feat/replace-playwright-with-cdp`. `src-tauri` now builds from a clean
> checkout with no network fetch of any third-party binary, and
> `.github/workflows/ci.yml` gained a `Rust (fmt + clippy + test)` job on
> macOS. The rest of this document is kept as the post-mortem.

## Symptom

`cargo check` / `cargo test` in `src-tauri` fails from any clean environment —
GitHub CI, a freshly cloned dev machine, or a fresh Paperclip worktree — with:

```
error: failed to run custom build command for `playwright v0.0.20`
  thread 'main' panicked at playwright-0.0.20/src/build.rs:70:5:
  file size is smaller than the driver
```

It succeeds on existing dev machines only because of a **stale cached driver**
at `/tmp/build-playwright-rust/driver.zip` left over from an older build.

## Root cause

`src-tauri/Cargo.toml` depends on `playwright = "0.0.20"` (used in
`src/state.rs` and `src/utils/resource_manager.rs` for browser control).

`playwright` 0.0.20 is the **latest published version** — the crate is
effectively abandoned. Its build script (`src/build.rs`) downloads a browser
driver **at build time** with a hardcoded version and URL:

```rust
const DRIVER_VERSION: &str = "1.11.0-1620331022000";   // Playwright 1.11.0, 2021-05-06
// https://playwright.azureedge.net/builds/driver{next}/playwright-{ver}-{platform}.zip
```

Then asserts the download is a real driver:

```rust
fn check_size(p: &Path) { assert!(size(p) > 10_000_000, "file size is smaller than the driver"); }
```

`playwright.azureedge.net` has been **decommissioned by Microsoft** (Playwright
moved to `playwright.download.prss.microsoft.com` / `cdn.playwright.dev`). The
old URL now returns **HTTP 404**, so the "driver" written to disk is a small 404
body, the size assert fails, and the build panics.

There is no environment-variable escape hatch in the build script; the only
non-download path (`only-for-docs-rs`) writes an empty file, which also fails the
size assert.

## Why it matters beyond CI

- New contributors cannot build the app without first obtaining a cached driver.
- Fresh Paperclip execution worktrees are similarly fragile.
- Any local "cargo check passed" is only trustworthy if the cache happens to be
  present — it is not a real clean-build signal.

## Options (a real decision, not a config tweak)

1. **Fork/patch `playwright-rust`** to point at the current Microsoft CDN and a
   supported driver version, consumed via `[patch.crates-io]`. Cleanest correct
   fix; keeps the dependency.
2. **Vendor the driver** and seed `/tmp/build-playwright-rust/driver.zip` (or the
   crate's expected cache path) in CI and setup docs. Keeps the abandoned crate
   alive but hides the fragility; not recommended as the only measure.
3. **Replace `playwright-rust`** with a maintained browser-automation approach
   (e.g. driving the JS Playwright over CDP, or `chromiumoxide`). Largest change,
   removes the abandoned dependency entirely.

## Resolution

**Option 3 was taken.** `chromiumoxide` speaks CDP directly from pure Rust: no
bundled Node runtime, no driver download at build time, and no 17.7MB blob
`include_bytes!`d into every binary. The port was close to 1:1 because page
operations already went through JavaScript evaluation.

The decisive detail, found while investigating: the crate's
`/tmp/build-playwright-rust/driver.zip` cache is gated behind
`cfg!(debug_assertions)`, so **release builds never consulted it** and always
attempted the dead download. `bun run tauri build` had therefore been failing
for everyone, on every machine — only debug builds worked, and only where a
stale driver happened to be cached. The first successful release build in this
repo since the CDN went dark was produced on the replacement branch.

A copy of the old 2021 driver (sha256 `29a5901…`, 17,723,787 bytes) is preserved
at `~/.local/share/juno-build-artifacts/` should the old path ever need
reconstructing.

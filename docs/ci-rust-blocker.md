# Rust CI blocker: `src-tauri` cannot build from a clean checkout

**Status:** open
**Discovered:** 2026-07-23, while adding build CI (PR #477)

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

Until one of these lands, CI intentionally gates only the frontend
(`tsc` + `vite build`); see `.github/workflows/ci.yml`.

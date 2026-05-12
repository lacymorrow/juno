# Releases & Auto-Updater

How Juno gets shipped, and how the in-app updater works.

## TL;DR — cutting a release

```bash
cd ~/repo/juno
bun run release patch   # or minor / major / explicit version
```

That single command does everything. The rest of this doc explains what's happening under the hood and how to fix things when they break.

---

## Architecture

Juno ships as a **universal macOS DMG** (arm64 + x86_64 lipo'd together). Two parallel pipelines run on every release:

```
bun run release patch
  │
  ├── 1. Local: bump versions across all Cargo.toml + package.json
  ├── 2. Local: build juno-cua CLI (arm64 + x86_64 + universal lipo)
  ├── 3. Local: git commit + tag (v0.X.Y + cua-v0.X.Y)
  ├── 4. git push origin HEAD --tags
  │       │
  │       ├── triggers .github/workflows/release-tauri.yml on v0.X.Y
  │       │     → universal macOS build (~30 min)
  │       │     → publishes GitHub Release with DMG, .app.tar.gz,
  │       │       .sig, and latest.json
  │       │
  │       └── triggers .github/workflows/release-cua.yml on cua-v0.X.Y
  │             → publishes juno-cua binaries to its own release
  │             → updates lacymorrow/homebrew-tap
  │
  ├── 5. Local: npm publish juno-cua
  ├── 6. Local: update homebrew-tap formula (for juno-cua)
  └── 7. Local: poll for CI DMG (waitForDmg, 45min timeout)
          → once published, write juno-www/public/downloads/release.json
          → commit + push juno-www
```

Two GitHub Releases get created per version: `v0.X.Y` (the Tauri app) and `cua-v0.X.Y` (the CLI). They live in the same repo but represent different artifacts.

## Auto-updater

The in-app updater uses [`tauri-plugin-updater`](https://v2.tauri.app/plugin/updater/).

**How it works at runtime:**
1. User clicks "Check for Updates" → `useUpdater().checkForUpdates()` (`src/hooks/useUpdater.ts`)
2. Plugin fetches `https://github.com/lacymorrow/juno/releases/latest/download/latest.json`
3. Plugin parses the manifest, compares against current `Cargo.toml` version
4. If newer, returns `Update` object with version + signature + URL
5. User confirms → `update.downloadAndInstall()` + `relaunch()`

**How `latest.json` is built:**
`tauri-action` in CI generates it automatically when `bundle.createUpdaterArtifacts: true` is set in `tauri.conf.json`. The manifest has one entry per platform/arch with the matching `.sig` content embedded.

**Cryptographic chain:**
- Keypair generated once with `npm run tauri signer generate -- -w ~/.tauri/juno.key`
- Public key embedded in `tauri.conf.json` → `plugins.updater.pubkey`
- Private key + password stored in GitHub Actions secrets (`TAURI_SIGNING_PRIVATE_KEY`, `TAURI_SIGNING_PRIVATE_KEY_PASSWORD`)
- CI signs `.app.tar.gz` during build, embeds signature in `latest.json`
- Client verifies signature against embedded pubkey before installing

**If the private key is lost,** existing installs can never receive updates again. Back up `~/.tauri/juno.key` securely.

## Required GitHub secrets

| Secret | Purpose | Source |
|--------|---------|--------|
| `TAURI_SIGNING_PRIVATE_KEY` | Sign updater artifacts in CI | `cat ~/.tauri/juno.key` |
| `TAURI_SIGNING_PRIVATE_KEY_PASSWORD` | Decrypt the private key | What you typed when generating the key |
| `HOMEBREW_TAP_TOKEN` | Push to lacymorrow/homebrew-tap from CI | GitHub PAT with `repo` scope |
| `GITHUB_TOKEN` | Create releases | Automatic |

## Local prerequisites

For `bun run release` to work, you need:

- `gh` CLI authenticated as someone with push access (`gh auth status`)
- `cargo` and a working Rust toolchain
- `~/repo/juno-www` cloned at that exact path (release script writes to it)
- `~/repo/homebrew-tap` cloned at that exact path (release script updates the formula)
- `npm` logged in for `juno-cua` package publishing

## Detailed steps

### 1. Pre-flight (`release.ts` runs these checks)

- Working tree clean (`git status --porcelain` is empty)
- `gh` and `cargo` binaries present
- Prompts for bump type (patch/minor/major or explicit version)

### 2. Version bump (`scripts/bump-version.sh`)

Updates all of these in lockstep:
- Root `package.json`
- `src-tauri/Cargo.toml`
- `src-tauri/mcp-server-os-level/Cargo.toml`
- `tauri-plugin-voice-transcription/Cargo.toml` + its `package.json` + `api/package.json`
- `crates/juno-cua/Cargo.toml`
- `packages/juno-cua/package.json`
- `backend-server/package.json`

### 3. juno-cua build

Builds `juno-cua` for `aarch64-apple-darwin` and `x86_64-apple-darwin`, then `lipo`s them into a universal binary. Tars all three, computes SHA256s for the homebrew formula.

### 4. Tag + push

Tags both `v0.X.Y` (main app) and `cua-v0.X.Y` (CLI). Pushes triggers two workflows in parallel.

### 5. npm + homebrew (juno-cua only)

`npm publish --access public` from `packages/juno-cua`. Updates `lacymorrow/homebrew-tap/Formula/juno-cua.rb` with new SHAs and version, commits, pushes.

### 6. Wait for CI Tauri build

`waitForDmg()` polls `gh release view v0.X.Y --json assets` every 15s until the `.dmg` shows up. Timeout: **45 min** (universal macOS builds run ~30 min on cold runner cache; budget for slow builds).

### 7. Sync juno-www

Once the DMG asset URL is known, writes `juno-www/public/downloads/release.json` with:
```json
{
  "version": "v0.X.Y",
  "file": "https://github.com/lacymorrow/juno/releases/download/v0.X.Y/Juno_X.Y.Z_universal.dmg",
  "releasedAt": "..."
}
```
Commits and pushes to `juno-www` main.

## Troubleshooting

### `waitForDmg` timed out

The CI build took longer than 45 min, or it failed. Check:
```bash
gh run list --workflow=release-tauri.yml --limit=3
gh run view <run-id> --log-failed
```
If the build did eventually succeed, manually update `juno-www`:
```bash
echo '{
  "version": "v0.X.Y",
  "file": "https://github.com/lacymorrow/juno/releases/download/v0.X.Y/Juno_X.Y.Z_universal.dmg",
  "releasedAt": "TIMESTAMP"
}' > ~/repo/juno-www/public/downloads/release.json
cd ~/repo/juno-www && git add public/downloads/release.json && git commit -m "release: update download to Juno v0.X.Y" && git push
```

### CI fails on "spending limit"

The repo is **public** so this should never happen — GitHub Actions has unlimited minutes for public repos. If you see this, the repo got accidentally flipped private. Check `gh repo view --json visibility`.

### macOS updater downloads update but app fails to launch

Gatekeeper is quarantining the binary because we don't have an Apple Developer ID. Until that's set up, end-user auto-updates won't actually complete. Users have to manually allow the app via System Settings → Privacy & Security each time. **This is the single biggest known limitation.** Fixing it requires:
1. Apple Developer Program membership ($99/yr)
2. Developer ID Application certificate
3. Adding cert + notarization secrets to GitHub Actions
4. ~15 lines of additional workflow YAML

### `latest.json.version` doesn't match the git tag

You tagged without running `release.ts` (which bumps versions first). Don't do that. The version inside `Cargo.toml` is what ends up in `latest.json`, not the tag name.

### "I want to test CI without doing a real release"

Push a `v*-rc*` tag manually:
```bash
git tag v0.X.Y-rc1
git push origin v0.X.Y-rc1
gh run watch
```
After verifying, clean up:
```bash
gh release delete v0.X.Y-rc1 --yes --cleanup-tag
```

## File map

| File | Purpose |
|------|---------|
| `scripts/release.ts` | Orchestrator — the one command you run |
| `scripts/bump-version.sh` | Bumps versions across all manifests |
| `.github/workflows/release-tauri.yml` | CI: builds universal macOS DMG, generates `latest.json` |
| `.github/workflows/release-cua.yml` | CI: builds juno-cua binaries, updates homebrew tap |
| `src-tauri/tauri.conf.json` → `plugins.updater` | Pubkey + endpoint config |
| `src-tauri/tauri.conf.json` → `bundle.createUpdaterArtifacts` | Tells `tauri build` to emit `.app.tar.gz` + `.sig` |
| `src-tauri/src/lib.rs` | Registers `tauri_plugin_updater::Builder::new().build()` |
| `src-tauri/capabilities/default.json` | `updater:default`, `updater:allow-check`, `updater:allow-download-and-install` |
| `src/hooks/useUpdater.ts` | Frontend hook: `checkForUpdates()` + `installUpdate()` |
| `src/App.tsx` | Calls hook from "Check for Updates" menu action |
| `src/components/ModalSystem.tsx` | Renders the update prompt; uses `onInstallUpdate` callback |

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
  └── 6. Local: update homebrew-tap formula (for juno-cua)

juno-www (marketing site) pulls the latest release from the GitHub API
on demand — no sync step. See juno-www/app/api/release/route.ts.
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

## Code signing and notarization (Apple)

This is a different thing from the updater key above, and confusing the two is
how an unsigned demo build got handed to someone. Two signatures matter:

| Signature | What it proves | Where the key lives |
|-----------|----------------|---------------------|
| Tauri updater key | that an update came from us | `~/.tauri/juno.key`, `TAURI_SIGNING_PRIVATE_KEY` in CI |
| Apple Developer ID + notarization ticket | that macOS will let the app open at all | login keychain, `APPLE_*` repo secrets in CI |

macOS puts a quarantine attribute on anything downloaded. Gatekeeper then
refuses an ad-hoc signed app with **"This app is damaged and can't be opened.
You should move it to the Trash."** and refuses a signed but un-notarized one
with **"Apple could not verify this app is free of malware."** Neither message
mentions signing, so the failure looks like a corrupt download.

### In CI

`.github/workflows/release-tauri.yml` exports whichever of `APPLE_CERTIFICATE`,
`APPLE_CERTIFICATE_PASSWORD`, `APPLE_SIGNING_IDENTITY`, `APPLE_ID`,
`APPLE_PASSWORD` and `APPLE_TEAM_ID` exist as repo secrets, and `tauri-action`
signs and notarizes with them. With no secrets set the step exports nothing and
the release build comes out unsigned.

### Locally

`bun run tauri:build` (and `--demo`) does the whole thing through
`scripts/tauri-build.sh`:

1. Finds a `Developer ID Application` identity with `security find-identity -v -p codesigning`
   and exports it as `APPLE_SIGNING_IDENTITY`, so tauri signs the app and the DMG
   during bundling. If several are installed it picks the first in sorted order
   and says which.
2. Submits the finished DMG with `xcrun notarytool submit --wait`, using the
   keychain profile named by `JUNO_NOTARY_PROFILE` (default `juno`), then
   staples the ticket to the DMG and to the `.app` on disk.
3. Runs `codesign -dv`, `spctl -a -vvv -t install` on the app and
   `xcrun stapler validate` on the DMG, and prints a boxed verdict. Anything
   other than "SIGNED, NOTARIZED, STAPLED. SHIPPABLE." means do not hand the
   artifact to anyone.

Run `bun run tauri:build`, not `bun tauri build`. The second one calls the tauri
CLI directly and skips all of the above.

**One-time setup on a new machine:**

```bash
# 1. Developer ID Application certificate in the login keychain
#    (Xcode > Settings > Accounts > Manage Certificates, or double-click the .p12)
security find-identity -v -p codesigning     # should list "Developer ID Application: ..."

# 2. notarytool credentials, stored in the keychain, never in the repo
xcrun notarytool store-credentials juno \
  --apple-id <apple-id> --team-id <TEAMID> --password <app-specific-password>
xcrun notarytool history --keychain-profile juno   # should succeed
```

The app-specific password comes from appleid.apple.com, not your Apple ID
password.

**Escape hatches**, both of which produce something that will be refused once
downloaded, and both of which say so loudly at the end of the build:

| Variable | Effect |
|----------|--------|
| `JUNO_SKIP_NOTARIZE=1` | sign, but skip the notarization round trip. For iterating when you only care that it compiles. |
| `JUNO_UNSIGNED_BUILD=1` | no updater artifacts, no Developer ID signature, no notarization. For a machine with no certificate. |

There is no flag to skip signing on its own. It costs seconds, needs no
network, and a build worth bundling is worth signing. Missing the certificate
or the notary profile is a hard error, raised before the build starts rather
than after twenty minutes of cargo, so the only way to get an unsigned artifact
is to ask for one by name.

## Required GitHub secrets

| Secret | Purpose | Source |
|--------|---------|--------|
| `TAURI_SIGNING_PRIVATE_KEY` | Sign updater artifacts in CI | `cat ~/.tauri/juno.key` |
| `TAURI_SIGNING_PRIVATE_KEY_PASSWORD` | Decrypt the private key | What you typed when generating the key |
| `HOMEBREW_TAP_TOKEN` | Push to lacymorrow/homebrew-tap from CI | GitHub PAT with `repo` scope |
| `GITHUB_TOKEN` | Create releases | Automatic |
| `APPLE_CERTIFICATE`, `APPLE_CERTIFICATE_PASSWORD`, `APPLE_SIGNING_IDENTITY`, `APPLE_ID`, `APPLE_PASSWORD`, `APPLE_TEAM_ID` | Code sign and notarize the release build | See [Code signing and notarization](#code-signing-and-notarization-apple) |

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

### 6. CI Tauri build

The `v*` tag push triggers `.github/workflows/release-tauri.yml`. The release script does **not** wait for it — once the tag is pushed, the script exits. The CI build runs ~30 min and publishes the DMG, `.app.tar.gz`, `.sig`, and `latest.json` to a GitHub Release.

### 7. juno-www updates itself

`juno-www/app/api/release/route.ts` hits the GitHub API (`/repos/lacymorrow/juno/releases/latest`) with Next.js ISR (`revalidate: 300`). Within ~5 minutes of CI publishing the new release, the marketing site's download button picks up the new DMG URL automatically. No commit or sync needed.

## Troubleshooting

### CI build failed or never published

Check workflow runs:
```bash
gh run list --workflow=release-tauri.yml --limit=3
gh run view <run-id> --log-failed
```
Once CI does publish, juno-www will pick up the new release on the next ISR revalidation (max 5 min after first request).

### CI fails on "spending limit"

The repo is **public** so this should never happen — GitHub Actions has unlimited minutes for public repos. If you see this, the repo got accidentally flipped private. Check `gh repo view --json visibility`.

### "This app is damaged and can't be opened" / "Apple could not verify this app"

The artifact is not signed and notarized. It is not a corrupt download, and the
message does not say so. Check the artifact itself:

```bash
codesign -dv --verbose=4 <app>     # want: Authority=Developer ID Application, TeamIdentifier set
xcrun stapler validate <dmg>       # want: The validate action worked!
spctl -a -vvv -t install <app>     # want: accepted, source=Notarized Developer ID
```

`Signature=adhoc` with `TeamIdentifier=not set` means it was built without a
Developer ID. If it came from `bun run tauri:build`, the verdict box at the end
of that build already said so. If it came from `bun tauri build`, that command
bypasses the wrapper and does none of this: rebuild with `bun run tauri:build`.
If it came from CI, the `APPLE_*` repo secrets are missing or empty. See
[Code signing and notarization](#code-signing-and-notarization-apple).

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
| `scripts/tauri-build.sh` | Local `bun run tauri:build`: updater key, build identity, Apple signing, notarization, stapling, Gatekeeper verdict |
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

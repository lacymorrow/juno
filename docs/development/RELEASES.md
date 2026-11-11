# Releases & Auto-Updater

How Juno gets shipped, and how the in-app updater works.

## TL;DR: nothing builds on your Mac

```
merge a PR to main
  └── release-every-merge.yml (ubuntu)
        shipx patch --yes: bump every version, commit "release: vX.Y.Z", tag, push
        └── release-tauri.yml (macos-latest, prerelease=true)
              signed + notarized build → GitHub prerelease with DMG,
              .app.tar.gz, .sig, latest.json

scripts/juno-build.sh vX.Y.Z          install that prerelease and try it
scripts/juno-build.sh promote [vX.Y.Z]  make it a full release (default: newest prerelease)
```

**A prerelease reaches nobody.** The auto-updater and juno-www both read
`/releases/latest`, and GitHub never returns a prerelease there. Promoting
flips the flag and marks it Latest, so the bytes users get are the bytes you
tested. Nothing is rebuilt.

**Trying a branch before it merges:** `scripts/juno-build.sh` (current branch)
or `scripts/juno-build.sh <branch>`. It dispatches `build-branch.yml`, waits,
downloads the signed `.app`, replaces `/Applications/Juno.app` (old copy goes
to the Trash) and opens it. `--no-install` just downloads. A commit already
built is downloaded again instead of rebuilt.

**Pushing a `v*` tag by hand no longer builds.** Since LAC-7145 `build-tauri`
runs in a `release` GitHub Environment whose deployment branch policy allows
`main` and nothing else, so a build on a tag ref is refused before its first
step. The gate fails such a push rather than building it. To build a tag shipx
already cut, dispatch it on main:

```bash
gh workflow run release-tauri.yml --ref main -f tag=v0.9.0 -f prerelease=false
```

Note that only the *ref restriction* is live. Moving the credentials themselves
into that environment is still outstanding and needs Lacy; see
[Which of them belong in the `release` environment](#which-of-them-belong-in-the-release-environment)
and LAC-7194.

**That works only for a tag main already carries, at a `release:` commit.** The
`release` environment restricts the ref the build *runs* on, not the ref it
*checks out*, so `-f tag=` would otherwise have been a way to build any tree at
all from an allowed ref. The gate therefore validates the input before anything
is signed: it must be a real tag, pointing at a commit main already contains
(`identical` or `behind` in `compare/main...<tag>`), whose message starts with
`release:`. It is then resolved to a SHA once and the build checks out that SHA,
so re-pointing the tag afterwards changes nothing. Anything else fails the gate
by name (LAC-7166).

In practice that means you dispatch a tag shipx already cut. To release a tree
that has no such tag, merge it to main and let `release-every-merge.yml` cut
one; `bun run release` (shipx) is unaffected either way, because it writes a
`release:` commit that the gate skips in favour of that dispatch.

**Latest is pinned to the highest version.** Builds finish out of order, and
GitHub marks whichever full release was created last as Latest. After a full
release build, `release-tauri.yml` points Latest at the highest `vX.Y.Z`, and
`release-cua.yml` never marks a CLI release Latest.

---

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
| `HOMEBREW_TAP_TOKEN` | Push to lacymorrow/homebrew-tap from CI | GitHub PAT with `repo` scope. Move to `release` env |
| `RELEASE_SSH_KEY` | Deploy key that pushes the release commit past the main ruleset | `scripts/apply-branch-protection.sh`. Move to `release` env |
| `NPM_TOKEN` | Publish juno-cua | npm automation token. Move to `release` env |
| `GITHUB_TOKEN` | Create releases | Automatic |
| `APPLE_CERTIFICATE`, `APPLE_CERTIFICATE_PASSWORD`, `APPLE_SIGNING_IDENTITY`, `APPLE_ID`, `APPLE_PASSWORD`, `APPLE_TEAM_ID` | Code sign and notarize the release build | See [Code signing and notarization](#code-signing-and-notarization-apple) |

### Which of them belong in the `release` environment

This repo is public and every fleet agent pushes with admin-level credentials,
and **admins bypass rulesets**. There is no admin bypass for an environment's
deployment branch policy, which is why the `release` environment (deployment
branches: `main` only) is the instrument here rather than another rule. A secret
held there cannot be read by a workflow on any other branch; a repo-scoped
secret on a public repo can be read by anyone who can push a workflow file.

Move these into **Settings → Environments → release → Environment
secrets**, then **delete the repository-scoped copy** — step two is the one that
closes anything. Moving a value needs the plaintext, so it is Lacy's to do;
nothing breaks in the meantime, because a job naming an environment still
resolves a repo-scoped secret (measured, run 42114623977).

Of the ten names first listed here, three are done, six still hold a real value
at repo scope, and `TAURI_SIGNING_PRIVATE_KEY_PASSWORD` holds an empty one, so
it has nothing to move. Deleting the repo-scoped copy of any of the six breaks
no workflow job except the guard itself, which reddens rung 3 by design until the name moves to `ENV_ONLY`: every consumer of every name in the table declares
`environment: release`, re-verified against `origin/main` on 2026-11-09
(`build-tauri`, `build-demo`, `release`, `homebrew`). Read the per-row status
below rather than this count, and the guard rather than either.

That last property is also why the first attempt at this was inert with no
failing check anywhere (LAC-7194): the environment was attached and documented
as done, none of the seven actually moved, the only value that did move was
`APPLE_TEAM_ID` which was meant to stay, and every build kept resolving the rest
from repo scope exactly as before. The exposure itself is not new -- these have
been repo-scoped since September -- but believing it closed is.
`.github/workflows/secret-scope-guard.yml` is the executable form of this
table — it runs a job that names no environment, so a secret it can still read
is a secret that is still repo-scoped, and it fails when reality stops matching
what is declared here. After moving a secret, move its name from `PENDING` to
`ENV_ONLY` in that file; the guard reddens until you do.

| Secret | Why it moves |
|--------|--------------|
| `TAURI_SIGNING_PRIVATE_KEY` | **The updater trust root.** The app verifies only the minisign `pubkey` in `tauri.conf.json`, so this key alone can hand every install a malicious update. Read only by `release-tauri.yml`. |
| `TAURI_SIGNING_PRIVATE_KEY_PASSWORD` | Decrypts the above. **Nothing to move**: the stored value is empty, so the updater key carries no passphrase (unchanged since 2026-09-21). Already `ENV_ONLY` in the guard, which still bites the case that matters — a real passphrase stored at repo scope reddens rung 1. Deleting the empty repo-scoped copy is optional and changes nothing to the build: `release-tauri.yml` maps it into `env:` with `${{ secrets.… }}`, and an undefined secret evaluates to the empty string, so the bundler sees the same empty variable either way. It is left in place only because there is no material here to protect. |
| `APPLE_ID`, `APPLE_PASSWORD` | Notarization. Read only by `release-tauri.yml` and `demo-build.yml`, both main-only. |
| `RELEASE_SSH_KEY` | **A direct write to `main`.** The deploy key provisioned to bypass the `main` ruleset (see `docs/rules/branch-protection.md`). Read only by the `release` job of `release-every-merge.yml`, which now declares `environment: release` (LAC-7170). |
| `NPM_TOKEN` | Publishes `juno-cua`. Same job, same reason. |
| `HOMEBREW_TAP_TOKEN` | Single-repo PAT that pushes `lacymorrow/homebrew-tap`. Read only by the `homebrew` job of `release-cua.yml`, which names `environment: release`. That became possible when the CLI release moved from a `cua-v*` tag push (a tag ref the main-only environment refuses, and a tag policy would protect nothing with no tag ruleset) to a dispatch from main that `release-every-merge.yml` starts (LAC-7267). |
| `APPLE_API_ISSUER`, `APPLE_API_KEY`, `APPLE_API_KEY_P8_B64` | Notarization (App Store Connect key). **Done 2026-11-09** (LAC-7193): created directly in the environment. They are `ENV_ONLY` in the guard, and no guard run has reddened, so no readable repo-scoped copy existed at any run since 2026-11-05; a copy added later reddens rung 1. Worth noting what that bought: the guard listed these three by name while they existed at neither scope, so creating them at repo scope would have failed a check on the next push to main. |

Staying at repo scope, deliberately:

- `APPLE_CERTIFICATE`, `APPLE_CERTIFICATE_PASSWORD`, `APPLE_SIGNING_IDENTITY` —
  `build-branch.yml` imports the certificate on arbitrary branches and `exit 1`s
  without it, so scoping these would take "run Juno from a branch" with them. A
  leaked certificate with no notarization credential cannot produce a build that
  opens on a stranger's Mac, and it is revocable.
- `APPLE_TEAM_ID` — not a secret. It is embedded in the public signing identity
  string.
- `GITHUB_TOKEN` — minted per run, already scoped.

**Never add required reviewers to `release`.** `release-every-merge.yml`'s
release job runs on every merge, so a reviewer rule would hold every merge
behind a manual approval. Checked 2026-11-05: the environment's only protection
rule is the branch policy.

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

Tags both `v0.X.Y` (main app) and `cua-v0.X.Y` (CLI). A tag push starts nothing: `release-every-merge.yml` dispatches `release-tauri.yml` and `release-cua.yml` from main, because only a main run can use the `release` environment (LAC-7267).

### 5. npm + homebrew (juno-cua only)

`npm publish --access public` from `packages/juno-cua`. Updates `lacymorrow/homebrew-tap/Formula/juno-cua.rb` with new SHAs and version, commits, pushes.

The formula rewrite is three pattern substitutions against a file in a repo this one does not control, so `scripts/update-homebrew-formula.sh` proves on disk that all three landed (version, both `cua-v*` download urls, both checksums) **before** anything is committed. A pattern that matches nothing — the formula reformatted, renamed, or grown a third platform block — fails the job rather than publishing a release whose tap still serves the previous version (LAC-7401). A red `Update Homebrew formula` step naming one of those means the formula's shape changed and the script needs updating, not that the release is broken; nothing was pushed to the tap. Tested by `scripts/__tests__/update-homebrew-formula.test.sh` in CI. The suite runs twice: on ubuntu in `release-report`, and on `macos-latest` in `homebrew-formula-macos`, because the release's own `homebrew` job runs on macOS and "the pattern matched nothing" is exactly where BSD and GNU tools diverge.

The `homebrew` job reads that script from the **run's ref** (main), not from the built tree, because the rewriter is release *tooling* versioned with the workflow that calls it. A release dispatched from a branch that does not have the script therefore fails with `No such file`, which is the intended failure: pinning the rewriter to an older built tree would silently run that tree's version of it against today's tap.

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

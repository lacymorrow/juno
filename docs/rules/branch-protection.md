# Branch protection on `main`

`main` requires CI to be green. The rule is a file, `.github/rulesets/main.json`,
applied and verified by `scripts/apply-branch-protection.sh`.

```bash
scripts/apply-branch-protection.sh          # create or update, then verify
scripts/apply-branch-protection.sh --check  # verify only, non-zero on drift
```

## What it requires

The three jobs in `ci.yml`, matched by name:

- `Frontend (typecheck + build)`
- `Rust (fmt + clippy + test)`
- `Rust (x86_64 check)`

GitHub matches required checks as strings, so renaming a job in `ci.yml` without
renaming it here blocks every merge forever. The apply script refuses to run
when a required context is no longer a job name in `ci.yml`.

CodeQL is deliberately not required. It is a scanner on a schedule, and a
scheduled scanner in the merge path turns its flakiness into a merge outage.

`strict_required_status_checks_policy` is off: a branch does not have to be
up to date with `main` to merge. Turning it on would force a rebase on every
pull request, which is the wrong tax for this cadence.

## Why it exists

Before 2026-10-17 `main` had no protection rule at all. `gh api
repos/:owner/:repo/branches/main/protection` returned 404 and there were no
rulesets, so no check was required to merge anything. `release-every-merge.yml`
then cut a version on every merge regardless. That is how `cb8542b5` landed
with `Rust (fmt + clippy + test)` already failing on the commit, and how
v0.8.207 and v0.8.208 shipped from a red tree (LAC-4623, LAC-4625).

The protection was missing invisibly. Nothing in the repository stated what
`main` required, so nothing could notice that it required nothing. That is why
the rule is committed here rather than clicked into a settings page.

## Nobody is exempt, and that is the point, not an oversight

The original suggestion was to leave admins unenforced so a red check would not
block the owner. That rested on there being a separate agent identity to gate.
There is not. `lacymorrow` is the only collaborator on the repository and
merged every recent pull request, the red `#825` included; the fleet's agents
act with those credentials. An admin bypass would therefore exempt one hundred
percent of merges and the rule would stop nothing at all.

So `bypass_actors` names no human and no role. A red pull request cannot be
merged by anyone, owner included. The escape hatch is not a bypass: it is
disabling the ruleset, which takes one call and leaves a record.

## The one bypass actor is a deploy key, and why it has to be

Two measured facts shape this.

**Required status checks gate pushes, not only merges.** Pushing a commit that
has no checks is rejected with `GH013 ... Required status check "..." is
expected`, and that happens to a repository admin as readily as to anyone else.
Verified directly against this repository on a throwaway branch and ruleset.

**On a user-owned repository, GitHub Actions cannot be a bypass actor.** Adding
it is rejected: `Actor GitHub Actions integration must be part of the ruleset
source or owner organization`. `lacymorrow/juno` has no owning organization.

`release-every-merge.yml` runs shipx, which commits the version bump and pushes
it to `main`. With protection on and no bypass, that push is rejected and the
release cadence stops. So the push needs a credential that *can* be a bypass
actor. A deploy key is the narrowest one available: a single repository, git
only, no API surface, revoked in one call, and narrower than the personal
access token it replaces. Its private half is the `RELEASE_SSH_KEY` secret,
which `actions/checkout` consumes via `ssh-key:`.

Forked pull requests never receive secrets, so the exposure is workflows
running on `main`, which already hold `contents: write`.

## The consequence that is easy to miss

A `GITHUB_TOKEN` push starts no workflows. A deploy key push does. So shipx's
tag push now fires `release-tauri.yml`'s `push: tags: ["v*"]` trigger, which
`release-every-merge.yml` already dispatches explicitly with `prerelease=true`.
Left alone, every automated release would build twice, and the tag-triggered
half would carry no inputs, so `prerelease` would default to false and publish
straight to the stable channel.

`release-tauri.yml` therefore opens with a `gate` job that skips a tag pointing
at shipx's own `release:` commit. A tag pushed by hand still builds a full
release. The gate resolves the commit message over the API rather than reading
`github.event.head_commit`, which GitHub documents only as "object or null" and
does not promise for tag pushes.

This consequence belongs to any bypass credential, not to deploy keys
specifically. A personal access token would behave the same way.

## The second line of defence

Protection stops a red pull request merging. It cannot stop a push that
bypasses it, which is the hole those two releases fell through, so
`release-every-merge.yml` checks independently. It no longer triggers on the
push to `main`; it triggers on CI's completion, and its `gate` job refuses to
cut a version unless the run concluded `success` on the exact commit that is
still `main`'s head. See the header of that file for why waiting inside a
push-triggered job was the wrong shape.

# Branch protection on `main`

`main` requires CI to be green. The rule is a file, `.github/rulesets/main.json`,
applied and verified by `scripts/apply-branch-protection.sh`.

```bash
scripts/apply-branch-protection.sh --credentials  # deploy key + secret only
scripts/apply-branch-protection.sh                # all of it, then verify
scripts/apply-branch-protection.sh --check        # verify only, non-zero on drift
```

## Order of operations, and the one step that cannot move

1. `--credentials`, **before** the pull request that introduces any of this merges.
2. Merge.
3. The full apply, which creates the ruleset.
4. `--check`.

Step 1 is early for a reason that is easy to get wrong. For a push, GitHub reads
the workflow file **as it exists in the pushed commit**, so the merge commit
already carries the `workflow_run` version of `release-every-merge.yml`. The new
release path is therefore live the instant that merge lands, and its very first
run reads `RELEASE_SSH_KEY`. Creating the credential after the merge is a race
against a release. Creating it before is inert: a deploy key nothing uses and a
secret nothing reads until the workflow that wants it exists.

Step 3 is late for the mirror-image reason. Applying the ruleset before the
merge would block that merge, because the workflow being replaced still pushes
with `GITHUB_TOKEN` and nothing has granted that a bypass.

## What it requires

One job in `ci.yml`, matched by name:

- `Rust (fmt + clippy + test)`

That is the board's decision of 2026-10-25, and it is the check that would have
stopped the incident. On `#825` this job failed and the other two passed, so
requiring it alone is enough to have blocked `cb8542b5`.

The other two jobs in `ci.yml` stay optional on purpose. `Frontend (typecheck +
build)` runs the vitest suite, and the board ruled frontend tests a candidate
rather than a defect: nothing has merged red through them. `Rust (x86_64 check)`
is a `cargo check` that proves the Intel target still compiles; it has not been
the thing that broke. Each is one line in `.github/rulesets/main.json` plus one
apply, whenever one of them does become a defect.

Keeping the required set small is not only deference. Every required check gates
pushes as well as merges (see below), so each one widens what the release
credential has to bypass, and the `Rust` job already takes some twenty minutes.

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

## Repository admins bypass, by decision, with the counter-argument on record

`bypass_actors` names the repository admin role, which is the ruleset spelling
of the old `enforce_admins: false`. Decided by the board on 2026-10-25.

The reasoning given: the defect is agent pull requests merging red, and
requiring the check fixes that. Turning it on for admins also gates the one
human who cuts releases, on a one-person release cadence. Wrong trade today,
and a one-line flip whenever it stops being.

The counter-argument this overrules, kept because it is the thing to re-read
when the trade changes: `lacymorrow` is the only collaborator on the
repository and merged every recent pull request, the red `#825` included, and
the fleet's agents act with those credentials. An admin bypass is therefore
capable of exempting every merge, including the agent merges it is meant to
stop. What makes it an acceptable trade rather than a hole is that a bypass is
*available*, not automatic: GitHub's merge button still refuses a red pull
request and has to be deliberately overridden, which is a different act from
not being asked at all.

To reverse it, drop the `RepositoryRole` entry from
`.github/rulesets/main.json` and re-apply. One call, either direction.

## Two bypass actors: the admin role above, and a deploy key

Two measured facts shape this.

**Required status checks gate pushes, not only merges.** Writing a commit that
has no checks is rejected: over git with `GH013 ... Required status check "..."
is expected`, over the API with `409 Repository rule violations found\n\nRequired
status check "Rust (fmt + clippy + test)" is expected`. Verified directly
against this repository on a throwaway branch and ruleset, twice: once on
2026-10-17 and again on 2026-10-27 with the admin bypass added and then removed
as a control. With the bypass present the admin write succeeded; with it
removed, the identical write was refused. That is what makes the admin entry
above a real grant rather than a no-op.

**The admin bypass does not cover the release bot, which is why a second actor
is still needed.** `release-every-merge.yml` pushes as `github-actions[bot]`
holding `GITHUB_TOKEN`, which carries write permission, not the admin role, so
the `RepositoryRole` entry does nothing for it. Reading "admins bypass" as
"no bypass credential needed" is the easy mistake here.

**On a user-owned repository, GitHub Actions cannot be a bypass actor.** Adding
it is rejected: `Actor GitHub Actions integration must be part of the ruleset
source or owner organization`. `lacymorrow/juno` has no owning organization.

`release-every-merge.yml` runs shipx, which commits the version bump and pushes
it to `main`. With protection on and no bypass, that push is rejected and the
release cadence stops. So the push needs a credential that *can* be a bypass
actor and is not the admin role. A deploy key is the narrowest one available: a single repository, git
only, no API surface, revoked in one call, and narrower than the personal
access token it replaces. Its private half is the `RELEASE_SSH_KEY` secret,
which `actions/checkout` consumes via `ssh-key:`.

Forked pull requests never receive secrets. Everything else does: a push to a
branch in this repository, and a pull request opened from one, both run with
the full secret set. So the exposure is every non-fork run, not only the ones
on `main`.

The ruleset cannot narrow that further. GitHub accepts an `actor_id` on a
`DeployKey` bypass actor and silently stores null, so "this one key may bypass"
is not expressible and the grant is really *any* deploy key on the repository.
The deploy key is still the narrowest credential available for the push; it is
the bypass *grant* that is wider than it looks. `--check` closes the gap from
the other side by asserting the repository has exactly one deploy key and that
it is this one.

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

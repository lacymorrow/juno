#!/usr/bin/env bash
#
# main's branch protection, as a file somebody can read in review.
#
# Why a script and not a setting someone clicked: the protection that was
# missing was missing *invisibly* (LAC-4625). Nothing in the repository said
# what main required, so nothing could notice that it required nothing, and
# v0.8.207 and v0.8.208 shipped from a red tree. The rule now lives in
# .github/rulesets/main.json, this applies it, and `--check` fails when the
# live one has drifted away from the file.
#
#   scripts/apply-branch-protection.sh --credentials   deploy key + secret only
#   scripts/apply-branch-protection.sh                 all of it, then verify
#   scripts/apply-branch-protection.sh --check         verify only, non-zero on drift
#
# `--check` verifies two things, because the ruleset only describes one of
# them: that the live ruleset matches the file, and that the repository has
# exactly one deploy key and it is ours. See check_bypass_keys for why the
# second one cannot be expressed in the file.
#
# **Run --credentials BEFORE the pull request that introduces this merges.**
# For a push, GitHub reads the workflow file as it exists in the pushed commit,
# so the merge commit already carries the `workflow_run` version of
# release-every-merge.yml. The new release path therefore goes live the moment
# that merge lands, and the very first run reads RELEASE_SSH_KEY. Creating the
# credential afterwards is a race against a release. Creating it early is inert:
# a deploy key nothing uses and a secret nothing reads until the workflow exists.
#
# Needs gh (authenticated, admin on the repo), jq and ssh-keygen.
#
# The full reasoning, including why the bypass actor is a deploy key, is in
# docs/rules/branch-protection.md.
set -euo pipefail

here="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
spec="$here/.github/rulesets/main.json"
repo="${REPO:-$(gh repo view --json nameWithOwner -q .nameWithOwner)}"
name="$(jq -r .name "$spec")"
key_title="release-every-merge (ruleset bypass)"
secret="RELEASE_SSH_KEY"

live_id() {
  gh api "repos/$repo/rulesets" --jq ".[] | select(.name == \"$name\") | .id"
}

# Only the fields this repository owns. GitHub decorates a ruleset with an id,
# timestamps, a node_id and _links, none of which belong in a diff.
#
# It also returns the arrays in an order of its own choosing, which is not the
# order they were sent in: POST a bypass_actors of [RepositoryRole, DeployKey]
# and every later GET answers [DeployKey, RepositoryRole] (measured against
# this repository, 2026-10-27). `jq -S` sorts object keys recursively and
# leaves array order alone, so comparing the arrays as returned would report
# drift forever on a ruleset that matches the file exactly. Canonicalise both
# sides instead of trusting either order.
mine() {
  jq -S '
    {name, target, enforcement, conditions, bypass_actors, rules}
    | .bypass_actors |= sort_by(.actor_type, (.actor_id // 0), .bypass_mode)
    | .rules |= (
        sort_by(.type)
        | map(
            if .type == "required_status_checks"
            then .parameters.required_status_checks |= sort_by(.context)
            else . end
          )
      )
  '
}

# The ruleset file is not a complete statement of who can bypass main, and it
# cannot be made into one. GitHub accepts an `actor_id` on a `DeployKey` bypass
# actor and silently stores null: HTTP 201, no 422, while a `RepositoryRole` id
# in the same request body is preserved (measured against this repository,
# 2026-10-27). So the grant reads "any deploy key on this repository", and
# .github/rulesets/main.json says `"actor_id": null` because that is the only
# thing it can say.
#
# A grant that cannot be narrowed can still be made observable. Assert the set
# it ranges over has exactly one member and that the member is ours. That is
# the control the ruleset cannot express, and it is the only check here that
# would notice a second key minted by the SIGPIPE path ensure_credential used
# to have.
check_bypass_keys() {
  local keys count title
  keys="$(gh api "repos/$repo/keys")"
  count="$(jq 'length' <<<"$keys")"
  if [ "$count" -ne 1 ]; then
    echo "Expected exactly 1 deploy key on $repo, found $count." >&2
    echo "Every deploy key on the repository can bypass '$name' and the ruleset cannot name one, so the count is the control." >&2
    jq -r '.[] | "  id=\(.id)\ttitle=\(.title)\tread_only=\(.read_only)"' <<<"$keys" >&2
    return 1
  fi
  title="$(jq -r '.[0].title' <<<"$keys")"
  if [ "$title" != "$key_title" ]; then
    echo "The one deploy key on $repo is titled '$title', not '$key_title'." >&2
    echo "It can bypass '$name'. Identify it before trusting the ruleset." >&2
    return 1
  fi
  echo "One deploy key on $repo, '$title', and it is the release credential."
}

# Reports every problem it finds in one pass. Making the operator re-run to
# discover the second one is how a drifted ruleset hides an extra bypass key.
check() {
  local id rc=0
  id="$(live_id)"
  if [ -z "$id" ]; then
    echo "No ruleset named '$name' on $repo. main is unprotected." >&2
    rc=1
  elif diff -u <(mine <"$spec") <(gh api "repos/$repo/rulesets/$id" | mine); then
    echo "Ruleset '$name' ($id) matches $spec."
  else
    echo "The live ruleset has drifted from $spec (left: the file, right: GitHub)." >&2
    rc=1
  fi
  check_bypass_keys || rc=1
  return "$rc"
}

# A required check that never reports blocks every merge forever, and renaming
# a job in ci.yml is the easy way to cause it. GitHub matches these by string,
# so assert each one is still a job name before handing it over.
assert_contexts() {
  while read -r context; do
    grep -Fq "name: $context" "$here/.github/workflows/ci.yml" ||
      {
        echo "Required check '$context' is not a job name in ci.yml. Fix one or the other first." >&2
        return 1
      }
  done < <(jq -r '.rules[] | select(.type == "required_status_checks") | .parameters.required_status_checks[].context' "$spec")
}

# Where the generated private key lives while it is on disk, and the trap that
# removes it. Both are at script scope on purpose.
#
# This used to be `local tmp` with `trap 'rm -rf "$tmp"' RETURN` inside
# ensure_credential, which never ran on the path that matters. Under `set -e` a
# failing command inside a function terminates the shell rather than returning
# from it, so the RETURN trap is skipped and `$tmp/key` survives, mode 700, with
# the key intact until the machine reboots. Ordering is what made that serious:
# the public half is registered as a write-capable deploy key, and therefore a
# `main` bypass actor, *before* `gh secret set` stores the private half, so a
# failure at that one step leaks a live bypass credential.
#
# EXIT is the only trap that fires on a `set -e` termination. The RETURN trap is
# kept as well, because on the success path `apply` goes on to make two more
# network calls and there is no reason for the key to still be on disk for them.
tmp=""
cleanup() {
  [ -n "${tmp:-}" ] && rm -rf "$tmp"
  tmp=""
  return 0
}
trap cleanup EXIT

# The bypass actor. Required checks gate pushes as well as merges, and shipx
# pushes the `release:` bump straight to main, so without a credential that can
# bypass, the first merge after the ruleset lands would stop the release
# cadence dead. Idempotent, and safe to run long before the ruleset exists.
#
# Membership is tested by capturing the list first and matching against the
# variable. Never pipe into `grep -q` here: `grep -q` exits on its first match,
# the process upstream takes SIGPIPE, `pipefail` reports 141, and the `if` reads
# false on a list that does contain the title. The false branch then mints a
# *second* deploy key and overwrites $secret with its private half, leaving key
# #1 registered, write-capable, still a bypass actor, and with its own private
# half stored nowhere. `--check` could not see it either, which is why it now
# counts deploy keys.
ensure_credential() {
  local titles secrets
  titles="$(gh api "repos/$repo/keys" --jq '.[].title')"
  if grep -Fqx "$key_title" <<<"$titles"; then
    echo "Deploy key '$key_title' is already present."
    secrets="$(gh secret list --repo "$repo" --json name --jq '.[].name')"
    grep -Fqx "$secret" <<<"$secrets" ||
      {
        echo "The deploy key exists but the $secret secret does not, so the release workflow cannot use it." >&2
        echo "Delete the key titled '$key_title' and re-run to mint a fresh pair." >&2
        return 1
      }
    echo "The $secret secret is present."
    return 0
  fi

  tmp="$(mktemp -d)"
  trap cleanup RETURN
  ssh-keygen -t ed25519 -N '' -C "$key_title" -f "$tmp/key" -q
  gh api -X POST "repos/$repo/keys" \
    -f title="$key_title" -f key="$(cat "$tmp/key.pub")" -F read_only=false \
    --jq '"Added deploy key \(.id)."'
  gh secret set "$secret" --repo "$repo" <"$tmp/key"
  echo "Stored its private half as the $secret secret."
}

apply_ruleset() {
  local id
  id="$(live_id)"
  if [ -n "$id" ]; then
    gh api -X PUT "repos/$repo/rulesets/$id" --input "$spec" --jq '"Updated ruleset \(.id)."'
  else
    gh api -X POST "repos/$repo/rulesets" --input "$spec" --jq '"Created ruleset \(.id)."'
  fi
}

case "${1:-apply}" in
--check)
  check
  ;;
--credentials)
  ensure_credential
  ;;
apply)
  assert_contexts
  ensure_credential
  apply_ruleset
  check
  ;;
*)
  echo "usage: $(basename "$0") [--credentials | --check]" >&2
  exit 2
  ;;
esac

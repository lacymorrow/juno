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

check() {
  local id
  id="$(live_id)"
  if [ -z "$id" ]; then
    echo "No ruleset named '$name' on $repo. main is unprotected." >&2
    return 1
  fi
  if diff -u <(mine <"$spec") <(gh api "repos/$repo/rulesets/$id" | mine); then
    echo "Ruleset '$name' ($id) matches $spec."
    return 0
  fi
  echo "The live ruleset has drifted from $spec (left: the file, right: GitHub)." >&2
  return 1
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

# The bypass actor. Required checks gate pushes as well as merges, and shipx
# pushes the `release:` bump straight to main, so without a credential that can
# bypass, the first merge after the ruleset lands would stop the release
# cadence dead. Idempotent, and safe to run long before the ruleset exists.
ensure_credential() {
  if gh api "repos/$repo/keys" --jq '.[].title' | grep -Fqx "$key_title"; then
    echo "Deploy key '$key_title' is already present."
    gh secret list --repo "$repo" | awk '{print $1}' | grep -Fqx "$secret" ||
      {
        echo "The deploy key exists but the $secret secret does not, so the release workflow cannot use it." >&2
        echo "Delete the key titled '$key_title' and re-run to mint a fresh pair." >&2
        return 1
      }
    echo "The $secret secret is present."
    return 0
  fi

  local tmp
  tmp="$(mktemp -d)"
  trap 'rm -rf "$tmp"' RETURN
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

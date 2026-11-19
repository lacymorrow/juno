#!/usr/bin/env bash
#
# Decides whether a PR's changed files can reach the Rust suite (LAC-8428,
# LAC-8445). Paths arrive on stdin, one per line; renames list BOTH names.
# Prints `true` (run Rust) or `false` (skip the macOS jobs) and the reason on
# stderr. ci.yml's "Detect Rust-relevant changes" job calls this, and
# scripts/__tests__/ci-rust-relevant.test.sh pins every arm, so a reorder of
# the case below cannot fail open unnoticed.
#
# Fail-safe: this lists what is known NOT to reach cargo. Anything else runs Rust.
#   docs/ marketing/ media/   nothing in cargo reads them
#   *.md at the repo root     same
#   src/ except src/components/settings/   the frontend; registry.rs
#       include_str!s files under settings/ and asserts on them.
# scripts/check-rust-skip-reads.sh fails CI if a .rs file starts reading a
# skipped directory, which is what keeps this list true.

set -euo pipefail

cap=3000
files=$(cat)

yes() { echo "$1" >&2; echo true; exit 0; }

# The files endpoint stops at 3000. A list that long is not a docs PR.
[ "$(printf '%s\n' "$files" | grep -c .)" -lt "$cap" ] || yes "File list reached the API cap; running Rust."

while IFS= read -r f; do
  [ -n "$f" ] || continue
  case "$f" in
    src/components/settings/*) yes "$f is read by a Rust test." ;;
    docs/*|marketing/*|media/*|src/*) ;;
    */*) yes "$f is outside the known-safe list." ;;
    *.md) ;;
    *) yes "$f is outside the known-safe list." ;;
  esac
done <<< "$files"

echo "No changed file can reach the Rust suite." >&2
echo false

#!/usr/bin/env bash
#
# Fails if Rust code reads a path ci-rust-relevant.sh treats as safe to skip
# (LAC-8445). Without this, a new include_str!("../../docs/x") makes a docs-only
# PR skip the one suite that reads it. Allowed: src/components/settings/, which
# the classifier already sends to Rust. Heuristic: a `../` followed by a skipped
# directory in a literal, or `.join("..")` followed by `.join("<skipped>")`.
# Run from the repo root; ROOT overrides for the test.

set -euo pipefail

root="${ROOT:-.}"
status=0
while IFS= read -r f; do
  hits=$(perl -0777 -ne '
    s/\s+//g;
    while (/((?:\.\.\/)+(?:docs|marketing|media|src)\/[^"]*|\.\.\/[A-Za-z0-9_-]+\.md|"\.\."\)\.join\("(?:docs|marketing|media|src)")/g) { print "$1\n" }
  ' "$f" | grep -v '\.\./src/components/settings/' || true)
  if [ -n "$hits" ]; then
    echo "::error file=$f::reads a path ci.yml skips Rust for: $(echo "$hits" | head -1)" >&2
    status=1
  fi
done < <(find "$root/src-tauri" -name '*.rs' -not -path '*/target/*')
exit "$status"

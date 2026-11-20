#!/usr/bin/env bash
# shellcheck disable=SC2015
# Behaviour tests for scripts/ci-rust-relevant.sh and check-rust-skip-reads.sh.
set -uo pipefail
here="$(cd "$(dirname "$0")" && pwd)"
pass=0; fail=0
ok() { pass=$((pass+1)); }
bad() { fail=$((fail+1)); echo "FAIL: $1" >&2; }

expect() { # want name paths...
  local want="$1" name="$2"; shift 2
  local got; got=$(printf '%s\n' "$@" | "$here/../ci-rust-relevant.sh" 2>/dev/null)
  [ "$got" = "$want" ] && ok || bad "$name: want $want got $got"
}

expect false "docs only"        docs/a.md docs/deep/b.png
expect false "marketing+media"  marketing/x.md media/y.mp4
expect false "root md"          README.md CHANGELOG.md
expect false "frontend src"     src/App.tsx src/lib/x.ts
expect false "all safe mixed"   docs/a.md README.md src/a.ts
expect true  "settings src"     src/components/settings/sections/GeneralSettings.tsx
expect true  "rust"             src-tauri/src/lib.rs
expect true  "workflow"         .github/workflows/ci.yml
expect true  "root non-md"      Cargo.toml
expect true  "scripts dir"      scripts/ci-rust-relevant.sh
expect true  "safe + one rust"  docs/a.md src-tauri/src/lib.rs
expect true  "rename out of skip dir (both names listed)" docs/old.rs src-tauri/src/new.rs
expect true  "settings rename into src" src/components/settings/A.tsx src/A.tsx
expect true  "empty-ish safe list but rust last" docs/a.md src/a.ts Cargo.lock

big=$(seq 1 3000 | sed 's|^|docs/f|')
[ "$(printf '%s\n' "$big" | "$here/../ci-rust-relevant.sh" 2>/dev/null)" = true ] && ok || bad "3000-file cap"
small=$(seq 1 2999 | sed 's|^|docs/f|')
[ "$(printf '%s\n' "$small" | "$here/../ci-rust-relevant.sh" 2>/dev/null)" = false ] && ok || bad "2999 files still skip"

# Guard
t=$(mktemp -d); trap 'rm -rf "$t"' EXIT
mkdir -p "$t/src-tauri/src"
echo 'fn a() { include_str!("../../../src/components/settings/A.tsx"); }' > "$t/src-tauri/src/ok.rs"
ROOT="$t" "$here/../check-rust-skip-reads.sh" 2>/dev/null && ok || bad "guard allows settings/"
for body in 'include_str!("../../docs/x.md")' 'include_bytes!("../../media/a.png")' 'p.join("..").join("marketing")' 'include_str!("../../../src/App.tsx")' 'include_str!("../../CHANGELOG.md")'; do
  echo "fn b() { $body; }" > "$t/src-tauri/src/bad.rs"
  ROOT="$t" "$here/../check-rust-skip-reads.sh" 2>/dev/null && bad "guard misses: $body" || ok
done
rm "$t/src-tauri/src/bad.rs"
ROOT="$here/../.." "$here/../check-rust-skip-reads.sh" 2>/dev/null && ok || bad "guard passes on the real tree"

echo "passed=$pass failed=$fail"
[ "$fail" -eq 0 ]

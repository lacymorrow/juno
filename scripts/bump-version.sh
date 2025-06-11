#!/usr/bin/env bash
# bump-version.sh - bump Rust crate versions (Cargo.toml) and Node package versions (package.json)
# Usage: ./scripts/bump-version.sh <new-version>

set -euo pipefail

if [[ $# -ne 1 ]]; then
  echo "Usage: $(basename "$0") <new-version>" >&2
  exit 1
fi

NEW_VERSION="$1"

echo "\n🔄  Bumping repository version to $NEW_VERSION...\n"

# ----------------------
# 1. Rust crates
# ----------------------

if ! command -v cargo-workspaces >/dev/null 2>&1; then
  echo "cargo-workspaces not found – installing (this is a one-time step)…"
  cargo install cargo-workspaces --quiet
fi

# --yes answers the interactive prompt automatically
cargo workspaces version "$NEW_VERSION" --force --yes

echo "✓ Rust crate versions updated."

# ----------------------
# 2. Node packages (if any)
# ----------------------
if command -v bunx >/dev/null 2>&1; then
  if bunx --yes changeset --help >/dev/null 2>&1; then
    # Use changesets if the repo has it configured
    echo "Updating package.json versions via changesets…"
    bunx --yes changeset version --snapshot "$NEW_VERSION"
  else
    # Fallback: patch all package.json files with jq
    if command -v jq >/dev/null 2>&1; then
      echo "Updating package.json files with jq…"
      for pkg in $(git ls-files -- '*.json' | grep -E 'package\.json$'); do
        jq --arg v "$NEW_VERSION" '(.version) |= $v' "$pkg" > "$pkg.tmp" && mv "$pkg.tmp" "$pkg"
      done
    else
      echo "Skipping Node version bump – jq not available."
    fi
  fi
fi

echo "\n✅  Version bump complete. Commit the changes with:\n   git commit -am \"chore: bump version to $NEW_VERSION\"\n"
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

# Ensure we have cargo-set-version
if ! command -v cargo-set-version >/dev/null 2>&1; then
  echo "Installing cargo-edit (provides cargo set-version)…"
  cargo install cargo-edit --version 0.12.3 --locked --quiet
fi

# Move into the Rust workspace (src-tauri) if it exists
if [ -d "src-tauri" ] && [ -f "src-tauri/Cargo.toml" ]; then
  pushd src-tauri >/dev/null
  cargo set-version --workspace "$NEW_VERSION"
  popd >/dev/null
else
  echo "⚠️  No src-tauri workspace found; skipping Rust crate bump."
fi

echo "✓ Rust crate versions updated."

# ----------------------
# 2. Node packages (if any)
# ----------------------
if command -v bunx >/dev/null 2>&1; then
  if bunx --yes @changesets/cli --help >/dev/null 2>&1; then
    echo "Updating package.json versions via changesets…"
    bunx --yes @changesets/cli version --snapshot "$NEW_VERSION"
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
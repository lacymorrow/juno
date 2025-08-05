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
# Always update the root package.json first
if [ -f "package.json" ]; then
  if command -v jq >/dev/null 2>&1; then
    echo "Updating root package.json..."
    jq --arg v "$NEW_VERSION" '.version = $v' "package.json" > "package.json.tmp" && mv "package.json.tmp" "package.json"
  elif command -v bunx >/dev/null 2>&1; then
    echo "Updating root package.json with npm..."
    bunx npm version "$NEW_VERSION" --no-git-tag-version
  else
    echo "⚠️  Cannot update package.json - no jq or npm available"
  fi
fi

# Update other package.json files
if command -v jq >/dev/null 2>&1; then
  echo "Updating other package.json files..."
  for pkg in $(find . -name "package.json" -type f | grep -v node_modules | grep -v target | grep -v "^\./package.json$"); do
    if [ -f "$pkg" ]; then
      echo "  Updating $pkg"
      jq --arg v "$NEW_VERSION" '(.version) |= $v' "$pkg" > "$pkg.tmp" && mv "$pkg.tmp" "$pkg"
    fi
  done
else
  echo "Skipping other package.json updates – jq not available."
fi

echo "\n✅  Version bump complete. Commit the changes with:\n   git commit -am \"chore: bump version to $NEW_VERSION\"\n"
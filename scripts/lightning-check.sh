#!/bin/bash

# Lightning Check - Ultra-Fast Development Check
# Skips problematic dependencies for maximum speed
# Usage: ./scripts/lightning-check.sh

set -e

echo "⚡⚡ Lightning fast check (main crate only)..."

# Change to project root
cd "$(dirname "$0")/.."

# Check only the main Tauri crate (excluding MCP server binaries)
CARGO_INCREMENTAL=1 cargo check \
    --manifest-path=src-tauri/Cargo.toml \
    --lib \
    --message-format=short \
    --quiet \
    2>&1

echo "✅ Lightning check completed!"
echo "⏱️  For full validation, run: ./scripts/quick-check.sh"

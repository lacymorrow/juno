#!/bin/bash

# Ultra-Fast Syntax Check - Skips heavy dependencies
# Usage: ./scripts/syntax-check.sh

set -e

echo "⚡ Running ultra-fast syntax check..."

# Change to project root
cd "$(dirname "$0")/.."

# Check main crate only (fastest)
echo "Checking main crate..."
CARGO_INCREMENTAL=1 cargo check \
    --manifest-path=src-tauri/Cargo.toml \
    --lib \
    --bins \
    --message-format=short \
    --quiet \
    2>&1

echo "✅ Syntax check completed in record time!"

# Optional: Show what we skipped for speed
echo "💡 Tip: Run './scripts/quick-check.sh' for full workspace check"
echo "💡 Tip: Run 'cargo test --no-run' to check test compilation"

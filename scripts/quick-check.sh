#!/bin/bash

# Quick Check Script - Optimized for Speed
# Usage: ./scripts/quick-check.sh

set -e

echo "🚀 Running optimized cargo check..."

# Change to project root
cd "$(dirname "$0")/.."

# Use workspace-level cargo check for maximum efficiency
CARGO_INCREMENTAL=1 cargo check \
    --workspace \
    --message-format=short \
    --quiet \
    2>&1

echo "✅ Quick check completed successfully!"

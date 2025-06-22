#!/bin/bash

# Cargo Watch - Continuous Development Check
# Automatically runs lightning-fast checks on file changes
# Usage: ./scripts/cargo-watch.sh [mode]
# Modes: lightning (default), syntax, quick

set -e

WATCH_MODE="${1:-lightning}"

echo "🔄 Starting Cargo Watch in $WATCH_MODE mode..."
echo "   Press Ctrl+C to stop"

# Change to project root
cd "$(dirname "$0")/.."

# Check if cargo-watch is installed
if ! command -v cargo-watch &>/dev/null; then
    echo "📦 Installing cargo-watch..."
    cargo install cargo-watch
fi

# Different watch commands based on mode
case "$WATCH_MODE" in
"lightning")
    echo "⚡ Watching for changes - Lightning mode (library only)"
    cargo watch -c -w src-tauri/src -x 'check --manifest-path=src-tauri/Cargo.toml --lib --message-format=short --quiet'
    ;;
"syntax")
    echo "⚡ Watching for changes - Syntax mode (main crate)"
    cargo watch -c -w src-tauri/src -x 'check --manifest-path=src-tauri/Cargo.toml --lib --bins --message-format=short --quiet'
    ;;
"quick")
    echo "⚡ Watching for changes - Quick mode (full workspace)"
    cargo watch -c -w src-tauri/src -w tauri-plugin-voice-transcription/src -x 'check --workspace --message-format=short --quiet'
    ;;
*)
    echo "❌ Unknown mode: $WATCH_MODE"
    echo "   Available modes: lightning, syntax, quick"
    exit 1
    ;;
esac

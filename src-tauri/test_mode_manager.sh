#!/bin/bash

# Simple script to test just the mode manager module
cd /Users/lacymorrow/repo/juno/src-tauri

echo "Running mode manager tests..."
cargo test mode_manager::tests --lib --no-fail-fast -- --show-output

echo "Test run complete!"
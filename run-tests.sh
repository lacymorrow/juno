#!/bin/bash

# Simple test script for the Anthropic Computer Use Tools

# Create results directory
mkdir -p logs

echo "=============================================="
echo "Running Basic Tests for Anthropic Computer Use"
echo "=============================================="

# Run cargo check
echo "Running cargo check..."
cargo check --manifest-path src-tauri/Cargo.toml >logs/cargo-check.log 2>&1
if [ $? -eq 0 ]; then
    echo "✅ Cargo check passed"
else
    echo "❌ Cargo check failed - see logs/cargo-check.log"
    exit 1
fi

# Run unit tests
echo "Running cargo tests..."
(cd src-tauri && cargo test) >logs/cargo-test.log 2>&1
if [ $? -eq 0 ]; then
    echo "✅ Cargo tests passed"
else
    echo "❌ Cargo tests failed - see logs/cargo-test.log"
    exit 1
fi

# Run focused element test
echo "Running focused element test..."
(cd src-tauri && cargo run -- --test-focused-element-ns) >logs/focused-element.log 2>&1
if [ $? -eq 0 ]; then
    echo "✅ Focused element test passed"
else
    echo "❌ Focused element test failed - see logs/focused-element.log"
fi

# Run accessibility check
echo "Running accessibility check..."
(cd src-tauri && cargo run -- --check-accessibility) >logs/accessibility.log 2>&1
if [ $? -eq 0 ]; then
    echo "✅ Accessibility check passed"
else
    echo "❌ Accessibility check failed - see logs/accessibility.log"
fi

# Run an SDK example test
echo "Running 'get all apps' example test..."
(cd src-tauri/mcp-server-os-level && cargo run --example test_get_all_apps) >logs/get-all-apps.log 2>&1
if [ $? -eq 0 ]; then
    echo "✅ Get all apps example test passed"
else
    echo "❌ Get all apps example test failed - see logs/get-all-apps.log"
fi

# Check if app is running and run QA tests if it is
echo ""
echo "Checking if Tauri app is running for QA tests..."
if nc -z localhost 1420 2>/dev/null; then
    echo "✅ Tauri app detected on port 1420, running QA tests..."
    ./test-qa.sh
else
    echo "ℹ️ Tauri app not running. To run QA tests:"
    echo "   1. Start the app in another terminal with 'pnpm tauri dev'"
    echo "   2. Run './test-qa.sh' manually"
fi

echo ""
echo "=============================================="
echo "All tests completed"
echo "=============================================="

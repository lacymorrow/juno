#!/bin/bash

# Comprehensive Test Script for Anthropic Computer Use Tools
# This script runs:
# 1. Cargo tests
# 2. CLI tests
# 3. SDK example tests
# 4. QA functional tests

# Exit on error
set -e

# Set working directory to project root
cd "$(dirname "$0")"

# Set environment variables
export RUST_LOG=info
export TEST_RESULTS_DIR="./test-results"

# Create results directory if it doesn't exist
mkdir -p "$TEST_RESULTS_DIR"

echo "=============================================="
echo "Anthropic Computer Use Tools - Test Runner"
echo "=============================================="
echo "Started: $(date)"
echo ""

# Function to run a test and log results
run_test() {
    local test_name=$1
    local command=$2
    local log_file="$TEST_RESULTS_DIR/${test_name// /_}.log"

    echo "----------------------------------------"
    echo "Running: $test_name"
    echo "Command: $command"
    echo "Log file: $log_file"
    echo ""

    # Ensure directory exists
    mkdir -p "$(dirname "$log_file")"

    # Run the command and capture output
    set +e # Temporarily disable exit on error
    eval "$command" >"$log_file" 2>&1
    local status=$?
    set -e # Re-enable exit on error

    if [ $status -eq 0 ]; then
        echo "✅ PASSED: $test_name"
    else
        echo "❌ FAILED: $test_name (status code: $status)"
        echo "Check log file for details: $log_file"

        # Show the last few lines of the log file
        echo "Last few lines of the log:"
        tail -n 10 "$log_file"
    fi

    return $status
}

# 1. Run Cargo tests
echo "SECTION: Cargo Tests"
echo "--------------------------------------------"

run_test "Cargo check" "cargo check --manifest-path src-tauri/Cargo.toml"
run_test "Cargo tests" "cd src-tauri && cargo test -- --nocapture"

# 2. Run CLI tests
echo ""
echo "SECTION: CLI Tests"
echo "--------------------------------------------"

# Allow CLI tests to fail without stopping the script
set +e
run_test "Test focused element" "cd src-tauri && cargo run -- --test-focused-element-ns"
run_test "Check accessibility" "cd src-tauri && cargo run -- --check-accessibility"
set -e

# 3. Run SDK example tests
echo ""
echo "SECTION: SDK Example Tests"
echo "--------------------------------------------"

# Allow example tests to fail without stopping the script
set +e
run_test "Test get all apps" "cd src-tauri/mcp-server-os-level && cargo run --example test_get_all_apps"

# Conditionally run some examples that might require active windows
if [ -z "$CI" ]; then
    # These tests require user interaction and should be skipped in CI environments
    echo "Running interactive examples (these might open windows or move cursor)..."
    run_test "Test click by role" "cd src-tauri/mcp-server-os-level && cargo run --example test_click_by_role || true"
fi
set -e

# 4. Run QA tests (requires app to be running)
echo ""
echo "SECTION: QA tests"
echo "--------------------------------------------"

echo "QA tests require the application to be running."
echo "If you want to run QA tests, please:"
echo "1. Start the app with 'pnpm tauri dev' in a separate terminal"
echo "2. Run ./test-qa.sh manually after the app is running"

echo ""
echo "=============================================="
echo "Test Summary"
echo "=============================================="
echo "Completed: $(date)"
echo "See $TEST_RESULTS_DIR directory for detailed logs"

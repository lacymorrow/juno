#!/bin/bash

# Script to run Rust unit tests with verbose output
# This helps to see which tests are available and running

# Color codes for output formatting
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[0;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# Log directory
mkdir -p logs

# Function to run tests with filtering
run_tests() {
	local package=$1
	local filter=$2
	local test_type=$3

	echo -e "${BLUE}Running $test_type tests for $package${NC}"

	if [ -n "$filter" ]; then
		echo -e "${YELLOW}Filter: $filter${NC}"
		(cd $package && RUST_BACKTRACE=1 cargo test $filter -- --nocapture --show-output) | tee logs/${package//\//-}-${test_type//\//-}.log
	else
		(cd $package && RUST_BACKTRACE=1 cargo test -- --nocapture --show-output) | tee logs/${package//\//-}-${test_type//\//-}.log
	fi

	if [ ${PIPESTATUS[0]} -eq 0 ]; then
		echo -e "${GREEN}✅ $test_type tests passed${NC}"
		return 0
	else
		echo -e "${RED}❌ $test_type tests failed${NC}"
		return 1
	fi
}

# Run all tests for src-tauri
echo "======================================================"
echo "Running all Rust unit tests with verbose output"
echo "======================================================"

# Run all tests in the main package
run_tests "src-tauri" "" "all"

# Run all tests in the SDK
run_tests "src-tauri/mcp-server-os-level" "" "all"

# Optional: Run specific test modules if needed
# run_tests "src-tauri" "cli::" "CLI"
# run_tests "src-tauri/mcp-server-os-level" "platforms::macos::" "macOS"

echo -e "\n${BLUE}Test logs are available in the logs directory${NC}"

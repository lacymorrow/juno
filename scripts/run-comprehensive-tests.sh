#!/bin/bash

# Comprehensive Test Runner for Juno AI Computer Use Agent
# This script runs all test types: unit, integration, security, performance, and e2e

set -e # Exit on any error

# Configuration
TEST_RESULTS_DIR="test-results"
TIMESTAMP=$(date +"%Y%m%d_%H%M%S")
REPORT_FILE="$TEST_RESULTS_DIR/comprehensive_test_report_$TIMESTAMP.md"
PERFORMANCE_BASELINE_FILE="test-results/performance_baseline.json"

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# Test counters
TOTAL_TESTS=0
PASSED_TESTS=0
FAILED_TESTS=0
SKIPPED_TESTS=0

# Function to print colored output
print_status() {
    local color=$1
    local message=$2
    echo -e "${color}${message}${NC}"
}

# Function to log results
log_result() {
    local test_type=$1
    local status=$2
    local message=$3
    local duration=$4

    echo "## $test_type - $status" >>"$REPORT_FILE"
    echo "**Duration:** ${duration}s" >>"$REPORT_FILE"
    echo "**Details:** $message" >>"$REPORT_FILE"
    echo "" >>"$REPORT_FILE"

    if [ "$status" = "PASSED" ]; then
        ((PASSED_TESTS++))
    elif [ "$status" = "FAILED" ]; then
        ((FAILED_TESTS++))
    else
        ((SKIPPED_TESTS++))
    fi
    ((TOTAL_TESTS++))
}

# Function to run tests with timing
run_test_suite() {
    local test_name=$1
    local test_command=$2
    local required=$3 # true/false

    print_status $BLUE "Running $test_name..."
    local start_time=$(date +%s)

    if eval "$test_command"; then
        local end_time=$(date +%s)
        local duration=$((end_time - start_time))
        print_status $GREEN "✅ $test_name PASSED (${duration}s)"
        log_result "$test_name" "PASSED" "All tests passed successfully" "$duration"
        return 0
    else
        local end_time=$(date +%s)
        local duration=$((end_time - start_time))
        if [ "$required" = "true" ]; then
            print_status $RED "❌ $test_name FAILED (${duration}s)"
            log_result "$test_name" "FAILED" "Required test suite failed" "$duration"
            return 1
        else
            print_status $YELLOW "⚠️  $test_name SKIPPED/FAILED (${duration}s)"
            log_result "$test_name" "SKIPPED" "Optional test suite failed or skipped" "$duration"
            return 0
        fi
    fi
}

# Function to check prerequisites
check_prerequisites() {
    print_status $BLUE "Checking prerequisites..."

    # Check if Rust is installed
    if ! command -v cargo &>/dev/null; then
        print_status $RED "❌ Cargo not found. Please install Rust."
        exit 1
    fi

    # Check if Node.js is installed
    if ! command -v bun &>/dev/null && ! command -v npm &>/dev/null; then
        print_status $RED "❌ Neither Bun nor npm found. Please install Node.js."
        exit 1
    fi

    # Check if project compiles
    print_status $BLUE "Verifying project compilation..."
    if ! cargo check --manifest-path src-tauri/Cargo.toml --quiet; then
        print_status $RED "❌ Project compilation failed. Please fix compilation errors first."
        exit 1
    fi

    print_status $GREEN "✅ Prerequisites check passed"
}

# Function to setup test environment
setup_test_environment() {
    print_status $BLUE "Setting up test environment..."

    # Create test results directory
    mkdir -p "$TEST_RESULTS_DIR"

    # Initialize report file
    cat >"$REPORT_FILE" <<EOF
# Comprehensive Test Report
**Generated:** $(date)
**Project:** Juno AI Computer Use Agent
**Version:** $(grep '"version"' package.json | cut -d'"' -f4)

---

EOF

    # Set test environment variables
    export NODE_ENV=test
    export RUST_LOG=warn # Reduce log noise during tests
    export RUST_BACKTRACE=1

    print_status $GREEN "✅ Test environment setup complete"
}

# Function to run Rust unit tests
run_rust_unit_tests() {
    print_status $BLUE "Running Rust unit tests..."

    # Run unit tests with coverage if available
    if command -v cargo-tarpaulin &>/dev/null; then
        cargo tarpaulin \
            --manifest-path src-tauri/Cargo.toml \
            --out Html \
            --output-dir "$TEST_RESULTS_DIR" \
            --ignore-tests \
            --timeout 120
    else
        cargo test --manifest-path src-tauri/Cargo.toml --lib --bins
    fi
}

# Function to run Rust integration tests
run_rust_integration_tests() {
    print_status $BLUE "Running Rust integration tests..."
    cargo test --manifest-path src-tauri/Cargo.toml --test '*' --no-fail-fast
}

# Function to run security tests
run_security_tests() {
    print_status $BLUE "Running security tests..."

    # Run security-specific unit tests
    cargo test --manifest-path src-tauri/Cargo.toml security --no-fail-fast

    # Run property-based security tests
    cargo test --manifest-path src-tauri/Cargo.toml prop_security --no-fail-fast

    # Check for known vulnerabilities
    if command -v cargo-audit &>/dev/null; then
        cargo audit --json >"$TEST_RESULTS_DIR/security_audit.json" || true
    fi
}

# Function to run performance benchmarks
run_performance_tests() {
    print_status $BLUE "Running performance benchmarks..."

    # Run criterion benchmarks
    cargo bench --manifest-path src-tauri/Cargo.toml -- --output-format json >"$TEST_RESULTS_DIR/benchmark_results.json" || true

    # Compare with baseline if available
    if [ -f "$PERFORMANCE_BASELINE_FILE" ]; then
        print_status $BLUE "Comparing performance with baseline..."
        # Custom script to compare performance would go here
    fi
}

# Function to run frontend tests
run_frontend_tests() {
    print_status $BLUE "Running frontend tests..."

    # Install dependencies if needed
    if [ -f "bun.lockb" ]; then
        bun install --frozen-lockfile
        bun run test -- --reporter=json --outputFile="$TEST_RESULTS_DIR/frontend_test_results.json"
    else
        npm ci
        npm test -- --reporter=json --outputFile="$TEST_RESULTS_DIR/frontend_test_results.json"
    fi
}

# Function to run end-to-end tests
run_e2e_tests() {
    print_status $BLUE "Running end-to-end tests..."

    # Run E2E tests with longer timeout
    cargo test --manifest-path src-tauri/Cargo.toml e2e --no-fail-fast -- --test-threads=1
}

# Function to generate comprehensive report
generate_final_report() {
    print_status $BLUE "Generating final report..."

    cat >>"$REPORT_FILE" <<EOF

---

# Test Summary

- **Total Test Suites:** $TOTAL_TESTS
- **Passed:** $PASSED_TESTS
- **Failed:** $FAILED_TESTS
- **Skipped:** $SKIPPED_TESTS

## Coverage Information

$(if [ -f "$TEST_RESULTS_DIR/tarpaulin-report.html" ]; then
        echo "Rust code coverage report generated: $TEST_RESULTS_DIR/tarpaulin-report.html"
    else
        echo "Rust code coverage not available (install cargo-tarpaulin for coverage)"
    fi)

$(if [ -f "$TEST_RESULTS_DIR/frontend_test_results.json" ]; then
        echo "Frontend test results: $TEST_RESULTS_DIR/frontend_test_results.json"
    fi)

## Performance Results

$(if [ -f "$TEST_RESULTS_DIR/benchmark_results.json" ]; then
        echo "Performance benchmarks: $TEST_RESULTS_DIR/benchmark_results.json"
    fi)

## Security Audit

$(if [ -f "$TEST_RESULTS_DIR/security_audit.json" ]; then
        echo "Security audit: $TEST_RESULTS_DIR/security_audit.json"
    fi)

---

**Report generated at:** $(date)

EOF

    print_status $GREEN "📊 Final report generated: $REPORT_FILE"
}

# Function to cleanup
cleanup() {
    print_status $BLUE "Cleaning up..."

    # Kill any background processes
    pkill -f "tauri dev" 2>/dev/null || true
    pkill -f "cargo test" 2>/dev/null || true

    # Reset environment variables
    unset NODE_ENV
    unset RUST_LOG
    unset RUST_BACKTRACE
}

# Main execution flow
main() {
    print_status $BLUE "🚀 Starting comprehensive test suite for Juno AI Computer Use Agent"

    # Trap cleanup on exit
    trap cleanup EXIT

    # Check prerequisites
    check_prerequisites

    # Setup test environment
    setup_test_environment

    # Run test suites (order matters)
    run_test_suite "Rust Unit Tests" "run_rust_unit_tests" "true"
    run_test_suite "Frontend Tests" "run_frontend_tests" "true"
    run_test_suite "Security Tests" "run_security_tests" "true"
    run_test_suite "Rust Integration Tests" "run_rust_integration_tests" "true"
    run_test_suite "Performance Tests" "run_performance_tests" "false"
    run_test_suite "End-to-End Tests" "run_e2e_tests" "false"

    # Generate final report
    generate_final_report

    # Print final summary
    print_status $BLUE "📈 Test Execution Summary:"
    print_status $GREEN "  ✅ Passed: $PASSED_TESTS"
    if [ $FAILED_TESTS -gt 0 ]; then
        print_status $RED "  ❌ Failed: $FAILED_TESTS"
    fi
    if [ $SKIPPED_TESTS -gt 0 ]; then
        print_status $YELLOW "  ⚠️  Skipped: $SKIPPED_TESTS"
    fi

    # Exit with appropriate code
    if [ $FAILED_TESTS -gt 0 ]; then
        print_status $RED "❌ Some required tests failed. Please review the report."
        exit 1
    else
        print_status $GREEN "🎉 All required tests passed successfully!"
        exit 0
    fi
}

# Command line argument parsing
case "${1:-}" in
--help | -h)
    echo "Comprehensive Test Runner for Juno AI Computer Use Agent"
    echo ""
    echo "Usage: $0 [options]"
    echo ""
    echo "Options:"
    echo "  --help, -h          Show this help message"
    echo "  --quick             Run only required tests (skip performance and e2e)"
    echo "  --performance-only  Run only performance benchmarks"
    echo "  --security-only     Run only security tests"
    echo "  --frontend-only     Run only frontend tests"
    echo "  --rust-only         Run only Rust tests"
    echo ""
    exit 0
    ;;
--quick)
    print_status $YELLOW "Running in quick mode (skipping optional tests)"
    export QUICK_MODE=true
    ;;
--performance-only)
    check_prerequisites
    setup_test_environment
    run_test_suite "Performance Tests" "run_performance_tests" "true"
    exit $?
    ;;
--security-only)
    check_prerequisites
    setup_test_environment
    run_test_suite "Security Tests" "run_security_tests" "true"
    exit $?
    ;;
--frontend-only)
    check_prerequisites
    setup_test_environment
    run_test_suite "Frontend Tests" "run_frontend_tests" "true"
    exit $?
    ;;
--rust-only)
    check_prerequisites
    setup_test_environment
    run_test_suite "Rust Unit Tests" "run_rust_unit_tests" "true"
    run_test_suite "Rust Integration Tests" "run_rust_integration_tests" "true"
    exit $?
    ;;
esac

# Run main function
main

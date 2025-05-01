#!/bin/bash

# Test script for the Anthropic Computer Use Tools QA functions
# This script tests various mouse interactions using the built-in QA test commands

# Set environment variables for tests
export RUST_LOG=info

echo "Starting QA Tests for Anthropic Computer Use Tools"
echo "=================================================="

# Function to make Tauri command calls using JSON-RPC over STDIN
invoke_command() {
    local command=$1
    local args=$2
    local json="{\"cmd\":\"$command\"$args}"
    echo "Invoking: $command with $args"
    echo $json | nc localhost 1420
    sleep 1 # Give time for the command to complete
}

# Test 1: Click Visualization Test
echo "Running Click Visualization Test..."
invoke_command "qa_test_click_visualization" ",\"args\":[]"

# Test 2: Single Left Click Test
echo "Running Single Left Click Test..."
invoke_command "qa_test_click" ",\"args\":{\"x\":400,\"y\":400,\"clickType\":\"left\"}"

# Test 3: Right Click Test
echo "Running Right Click Test..."
invoke_command "qa_test_click" ",\"args\":{\"x\":500,\"y\":400,\"clickType\":\"right\"}"

# Test 4: Coordinate Transformation Test
echo "Running Coordinate Transformation Test..."
invoke_command "qa_test_coordinate_transformation" ",\"args\":{\"x\":300,\"y\":300}"

# Test 5: Click Series Test (multiple clicks in different positions)
echo "Running Click Series Test..."
invoke_command "qa_test_click_series" ",\"args\":{\"positions\":[[200,200,\"left\"],[300,300,\"right\"],[400,400,\"double\"]]}"

# Test 6: Text Selection Test
echo "Running Text Selection Test..."
invoke_command "qa_test_select_text" ",\"args\":[]"

# Test 7: Scroll Test
echo "Running Scroll Test..."
invoke_command "qa_test_scroll" ",\"args\":{\"direction\":\"down\",\"amount\":5}"

echo "QA Tests Completed"

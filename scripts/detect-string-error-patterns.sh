#!/bin/bash

# Detect String-Based Error Detection Anti-Patterns
# This script identifies fragile string-based error detection patterns that need refactoring

set -euo pipefail

echo "🔍 Scanning for String-Based Error Detection Anti-Patterns..."
echo "============================================================"

# Colors for output
RED='\033[0;31m'
YELLOW='\033[1;33m'
GREEN='\033[0;32m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

ISSUES_FOUND=0

# Function to report issue
report_issue() {
    local file="$1"
    local line="$2"
    local pattern="$3"
    local context="$4"

    echo -e "${RED}❌ ANTI-PATTERN DETECTED${NC}"
    echo -e "   File: ${BLUE}$file${NC}"
    echo -e "   Line: $line"
    echo -e "   Pattern: ${YELLOW}$pattern${NC}"
    echo -e "   Context: $context"
    echo ""
    ((ISSUES_FOUND++))
}

# Check for string.contains() error detection patterns
echo -e "${BLUE}Checking for .contains() error detection...${NC}"
while IFS=: read -r file line match; do
    if [[ -n "$file" && -n "$line" && -n "$match" ]]; then
        # Skip test files and this script
        if [[ "$file" != *"test"* && "$file" != *"script"* ]]; then
            context=$(sed -n "${line}p" "$file" | xargs)
            report_issue "$file" "$line" ".contains() string matching" "$context"
        fi
    fi
done < <(grep -rn "\.contains(" src-tauri/src/ --include="*.rs" | grep -E "(error|Error)" | grep -v "test")

# Check for to_lowercase().contains() patterns
echo -e "${BLUE}Checking for .to_lowercase().contains() patterns...${NC}"
while IFS=: read -r file line match; do
    if [[ -n "$file" && -n "$line" && -n "$match" ]]; then
        if [[ "$file" != *"test"* && "$file" != *"script"* ]]; then
            context=$(sed -n "${line}p" "$file" | xargs)
            report_issue "$file" "$line" ".to_lowercase().contains() pattern" "$context"
        fi
    fi
done < <(grep -rn "\.to_lowercase()\.contains(" src-tauri/src/ --include="*.rs")

# Check for specific problematic functions
echo -e "${BLUE}Checking for specific problematic functions...${NC}"

# Check for is_network_error function
if grep -rn "fn is_network_error" src-tauri/src/ --include="*.rs" >/dev/null; then
    while IFS=: read -r file line match; do
        if [[ -n "$file" && -n "$line" ]]; then
            report_issue "$file" "$line" "is_network_error() function" "String-based network error detection"
        fi
    done < <(grep -rn "fn is_network_error" src-tauri/src/ --include="*.rs")
fi

# Check for determine_error_pattern function
if grep -rn "fn determine_error_pattern" src-tauri/src/ --include="*.rs" >/dev/null; then
    while IFS=: read -r file line match; do
        if [[ -n "$file" && -n "$line" ]]; then
            report_issue "$file" "$line" "determine_error_pattern() function" "String-based error pattern detection"
        fi
    done < <(grep -rn "fn determine_error_pattern" src-tauri/src/ --include="*.rs")
fi

# Check for classify_error functions using string matching
if grep -rn "fn classify_error" src-tauri/src/ --include="*.rs" >/dev/null; then
    while IFS=: read -r file line match; do
        if [[ -n "$file" && -n "$line" ]]; then
            # Check if this function uses string matching
            if grep -A 20 -B 2 "fn classify_error" "$file" | grep -E "(contains\(|to_lowercase)" >/dev/null; then
                report_issue "$file" "$line" "classify_error() with string matching" "Error classification using string patterns"
            fi
        fi
    done < <(grep -rn "fn classify_error" src-tauri/src/ --include="*.rs")
fi

# Check for error message string matching in match statements
echo -e "${BLUE}Checking for error matching in match/if statements...${NC}"
while IFS=: read -r file line match; do
    if [[ -n "$file" && -n "$line" && -n "$match" ]]; then
        if [[ "$file" != *"test"* && "$file" != *"script"* ]]; then
            # Check if it's part of error handling
            if echo "$match" | grep -iE "(error|fail|timeout|network|connection)" >/dev/null; then
                context=$(sed -n "${line}p" "$file" | xargs)
                report_issue "$file" "$line" "String matching in conditionals" "$context"
            fi
        fi
    fi
done < <(grep -rn 'if.*\.contains(' src-tauri/src/ --include="*.rs" | head -10)

# Summary
echo "============================================================"
if [ $ISSUES_FOUND -eq 0 ]; then
    echo -e "${GREEN}✅ No string-based error detection anti-patterns found!${NC}"
    exit 0
else
    echo -e "${RED}❌ Found $ISSUES_FOUND string-based error detection anti-patterns${NC}"
    echo ""
    echo -e "${YELLOW}📋 Next Steps:${NC}"
    echo "1. Review docs/rules/ERROR_HANDLING_BEST_PRACTICES.md"
    echo "2. Implement structured error types for each affected file"
    echo "3. Replace string matching with trait-based classification"
    echo "4. Add proper error type hierarchies"
    echo "5. Test error handling paths"
    echo ""
    echo -e "${BLUE}💡 See the migration strategy in ERROR_HANDLING_BEST_PRACTICES.md${NC}"
    exit 1
fi

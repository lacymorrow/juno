#!/bin/bash

# Detect Magic Numbers in Rust Codebase
# This script identifies potential magic numbers that should be centralized

echo "🔍 Scanning for Magic Numbers in Rust codebase..."
echo "================================================"

# Colors for output
RED='\033[0;31m'
YELLOW='\033[1;33m'
GREEN='\033[0;32m'
NC='\033[0m' # No Color

# Track found issues
ISSUES_FOUND=0

# Function to print findings
print_finding() {
    local file=$1
    local line=$2
    local content=$3
    local severity=$4
    
    if [ "$severity" = "HIGH" ]; then
        echo -e "${RED}HIGH${NC}: $file:$line -> $content"
    elif [ "$severity" = "MEDIUM" ]; then
        echo -e "${YELLOW}MEDIUM${NC}: $file:$line -> $content"
    else
        echo -e "${GREEN}LOW${NC}: $file:$line -> $content"
    fi
    
    ((ISSUES_FOUND++))
}

# Search for potential magic numbers in Rust files
echo "Scanning src-tauri/src/ directory..."
echo

# 1. Look for hardcoded Duration values
echo "1. Hardcoded Duration values:"
while IFS= read -r line; do
    file=$(echo "$line" | cut -d: -f1)
    line_num=$(echo "$line" | cut -d: -f2)
    content=$(echo "$line" | cut -d: -f3-)
    
    # Skip if it's in constants.rs (already centralized)
    if [[ "$file" == *"constants.rs"* ]]; then
        continue
    fi
    
    # Check if the number is already using a constant
    if [[ "$content" == *"timeouts::"* ]] || [[ "$content" == *"monitor_sessions::"* ]]; then
        continue
    fi
    
    print_finding "$file" "$line_num" "$content" "MEDIUM"
done < <(grep -rn "Duration::from_\w*(\d\+)" src-tauri/src/ --include="*.rs" 2>/dev/null)

echo

# 2. Look for local const declarations with numeric values
echo "2. Local const declarations with numeric values:"
while IFS= read -r line; do
    file=$(echo "$line" | cut -d: -f1)
    line_num=$(echo "$line" | cut -d: -f2)
    content=$(echo "$line" | cut -d: -f3-)
    
    # Skip if it's in constants.rs (this is where they should be)
    if [[ "$file" == *"constants.rs"* ]]; then
        continue
    fi
    
    # Skip commented out constants
    if [[ "$content" == *"//"* ]]; then
        continue
    fi
    
    print_finding "$file" "$line_num" "$content" "HIGH"
done < <(grep -rn "const [A-Z_]*: [ui]\d* = \d\+" src-tauri/src/ --include="*.rs" 2>/dev/null)

echo

# 3. Look for timeout/sleep calls with magic numbers
echo "3. Sleep/timeout calls with magic numbers:"
while IFS= read -r line; do
    file=$(echo "$line" | cut -d: -f1)
    line_num=$(echo "$line" | cut -d: -f2)
    content=$(echo "$line" | cut -d: -f3-)
    
    # Skip if it's in constants.rs or already using constants
    if [[ "$file" == *"constants.rs"* ]] || [[ "$content" == *"::"* ]]; then
        continue
    fi
    
    print_finding "$file" "$line_num" "$content" "MEDIUM"
done < <(grep -rn "sleep\|timeout.*(\d\+)" src-tauri/src/ --include="*.rs" 2>/dev/null)

echo

# 4. Look for potential port numbers
echo "4. Potential hardcoded port numbers:"
while IFS= read -r line; do
    file=$(echo "$line" | cut -d: -f1)
    line_num=$(echo "$line" | cut -d: -f2)
    content=$(echo "$line" | cut -d: -f3-)
    
    # Skip if it's in constants.rs or already using constants
    if [[ "$file" == *"constants.rs"* ]] || [[ "$content" == *"ports::"* ]]; then
        continue
    fi
    
    print_finding "$file" "$line_num" "$content" "LOW"
done < <(grep -rn ":[0-9]\{4,5\}" src-tauri/src/ --include="*.rs" 2>/dev/null | grep -v "http" | head -10)

echo

# 5. Look for magic numbers in array indexing or size definitions
echo "5. Potential magic numbers in array operations:"
while IFS= read -r line; do
    file=$(echo "$line" | cut -d: -f1)
    line_num=$(echo "$line" | cut -d: -f2)
    content=$(echo "$line" | cut -d: -f3-)
    
    # Skip if it's in constants.rs, test files, or using constants
    if [[ "$file" == *"constants.rs"* ]] || [[ "$file" == *"test"* ]] || [[ "$content" == *"::"* ]]; then
        continue
    fi
    
    # Only show larger numbers that might be meaningful
    if [[ "$content" =~ [0-9]{2,} ]]; then
        print_finding "$file" "$line_num" "$content" "LOW"
    fi
done < <(grep -rn "\[\d\+\]\|Vec::with_capacity(\d\+)\|\.len() [><=] \d\+" src-tauri/src/ --include="*.rs" 2>/dev/null | head -5)

echo
echo "================================================"
echo "Scan complete!"

if [ $ISSUES_FOUND -eq 0 ]; then
    echo -e "${GREEN}✅ No magic numbers detected!${NC}"
else
    echo -e "${YELLOW}⚠️  Found $ISSUES_FOUND potential magic numbers${NC}"
    echo
    echo "Consider centralizing these values in src-tauri/src/constants.rs"
    echo "Follow the existing module structure:"
    echo "  - timeouts:: for duration-related constants"
    echo "  - agent_config:: for AI/agent configuration"
    echo "  - monitor_sessions:: for monitoring timeouts"
    echo "  - ports:: for network port numbers"
    echo "  - platform_macos:: for macOS-specific values"
fi

echo
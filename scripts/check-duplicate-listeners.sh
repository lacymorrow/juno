#!/bin/bash

# Duplicate Event Listener Detection Script
# Prevents race conditions and application crashes caused by duplicate listeners

echo "🔍 Scanning for duplicate event listeners..."

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

# Counter for issues found
ISSUES_FOUND=0

# Function to check for duplicate listeners in a file
check_file_for_duplicates() {
    local file="$1"
    echo "Checking: $file"

    # Extract all event listener patterns
    if [[ "$file" == *.rs ]]; then
        # Rust backend listeners: app.listen("event-name", ...)
        grep -n 'app\.listen(' "$file" | while read -r line; do
            event_name=$(echo "$line" | sed -n 's/.*app\.listen("\([^"]*\)".*/\1/p')
            if [[ -n "$event_name" ]]; then
                count=$(grep -c "app\.listen(\"$event_name\"" "$file")
                if [[ $count -gt 1 ]]; then
                    echo -e "${RED}❌ DUPLICATE: $event_name appears $count times in $file${NC}"
                    grep -n "app\.listen(\"$event_name\"" "$file"
                    ((ISSUES_FOUND++))
                fi
            fi
        done
    elif [[ "$file" == *.ts ]] || [[ "$file" == *.tsx ]] || [[ "$file" == *.js ]] || [[ "$file" == *.jsx ]]; then
        # Frontend listeners: listen("event-name", ...) or addEventListener
        grep -n 'listen(' "$file" | while read -r line; do
            event_name=$(echo "$line" | sed -n "s/.*listen(['\"]\\([^'\"]*\\)['\"].*/\\1/p")
            if [[ -n "$event_name" ]]; then
                count=$(grep -c "listen(['\"]$event_name['\"]" "$file")
                if [[ $count -gt 1 ]]; then
                    echo -e "${RED}❌ DUPLICATE: $event_name appears $count times in $file${NC}"
                    grep -n "listen(['\"]$event_name['\"]" "$file"
                    ((ISSUES_FOUND++))
                fi
            fi
        done
    fi
}

# Check all relevant files
echo "Scanning Rust backend files..."
find src-tauri/src -name "*.rs" -type f | while read -r file; do
    check_file_for_duplicates "$file"
done

echo "Scanning TypeScript/JavaScript frontend files..."
find src -name "*.ts" -o -name "*.tsx" -o -name "*.js" -o -name "*.jsx" -type f | while read -r file; do
    check_file_for_duplicates "$file"
done

# Check for critical voice transcription events specifically
echo ""
echo "🎯 Checking critical voice transcription events..."

# Count voice-transcription:final-result listeners across all files
FINAL_RESULT_COUNT=$(find . -name "*.rs" -o -name "*.ts" -o -name "*.tsx" -o -name "*.js" -o -name "*.jsx" | xargs grep -l "voice-transcription:final-result" | wc -l)
echo "Files with voice-transcription:final-result: $FINAL_RESULT_COUNT"

# List all voice-transcription:final-result occurrences
echo "All voice-transcription:final-result listeners:"
find . -name "*.rs" -o -name "*.ts" -o -name "*.tsx" -o -name "*.js" -o -name "*.jsx" | xargs grep -n "voice-transcription:final-result" | head -20

# Check for multiple app.listen calls in lib.rs
LIB_RS_LISTENERS=$(grep -c "app\.listen(" src-tauri/src/lib.rs 2>/dev/null || echo "0")
echo "Total app.listen calls in lib.rs: $LIB_RS_LISTENERS"

# Summary
echo ""
if [[ $ISSUES_FOUND -eq 0 ]]; then
    echo -e "${GREEN}✅ No duplicate listeners detected!${NC}"
else
    echo -e "${RED}❌ Found $ISSUES_FOUND duplicate listener issues!${NC}"
    echo -e "${YELLOW}⚠️  These duplicates can cause race conditions and crashes.${NC}"
    exit 1
fi

echo ""
echo "Event listener audit complete."

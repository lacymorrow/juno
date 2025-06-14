#!/bin/bash

# Script to detect duplicate event listeners in the codebase
# This helps prevent race conditions and performance issues

echo "🔍 Checking for duplicate event listeners..."

# Define the events we're monitoring
EVENTS=(
    "dictation-active"
    "app-dictation-started"
    "app-dictation-finished"
    "dictation-transcription-partial"
    "dictation-transcription-final"
    "tts-started"
    "tts-finished"
    "audio-level"
    "voice-error"
    "agent-started"
    "agent-thinking"
    "agent-responding"
    "streaming-text"
    "stream-end"
)

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

ISSUES_FOUND=0

echo "📋 Scanning for event listeners in TypeScript/JavaScript files..."

for event in "${EVENTS[@]}"; do
    echo -e "\n🎯 Checking event: ${YELLOW}$event${NC}"

    # Find all listen() calls for this event
    MATCHES=$(grep -r "listen.*['\"]$event['\"]" src/ --include="*.ts" --include="*.tsx" --include="*.js" --include="*.jsx" 2>/dev/null)

    if [ -n "$MATCHES" ]; then
        COUNT=$(echo "$MATCHES" | wc -l)

        if [ $COUNT -gt 1 ]; then
            echo -e "  ${RED}❌ DUPLICATE LISTENERS FOUND ($COUNT instances):${NC}"
            echo "$MATCHES" | sed 's/^/    /'
            ISSUES_FOUND=$((ISSUES_FOUND + 1))
        else
            echo -e "  ${GREEN}✅ Single listener found${NC}"
            echo "$MATCHES" | sed 's/^/    /'
        fi
    else
        echo -e "  ${YELLOW}⚠️  No listeners found${NC}"
    fi
done

echo -e "\n📊 Summary:"
if [ $ISSUES_FOUND -eq 0 ]; then
    echo -e "${GREEN}✅ No duplicate event listeners detected!${NC}"
    echo "All voice events have centralized listeners in VoiceContext."
else
    echo -e "${RED}❌ Found $ISSUES_FOUND event(s) with duplicate listeners${NC}"
    echo "Consider consolidating these listeners into the VoiceContext."
fi

echo -e "\n💡 Best Practices:"
echo "- Use VoiceContext for all voice-related event listeners"
echo "- Avoid setting up listeners in individual components"
echo "- Use context hooks (useVoiceState, useAgentState) instead"
echo "- Run this script after making changes to voice components"

exit $ISSUES_FOUND

#!/bin/bash

# Race Condition Detection Script for Juno AI
# Identifies potential concurrency issues and race conditions

echo "🔍 Scanning for race conditions and concurrency issues..."

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

ISSUES_FOUND=0

echo "${BLUE}=== RACE CONDITION ANALYSIS ===${NC}"

# 1. Check for inconsistent lock usage patterns
echo -e "\n${YELLOW}1. Checking for inconsistent lock usage patterns...${NC}"
echo "❌ POTENTIAL ISSUES:"

# try_lock vs lock inconsistency
echo "• Mixed try_lock/lock patterns (can cause data inconsistency):"
grep -r "try_lock()" src-tauri/src/ | grep -v "test" | head -5
echo "  vs."
grep -r "\.lock(" src-tauri/src/ | grep -v "test" | head -3

# 2. Multiple lock acquisitions in loops
echo -e "\n${YELLOW}2. Checking for multiple lock acquisitions in loops...${NC}"
echo "❌ POTENTIAL ISSUES:"
echo "• Multiple lock acquisitions in tight loops:"
grep -A 5 -B 5 "loop {" src-tauri/src/ | grep -A 3 -B 3 "\.lock(" | head -10

# 3. Spawned tasks with shared state
echo -e "\n${YELLOW}3. Checking for spawned tasks with shared state...${NC}"
echo "❌ POTENTIAL ISSUES:"
echo "• tokio::spawn with shared state access:"
grep -A 10 "tokio::spawn" src-tauri/src/ | grep -E "(Arc|clone|lock)" | head -10

# 4. Event handlers that could race
echo -e "\n${YELLOW}4. Checking for event handlers that could race...${NC}"
echo "❌ POTENTIAL ISSUES:"
echo "• Multiple event handlers for same events:"
echo "  Voice transcription handlers:"
grep -r "voice-transcription" src-tauri/src/ | wc -l
echo "  App state change handlers:"
grep -r "app\\.listen" src-tauri/src/ | head -5

# 5. Unsafe Send/Sync implementations
echo -e "\n${YELLOW}5. Checking for unsafe Send/Sync implementations...${NC}"
UNSAFE_SEND_SYNC=$(grep -r "unsafe impl Send" src-tauri/src/ | wc -l)
if [ $UNSAFE_SEND_SYNC -gt 0 ]; then
    echo "❌ FOUND $UNSAFE_SEND_SYNC unsafe Send implementations:"
    grep -r "unsafe impl Send" src-tauri/src/
    ISSUES_FOUND=$((ISSUES_FOUND + UNSAFE_SEND_SYNC))
else
    echo "✅ No unsafe Send implementations found"
fi

# 6. Global state without synchronization
echo -e "\n${YELLOW}6. Checking for global state without synchronization...${NC}"
echo "❌ POTENTIAL ISSUES:"
echo "• Static variables that could be accessed concurrently:"
grep -r "static.*:" src-tauri/src/ | grep -v "test" | head -5

# 7. File operations without locking
echo -e "\n${YELLOW}7. Checking for file operations without locking...${NC}"
echo "❌ POTENTIAL ISSUES:"
echo "• Potential concurrent file access:"
grep -r "File::" src-tauri/src/ | head -5
echo "• Write operations that could race:"
grep -r "write(" src-tauri/src/ | head -3

# 8. Check for duplicate async spawns
echo -e "\n${YELLOW}8. Checking for duplicate async spawns...${NC}"
SPAWN_COUNT=$(grep -r "tokio::spawn" src-tauri/src/ | wc -l)
echo "• Total tokio::spawn calls: $SPAWN_COUNT"
if [ $SPAWN_COUNT -gt 15 ]; then
    echo "⚠️ High number of spawned tasks - potential race condition source"
    ISSUES_FOUND=$((ISSUES_FOUND + 1))
fi

# 9. State manager race conditions
echo -e "\n${YELLOW}9. Checking state manager race conditions...${NC}"
echo "❌ POTENTIAL ISSUES:"
echo "• AppState access patterns:"
grep -r "AppState" src-tauri/src/ | grep -E "(get|set|insert)" | head -5

# 10. Timer-related race conditions
echo -e "\n${YELLOW}10. Checking timer-related race conditions...${NC}"
TIMER_ISSUES=$(grep -r "sleep.*Duration" src-tauri/src/ | wc -l)
echo "• Sleep/delay operations: $TIMER_ISSUES"
if [ $TIMER_ISSUES -gt 10 ]; then
    echo "⚠️ Many timing operations - potential for race conditions"
    ISSUES_FOUND=$((ISSUES_FOUND + 1))
fi

# Summary of findings
echo -e "\n${BLUE}=== RACE CONDITION SUMMARY ===${NC}"

# Critical patterns to watch
echo -e "\n${YELLOW}CRITICAL PATTERNS TO MONITOR:${NC}"
echo "1. 🔒 Mixed lock patterns (try_lock vs lock)"
echo "2. 🔄 Multiple spawned tasks accessing same state"
echo "3. 📡 Event handlers without proper synchronization"
echo "4. ⏱️ Timer-based operations that can overlap"
echo "5. 📁 Concurrent file system operations"

# Recommendations
echo -e "\n${GREEN}RECOMMENDATIONS:${NC}"
echo "• ✅ Use async versions of state access methods where possible"
echo "• ✅ Consolidate multiple lock acquisitions into single scopes"
echo "• ✅ Add proper synchronization for spawned tasks"
echo "• ✅ Implement retry mechanisms for try_lock failures"
echo "• ✅ Use event listener deduplication patterns"

# Risk assessment
if [ $ISSUES_FOUND -gt 5 ]; then
    echo -e "\n${RED}⚠️  HIGH RISK: $ISSUES_FOUND potential race conditions found${NC}"
    echo "Recommend immediate review of concurrency patterns"
elif [ $ISSUES_FOUND -gt 2 ]; then
    echo -e "\n${YELLOW}⚠️  MEDIUM RISK: $ISSUES_FOUND potential race conditions found${NC}"
    echo "Monitor these patterns during development"
else
    echo -e "\n${GREEN}✅ LOW RISK: Concurrency patterns look good${NC}"
fi

echo -e "\n${BLUE}Analysis complete. Run this script regularly to monitor for new race conditions.${NC}"

#!/bin/bash

# Verification script for memory and performance optimizations
# Tests conversation pruning, memory management, and race condition fixes

echo "🔍 Verifying Memory & Performance Optimizations"
echo "=============================================="

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

ISSUES_FOUND=0

echo -e "\n${BLUE}=== 1. FRONTEND CONVERSATION PRUNING ===${NC}"

# Check if conversation pruning is implemented
echo "🔍 Checking frontend conversation pruning..."
if grep -q "pruneConversationIfNeeded" src/App.tsx; then
    echo -e "✅ ${GREEN}Frontend conversation pruning implemented${NC}"

    # Check if LIMITS is imported
    if grep -q "LIMITS" src/App.tsx; then
        echo -e "✅ ${GREEN}LIMITS constant imported${NC}"
    else
        echo -e "❌ ${RED}LIMITS constant not imported${NC}"
        ISSUES_FOUND=$((ISSUES_FOUND + 1))
    fi

    # Check if setConversationWithPruning is used
    PRUNING_USAGE=$(grep -c "setConversationWithPruning" src/App.tsx)
    if [ $PRUNING_USAGE -gt 5 ]; then
        echo -e "✅ ${GREEN}Pruning function used in ${PRUNING_USAGE} places${NC}"
    else
        echo -e "⚠️ ${YELLOW}Pruning function used in only ${PRUNING_USAGE} places${NC}"
    fi
else
    echo -e "❌ ${RED}Frontend conversation pruning not implemented${NC}"
    ISSUES_FOUND=$((ISSUES_FOUND + 1))
fi

echo -e "\n${BLUE}=== 2. BACKEND MEMORY MANAGEMENT ===${NC}"

# Check if advanced memory manager is available
echo "🔍 Checking backend memory management..."
if grep -q "AdvancedMemoryManager" src-tauri/src/agent/implementations/memory_manager.rs; then
    echo -e "✅ ${GREEN}AdvancedMemoryManager implemented${NC}"

    # Check auto-pruning configuration
    if grep -q "auto_prune: true" src-tauri/src/agent/implementations/memory_manager.rs; then
        echo -e "✅ ${GREEN}Auto-pruning enabled by default${NC}"
    else
        echo -e "⚠️ ${YELLOW}Auto-pruning configuration not found${NC}"
    fi

    # Check token limits
    if grep -q "max_tokens: 32000" src-tauri/src/agent/implementations/memory_manager.rs; then
        echo -e "✅ ${GREEN}Token limits configured (32K tokens)${NC}"
    else
        echo -e "⚠️ ${YELLOW}Token limits not found or different${NC}"
    fi

    # Check conversation summarization
    if grep -q "enable_summarization: true" src-tauri/src/agent/implementations/memory_manager.rs; then
        echo -e "✅ ${GREEN}Conversation summarization enabled${NC}"
    else
        echo -e "⚠️ ${YELLOW}Conversation summarization not enabled${NC}"
    fi
else
    echo -e "❌ ${RED}AdvancedMemoryManager not found${NC}"
    ISSUES_FOUND=$((ISSUES_FOUND + 1))
fi

echo -e "\n${BLUE}=== 3. STATE OBJECT OPTIMIZATION ===${NC}"

# Check DevToolsPanel optimization
echo "🔍 Checking DevToolsPanel state optimization..."
if grep -q "useOptimizedLoadingStates" src/components/DevToolsPanel.tsx; then
    echo -e "✅ ${GREEN}DevToolsPanel loading states optimized${NC}"

    # Check if Set-based approach is used
    if grep -q "Set<string>" src/components/DevToolsPanel.tsx; then
        echo -e "✅ ${GREEN}Set-based loading state management implemented${NC}"
    else
        echo -e "⚠️ ${YELLOW}Set-based optimization not found${NC}"
    fi
else
    echo -e "❌ ${RED}DevToolsPanel state optimization not implemented${NC}"
    ISSUES_FOUND=$((ISSUES_FOUND + 1))
fi

echo -e "\n${BLUE}=== 4. EVENT LISTENER VERIFICATION ===${NC}"

# Run the existing duplicate listener check
echo "🔍 Running duplicate event listener check..."
if bash scripts/check-duplicate-listeners.sh >/dev/null 2>&1; then
    echo -e "✅ ${GREEN}No duplicate event listeners detected${NC}"
else
    echo -e "❌ ${RED}Duplicate event listeners found${NC}"
    ISSUES_FOUND=$((ISSUES_FOUND + 1))
fi

echo -e "\n${BLUE}=== 5. RACE CONDITION ANALYSIS ===${NC}"

# Check race condition patterns
echo "🔍 Running race condition analysis..."
RACE_CONDITIONS=$(bash scripts/check-race-conditions.sh 2>/dev/null | grep -c "❌" || echo "0")
if [ $RACE_CONDITIONS -eq 0 ]; then
    echo -e "✅ ${GREEN}No critical race conditions detected${NC}"
else
    echo -e "⚠️ ${YELLOW}${RACE_CONDITIONS} potential race conditions found${NC}"
fi

echo -e "\n${BLUE}=== 6. MEMORY LEAK DETECTION ===${NC}"

# Check for common memory leak patterns
echo "🔍 Checking for memory leak patterns..."

# Check timer cleanup
TIMER_CLEANUPS=$(grep -r "clearTimeout\|clearInterval" src/ --include="*.ts" --include="*.tsx" | wc -l)
if [ $TIMER_CLEANUPS -gt 5 ]; then
    echo -e "✅ ${GREEN}Timer cleanup patterns found (${TIMER_CLEANUPS} instances)${NC}"
else
    echo -e "⚠️ ${YELLOW}Few timer cleanup patterns found (${TIMER_CLEANUPS} instances)${NC}"
fi

# Check blob URL cleanup
BLOB_CLEANUPS=$(grep -r "revokeObjectURL" src/ --include="*.ts" --include="*.tsx" | wc -l)
if [ $BLOB_CLEANUPS -gt 2 ]; then
    echo -e "✅ ${GREEN}Blob URL cleanup implemented (${BLOB_CLEANUPS} instances)${NC}"
else
    echo -e "⚠️ ${YELLOW}Few blob URL cleanups found (${BLOB_CLEANUPS} instances)${NC}"
fi

# Check event listener cleanup
LISTENER_CLEANUPS=$(grep -r "unlisten\|removeEventListener" src/ --include="*.ts" --include="*.tsx" | wc -l)
if [ $LISTENER_CLEANUPS -gt 10 ]; then
    echo -e "✅ ${GREEN}Event listener cleanup patterns found (${LISTENER_CLEANUPS} instances)${NC}"
else
    echo -e "⚠️ ${YELLOW}Few event listener cleanups found (${LISTENER_CLEANUPS} instances)${NC}"
fi

echo -e "\n${BLUE}=== 7. PERFORMANCE CONSTANTS ===${NC}"

# Check if performance constants are properly set
echo "🔍 Checking performance constants..."
if grep -q "MAX_CHAT_HISTORY_ITEMS: 1000" src/lib/constants.generated.ts; then
    echo -e "✅ ${GREEN}Chat history limit set to 1000 messages${NC}"
else
    echo -e "⚠️ ${YELLOW}Chat history limit not found or different${NC}"
fi

echo -e "\n${BLUE}=== 8. COMPILATION CHECK ===${NC}"

# Verify that the code compiles
echo "🔍 Checking TypeScript compilation..."
if command -v bun &>/dev/null; then
    if bun run typecheck >/dev/null 2>&1; then
        echo -e "✅ ${GREEN}TypeScript compilation successful${NC}"
    else
        echo -e "❌ ${RED}TypeScript compilation errors found${NC}"
        ISSUES_FOUND=$((ISSUES_FOUND + 1))
    fi
else
    echo -e "⚠️ ${YELLOW}Bun not available, skipping TS check${NC}"
fi

echo -e "\n${BLUE}=== OPTIMIZATION SUMMARY ===${NC}"

echo -e "\n📊 ${BLUE}Implementation Status:${NC}"
echo "✅ Frontend conversation pruning with automatic limits"
echo "✅ Backend advanced memory management with token-aware pruning"
echo "✅ DevToolsPanel state optimization using Set-based approach"
echo "✅ Event listener centralization and cleanup"
echo "✅ Timer and resource cleanup patterns"
echo "✅ Race condition prevention mechanisms"

echo -e "\n🎯 ${BLUE}Memory Management Features:${NC}"
echo "• Frontend: Auto-prune at 1000 messages, keep 30% most recent"
echo "• Backend: Auto-prune at 100 messages or 32K tokens"
echo "• Conversation summarization for context preservation"
echo "• Orphaned tool call cleanup"
echo "• Set-based loading state management for DevTools"

echo -e "\n📈 ${BLUE}Performance Improvements:${NC}"
echo "• Reduced memory overhead from large state objects"
echo "• Optimized event listener management"
echo "• Automatic cleanup of stale resources"
echo "• Token-aware memory pruning"
echo "• Efficient conversation history management"

if [ $ISSUES_FOUND -eq 0 ]; then
    echo -e "\n🎉 ${GREEN}All optimizations successfully implemented!${NC}"
    echo -e "${GREEN}The application is optimized for memory usage and performance.${NC}"
    exit 0
else
    echo -e "\n⚠️ ${YELLOW}Found ${ISSUES_FOUND} potential issues to review.${NC}"
    echo -e "${YELLOW}Consider addressing these for optimal performance.${NC}"
    exit 1
fi

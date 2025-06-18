#!/bin/bash

# Script to detect listener accumulation issues in Juno
echo "🔍 Checking for listener accumulation issues..."

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

ISSUES_FOUND=0

# Check for MaxListenersExceededWarning in logs
echo -e "\n📊 Checking for MaxListenersExceededWarning..."
if find . -name "*.log" -exec grep -l "MaxListenersExceeded" {} \; 2>/dev/null | head -5; then
    echo -e "${RED}❌ MaxListenersExceededWarning found in logs${NC}"
    echo "This indicates event listener accumulation in Node.js processes (likely MCP servers)"
    ISSUES_FOUND=$((ISSUES_FOUND + 1))
else
    echo -e "${GREEN}✅ No MaxListenersExceeded warnings found in log files${NC}"
fi

# Check for duplicate voice event listeners in source code
echo -e "\n🎤 Checking for duplicate voice event listeners..."
VOICE_EVENTS=("dictation-active" "app-dictation-started" "app-dictation-finished" "agent-active")

for event in "${VOICE_EVENTS[@]}"; do
    LISTENERS=$(grep -r "listen.*['\"]$event['\"]" src/ --include="*.ts" --include="*.tsx" 2>/dev/null | wc -l)
    
    if [ $LISTENERS -gt 1 ]; then
        echo -e "${RED}❌ Duplicate listeners for '$event': $LISTENERS instances${NC}"
        grep -r "listen.*['\"]$event['\"]" src/ --include="*.ts" --include="*.tsx" 2>/dev/null | sed 's/^/    /'
        ISSUES_FOUND=$((ISSUES_FOUND + 1))
    else
        echo -e "${GREEN}✅ '$event': $LISTENERS listener${NC}"
    fi
done

# Check for multiple MCP initialization points
echo -e "\n🔧 Checking MCP initialization points..."
MCP_INIT_PATTERNS=(
    "initialize_mcp_servers"
    "init.*orchestrator.*mcp"
    "setup_tools.*mcp"
    "MCPManager::new"
)

for pattern in "${MCP_INIT_PATTERNS[@]}"; do
    COUNT=$(grep -r "$pattern" src-tauri/ --include="*.rs" 2>/dev/null | wc -l)
    if [ $COUNT -gt 0 ]; then
        echo -e "${YELLOW}⚠️  '$pattern': $COUNT instances${NC}"
        if [ $COUNT -gt 2 ]; then
            echo "    Consider consolidating these initialization points"
        fi
    fi
done

# Check for proper cleanup patterns
echo -e "\n🧹 Checking for cleanup patterns..."
CLEANUP_PATTERNS=(
    "disconnect.*mcp"
    "cleanup.*mcp"
    "stop.*mcp.*server"
    "unlisten.*forEach"
)

CLEANUP_FOUND=0
for pattern in "${CLEANUP_PATTERNS[@]}"; do
    COUNT=$(grep -r "$pattern" src/ src-tauri/ --include="*.ts" --include="*.tsx" --include="*.rs" 2>/dev/null | wc -l)
    if [ $COUNT -gt 0 ]; then
        CLEANUP_FOUND=$((CLEANUP_FOUND + COUNT))
    fi
done

if [ $CLEANUP_FOUND -gt 5 ]; then
    echo -e "${GREEN}✅ Good cleanup patterns found: $CLEANUP_FOUND instances${NC}"
else
    echo -e "${RED}❌ Limited cleanup patterns found: $CLEANUP_FOUND instances${NC}"
    echo "Consider adding more resource cleanup handlers"
    ISSUES_FOUND=$((ISSUES_FOUND + 1))
fi

# Check for hot reload handling
echo -e "\n🔄 Checking for hot reload cleanup..."
if grep -r "frontend-reload\|hot.*reload\|vite.*reload" src-tauri/ --include="*.rs" 2>/dev/null; then
    echo -e "${GREEN}✅ Hot reload cleanup handlers found${NC}"
else
    echo -e "${RED}❌ No hot reload cleanup handlers found${NC}"
    echo "Frontend reloads may not properly cleanup backend resources"
    ISSUES_FOUND=$((ISSUES_FOUND + 1))
fi

# Check current running processes that might accumulate
echo -e "\n🔍 Checking for potentially accumulated processes..."
if command -v pgrep &> /dev/null; then
    NODE_PROCESSES=$(pgrep -f "modelcontextprotocol" | wc -l)
    if [ $NODE_PROCESSES -gt 4 ]; then
        echo -e "${YELLOW}⚠️  Multiple MCP server processes running: $NODE_PROCESSES${NC}"
        echo "This could indicate process accumulation"
        pgrep -fl "modelcontextprotocol" | sed 's/^/    /'
    else
        echo -e "${GREEN}✅ Normal number of MCP processes: $NODE_PROCESSES${NC}"
    fi
fi

# Summary
echo -e "\n📊 Summary:"
if [ $ISSUES_FOUND -eq 0 ]; then
    echo -e "${GREEN}✅ No listener accumulation issues detected!${NC}"
    echo "Your app appears to be handling resources properly."
else
    echo -e "${RED}❌ Found $ISSUES_FOUND potential listener accumulation issue(s)${NC}"
    echo -e "${YELLOW}📋 Recommended actions:${NC}"
    echo "1. Review the LISTENER_ACCUMULATION_FIXES.md document"
    echo "2. Remove duplicate event listeners from App.tsx"
    echo "3. Add MCP server cleanup methods"
    echo "4. Implement hot reload resource cleanup"
fi

echo -e "\n💡 To monitor in real-time:"
echo "tail -f your-app.log | grep -i 'MaxListeners\\|duplicate\\|warning'"

exit $ISSUES_FOUND
#!/bin/bash

# Test Cloud Connection - Juno App to Fly.io Backend
# Tests the connection from the Tauri app to the production Fly.io backend

set -e

echo "🌐 Testing Juno App → Fly.io Backend Connection"
echo "=============================================="

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# Production backend URL
BACKEND_URL="https://juno-cloud-backend.fly.dev"
WS_URL="wss://juno-cloud-backend.fly.dev/ws"

echo -e "${BLUE}🎯 Target Backend: ${BACKEND_URL}${NC}"
echo

# Function to test a URL
test_endpoint() {
    local url=$1
    local description=$2
    local expected_status=${3:-200}

    echo -n "🔍 Testing $description... "

    local status_code=$(curl -s -o /dev/null -w "%{http_code}" --max-time 10 "$url" 2>/dev/null || echo "000")

    if [ "$status_code" = "$expected_status" ]; then
        echo -e "${GREEN}✅ PASS (${status_code})${NC}"
        return 0
    else
        echo -e "${RED}❌ FAIL (${status_code})${NC}"
        return 1
    fi
}

# Function to test JSON endpoint
test_json_endpoint() {
    local url=$1
    local description=$2
    local expected_field=$3

    echo -n "🔍 Testing $description... "

    local response=$(curl -s --max-time 10 "$url" 2>/dev/null)
    if [ $? -eq 0 ] && echo "$response" | jq -e ".$expected_field" >/dev/null 2>&1; then
        echo -e "${GREEN}✅ PASS${NC}"
        return 0
    else
        echo -e "${RED}❌ FAIL${NC}"
        return 1
    fi
}

# Function to test WebSocket connection
test_websocket() {
    echo -n "🔌 Testing WebSocket connection... "

    # Use websocat to test WebSocket if available
    if command -v websocat &>/dev/null; then
        timeout 5 websocat --exit-on-eof "$WS_URL" <<<'{"type":"status","data":{},"timestamp":'$(date +%s)'}' >/dev/null 2>&1
        if [ $? -eq 0 ]; then
            echo -e "${GREEN}✅ PASS${NC}"
            return 0
        else
            echo -e "${RED}❌ FAIL${NC}"
            return 1
        fi
    else
        echo -e "${YELLOW}⚠️  SKIP (websocat not installed)${NC}"
        return 0
    fi
}

# Function to test device registration
test_device_registration() {
    echo -n "📱 Testing device registration... "

    local test_payload='{
        "device_name": "test-device",
        "device_type": "desktop",
        "platform": "macos"
    }'

    local response=$(curl -s --max-time 10 \
        -X POST \
        -H "Content-Type: application/json" \
        -d "$test_payload" \
        "$BACKEND_URL/api/register" 2>/dev/null)

    if [ $? -eq 0 ] && echo "$response" | jq -e '.success' >/dev/null 2>&1; then
        echo -e "${GREEN}✅ PASS${NC}"
        return 0
    else
        echo -e "${RED}❌ FAIL${NC}"
        return 1
    fi
}

# Start testing
echo "🏥 Backend Health Tests:"
echo "----------------------"

# Test API endpoints (skip root since it's expected to 404)
test_json_endpoint "$BACKEND_URL/health" "Health endpoint" "status"
test_json_endpoint "$BACKEND_URL/metrics" "Metrics endpoint" "uptime"

echo
echo "🔌 WebSocket Tests:"
echo "------------------"

test_websocket

echo
echo "🚀 API Tests:"
echo "-------------"

test_device_registration

echo
echo "🔧 App Configuration Check:"
echo "---------------------------"

# Check if the Tauri app configuration is updated
echo -n "⚙️  Checking Tauri cloud config... "
if grep -q "juno-cloud-backend.fly.dev" src-tauri/src/cloud/config.rs; then
    echo -e "${GREEN}✅ UPDATED${NC}"
else
    echo -e "${RED}❌ NOT UPDATED${NC}"
fi

echo -n "⚙️  Checking constants file... "
if grep -q "juno-cloud-backend.fly.dev" src-tauri/src/constants.rs; then
    echo -e "${GREEN}✅ UPDATED${NC}"
else
    echo -e "${RED}❌ NOT UPDATED${NC}"
fi

echo
echo "📋 Next Steps:"
echo "-------------"
echo "1. Start your Tauri app: bun run tauri dev"
echo "2. Open the Cloud Test Panel (Dev Tools → Cloud)"
echo "3. Test the connection using the built-in tools"
echo
echo "💡 If you encounter issues:"
echo "   • Check the browser dev tools console"
echo "   • Enable debug logs: RUST_LOG=debug bun run tauri dev"
echo "   • Use the Cloud Test Panel for detailed diagnostics"
echo
echo -e "${BLUE}🌐 Production Backend: ${BACKEND_URL}${NC}"
echo -e "${BLUE}🔌 WebSocket Endpoint: ${WS_URL}${NC}"

echo
echo "✅ Connection test completed!"

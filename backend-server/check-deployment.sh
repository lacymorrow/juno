#!/bin/bash

# Juno Cloud Backend - Deployment Status Checker
# Validates the current Fly.io deployment health and functionality

set -e

echo "🚀 Juno Cloud Backend - Deployment Status Check"
echo "================================================"

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# Production URL
PROD_URL="https://juno-cloud-backend.fly.dev"

# Function to check a URL with timeout
check_url() {
    local url=$1
    local description=$2

    echo -n "🔍 Checking $description... "

    if curl -s --max-time 10 --fail "$url" >/dev/null 2>&1; then
        echo -e "${GREEN}✅ PASS${NC}"
        return 0
    else
        echo -e "${RED}❌ FAIL${NC}"
        return 1
    fi
}

# Function to check JSON endpoint
check_json_endpoint() {
    local url=$1
    local description=$2
    local expected_field=$3

    echo -n "🔍 Checking $description... "

    response=$(curl -s --max-time 10 --fail "$url" 2>/dev/null)
    if [ $? -eq 0 ] && echo "$response" | jq -e ".$expected_field" >/dev/null 2>&1; then
        echo -e "${GREEN}✅ PASS${NC}"
        return 0
    else
        echo -e "${RED}❌ FAIL${NC}"
        echo "  Response: $response"
        return 1
    fi
}

# Function to test API endpoint
test_api_endpoint() {
    echo -n "🔍 Testing device registration API... "

    response=$(curl -s --max-time 10 -X POST \
        -H "Content-Type: application/json" \
        -d '{"device_name":"deployment-test","device_type":"test","platform":"test"}' \
        "$PROD_URL/api/register" 2>/dev/null)

    if [ $? -eq 0 ] && echo "$response" | jq -e '.success' >/dev/null 2>&1; then
        echo -e "${GREEN}✅ PASS${NC}"
        return 0
    else
        echo -e "${RED}❌ FAIL${NC}"
        echo "  Response: $response"
        return 1
    fi
}

echo ""
echo -e "${BLUE}📊 Basic Connectivity Tests${NC}"
echo "----------------------------"

# Basic connectivity
total_tests=0
passed_tests=0

# Health check
if check_url "$PROD_URL/health" "Health endpoint"; then
    ((passed_tests++))
fi
((total_tests++))

# Metrics endpoint
if check_url "$PROD_URL/metrics" "Metrics endpoint"; then
    ((passed_tests++))
fi
((total_tests++))

echo ""
echo -e "${BLUE}🔬 API Functionality Tests${NC}"
echo "---------------------------"

# Health endpoint JSON structure
if check_json_endpoint "$PROD_URL/health" "Health JSON structure" "status"; then
    ((passed_tests++))
fi
((total_tests++))

# Device registration API
if test_api_endpoint; then
    ((passed_tests++))
fi
((total_tests++))

echo ""
echo -e "${BLUE}📈 Detailed Health Information${NC}"
echo "-------------------------------"

echo -n "📊 Fetching health details... "
health_response=$(curl -s --max-time 10 "$PROD_URL/health" 2>/dev/null)
if [ $? -eq 0 ]; then
    echo -e "${GREEN}✅ SUCCESS${NC}"
    echo ""
    echo "Health Status:"
    echo "$health_response" | jq '.' 2>/dev/null || echo "$health_response"
else
    echo -e "${RED}❌ FAILED${NC}"
fi

echo ""
echo -e "${BLUE}🚀 Fly.io Deployment Status${NC}"
echo "----------------------------"

# Check if flyctl is available
if command -v flyctl &>/dev/null; then
    echo "📋 App Status:"
    flyctl status --app juno-cloud-backend 2>/dev/null || echo "Could not fetch Fly.io status"

    echo ""
    echo "📊 Recent Logs:"
    flyctl logs --app juno-cloud-backend --no-tail 2>/dev/null | tail -5 || echo "Could not fetch logs"
else
    echo "⚠️  Fly CLI not available - install with: brew install flyctl"
fi

echo ""
echo -e "${BLUE}📋 Test Summary${NC}"
echo "----------------"

if [ $passed_tests -eq $total_tests ]; then
    echo -e "${GREEN}✅ ALL TESTS PASSED${NC} ($passed_tests/$total_tests)"
    echo ""
    echo -e "${GREEN}🎉 Deployment is healthy and operational!${NC}"
    echo ""
    echo "🔗 Production URLs:"
    echo "   • Main: $PROD_URL"
    echo "   • Health: $PROD_URL/health"
    echo "   • Metrics: $PROD_URL/metrics"
    echo "   • API: $PROD_URL/api"
    echo "   • WebSocket: wss://juno-cloud-backend.fly.dev/ws"
    exit 0
else
    echo -e "${RED}❌ SOME TESTS FAILED${NC} ($passed_tests/$total_tests)"
    echo ""
    echo -e "${YELLOW}🔧 Troubleshooting steps:${NC}"
    echo "   1. Check Fly.io app status: flyctl status"
    echo "   2. Check logs: flyctl logs"
    echo "   3. Verify health endpoint manually: curl $PROD_URL/health"
    echo "   4. See FLY_DEPLOYMENT_RULES.md for troubleshooting guide"
    exit 1
fi

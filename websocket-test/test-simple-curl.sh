#!/bin/bash

echo "🚀 Juno AI Cloud Control - Simple Curl Test"
echo "==========================================="
echo ""

echo "📱 Step 1: Register a test device..."
REGISTRATION_RESPONSE=$(curl -s -X POST -H "Content-Type: application/json" \
    -d '{"device_name":"curl-test-agent","device_type":"desktop","platform":"macos"}' \
    https://juno-cloud-backend.fly.dev/api/register)

echo "✅ Registration Response:"
echo "$REGISTRATION_RESPONSE" | jq '.'
echo ""

# Extract API key and device ID
API_KEY=$(echo "$REGISTRATION_RESPONSE" | jq -r '.api_key')
DEVICE_ID=$(echo "$REGISTRATION_RESPONSE" | jq -r '.device_id')

echo "🔑 Extracted Credentials:"
echo "   Device ID: $DEVICE_ID"
echo "   API Key: $API_KEY"
echo ""

echo "🔗 Instructions for your Juno app:"
echo "1. Open Juno Settings → Network → Cloud Control Testing"
echo "2. Set the password to: $API_KEY"
echo "3. Click 'Set Password' and 'Test Connection'"
echo ""

echo "📋 Manual WebSocket test command:"
echo ""
echo "node -e \""
echo "const WebSocket = require('ws');"
echo "const ws = new WebSocket('wss://juno-cloud-backend.fly.dev/ws');"
echo "ws.on('open', () => {"
echo "  console.log('Connected!');"
echo "  ws.send(JSON.stringify({"
echo "    type: 'auth',"
echo "    api_key: '$API_KEY',"
echo "    device_id: '$DEVICE_ID'"
echo "  }));"
echo "  setTimeout(() => {"
echo "    ws.send(JSON.stringify({"
echo "      type: 'command',"
echo "      command: 'take_screenshot',"
echo "      content: 'Take a screenshot'"
echo "    }));"
echo "  }, 2000);"
echo "  setTimeout(() => ws.close(), 5000);"
echo "});"
echo "ws.on('message', (data) => console.log('Received:', data.toString()));"
echo "\""
echo ""

echo "✨ Test completed! Your cloud backend is ready for testing."

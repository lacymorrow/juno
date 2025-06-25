#!/bin/bash

# Configuration
API_KEY="eea05e0e52e3b07be0647b862ff71680168858d68cbb0c3e83dcb9c77dc87af0"
HMAC_SECRET="7fd8a36b1fec73e50ca6be13e47992beb5c48e2a9a0af41328626378b8418244"
SERVER_HOST="juno-cloud-backend.fly.dev"

# Get query from command line arguments
QUERY="${*:-Hello, AI agent! Please introduce yourself and tell me what you can do.}"

echo "🤖 Calling Juno AI Agent via HTTP..."
echo "📝 Query: \"$QUERY\""
echo ""

# Generate timestamp
TIMESTAMP=$(date +%s)

# Generate UUID for command ID
COMMAND_ID=$(uuidgen | tr '[:upper:]' '[:lower:]')

# Create the request body
REQUEST_BODY=$(
    cat <<EOF
{
  "id": "$COMMAND_ID",
  "command_type": "text_query",
  "payload": {
    "query": "$QUERY"
  },
  "timestamp": $TIMESTAMP
}
EOF
)

# Generate HMAC signature
METHOD="POST"
PATH="/api/command"
PAYLOAD="$METHOD:$PATH:$REQUEST_BODY:$TIMESTAMP"

# Generate HMAC using openssl
SIGNATURE=$(echo -n "$PAYLOAD" | openssl dgst -sha256 -hmac "$HMAC_SECRET" -binary | xxd -p -c 256)

echo "🔐 Generated authentication signature..."
echo "📡 Sending request to server..."

# Make the HTTP request
RESPONSE=$(curl -s -w "\nHTTP_STATUS:%{http_code}\n" \
    -X POST \
    -H "Content-Type: application/json" \
    -H "X-API-Key: $API_KEY" \
    -H "X-Timestamp: $TIMESTAMP" \
    -H "X-Signature: $SIGNATURE" \
    -H "User-Agent: Juno-Agent-CLI/1.0" \
    -d "$REQUEST_BODY" \
    "https://$SERVER_HOST$PATH" 2>/dev/null)

# Extract HTTP status code
HTTP_STATUS=$(echo "$RESPONSE" | grep "HTTP_STATUS:" | cut -d: -f2)
RESPONSE_BODY=$(echo "$RESPONSE" | sed '/HTTP_STATUS:/d')

echo ""

if [ "$HTTP_STATUS" = "200" ] || [ "$HTTP_STATUS" = "201" ]; then
    echo "✅ Request successful!"
    echo ""
    echo "🤖 AI Agent Response:"
    echo "─────────────────────────────────────────────────────"

    # Try to parse JSON response and extract the text
    if command -v jq >/dev/null 2>&1; then
        # Use jq if available for pretty JSON parsing
        echo "$RESPONSE_BODY" | jq -r '.data.text // .text // .message // .'
    else
        # Fallback to basic parsing
        echo "$RESPONSE_BODY" | sed 's/.*"text"[[:space:]]*:[[:space:]]*"\([^"]*\)".*/\1/' | head -1
        if [ ${PIPESTATUS[1]} -ne 0 ]; then
            echo "$RESPONSE_BODY"
        fi
    fi

    echo "─────────────────────────────────────────────────────"
    echo ""
    echo "✨ Query completed successfully!"

elif [ "$HTTP_STATUS" = "404" ]; then
    echo "❌ Endpoint not found (HTTP 404)"
    echo "💡 The server might only support WebSocket connections."
    echo ""
    echo "🔄 Try using the WebSocket version instead:"
    echo "   node call-agent.js \"$QUERY\""

elif [ "$HTTP_STATUS" = "401" ] || [ "$HTTP_STATUS" = "403" ]; then
    echo "❌ Authentication failed (HTTP $HTTP_STATUS)"
    echo "🔐 Please check your API key and HMAC secret"
    echo ""
    echo "Response:"
    echo "$RESPONSE_BODY"

else
    echo "❌ Request failed (HTTP $HTTP_STATUS)"
    echo ""
    echo "Response:"
    echo "$RESPONSE_BODY"

    if [ -z "$HTTP_STATUS" ]; then
        echo ""
        echo "💡 Connection may have failed. Check your internet connection."
    fi
fi

echo ""
echo "📋 Debug Info:"
echo "   Command ID: $COMMAND_ID"
echo "   Timestamp: $TIMESTAMP"
echo "   Signature: ${SIGNATURE:0:16}..."
echo "   HTTP Status: $HTTP_STATUS"

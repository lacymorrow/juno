#!/bin/bash

# Juno WebSocket Test Server Startup Script

echo "🚀 Starting Juno WebSocket Test Server..."
echo ""

# Check if Node.js is installed
if ! command -v node &>/dev/null; then
    echo "❌ Node.js is not installed. Please install Node.js to run the server."
    exit 1
fi

# Check if dependencies are installed
if [ ! -d "node_modules" ]; then
    echo "📦 Installing dependencies..."
    npm install
    echo ""
fi

# Start the server
echo "🎯 Server will run on: http://localhost:8080"
echo "📡 WebSocket endpoint: ws://localhost:8080"
echo "🏥 Health check: http://localhost:8080/health"
echo ""
echo "Press Ctrl+C to stop the server"
echo "======================================="
echo ""

# Start with output
node server.js

#!/bin/bash

# Test script for debug request logging feature
# This script helps verify that the debug logging is working properly

echo "🧪 Testing Debug Request Logging Feature"
echo "========================================"

# Check if we're in the right directory
if [ ! -f "src-tauri/Cargo.toml" ]; then
    echo "❌ Error: Please run this script from the juno project root directory"
    exit 1
fi

# Create debug directory
echo "📁 Creating debug directory..."
mkdir -p debug

# Check if chrono dependency is available
echo "🔍 Checking dependencies..."
if ! grep -q "chrono" src-tauri/Cargo.toml; then
    echo "⚠️  Warning: chrono dependency might be missing from Cargo.toml"
    echo "   The debug feature uses chrono for timestamps"
else
    echo "✅ chrono dependency found"
fi

# Check if the debug feature is implemented
echo "🔍 Checking implementation..."
if grep -q "save_debug_request" src-tauri/src/agent/providers/anthropic.rs; then
    echo "✅ save_debug_request function found"
else
    echo "❌ save_debug_request function not found"
fi

if grep -q "#\[cfg(debug_assertions)\]" src-tauri/src/agent/providers/anthropic.rs; then
    echo "✅ Debug assertions guard found"
else
    echo "❌ Debug assertions guard not found"
fi

# Check gitignore
echo "🔍 Checking .gitignore..."
if grep -q "debug/" .gitignore; then
    echo "✅ debug/ directory is in .gitignore"
else
    echo "⚠️  Warning: debug/ directory should be added to .gitignore"
fi

echo ""
echo "🚀 To test the feature:"
echo "1. Run: RUST_LOG=debug bun run tauri dev"
echo "2. Trigger an agent request (Option+D or chat interface)"
echo "3. Check for files: ls -la debug/"
echo "4. View a request: cat debug/agent_request_*.json | jq ."
echo ""
echo "📝 Expected behavior:"
echo "- Files saved only in debug builds"
echo "- Timestamp-based filenames"
echo "- Complete, unredacted request data"
echo "- Automatic directory creation"
echo ""
echo "✨ Happy debugging!"

#!/bin/bash

echo "Testing Input Monitoring Permission Check..."
echo "============================================"

# Test 1: Check TCC database directly
echo ""
echo "Test 1: Checking TCC database..."
sqlite3 "$HOME/Library/Application Support/com.apple.TCC/TCC.db" \
  "SELECT client, allowed FROM access WHERE service='kTCCServiceListenEvent';" 2>/dev/null || \
  echo "Cannot access TCC database (this is normal without Full Disk Access)"

# Test 2: Try AppleScript test
echo ""
echo "Test 2: Testing with AppleScript..."
osascript -e '
use framework "Foundation"
use framework "AppKit"
try
    tell application "System Events"
        key code 0
    end tell
    return "Input monitoring: GRANTED"
on error
    return "Input monitoring: NOT GRANTED"
end try
'

# Test 3: Check if we can query IOHIDRequestTypeListenEvent
echo ""
echo "Test 3: Checking IOHIDRequestTypeListenEvent..."
ioreg -r -k "IOHIDInterface" | grep -q "IOHIDInterface" && \
  echo "HID interfaces found (hardware present)" || \
  echo "No HID interfaces found"

# Test 4: Try to test with a simple key event
echo ""
echo "Test 4: Testing key event capability..."
osascript -e 'tell application "System Events" to keystroke ""' 2>&1 | \
  grep -q "not allowed" && \
  echo "Key events: NOT ALLOWED (no input monitoring permission)" || \
  echo "Key events: ALLOWED or inconclusive"

echo ""
echo "============================================"
echo "To grant Input Monitoring permission:"
echo "1. Open System Settings > Privacy & Security > Input Monitoring"
echo "2. Enable the toggle for Juno (or Terminal if testing from command line)"
echo ""
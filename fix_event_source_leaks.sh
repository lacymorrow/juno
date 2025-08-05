#!/bin/bash

# Script to fix CGEventSource resource leaks by replacing direct creation with pooled sources

FILE="/Users/lacymorrow/repo/juno/src-tauri/mcp-server-os-level/src/platforms/macos/interaction.rs"

# Create a backup
cp "$FILE" "$FILE.bak"

# Replace CGEventSource::new with get_pooled_event_source
sed -i '' 's/CGEventSource::new(CGEventSourceStateID::HIDSystemState)\.map_err(|_| {\?$/get_pooled_event_source()\.map_err(|e| {/g' "$FILE"
sed -i '' 's/CGEventSource::new(CGEventSourceStateID::HIDSystemState) *$/get_pooled_event_source()/g' "$FILE"

# Fix error messages to use the error from get_pooled_event_source
sed -i '' 's/AutomationError::PlatformError("\([^"]*\)"\.to_string())/AutomationError::PlatformError(format!("\1: {}", e))/g' "$FILE"

echo "Fixed CGEventSource creation patterns in $FILE"
echo "Remember to add release_event_source(source) calls where the source goes out of scope!"
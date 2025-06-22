#!/bin/bash
# Quick cargo check with minimal context output
# Shows ✅ on success, filtered errors on failure

cd "$(dirname "$0")/.."

# Single-run cargo check with smart filtering
output=$(cargo check --manifest-path src-tauri/Cargo.toml --message-format=short 2>&1)
exit_code=$?

if [ $exit_code -eq 0 ]; then
	echo "✅ Compilation passed"
else
	echo "❌ Compilation failed:"
	echo "$output" | grep -E "(error|warning):" | head -20
	exit 1
fi

exit $exit_code

# juno-cua — Headless Computer Use Agent CLI

Standalone CLI that exposes Juno's computer use tools (screenshot, click, type, scroll, key press, clipboard, UI tree, etc.) without requiring the Tauri GUI runtime.

## Why?

- **Agent integration** — OpenClaw or any automation agent can call individual CUA tools via shell commands
- **Scripting** — Chain computer-use operations in bash/Python/whatever
- **Headless servers** — Run on machines without a display (with virtual framebuffers)
- **Testing** — Validate CUA tool behavior in CI

## Build

```sh
cd /path/to/juno
cargo build -p juno-cua --release
# Binary at: target/release/juno-cua
```

## Prerequisites

- **macOS**: Grant accessibility permissions to the binary in System Settings → Privacy & Security → Accessibility
- **Screen Recording** permission for screenshots

## Usage

All commands output JSON to stdout by default.

```sh
# Take a screenshot
juno-cua screenshot

# Click at coordinates
juno-cua click --x 500 --y 300
juno-cua click --x 500 --y 300 --button right
juno-cua click --x 500 --y 300 --button double

# Move mouse
juno-cua mouse-move --x 100 --y 200

# Get cursor position
juno-cua cursor-position

# Type text
juno-cua type-text --text "hello world"

# Press a key (with optional modifier)
juno-cua press-key --key Enter
juno-cua press-key --key c --modifier cmd

# Scroll
juno-cua scroll --x 500 --y 500 --direction down --amount 5

# Get focused element info
juno-cua focused-element

# Clipboard
juno-cua get-clipboard
juno-cua set-clipboard --content "copied text"

# UI tree (accessibility tree)
juno-cua ui-tree
juno-cua ui-tree --app "Safari"

# Find elements by selector
juno-cua find-elements --selector "AXButton"

# Open apps/URLs
juno-cua open-app --name "Terminal"
juno-cua open-url --url "https://example.com"

# Wait
juno-cua wait --ms 1000

# List all available tools (JSON schema)
juno-cua list-tools

# Generic tool call (any tool name + JSON args)
juno-cua call --tool pressKey --args '{"key":"Enter"}'
```

## Output Formats

```sh
juno-cua --format json screenshot      # Compact JSON (default)
juno-cua --format pretty screenshot    # Pretty-printed JSON
juno-cua --format quiet screenshot     # Silent (errors to stderr)
```

## Error Handling

On failure, `juno-cua` prints a JSON error to stderr and exits with code 1:

```json
{"error": "Failed to initialize Desktop engine. Check accessibility permissions."}
```

This makes it easy for callers to distinguish success (exit 0, JSON on stdout) from failure (exit 1, JSON error on stderr).

## OpenClaw Integration

Once built and on PATH, OpenClaw can invoke tools directly:

```sh
# Screenshot → pipe to analysis
juno-cua screenshot | jq -r '.screenshot_base64' | base64 -d > /tmp/screen.png

# Automated form filling
juno-cua click --x 300 --y 200
juno-cua type-text --text "user@example.com"
juno-cua press-key --key Tab
juno-cua type-text --text "password123"
juno-cua press-key --key Enter
```

# juno-cua — Desktop Automation for AI Agents

You have access to `juno-cua`, a CLI tool for macOS desktop automation. It lets you take screenshots, click, type, scroll, read accessibility trees, and control applications — all from the command line with JSON output.

## Quick Check

```bash
which juno-cua || echo "Not installed — run: brew install lacymorrow/tap/juno-cua"
```

## Installation

```bash
brew install lacymorrow/tap/juno-cua
```

Requires macOS accessibility permissions (System Settings → Privacy & Security → Accessibility).

## When to Use juno-cua

**USE juno-cua when you need to:**
- See what's on screen (screenshot, UI tree)
- Click buttons, menus, or UI elements
- Type into GUI applications (not terminal)
- Fill forms in native apps
- Navigate between applications
- Automate repetitive GUI tasks
- Read or set the clipboard

**DON'T use juno-cua when:**
- You can accomplish the task with shell commands, file I/O, or APIs
- You're working in a terminal-only context with no GUI needed
- The task involves web scraping (use curl/fetch instead)

## Tool Reference

### Screenshot & Vision

```bash
# Take a screenshot — returns {"screenshot_base64": "..."}
juno-cua screenshot

# Get the accessibility tree (structured UI representation)
juno-cua ui-tree
juno-cua ui-tree --app "Safari"

# Find specific UI elements
juno-cua find-elements --selector "AXButton"

# Get info about the currently focused element
juno-cua focused-element
```

### Mouse

```bash
# Click at screen coordinates
juno-cua click --x 500 --y 300
juno-cua click --x 500 --y 300 --button right
juno-cua click --x 500 --y 300 --button double

# Move mouse without clicking
juno-cua mouse-move --x 500 --y 300

# Get current cursor position
juno-cua cursor-position

# Scroll at a position
juno-cua scroll --x 500 --y 300 --direction down
juno-cua scroll --x 500 --y 300 --direction down --amount 5
```

### Keyboard

```bash
# Type text (simulates keystrokes)
juno-cua type-text --text "Hello, world!"

# Press a key with optional modifier
juno-cua press-key --key Return
juno-cua press-key --key c --modifier cmd    # Cmd+C (copy)
juno-cua press-key --key Tab
juno-cua press-key --key space

# Hold and release keys
juno-cua hold-key --key shift --duration-ms 500
juno-cua release-key --key shift
```

### System

```bash
# Clipboard
juno-cua get-clipboard
juno-cua set-clipboard --content "copied text"

# Launch apps / open URLs
juno-cua open-app --name "Safari"
juno-cua open-url --url "https://example.com"

# Wait (useful between UI actions)
juno-cua wait --ms 1000
```

### Advanced

```bash
# List all tools with full JSON schemas
juno-cua list-tools

# Generic tool call (for tools not exposed as subcommands)
juno-cua call --tool leftClick --args '{"x": 100, "y": 200}'

# Print capabilities catalog
juno-cua capabilities
```

## Output

All commands return JSON by default. Use `--format pretty` for human-readable output, or `--format quiet` for silent execution (exit code only).

```bash
# JSON (default)
juno-cua cursor-position
# → {"x":512,"y":384}

# Pretty-printed
juno-cua cursor-position --format pretty

# Silent — just check exit code
juno-cua click --x 100 --y 200 --format quiet && echo "clicked"
```

## Patterns

### Screenshot → Analyze → Act Loop

The most common pattern for GUI automation:

```bash
# 1. See what's on screen
juno-cua screenshot
# (analyze the base64 image to find UI elements)

# 2. Act on what you see
juno-cua click --x 340 --y 220

# 3. Wait for UI to update
juno-cua wait --ms 500

# 4. Verify the result
juno-cua screenshot
```

### Accessibility-First (Preferred)

When possible, use the accessibility tree instead of screenshots — it's faster and more reliable:

```bash
# Get structured UI info
juno-cua ui-tree --app "System Settings"

# Find specific elements
juno-cua find-elements --selector "AXButton"

# Check focused element after an action
juno-cua focused-element
```

### Form Filling

```bash
# Click the first field
juno-cua click --x 300 --y 200
juno-cua wait --ms 200

# Type and tab to next field
juno-cua type-text --text "John Doe"
juno-cua press-key --key Tab
juno-cua type-text --text "john@example.com"
juno-cua press-key --key Tab
juno-cua type-text --text "password123"

# Submit
juno-cua press-key --key Return
```

### App Navigation

```bash
# Open an app
juno-cua open-app --name "Finder"
juno-cua wait --ms 1000

# Use keyboard shortcuts
juno-cua press-key --key n --modifier cmd    # New window
juno-cua press-key --key g --modifier cmd    # Go to folder
juno-cua wait --ms 500
juno-cua type-text --text "/tmp"
juno-cua press-key --key Return
```

### Copy Text from GUI

```bash
# Select all and copy
juno-cua press-key --key a --modifier cmd
juno-cua press-key --key c --modifier cmd
juno-cua wait --ms 200

# Read clipboard
juno-cua get-clipboard
```

## Error Handling

All errors are returned as JSON on stderr with a non-zero exit code:

```json
{"error": "Screenshot failed: Check accessibility permissions."}
```

Common issues:
- **Accessibility permissions**: Grant in System Settings → Privacy & Security → Accessibility
- **App not found**: Check exact app name with `open-app`
- **Coordinates out of bounds**: Use `screenshot` first to find valid coordinates

## MCP Server (Alternative to CLI)

For agents that support MCP (Model Context Protocol), `juno-cua` can run as a structured tool server instead of being called via Bash. This gives typed tool schemas and native image content blocks for screenshots.

### Setup

Add to your `.mcp.json` (works in Claude Code, Cursor, Codex, Gemini CLI, etc.):

```json
{
  "mcpServers": {
    "juno": {
      "command": "juno-cua",
      "args": ["serve-mcp"]
    }
  }
}
```

The MCP server exposes the same tools as the CLI — screenshots return as MCP image content blocks automatically.

### When to use MCP vs CLI

- **MCP**: Best when your agent natively supports MCP tools. Gives typed schemas, streaming-friendly output, and image content blocks.
- **CLI (Bash)**: Works with any agent that can run shell commands. Zero config, universal compatibility.

Both use the same `juno-cua` binary and the same underlying `Desktop` engine.

## Full Juno Orchestrator

If Juno.app is installed, you also have access to the full multi-agent orchestrator:

```bash
# Natural language desktop automation (requires Juno.app)
juno query "Open Safari, navigate to github.com, and take a screenshot"
```

This routes through Juno's hierarchical agent system (Desktop Agent, Browser Agent, File Agent) for complex multi-step tasks. Install from https://github.com/lacymorrow/juno/releases.

### Full Juno MCP Server

The full Juno binary can also run as an MCP server, exposing everything `juno-cua` has PLUS the `query` tool:

```json
{
  "mcpServers": {
    "juno": {
      "command": "juno",
      "args": ["mcp", "serve"]
    }
  }
}
```

The `query` tool is the key difference — it delegates to the multi-agent orchestrator. No other MCP server offers a full AI orchestrator as a single tool call.

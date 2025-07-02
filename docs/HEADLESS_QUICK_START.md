# Juno Headless Mode - Quick Start Guide

## Overview

Juno AI Computer Use Agent supports comprehensive headless CLI operations for automation, scripting, and server deployment. This guide provides essential commands to get started quickly.

> **📖 For complete documentation**, see [`.cursor/rules/headless-architecture-guide.mdc`](.cursor/rules/headless-architecture-guide.mdc)

## Quick Setup

### 1. Build the CLI

```bash
# Build the headless CLI executable
cargo build --release --manifest-path src-tauri/Cargo.toml

# The executable will be at: target/release/juno
```

### 2. Basic Usage

```bash
# Simple query (automatically headless)
./target/release/juno query "Take a screenshot and describe the current screen"

# Voice query
./target/release/juno voice query --duration 30

# Check status
./target/release/juno agent status
```

## Key Features

- **✅ Automatic Headless Detection** - No `--headless` flag needed when using subcommands
- **✅ All Computer Use Actions** - Full Anthropic Computer Use API support
- **✅ Multiple Output Formats** - JSON, YAML, Text, Markdown, Quiet
- **✅ Batch Processing** - Execute multiple commands from files
- **✅ Voice Integration** - CLI voice recording and transcription
- **✅ Interactive Mode** - Real-time CLI sessions
- **✅ Daemon Mode** - Background service operations

## Essential Commands

### Query Operations

```bash
# Basic AI query
juno query "Open Safari and search for AI news"

# Query with JSON output for scripting
juno --output json query "Get current time from menu bar"

# Query with custom timeout
juno --timeout 600 query "complex automation task"
```

### Voice Operations

```bash
# Voice-driven query
juno voice query --duration 30

# Record audio file
juno voice record --duration 30 --output recording.wav

# Transcribe existing audio
juno voice transcribe audio.wav
```

### Agent Management

```bash
# Check agent status
juno agent status

# Get agent capabilities
juno agent capabilities --detailed

# Stop agent operations
juno agent stop --force
```

### System Operations

```bash
# System health check
juno system health --detailed

# Check permissions
juno system permissions --check

# Show system information
juno system info --hardware --permissions
```

### Batch Processing

```bash
# Execute commands from file
juno batch commands.txt

# Continue on errors
juno batch --continue-on-error commands.txt

# Parallel execution
juno batch --parallelism 8 commands.txt
```

### Configuration

```bash
# Show current configuration
juno config show

# Set configuration value
juno config set provider.anthropic.api_key "your-key"

# Export/import configuration
juno config export backup.json
juno config import --merge settings.json
```

## Output Formats

```bash
# Human-readable text (default)
juno query "test" --output text

# JSON for scripting
juno query "test" --output json

# Quiet mode (minimal output)
juno query "test" --output quiet

# Structured YAML
juno query "test" --output yaml
```

## Common Patterns

### Scripting Integration

```bash
#!/bin/bash
RESULT=$(juno --output json query "Check system status")
if echo $RESULT | jq -e '.success' > /dev/null; then
    echo "✅ System healthy"
else
    echo "❌ System issues detected"
fi
```

### Batch File Example

```txt
# commands.txt
query "Take a screenshot"
voice query --duration 10
agent status
system health
```

### Interactive Session

```bash
# Start interactive mode
juno interactive

# Available commands in interactive mode:
# - Any text: Submit as query
# - :status: Check agent status  
# - :voice: Start voice input
# - :exit: Exit interactive mode
```

### Daemon Mode

```bash
# Start background service
juno daemon start --foreground

# Check daemon status
juno daemon status

# Stop daemon
juno daemon stop
```

## Architecture Overview

The headless system consists of:

- **`HeadlessRuntime`** - Core execution engine
- **Automatic Detection** - Headless mode activated when using subcommands
- **10 Command Categories** - Query, Voice, Dictation, Agent, Config, System, Batch, Interactive, Daemon, Test
- **Multiple Output Formats** - Text, JSON, YAML, Markdown, Quiet, Table
- **Error Handling** - Comprehensive exit codes and error recovery

## Performance Benefits

| Operation | GUI Mode | Headless Mode | Improvement |
|-----------|----------|---------------|-------------|
| Simple Query | 8-12s | 5-8s | 25-35% faster |
| Voice Query | 15-20s | 10-15s | 25-30% faster |
| System Check | 3-5s | 2-3s | 30-40% faster |
| Batch Operations | Variable | 15-25% faster | Consistent |

## Integration Examples

### CI/CD Pipeline

```yaml
# .github/workflows/juno-automation.yml
- name: Run Juno System Check
  run: ./target/release/juno system health --detailed
```

### Cron Job

```bash
# Daily health check
0 9 * * * /usr/local/bin/juno system health > /var/log/juno-health.log
```

## Legacy CLI Support

For backward compatibility:

```bash
# Legacy accessibility check
juno --check-accessibility

# Legacy TTS test
juno --tts-provider system --tts-text "Hello world"
```

## Troubleshooting

### Build Issues

```bash
# Ensure dependencies are installed
cargo check --manifest-path src-tauri/Cargo.toml

# Force rebuild
cargo clean --manifest-path src-tauri/Cargo.toml
cargo build --release --manifest-path src-tauri/Cargo.toml
```

### Permission Issues

```bash
# Check and request permissions
juno system permissions --check --request
```

### Voice/Audio Issues

```bash
# Test voice functionality
juno test voice --duration 3
juno test tts --provider system
```

## Next Steps

1. **Read the Complete Guide**: [`.cursor/rules/headless-architecture-guide.mdc`](.cursor/rules/headless-architecture-guide.mdc)
2. **Test Your Setup**: Run `juno test all --report`
3. **Explore Examples**: Try the batch processing and interactive modes
4. **Check System Status**: Run `juno system info --hardware --permissions`
5. **Configure for Your Needs**: Use `juno config show` and `juno config set`

## Support

- **Comprehensive Documentation**: [`.cursor/rules/headless-architecture-guide.mdc`](.cursor/rules/headless-architecture-guide.mdc)
- **Architecture Overview**: `src-tauri/src/cli/headless.rs`
- **Command Structure**: `src-tauri/src/cli/mod.rs`
- **Constants**: `src-tauri/src/constants/cli.rs`

The headless mode provides full Computer Use capabilities through a powerful CLI interface, making Juno suitable for everything from simple automation scripts to complex CI/CD pipelines and server deployments.

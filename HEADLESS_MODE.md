# Juno Headless Mode

Juno AI Computer Use Agent supports comprehensive headless CLI operations for automation, scripting, and server deployment scenarios.

## Quick Start

```bash
# Build the CLI
cargo build --release --manifest-path src-tauri/Cargo.toml

# Run a query (automatically headless)
./target/release/juno query "Take a screenshot and describe what you see"

# Voice query
./target/release/juno voice query --duration 30

# Check status
./target/release/juno agent status
```

## Key Features

- ✅ **Automatic Headless Detection** - No `--headless` flag needed
- ✅ **All Computer Use Actions** - Full Anthropic Computer Use API support  
- ✅ **Multiple Output Formats** - JSON, YAML, Text, Markdown, Quiet
- ✅ **Batch Processing** - Execute commands from files
- ✅ **Voice Integration** - CLI voice recording and transcription
- ✅ **Interactive Mode** - Real-time CLI sessions
- ✅ **Daemon Mode** - Background service operations

## Documentation

- **📖 Quick Start Guide**: [`docs/HEADLESS_QUICK_START.md`](docs/HEADLESS_QUICK_START.md)
- **📚 Complete Architecture Guide**: [`.cursor/rules/headless-architecture-guide.mdc`](.cursor/rules/headless-architecture-guide.mdc)

## Available Commands

### Core Operations

- `juno query "text"` - Execute AI agent queries
- `juno voice query --duration 30` - Voice-driven queries
- `juno agent status` - Check agent status
- `juno system health` - System diagnostics

### Advanced Operations  

- `juno batch commands.txt` - Execute batch operations
- `juno interactive` - Interactive CLI session
- `juno daemon start` - Background service mode
- `juno config show` - Configuration management

### Output Formats

- `--output json` - For scripts and automation
- `--output quiet` - Minimal output for cron jobs
- `--output text` - Human-readable (default)

## Integration Examples

### CI/CD Pipeline

```yaml
- name: Run Juno Automation
  run: ./target/release/juno batch automation-commands.txt
```

### Script Integration

```bash
RESULT=$(juno --output json query "Check system status")
echo $RESULT | jq '.success'
```

### Cron Job

```bash
# Daily health check
0 9 * * * /usr/local/bin/juno system health > /var/log/juno.log
```

## Architecture

The headless system provides:

- **HeadlessRuntime** - Core execution engine in `src-tauri/src/cli/headless.rs`
- **10 Command Categories** - Comprehensive CLI structure in `src-tauri/src/cli/mod.rs`
- **Auto-Detection Logic** - Smart headless mode activation in `src-tauri/src/startup.rs`
- **Performance Benefits** - 25-35% faster than GUI mode for most operations

For complete documentation, examples, and advanced usage patterns, see the comprehensive guides linked above.

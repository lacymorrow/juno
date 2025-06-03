# Juno - AI Computer Use Agent

Juno is a Tauri v2 application with Anthropic Computer Use Bot capabilities for macOS automation.

**⚡ Quick Start**: `bun install` → `cp .env.example .env` → `bun run tauri dev`

## 📚 Documentation

**For comprehensive information, see [`docs/`](docs/) directory:**

- **[Getting Started](docs/README.md)** - Overview and quick reference
- **[Architecture](docs/architecture.md)** - System design and components  
- **[API Reference](docs/api-reference.md)** - All commands and signatures
- **[Agent System](docs/agent-system.md)** - AI agent architecture and tools
- **[Development Guide](docs/development.md)** - Setup, testing, contribution
- **[Configuration](docs/configuration.md)** - Environment variables and settings
- **[Troubleshooting](docs/troubleshooting.md)** - Common issues and solutions

## 🚀 Quick Setup

### Prerequisites
- Node.js 18+ and Bun
- Rust 1.70+ and Cargo  
- Tauri CLI v2

### Installation
```bash
# 1. Install dependencies
bun install

# 2. Setup environment
cp .env.example .env
# Edit .env with your API keys (see Configuration docs)

# 3. Development
bun run tauri dev

# 4. Testing
./run-all-tests.sh
```

### Required API Keys
- `ANTHROPIC_API_KEY` - Primary AI provider
- `OPENAI_API_KEY` - Alternative AI provider
- `ELEVENLABS_API_KEY` - Text-to-speech (optional)

See [Configuration](docs/configuration.md) for complete API key list.

## 🔧 Development

**After every Rust change**: `cargo check --manifest-path src-tauri/Cargo.toml`

## 🤖 For LLMs

This project includes [`llms.txt`](llms.txt) with optimized instructions for AI agents working with the codebase.

## 📖 Legacy Documentation

- `agent-roadmap.md` - Development roadmap
- `implementation-plan.md` - Current implementation status  
- `TESTING.md` - Testing procedures

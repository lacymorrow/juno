# Juno - AI Computer Use Agent ✅ COMPLETE

**Juno is a production-ready Tauri v2 application with COMPLETE Anthropic Computer Use Bot implementation for macOS automation.**

✅ **All 17 Computer Use actions implemented**  
✅ **Full macOS platform support**  
✅ **Timer system for long-running tasks**  
✅ **Multi-agent architecture**  
✅ **Voice integration & browser automation**

**⚡ Quick Start**: `bun install` → `cp .env.example .env` → `bun run tauri dev`

## 🎯 Implementation Status: COMPLETE

### ✅ Official Anthropic Computer Use Tools (100% Complete)
- **computer_20250124**: All 17 actions (screenshot, mouse, keyboard, scroll, wait)
- **str_replace_based_edit_tool**: Complete file operations (view, create, edit, insert)  
- **bash_20250124**: Shell command execution with full output capture
- **Fully compliant** with Claude 4, Sonnet 3.7, and Sonnet 3.5 specifications

### ✅ Complete macOS Platform Implementation  
- **Mouse Operations**: All click types, drag, move, position detection
- **Keyboard Operations**: Text input, key combinations, hold/release
- **Advanced Features**: 4-direction scrolling, clipboard ops, window management
- **Visual Processing**: Screenshot capture, accessibility tree navigation
- **Robust Clicking**: Multi-method with intelligent fallback strategies

### ✅ Enhanced Features Beyond Official Spec
- **🕐 Timer System**: Long-running task management with automatic context resumption
- **🤖 Multi-Agent Architecture**: Specialized agents (browser, coding, desktop, general)
- **🎤 Voice Integration**: Speech recognition and transcription
- **🌐 Browser Automation**: Advanced web interaction capabilities
- **⚙️ Multi-Provider Support**: Anthropic, OpenAI, Gemini integration

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
- `ANTHROPIC_API_KEY` - Primary AI provider for Computer Use
- `OPENAI_API_KEY` - Alternative AI provider
- `GOOGLE_GEMINI_API_KEY` - Gemini models support
- `ELEVENLABS_API_KEY` - Text-to-speech (optional)

See [Configuration](docs/configuration.md) for complete API key list.

## 🏗️ Architecture Overview

- **Frontend**: React/TypeScript with floating bar UI
- **Backend**: Rust with Tauri v2 framework  
- **Agent System**: Multi-provider AI support with tool integration
- **Desktop Control**: Native macOS APIs with accessibility permissions
- **Timer System**: Context-preserving task scheduling and resumption

## 🔧 Development

**Status**: ✅ Project compiles successfully (`cargo check` exit code 0)

**After every Rust change**: `cargo check --manifest-path src-tauri/Cargo.toml`

## 🔑 Key Implementation Files

- [`src-tauri/src/agent/tools/anthropic_computer_use.rs`](src-tauri/src/agent/tools/anthropic_computer_use.rs) - Official Anthropic Computer Use tools
- [`src-tauri/src/agent/tools/timer_tools.rs`](src-tauri/src/agent/tools/timer_tools.rs) - Timer system for long-running tasks
- [`src-tauri/mcp-server-os-level/src/platforms/macos/`](src-tauri/mcp-server-os-level/src/platforms/macos/) - Complete macOS automation
- [`src-tauri/src/agent/providers/factory.rs`](src-tauri/src/agent/providers/factory.rs) - AI provider management

## 🤖 For LLMs

This project includes [`llms.txt`](llms.txt) with optimized instructions for AI agents working with the codebase.

## 📖 Legacy Documentation

- `agent-roadmap.md` - Development roadmap
- `implementation-plan.md` - Current implementation status  
- `TESTING.md` - Testing procedures

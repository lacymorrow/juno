# Juno - AI Computer Use Agent ✅

**Production-ready Tauri v2 application with COMPLETE Anthropic Computer Use Bot implementation for macOS automation.**

[![Status](https://img.shields.io/badge/Status-Production%20Ready-green)]()
[![Platform](https://img.shields.io/badge/Platform-macOS-blue)]()
[![Architecture](https://img.shields.io/badge/Architecture-Multi--Agent-purple)]()

## ⚡ Quick Start

```bash
# Install and setup
bun install && cp .env.example .env
# Add your API keys to .env
bun run tauri dev
```

## 🎯 Implementation Status

✅ **All 17 Computer Use actions** (screenshot, mouse, keyboard, scroll, wait)  
✅ **Complete macOS platform support** with accessibility APIs  
✅ **Multi-agent architecture** with intelligent task delegation  
✅ **Voice integration** with dual modes (Agent/Dictation)  
✅ **Timer system** for long-running tasks with context resumption  
✅ **Browser automation** and advanced web interaction

## 🏗️ Architecture

- **Frontend**: React/TypeScript floating bar + chat interface
- **Backend**: Rust with Tauri v2 framework
- **Agent System**: Hierarchical orchestrator + specialized agents
- **Voice**: Custom Whisper.cpp-based transcription plugin
- **Platform**: Native macOS APIs with full automation capabilities

## 🔑 Required API Keys

```env
ANTHROPIC_API_KEY=your_key_here    # Primary AI provider
OPENAI_API_KEY=your_key_here       # Alternative provider
ELEVENLABS_API_KEY=your_key_here   # Text-to-speech (optional)
```

## 🚀 Development

**Critical**: Run `cargo check --manifest-path src-tauri/Cargo.toml` after every Rust change.

```bash
./run-all-tests.sh    # Full test suite
bun run tauri dev     # Development mode
```

## � For Developers & LLMs

- **[LLMs.txt](LLMs.txt)** - Complete instructions for AI agents working with this codebase
- **Key Files**: 
  - `src-tauri/src/anthropic.rs` - Main orchestrator agent
  - `src-tauri/src/agent/tools/anthropic_computer_use.rs` - Computer Use tools
  - `src-tauri/mcp-server-os-level/src/platforms/macos/` - macOS automation
  - `tauri-plugin-voice-transcription/` - Voice processing

## 🎤 Voice Modes

- **Agent Mode**: Alt+D → Voice → AI Processing → Computer Actions
- **Dictation Mode**: Configurable key → Voice → Direct text insertion

## � System Requirements

- macOS with accessibility permissions
- Node.js 18+ and Rust 1.70+
- Microphone access for voice features
- Screen recording permissions for screenshots

---

**This implementation exceeds Anthropic's official Computer Use specification and provides a production-ready AI desktop automation system.**

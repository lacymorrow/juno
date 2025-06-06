# Juno AI Computer Use Agent - Documentation

**Purpose**: Tauri v2 desktop app with Anthropic Computer Use Bot for macOS automation  
**Architecture**: Rust backend + React/TypeScript frontend  
**Key Features**: Desktop automation, browser control, voice transcription, multi-agent orchestration

## Quick Reference

### Core Components
- **Agent System**: `src-tauri/src/agent/` - AI agent implementations and tools
- **Desktop Control**: `src-tauri/src/commands/` - System automation commands  
- **Browser Integration**: `src-tauri/src/tools/browser_tools.rs` - Web automation
- **Voice System**: `tauri-plugin-voice-transcription/` - Speech recognition
- **Frontend**: `src/` - React UI with floating bar interface

### Entry Points
- **Main Agent**: `src-tauri/src/anthropic.rs::submit_query()`
- **Multi-Agent**: `src-tauri/src/commands/orchestrator.rs::submit_orchestrated_query()`
- **Desktop Tools**: `src-tauri/src/commands/*.rs`

## Documentation Structure

1. **[Architecture Overview](architecture.md)** - System design and data flow
2. **[API Reference](api-reference.md)** - All commands and their signatures
3. **[Agent System](agent-system.md)** - AI agent architecture and tools
4. **[Enhanced Timer System](enhanced-timer-system.md)** - Agent pause/resume with monitoring capabilities
5. **[Development Guide](development.md)** - Setup, testing, and contribution
6. **[Configuration](configuration.md)** - Environment variables and settings
7. **[Troubleshooting](troubleshooting.md)** - Common issues and solutions

## Key Concepts

**Agent Execution Flow**:
1. Query received via Tauri command
2. Escape key registered for cancellation
3. Agent brain initialized (BrainFactory)
4. Tools registered (desktop, browser, basic)
5. Agent runs with max 15 iterations
6. Escape key unregistered on completion

**Tool System**:
- **Desktop Tools**: Click, type, screenshot, window management
- **Browser Tools**: Navigate, extract content, interact with pages
- **Basic Tools**: File operations, shell commands
- **Voice Tools**: Transcription and TTS
- **Timer Tools**: Agent pause/resume with screen/file/app monitoring

**State Management**:
- **AppState**: Global application state with cancellation signals
- **Memory**: Agent conversation history and context
- **Browser**: Lazy-initialized browser controller

## Quick Setup

```bash
# Install dependencies
bun install

# Setup environment
cp .env.example .env
# Edit .env with API keys

# Development
bun run tauri dev

# Testing
bun run test
./test-rust-units.sh
```

## API Keys Required
- `ANTHROPIC_API_KEY` - Main AI provider
- `OPENAI_API_KEY` - Alternative AI provider  
- `ELEVENLABS_API_KEY` - Text-to-speech
- Additional providers: Gemini, Perplexity, HuggingFace, Replicate, FAL.ai 

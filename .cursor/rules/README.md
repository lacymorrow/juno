# Juno AI Computer Use Agent - Cursor Rules

This directory contains consolidated, concise cursor rules for the Juno AI Computer Use Agent project.

## Essential Rules

### [01-project-essentials.mdc](01-project-essentials.mdc)
Project overview, tech stack, structure, and key development requirements.

### [02-voice-and-ai.mdc](02-voice-and-ai.mdc)
Voice transcription system, AI agent architecture, and tool integration.

### [03-development-guide.mdc](03-development-guide.mdc)
Tauri development patterns, security requirements, testing, and workflow.

## Quick Reference

- **Always run**: `cargo check --manifest-path src-tauri/Cargo.toml` after Rust changes
- **Tech Stack**: Tauri v2, Rust backend, React/TypeScript frontend
- **Voice**: Custom plugin with Whisper.cpp, Alt+D to toggle
- **AI**: Multi-agent system with specialized Browser/Desktop/System agents
- **Security**: File operations need sandboxing (see TODOs in basic_tools.rs)

## Development Workflow

1. Make changes file-by-file
2. Run cargo check after Rust modifications
3. Test voice dictation: Alt+D → speak → verify transcription → check AI response
4. Use DevToolsPanel for manual testing
5. Keep files under 700 lines

This consolidated structure eliminates redundancy while maintaining essential guidance. 

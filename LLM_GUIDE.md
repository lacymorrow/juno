# 🤖 LLM Navigation Guide for Juno (dotdot)

## 🎯 Quick Navigation

**You are working with Juno** - An AI Computer Use Agent built with Tauri v2 and Rust.

### 📍 Where to Start
- **New to project?** → Start here, then [README.md](README.md)
- **Understanding architecture?** → [ARCHITECTURE.md](ARCHITECTURE.md) 
- **Making changes?** → [LLMs.txt](LLMs.txt) for comprehensive instructions
- **Claude Flow swarms?** → [CLAUDE.md](CLAUDE.md)

### 🗂️ Project Structure Map

```
dotdot/                      # Root directory (Juno project)
├── src-tauri/              # Rust backend (main logic)
│   ├── src/
│   │   ├── agent/          # AI agent system 
│   │   ├── commands/       # Tauri commands (50+ commands)
│   │   ├── cloud/          # Cloud connector
│   │   └── anthropic.rs    # Main orchestrator
│   └── Cargo.toml         # Rust dependencies
├── src/                    # React frontend
│   ├── components/         # UI components
│   ├── hooks/             # React hooks
│   └── App.tsx            # Main app component
├── docs/                   # Documentation
│   └── rules/             # Development guidelines
└── tauri-plugin-voice-transcription/  # Voice system
```

## 🔧 Common LLM Tasks

### 1. **Understanding the System**
```bash
# Read these files in order:
1. README.md           # Project overview
2. ARCHITECTURE.md     # System design
3. LLMs.txt           # Development rules
```

### 2. **Finding Code**
```bash
# Agent system
src-tauri/src/agent/        # All AI agent code
src-tauri/src/anthropic.rs  # Main orchestrator

# Commands/API
src-tauri/src/commands/     # All Tauri commands
src-tauri/src/commands/registry.rs  # Command list

# Frontend
src/App.tsx                 # Main UI
src/components/             # React components
```

### 3. **Making Changes**
```bash
# ALWAYS run after Rust changes:
cargo check --manifest-path src-tauri/Cargo.toml

# Development
bun run tauri dev

# Testing
./run-all-tests.sh
```

## 📋 Key Files Reference

### Core System Files
| File | Purpose | When to Read |
|------|---------|--------------|
| `src-tauri/src/anthropic.rs` | Main AI orchestrator | Understanding agent flow |
| `src-tauri/src/agent/core.rs` | Agent implementation | Modifying agent behavior |
| `src-tauri/src/commands/registry.rs` | All available commands | Finding API endpoints |
| `src-tauri/src/state.rs` | Application state | Understanding data flow |

### Configuration Files
| File | Purpose | When to Read |
|------|---------|--------------|
| `.env.example` | Required API keys | Initial setup |
| `src-tauri/tauri.conf.json` | App configuration | Build/permissions issues |
| `src-tauri/Cargo.toml` | Rust dependencies | Adding features |
| `package.json` | Frontend dependencies | UI changes |

### Documentation Files
| File | Purpose | When to Read |
|------|---------|--------------|
| `LLMs.txt` | Comprehensive dev guide | Before any changes |
| `ARCHITECTURE.md` | System design | Understanding flow |
| `API.md` | API reference | Using commands |
| `docs/rules/INDEX.md` | All documentation | Finding specific docs |

## 🚀 Quick Code Patterns

### Adding a New Command
```rust
// 1. Add to src-tauri/src/commands/your_module.rs
#[tauri::command]
pub async fn your_command(state: State<'_, AppState>) -> Result<String, String> {
    // Implementation
}

// 2. Register in src-tauri/src/commands/registry.rs
.invoke_handler(tauri::generate_handler![
    // ... existing commands ...
    your_module::your_command,
])
```

### Adding a Tool
```rust
// In src-tauri/src/agent/tools/your_tool.rs
pub fn register_tools(provider: &mut LocalToolProvider) {
    let tool = ToolDefinition {
        name: "tool_name".to_string(),
        description: "What it does".to_string(),
        input_schema: // ... schema
    };
    provider.register_tool(tool, |input| async move {
        // Implementation
    });
}
```

### Frontend Component
```typescript
// In src/components/YourComponent.tsx
import { invoke } from "@tauri-apps/api/core";

export const YourComponent = () => {
    const handleAction = async () => {
        const result = await invoke('your_command', { 
            param: value 
        });
    };
    // Component JSX
};
```

## ⚠️ Critical Rules

### 🔴 MANDATORY
1. **ALWAYS** run `cargo check` after Rust changes
2. **NEVER** use `std::process::exit()` - use `AgentError`
3. **NEVER** use `tokio::spawn()` in event listeners - use `tauri::async_runtime::spawn()`
4. **ALWAYS** use Tauri store for config - no direct file I/O

### 🟡 Important Patterns
- Use `Arc<TokioMutex<T>>` for shared state
- Follow async/await patterns consistently
- All tools need security validation
- Test both dev and production modes

## 🔍 Finding Information

### By Feature
- **Voice System** → `tauri-plugin-voice-transcription/`
- **Cloud/Remote** → `src-tauri/src/cloud/`
- **Browser Automation** → `src-tauri/src/agent/tools/browser_tools.rs`
- **Permissions** → `src-tauri/src/commands/permissions.rs`
- **Settings** → `src/components/settings/`

### By Technology
- **Rust Backend** → `src-tauri/`
- **React Frontend** → `src/`
- **Voice/Whisper** → `tauri-plugin-voice-transcription/`
- **Documentation** → `docs/`

### By Task Type
- **Adding Features** → Start with `src-tauri/src/commands/`
- **Fixing Bugs** → Check `docs/fixes/` and error handling
- **UI Changes** → `src/components/` and `src/App.tsx`
- **Agent Behavior** → `src-tauri/src/agent/`

## 📊 Project Stats
- **Language**: Rust (backend) + TypeScript (frontend)
- **Framework**: Tauri v2
- **Commands**: 50+ categorized commands
- **Agents**: Hierarchical multi-agent system
- **Platform**: macOS (primary), Windows/Linux (planned)

## 🆘 Troubleshooting

### Common Issues
1. **Compilation Error** → Run `cargo check` first
2. **Permission Error** → Check `src-tauri/Info.plist` and entitlements
3. **Voice Not Working** → Check microphone permissions
4. **Agent Not Responding** → Check API keys in settings

### Debug Commands
```bash
# Enable debug logging
RUST_LOG=debug bun run tauri dev

# Check for string error patterns
./scripts/detect-string-error-patterns.sh

# Test permissions
cargo test permission --manifest-path src-tauri/Cargo.toml
```

## 📚 Next Steps

1. **Read** [README.md](README.md) for project overview
2. **Study** [ARCHITECTURE.md](ARCHITECTURE.md) for system design
3. **Follow** [LLMs.txt](LLMs.txt) for development rules
4. **Explore** code starting from `src-tauri/src/anthropic.rs`

---

*This guide is optimized for LLM comprehension. For detailed instructions, see [LLMs.txt](LLMs.txt).*
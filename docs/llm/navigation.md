# 📍 LLM File Navigation Map

## 🎯 Purpose of Each File

### Root Directory Files (Key Documentation)

| File | Purpose | Read When |
|------|---------|-----------|
| **README.md** | Project overview, quick start, feature list | Starting with project |
| **LLM_GUIDE.md** | LLM-specific navigation and task guide | LLM agents start here |
| **ARCHITECTURE.md** | System design, component overview | Understanding system |
| **LLMs.txt** | Comprehensive development instructions | Before making changes |
| **CLAUDE.md** | Claude Flow swarm configuration | Using swarm features |
| **API.md** | Runtime API reference | Implementing features |
| **DEVELOPMENT.md** | Development patterns and guidelines | Writing new code |
| **ROADMAP.md** | Future plans and features | Planning work |

### Implementation Documentation (Move to docs/implementation/)

These files document specific implementations and should be moved to keep root clean:

| File Pattern | Content | New Location |
|--------------|---------|--------------|
| *_IMPLEMENTATION_*.md | Feature implementations | docs/implementation/features/ |
| *_FIX_*.md | Bug fixes and solutions | docs/fixes/ |
| *_SUMMARY.md | Implementation summaries | docs/implementation/summaries/ |
| *_ANALYSIS.md | Technical analysis | docs/implementation/analysis/ |

### Source Code Directories

#### Backend (Rust) - `src-tauri/`

| Directory | Purpose | Key Files |
|-----------|---------|-----------|
| **src/agent/** | AI agent system | core.rs, tools/, providers/ |
| **src/commands/** | Tauri API commands | registry.rs (command list) |
| **src/cloud/** | Cloud connectivity | connector.rs |
| **src/tts/** | Text-to-speech | system.rs, elevenlabs.rs |
| **src/** | Core application | anthropic.rs, state.rs, main.rs |

#### Frontend (React/TypeScript) - `src/`

| Directory | Purpose | Key Files |
|-----------|---------|-----------|
| **components/** | React UI components | FloatingBar.tsx, settings/ |
| **hooks/** | Custom React hooks | useSettings.ts, useConversation.ts |
| **lib/** | Utilities and helpers | ui-api.ts, constants.generated.ts |
| **types/** | TypeScript types | All type definitions |

#### Voice Plugin - `tauri-plugin-voice-transcription/`

| Directory | Purpose | Key Files |
|-----------|---------|-----------|
| **src/** | Voice transcription | controller.rs, always_listening.rs |
| **api/** | TypeScript bindings | index.ts |
| **models/** | Whisper models | ggml-tiny.en.bin |

### Configuration Files

| File | Purpose | Location |
|------|---------|----------|
| **.env.example** | API key template | Root |
| **tauri.conf.json** | Tauri app config | src-tauri/ |
| **Cargo.toml** | Rust dependencies | src-tauri/ |
| **package.json** | Node dependencies | Root |
| **tsconfig.json** | TypeScript config | Root |

### Documentation Structure - `docs/`

| Directory | Contents | Purpose |
|-----------|----------|---------|
| **rules/** | Development guidelines | Best practices and patterns |
| **fixes/** | Bug fix documentation | Historical fixes and solutions |
| **tars-integration/** | TARS migration docs | Event-driven architecture |
| **bug-tracking/** | Known issues | Bug tracking and prevention |

## 🔍 Quick File Finder

### "I need to..."

#### Understand the System
1. Start: `LLM_GUIDE.md`
2. Then: `README.md`
3. Deep dive: `ARCHITECTURE.md`
4. Details: `LLMs.txt`

#### Find API Commands
- Command list: `src-tauri/src/commands/registry.rs`
- Command implementations: `src-tauri/src/commands/*.rs`
- Frontend usage: `src/lib/ui-api.ts`

#### Modify Agent Behavior
- Main orchestrator: `src-tauri/src/anthropic.rs`
- Agent core: `src-tauri/src/agent/core.rs`
- Tools: `src-tauri/src/agent/tools/*.rs`
- Prompts: `src-tauri/src/agent/prompts/`

#### Work with Voice
- Plugin: `tauri-plugin-voice-transcription/src/`
- Commands: `src-tauri/src/commands/dictation.rs`
- Always listening: `src-tauri/src/commands/always_listening.rs`

#### Update UI
- Main app: `src/App.tsx`
- Components: `src/components/`
- Settings: `src/components/settings/`
- Styles: `src/styles/`

#### Fix Permissions
- macOS: `src-tauri/Info.plist`
- Entitlements: `src-tauri/juno.entitlements`
- Commands: `src-tauri/src/commands/permissions.rs`

#### Add Features
1. Define command: `src-tauri/src/commands/your_feature.rs`
2. Register: `src-tauri/src/commands/registry.rs`
3. Frontend: Create UI in `src/components/`
4. Invoke: Use `invoke()` from `@tauri-apps/api/core`

## 📊 File Statistics

- **Total Rust files**: ~150+
- **Total TypeScript files**: ~100+
- **Documentation files**: ~80+
- **Configuration files**: ~20
- **Test files**: Throughout codebase

## 🚀 Navigation Tips

1. **Use `Grep` tool** to find specific functionality
2. **Start from registry.rs** to understand available commands
3. **Follow imports** to understand dependencies
4. **Check test files** for usage examples
5. **Read error types** to understand failure modes

---

*This navigation map helps LLMs quickly find relevant files. For detailed documentation, start with [LLM_GUIDE.md](../../LLM_GUIDE.md).*
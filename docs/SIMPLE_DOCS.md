# Juno – Simple Docs

What this is

- Minimal quick start and where-things-live
- Links to canonical, verified docs

Quick Start

```bash
bun install
cp .env.example .env   # add keys
bun run tauri dev
```

Keys

- `ANTHROPIC_API_KEY` (required)
- Optional: `OPENAI_API_KEY`, `ELEVENLABS_API_KEY`

Daily Dev

```bash
RUST_LOG=debug bun run tauri dev
cargo check --manifest-path src-tauri/Cargo.toml --message-format=short 2>&1 | tee cargo-check-results.log
```

Where things live

- App entry: `src-tauri/src/anthropic.rs`
- Tools: `src-tauri/src/agent/tools/`
- Frontend: `src/`
- Rules for agents/UI: `docs/rules/INDEX.md` and `LLMs.txt`

Do/Don’t

- Do use generated constants (`src/lib/constants.generated.ts`)
- Do verify backend commands/events before wiring UI
- Don’t duplicate constants; don’t assume APIs that don’t exist

Canonical Docs

- Agent rules index: docs/rules/INDEX.md
- AI agent guidance: LLMs.txt
- Consolidated guide: docs/CONSOLIDATED_DOCUMENTATION.md (for deep dives)

Next steps

- This is Phase 1. Full consolidation will merge/retire legacy root docs.



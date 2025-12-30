# Juno - AI Computer Use Agent

Production-ready Tauri v2 desktop app with Anthropic Computer Use integration for macOS automation.

All non-entry documentation lives under `docs/` to avoid duplication.

## Quick Start

See `docs/SIMPLE_DOCS.md` for the minimal quick start and canonical links.

```bash
bun install && cp .env.example .env
RUST_LOG=debug cargo check --manifest-path src-tauri/Cargo.toml --message-format=short 2>&1 | tee cargo-check-results.log
bun run tauri dev
```

For deep docs, see `docs/CONSOLIDATED_DOCUMENTATION.md` (under verification). For AI agent guidance, read `LLMs.txt`. For rules, see `docs/rules/INDEX.md`.

## Testing & Automation

- Scenario matrix and automation guidance lives in [`docs/rules/ui_test_scenarios.mdc`](docs/rules/ui_test_scenarios.mdc). Follow it to script unit/integration checks, UI automation, and LLM-driven smoke runs.

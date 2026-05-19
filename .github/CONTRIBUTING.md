# Contributing to Juno

Thanks for your interest in Juno. This project is a native macOS desktop app built on Tauri v2 (Rust backend) + Vite/React (TypeScript frontend), with a custom Whisper voice plugin and a `juno-cua` CLI for AI-agent integration.

## Quick start

Prerequisites: **Rust** (latest stable), **Bun**, **macOS 14+**, Xcode CLT.

```bash
git clone https://github.com/lacymorrow/juno.git
cd juno
bun install
cp .env.example .env

# Run the full app (Tauri shell + Vite frontend)
bun run tauri:dev

# Or, frontend-only (Vite on :1420)
bun run dev
```

See [`CLAUDE.md`](../CLAUDE.md) for the full architecture guide, including the multi-agent orchestrator, MCP server layout, and platform-specific crates.

## Asking questions

Have a usage question, want to share what you built with Juno, or trade ideas with other users? Start a thread in [**Discussions**](https://github.com/lacymorrow/juno/discussions) — that's the right place for anything that isn't a confirmed bug or feature request.

## Reporting bugs

Open an issue using the **Bug Report** template. Include your Juno version (Juno › About), macOS version, exact steps to reproduce, and any Console.app logs filtered to `Juno`. If you're unsure whether something is a bug, ask in Discussions first.

## Proposing changes

For anything non-trivial (new command, agent tool, UI surface, public API), please open an issue first to discuss the approach. Small fixes (typos, docs, single-file bugs) can go straight to a PR.

## Pull requests

1. Branch from `main`. Use a descriptive name (`fix/voice-deadlock`, `feat/mcp-resources`).
2. Keep PRs focused — one logical change per PR.
3. Follow Conventional Commits (`feat:`, `fix:`, `chore:`, `docs:`, `refactor:`, `perf:`, `test:`).
4. Fill in the PR template — especially the test plan.

## Code style

**Rust:**
- Run `cargo check --manifest-path src-tauri/Cargo.toml` after every Rust change. Mandatory, 15 min timeout.
- Zero `.unwrap()` in production paths — use `?` or proper error handling.
- Use `tauri::async_runtime::spawn()`, not `tokio::spawn()`.
- Run `cargo fmt` and `cargo clippy` before pushing.

**TypeScript / React:**
- Strict TypeScript. No `any` without a comment explaining why.
- Server-side logic belongs in Rust, not the frontend.
- Run `bun run typecheck` before pushing.

**General:**
- Files should stay under 500 lines where possible.
- No new dependencies without justification — Juno aims to stay lean.

## Testing

```bash
bun test                                            # Frontend (Vitest)
cargo test --manifest-path src-tauri/Cargo.toml     # Backend
```

If you touch the Rust backend, both suites must pass.

## Security

Please **do not open public issues for security vulnerabilities**. See [SECURITY.md](SECURITY.md) for private disclosure instructions.

## License

By contributing, you agree that your contributions will be licensed under the same [FSL-1.1-MIT](../LICENSE) license that covers the project.

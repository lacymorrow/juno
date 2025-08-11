# Architecture Overview (Canonical)

This is the canonical location for the architecture overview. For a minimal entry point, see `../SIMPLE_DOCS.md`.

---

# Architecture Overview

## System Design

Juno is a production-ready Tauri v2 desktop application implementing a complete AI Computer Use agent with hierarchical architecture and advanced voice integration.

```
┌─────────────────┐    ┌─────────────────┐    ┌─────────────────┐
│   Frontend      │    │   Backend       │    │   Platform      │
│   React/TS      │◄──►│   Rust/Tauri    │◄──►│   macOS APIs    │
│   - Floating UI │    │   - Agents      │    │   - Automation  │
│   - Chat        │    │   - Tools       │    │   - Voice       │
│   - Settings    │    │   - State       │    │   - Browser     │
└─────────────────┘    └─────────────────┘    └─────────────────┘
```

## Hierarchical Agent System ✅

(Full details from the original `ARCHITECTURE.md` will be migrated here in this consolidation phase.)



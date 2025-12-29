# Juno AI Documentation

Welcome to the comprehensive documentation for the Juno AI application.

## 📚 Documentation Structure

### [Backend Documentation](./backend/README.md)
The Rust-based core of the application, handling system interactions, AI orchestration, and cloud connectivity.
- **[Architecture](./backend/architecture.md)**: System design, startup flow, and state management.
- **[Agents & AI](./backend/agents.md)**: The Agent Orchestrator, Tool use, and Model Context Protocol (MCP).
- **[Voice Plugin](./backend/voice_plugin.md)**: Deep dive into the custom `tauri-plugin-voice-transcription` crate.
- **[Commands API](./backend/commands.md)**: Reference for all Tauri IPC commands available to the frontend.

### [Frontend Documentation](./frontend/README.md)
The TypeScript/React UI, handling user interaction, state, and visualization.
- **[Architecture](./frontend/architecture.md)**: Application structure, providers, and entry points.
- **[State Management](./frontend/state_management.md)**: Contexts, Hooks, and Stores (Zustand/Context).
- **[Event Flow](./frontend/event_flow.md)**: Detailed diagrams of the event-driven architecture.
- **[Components](./frontend/components.md)**: Core component breakdown (FloatingBar, Permissions, etc.).

## 🗄️ Legacy Documentation
Older documentation files have been moved to the [`legacy/`](./legacy/) directory for reference.

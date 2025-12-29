# Event Flow & Data Architecture

Juno interactions are circular: Frontend -> Backend -> Frontend.

## Core Interaction Loop

```mermaid
sequenceDiagram
    participant User
    participant UI as React Frontend
    participant Rust as Tauri Backend
    participant AI as Agent/LLM

    User->>UI: Types "Calculate 5+5"
    UI->>Rust: invoke("submit_query", { text: "Calculate 5+5" })
    Rust->>UI: emit("BAR_STATE_UPDATE", { state: "thinking" })
    Rust->>AI: Prompts Agent
    AI->>Rust: Tool Call (Calculator)
    Rust->>UI: emit("AGENT_EVENT", { type: "tool_use", tool: "calculator" })
    Rust->>Rust: Executes 5+5
    Rust->>AI: Returns 10
    AI->>Rust: Final Answer "It's 10"
    Rust->>UI: emit("STREAMING_TEXT_STREAM", "It's")
    Rust->>UI: emit("STREAMING_TEXT_STREAM", " 10")
    Rust->>UI: emit("SYSTEM_BACKEND_RESPONSE", { done: true })
    UI->>UI: Updates Conversation State
```

## Voice Flow

```mermaid
graph TD
    User(User Speaks) -->|Audio Input| Plugin[Voice Plugin]
    Plugin -->|Partial Result| Backend[Rust Event Bus]
    Backend -->|VOICE_TRANSCRIPTION_PARTIAL_RESULT| React[VoiceContext]
    React -->|State Update| FloatingBar[Floating Bar UI]
    FloatingBar -->|Visual Feedback| User
```

## Key Event Handlers
- **`useBackendEvents.ts`**: The "Router" for incoming events. It contains a massive switch/case (or individual `useEffect` hooks) mapping event names to state setters.
- **`App.tsx`**: Listens for `WINDOW_FOCUS` or `SHORTCUT_TRIGGERED` to bring the app to the foreground.

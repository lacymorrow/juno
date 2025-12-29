# State Management

Juno uses a decentralized state model optimized for separate windows and event-driven updates.

## Hooks (`src/hooks/`)

### `useAppState.ts`
The primary store for UI state that doesn't belong in a specific context.
- **Implementation**: Uses a custom singleton pattern or React State lifted to `App.tsx`.
- **Tracks**:
  - `view`: 'chat' | 'settings' | 'permissions'
  - `isProcessing`: boolean (Loading state)
  - `serverStatus`: 'connected' | 'disconnected' | 'error'

### `useConversation.ts`
Manages the chat array `Message[]`.
- **Optimization**: Implements `pruneConversationIfNeeded` to keep the DOM light (max ~50 messages).
- **Persistence**: Messages are primarily ephemeral in RAM, but specialized "Saved" messages can be persisted via backend calls.

## Contexts (`src/contexts/`)

### `VoiceContext.tsx`
Dedicated provider for Voice and Agent feedback loops. This is separated to prevent re-renders of the main chat when high-frequency audio levels change.
- **State**:
  - `transcript`: Real-time string from `VOICE_TRANSCRIPTION_PARTIAL_RESULT`.
  - `audioLevel`: Float (0.0 - 1.0) for visualizer.
  - `agentState`: 'listening' | 'thinking' | 'speaking'.

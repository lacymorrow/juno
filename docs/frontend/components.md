# Core Components

### `FloatingBar.tsx`
The signature UI of Juno.
- **Design**: compact, pill-shaped window.
- **Props**: None (Self-contained, subscribed to `useAppState` and `VoiceContext`).
- **Variants**:
  - `Input`: Standard text input.
  - `Thinking`: Animated pulse loader.
  - `Voice`: Waveform visualizer.

### `PermissionsManager.tsx`
Handles the intricate macOS permission flow.
- **Detection**: Uses `setInterval` to poll backend permission checks.
- **UX**:
  1. Shows 'X' status.
  2. User clicks "Grant".
  3. App opens System Settings deep link.
  4. App waits for focus regain to re-check.

### `ChatContainer.tsx` & `ChatInput.tsx`
- **Virtualization**: Uses simple DOM node limiting (`useConversation` logic) rather than complex virtualization libraries (React Window) for simplicity, as chat sessions are standardly short.
- **Markdown**: Renders LLM output using `react-markdown` with custom code block highlighters.

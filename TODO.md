# TODO

## JSX/MDX Renderer

### Re-enable ActionButton command whitelist
- **File**: `src/components/ui/agent-actions.tsx`
- **Status**: Disabled — `isCommandAllowed()` currently returns `true` for all commands
- **Why disabled**: During development, all Tauri commands are allowed so the agent can invoke any command via `<ActionButton command="..." />`. This removes friction while building out the component system.
- **Before production**: Restore the `ALLOWED_COMMANDS` whitelist to restrict which Tauri commands agent-rendered JSX can invoke. The previous whitelist was: `open_url`, `open_application`, `get_system_info`, `capture_screenshot`, `submit_query`, `ui_handle_interaction`.
- **Risk if left disabled**: Agent-rendered JSX could invoke destructive commands (e.g., file deletion, shell execution) if the LLM is prompted to emit malicious JSX.

### Add floating pane for JSX/MDX display
- **Current**: JSX/MDX renders inline in the chat window via `MixedContentRenderer` / `JsxMessageRenderer`
- **Goal**: Add ability to display JSX/MDX content in a separate floating pane window
- **Approach**: Shared frontend state — both chat window and floating pane read from the same conversation store
- **Key**: `MixedContentRenderer` and `JsxMessageRenderer` are already portable (no chat-specific deps)

### Re-add `hasMixedContent()` fallback if conversation persistence is added
- **File**: `src/components/ChatMessageV2.tsx`
- **Context**: Removed the `hasMixedContent()` frontend fallback check since the backend already sets `isJsx` via `is_jsx_content()` on every `stream-end` event. If conversation messages are ever loaded from persistence (Tauri Store), they may not have `isJsx` set — at that point, either persist the flag or re-add the frontend detection.

# TODO

## JSX/MDX Renderer

### ~~Re-enable ActionButton command whitelist~~ ✅ DONE (LAC-2461)
- **File**: `src/components/ui/agent-actions.tsx`
- **Status**: Whitelist re-enabled. Non-whitelisted commands fall back to `submit_query` (agent-mediated execution).
- **Allowed commands**: `open_url`, `open_application`, `capture_screenshot_command`, `submit_query`, `get_system_stats`, `get_clipboard`, `set_clipboard`, `bash_command`.

### Add floating pane for JSX/MDX display
- **Current**: JSX/MDX renders inline in the chat window via `MixedContentRenderer` / `JsxMessageRenderer`
- **Goal**: Add ability to display JSX/MDX content in a separate floating pane window
- **Approach**: Shared frontend state — both chat window and floating pane read from the same conversation store
- **Key**: `MixedContentRenderer` and `JsxMessageRenderer` are already portable (no chat-specific deps)

### Re-add `hasMixedContent()` fallback if conversation persistence is added
- **File**: `src/components/ChatMessageV2.tsx`
- **Context**: Removed the `hasMixedContent()` frontend fallback check since the backend already sets `isJsx` via `is_jsx_content()` on every `stream-end` event. If conversation messages are ever loaded from persistence (Tauri Store), they may not have `isJsx` set — at that point, either persist the flag or re-add the frontend detection.

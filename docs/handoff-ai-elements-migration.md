# Handoff: AI Elements Chat UI Migration

## What Changed

Replaced kibo-ui chat primitives and custom components with the `ai-elements` library. Model selector moved from header into the input footer. Tool approval moved from modal to inline chat.

### Files Modified

| File | Change |
|------|--------|
| `src/components/chat/ChatInput.tsx` | Rewritten: kibo-ui `AIInput` → ai-elements `PromptInput` with model selector + agent mode toggle |
| `src/components/ChatMessageV2.tsx` | `ThinkingMessage` → `Reasoning`, `ToolCallMessage` → `Tool`, pulse cursor → `Shimmer`, added inline tool approval |
| `src/components/chat/ChatContainerV2.tsx` | Passes `onApprovalUpdate` prop |
| `src/components/AppHeader.tsx` | Removed `ProviderSelector`, `ModelSelector`, `AgentModeSelector` |
| `src/App.tsx` | Removed `ToolApprovalModal`, adapted submit/stop handlers (no more `FormEvent`), added `handleApprovalUpdate` |
| `src/hooks/useBackendEvents.ts` | Added `tool-approval-request` → inline conversation message; import from `ChatMessageV2` |
| `src/hooks/useConversation.ts` | Import from `ChatMessageV2` |
| `src/hooks/useChatScrolling.ts` | Import from `ChatMessageV2` |

### Files Added (install-only, not wired)

- `src/components/ai-elements/chain-of-thought.tsx`
- `src/components/ai-elements/queue.tsx`
- `src/components/ai-elements/tool.tsx`

### Files Deleted (Step 7 cleanup)

| File | Reason |
|------|--------|
| `src/components/ThinkingMessage.tsx` | Replaced by ai-elements `Reasoning` |
| `src/components/ToolCallMessage.tsx` | Replaced by ai-elements `Tool` |
| `src/components/ToolApprovalModal.tsx` | Replaced by inline approval in `ChatMessageV2` |
| `src/components/ChatMessage.tsx` | Replaced by `ChatMessageV2.tsx` |
| `src/components/chat/ChatContainer.tsx` | Replaced by `ChatContainerV2.tsx` |
| `src/components/ModelSelector.tsx` | Model selector moved into `ChatInput.tsx` footer |
| `src/components/ProviderSelector.tsx` | No longer needed (model selector handles provider context) |
| `src/components/AgentModeSelector.tsx` | Re-added as compact toggle in `ChatInput.tsx` footer |
| `src/components/ui/kibo-ui/` (entire dir) | `ai/*` replaced by ai-elements; `code-block/*` was only used by kibo-ui ai |

## What Works

- `npx tsc --noEmit` — clean
- `npm test -- --run` — 26/26 pass
- `bun run dev` — Vite starts on :1420

### Additional Changes (Step 7 continued)

| File | Change |
|------|--------|
| `src/types/chat.ts` | **New** — Canonical `ChatMessage` type extracted here |
| `src/components/ChatMessageV2.tsx` | Re-exports `ChatMessage` from `@/types/chat` |
| `src/hooks/useBackendEvents.ts` | Import `ChatMessage` from `@/types/chat` |
| `src/hooks/useConversation.ts` | Import `ChatMessage` from `@/types/chat` |
| `src/hooks/useChatScrolling.ts` | Import `ChatMessage` from `@/types/chat` |
| `src/components/chat/ChatContainerV2.tsx` | Import `ChatMessage` from `@/types/chat` |
| `src/components/chat/ChatInput.tsx` | Agent mode toggle added to footer |

## What's Left / Not Done

1. **chain-of-thought / queue components** — Installed but not wired into the UI. These are for future use (e.g., displaying chain-of-thought reasoning steps or queued agent tasks).

2. **Visual QA needed** — The full Tauri app (`bun run tauri:dev`) should be tested for:
   - PromptInput submit/clear/stop cycle
   - Model selector dropdown populates and updates backend
   - Agent mode toggle switches single ↔ multi
   - Reasoning auto-open/close during thinking streams
   - Tool component expand/collapse with input/output
   - Inline tool approval approve/deny buttons
   - Shimmer animation on streaming messages

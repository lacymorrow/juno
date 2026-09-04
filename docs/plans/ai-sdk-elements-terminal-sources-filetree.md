# Plan: Terminal + Sources + File Tree AI SDK Elements

## Context

Juno has 12 AI SDK Elements. This adds the next 3 highest-value components — all pure frontend, no Rust changes needed. These target the biggest UX gaps: shell output is rendered as plain JSON, research citations are buried in markdown, and file operations have no visual tree.

### Full AI SDK Elements Catalog (elements.ai-sdk.dev)

**Already in Juno (12):** chain-of-thought, code-block, confirmation, conversation, environment-variables, message, model-selector, prompt-input, queue, reasoning, shimmer, suggestion, tool

**Not yet in Juno (grouped by Juno fit):**

| Priority | Component | Category | What it does |
|----------|-----------|----------|-------------|
| **HIGH** | Terminal | Code | ANSI-colored terminal output with streaming |
| **HIGH** | Sources | Chatbot | Collapsible source URL list below messages |
| **HIGH** | File Tree | Code | Hierarchical file/folder browser |
| **HIGH** | Task | Chatbot | Task list with status (pending/running/done/error) |
| **HIGH** | Context | Chatbot | Token usage ring, cost display, breakdown |
| **HIGH** | Plan | Chatbot | Collapsible AI execution plan with streaming |
| Medium | Attachments | Chatbot | File/image/video attachment grid/inline/list |
| Medium | Checkpoint | Chatbot | Mark & restore conversation history points |
| Medium | Artifact | Code | Container for generated content with actions |
| Medium | Stack Trace | Code | Formatted error trace with clickable paths |
| Medium | Test Results | Code | Test suite pass/fail/skip display |
| Low | Inline Citation | Chatbot | Hover-card citations inline `[1]` |
| Low | Agent | Code | Agent identity display |
| Low | Commit | Code | Git commit display |
| Low | Snippet | Code | Code snippet (code-block covers this) |
| Low | JSX Preview | Code | Live JSX render (already have JsxMessageRenderer) |
| Low | Package Info | Code | npm package card |
| Low | Sandbox | Code | Code sandbox |
| Low | Schema Display | Code | JSON/DB schema viewer |
| Low | Web Preview | Code | Website preview iframe |
| N/A | Voice (6) | Voice | All handled by Rust backend |
| N/A | Workflow (7) | Workflow | Not applicable to current UI |
| N/A | Utility (2) | Utility | Image, Open In Chat |

---

## Phase A: Terminal Component

### Why
The agent's bash tool returns `{ "output": string, "exit_code": number }`. Currently this renders as JSON inside `<CodeBlock language="json">` — losing all ANSI colors from `git diff`, `ls --color`, compiler errors, etc. A Terminal component renders ANSI escape codes as styled HTML.

### New file: `src/components/ai-elements/terminal.tsx`

**Sub-components** (same compound pattern as tool.tsx):
- `Terminal` — Root container, dark bg, rounded, monospace. Props: `className`
- `TerminalHeader` — Flex row for title + actions
- `TerminalTitle` — Text label (e.g., "bash", "shell")
- `TerminalStatus` — Optional streaming/exit code badge
- `TerminalActions` — Container for action buttons
- `TerminalCopyButton` — Copies raw output to clipboard (strips ANSI). Uses `Button` from `ui/button.tsx`
- `TerminalContent` — Scrollable output area with ANSI rendering. Props: `output: string`, `isStreaming?: boolean`, `maxHeight?: string` (default `"400px"`)

**ANSI rendering approach**: Use a lightweight inline ANSI parser rather than adding `ansi-to-react` as a dependency. Parse the 7 basic SGR codes (reset, bold, dim, red/green/yellow/blue/cyan/white) into `<span>` elements with Tailwind classes. This covers 95%+ of real terminal output (git, compilers, ls) without a new npm dep.

Helper function: `parseAnsi(text: string): ReactNode[]` — splits on `\x1b[...m` sequences, maps to styled spans.

**Reuse**: `Button` from `ui/button.tsx`, `Badge` from `ui/badge.tsx`, `cn` from `lib/utils`

### Modify: `src/components/ChatMessageV2.tsx`

In the `tool_call_result` block (lines ~239-271), add a bash-specific handler **before** the generic tool_call_result block:

```tsx
// Bash tool results — render with Terminal component
if (msg.role === "tool_call_result" && msg.tool_name === "bash") {
  const output = typeof msg.tool_output === "object" ? msg.tool_output : { output: msg.tool_output };
  const exitCode = output?.exit_code ?? (msg.success !== false ? 0 : 1);

  return (
    <div className="flex justify-start w-full">
      <Terminal>
        <TerminalHeader>
          <TerminalTitle>bash</TerminalTitle>
          <TerminalStatus exitCode={exitCode} isStreaming={msg.isStreaming} />
          <TerminalActions>
            <TerminalCopyButton value={output?.output ?? msg.content} />
          </TerminalActions>
        </TerminalHeader>
        <TerminalContent output={output?.output ?? msg.content} />
      </Terminal>
    </div>
  );
}
```

**Imports to add**: `Terminal, TerminalHeader, TerminalTitle, TerminalStatus, TerminalActions, TerminalCopyButton, TerminalContent` from `@/components/ai-elements/terminal`

---

## Phase B: Sources Component

### Why
When the agent researches topics, URLs appear inline in markdown text. A collapsible "N sources" pill below the message is standard AI UX (like Perplexity, ChatGPT search). This extracts URLs and presents them as a structured, scannable list.

### New file: `src/components/ai-elements/sources.tsx`

**Sub-components**:
- `Sources` — Root, wraps Collapsible from `ui/collapsible.tsx`. Props: `className`
- `SourcesTrigger` — CollapsibleTrigger showing count badge. Props: `count: number`
- `SourcesContent` — CollapsibleContent container for source links
- `Source` — Single source link. Props: extends `<a>` props (`href`, `children`, etc.). Renders as external link with favicon and truncated URL

**URL extraction helper**: `extractUrls(text: string): string[]` — regex to find `https?://...` URLs in markdown content. Deduplicates. Filters out image URLs (`.png`, `.jpg`, `.gif`, `.svg`). Exported for reuse.

**Reuse**: `Collapsible, CollapsibleTrigger, CollapsibleContent` from `ui/collapsible.tsx`, `Badge` from `ui/badge.tsx`, `ExternalLinkIcon` from lucide-react

### Modify: `src/components/ChatMessageV2.tsx`

In the assistant message rendering block (lines ~275-365), inside `<MessageContent>` after the main content render, before the TTS section:

```tsx
{/* Sources — extracted URLs from assistant messages */}
{msg.role === "assistant" && msg.content && !msg.isStreaming && (() => {
  const urls = extractUrls(msg.content);
  if (urls.length === 0) return null;
  return (
    <Sources>
      <SourcesTrigger count={urls.length} />
      <SourcesContent>
        {urls.map((url) => (
          <Source key={url} href={url}>{url}</Source>
        ))}
      </SourcesContent>
    </Sources>
  );
})()}
```

**Imports to add**: `Sources, SourcesTrigger, SourcesContent, Source, extractUrls` from `@/components/ai-elements/sources`

---

## Phase C: File Tree Component

### Why
The file agent creates, reads, edits, and lists files. Currently file paths appear as text in tool args/output. A visual tree gives spatial awareness of what the agent touched — especially valuable when multiple files are modified in sequence.

### New file: `src/components/ai-elements/file-tree.tsx`

**Sub-components**:
- `FileTree` — Root container with expand/collapse state. Props: `expanded?: string[]`, `defaultExpanded?: string[]`, `onExpandedChange?`, `selectedPath?`, `onSelect?`, `className`
- `FileTreeFolder` — Collapsible directory node. Props: `path: string`, `name: string`, `children`
- `FileTreeFile` — Leaf file node. Props: `path: string`, `name: string`, `icon?: ReactNode`
- `FileTreeIcon` — Icon wrapper (auto-selects icon by file extension if none provided)
- `FileTreeName` — Text label
- `FileTreeActions` — Action container (e.g., open in editor)

**File icon helper**: `getFileIcon(name: string): ReactNode` — maps common extensions (`.ts`, `.tsx`, `.rs`, `.json`, `.md`, `.css`, `.html`) to lucide icons (`FileCode`, `FileText`, `FileJson`, `Cog`). Falls back to generic `File` icon.

**Tree builder helper**: `buildTree(paths: string[]): TreeNode[]` — takes flat file path array, builds nested tree structure. Groups by directory. Sorts folders first, then files alphabetically.

```typescript
type TreeNode = {
  name: string;
  path: string;
  type: "file" | "folder";
  children?: TreeNode[];
};
```

**Internal context** shares `expanded`, `selectedPath`, toggle callbacks with children (same pattern as Confirmation).

**Reuse**: `Collapsible, CollapsibleTrigger, CollapsibleContent` from `ui/collapsible.tsx`, `cn` from `lib/utils`, lucide icons

### Integration: JSX component whitelist

The File Tree is used as a **JSX response component** — the agent can emit `<FileTree>` in its response and the tri-modal renderer picks it up.

**Modify: `src/components/ui/mixed-content-renderer.tsx`**
- Add `"FileTree"` to the `JSX_COMPONENT_NAMES` array (line ~17)

**Modify: `src/components/ui/jsx-message-renderer.tsx`**
- Add FileTree to the `availableComponents` map (line ~320):
```tsx
import { FileTree, FileTreeFolder, FileTreeFile, buildTree } from "@/components/ai-elements/file-tree";

// In availableComponents:
FileTree: ({ paths, ...props }) => {
  const tree = buildTree(paths);
  return <FileTree {...props}>{renderTree(tree)}</FileTree>;
}
```

---

## Files Summary

### New files (3)
| File | Purpose |
|------|---------|
| `src/components/ai-elements/terminal.tsx` | ANSI-colored terminal output display |
| `src/components/ai-elements/sources.tsx` | Collapsible source URL list with count badge |
| `src/components/ai-elements/file-tree.tsx` | Hierarchical file/folder tree with icons |

### Modified files (3)
| File | Changes |
|------|---------|
| `src/components/ChatMessageV2.tsx` | Bash tool results use Terminal; assistant messages show Sources |
| `src/components/ui/jsx-message-renderer.tsx` | Add FileTree to `availableComponents` registry |
| `src/components/ui/mixed-content-renderer.tsx` | Add `"FileTree"` to `JSX_COMPONENT_NAMES` |

### No backend changes needed
All three components are purely presentational, consuming existing data from `ChatMessage`.

---

## Verification

### Per-component
1. `npx tsc --noEmit` — zero errors
2. `npm test -- --run` — all 26 tests pass

### Manual testing
- **Terminal**: Run a query that triggers bash (e.g., "list files in this directory") → verify colored output, exit code badge, copy button
- **Sources**: Ask a research question → verify URLs extracted and shown in collapsible pill below response
- **File Tree**: Use agent JSX channel or file operation → verify tree renders with correct hierarchy and icons

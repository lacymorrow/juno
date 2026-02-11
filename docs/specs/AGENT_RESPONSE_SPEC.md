# Agent Response Specification

## Vision

Juno is a keyboard-shortcut-activated AI assistant that lives on the user's desktop. When invoked, the agent can:

1. Give simple conversational responses
2. Run bash commands
3. Take over and control the computer as the user's assistant

Every agent response is **tri-modal** — it can include up to three simultaneous output channels:

| Channel | Purpose | Example (weather query) |
|---------|---------|------------------------|
| **Text** | Concise visual blurb shown in the chat | "It's 51°F with a low of 48° and high of 68°. 50% chance of rain." |
| **Voice** | Natural spoken response via TTS | "It's chilly. You might want a jacket and pants." |
| **Component** | Interactive TSX/React UI relevant to the query | Weather card with rain animation, or launching the native Weather app |

The agent does **not** need all three channels every time — but tri-modal responses are preferred for non-trivial queries because they serve non-technical users better.

---

## Response Channel Details

### 1. Text (Display Content)

The text channel is the **written summary** shown in the chat window. It should be:

- **Concise** — a blurb, not an essay. Think notification-level brevity.
- **Scannable** — key facts first, details below if needed.
- **Formatted** — markdown for structure (bullets, bold, code blocks when relevant).

Text is rendered via the `Response` component (streamdown) for streaming markdown, or plain spans for short messages.

**When to include**: Always, unless the response is purely a voice acknowledgment for a trivial action (e.g., "Done.").

### 2. Voice (Spoken Content)

The voice channel uses `<TTS>` XML tags in the agent's response to separate spoken content from display content. Content inside `<TTS>` tags is extracted during streaming and sent to the TTS engine immediately.

Key principles:
- **Conversational tone** — contractions, natural phrasing, personality
- **Different from text** — voice summarizes; text has details. Don't just read the text aloud.
- **Progressive** — multiple `<TTS>` tags can appear throughout a response for progress updates
- **Concise** — avoid reading lists, technical details, file paths, PIDs

**When to include**: For direct responses to questions, task confirmations, errors needing attention, and any response where the user is likely not looking at the screen.

**When to skip**: Self-evident actions (opening Calculator), status updates during long operations, purely technical output.

### 3. Component (Visual UI)

The component channel renders interactive React/TSX components inline in the chat. This is the most distinctive feature — instead of just text, the agent can output rich, interactive UI.

Components are detected via `is_jsx_content()` and rendered through `JsxMessageRenderer` which provides 50+ available components (shadcn/ui primitives, shapes, status cards, progress bars, icons, custom showcase components).

**Available component categories**:
- **Layout**: div, span, p, h1-h6
- **UI primitives**: Button, Card, Alert, Badge, Input, Select, Tabs, Dialog, Tooltip, etc.
- **Data display**: StatusCard, ProgressBar, ColorShowcase
- **Shapes**: Circle, Rectangle, Triangle
- **Icons**: 18 Lucide icons (Check, Star, Heart, Zap, Sparkles, etc.)
- **Demo**: VisualDemo (capability showcase)

**When to include**: Queries where a visual component adds value beyond text — weather, file organization results, system status, comparison tables, interactive confirmations, data visualization.

**When to skip**: Simple Q&A, quick confirmations, error messages that are better as text.

---

## Response Format in Agent Output

The agent's raw response combines all three channels in a single streamed output:

```xml
<TTS>It's chilly. You might want a jacket and pants.</TTS>

It's 51°F with a low of 48° and high of 68°. 50% chance of rain.

<Card>
  <CardHeader>
    <CardTitle>Weather — San Francisco</CardTitle>
  </CardHeader>
  <CardContent>
    <div className="flex items-center gap-4">
      <Badge>51°F</Badge>
      <span>Low 48° / High 68°</span>
      <Badge variant="outline">50% rain</Badge>
    </div>
  </CardContent>
</Card>
```

The backend parses this into:
- **TTS content**: Extracted from `<TTS>` tags → sent to TTS engine during streaming
- **Display text**: Everything outside `<TTS>` tags that isn't JSX → rendered as markdown
- **JSX components**: Detected by `is_jsx_content()` → rendered via `JsxMessageRenderer`

---

## Examples by Query Type

### Simple Query — "What time is it?"
```
Voice: "It's 3:47 PM."
Text: 3:47 PM, Saturday, February 8, 2026
Component: (none — too simple)
```

### Informational — "What's the weather?"
```
Voice: "It's chilly out. Grab a jacket — there's a fifty percent chance of rain later."
Text: 51°F • Low 48° / High 68° • 50% chance of rain • Sunset 5:42 PM
Component: Weather card with temperature, conditions, hourly forecast
```

### Action — "Open Spotify and play my liked songs"
```
Voice: "Playing your liked songs now."
Text: ✅ Spotify opened • Playing: Liked Songs (847 tracks)
Component: (none — action is self-evident, or mini player card)
```

### Complex Task — "Organize my Downloads folder"
```
Voice (start): "I'll organize your Downloads folder. Let me scan what's there first."
Text: Scanning Downloads... Found 127 files (23 images, 45 documents, 15 videos, 12 archives, 32 other)
Voice (end): "Done! I organized everything into folders by type."
Text: ✅ Organized 127 files into 5 categories
Component: Summary card with folder icons, file counts, and "Open in Finder" button
```

### Research — "Compare React vs Vue for my project"
```
Voice: "For your project, I'd lean toward React. Here's a quick comparison."
Text: Detailed comparison with pros/cons
Component: Side-by-side comparison card with ratings, ecosystem stats, learning curve indicators
```

### Error — "Delete that file"
```
Voice: "I can't find which file you mean. Could you tell me the name or where it is?"
Text: ❓ No file specified in context. Please provide a filename or path.
Component: (none)
```

### Computer Control — "Book a table at the Italian place for tonight"
```
Voice (start): "I'll look up Italian restaurants near you and check availability."
Text: Searching for Italian restaurants...
Voice (mid): "Found three options. Trattoria Roma has an 8pm opening."
Text: 🍝 Trattoria Roma — 8:00 PM available, 4.5★, 0.3 mi away
Component: Restaurant card with rating, distance, "Confirm Booking" button
Voice (end): "I've made a reservation for 8 PM at Trattoria Roma."
```

---

## Architecture: Current State vs. Target

### What Exists Today (Working)

| Capability | Status | Location |
|-----------|--------|----------|
| Text streaming | ✅ Working | `agent-text-stream` events → `useBackendEvents.ts` → `Response` component |
| TTS via `<TTS>` tags | ✅ Working | `emit_streaming_text_chunk()` extracts TTS → `invoke_tts()` |
| `isJsx` field on ChatMessage | ✅ Defined | `ChatMessage.tsx` / `ChatMessageV2.tsx` |
| `JsxMessageRenderer` | ✅ Working | 50+ components registered, `JsxRenderer` handles rendering |
| `is_jsx_content()` detection | ✅ Working | `anthropic.rs` — checks for JSX indicators in content |
| Keyboard shortcut trigger | ✅ Working | `useShortcutEvents.ts` → agent mode / dictation |
| Voice input (Whisper) | ✅ Working | `tauri-plugin-voice-transcription` |

### What's Missing (Gaps to Fill)

| Gap | Description | Priority |
|-----|-------------|----------|
| **JSX not wired to streaming** | `is_jsx_content()` only runs on specialist delegation results, not on streamed messages. `useBackendEvents.ts` never sets `isJsx: true`. | P0 |
| **No component-aware streaming** | Agent streams a mix of text + JSX. The frontend needs to detect JSX boundaries during streaming and render them correctly without flashing/reformatting. | P0 |
| **Agent prompt: component instructions** | The system prompt has TTS instructions but no guidance on when/how to output JSX components. Agents don't know they can render UI. | P0 |
| **Component library gaps** | Weather widgets, media players, file browsers, restaurant cards, etc. are not in `availableComponents`. The library is basic (shapes, status cards). | P1 |
| **Mixed content rendering** | A single message with text + JSX needs to split at JSX boundaries — render markdown above, component in the middle, markdown below. Current renderer is either/or. | P1 |
| **Native app integration** | "Open the Weather app" requires shell commands or AppleScript. Component can include a button that invokes a Tauri command to open native apps. | P2 |
| **Component interactivity** | Components with buttons (e.g., "Confirm Booking") need to invoke Tauri commands or trigger new agent queries. Currently JSX components are display-only. | P2 |

### Implementation Roadmap

**Phase 1: Wire JSX to Streaming Pipeline**
- Emit `jsx_content: bool` in `agent-stream-end` event payload
- Set `isJsx` on ChatMessage in `useBackendEvents.ts` when stream ends
- Detect JSX boundaries in streamed content for partial rendering

**Phase 2: Agent Prompt Enhancement**
- Add component rendering instructions to system prompt templates
- Document available components and when to use them
- Add few-shot examples showing tri-modal responses

**Phase 3: Mixed Content Renderer**
- Split message content at JSX boundaries
- Render: markdown → JSX component → markdown → JSX component → ...
- Handle streaming gracefully (don't flash between text and JSX modes)

**Phase 4: Expand Component Library**
- Weather card, media player card, file browser card
- System status dashboard, comparison table
- Interactive components that invoke Tauri commands

**Phase 5: Interactive Components**
- Wire component button clicks to Tauri commands
- Allow components to trigger new agent queries
- Bidirectional component ↔ agent communication

---

## Prompt Engineering Guidelines

### For System Prompt Authors

The agent needs explicit instructions about the tri-modal response format. Add to prompt templates:

```
## Response Format

You have THREE output channels. Use them together for the best user experience:

1. **Voice (<TTS> tags)**: Spoken aloud. Keep it conversational, brief, personality-driven.
2. **Text (markdown)**: Displayed in chat. Can be more detailed/technical than voice.
3. **Components (JSX)**: Rich UI rendered inline. Use for data, status, comparisons, visual feedback.

Guidelines:
- Prefer tri-modal responses for non-trivial queries
- Voice and text should complement, not duplicate
- Components add value when data has structure (lists, comparisons, status)
- Simple queries (yes/no, quick facts) may only need voice + text
- Trivial actions (open app) may only need voice

Available JSX components: Card, Alert, Badge, Button, StatusCard, ProgressBar,
Circle, Rectangle, Triangle, Tabs, and all shadcn/ui primitives.
```

### For Agent Response Quality

The three channels serve different cognitive modes:
- **Voice** → auditory, ambient, hands-free. User might be looking away.
- **Text** → reference, scanning, details. User can re-read.
- **Component** → visual comprehension, interaction, delight. Data-dense.

The agent should think: *"What does the user need to hear? What do they need to read? What do they need to see?"*

---

## Architectural Boundary: Backend vs. Frontend

The tri-modal response system spans both Rust and TypeScript, but the division is strict:

### Rust Backend (ALL logic)
- Agent execution, prompt construction, tool calling
- TTS extraction from `<TTS>` tags → audio synthesis and playback
- JSX detection (`is_jsx_content()`) → sets `is_jsx` flag on stream-end events
- Keyboard shortcut triggers agent invocation
- Microphone recording → transcription → query submission
- All AI provider API calls (Anthropic, OpenAI, Gemini, etc.)

### TypeScript Frontend (display ONLY)
- Renders text channel as markdown via `Response` (streamdown)
- Renders component channel as React via `JsxMessageRenderer` / `MixedContentRenderer`
- Displays streaming state (cursor, loading indicators)
- Sends user interactions via `invoke()` (button clicks, text input)

### What the frontend NEVER does
- Plays or records audio (no `getUserMedia`, no `Web Audio API`, no `AudioContext`)
- Registers keyboard shortcuts (global hotkeys are in Rust)
- Makes HTTP requests to AI providers
- Opens WebSocket connections
- Executes shell commands or file operations

### Interactive components
JSX action components (`ActionButton`, `QueryButton`, `OpenButton`, `CopyButton`) call `invoke()` to route actions to the backend. They do NOT execute logic directly — they are UI triggers that delegate to Rust.

---

## Technical Constraints

- **Streaming**: Text and TTS stream in real-time. JSX components can only be safely rendered after the JSX block is complete (opening + closing tags). Partial JSX should buffer, not render.
- **TTS latency**: First `<TTS>` tag should appear early in the response for fast audio feedback.
- **Component safety**: `JsxRenderer` sandboxes rendering. No `eval()`, no arbitrary code execution. Components must be from the registered `availableComponents` map.
- **Message size**: The conversation auto-prunes when exceeding `MAX_CHAT_HISTORY_ITEMS`. Large JSX components count the same as text messages.
- **Multi-agent**: In orchestrator mode, specialists can return JSX. The orchestrator should pass JSX through, not re-wrap or duplicate it.

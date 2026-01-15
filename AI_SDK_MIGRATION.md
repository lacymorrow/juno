# AI SDK Elements Migration Guide

## Overview

This document outlines the migration from existing chat components to the new AI SDK Elements from [ai-sdk.dev/elements](https://ai-sdk.dev/elements).

## Installed Components

All 19 AI SDK Element components have been fetched and adapted:

| Component | Location | Purpose |
|-----------|----------|---------|
| `message` | `src/components/ai-elements/message.tsx` | Message display with attachments, toolbar |
| `conversation` | `src/components/ai-elements/conversation.tsx` | Scrollable message container |
| `prompt-input` | `src/components/ai-elements/prompt-input.tsx` | Rich text input with attachments |
| `code-block` | `src/components/ai-elements/code-block.tsx` | Syntax-highlighted code |
| `reasoning` | `src/components/ai-elements/reasoning.tsx` | Chain-of-thought display |
| `tool` | `src/components/ai-elements/tool.tsx` | Tool call visualization |
| `loader` | `src/components/ai-elements/loader.tsx` | Loading states |
| `shimmer` | `src/components/ai-elements/shimmer.tsx` | Skeleton loading |
| `suggestion` | `src/components/ai-elements/suggestion.tsx` | Prompt suggestions |
| `sources` | `src/components/ai-elements/sources.tsx` | Citation/source display |
| `confirmation` | `src/components/ai-elements/confirmation.tsx` | User confirmation dialogs |
| `model-selector` | `src/components/ai-elements/model-selector.tsx` | Model picker |
| `toolbar` | `src/components/ai-elements/toolbar.tsx` | Action toolbar |
| `persona` | `src/components/ai-elements/persona.tsx` | AI persona/avatar |
| `checkpoint` | `src/components/ai-elements/checkpoint.tsx` | Progress checkpoints |
| `artifact` | `src/components/ai-elements/artifact.tsx` | Generated artifacts |
| `transcription` | `src/components/ai-elements/transcription.tsx` | Voice transcription |
| `controls` | `src/components/ai-elements/controls.tsx` | Playback controls |

## Dependencies Added

```bash
npm install ai use-stick-to-bottom streamdown nanoid
```

## Component Mapping

### Replace `ChatMessage.tsx` → AI SDK `Message`

**Before:**
```tsx
import { ChatMessageComponent } from "@/components/ChatMessage";

<ChatMessageComponent
  msg={msg}
  index={index}
  copyingMessageId={copyingMessageId}
  onCopyResponse={onCopyResponse}
/>
```

**After:**
```tsx
import { 
  Message, 
  MessageContent, 
  MessageToolbar,
  MessageActions 
} from "@/components/ai-elements";

<Message from={msg.role}>
  <MessageContent>
    {msg.content}
  </MessageContent>
  <MessageToolbar>
    <MessageActions onCopy={() => onCopyResponse(msg.content, index)} />
  </MessageToolbar>
</Message>
```

### Replace `ChatContainer.tsx` → AI SDK `Conversation`

**Before:**
```tsx
import { ScrollArea } from "@/components/ui/scroll-area";

<ScrollArea className="flex-1">
  {messages.map((msg, i) => (
    <ChatMessageComponent key={i} msg={msg} />
  ))}
</ScrollArea>
```

**After:**
```tsx
import { 
  Conversation, 
  ConversationContent,
  ConversationScrollButton,
  ConversationEmptyState 
} from "@/components/ai-elements";

<Conversation>
  <ConversationContent>
    {messages.length === 0 ? (
      <ConversationEmptyState
        title="Start a conversation"
        description="Ask me anything!"
      />
    ) : (
      messages.map((msg, i) => (
        <Message key={i} from={msg.role}>
          <MessageContent>{msg.content}</MessageContent>
        </Message>
      ))
    )}
  </ConversationContent>
  <ConversationScrollButton />
</Conversation>
```

### Replace `ChatInput.tsx` → AI SDK `PromptInput`

**Before:**
```tsx
import { AIInput, AIInputTextarea } from "@/components/ui/kibo-ui/ai";

<AIInput onSubmit={onSubmit}>
  <AIInputTextarea value={query} onChange={setQuery} />
</AIInput>
```

**After:**
```tsx
import { 
  PromptInput,
  PromptInputTextarea,
  PromptInputActions,
  PromptInputAction 
} from "@/components/ai-elements";

<PromptInput onSubmit={handleSubmit}>
  <PromptInputTextarea
    value={input}
    onChange={setInput}
    placeholder="Ask me anything..."
  />
  <PromptInputActions>
    <PromptInputAction type="submit" />
  </PromptInputActions>
</PromptInput>
```

### Replace `ToolCallMessage.tsx` → AI SDK `Tool`

**Before:**
```tsx
<ToolCallRequest tool_name={msg.tool_name} tool_args={msg.tool_args} />
```

**After:**
```tsx
import { Tool, ToolIcon, ToolName, ToolResult } from "@/components/ai-elements";

<Tool>
  <ToolIcon>{getToolIcon(tool.name)}</ToolIcon>
  <ToolName>{tool.name}</ToolName>
  <ToolResult status={tool.status}>
    {tool.result}
  </ToolResult>
</Tool>
```

### Replace `ThinkingMessage.tsx` → AI SDK `Reasoning`

**Before:**
```tsx
<ThinkingMessage />
```

**After:**
```tsx
import { Reasoning, ReasoningStep, ReasoningContent } from "@/components/ai-elements";

<Reasoning>
  <ReasoningStep>
    <ReasoningContent>
      {thinking.content}
    </ReasoningContent>
  </ReasoningStep>
</Reasoning>
```

### Replace `ModelSelector.tsx` → AI SDK `ModelSelector`

**Before:**
```tsx
import { ModelSelector } from "@/components/ModelSelector";

<ModelSelector
  value={selectedModel}
  onChange={setSelectedModel}
  models={availableModels}
/>
```

**After:**
```tsx
import { 
  ModelSelector,
  ModelSelectorTrigger,
  ModelSelectorContent,
  ModelSelectorItem 
} from "@/components/ai-elements";

<ModelSelector value={model} onValueChange={setModel}>
  <ModelSelectorTrigger />
  <ModelSelectorContent>
    {models.map(m => (
      <ModelSelectorItem key={m.id} value={m.id}>
        {m.name}
      </ModelSelectorItem>
    ))}
  </ModelSelectorContent>
</ModelSelector>
```

## Implementation Steps

### Phase 1: Core Components (High Priority)

1. **Update imports** - Add AI Elements to existing components
2. **Replace ChatContainer** with `Conversation` wrapper
3. **Replace ChatMessage** display with `Message` components
4. **Replace ChatInput** with `PromptInput`

### Phase 2: Enhanced Features

1. **Add `Reasoning`** for thinking/chain-of-thought
2. **Add `Tool`** for better tool call visualization  
3. **Add `CodeBlock`** with Shiki syntax highlighting
4. **Add `Suggestion`** for prompt suggestions

### Phase 3: Polish

1. **Add `Loader`** and `Shimmer` for loading states
2. **Add `Sources`** for citation display
3. **Add `Confirmation`** for action confirmations
4. **Migrate `ModelSelector`** to AI SDK version

## Usage Example

Here's a complete example of a modern AI chat interface:

```tsx
import {
  Conversation,
  ConversationContent,
  ConversationEmptyState,
  ConversationScrollButton,
  Message,
  MessageContent,
  MessageToolbar,
  MessageActions,
  PromptInput,
  PromptInputTextarea,
  PromptInputActions,
  PromptInputAction,
  Reasoning,
  Tool,
  Loader,
} from "@/components/ai-elements";

export function ModernChat() {
  const [messages, setMessages] = useState<UIMessage[]>([]);
  const [input, setInput] = useState("");
  const [isLoading, setIsLoading] = useState(false);

  return (
    <div className="flex flex-col h-full">
      <Conversation>
        <ConversationContent>
          {messages.length === 0 ? (
            <ConversationEmptyState
              title="Juno AI"
              description="Your AI desktop assistant"
            />
          ) : (
            messages.map((msg) => (
              <Message key={msg.id} from={msg.role}>
                {msg.reasoning && (
                  <Reasoning>{msg.reasoning}</Reasoning>
                )}
                <MessageContent>
                  {msg.content}
                </MessageContent>
                {msg.toolInvocations?.map((tool) => (
                  <Tool key={tool.id} {...tool} />
                ))}
                <MessageToolbar>
                  <MessageActions />
                </MessageToolbar>
              </Message>
            ))
          )}
          {isLoading && <Loader />}
        </ConversationContent>
        <ConversationScrollButton />
      </Conversation>

      <PromptInput onSubmit={handleSubmit}>
        <PromptInputTextarea
          value={input}
          onChange={setInput}
          disabled={isLoading}
        />
        <PromptInputActions>
          <PromptInputAction type="submit" disabled={isLoading} />
        </PromptInputActions>
      </PromptInput>
    </div>
  );
}
```

## Notes

- Components use Tailwind CSS for styling
- Dark mode is supported via `.dark` class
- All components are accessible (ARIA compliant)
- TypeScript types are included from the `ai` package

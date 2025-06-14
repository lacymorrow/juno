# Structured Response Implementation

## Overview

The Juno agent now supports structured responses with three distinct content types:
- **📝 Markdown Content**: Detailed information, documentation, lists, technical explanations
- **🎨 Visual Content**: React/JSX components for interactive displays, status cards, shapes
- **🗣️ Speech Content**: Concise text optimized for text-to-speech (TTS)

## Implementation Components

### 1. Enhanced Prompt Templates (`src-tauri/src/agent/prompts/templates.rs`)

All agent prompts have been updated to version 2.0.0 with structured output capabilities:

- **System Default**: Main single-agent prompt with structured format
- **Development Mode**: Self-aware prompt with development-specific visual components
- **Orchestrator**: Multi-agent coordination with structured delegation
- **Expert Agents**: Browser, coding, desktop, general experts
- **Specialist Agents**: Browser, desktop, file operation specialists

#### Structured Format Example:
```rust
🎯 **STRUCTURED RESPONSE FORMAT**
You can provide rich, multi-modal responses with separate content for different purposes:

**📝 MARKDOWN SECTION** (For detailed information):
```markdown
<!-- MARKDOWN_CONTENT -->
## Task Results
- **Status**: Completed
- **Details**: File created successfully
<!-- /MARKDOWN_CONTENT -->
```

**🎨 VISUAL SECTION** (For interactive components):
```jsx
{/* VISUAL_CONTENT */}
<Card>
  <CardHeader>
    <CardTitle>Task Complete</CardTitle>
  </CardHeader>
  <CardContent>
    <StatusCard status="success" message="File saved" icon={<CheckCircle />} />
  </CardContent>
</Card>
{/* /VISUAL_CONTENT */}
```

**🗣️ SPEECH SECTION** (For TTS):
```text
<!-- SPEECH_CONTENT -->
Done! Your file has been created and is ready to use.
<!-- /SPEECH_CONTENT -->
```
```

### 2. Frontend Parser (`src/lib/structured-response-parser.ts`)

#### Core Functions:

- **`parseStructuredResponse(content: string): StructuredResponse`**
  - Extracts markdown, visual, and speech sections from agent responses
  - Returns structured object with parsed content

- **`hasStructuredContent(content: string): boolean`**
  - Quickly detects if content contains structured sections

- **`getRenderContent(parsed: StructuredResponse)`**
  - Determines optimal rendering strategy (visual > markdown > text)
  - Returns content and type for rendering, plus speech text for TTS

#### Content Prioritization:
1. **Visual Content**: JSX components for rich interactive displays
2. **Markdown Content**: Formatted documentation and explanations  
3. **Original Content**: Fallback for non-structured responses
4. **Speech Content**: Always used for TTS when available

### 3. Frontend Integration (`src/App.tsx`)

#### Enhanced Message Types:
```typescript
type ChatMessage = {
  // ... existing fields
  isStructured?: boolean;           // Flag for structured content
  structuredContent?: StructuredResponse; // Parsed structured data
  renderType?: 'jsx' | 'markdown' | 'text'; // Determined render type
  speechText?: string;              // Text optimized for TTS
};
```

#### Integration Points:

1. **Streaming End Handler**: Parses structured content when streaming completes
2. **Backend Response Handler**: Parses non-streaming responses
3. **Message Rendering**: Handles different content types appropriately
4. **TTS Integration**: Uses speech text when available

#### Streaming Compatibility:
- Structured parsing only occurs after streaming completes
- Maintains existing streaming indicators and progressive text display
- Preserves JSX detection for non-structured content

## Usage Examples

### Agent Response with All Sections:
```
<!-- MARKDOWN_CONTENT -->
## Document Creation Summary

### File Details
- **Name**: meeting-notes.txt
- **Location**: ~/Documents/
- **Size**: 1.2KB
- **Format**: Plain text with markdown

### Actions Taken
1. Created new file
2. Added content structure
3. Saved to Documents folder
4. Verified file integrity
<!-- /MARKDOWN_CONTENT -->

{/* VISUAL_CONTENT */}
<Card>
  <CardHeader>
    <CardTitle>Document Created Successfully</CardTitle>
  </CardHeader>
  <CardContent>
    <StatusCard status="success" message="File saved to Documents" icon={<CheckCircle />} />
    <Separator />
    <div className="flex items-center gap-2">
      <Badge variant="secondary">Plain Text</Badge>
      <Badge variant="outline">1.2KB</Badge>
      <Badge variant="secondary">Markdown</Badge>
    </div>
  </CardContent>
</Card>
{/* /VISUAL_CONTENT */}

<!-- SPEECH_CONTENT -->
Perfect! I've created your meeting notes document and saved it to your Documents folder. The file is ready to use.
<!-- /SPEECH_CONTENT -->
```

### Visual-Only Response:
```
{/* VISUAL_CONTENT */}
<Card>
  <CardHeader>
    <CardTitle>Circle Shape</CardTitle>
  </CardHeader>
  <CardContent>
    <Circle size={100} color="blue" borderColor="black" borderWidth={2} />
  </CardContent>
</Card>
{/* /VISUAL_CONTENT */}
```

### Speech-Optimized Response:
```
<!-- SPEECH_CONTENT -->
Task completed successfully. The application is now running and ready for your input.
<!-- /SPEECH_CONTENT -->
```

## Benefits

### For Users:
- **Rich Visual Feedback**: Interactive status displays, progress indicators, shapes
- **Natural Voice Interaction**: TTS optimized for conversation flow
- **Comprehensive Information**: Detailed markdown documentation when needed

### For Developers:
- **Streaming Compatible**: Works seamlessly with existing streaming infrastructure
- **Backward Compatible**: Non-structured responses continue to work
- **Flexible**: Agents can use any combination of content types

### For Content Types:
- **Visual**: Perfect for status updates, demonstrations, interactive elements
- **Markdown**: Ideal for documentation, code, structured information
- **Speech**: Optimized for natural conversation and voice interfaces

## Technical Architecture

### Content Flow:
1. **Agent Generation**: Prompts guide structured output creation
2. **Streaming**: Text streams normally, parsing occurs at completion
3. **Parsing**: Frontend extracts three content types
4. **Rendering**: Visual > Markdown > Text priority
5. **TTS**: Uses speech content for natural voice output

### Performance:
- **Lazy Parsing**: Only parses when structured content detected
- **Efficient Detection**: Quick regex checks before full parsing
- **Memory Efficient**: Structured content stored only when present

### Compatibility:
- **Existing Responses**: Continue to work without modification
- **JSX Detection**: Fallback for non-structured JSX content
- **Streaming**: Full compatibility with real-time text streaming

## Testing Scenarios

### Basic Functionality:
1. **Structured Response**: All three sections render correctly
2. **Mixed Content**: Combination of any sections works
3. **Fallback**: Non-structured content displays normally
4. **Streaming**: Progressive text works with final parsing

### Content Types:
1. **Visual**: JSX components render properly
2. **Markdown**: Rich formatting displays correctly
3. **Speech**: TTS uses appropriate text
4. **Priority**: Visual content takes precedence over markdown

### Integration:
1. **Voice Flow**: Speech content enhances TTS experience
2. **Visual Feedback**: Status cards show operation progress
3. **Documentation**: Markdown provides detailed information

## Future Enhancements

### Possible Extensions:
- **Audio Content**: Direct audio file sections
- **Image Content**: Base64 image embedding
- **Interactive Content**: Form components for user input
- **Animation Content**: Motion graphics and transitions

### Backend Integration:
- **TTS Backend**: Process speech content server-side
- **Content Validation**: Verify structured format compliance
- **Performance Optimization**: Cache parsed content

This implementation provides a robust foundation for multi-modal agent responses while maintaining compatibility with existing functionality and streaming capabilities.
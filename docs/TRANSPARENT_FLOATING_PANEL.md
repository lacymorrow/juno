# Transparent Floating Panel System

## Overview

The Transparent Floating Panel is a revolutionary interface component that reimagines the Juno AI app as an elegant, transparent overlay that floats above the macOS desktop. This system provides seamless access to AI capabilities without disrupting the user's workflow.

## Features

### 🎨 **Visual Design**

- **Glass Effect**: Beautiful backdrop blur with rgba transparency
- **Dynamic Sizing**: Automatically adjusts based on content and activity
- **Color-coded Status**: Visual feedback for different states
  - 🔵 Blue: Agent thinking/responding
  - 🟠 Orange: Dictation mode active
  - 🟣 Purple: Text-to-speech playing
- **Smooth Animations**: Fluid transitions between modes and states

### 🖱️ **Interaction Modes**

#### **Compact Mode** (80x40px)

- Minimal footprint showing only status icon
- Click to expand or hover for auto-expansion
- Draggable for repositioning
- Visual hover feedback

#### **Expanded Mode** (350x120px)

- Status display with current activity
- Quick input field for immediate queries
- Control buttons (Chat, Settings, Minimize)
- Audio level visualization during voice input

#### **Chat Mode** (400x300px)

- Full conversation interface
- Message history display
- Text input for typed queries
- Scrollable message list

#### **Settings Mode** (320x200px)

- Panel configuration options
- Opacity adjustment
- Auto-hide preferences
- Voice mode settings

### 🎤 **Real-time Status Integration**

- **Voice Activity**: Shows dictation vs agent listening states
- **Audio Visualization**: Animated bars during voice input
- **Agent Status**: Visual feedback for thinking/responding states
- **TTS Feedback**: Indicates when AI is speaking

## Architecture

### Component Structure

```
src/
├── components/
│   └── TransparentFloatingPanel.tsx    # Main panel component
├── FloatingPanel.tsx                    # Page wrapper component
├── styles/
│   └── globals.css                      # Enhanced glass effects
└── main.tsx                            # Routing configuration
```

### Window Configuration

```json
// src-tauri/tauri.conf.json
{
  "label": "floating-panel",
  "url": "/floating-panel",
  "width": 80,
  "height": 40,
  "decorations": false,
  "transparent": true,
  "alwaysOnTop": true,
  "resizable": false,
  "skipTaskbar": true,
  "visible": true,
  "shadow": false,
  "x": 20,
  "y": 20
}
```

## Usage Guide

### Starting the Panel

1. **Launch Application**:

   ```bash
   bun run tauri dev
   ```

2. **Locate Panel**: Look for a small transparent glass panel in the top-left corner (20px from edges)

3. **Interact with Panel**:
   - **Click**: Expand from compact mode
   - **Hover**: Auto-expand after brief delay
   - **Drag**: Click and drag to reposition anywhere on screen

### Panel Interactions

#### **Compact Mode**

- Single click expands to full interface
- Hover triggers auto-expansion
- Drag to move panel position
- Shows current status via icon color/animation

#### **Expanded Mode**

- **Quick Input**: Type queries directly and press "Ask"
- **Chat Button** (💬): Open full chat interface
- **Settings Button** (⚙️): Access panel configuration
- **Minimize Button** (📐): Return to compact mode

#### **Chat Mode**

- View conversation history
- Type new messages
- Send button or Enter to submit
- Back button returns to expanded mode

#### **Settings Mode**

- Adjust panel opacity
- Configure auto-hide behavior
- Set voice mode preferences
- Back button returns to expanded mode

### Voice Integration

The panel automatically responds to voice events:

- **Dictation Mode** (Option+Space): Orange glow, typing icon
- **Agent Mode** (Option+D): Blue glow, microphone icon
- **Audio Levels**: Animated bars show input volume
- **TTS Playback**: Purple glow during speech output

## Development Guide

### Key Components

#### **TransparentFloatingPanel.tsx**

Main component managing all panel functionality:

```typescript
interface PanelState {
  mode: "compact" | "expanded" | "chat" | "settings";
  agentStatus: "idle" | "listening" | "thinking" | "responding" | "error";
  voiceMode: "dictation" | "agent" | "idle";
  isListening: boolean;
  isTranscribing: boolean;
  isSpeaking: boolean;
  transcriptionText?: string;
  currentResponse?: string;
  error?: string;
  audioLevel: number;
}
```

#### **Event Listeners**

The panel listens for these Tauri events:

```typescript
// Agent Events
"agent-started" | "agent-thinking" | "agent-responding"

// Voice Events  
"dictation-active" | "app-dictation-started" | "app-dictation-finished"

// Transcription Events
"dictation-transcription-partial" | "dictation-transcription-final"

// AI Response Events
"streaming-text" | "stream-end"

// Audio Events
"audio-level" | "tts-started" | "tts-finished"
```

#### **Drag System**

Implements smooth dragging with global mouse events:

```typescript
const handleMouseDown = (e: React.MouseEvent) => {
  // Detect draggable areas
  // Set drag state and offset
};

useEffect(() => {
  // Global mouse move/up listeners
  // Smooth position updates
}, [isDragging, dragOffset]);
```

### CSS Classes

#### **Glass Effects**

```css
.glass-panel-dark {
  background: rgba(0, 0, 0, 0.2);
  backdrop-filter: blur(24px);
  border: 1px solid rgba(255, 255, 255, 0.05);
  box-shadow: 0 12px 40px 0 rgba(0, 0, 0, 0.5);
}
```

#### **Status Glows**

```css
.panel-glow-blue    /* Agent active */
.panel-glow-orange  /* Dictation mode */
.panel-glow-purple  /* TTS speaking */
```

#### **Animations**

```css
.audio-bar          /* Voice level visualization */
.panel-transition   /* Mode change animations */
.draggable-panel    /* Drag state management */
```

### Adding New Features

#### **1. New Panel Mode**

```typescript
// Add to PanelState interface
mode: "compact" | "expanded" | "chat" | "settings" | "newmode";

// Add mode case in render
{panelState.mode === "newmode" && (
  <div className="w-full h-full p-3">
    {/* New mode content */}
  </div>
)}

// Update getPanelDimensions()
case "newmode":
  return { width: 300, height: 200 };
```

#### **2. New Event Listener**

```typescript
useEffect(() => {
  const setupListeners = async () => {
    listeners.push(
      await listen("new-event", (event) => {
        setPanelState(prev => ({ 
          ...prev, 
          newProperty: event.payload 
        }));
      })
    );
  };
}, []);
```

#### **3. New Status Indicator**

```typescript
const getStatusIcon = () => {
  if (panelState.newStatus) {
    return <NewIcon className="h-4 w-4 text-new-color animate-pulse" />;
  }
  // ... existing logic
};
```

## Configuration Options

### Panel Positioning

```typescript
// Default position (top-left)
defaultPosition={{ x: 20, y: 20 }}

// Custom positioning
defaultPosition={{ x: 100, y: 50 }}
```

### Opacity Settings

```typescript
// Default opacity
opacity={0.92}

// More transparent
opacity={0.7}

// Less transparent  
opacity={0.98}
```

### Size Constraints

```typescript
// Maximum width
maxWidth={400}

// Custom dimensions in getPanelDimensions()
case "expanded":
  return { width: 400, height: 150 };
```

## Troubleshooting

### Common Issues

#### **Panel Not Visible**

- Check `tauri.conf.json` has `"visible": true` for floating-panel
- Verify panel position is within screen bounds
- Ensure no other windows are covering the panel

#### **Dragging Not Working**

- Verify `data-draggable` attributes on draggable elements
- Check event propagation isn't being stopped incorrectly
- Ensure `pointer-events: auto` is set on panel

#### **Buttons Not Clickable**

- Add `stopPropagation()` to button click handlers
- Verify `cursor-pointer` class is applied
- Check z-index stacking order

#### **Auto-expansion Issues**

- Verify hover state management
- Check useEffect dependencies for expansion logic
- Ensure timer cleanup in useEffect return

### Debug Mode

Enable debug information by adding to component:

```typescript
// Add debug state display
{process.env.NODE_ENV === 'development' && (
  <div className="absolute top-0 right-0 text-xs bg-black/50 p-1">
    Mode: {panelState.mode} | Hover: {isHovered.toString()}
  </div>
)}
```

## Performance Considerations

### Optimization Tips

1. **Event Listener Cleanup**: Always return cleanup functions from useEffect
2. **Drag Performance**: Use `will-change: transform` for smooth dragging
3. **Memory Management**: Limit message history to prevent memory leaks
4. **Animation Performance**: Use CSS transforms instead of layout changes

### Resource Usage

- **Memory**: ~2-5MB for panel component
- **CPU**: Minimal when idle, <1% during animations
- **GPU**: Hardware-accelerated backdrop blur effects

## Future Enhancements

### Planned Features

1. **Multi-monitor Support**: Detect and constrain to active monitor
2. **Gesture Controls**: Swipe gestures for mode switching
3. **Keyboard Shortcuts**: Global hotkeys for panel control
4. **Theme Customization**: User-selectable color schemes
5. **Plugin System**: Extensible panel modes via plugins

### API Extensions

```typescript
// Planned panel API
interface PanelAPI {
  setMode(mode: PanelMode): void;
  setPosition(x: number, y: number): void;
  addCustomMode(mode: CustomPanelMode): void;
  registerEventHandler(event: string, handler: Function): void;
}
```

## Contributing

### Development Setup

1. **Clone Repository**: Standard Juno development setup
2. **Install Dependencies**: `bun install`
3. **Start Development**: `bun run tauri dev`
4. **Test Panel**: Look for floating panel in top-left corner

### Code Style

- Use TypeScript for all new code
- Follow existing naming conventions
- Add proper event cleanup in useEffect
- Include accessibility attributes where appropriate

### Testing

```bash
# Run development build
bun run tauri dev

# Test interactions
# 1. Drag panel around screen
# 2. Click to expand/collapse
# 3. Test all button interactions
# 4. Verify voice event integration
```

---

*This documentation covers the complete Transparent Floating Panel system. For questions or contributions, refer to the main Juno documentation or create an issue in the repository.*

# Floating Panel 3D Shelf Effect Documentation

## Overview

The Juno floating panel features a sophisticated 3D "shelf" effect that makes the panel appear to be stored away on a virtual shelf when not in use, and smoothly pulled forward when hovered or active. This creates an intuitive and visually appealing user experience.

## Architecture

### Components

1. **FloatingPanel.tsx** - Main container with 3D perspective and Rust hover detection
2. **TransparentFloatingPanel.tsx** - Panel content with modes and interactions
3. **Rust Tracking System** - Native macOS hover detection via Cocoa APIs
4. **CSS 3D Transforms** - Hardware-accelerated animations

## 3D Shelf Effect Implementation

### CSS 3D Transform System

The shelf effect uses CSS 3D transforms with hardware acceleration:

```css
/* Parent container provides 3D perspective */
perspective: "1200px"
perspectiveOrigin: "center bottom"

/* Shelf stored state (not hovered) */
transform: "rotateX(-12deg) translateY(-8px) translateZ(-50px) scale(0.95)"
opacity: 0.85
filter: "blur(0.5px)"

/* Shelf active state (hovered) */
transform: "rotateX(0deg) translateY(0px) translateZ(0px) scale(1)"
opacity: 1
filter: "blur(0px)"
```

### Animation Properties

- **Duration**: 700ms with `ease-out` timing
- **Transform Origin**: `center bottom` for natural shelf-like rotation
- **Hardware Acceleration**: Uses `translateZ` for GPU optimization
- **Smooth Transitions**: `cubic-bezier(0.4, 0, 0.2, 1)` for professional feel

## Rust-Based Hover Detection

### Native macOS Integration

The system uses Cocoa's `NSTrackingArea` for precise window hover detection:

```rust
// Location: src-tauri/src/lib.rs
pub fn setup_tracking_area(window: &tauri::WebviewWindow<tauri::Wry>, app_handle: AppHandle)
```

### Event Flow

1. **Tracking Area Setup**: Native Cocoa tracking area created for window
2. **Mouse Events**: `mouseEntered` and `mouseExited` delegate methods
3. **Event Emission**: Rust emits `mouse-entered-window` and `mouse-left-window`
4. **React Listeners**: Frontend listens for events and updates hover state

### Advantages Over JavaScript

- **Works when window unfocused**: Detects hover even without focus
- **System-level precision**: Native OS integration for accurate detection
- **Performance**: No JavaScript polling or DOM event overhead
- **Reliability**: Consistent behavior across different window states

## Window Management System

### Delayed Resizing Strategy

To prevent visual clipping during 3D animations, window resizing is delayed:

```typescript
// Delay window resize to allow CSS transitions to complete
const resizeTimer = setTimeout(async () => {
  const dimensions = getWindowDimensions();
  await appWindow?.setSize(new LogicalSize(dimensions.width, dimensions.height));
}, 750); // 50ms longer than 700ms CSS transition
```

### Window Size Calculation

```typescript
// Panel dimensions based on mode
const getPanelDimensions = () => {
  switch (panelState.mode) {
    case "compact": return { width: 160, height: 80 };
    case "expanded": return { width: 350, height: 120 };
    case "chat": return { width: 400, height: 300 };
    case "settings": return { width: 320, height: 200 };
  }
};

// Window dimensions (panel + padding)
const getWindowDimensions = () => {
  const panel = getPanelDimensions();
  return {
    width: panel.width + 24,  // p-3 = 12px padding on all sides
    height: panel.height + 24
  };
};
```

### Initial Window Configuration

```json
// src-tauri/tauri.conf.json
{
  "label": "floating-panel",
  "width": 450,    // Large enough for transitions
  "height": 350,   // Accommodates largest panel mode
  "decorations": false,
  "transparent": true,
  "alwaysOnTop": true,
  "shadow": false  // No shadow for clean 3D effect
}
```

## Panel Modes and Transitions

### Mode Hierarchy

1. **Compact** (160×80) - Minimal presence, just status icon
2. **Expanded** (350×120) - Quick controls and status
3. **Chat** (400×300) - Full conversation interface
4. **Settings** (320×200) - Configuration options

### Auto-Expansion Logic

```typescript
const hasActivity = 
  panelState.isListening ||
  panelState.isTranscribing ||
  panelState.isSpeaking ||
  panelState.agentStatus !== "idle";

// Expand on activity or hover
if ((hasActivity || isHovered) && panelState.mode === "compact") {
  setPanelState(prev => ({ ...prev, mode: "expanded" }));
}

// Auto-collapse after 2.5s delay (accounts for transition + resize time)
setTimeout(() => {
  setPanelState(prev => ({ ...prev, mode: "compact" }));
}, 2500);
```

## Enhanced Visual Effects

### Layered 3D Depth

The panel uses multiple transform layers for enhanced depth:

```typescript
// Parent container (FloatingPanel.tsx)
transform: isHovered 
  ? "rotateX(0deg) translateY(0px) translateZ(0px) scale(1)"
  : "rotateX(-12deg) translateY(-8px) translateZ(-50px) scale(0.95)"

// Content layer (TransparentFloatingPanel.tsx)
transform: isWindowHovered 
  ? "translateZ(5px)" 
  : "translateZ(0px)"
```

### State-Based Styling

Different visual states based on panel activity:

```typescript
const getBackgroundColor = () => {
  if (panelState.voiceMode === "dictation") {
    return "bg-gradient-to-br from-orange-500/20 to-orange-600/30";
  }
  if (panelState.voiceMode === "agent" || panelState.agentStatus !== "idle") {
    return "bg-gradient-to-br from-blue-500/20 to-blue-600/30";
  }
  if (panelState.isSpeaking) {
    return "bg-gradient-to-br from-purple-500/20 to-purple-600/30";
  }
  return "bg-gradient-to-br from-gray-800/20 to-gray-900/30";
};
```

## Performance Optimizations

### Hardware Acceleration

- **GPU Transforms**: All 3D transforms use `translateZ` for GPU acceleration
- **Will-Change**: CSS `will-change: transform, opacity, filter` for optimization
- **Backface Visibility**: `backface-visibility: hidden` prevents rendering issues

### Memory Management

- **Event Cleanup**: Proper cleanup of Rust event listeners
- **Timer Management**: Cleanup of setTimeout timers on unmount
- **State Optimization**: Minimal re-renders with targeted state updates

### Transition State Tracking

```typescript
const [isTransitioning, setIsTransitioning] = useState(false);

// Prevents unnecessary operations during animations
setIsTransitioning(true);
setTimeout(() => {
  // Resize window after animation
  setIsTransitioning(false);
}, 750);
```

## Accessibility Features

### Keyboard Navigation

- **Tab Order**: Proper tab order through interactive elements
- **Focus Management**: Focus states for all interactive components
- **Escape Key**: Integrated with Juno's global escape key system

### Screen Reader Support

- **ARIA Labels**: Proper labeling for all interactive elements
- **Role Attributes**: Semantic roles for panel sections
- **State Announcements**: Screen reader announcements for mode changes

## Troubleshooting

### Common Issues

1. **Animation Clipping**: Ensure window size is large enough during transitions
2. **Hover Detection**: Verify Rust tracking area is properly initialized
3. **Performance**: Check for excessive re-renders during transitions

### Debug Mode

Enable debug logging:

```bash
RUST_LOG=debug bun run tauri dev
```

### Event Monitoring

Monitor hover events in browser console:

```typescript
// Add to FloatingPanel.tsx for debugging
useEffect(() => {
  console.log("Hover state changed:", isHovered);
}, [isHovered]);
```

## Future Enhancements

### Planned Features

1. **Customizable Shelf Angle**: User-configurable rotation degrees
2. **Multiple Shelf Positions**: Top, bottom, left, right shelf orientations
3. **Physics-Based Animation**: Spring physics for more natural movement
4. **Gesture Support**: Trackpad gesture integration for macOS

### Performance Improvements

1. **Intersection Observer**: More efficient hover detection
2. **Animation Batching**: Batch multiple state changes
3. **Memory Pooling**: Reuse animation objects

## Integration Points

### Voice System Integration

The panel automatically responds to voice system events:

- **Dictation Mode**: Orange glow and expanded state
- **Agent Mode**: Blue glow and listening indicators
- **TTS Active**: Purple glow and speaking animations

### Memory System Integration

According to a memory from a past conversation, the app has two modes:

- **Dictation mode**: Outputs dictated text to current focus (spacebar)
- **Agent mode**: Triggers AI agent with dictated input (Option+D)

The floating panel visually represents these modes with appropriate colors and animations.

## Code References

### Key Files

- `src/FloatingPanel.tsx` - Main container with 3D perspective
- `src/components/TransparentFloatingPanel.tsx` - Panel content and modes
- `src-tauri/src/lib.rs` - Rust hover detection system
- `src-tauri/src/constants.rs` - Window label constants
- `src/styles/globals.css` - 3D effect CSS classes

### CSS Classes

- `.floating-panel-shelf-effect` - Main 3D container
- `.shelf-stored` - Hidden/stored state
- `.shelf-active` - Active/visible state
- `.glass-panel-dark` - Glass morphism effect
- `.panel-glow-*` - State-based glow effects

This documentation provides a complete reference for understanding, maintaining, and extending the floating panel's 3D shelf effect system.

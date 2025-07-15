# Juno Tray Menu Improvements

## Overview

The Juno application now features a comprehensive tray menu system with dynamic icon states and state-aware functionality. The tray menu automatically updates to reflect the current application state, providing users with visual feedback about what the application is doing.

## Features

### 🎨 Dynamic Icon States

The tray icon changes to reflect different application states:

- **Default** (`🔷 Juno - Ready`): Standard blue icon when the app is idle
- **Agent Active** (`🤖 Juno - Agent Active`): Blue-tinted icon when the AI agent is executing commands
- **Dictation Active** (`🎤 Juno - Dictation Active`): Orange-tinted icon when dictation mode is active
- **Always Listening** (`👂 Juno - Always Listening`): Green-tinted icon when always listening mode is enabled
- **Processing** (`⚙️ Juno - Processing`): Animated/processing icon during transcription, thinking, etc.
- **Error** (`❌ Juno - Error`): Red-tinted icon when there's an error state

### 📋 Enhanced Menu Items

The tray menu includes the following options:

- **Show/Hide Juno** - Toggle main application window
- **New Chat** (⌘N) - Start a new conversation
- **Show/Hide Floating Bar** (⌘B) - Toggle the floating control bar
- **Developer Tools** (⌘⌥I) - Open development tools

#### Voice Control Information

- **Agent Mode** (⌥D) - Information about agent mode shortcut
- **Dictation Mode** - Information about dictation mode shortcut  
- **Stop Current Task** (Escape) - Information about emergency stop

#### System Actions

- **Settings...** (⌘,) - Open application settings
- **Quit Juno** (⌘Q) - Exit the application

### 🔄 Automatic State Monitoring

The tray icon automatically updates based on application events:

- Listens for `agent-active` events
- Listens for `dictation-active` events  
- Listens for `always-listening-mode-changed` events
- Listens for `floating-bar-state-changed` events for processing states
- Automatically determines appropriate state when one activity ends

## Technical Implementation

### Icon System

The tray menu uses embedded PNG icons stored directly in the binary:

```rust
// State-specific tray icons
const TRAY_ICON_DEFAULT: &[u8] = include_bytes!("../../icons/tray/32x32.png");
const TRAY_ICON_AGENT_ACTIVE: &[u8] = include_bytes!("../../icons/tray/32x32-agent.png");
const TRAY_ICON_DICTATION_ACTIVE: &[u8] = include_bytes!("../../icons/tray/32x32-dictation.png");
const TRAY_ICON_ALWAYS_LISTENING: &[u8] = include_bytes!("../../icons/tray/32x32-listening.png");
const TRAY_ICON_ERROR: &[u8] = include_bytes!("../../icons/tray/32x32-error.png");
const TRAY_ICON_PROCESSING: &[u8] = include_bytes!("../../icons/tray/32x32-processing.png");
```

### State Management

The `TrayIconManager` handles dynamic icon changes:

```rust
pub struct TrayIconManager {
    tray_icon: Option<TrayIcon<tauri::Wry>>,
    current_state: TrayIconState,
}
```

### API Usage

#### Manual State Updates

```rust
use crate::menu::tray_menu;

// Set specific states
tray_menu::set_agent_active().await;
tray_menu::set_dictation_active().await;
tray_menu::set_always_listening().await;
tray_menu::set_processing().await;
tray_menu::set_error().await;
tray_menu::set_default().await;

// Or use the generic function
tray_menu::set_tray_icon_state(TrayIconState::AgentActive).await;
```

#### Event-Based Updates

The system automatically monitors these events:

- `agent-active` - Updates to Agent Active or Default state
- `dictation-active` - Updates to Dictation Active or Default state  
- `always-listening-mode-changed` - Updates to Always Listening or Default state
- `floating-bar-state-changed` - Updates to Processing or Error states

### State Priority

When multiple states could be active simultaneously, the system uses this priority order:

1. **Error** - Takes precedence over all other states
2. **Agent Active** - High priority for active AI operations
3. **Dictation Active** - High priority for active voice input
4. **Always Listening** - Medium priority for background listening
5. **Processing** - Low priority for background processing
6. **Default** - Fallback when no other states are active

## Icon Customization

To customize the tray icons for different states:

1. Create your icon variants in `src-tauri/icons/tray/`:
   - `32x32.png` - Default state
   - `32x32-agent.png` - Agent active state
   - `32x32-dictation.png` - Dictation active state
   - `32x32-listening.png` - Always listening state
   - `32x32-error.png` - Error state  
   - `32x32-processing.png` - Processing state

2. The icons should be 32x32 pixels in PNG format
3. Icons are embedded at compile time, so you'll need to rebuild after changes

## Best Practices

### Design Guidelines

- Use consistent visual language across all icon states
- Ensure icons are clearly distinguishable at small sizes
- Consider accessibility with sufficient contrast
- Use color coding that matches user expectations:
  - Blue: Default/neutral
  - Orange: Active input/dictation
  - Green: Listening/monitoring
  - Red: Error/warning
  - Gray: Processing/thinking

### Performance Considerations

- Icons are embedded in the binary, so keep file sizes reasonable
- State changes are debounced to prevent rapid flickering
- The system only updates the icon when the state actually changes

### Integration Tips

- The tray menu integrates with Tauri's event system
- State changes emit events that other parts of the app can listen to
- The floating bar and tray menu stay synchronized automatically
- Menu items trigger standard application events for consistency

## Future Enhancements

Potential improvements for the tray menu system:

1. **Animated Icons** - Support for animated GIF or multi-frame icons for processing states
2. **Badge Notifications** - Overlay badges for counts (e.g., number of tasks in queue)
3. **Context-Sensitive Menus** - Different menu items based on current state
4. **Themes** - Light/dark mode icon variants
5. **Custom Tooltips** - More detailed status information in tooltips
6. **Progress Indicators** - Visual progress for long-running operations

## Related Files

- `src-tauri/src/menu/tray_menu.rs` - Main tray menu implementation
- `src-tauri/src/constants/menus.rs` - Menu item constants  
- `src-tauri/icons/tray/` - Tray icon assets
- `src-tauri/src/state_management.rs` - Application state management
- `src-tauri/src/commands/floating_bar.rs` - Floating bar state integration

---

This improved tray menu system provides a much better user experience by giving immediate visual feedback about application state and offering convenient access to key functionality right from the system tray.

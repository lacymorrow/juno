# Microphone Permission API for Voice Transcription Plugin

This document describes the new AVAudioSession-based microphone permission API added to the voice transcription plugin.

## Overview

The voice transcription plugin now includes direct AVAudioSession integration for proper microphone permission handling on macOS. This ensures that:

1. Permission is checked before attempting to record
2. Users are prompted with the native macOS permission dialog when needed
3. Permission state is cached for performance
4. Audio session is properly initialized after permission is granted

## New Commands

### `check_microphone_permission`

Checks the current microphone permission status without prompting the user.

```typescript
import { invoke } from '@tauri-apps/api/core';

const status = await invoke('plugin:voice-transcription|check_microphone_permission');
// Returns: "granted" | "denied" | "undetermined" | "not_applicable"
```

### `request_microphone_permission`

Requests microphone permission from the user if not already granted.

```typescript
import { invoke } from '@tauri-apps/api/core';

const status = await invoke('plugin:voice-transcription|request_microphone_permission');
// Returns: "granted" | "denied" | "undetermined" | "not_applicable"
```

### `ensure_microphone_ready`

Ensures microphone is ready for use (checks hardware, permission, and initializes audio session).

```typescript
import { invoke } from '@tauri-apps/api/core';

try {
  await invoke('plugin:voice-transcription|ensure_microphone_ready');
  // Microphone is ready to use
} catch (error) {
  // Handle error (permission denied, no hardware, etc.)
}
```

## Automatic Permission Checking

The plugin automatically checks microphone permission when:

1. **Plugin Initialization**: Logs permission status during startup
2. **Start Dictation**: Checks permission before starting voice recording
3. **Start Always Listening**: Checks permission before enabling always-on mode

If permission is denied, these operations will fail with a `MicrophonePermissionDenied` error.

## Error Handling

When microphone permission is denied, the plugin returns:
```
Error: Microphone permission denied. Please grant microphone access in System Settings > Privacy & Security > Microphone
```

## Frontend Integration Example

```typescript
// Check permission before starting voice features
async function startVoiceFeature() {
  try {
    // Check current permission status
    const status = await invoke('plugin:voice-transcription|check_microphone_permission');
    
    if (status === 'denied') {
      // Show user a message about granting permission
      alert('Microphone access is required. Please grant permission in System Settings.');
      return;
    }
    
    if (status === 'undetermined') {
      // Request permission
      const newStatus = await invoke('plugin:voice-transcription|request_microphone_permission');
      if (newStatus !== 'granted') {
        alert('Microphone permission was not granted.');
        return;
      }
    }
    
    // Permission is granted, start voice feature
    await invoke('plugin:voice-transcription|start_dictation');
  } catch (error) {
    console.error('Failed to start voice feature:', error);
  }
}
```

## Technical Details

### macOS Implementation

- Uses `AVAudioSession` for permission checking and requests
- Implements proper completion handlers for async permission requests
- Caches permission state to avoid repeated system calls
- Initializes audio session with `AVAudioSessionCategoryPlayAndRecord`

### Permission States

- **Granted**: User has granted microphone access
- **Denied**: User has explicitly denied microphone access
- **Undetermined**: User has not been asked for permission yet
- **Not Applicable**: Non-macOS platform where permissions are handled differently

### Hardware Detection

The plugin also checks for microphone hardware availability using:
- `system_profiler SPAudioDataType`
- `ioreg` for audio device detection
- Fallback assumption that modern Macs have built-in microphones

## Migration Notes

The existing permission system at the app level (in `src-tauri/src/commands/permissions.rs`) continues to work and provides a complementary approach. The plugin-level checks are more accurate for voice-specific features.

## Security Considerations

1. Permission is requested on the main thread as required by macOS
2. No admin privileges are required
3. Users maintain full control through System Settings
4. Permission state is respected throughout the app lifecycle
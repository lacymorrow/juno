# Integration Guide: Using the Voice Transcription Plugin in Juno

This guide shows how to migrate Juno to use the extracted voice transcription plugin.

## 1. Update Cargo.toml

Replace the direct dependencies with the plugin:

```toml
# Remove these dependencies (they're now in the plugin):
# whisper-rs = "0.11.0"
# cpal = "0.15"
# rubato = "0.14.1"
# hound = "3.5"

# Add the plugin:
[dependencies]
tauri-plugin-voice-transcription = { path = "./tauri-plugin-voice-transcription" }
```

## 2. Update src/lib.rs

Remove the voice_control module and update the initialization:

```rust
// Remove:
// pub mod voice_control;
// use voice_control::VoiceController;

// In the run() function, replace VoiceController initialization with:
tauri::Builder::default()
    .plugin(tauri_plugin_voice_transcription::init())
    // ... rest of your plugins
```

## 3. Remove Voice Control Files

Delete these files as they're now in the plugin:
- `src-tauri/src/voice_control.rs`
- `src-tauri/src/commands/voice_control.rs`

## 4. Update Frontend Code

Replace the direct Tauri invokes with plugin API:

```typescript
// Before:
import { invoke } from '@tauri-apps/api/core';
await invoke('start_dictation_command');

// After:
import { startDictation } from 'tauri-plugin-voice-transcription-api';
await startDictation();
```

## 5. Update Event Listeners

Update event names to use the plugin's event namespace:

```typescript
// Before:
listen('app-dictation-started', handler);
listen('app-dictation-partial-result', handler);

// After:
listen('voice-transcription:dictation-started', handler);
listen('voice-transcription:partial-result', handler);
```

## 6. Update Global Shortcuts

The global shortcut handler needs to use the plugin commands:

```rust
// In the global shortcut handler:
if shortcut == &dictation_toggle_shortcut && event.state() == ShortcutState::Pressed {
    tauri::async_runtime::spawn(async move {
        // Use the plugin command
        if let Err(e) = app_handle_clone.invoke("plugin:voice-transcription|toggle_dictation", ()) {
            tracing::error!("Failed to toggle dictation: {}", e);
        }
    });
}
```

## 7. Update Configuration

Add plugin configuration to `tauri.conf.json`:

```json
{
  "plugins": {
    "voice-transcription": {
      "modelPath": "models/ggml-tiny.en.bin"
    }
  }
}
```

## 8. Model Path Environment Variable

The plugin uses configuration instead of environment variables. Update any code that sets `VOICE_MODEL_PATH` to use the plugin's configuration or the `setModelPath` API.

## Benefits of Using the Plugin

1. **Reusability**: Can be used in other Tauri apps
2. **Separation of Concerns**: Voice functionality is isolated
3. **Easier Testing**: Plugin can be tested independently
4. **Better API**: TypeScript bindings provide type safety
5. **Standardized Events**: Consistent event naming with plugin namespace 

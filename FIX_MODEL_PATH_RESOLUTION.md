# Fix Model Path Resolution

## Problem
The model is being bundled at:
```
Contents/Resources/_up_/tauri-plugin-voice-transcription/models/ggml-tiny.en.bin
```

But the code was looking for it at:
```
Contents/Resources/models/ggml-tiny.en.bin
```

## Solution Applied
Updated `tauri-plugin-voice-transcription/src/utils.rs` to check for the actual bundled path:
- `_up_/tauri-plugin-voice-transcription/models/ggml-tiny.en.bin`

## To Apply the Fix

1. Rebuild the voice transcription plugin:
   ```bash
   cd tauri-plugin-voice-transcription
   cargo build --release
   cd ..
   ```

2. Rebuild the application:
   ```bash
   bun run tauri build
   ```

The model should now be found in the production build.

## Why This Happens
Tauri bundles resources with the `_up_` prefix and preserves the relative path structure from where they're sourced. Since we're bundling from `../tauri-plugin-voice-transcription/models/*`, it creates the full path structure in the bundle.
# Voice Transcription Production Build Fix

## Issue Summary

**Error**: `Failed to toggle dictation: Initialization error: Voice transcription is not available. Initialization failed: Model not found: models/ggml-tiny.en.bin`

**Problem**: In production builds, the Whisper model file `models/ggml-tiny.en.bin` was not being found by the voice transcription plugin's path resolution system.

## Root Cause Analysis

1. **Bundled Resources Structure**: In production builds, Tauri bundles resources into a `_up_` directory structure within the app's resource directory.

2. **Inconsistent Path Resolution**: The voice transcription plugin's `resolve_model_path` function wasn't checking for the `_up_` directory pattern that other bundled resources (like sound files and environment files) use.

3. **Bundle Configuration**: The model files are correctly bundled via the configuration in `src-tauri/tauri.conf.json`:
   ```json
   "resources": [
     "../tauri-plugin-voice-transcription/models/*",
     "Info.plist",
     "../public/sounds/**/*",
     "../.env"
   ]
   ```

4. **Path Resolution Gap**: The original code only tried:
   - `app.path().resolve(model_path, tauri::path::BaseDirectory::Resource)`
   
   But should also try the `_up_` pattern used by other bundled resources:
   - `resource_dir.join("_up_").join(model_path)`

## Solution Implemented

### Modified File: `tauri-plugin-voice-transcription/src/utils.rs`

Enhanced the `resolve_model_path` function to include comprehensive bundled resource path checking:

```rust
// Strategy 1: Try bundled resources (production apps)
tracing::info!("Strategy 1: Checking bundled resources...");

// First try the direct resource resolution
if let Ok(resource_path) = app.path().resolve(model_path, tauri::path::BaseDirectory::Resource) {
    // ... existing logic
}

// Try the _up_ directory pattern used by other bundled resources in production
if let Ok(resource_dir) = app.path().resource_dir() {
    let possible_bundled_paths = vec![
        // Primary bundled path in production builds (_up_ directory)
        resource_dir.join("_up_").join(model_path),
        resource_dir.join("_up_").join("tauri-plugin-voice-transcription").join(model_path),
        // Try without the models prefix in case bundling flattens structure
        if model_path.starts_with("models/") {
            resource_dir.join("_up_").join(&model_path[7..])
        } else {
            resource_dir.join("_up_").join(model_path)
        },
        // Legacy direct paths for backward compatibility
        resource_dir.join(model_path),
        resource_dir.join("tauri-plugin-voice-transcription").join(model_path),
    ];

    for test_path in possible_bundled_paths.iter() {
        if test_path.exists() {
            return test_path.to_string_lossy().to_string();
        }
    }
}
```

### Key Improvements

1. **Production Build Compatibility**: Added support for the `_up_` directory pattern used in production builds
2. **Multiple Path Strategies**: Tries various possible bundled resource locations
3. **Better Logging**: Enhanced tracing to help debug path resolution issues
4. **Backward Compatibility**: Maintains support for existing development and legacy deployment patterns

### Pattern Consistency

This fix aligns the voice transcription plugin with how other bundled resources are handled in the main Juno codebase:

- **Sound files** (`src-tauri/src/commands/sound.rs`): Uses `resource_dir.join("_up_").join(file_path)`
- **Environment files** (`src-tauri/src/lib.rs`): Uses `resource_dir.join("_up_").join(".env")`
- **Voice models** (now): Uses the same `_up_` pattern

## Testing Verification

The fix addresses the specific error by ensuring the model file can be found in production builds through multiple path resolution strategies:

1. Direct resource resolution (existing)
2. `_up_` directory pattern (new - production builds)
3. App data directory (existing - user-installed models)
4. Development mode paths (existing)
5. Current working directory (existing - fallback)

## Files Modified

- `tauri-plugin-voice-transcription/src/utils.rs` - Enhanced model path resolution

## Expected Result

After this fix, production builds should successfully locate the bundled Whisper model file and initialize voice transcription without the "Model not found" error.

The app will now properly support both Dictation mode (spacebar) and Agent mode (Option+D) in production builds.
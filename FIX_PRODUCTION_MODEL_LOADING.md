# Fix for Production Model Loading Issue

## Problem

When running the built application, the Whisper model file (`ggml-tiny.en.bin`) cannot be found, causing voice features to fail with these errors:

```
Failed to resolve resource path
Failed to get resource directory
Model file does not exist at resolved path: models/ggml-tiny.en.bin
```

## Root Cause

1. The bundled resources are not being found in the production build
2. The `app.path().resource_dir()` call is failing with "unknown path"
3. The model files are bundled but the path resolution strategy isn't finding them

## Solutions

### Solution 1: Update Bundle Resources Configuration

The current configuration bundles from multiple locations. We need to ensure the models are in the correct location:

```json
// tauri.conf.json
"resources": [
  "models/*",  // Add this to bundle from src-tauri/models/
  "../tauri-plugin-voice-transcription/models/*",
  "Info.plist",
  "../public/sounds/**/*",
  "../.env"
]
```

### Solution 2: Copy Models to Multiple Locations

Before building, ensure models are in all expected locations:

```bash
# Copy models to ensure they're available
cp tauri-plugin-voice-transcription/models/*.bin src-tauri/models/
cp tauri-plugin-voice-transcription/models/*.bin models/
```

### Solution 3: Fix Resource Path Resolution

Update the voice transcription plugin to handle production builds better:

```rust
// In utils.rs, add a fallback for macOS bundle structure
pub fn resolve_model_path<R: Runtime>(app: &tauri::AppHandle<R>, model_path: &str) -> String {
    // ... existing code ...
    
    // Add macOS-specific bundle check
    #[cfg(target_os = "macos")]
    {
        if let Ok(bundle_path) = std::env::current_exe() {
            // Check in Resources directory of the app bundle
            let resources_path = bundle_path
                .parent() // MacOS
                .and_then(|p| p.parent()) // Contents
                .map(|p| p.join("Resources").join(model_path));
                
            if let Some(path) = resources_path {
                if path.exists() {
                    return path.to_string_lossy().to_string();
                }
            }
        }
    }
    
    // ... rest of existing code ...
}
```

### Solution 4: Use Embedded Resources

Instead of file-based resources, embed the model directly:

1. Create a build script to embed the model:

```rust
// build.rs
fn main() {
    // This embeds the model file at compile time
    println!("cargo:rerun-if-changed=models/ggml-tiny.en.bin");
}
```

2. Use include_bytes! to embed the model:

```rust
// In the plugin
const MODEL_BYTES: &[u8] = include_bytes!("../models/ggml-tiny.en.bin");

// Write to temp file when needed
fn get_model_path() -> PathBuf {
    let temp_dir = std::env::temp_dir();
    let model_path = temp_dir.join("ggml-tiny.en.bin");
    
    if !model_path.exists() {
        std::fs::write(&model_path, MODEL_BYTES).expect("Failed to write model");
    }
    
    model_path
}
```

## Recommended Fix

The quickest fix is to:

1. Ensure models are in `src-tauri/models/` directory
2. Update tauri.conf.json to include `"models/*"` in resources
3. Rebuild the application

## Testing

After applying the fix:

1. Build the application: `bun run tauri build`
2. Run the built app from: `src-tauri/target/release/bundle/macos/Juno.app`
3. Check logs for successful model loading

## Long-term Solution

Consider implementing a model downloader that fetches models on first run if they're not bundled, storing them in the app's data directory. This would reduce bundle size and handle missing models gracefully.
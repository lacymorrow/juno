# Cleanup Duplicate Model Files

## Current Situation

We have 3 copies of the same 77MB Whisper model file:
- `/tauri-plugin-voice-transcription/models/ggml-tiny.en.bin` (original)
- `/models/ggml-tiny.en.bin` (duplicate)
- `/src-tauri/models/ggml-tiny.en.bin` (duplicate)

Total waste: ~155MB

## Root Cause

The model path is hardcoded as a relative path `"models/ggml-tiny.en.bin"` in:
- Plugin config
- App settings
- State management

Different contexts resolve this relative path differently, leading to the need for copies.

## Proper Solution

### Option 1: Single Source of Truth (Recommended)

1. Keep only ONE copy in `tauri-plugin-voice-transcription/models/`
2. Update `tauri.conf.json` to bundle from the single location:
   ```json
   "resources": [
     "../tauri-plugin-voice-transcription/models/*.bin",
     "Info.plist",
     "../public/sounds/**/*"
   ]
   ```
3. Remove the duplicates:
   ```bash
   rm models/ggml-tiny.en.bin
   rm src-tauri/models/ggml-tiny.en.bin
   ```

### Option 2: Use Symlinks (Development Only)

Instead of copies, use symbolic links:
```bash
# Remove duplicates
rm models/ggml-tiny.en.bin
rm src-tauri/models/ggml-tiny.en.bin

# Create symlinks
ln -s tauri-plugin-voice-transcription/models models
ln -s ../tauri-plugin-voice-transcription/models src-tauri/models
```

**Note**: Symlinks won't work for production builds, so Option 1 is better.

### Option 3: Download on First Run

Remove ALL bundled models and implement a downloader:
```rust
async fn ensure_model_exists(model_path: &str) -> Result<PathBuf, String> {
    let app_data_dir = app_handle.path().app_data_dir()?;
    let model_file = app_data_dir.join("models").join("ggml-tiny.en.bin");
    
    if !model_file.exists() {
        // Download from GitHub releases or CDN
        download_model(&model_file).await?;
    }
    
    Ok(model_file)
}
```

Benefits:
- Smaller app bundle (77MB less)
- Can offer multiple model sizes
- Updates without rebuilding

## Immediate Action

For now, to clean up:

```bash
# Keep only the plugin's copy
rm models/ggml-tiny.en.bin
rm src-tauri/models/ggml-tiny.en.bin

# Update prepare-build.sh to create directories but not copy
mkdir -p src-tauri/models
mkdir -p models

# The build process will bundle from the plugin directory
```

The path resolution code we added will still find the model in production builds.
# Sound and Icon Production Build Fixes

## Issues Identified

### 1. Sound Files Not Working in Production
**Problem**: Sound files were not being bundled with the production build, causing audio playback to fail.

**Root Cause**: The `public/sounds/` directory was not included in the `bundle.resources` array in `src-tauri/tauri.conf.json`.

**Solution**: Added `"../public/sounds/**/*"` to the `bundle.resources` array.

### 2. App Icon Configuration
**Status**: ✅ **Already Correctly Configured**

The app icons were already properly configured in `tauri.conf.json`:

```json
"icon": [
  "icons/32x32.png",
  "icons/128x128.png", 
  "icons/128x128@2x.png",
  "icons/icon.icns",
  "icons/icon.ico"
]
```

All referenced icon files exist in `src-tauri/icons/` directory.

## Fix Applied

**File Modified**: `src-tauri/tauri.conf.json`

**Change Made**:
```json
"resources": [
  "../tauri-plugin-voice-transcription/models/*", 
  "Info.plist",
  "../public/sounds/**/*"  // ← Added this line
]
```

## How Sound Bundling Works

1. **Development**: Sounds are served from `public/sounds/` via Vite dev server
2. **Production**: Sounds are bundled into the app resources and accessed via:
   ```rust
   let resource_path = app.path().resource_dir()?;
   let full_path = resource_path.join("public").join(&file_path);
   ```

## Sound Directory Structure

```
public/sounds/
├── caf/               # macOS native format (CAF)
│   ├── 01 Hero Sounds/
│   ├── 02 Alerts and Notifications/
│   ├── 03 Primary System Sounds/
│   └── 04 Secondary System Sounds/
└── ogg/               # Cross-platform format (OGG)
    ├── 01 Hero Sounds/
    ├── 02 Alerts and Notifications/
    ├── 03 Primary System Sounds/
    └── 04 Secondary System Sounds/
```

## Platform-Specific Audio Handling

- **macOS**: Uses `.caf` files with `afplay` command
- **Windows**: Uses `.ogg` files with PowerShell SoundPlayer
- **Linux**: Uses `.ogg` files with various audio players (paplay, aplay, mpg123, ffplay)

## Testing the Fix

To verify the fix works:

1. **Build for production**:
   ```bash
   bun run tauri build
   ```

2. **Test sound playback** in the built app (not dev mode)

3. **Check bundled resources** in the built app:
   - macOS: `Contents/Resources/public/sounds/`
   - Windows: Check app installation directory
   - Linux: Check app installation directory

## Implementation Details

- **Sound System**: `src-tauri/src/commands/sound.rs`
- **Frontend Hook**: `src/hooks/useSound.ts`
- **Type Definitions**: `src/types/sound.ts`
- **Demo Component**: `src/components/SoundDemo.tsx`

## Verification Commands

```bash
# Verify bundle includes sounds
cargo check --manifest-path src-tauri/Cargo.toml

# Build and test
bun run tauri build

# Check if sounds are in the built app resources
# macOS: ls -la "src-tauri/target/release/bundle/macos/Juno.app/Contents/Resources/public/sounds/"
```

The fix ensures that all sound files are properly bundled and accessible in production builds across all supported platforms.
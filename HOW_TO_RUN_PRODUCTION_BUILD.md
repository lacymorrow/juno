# How to Run the Production Build

## The Problem
You're running the raw binary:
```
/Users/lacymorrow/repo/juno/target/universal-apple-darwin/release/juno
```

This doesn't have access to the bundled resources (models, sounds, etc.).

## The Solution
Run the actual macOS app bundle instead:

### Option 1: Open the App Bundle (Recommended)
```bash
open /Users/lacymorrow/repo/juno/src-tauri/target/universal-apple-darwin/release/bundle/macos/Juno.app
```

Or if you built for your specific architecture:
```bash
open /Users/lacymorrow/repo/juno/src-tauri/target/release/bundle/macos/Juno.app
```

### Option 2: Run the Binary Inside the Bundle
```bash
/Users/lacymorrow/repo/juno/src-tauri/target/release/bundle/macos/Juno.app/Contents/MacOS/Juno
```

### Option 3: Copy to Applications and Run
```bash
cp -r /Users/lacymorrow/repo/juno/src-tauri/target/release/bundle/macos/Juno.app /Applications/
open /Applications/Juno.app
```

## Why This Matters
The app bundle (`.app`) contains:
- The executable binary
- All bundled resources (models, sounds, icons)
- Info.plist and other metadata
- Proper entitlements for macOS permissions

Running the raw binary skips all of this structure, which is why resources can't be found.

## For Development
If you want to run in development mode with hot reload:
```bash
bun run tauri dev
```

This will handle resource loading differently and work with the source files directly.
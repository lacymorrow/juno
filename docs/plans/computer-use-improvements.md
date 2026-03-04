# Computer Use Improvements Plan

> Generated 2026-03-03 from deep research into the last year of computer use advances.
> Tier 1 is already implemented. Tiers 2-4 are ready for implementation.

## Status

| Tier | Items | Status |
|------|-------|--------|
| Tier 1 | Zoom, Screenshot Limiting, Prompt Caching, ScreenCaptureKit | **DONE** |
| Tier 2 | JPEG Compression, Cursor Metadata, Action Cooldown | Ready |
| Tier 3 | AX Verification, Grounding/Planning Split, Action Batching | Ready |
| Tier 4 | Tiered Permissions, Memory Isolation, Streamable HTTP MCP | Ready |

---

## Tier 1: Completed

### 1.1 Enable Zoom Action (computer_20251124)
- **Files**: `anthropic.rs`, `anthropic_computer_use.rs`
- **What**: Added `enable_zoom: true` to computer tool definition for `computer_20251124`. Implemented full zoom handler that validates `region: [x0, y0, x1, y1]`, captures screenshot, crops to region at native resolution, and returns base64 PNG without downscaling.

### 1.2 Screenshot History Limiting
- **Files**: `anthropic.rs`
- **What**: Added `limit_screenshot_history()` method with `MAX_RECENT_SCREENSHOTS = 3`. Scans tool_result blocks for image content, counts from end, replaces older screenshots with text placeholders. Saves ~7,000 tokens per step.

### 1.3 Prompt Caching
- **Files**: `anthropic.rs`, `constants/api.rs`
- **What**: Converted system prompt from plain string to `SystemContentBlock` with `cache_control: {"type": "ephemeral"}`. Added cache_control to last tool in tools array. Added `prompt-caching-2024-07-31` beta flag. Reduces input token costs ~90%, latency ~50-80%.

### 1.4 ScreenCaptureKit Migration
- **Files**: `mcp-server-os-level/Cargo.toml`, `mcp-server-os-level/src/platforms/macos/utils.rs`
- **What**: Added `screencapturekit` v1.5.1 as optional dep (enabled by default). Created `capture_via_screencapturekit()` using `SCScreenshotManager::capture_image()` → `rgba_data()` → `ImageBuffer`. Unified `capture_display_buffer()` tries SCK first, falls back to legacy `CGDisplay::screenshot()` for macOS < 14.0. All three public screenshot functions now use the unified path.

---

## Tier 2: Quick Wins (< 1 hour each)

### 2.1 JPEG Screenshot Compression
- **Impact**: ~60% bandwidth reduction per screenshot
- **Effort**: Tiny (< 30 min)
- **Risk**: Low

**Current state**: Always PNG via `ImageFormat::Png` in `commands/core.rs:77`. Anthropic API media_type hardcoded to `"image/png"` in `anthropic.rs:1308`.

**Implementation**:
1. In `commands/core.rs` — change `ImageFormat::Png` to `ImageFormat::Jpeg(85)` (quality 85 is visually lossless for UI screenshots)
2. In `anthropic.rs` — change `media_type: "image/png"` to `media_type: "image/jpeg"` in the image content block builder
3. In `mcp-server-os-level/src/platforms/macos/utils.rs` — update `encode_imagebuffer_to_base64_png()` to accept a format parameter, or add `encode_imagebuffer_to_base64_jpeg()`
4. Keep PNG path available for zoom action (lossless crops matter more there)

**Why JPEG is safe**: Anthropic's Computer Use API accepts both PNG and JPEG. Screenshots of UI are mostly flat colors and text — JPEG quality 85 is indistinguishable. Claude's vision works equally well on both formats.

### 2.2 Cursor Position in Tool Results
- **Impact**: Helps Claude orient without re-reading the entire screen
- **Effort**: Tiny (< 30 min)
- **Risk**: None

**Current state**: Cursor position is used internally to find the correct display (`capture_and_encode_screenshot` in `utils.rs:194-202`), but NOT included in tool results sent to the API.

**Implementation**:
1. In `anthropic_computer_use.rs` — after screenshot action, include cursor (x,y) in the tool result text:
   ```
   Screenshot captured. Cursor at (523, 341) in standard coordinates.
   ```
2. Read cursor position from `CGEvent::location()` (already used in `capture_and_encode_screenshot`)
3. Transform to standard coordinates via `coordinates::transform_screen_to_standard_coordinates()`

### 2.3 Action Cooldown / Wait Inference
- **Impact**: Reduces "clicked too fast" failures by 20-30%
- **Effort**: Small (< 1 hour)
- **Risk**: Low

**Current state**: No delay between actions. The agent fires click/type immediately. UI may still be loading/animating.

**Implementation**:
1. In `anthropic_computer_use.rs` — track `last_action_timestamp` (static `AtomicU64` or field on brain)
2. After click/type/key actions, record timestamp
3. Before next action, check elapsed time. If < 500ms, insert a brief sleep (300-500ms)
4. Make cooldown configurable via tool settings
5. Skip cooldown for screenshot/cursor_position actions (read-only, no UI change)

---

## Tier 3: Medium Impact (2-4 hours each)

### 3.1 Accessibility-as-Verification
- **Impact**: Saves ~2-3 seconds per verification step (no screenshot needed)
- **Effort**: Medium (2-3 hours)
- **Risk**: Medium (AX API can be flaky for some apps)

**Current state**: After click/type, the agent takes a full screenshot to verify success. Each screenshot = API call + image encode + token cost. Tool results from click/type only return `{"success": true}` or error.

**Existing infrastructure**: `mcp-server-os-level` already has full AX API access — `MacOSUIElement`, `find_elements`, `get_focused_element`, `get_element_at_position`.

**Implementation**:
1. After a `click` action at (x, y):
   - Query `get_element_at_position(x, y)` via AX API
   - Return element role, title, value, and focused state in tool result
   - Example: `Clicked at (523, 341). Element: AXButton "Submit" — focused: true`
2. After a `type` action:
   - Query focused element's value via AX API
   - Return the current text content
   - Example: `Typed "hello". Focused element: AXTextField value="hello world"`
3. After a `key` action (e.g., Enter, Tab):
   - Query newly focused element
   - Example: `Pressed Enter. Focus moved to: AXButton "Continue"`
4. Make this opt-in via a flag on the computer tool config (some apps have poor AX support)

**Why this is a big deal**: Currently every action costs ~3s (screenshot capture + encode + API round-trip for image tokens). AX verification is ~50ms and returns structured text (cheap tokens). The agent can decide if it needs a fresh screenshot or can proceed based on AX feedback.

### 3.2 Separate Grounding from Planning (Dual-Model)
- **Impact**: 60-80% cost reduction per agent step
- **Effort**: Medium-High (3-4 hours)
- **Risk**: Medium (requires careful prompt engineering)

**Current state**: Every screenshot goes to the same model (Opus/Sonnet) that does both:
- **Grounding**: "Where is the Submit button?" (image → coordinates)
- **Planning**: "What should I do next?" (reasoning → action)

Grounding is mechanical — a cheap model (Haiku) can do it. Planning needs the expensive model.

**Implementation**:
1. Add a `GroundingProvider` trait alongside the existing `AnthropicBrain`
2. Create `HaikuGrounder` that takes a screenshot + query and returns:
   - List of UI elements with bounding boxes
   - Text content visible on screen
   - Current focused element
3. In the agent loop (`decide_next_action_streaming`):
   - Before sending the full conversation to Opus/Sonnet, run the screenshot through `HaikuGrounder`
   - Replace the image content block with the grounding text summary
   - Opus/Sonnet receives text description (cheap tokens) instead of image (expensive tokens)
4. Keep image pass-through available when grounding confidence is low

**Architecture note**: This maps to the "Agent S3" approach — separate grounding model from planning model. Agent S3 achieved 72.6% on OSWorld with this split.

### 3.3 Action Batching
- **Impact**: 15-20% speed improvement for multi-action sequences
- **Effort**: Medium (2-3 hours)
- **Risk**: Low

**Current state**: Each action is one API round-trip. Typing "hello" + pressing Enter = 2 round-trips minimum.

**Implementation**:
1. Detect when Claude returns multiple tool_use blocks in a single response (already supported by the API)
2. Execute them sequentially within a single turn instead of sending each result back individually
3. Batch the results and send them all at once
4. Add a `batch_actions` tool that accepts an array of actions:
   ```json
   {"actions": [
     {"type": "click", "x": 100, "y": 200},
     {"type": "type", "text": "hello"},
     {"type": "key", "key": "Return"}
   ]}
   ```
5. Include action cooldown (Tier 2.3) between batched actions

---

## Tier 4: High Effort, Long-Term (1-2 days each)

### 4.1 Tiered Permission System
- **Impact**: Security hardening (addresses security audit items)
- **Effort**: High (1-2 days)
- **Risk**: Medium (must not break existing workflows)

**Levels**:
1. **Read-only**: Screenshots, AX queries, clipboard read
2. **UI interaction**: Click, type, scroll, key press
3. **File write**: Create/modify files within workspace
4. **Shell execution**: Run shell commands (with whitelist)
5. **System config**: Modify system settings, install software

**Implementation**: Add `PermissionLevel` enum to `AppState`. Each tool checks `required_permission_level()` before executing. Orchestrator can escalate on behalf of user with confirmation.

### 4.2 Cross-Agent Memory Isolation
- **Impact**: Prevents context pollution, enables parallel specialist execution
- **Effort**: High (1 day)
- **Risk**: Low

**Current state**: Specialists get fresh `SimpleMemoryManager::new()` per task. But orchestrator shares one persistent `Arc<TokioMutex<SimpleMemoryManager>>` that accumulates everything.

**Implementation**: Give orchestrator a structured memory with per-specialist summaries instead of raw conversation history. Each specialist returns a structured result object, not raw messages.

### 4.3 Streamable HTTP for MCP
- **Impact**: Better for multi-client (Juno Cloud), supports server push
- **Effort**: High (1-2 days)
- **Risk**: Medium

**Current state**: Dual transport (stdio subprocess + HTTP JSON-RPC). MCP spec now defines Streamable HTTP as the recommended transport.

**Implementation**: Add `StreamableHttpTransport` alongside existing `SubprocessTransport` and `HttpTransport`. Uses SSE for server→client push, HTTP POST for client→server requests.

---

## Implementation Order (Recommended)

```
Week 1:  2.1 (JPEG)  →  2.2 (Cursor)  →  2.3 (Cooldown)     [3 quick wins]
Week 2:  3.1 (AX Verification)                                 [biggest behavioral win]
Week 3:  3.3 (Action Batching)  →  3.2 (Dual-Model)           [speed + cost]
Later:   4.1 → 4.2 → 4.3                                      [as needed]
```

## Key Files Reference

| Area | Primary File(s) |
|------|-----------------|
| Agent loop | `src-tauri/src/agent/providers/anthropic.rs` |
| Computer use tools | `src-tauri/src/agent/tools/anthropic_computer_use.rs` |
| Screenshot capture | `src-tauri/mcp-server-os-level/src/platforms/macos/utils.rs` |
| Screenshot command | `src-tauri/src/commands/core.rs` |
| Coordinate system | `src-tauri/src/utils/coordinates.rs` |
| Orchestrator | `src-tauri/src/commands/orchestrator.rs` |
| MCP integration | `src-tauri/src/agent/tools/mcp_integration.rs` |
| Tool versioning | `src-tauri/src/agent/tools/tool_versioning.rs` |
| API constants | `src-tauri/src/constants/api.rs` |
| AX API | `src-tauri/mcp-server-os-level/src/platforms/macos/` |

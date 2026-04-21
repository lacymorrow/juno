# Competitive Computer Use Audit

**Date:** 2026-04-21
**Author:** Automated audit via Claude Code
**Scope:** Juno vs OpenAI Codex vs Anthropic Computer Use API vs Google Gemini

---

## Executive Summary

Juno has **full action parity** with Anthropic's Computer Use API (17 actions) and **exceeds** Codex on raw capability. However, Juno has significant gaps in **safety/trust UX** — the area Codex invested most heavily in. Juno's unique moat is its native macOS + voice + multi-agent + MCP stack, which no competitor matches.

**Critical finding:** Juno is leaving model capability on the table by downscaling screenshots instead of supporting Opus 4.7's native 2,576px 1:1 coordinate mode.

---

## Competitors Analyzed

| Product | Company | Architecture | Launch |
|---------|---------|-------------|--------|
| Computer Use API | Anthropic | API-first, caller owns environment | Beta (2024+) |
| Codex Computer Use | OpenAI | Desktop-native macOS app | April 16, 2026 |
| Gemini Computer Use | Google | Browser-anchored, DOM-aware | Q2 2026 |
| **Juno** | Lacy Morrow | Native macOS Tauri app, Rust backend | — |

---

## 1. Action Parity Matrix

| Action | Anthropic API | Codex | Juno | Status |
|--------|:---:|:---:|:---:|---|
| `screenshot` | Yes | Yes | Yes | Parity |
| `left_click` | Yes | Yes | Yes | Parity |
| `right_click` | Yes | Yes | Yes | Parity |
| `middle_click` | Yes | ? | Yes | Parity |
| `double_click` | Yes | Yes | Yes | Parity |
| `triple_click` | Yes | ? | Yes | Parity |
| `left_click_drag` | Yes | ? | Yes | Parity |
| `mouse_move` | Yes | Yes | Yes | Parity |
| `left_mouse_down` | Yes | ? | Yes | Parity |
| `left_mouse_up` | Yes | ? | Yes | Parity |
| `key` | Yes | Yes | Yes | Parity |
| `hold_key` | Yes | ? | Yes | Parity |
| `type` | Yes | Yes | Yes | Parity |
| `scroll` (directional) | Yes | Yes | Yes | Parity |
| `wait` | Yes | ? | Yes | Parity |
| `cursor_position` | Yes | ? | Yes | Parity |
| `zoom` (region) | Yes | No | Yes | AHEAD |
| Modifier keys on click/scroll | Yes | ? | No | BEHIND |
| Clipboard manipulation | No (use bash) | Yes | Yes | AHEAD vs Anthropic |

**Verdict:** 17/17 Anthropic actions implemented. One gap: modifier key support on click/scroll.

---

## 2. Screenshot & Vision

| Capability | Anthropic | Codex | Juno | Status |
|------------|:---:|:---:|:---:|---|
| Max resolution (Opus 4.7) | 2,576px, 1:1 coords | Unspecified | Standard res scaling | **BEHIND** |
| Resolution scaling | Caller handles | Automatic | Lanczos3 | Parity |
| JPEG compression | Caller handles | Automatic | Quality 85 | Parity |
| PNG for detail regions | Caller handles | ? | PNG for zoom | Parity |
| Multi-monitor | Caller handles | Single? | Cursor-based detection | AHEAD |
| Cursor position metadata | Not built-in | Unspecified | Returns coords as text | AHEAD |
| Zoom (native-res region) | Yes (enable_zoom) | No | Yes (Retina res) | AHEAD |
| Screenshot history limiting | No (stateless) | ? | MAX_RECENT_SCREENSHOTS=3 | AHEAD |

### GAP: Opus 4.7 Resolution Support
Opus 4.7 supports up to 2,576px on the long edge with 1:1 pixel coordinates (no scale-factor conversion required). Juno currently downscales all screenshots to standard resolutions via `select_best_resolution()`. This reduces click accuracy unnecessarily.

**Fix:** Detect model version. When using Opus 4.7+, skip downscaling and pass `display_width_px`/`display_height_px` matching actual (or lightly scaled) dimensions.

---

## 3. Safety & Permissions

| Capability | Anthropic | Codex | Juno | Status |
|------------|:---:|:---:|:---:|---|
| Prompt injection classifier | Yes (auto) | ? | No | **BEHIND** |
| App-level allowlist | No (caller) | Yes | No | **BEHIND** |
| Sensitive action warnings | No (caller) | Yes | No | **BEHIND** |
| Task cancellation | No (caller) | Yes | Yes (Escape) | Parity w/ Codex |
| Sandboxed environment | Docker ref impl | macOS sandbox | No sandbox | **BEHIND** |
| Terminal automation block | No | Yes | No | Different philosophy |
| Self-automation prevention | No | Yes | No | **BEHIND** |
| macOS permission validation | No (not native) | Yes | Yes (5 types) | AHEAD vs Anthropic |
| Action cooldown | No | ? | 300ms | AHEAD |
| Human confirmation loop | API pattern | Session approval | No | **BEHIND** |

### Critical Safety Gaps

1. **No app allowlist** — Agent can interact with any app. Codex requires per-app permission.
2. **No sensitive action warnings** — No detection of destructive operations.
3. **No prompt injection defense** — Anthropic runs classifiers on screenshots automatically.
4. **No human confirmation loop** — No "approve before executing" UX for risky actions.
5. **No self-automation prevention** — Agent could interact with Juno's own UI windows.

---

## 4. Agent Architecture

| Capability | Anthropic | Codex | Juno | Status |
|------------|:---:|:---:|:---:|---|
| Multi-agent orchestration | No | Yes (parallel bg) | Yes (orchestrator) | AHEAD vs Anthropic |
| Background execution | No (sync API) | Yes (non-blocking) | Yes (Tokio async) | Parity |
| Parallel agents | Caller manages | Native sessions | Task queue (max 12) | Parity |
| Memory isolation | Stateless | Memory preview | SpecialistSummary | AHEAD vs Anthropic |
| Extended thinking | Yes (budget_tokens) | GPT-5.x reasoning | Not implemented | **BEHIND** |
| Iteration limits | Caller handles | Built-in | Not implemented | **BEHIND** |

---

## 5. Companion Tools & Integrations

| Tool | Anthropic | Codex | Juno | Status |
|------|:---:|:---:|:---:|---|
| Bash/shell | Yes | Yes | Yes | Parity |
| Text editor | Yes | Yes | Yes | Parity |
| Browser automation | No (use CU) | In-app browser | Playwright + Safari | AHEAD |
| MCP integration | No | 90+ plugins | Yes (STDIO + HTTP) | Different model |
| Voice input | No | No | Yes (Whisper) | AHEAD |
| TTS output | No | No | Yes (ElevenLabs) | AHEAD |
| Cloud remote control | No | ? | Yes (WebSocket) | AHEAD |

---

## 6. Developer / User Experience

| Capability | Anthropic | Codex | Juno | Status |
|------------|:---:|:---:|:---:|---|
| Zero setup | Docker pull | Install app | Install app | Parity w/ Codex |
| Plugin marketplace | No | 90+ plugins | Manual MCP | **BEHIND** |
| Native macOS | Docker/VNC | Native | Native (Tauri) | AHEAD vs Anthropic |
| Multi-window | No | Single? | Yes (6 windows) | AHEAD |
| Headless/CLI | API-only | ? | juno-cua CLI | AHEAD vs Codex |

---

## 7. Where Juno is AHEAD

| Advantage | Details |
|-----------|---------|
| Native macOS execution | Not Docker/VNC — direct AX, ScreenCaptureKit |
| Voice input/output | Whisper transcription + ElevenLabs TTS |
| Multi-monitor support | Cursor-based display detection |
| Zoom at Retina resolution | Native-res region capture, PNG |
| Browser automation | Playwright + Safari AppleScript (dedicated tools) |
| Multi-agent orchestration | Hierarchical orchestrator + specialist agents |
| MCP extensibility | Open standard, STDIO + HTTP transport |
| Cloud remote control | WebSocket connector for remote agent control |
| CLI mode | juno-cua for headless/external agent use |
| Action cooldown | 300ms between UI actions prevents racing |
| Memory isolation | SpecialistSummary for cross-agent context |
| Token optimization | JPEG quality 85 + screenshot history limiting |

---

## 8. Priority Gap Analysis

### P0 — Must fix (safety/trust, competitive table stakes)

| # | Gap | Impact | Effort | Status |
|---|-----|--------|--------|--------|
| 1 | **Opus 4.5+ high-res resolution (up to 2,576px)** | Click accuracy, model capability | Medium | **DONE** |
| 2 | **Human confirmation for risky actions** | User trust, safety | Medium | Deferred (needs frontend modal UX) |
| 3 | **App blocklist / scope boundaries** | Prevent unintended app control | Medium | **DONE** (blocked apps list) |
| 4 | **Self-automation prevention** | Security (agent can't manipulate Juno) | Small | **DONE** |

### P1 — Important for competitive positioning

| # | Gap | Impact | Effort | Status |
|---|-----|--------|--------|--------|
| 5 | **Sensitive action detection** | Warn before destructive ops | Medium | **DONE** |
| 6 | **Modifier keys on click/scroll** | Range select, multi-select support | Small | **DONE** (click only, scroll follow-up) |
| 7 | **Iteration / cost guardrails** | Prevent runaway agent loops | Small | **ALREADY EXISTS** (agent_runner.rs:670) |
| 8 | **Action audit log** | Reviewable history of agent actions | Medium | **DONE** (event-based) |

### P2 — Future differentiation

| # | Gap | Impact | Effort | Status |
|---|-----|--------|--------|--------|
| 9 | Plugin/integration catalog | One-click MCP server install | Large | Not started |
| 10 | Prompt injection defense | Screenshot text classifier | Large | Not started |
| 11 | Parallel background sessions | Codex's signature UX | Large | Not started |
| 12 | DOM-aware browser automation | Gemini's strength | Large | Not started |

---

## Implementation Details (2026-04-21)

### Files Modified

| File | Changes |
|------|---------|
| `src-tauri/src/constants/ui.rs` | Added HD_WXGA, HD_1080, ULTRA_HD resolutions + model-aware selection |
| `src-tauri/src/utils/coordinates.rs` | Added CURRENT_MODEL global, model-aware resolution in scaling pipeline |
| `src-tauri/src/agent/providers/factory.rs` | Publishes model name at brain creation |
| `src-tauri/src/agent/providers/types.rs` | Added Opus 4.7, Sonnet 4.6 model IDs to OPUS_4_5_PLUS list |
| `src-tauri/src/agent/tools/anthropic_computer_use.rs` | Self-automation prevention, blocked apps, modifier keys, sensitive action detection, audit log, UTF-8 fix |
| `src-tauri/src/constants/events.rs` | Added COMPUTER_USE_AUDIT event constant |

### Remaining Follow-up

- **Human confirmation modal**: Requires frontend React component + Tauri event round-trip. Pattern exists in `pending_tool_approvals`.
- **Modifier keys on scroll**: `scroll_window()` doesn't accept modifiers — needs lower-level CGEvent changes.
- **User-configurable app allowlist**: Currently hardcoded `BLOCKED_BUNDLE_IDS`. Should load from Tauri Store.
- **Prompt injection defense**: Would require screenshot OCR/classifier — significant effort.

---

## Sources

- [Anthropic Computer Use Docs](https://platform.claude.com/docs/en/docs/agents-and-tools/computer-use)
- [Codex Computer Use](https://developers.openai.com/codex/app/computer-use)
- [Computer Use Agents 2026 Comparison](https://www.digitalapplied.com/blog/computer-use-agents-2026-claude-openai-gemini-matrix)
- [Codex Desktop April 2026 Update](https://smartscope.blog/en/generative-ai/chatgpt/codex-desktop-major-update-april-2026/)

# Security Audit - Juno Rust Backend

**Date:** 2026-02-08
**Scope:** All Rust source files in `src-tauri/src/` and plugins

> These are security vulnerabilities identified during a comprehensive code audit.
> They are documented here for tracking and should be addressed with dedicated security review.

> **Status pass (2026-09-11):** each item below now carries a **Status (2026-09-11)** line
> recording what the RC verification found and which fix PR closed it (PRs #534, #535, #537).
> The original 2026-02-08 audit text is unchanged; only status annotations were appended.
> Legend: **Fixed** (verified closed), **Mitigated** (risk reduced, root cause remains),
> **Partial** (one vector closed, others remain), **Residual** (unchanged, impact noted),
> **Not re-verified** (outside the scope of this pass).

---

## CRITICAL

### 1. Command Injection in `open_file_and_type`
- **File:** `src-tauri/src/agent/tools/desktop_tools.rs:587`
- **Issue:** `format!("open '{}'", args.file_path)` passes user-supplied file_path into a shell command via single-quote interpolation. A path containing `'` breaks out of the quotes and allows arbitrary shell command execution.
- **Fix:** Use `std::process::Command::new("open").arg(&args.file_path)` instead of shell string interpolation.
- **Status (2026-09-11):** **Fixed in #535.** `open` is spawned directly via `std::process::Command` with the path as an argument; no shell is involved.

### 2. Trivially Bypassable Bash Command Validation
- **File:** `src-tauri/src/commands/shell.rs:222-276`
- **Issue:** Blocklist-based validation using substring matching. `rm -rf /` is blocked but `rm -r -f /` is not. `sudo` blocked but `su`, `pkexec`, `bash -c "sudo ..."` are not. In debug mode, all validation is skipped.
- **Fix:** Switch to allowlist approach or use sandboxed execution.
- **Status (2026-09-11):** **Fixed in #535.** The debug-build bypass is removed (debug and release validate identically); the blocklist normalizes case and whitespace and catches `rm` flag permutations. Still a blocklist rather than the suggested allowlist; #3's approval gate now sits in front of it.

### 3. Unrestricted Bash Command Execution from AI Agent
- **File:** `src-tauri/src/agent/tools/anthropic_computer_use.rs:787-837`
- **Issue:** `execute_bash_tool` accepts any shell command from the AI agent with only the weak validation from #2. Prompt injection via webpage content could manipulate the agent into executing harmful commands.
- **Fix:** Allowlist, sandbox, or require user confirmation for commands.
- **Status (2026-09-11):** **Fixed in #535.** Default shell risk tier raised to High in `risk_classifier.rs`, so every agent bash command routes through the existing user-approval prompt (60s timeout, deny supported).

### 4. JavaScript Injection via Browser CSS Selectors
- **File:** `src-tauri/src/agent/tools/browser_controller.rs:1196-1401`
- **Issue:** CSS selectors embedded in JS strings with only double-quote escaping. Backslashes, newlines, and other characters can break out of the string context.
- **Fix:** Implement proper JS string escaping or pass values as function arguments.
- **Status (2026-09-11):** **Fixed in #537.** Untrusted selectors, values, and attribute/property names are JSON-encoded via `BrowserController::js_string_literal()` at every interpolation site; unit tests cover the breakout payloads.

### 5. Cloud Signature Verification Bypassed
- **File:** `src-tauri/src/cloud/security.rs:80-83`
- **Issue:** Invalid HMAC signatures log a warning but execution continues. Remote attackers can send commands without valid signatures.
- **Fix:** Return `Err(CloudError::SecurityError(...))` on signature failure.
- **Status (2026-09-11):** **Fixed in #534.** An invalid signature now returns `Err` and rejects the command instead of warning and continuing.

### 6. All Cloud Security Levels Collapsed to Low
- **File:** `src-tauri/src/cloud/config.rs:115-118, 163-165`
- **Issue:** Medium and High security levels behave identically to Low. `migrate_to_permissive_defaults()` forces `SecurityLevel::Low` on every config load, overriding user settings.
- **Fix:** Implement differentiated behavior. Don't override user-configured security levels.
- **Status (2026-09-11):** **Fixed in #534.** `migrate_to_permissive_defaults()` removed; stored levels are preserved, new configs default to High, unknown stored values fail closed to High.

### 7. API Key Sent in Plaintext in WebSocket Auth
- **File:** `src-tauri/src/cloud/auth.rs:98-105`
- **Issue:** Raw `api_key` included in JSON payload. Exposed in logging, serialization, server-side storage.
- **Fix:** Remove api_key from payload. Use token-only auth or HMAC signing.
- **Status (2026-09-11):** **Mitigated in #534.** The cloud settings UI (set-API-key / start-connector / enable flows) was removed and cloud stays disabled by default (LAC-3729), so the channel is unreachable in this release. The auth payload itself still carries the raw `api_key` (`cloud/auth.rs`); token-only auth remains open.

### 8. Timing-Vulnerable Signature Comparison
- **File:** `src-tauri/src/cloud/auth.rs:216`
- **Issue:** `==` string comparison for HMAC signatures enables timing attacks.
- **Fix:** Use constant-time comparison (`subtle::ConstantTimeEq` or `mac.verify_slice()`).
- **Status (2026-09-11):** **Fixed in #534.** HMAC verification uses constant-time `verify_slice` on the base64-decoded signature; invalid base64 is treated as an invalid signature.

### 9. No Rate Limiting on Cloud Commands
- **File:** `src-tauri/src/cloud/security.rs:282-286`
- **Issue:** `check_rate_limit()` always returns `Ok(())`. Combined with permissive security, allows unlimited remote command execution.
- **Fix:** Implement rate limiting with configurable thresholds.
- **Status (2026-09-11):** **Fixed in #534.** `check_rate_limit` is a real token bucket (30/min per command type, `utils/rate_limiter.rs`) and fails closed.

### 10. No Confirmation Required for Cloud Commands
- **File:** `src-tauri/src/cloud/security.rs:240-243`
- **Issue:** `requires_confirmation()` always returns `false`. Destructive commands execute without user consent.
- **Fix:** Require confirmation for `SystemCommand` and `ConfigUpdate` types.
- **Status (2026-09-11):** **Fixed in #534.** `requires_confirmation` returns `true` for `SystemCommand` and `ConfigUpdate`.

### 11. Cloud Enabled by Default
- **File:** `src-tauri/src/cloud/config.rs:122-123`
- **Issue:** Cloud connectivity is `enabled: true` by default with auto-connect to production server. Combined with permissive security, opens remote command channel immediately.
- **Fix:** Default to `enabled: false`, require explicit opt-in.
- **Status (2026-09-11):** **Previously fixed (LAC-3729).** `enabled: false` is the default in `cloud/config.rs`; reaffirmed during #534.

---

## HIGH

### 12. Path Traversal Bypass in Debug Mode
- **File:** `src-tauri/src/agent/tools/basic_tools.rs:232-234`
- **Issue:** Only blocks `../../../..` (4+ levels). Three levels of traversal enough to reach filesystem root.
- **Fix:** Canonicalize paths and enforce workspace boundaries in all modes.
- **Status (2026-09-11):** **Residual.** Debug builds still use the relaxed substring check (`basic_tools.rs` `debug_mode`); the production path now canonicalizes through the shared `path_security` helper introduced in #535.

### 13. Path Traversal in str_replace_tool
- **File:** `src-tauri/src/agent/tools/anthropic_computer_use.rs:66-88`
- **Issue:** Checks for `../` but not absolute paths. `/etc/shadow` passes validation when `allow_absolute_paths: false`.
- **Fix:** Check `Path::is_absolute()` and canonicalize against workspace boundary.
- **Status (2026-09-11):** **Fixed in #535.** Paths are canonicalized (resolving `..` and symlinks) and must resolve inside an allowed workspace root.

### 14. File Write Without Workspace Boundary Check
- **File:** `src-tauri/src/agent/tools/anthropic_computer_use.rs:897-975`
- **Issue:** str_replace and create commands use weak path validation. No workspace boundary enforcement.
- **Fix:** Canonicalize and validate against workspace directory.
- **Status (2026-09-11):** **Fixed in #535.** Same canonicalize-plus-boundary enforcement as #13, via the shared `path_security.rs` helper.

### 15. Bypassable JavaScript Safety Validation
- **File:** `src-tauri/src/agent/tools/safari_tools.rs:86-129`
- **Issue:** Simple substring matching for blocked patterns. Bypassed via template literals, indirect access, concatenation.
- **Fix:** Require user confirmation or remove the tool.
- **Status (2026-09-12):** **Gated.** `safari_execute_javascript` is now classified High in `risk_classifier.rs`, so `AgentRunner::check_batch_approval` requires user approval before any arbitrary JS reaches Safari — the same pattern that gates agent bash (#3, PR #535). `validate_javascript_safety` stays as advisory defense-in-depth with its cheap bypasses closed (matching now runs on lowercased, whitespace-stripped text; the previously dead `Function(` / `innerHTML =` patterns match again; constructor chains and `document.cookie` added). Identifier concatenation (`window["ev"+"al"]`) still passes the filter by design — catching it needs a JS parser, and the approval gate, not the filter, is the control. Removing the tool outright remains an open product decision.

### 16. No URL Protocol Validation for Safari Navigation
- **File:** `src-tauri/src/agent/tools/safari_tools.rs:544-571`
- **Issue:** Accepts `javascript:`, `file:///`, `data:` URLs.
- **Fix:** Validate URL starts with `http://` or `https://`.
- **Status (2026-09-11):** **Fixed in #537.** `validate_navigation_url()` parses with the `url` crate and allows only exact `http`/`https`, applied at the `SafariTools::navigate_to_url` choke point and again at the command layer.

### 17. Browser Launched with Security Disabled
- **File:** `src-tauri/src/agent/tools/browser_controller.rs:583-587`
- **Issue:** `--no-sandbox` and `--disable-web-security` flags disable Chrome security.
- **Fix:** Remove these flags.
- **Status (2026-09-11):** **Fixed in #537.** `--no-sandbox` and `--disable-web-security` removed from every launch path; the unused `NO_SANDBOX_FLAG` constant was deleted so the flag cannot quietly return.

### 18. No Path Validation in Enhanced Coding Tools
- **File:** `src-tauri/src/agent/tools/enhanced_coding_tools.rs:292-663`
- **Issue:** `smart_create_file` writes to arbitrary paths without validation.
- **Fix:** Add path validation consistent with basic_tools.rs security model.
- **Status (2026-09-11):** **Fixed in #535.** All `smart_create_file` writes funnel through `create_file_with_content`, which enforces the same canonicalize-plus-boundary validation.

### 19. .env Files Readable by Agent
- **File:** `src-tauri/src/agent/tools/basic_tools.rs:73`
- **Issue:** Allowed extensions include "env", exposing credentials to AI agent.
- **Fix:** Remove "env" from allowed extensions.
- **Status (2026-09-11):** **Fixed in #535.** `env` removed from allowed read extensions; `env`, `pem`, and `key` extensions plus `.env` / `.env.*` filenames are blocked in production.

### 20. Arbitrary JS Execution via Safari Command
- **File:** `src-tauri/src/commands/safari_tools.rs:94`
- **Issue:** `safari_execute_javascript` Tauri command has no validation.
- **Fix:** Add validation, rate limiting, restrict to debug mode.
- **Status (2026-09-12):** **Gated.** The webview-invokable surface is closed: the `safari_execute_javascript` command and the arbitrary-JS branch of `execute_safari_tool` now refuse with an explanatory error (no frontend caller existed; the command layer cannot prompt, so refusal is the honest gate). The agent pipeline reaches JS execution only through `execute_safari_tool_for_agent` — a plain function, not a Tauri command — which runs only after the High-risk approval gate from #15. Removing the capability entirely remains an open product decision.

### 21. Gemini API Key Exposed in URL Query Parameter
- **File:** `src-tauri/src/agent/providers/gemini.rs:318-321`
- **Issue:** API key in URL query parameter, logged by HTTP clients/proxies.
- **Fix:** Use `x-goog-api-key` HTTP header instead.
- **Status (2026-09-11):** **Fixed in #537.** Key moved to the `x-goog-api-key` header; a sweep found no other key-in-URL pattern in the tree.

### 22. `unsafe impl Send + Sync` for Voice Controllers
- **File:** `tauri-plugin-voice-transcription/src/controller.rs:782-783`
- **File:** `tauri-plugin-voice-transcription/src/always_listening.rs:1292-1293`
- **Issue:** Overrides compiler safety guarantees. Controllers contain non-Send types (`cpal::Stream`).
- **Fix:** Restructure to avoid non-Send types or add SAFETY documentation.
- **Status (2026-09-11):** **Previously addressed.** The `Sync` impls are gone; the remaining `unsafe impl Send`s carry SAFETY documentation and the controllers are wrapped in `Arc<Mutex<...>>` at call sites (the audit's alternative fix).

### 23. Cloud Timestamp Validation Accepts Any Skew
- **File:** `src-tauri/src/cloud/security.rs:109-113`
- **Issue:** Commands with >1 hour timestamp skew are warned but accepted. Enables replay attacks.
- **Fix:** Reject commands beyond 5-minute window.
- **Status (2026-09-11):** **Fixed in #534.** Skew beyond the window now rejects the command instead of warning.

### 24. Cloud Auth Assumes Success Without Validation
- **File:** `src-tauri/src/cloud/client.rs:285-308`
- **Issue:** Sets state to Authenticated without validating server response.
- **Fix:** Wait for and validate auth response.
- **Status (2026-09-11):** **Residual.** `cloud/client.rs` still transitions to `Authenticated` without validating a server response. Impact is limited by cloud being disabled by default and by the fail-closed signature and timestamp checks (#534) on the command path.

### 25. Cloud Denied Commands Blacklist Easily Bypassed
- **File:** `src-tauri/src/cloud/config.rs:38-59`
- **Issue:** Exact substring matching. `rm -r -f /` bypasses `rm -rf` block.
- **Fix:** Robust command parsing or whitelist model.
- **Status (2026-09-11):** **Residual.** The substring blacklist is unchanged. Impact reduced by #534: mandatory confirmation for `SystemCommand`/`ConfigUpdate`, real rate limiting, and the disabled-by-default cloud channel.

### 26. MCP Server Spawns Arbitrary Processes
- **File:** `src-tauri/src/agent/tools/mcp_integration.rs:248-272`
- **Issue:** No validation of MCP server command/args. Allows arbitrary process execution.
- **Fix:** Allowlist known MCP executables or require user approval.
- **Status (2026-09-11):** **Fixed in #534.** MCP servers must be explicitly approved before spawn (`approved: bool`, serde default `false`, so pre-existing configs are unapproved); approval happens only through the `approve_mcp_server` command and persists via `ToolConfigManager`.

---

## MEDIUM

### 27. Workspace Root Defaults to Unreliable `current_dir()`
- **File:** `src-tauri/src/agent/tools/basic_tools.rs:149-150`
- **Issue:** In Tauri, cwd may be `/` or home, making boundary checks ineffective.
- **Fix:** Use well-known app-specific directory.
- **Status (2026-09-11):** **Residual.** Workspace root still defaults to `current_dir()`. Both agent file surfaces now share `path_security.rs` (#535), so this is a one-place fix when picked up.

### 28. TOCTOU Race in File Operations
- **File:** `src-tauri/src/agent/tools/anthropic_computer_use.rs:908-966`
- **Issue:** Read-modify-write without atomic operations.
- **Fix:** Use atomic file operations (write-to-temp-then-rename).
- **Status (2026-09-11):** **Not re-verified** in the 2026-09-11 pass.

### 29. No URL Validation on `safari_navigate` Command
- **File:** `src-tauri/src/commands/safari_tools.rs:70`
- **Issue:** URL passed directly to Safari without protocol validation.
- **Fix:** Validate URL starts with http:// or https://.
- **Status (2026-09-11):** **Fixed in #537.** `safari_navigate` and `execute_safari_tool`'s navigate branch both validate through `validate_navigation_url()` (defense in depth on top of the #16 choke point).

### 30. `update_cloud_config` Does Not Validate server_url
- **File:** `src-tauri/src/commands/cloud.rs:52-98`
- **Issue:** Accepts empty strings, invalid URLs, non-WebSocket URLs.
- **Fix:** Call `config.validate()` before applying.
- **Status (2026-09-11):** **Not re-verified** in the 2026-09-11 pass.

### 31. `import_settings` No Semantic Validation
- **File:** `src-tauri/src/commands/settings.rs:277-291`
- **Issue:** No validation of imported settings values, no backup before overwrite.
- **Fix:** Add semantic validation and backup.
- **Status (2026-09-11):** **Not re-verified** in the 2026-09-11 pass.

### 32. Incomplete Sensitive File Blocklist
- **File:** `src-tauri/src/agent/tools/basic_tools.rs:244-246`
- **Issue:** Only blocks `/etc/passwd` and `/etc/shadow`. Many other sensitive files accessible.
- **Fix:** Rely on workspace boundary check instead.

- **Status (2026-09-11):** **Improved by #535** (blocked extensions and dotfile names expanded, canonicalize-plus-boundary enforcement added, which is the fix this item recommends); not exhaustively re-verified.
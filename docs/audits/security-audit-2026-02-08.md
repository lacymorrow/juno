# Security Audit - Juno Rust Backend

**Date:** 2026-02-08
**Scope:** All Rust source files in `src-tauri/src/` and plugins

> These are security vulnerabilities identified during a comprehensive code audit.
> They are documented here for tracking and should be addressed with dedicated security review.

---

## CRITICAL

### 1. Command Injection in `open_file_and_type`
- **File:** `src-tauri/src/agent/tools/desktop_tools.rs:587`
- **Issue:** `format!("open '{}'", args.file_path)` passes user-supplied file_path into a shell command via single-quote interpolation. A path containing `'` breaks out of the quotes and allows arbitrary shell command execution.
- **Fix:** Use `std::process::Command::new("open").arg(&args.file_path)` instead of shell string interpolation.

### 2. Trivially Bypassable Bash Command Validation
- **File:** `src-tauri/src/commands/shell.rs:222-276`
- **Issue:** Blocklist-based validation using substring matching. `rm -rf /` is blocked but `rm -r -f /` is not. `sudo` blocked but `su`, `pkexec`, `bash -c "sudo ..."` are not. In debug mode, all validation is skipped.
- **Fix:** Switch to allowlist approach or use sandboxed execution.

### 3. Unrestricted Bash Command Execution from AI Agent
- **File:** `src-tauri/src/agent/tools/anthropic_computer_use.rs:787-837`
- **Issue:** `execute_bash_tool` accepts any shell command from the AI agent with only the weak validation from #2. Prompt injection via webpage content could manipulate the agent into executing harmful commands.
- **Fix:** Allowlist, sandbox, or require user confirmation for commands.

### 4. JavaScript Injection via Browser CSS Selectors
- **File:** `src-tauri/src/agent/tools/browser_controller.rs:1196-1401`
- **Issue:** CSS selectors embedded in JS strings with only double-quote escaping. Backslashes, newlines, and other characters can break out of the string context.
- **Fix:** Implement proper JS string escaping or pass values as function arguments.

### 5. Cloud Signature Verification Bypassed
- **File:** `src-tauri/src/cloud/security.rs:80-83`
- **Issue:** Invalid HMAC signatures log a warning but execution continues. Remote attackers can send commands without valid signatures.
- **Fix:** Return `Err(CloudError::SecurityError(...))` on signature failure.

### 6. All Cloud Security Levels Collapsed to Low
- **File:** `src-tauri/src/cloud/config.rs:115-118, 163-165`
- **Issue:** Medium and High security levels behave identically to Low. `migrate_to_permissive_defaults()` forces `SecurityLevel::Low` on every config load, overriding user settings.
- **Fix:** Implement differentiated behavior. Don't override user-configured security levels.

### 7. API Key Sent in Plaintext in WebSocket Auth
- **File:** `src-tauri/src/cloud/auth.rs:98-105`
- **Issue:** Raw `api_key` included in JSON payload. Exposed in logging, serialization, server-side storage.
- **Fix:** Remove api_key from payload. Use token-only auth or HMAC signing.

### 8. Timing-Vulnerable Signature Comparison
- **File:** `src-tauri/src/cloud/auth.rs:216`
- **Issue:** `==` string comparison for HMAC signatures enables timing attacks.
- **Fix:** Use constant-time comparison (`subtle::ConstantTimeEq` or `mac.verify_slice()`).

### 9. No Rate Limiting on Cloud Commands
- **File:** `src-tauri/src/cloud/security.rs:282-286`
- **Issue:** `check_rate_limit()` always returns `Ok(())`. Combined with permissive security, allows unlimited remote command execution.
- **Fix:** Implement rate limiting with configurable thresholds.

### 10. No Confirmation Required for Cloud Commands
- **File:** `src-tauri/src/cloud/security.rs:240-243`
- **Issue:** `requires_confirmation()` always returns `false`. Destructive commands execute without user consent.
- **Fix:** Require confirmation for `SystemCommand` and `ConfigUpdate` types.

### 11. Cloud Enabled by Default
- **File:** `src-tauri/src/cloud/config.rs:122-123`
- **Issue:** Cloud connectivity is `enabled: true` by default with auto-connect to production server. Combined with permissive security, opens remote command channel immediately.
- **Fix:** Default to `enabled: false`, require explicit opt-in.

---

## HIGH

### 12. Path Traversal Bypass in Debug Mode
- **File:** `src-tauri/src/agent/tools/basic_tools.rs:232-234`
- **Issue:** Only blocks `../../../..` (4+ levels). Three levels of traversal enough to reach filesystem root.
- **Fix:** Canonicalize paths and enforce workspace boundaries in all modes.

### 13. Path Traversal in str_replace_tool
- **File:** `src-tauri/src/agent/tools/anthropic_computer_use.rs:66-88`
- **Issue:** Checks for `../` but not absolute paths. `/etc/shadow` passes validation when `allow_absolute_paths: false`.
- **Fix:** Check `Path::is_absolute()` and canonicalize against workspace boundary.

### 14. File Write Without Workspace Boundary Check
- **File:** `src-tauri/src/agent/tools/anthropic_computer_use.rs:897-975`
- **Issue:** str_replace and create commands use weak path validation. No workspace boundary enforcement.
- **Fix:** Canonicalize and validate against workspace directory.

### 15. Bypassable JavaScript Safety Validation
- **File:** `src-tauri/src/agent/tools/safari_tools.rs:86-129`
- **Issue:** Simple substring matching for blocked patterns. Bypassed via template literals, indirect access, concatenation.
- **Fix:** Require user confirmation or remove the tool.

### 16. No URL Protocol Validation for Safari Navigation
- **File:** `src-tauri/src/agent/tools/safari_tools.rs:544-571`
- **Issue:** Accepts `javascript:`, `file:///`, `data:` URLs.
- **Fix:** Validate URL starts with `http://` or `https://`.

### 17. Browser Launched with Security Disabled
- **File:** `src-tauri/src/agent/tools/browser_controller.rs:583-587`
- **Issue:** `--no-sandbox` and `--disable-web-security` flags disable Chrome security.
- **Fix:** Remove these flags.

### 18. No Path Validation in Enhanced Coding Tools
- **File:** `src-tauri/src/agent/tools/enhanced_coding_tools.rs:292-663`
- **Issue:** `smart_create_file` writes to arbitrary paths without validation.
- **Fix:** Add path validation consistent with basic_tools.rs security model.

### 19. .env Files Readable by Agent
- **File:** `src-tauri/src/agent/tools/basic_tools.rs:73`
- **Issue:** Allowed extensions include "env", exposing credentials to AI agent.
- **Fix:** Remove "env" from allowed extensions.

### 20. Arbitrary JS Execution via Safari Command
- **File:** `src-tauri/src/commands/safari_tools.rs:94`
- **Issue:** `safari_execute_javascript` Tauri command has no validation.
- **Fix:** Add validation, rate limiting, restrict to debug mode.

### 21. Gemini API Key Exposed in URL Query Parameter
- **File:** `src-tauri/src/agent/providers/gemini.rs:318-321`
- **Issue:** API key in URL query parameter, logged by HTTP clients/proxies.
- **Fix:** Use `x-goog-api-key` HTTP header instead.

### 22. `unsafe impl Send + Sync` for Voice Controllers
- **File:** `tauri-plugin-voice-transcription/src/controller.rs:782-783`
- **File:** `tauri-plugin-voice-transcription/src/always_listening.rs:1292-1293`
- **Issue:** Overrides compiler safety guarantees. Controllers contain non-Send types (`cpal::Stream`).
- **Fix:** Restructure to avoid non-Send types or add SAFETY documentation.

### 23. Cloud Timestamp Validation Accepts Any Skew
- **File:** `src-tauri/src/cloud/security.rs:109-113`
- **Issue:** Commands with >1 hour timestamp skew are warned but accepted. Enables replay attacks.
- **Fix:** Reject commands beyond 5-minute window.

### 24. Cloud Auth Assumes Success Without Validation
- **File:** `src-tauri/src/cloud/client.rs:285-308`
- **Issue:** Sets state to Authenticated without validating server response.
- **Fix:** Wait for and validate auth response.

### 25. Cloud Denied Commands Blacklist Easily Bypassed
- **File:** `src-tauri/src/cloud/config.rs:38-59`
- **Issue:** Exact substring matching. `rm -r -f /` bypasses `rm -rf` block.
- **Fix:** Robust command parsing or whitelist model.

### 26. MCP Server Spawns Arbitrary Processes
- **File:** `src-tauri/src/agent/tools/mcp_integration.rs:248-272`
- **Issue:** No validation of MCP server command/args. Allows arbitrary process execution.
- **Fix:** Allowlist known MCP executables or require user approval.

---

## MEDIUM

### 27. Workspace Root Defaults to Unreliable `current_dir()`
- **File:** `src-tauri/src/agent/tools/basic_tools.rs:149-150`
- **Issue:** In Tauri, cwd may be `/` or home, making boundary checks ineffective.
- **Fix:** Use well-known app-specific directory.

### 28. TOCTOU Race in File Operations
- **File:** `src-tauri/src/agent/tools/anthropic_computer_use.rs:908-966`
- **Issue:** Read-modify-write without atomic operations.
- **Fix:** Use atomic file operations (write-to-temp-then-rename).

### 29. No URL Validation on `safari_navigate` Command
- **File:** `src-tauri/src/commands/safari_tools.rs:70`
- **Issue:** URL passed directly to Safari without protocol validation.
- **Fix:** Validate URL starts with http:// or https://.

### 30. `update_cloud_config` Does Not Validate server_url
- **File:** `src-tauri/src/commands/cloud.rs:52-98`
- **Issue:** Accepts empty strings, invalid URLs, non-WebSocket URLs.
- **Fix:** Call `config.validate()` before applying.

### 31. `import_settings` No Semantic Validation
- **File:** `src-tauri/src/commands/settings.rs:277-291`
- **Issue:** No validation of imported settings values, no backup before overwrite.
- **Fix:** Add semantic validation and backup.

### 32. Incomplete Sensitive File Blocklist
- **File:** `src-tauri/src/agent/tools/basic_tools.rs:244-246`
- **Issue:** Only blocks `/etc/passwd` and `/etc/shadow`. Many other sensitive files accessible.
- **Fix:** Rely on workspace boundary check instead.

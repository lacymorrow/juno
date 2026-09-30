# Why are `commands/` input validators disabled in production builds?

Research for LAC-4004. Not a fix — the fix belongs in a follow-up. This
document names the cause, sorts the validators into "still doing something
useful", "duplicated elsewhere", and "dead weight", and proposes a phased
change with the blast radius spelled out for each phase.

Audit date: 2026-09-22. Repo state: `main` at `a6792234`.

## TL;DR

- **Cause is accidental, not deliberate.** `DebugConfig` (introduced 2025-06-23
  in `debug_utils.rs`) bundled six independent toggles behind one factory that
  is off in release. Validation got swept into the same off-in-release group as
  logging, notifications and timing — because the module was framed as "dev
  capabilities enabled in production functions", not as security enforcement.
- **The precedent for the fix already exists.** PR #535 (874fb610,
  2026-09-11) removed the exact same debug-mode bypass from
  `validate_command_security` in the same file (`shell.rs`) and added the
  `path_security` module. That PR fixed the command-content check but did not
  touch the `DebugConfig::validate_inputs` gate that still hides the timeout
  check two functions away.
- **Two validators are load-bearing** (no other layer enforces them):
  `bash_command`'s `timeout > 3600` upper bound and `core::wait`'s
  `valid_duration_seconds` cap. The wait cap is the most acute — combined with
  the `std::thread::sleep` in the underlying macOS interaction module, a model
  can starve a Tokio worker for years in one call.
- **Two validators are strictly weaker duplicates** of a real enforcement
  layer that already exists but is not wired to these commands:
  `filesystem.rs` and `text_editor.rs` call `valid_file_path` (which only
  greps for `..`) while `agent/tools/path_security.rs` has the full
  canonicalize + workspace-boundary + sensitive-file blocklist and TOCTOU-hardened
  I/O — used by the agent's `basic_tools` and Anthropic `str_replace_editor`
  path, but not by these Tauri commands.
- **The rest are sanity assertions** dressed up as validation. Empty selector
  strings and out-of-range mouse coordinates change error messages, not
  outcomes.

## What is true today

The scope in the LAC-4004 description is understated. `validate_inputs` gates
the following call sites (grep is authoritative):

| File | Sites |
|---|---|
| `commands/mouse.rs` | 18 |
| `commands/window.rs` | 6 |
| `commands/keyboard.rs` | 5 |
| `commands/text_editor.rs` | 4 |
| `commands/filesystem.rs` | 3 |
| `commands/element.rs` | 2 |
| `commands/core.rs` | 2 |
| `commands/app_url.rs` | 2 |
| `commands/shell.rs` | 1 |
| **Total** | **43** |

`DebugConfig::production_mode()` at `debug_utils.rs:37-46` sets every flag
false. `DebugConfig::from_build_mode()` at `:25-34` sets every flag to
`cfg!(debug_assertions)`, which is `false` in release. Both factories agree
in a release build: no validation.

`should_enable_debug()` at `debug_utils.rs:310-312` — used by most command
handlers to build a `DebugConfig` — flips to development mode when *any* of
these is true: the per-call `debug_mode: bool` argument, `state.is_debug_mode()`
(a runtime toggle stored in `ui_settings.debug_mode`), or
`cfg!(debug_assertions)`. So a release-build user who turns on debug in the UI
does turn these validators back on. That is not a security control — it is a
diagnostic switch.

### The debug_mode argument at agent call sites

`commands/shell.rs::bash_command` and `commands/filesystem.rs::*` take an
`Option<bool> debug_mode` argument. Different agent-side callers pass
different things:

| Caller | debug_mode passed | Effect in release |
|---|---|---|
| `agents/system_agent.rs` (bash, filesystem, text_editor) | `Some(true)` | validators run |
| `cloud/commands.rs::execute_shell_command` (bash) | `Some(true)` | validators run |
| `agent/tools/anthropic_computer_use.rs::bash` (line 2287) | `None` | validators skipped |
| `cli/headless.rs::wait` | (arg not on `wait`; validation still runs iff `state.is_debug_mode()` or debug build) | skipped in release |
| Frontend `invoke("bash_command", …)` etc. | `None` (usually) | skipped in release |

**The primary agent path** — the Anthropic Computer Use `bash` tool that most
production model turns route through — explicitly passes `None` and therefore
skips validation. `system_agent.rs` is a legacy/parallel routing path that
happens to pass `Some(true)`, but relying on that is a coincidence, not a
design.

## Why is it this way?

Three theories were floated in the issue. The evidence supports #3 (accidental
sweep) with a shading of #1 (viewed as diagnostics).

- **Theory 1 (deliberate — validators are dev-only assertions, real
  enforcement lives elsewhere).** The `DebugConfig` docstring says the module
  "provides debug capabilities that can be conditionally enabled in production
  functions, eliminating the need for separate `dev_` wrapper functions"
  (`debug_utils.rs:1-4`). This is the theory-1 mindset: the validators were
  conceived as diagnostic assertions promoted from `dev_` wrappers, not as
  security controls. Consistent with cause but does not survive contact with
  the actual check contents — `timeout > 3600` and `duration_seconds ≤ 60`
  are not diagnostics, they are the only bounds.
- **Theory 2 (performance).** No evidence. The validators are trivial: a
  string trim, an `f64` comparison, a `contains("..")` scan. Nothing in the
  git history mentions a hot-path measurement, and none of these functions
  are in a hot loop (they run once per agent tool call).
- **Theory 3 (accidental sweep).** Best fit. `DebugConfig` bundles
  `enabled`, `log_operations`, `send_notifications`, `validate_inputs`,
  `time_operations`, `emit_visualizations` into one struct. The factories set
  them as a group. When someone wrote the first `production_mode()` they
  disabled every flag together — reasonable for the other five, wrong for
  `validate_inputs` given what the validators actually check. The scope
  (43 call sites across 9 files) makes this a systemic collapse, not a
  per-site judgment.

The precedent from PR #535 (`fix(security): close agent shell injection and
file-write boundary criticals`, 2026-09-11, 874fb610) is the strongest
evidence. The commit message says "the debug-mode validation bypass is
removed" — but only for `validate_command_security`, a separate function.
The same file (`shell.rs`) has `bash_command`'s `timeout` and `non_empty`
checks still behind `debug_config.validate_inputs`, five lines away from the
now-unconditional command-content check. If theory 1 were right and the
project had made a deliberate decision to keep validators as dev-only
assertions, that commit would not have made a partial exception. The
exception was made because the author saw *that specific* validator as
security-critical; they did not zoom out and audit the other 42 sites.

## Load-bearing vs duplicated vs dead-weight

Verified per site by reading each command handler and mapping it to
`path_security.rs`, `risk_classifier.rs`, and `permission_gate.rs`.

### Genuinely load-bearing (no alternative enforcement)

1. **`commands/shell.rs::bash_command` — `timeout == 0` and `timeout > 3600`**
   (`shell.rs:730-760`).
   - `validate_command_security` runs unconditionally but only inspects the
     command *content*, not the timeout arg.
   - `risk_classifier::classify_shell_risk` classifies patterns like `sudo`,
     `rm -rf`, `mkfs` and gates them behind human approval — again, content-only.
   - `DEFAULT_TIMEOUT = 120s` (`shell.rs:55`) covers the `None` case only.
     A model that passes `timeout_seconds = Some(0)` gets a session that
     sits blocking with timeout 0. A model that passes `Some(999_999_999)`
     gets a session that can block until the OS reclaims it.
   - **Consequence**: bash sessions can be pinned indefinitely by a model
     over the Anthropic tool path.

2. **`commands/core.rs::wait` — `valid_duration_seconds` (≤ 60s cap)**
   (`core.rs:912-914`).
   - `wait` is exposed to the model as the Anthropic `computer` action
     `wait`, routed at `anthropic_computer_use.rs:2190`.
   - `risk_classifier::classify_computer_use_risk` returns Low for anything
     other than `screenshot`, `cursor_position`, or destructive key combos
     (`risk_classifier.rs:143-165`) — so `wait` bypasses approval.
   - The underlying implementation is
     `mcp-server-os-level/src/platforms/macos/interaction.rs:2001-2008`:
     `std::thread::sleep(Duration::from_millis(duration_ms))`. This is
     called from a `#[tauri::command] async fn` and pins a Tokio worker
     for the sleep duration.
   - `duration_ms = (duration_sec * 1000.0).max(0.0) as u64` — no upper
     bound. `duration_sec = 1e12` casts to a `u64` that overflows the
     runtime worker for years.
   - **Consequence**: worker-starvation vector by a single model tool
     call. This is the most acute of the two — it does not even need a
     runaway shell.

### Strictly weaker duplicates (a stronger check exists but is not wired to these commands)

3. **`commands/filesystem.rs::list_files / get_file_content / set_file_content`
   — `valid_file_path`** (`filesystem.rs:76, 214, 317`).
   - The check: reject empty path or path containing `..`
     (`debug_utils.rs:287-306`).
   - The proper check: `agent/tools/path_security.rs::resolve_within_default_roots`
     canonicalizes the path, walks intermediate symlinks, requires the result
     to be inside `cwd` (only if writable, not `/`) or `~/Juno`, and blocks
     `.env`, `id_rsa`, `.aws/credentials`, `.pem`, `.p12`, keychains, etc.
     Plus `read_to_string_checked` / `write_checked` re-verify the open handle's
     path with `F_GETPATH` to close the TOCTOU window on symlink swap
     (audit item #28).
   - Path_security is used by `basic_tools::read_file`,
     `anthropic_computer_use::validate_file_path` (str_replace editor tool),
     and `enhanced_coding_tools::create_file_with_content`. It is NOT called
     by `commands/filesystem.rs::*`.
   - **Blast radius note**: `commands/filesystem::*` are also invoked directly
     by the frontend for user-driven file operations. A frontend file browser
     is not normally sandboxed to `~/Juno`; blindly wiring `path_security`
     into these commands would break user-directed browsing.

4. **`commands/text_editor.rs::text_editor_view/create/str_replace/insert`
   — `valid_file_path` + `non_empty_text`** (`text_editor.rs:54, 98, 213, 309`).
   - Same story as #3. The agent-facing Anthropic tool path
     (`anthropic_computer_use.rs::str_replace_editor`) uses `path_security`;
     the Tauri command versions do not. `system_agent.rs` invokes them without
     `debug_mode` (`text_editor_view` at `system_agent.rs:198` — the function
     signature doesn't even accept `debug_mode`, it hardcodes `cfg!(debug_assertions)`
     at `text_editor.rs:46`).
   - `text_editor_view` in release builds has no path check at all.

### Sanity assertions with negligible security value

5. **`commands/core.rs::set_clipboard` — `non_empty_text`** (`core.rs:1021`).
   Empty clipboard is a valid state.

6. **`commands/element.rs::find_element_by_selector /
   click_element_by_selector` — `non_empty_text`** (`element.rs:239, 315`).
   Empty selector just returns "element not found" a few frames later.

7. **`commands/mouse.rs` — `valid_coordinates` (18 sites)**. Out-of-range
   coordinates cause the underlying `cliclick` / CoreGraphics call to no-op
   or click at (0,0). Diagnostic value only.

8. **`commands/keyboard.rs` (5), `commands/window.rs` (6),
   `commands/app_url.rs` (2), `commands/text_editor.rs::non_empty_text` (4)**.
   `non_empty_text` / duration checks. None have security implications
   distinct from what the underlying platform call already enforces.

## Proposed change (phased; separate follow-up issues)

**Do not** flip `production_mode().validate_inputs = true`. That would
enable all 43 sites at once, including the strictly weaker
`valid_file_path` on `filesystem.rs` (which would then contradict the
frontend's need for unbounded file access), and it would leave the two
load-bearing checks still hidden behind a runtime toggle a user could turn
off. Instead:

### Fix A — Security-critical checks, always on (HIGH urgency)

- Move `bash_command`'s `timeout == 0` and `timeout > 3600` checks out of
  `if debug_config.validate_inputs { … }` — they run unconditionally.
- Move `core::wait`'s `valid_duration_seconds` check out of the gate.
- **Also fix**: `wait`'s underlying `std::thread::sleep` in
  `mcp-server-os-level/src/platforms/macos/interaction.rs:2004`. Replace
  with `tokio::task::spawn_blocking` at the `commands/core.rs` boundary, or
  `tokio::time::sleep` at the async level. The cap is defense-in-depth; the
  runtime fix is the real one.
- **Blast radius**: near-zero. `timeout = 0` already breaks execution
  (empty pipe read never completes). `timeout > 3600` on a bash command is
  almost always a model mistake — worth erroring on. The `wait` cap of 60s
  is generous for a UI wait.

### Fix B — Filesystem / text_editor commands (MEDIUM urgency, needs product decision)

- Split each affected Tauri command into an agent-facing variant and a
  user-facing variant, OR route both through `path_security` with a
  configurable roots list.
- Agent-facing: call `path_security::resolve_within_default_roots` for path
  resolution, then use `read_to_string_checked` / `write_checked` for I/O.
  Delete the weaker debug-gated `valid_file_path` check.
- User-facing (invoked from the file picker, drag-drop, etc.): keep an
  unbounded path but require a Tauri-level provenance flag or restrict to
  handles obtained from a system file dialog.
- **Blast radius**: MEDIUM-HIGH. Requires deciding whether user-directed
  file reads/writes from the frontend should also be sandboxed to
  `cwd + ~/Juno`. This is a product call, not just a security call.
- **Interim mitigation** (if the split is deferred): change
  `system_agent.rs`'s calls to `text_editor::*` to pass through
  `path_security` at the callsite instead of trusting the underlying command.

### Fix C — Sanity validators (LOW urgency)

- Delete the `non_empty_text` and `valid_coordinates` checks at the 34
  low-value sites (mouse, window, keyboard, app_url, clipboard,
  selectors). They were dead in release before; making them run adds
  cost and error surface for zero security value.
- Alternative: gate them behind `#[cfg(debug_assertions)]` as compile-time
  assertions, so they help during development but do not exist in the
  release binary at all. Same runtime behaviour as today but removes the
  false sense of a runtime "validation layer".

### Fix D — Retire `DebugConfig::validate_inputs`

- After A/B/C, `validate_inputs` has no remaining call sites. Remove the
  field and update `production_mode` / `development_mode` / `from_build_mode`
  accordingly.
- Consider a broader retirement of the `DebugConfig` pattern in a separate
  cleanup: `log_operations`, `send_notifications`, `time_operations`,
  `emit_visualizations` are all better served by `tracing` filters,
  `#[cfg(debug_assertions)]` guards, or inline `state.is_debug_mode()`
  checks at the specific site that needs them.

## Related follow-ups filed / to file

- `core::wait` blocks a Tokio worker on `std::thread::sleep`
  (`interaction.rs:2004`). Same root as Fix A but a separate defect —
  needs its own issue.
- `text_editor_view`'s hardcoded `cfg!(debug_assertions)` (bypasses even
  the runtime debug toggle) — smallest scope, should be folded into Fix B.
- `commands/mouse.rs` has 18 `validate_inputs` sites for the same
  `valid_coordinates` check. A batch delete is the right move (Fix C),
  not a batch enable.

## Sources

- `src-tauri/src/commands/debug_utils.rs` (whole file; L14-59 for the
  factories, L157-180 for the validator gate, L228-307 for the validators).
- Every call site listed above, cross-checked in-place.
- `src-tauri/src/agent/tools/path_security.rs` (whole file).
- `src-tauri/src/agent/tools/risk_classifier.rs` (whole file).
- `src-tauri/src/permission_gate.rs` (whole file — capability prompts, not
  a validator layer, so not directly relevant beyond confirming it does
  not cover any of the checks here).
- `src-tauri/src/agents/system_agent.rs:58-186` (agent-side callers of
  bash / list_files / get_file_content / set_file_content / text_editor).
- `src-tauri/src/agent/tools/anthropic_computer_use.rs:2185-2295` (agent
  Anthropic-tool callers of `wait` and `bash_command`).
- `src-tauri/src/cloud/commands.rs:682-706` (cloud caller of `bash_command`).
- `src-tauri/mcp-server-os-level/src/platforms/macos/interaction.rs:2000-2008`
  (the `std::thread::sleep` behind `wait`).
- Git log for `debug_utils.rs`: first appears `df82759b` 2025-06-23 as part
  of the same commit that created every affected commands module.
- Git commit `874fb610` (PR #535, 2026-09-11) — precedent for removing the
  debug-mode bypass in `validate_command_security`.

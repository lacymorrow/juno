//! Desktop UI automation through accessibility APIs
//!
//! This module provides a cross-platform API for automating desktop applications
//! through accessibility APIs, inspired by Playwright's web automation model.

use crate::platforms::AccessibilityEngine;
use anyhow::Result;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use serde_json::{self, from_value, json};
use std::collections::HashMap;
use std::fs;
use std::io::Read;
use std::process::{Command, Stdio};
use std::str::FromStr;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tracing::{error, info};

// Make element module public
pub mod background;
pub mod element;
mod errors;
pub mod input_tier;
mod locator;
pub mod platforms;
mod selector;
#[cfg(test)]
mod tests;

// Now UIElement is publicly accessible via computer_use_ai_sdk::element::UIElement
// We still re-export it for convenience
pub use element::{ElementTreeNode, UIElement, UIElementAttributes};
pub use errors::AutomationError;
pub use input_tier::{InputOutcome, InputTier};
pub use locator::Locator;
pub use selector::Selector;

// Log Entry Struct
#[derive(Serialize, Deserialize, Clone)]
pub struct LogEntry {
    timestamp: u64,
    pub level: String,
    pub message: String,
}

// --- Tool Definition Structures (for Anthropic) ---

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ToolInputSchema {
    #[serde(rename = "type")]
    pub type_: String, // Typically "object"
    pub properties: HashMap<String, ToolParameter>,
    pub required: Vec<String>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ToolParameter {
    #[serde(rename = "type")]
    pub type_: String, // e.g., "string", "number", "boolean"
    pub description: String,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ToolDefinition {
    pub name: String,
    pub description: String,
    pub input_schema: ToolInputSchema,
}

// --- End Tool Definition Structures ---

/// Events synthesized by this crate carry this value in the CGEvent
/// event-source user-data field, so the host app's own key monitors and
/// hotkey paths can recognise and ignore them. Spells "JUNO".
pub const SYNTHESIZED_EVENT_MARKER: i64 = 0x4A55_4E4F;

/// How often the `bash` tool checks on a child process while waiting out a
/// caller-supplied timeout.
const BASH_POLL_INTERVAL_MS: u64 = 50;

/// How long the two pipe readers get to hand their buffers over once the
/// command's process group has been killed. SIGKILL closes the pipes at once,
/// so this is slack for thread scheduling, not time for the command to work in.
const BASH_PIPE_DRAIN_GRACE: Duration = Duration::from_secs(2);

/// Kill everything the command started, then reap the direct child.
///
/// `run_shell_command` spawns the shell as the leader of its own process group,
/// so one `killpg` reaches the grandchildren too. Killing only the shell is not
/// enough: `sh -c 'a; b'` forks rather than execs, and the grandchild inherits
/// the write ends of our stdout/stderr pipes. It would keep them open — and the
/// reader threads blocked — for as long as it felt like running, which is how a
/// one-second budget used to take thirty seconds.
fn kill_shell_process_group(child: &mut std::process::Child) {
    #[cfg(unix)]
    {
        // The child is its own group leader, so its pid is the group id. It has
        // not been reaped yet, so the id still belongs to this group and cannot
        // have been recycled onto an unrelated one. Guard the sign anyway: for
        // `killpg`, 0 means "the caller's own group".
        if let Ok(pgid) = i32::try_from(child.id()) {
            if pgid > 0 {
                // SAFETY: `killpg` is a plain signal syscall with no memory
                // effects. `pgid` is a live, unreaped group we created above.
                unsafe {
                    libc::killpg(pgid, libc::SIGKILL);
                }
            }
        }
    }
    // Covers non-unix targets, and the case where the group was already gone.
    let _ = child.kill();
    let _ = child.wait();
}

/// Wait for one reader thread's buffer, giving up at `deadline`.
///
/// `None` means the reader is still blocked on a pipe that some process other
/// than the one we waited on is holding open.
fn take_pipe_output(
    receiver: &std::sync::mpsc::Receiver<Vec<u8>>,
    deadline: Instant,
) -> Option<Vec<u8>> {
    match receiver.recv_timeout(deadline.saturating_duration_since(Instant::now())) {
        Ok(buffer) => Some(buffer),
        // The reader already handed its buffer over, or panicked. Either way
        // nothing more is coming and nothing is blocking.
        Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => Some(Vec::new()),
        Err(std::sync::mpsc::RecvTimeoutError::Timeout) => None,
    }
}

/// Run a shell command, bounded by `timeout_seconds` when the caller supplied one.
///
/// `timeout_seconds: None` is the path this tool has always taken: run to
/// completion, no limit, a hung command hangs the caller. No default is
/// substituted, because imposing one would newly kill slow-but-working commands
/// that work today.
///
/// `Some(seconds)` bounds the whole call, not just the shell's own lifetime. On
/// expiry the shell's entire process group is killed, which closes the pipes the
/// shell's children inherited and lets the reader threads finish; without that,
/// a forked grandchild holds the pipes and the call returns when *it* is done.
/// Whatever output arrived first is returned alongside `timed_out: true`. The
/// two pipes are drained on their own threads: polling `try_wait` while a chatty
/// command fills a pipe buffer would deadlock, since nothing would be reading.
fn run_shell_command(
    program: &str,
    args: &[String],
    timeout_seconds: Option<u64>,
) -> Result<Value, AutomationError> {
    let Some(timeout_seconds) = timeout_seconds else {
        return match Command::new(program).args(args).output() {
            Ok(output) => Ok(json!({
                "stdout": String::from_utf8_lossy(&output.stdout).to_string(),
                "stderr": String::from_utf8_lossy(&output.stderr).to_string(),
                "exit_code": output.status.code(),
                "success": output.status.success()
            })),
            Err(e) => Err(AutomationError::Internal(format!(
                "Failed to execute bash command '{}': {}",
                args.join(" "),
                e
            ))),
        };
    };

    let mut command = Command::new(program);
    command
        .args(args)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        // Give the shell a process group of its own, with itself as leader, so
        // everything it forks can be killed as one unit when the budget runs
        // out. The side effect is that the command is no longer in our
        // terminal's foreground group, which for a non-interactive tool is what
        // we want anyway: it can no longer steal our stdin.
        command.process_group(0);
    }
    let mut child = command.spawn().map_err(|e| {
        AutomationError::Internal(format!(
            "Failed to execute bash command '{}': {}",
            args.join(" "),
            e
        ))
    })?;

    let (stdout_sender, stdout_receiver) = std::sync::mpsc::channel();
    let (stderr_sender, stderr_receiver) = std::sync::mpsc::channel();
    let mut stdout_pipe = child.stdout.take();
    let mut stderr_pipe = child.stderr.take();
    std::thread::spawn(move || {
        let mut buffer = Vec::new();
        if let Some(pipe) = stdout_pipe.as_mut() {
            let _ = pipe.read_to_end(&mut buffer);
        }
        let _ = stdout_sender.send(buffer);
    });
    std::thread::spawn(move || {
        let mut buffer = Vec::new();
        if let Some(pipe) = stderr_pipe.as_mut() {
            let _ = pipe.read_to_end(&mut buffer);
        }
        let _ = stderr_sender.send(buffer);
    });

    // Elapsed time is measured against `Instant`, and compared as an elapsed
    // value rather than a precomputed deadline, so an absurd `timeout_seconds`
    // cannot overflow the addition.
    let budget = Duration::from_secs(timeout_seconds);
    let started_at = Instant::now();
    let mut exit_status = loop {
        match child.try_wait() {
            Ok(Some(status)) => break Some(status),
            Ok(None) => {}
            Err(e) => {
                return Err(AutomationError::Internal(format!(
                    "Failed to poll bash command '{}': {}",
                    args.join(" "),
                    e
                )))
            }
        }
        if started_at.elapsed() >= budget {
            break None;
        }
        std::thread::sleep(Duration::from_millis(BASH_POLL_INTERVAL_MS));
    };

    let mut timeout_error = None;
    if exit_status.is_none() {
        // Budget spent. Killing the group (not just the shell) is what closes
        // the pipes, which is what lets the readers below return.
        kill_shell_process_group(&mut child);
        timeout_error = Some(format!(
            "bash command exceeded its {}s timeout and was killed",
            timeout_seconds
        ));
    }

    // The readers get whatever is left of the budget, and never less than the
    // handover grace, since after a kill there is no budget left to give them.
    let drain_deadline = Instant::now()
        + budget
            .saturating_sub(started_at.elapsed())
            .max(BASH_PIPE_DRAIN_GRACE);
    let stdout_buffer = take_pipe_output(&stdout_receiver, drain_deadline);
    let stderr_buffer = take_pipe_output(&stderr_receiver, drain_deadline);

    if stdout_buffer.is_none() || stderr_buffer.is_none() {
        // The shell is gone but a process it started still holds a pipe open —
        // it escaped the group, by calling `setsid` or similar. Waiting on it is
        // the unbounded wait this timeout exists to prevent, so stop waiting.
        // The reader threads are left to finish and exit on their own; nothing
        // else is holding them.
        exit_status = None;
        timeout_error = Some(format!(
            "bash command exceeded its {}s timeout; a process it started outlived it \
             and is still holding its output pipes open, so this output may be incomplete",
            timeout_seconds
        ));
    }

    let stdout = String::from_utf8_lossy(&stdout_buffer.unwrap_or_default()).to_string();
    let stderr = String::from_utf8_lossy(&stderr_buffer.unwrap_or_default()).to_string();

    match exit_status {
        Some(status) => Ok(json!({
            "stdout": stdout,
            "stderr": stderr,
            "exit_code": status.code(),
            "success": status.success(),
            "timed_out": false
        })),
        None => Ok(json!({
            "stdout": stdout,
            "stderr": stderr,
            "exit_code": Value::Null,
            "success": false,
            "timed_out": true,
            "error": timeout_error
        })),
    }
}

/// Insert text into the focused app without touching the pasteboard.
///
/// Posts the text as unicode keyboard events to the AX-focused process
/// (frontmost app, then the HID tap, when no PID is known), falling back to
/// AX insert at the selection, then clipboard paste, then per-character
/// typing. Apps that reject unicode keystrokes (Ghostty) are pasted into
/// directly. Returns a label naming the path that succeeded, for logging.
#[cfg(target_os = "macos")]
pub fn insert_text_clipboard_free(text: &str) -> Result<&'static str, AutomationError> {
    platforms::macos::text_insertion::insert_text_clipboard_free(text)
}

#[cfg(not(target_os = "macos"))]
pub fn insert_text_clipboard_free(_text: &str) -> Result<&'static str, AutomationError> {
    Err(AutomationError::UnsupportedOperation(
        "Clipboard-free text insertion is only implemented on macOS".to_string(),
    ))
}

/// Paste `text` into the frontmost app with Cmd+V.
///
/// With `retain_clipboard` false the previous pasteboard contents are
/// snapshotted and restored after ~500 ms (changeCount-guarded), and the
/// temporary item is marked transient so clipboard managers skip it. With
/// true, the text is written as an ordinary pasteboard item and stays there —
/// dictation's "leave the transcript on the clipboard".
#[cfg(target_os = "macos")]
pub fn paste_text_global(text: &str, retain_clipboard: bool) -> Result<(), AutomationError> {
    platforms::macos::interaction::paste_text(text, None, retain_clipboard)
}

#[cfg(not(target_os = "macos"))]
pub fn paste_text_global(_text: &str, _retain_clipboard: bool) -> Result<(), AutomationError> {
    Err(AutomationError::UnsupportedOperation(
        "Global paste is only implemented on macOS".to_string(),
    ))
}

// Define a new struct to hold click result information - move to module level
#[derive(Debug)]
pub struct ClickResult {
    pub method: String,
    pub coordinates: Option<(f64, f64)>,
    pub details: String,
}

/// The main entry point for UI automation
#[derive(Clone)]
pub struct Desktop {
    engine: Arc<dyn platforms::AccessibilityEngine + Send + Sync>,
    #[allow(dead_code)] // Keep field, might be used for platform differences or config later
    use_background_apps: bool,
    #[allow(dead_code)] // Keep field, might be used for platform differences or config later
    activate_app: bool,
}

impl Desktop {
    /// Initializes the Desktop environment.
    pub fn new(use_background_apps: bool, activate_app: bool) -> Result<Self, AutomationError> {
        let engine_result = platforms::create_engine(use_background_apps, activate_app);

        match engine_result {
            Ok(engine) => {
                info!("Desktop engine initialized successfully.");
                Ok(Self {
                    engine: Arc::from(engine as Box<dyn AccessibilityEngine + Send + Sync>),
                    use_background_apps,
                    activate_app,
                })
            }
            Err(e) => {
                error!("Failed to initialize desktop engine: {}", e);
                Err(e)
            }
        }
    }

    /// Initializes the Desktop environment with auto-redirect permission handling.
    /// When permissions are denied, automatically opens System Settings for the user.
    pub fn new_with_auto_redirect(
        use_background_apps: bool,
        activate_app: bool,
        auto_open_settings: bool,
    ) -> Result<Self, AutomationError> {
        let engine_result = platforms::create_engine_with_auto_redirect(
            use_background_apps,
            activate_app,
            auto_open_settings,
        );

        match engine_result {
            Ok(engine) => {
                info!("Desktop engine with auto-redirect initialized successfully.");
                Ok(Self {
                    engine: Arc::from(engine as Box<dyn AccessibilityEngine + Send + Sync>),
                    use_background_apps,
                    activate_app,
                })
            }
            Err(e) => {
                error!(
                    "Failed to initialize desktop engine with auto-redirect: {}",
                    e
                );
                Err(e)
            }
        }
    }

    /// Returns a reference to the underlying accessibility engine.
    pub fn engine(&self) -> Arc<dyn platforms::AccessibilityEngine + Send + Sync> {
        self.engine.clone()
    }

    /// Get the root UI element representing the entire desktop
    pub fn root(&self) -> UIElement {
        self.engine.get_root_element()
    }

    /// Create a locator to find elements matching the given selector
    pub fn locator(&self, selector: impl Into<Selector>) -> Locator {
        Locator::new(Arc::clone(&self.engine), selector.into())
    }

    /// Returns the accessibility element at the given screen coordinates, if any.
    /// Uses native platform hit-testing (~1-5ms on macOS).
    pub fn element_at_position(&self, x: f64, y: f64) -> Option<UIElement> {
        self.engine.element_at_position(x, y)
    }

    /// Get the currently focused element
    pub fn focused_element(&self) -> Result<UIElement, AutomationError> {
        self.engine.get_focused_element()
    }

    /// List all running applications
    pub fn applications(&self) -> Result<Vec<UIElement>, AutomationError> {
        self.engine.get_applications()
    }

    /// Find an application by name
    pub fn application(&self, name: &str) -> Result<UIElement, AutomationError> {
        self.engine.get_application_by_name(name)
    }

    /// Open an application by name
    pub fn open_application(&self, app_name: &str) -> Result<UIElement, AutomationError> {
        self.engine.open_application(app_name)
    }

    /// Open a URL in a specified browser (or default browser if None)
    pub fn open_url(&self, url: &str, browser: Option<&str>) -> Result<UIElement, AutomationError> {
        self.engine.open_url(url, browser)
    }

    /// Type text globally using keyboard simulation.
    pub fn type_text(&self, text: &str) -> Result<(), AutomationError> {
        self.engine.type_text(text)
    }

    /// Get the current clipboard content
    pub fn get_clipboard_content(&self) -> Result<String, AutomationError> {
        self.engine.get_clipboard_content()
    }

    /// Set the clipboard content
    pub fn set_clipboard_content(&self, content: &str) -> Result<(), AutomationError> {
        self.engine.set_clipboard_content(content)
    }

    /// Hold down a modifier key.
    pub fn hold_key(&self, key: &str, duration_ms: Option<u64>) -> Result<(), AutomationError> {
        self.engine.hold_key(key, duration_ms)
    }

    /// Release a modifier key.
    pub fn release_key(&self, key: &str) -> Result<(), AutomationError> {
        self.engine.release_key(key)
    }

    /// Wait for a specified duration.
    pub fn wait(&self, duration_ms: u64) -> Result<(), AutomationError> {
        self.engine.wait(duration_ms)
    }

    /// Get the current mouse cursor position.
    pub fn cursor_position(&self) -> Result<(f64, f64), AutomationError> {
        self.engine.cursor_position()
    }

    /// Move the mouse cursor to the specified coordinates.
    pub fn mouse_move(&self, x: f64, y: f64) -> Result<(), AutomationError> {
        self.engine.mouse_move(x, y)
    }

    /// Simulate pressing the left mouse button down at the specified coordinates.
    pub fn left_mouse_down(&self, x: f64, y: f64) -> Result<(), AutomationError> {
        self.engine.left_mouse_down(x, y)
    }

    /// Simulate releasing the left mouse button at the specified coordinates.
    pub fn left_mouse_up(&self, x: f64, y: f64) -> Result<(), AutomationError> {
        self.engine.left_mouse_up(x, y)
    }

    /// Simulate a standard left click (down + up) at specified coordinates.
    pub fn left_click(
        &self,
        x: f64,
        y: f64,
        modifiers: Option<&str>,
    ) -> Result<(), AutomationError> {
        self.engine.left_click(x, y, modifiers)
    }

    /// Click without warping the system cursor — tiered: SkyLight → CGEventPostToPid → HID-restore.
    ///
    /// `allow_physical` gates the last tier. `Ok(None)` means the step needs the
    /// physical cursor and consent for it has not been given.
    pub fn left_click_no_warp(
        &self,
        x: f64,
        y: f64,
        modifiers: Option<&str>,
        allow_physical: bool,
    ) -> Result<Option<InputOutcome>, AutomationError> {
        self.engine
            .left_click_no_warp(x, y, modifiers, allow_physical)
    }

    /// Right-click without warping the cursor.
    pub fn right_click_no_warp(
        &self,
        x: f64,
        y: f64,
        allow_physical: bool,
    ) -> Result<Option<InputOutcome>, AutomationError> {
        self.engine.right_click_no_warp(x, y, allow_physical)
    }

    /// Middle-click without warping the cursor.
    pub fn middle_click_no_warp(
        &self,
        x: f64,
        y: f64,
        modifiers: Option<&str>,
        allow_physical: bool,
    ) -> Result<Option<InputOutcome>, AutomationError> {
        self.engine
            .middle_click_no_warp(x, y, modifiers, allow_physical)
    }

    /// Double-click without warping the cursor.
    pub fn double_click_no_warp(
        &self,
        x: f64,
        y: f64,
        modifiers: Option<&str>,
        allow_physical: bool,
    ) -> Result<Option<InputOutcome>, AutomationError> {
        self.engine
            .double_click_no_warp(x, y, modifiers, allow_physical)
    }

    /// Triple-click without warping the cursor.
    pub fn triple_click_no_warp(
        &self,
        x: f64,
        y: f64,
        modifiers: Option<&str>,
        allow_physical: bool,
    ) -> Result<Option<InputOutcome>, AutomationError> {
        self.engine
            .triple_click_no_warp(x, y, modifiers, allow_physical)
    }

    /// Press the left button without warping the cursor.
    pub fn left_mouse_down_no_warp(
        &self,
        x: f64,
        y: f64,
        allow_physical: bool,
    ) -> Result<Option<InputOutcome>, AutomationError> {
        self.engine.left_mouse_down_no_warp(x, y, allow_physical)
    }

    /// Release the left button without warping the cursor.
    pub fn left_mouse_up_no_warp(
        &self,
        x: f64,
        y: f64,
        allow_physical: bool,
    ) -> Result<Option<InputOutcome>, AutomationError> {
        self.engine.left_mouse_up_no_warp(x, y, allow_physical)
    }

    /// Drag without warping the cursor.
    pub fn left_click_drag_no_warp(
        &self,
        start_x: f64,
        start_y: f64,
        end_x: f64,
        end_y: f64,
        allow_physical: bool,
    ) -> Result<Option<InputOutcome>, AutomationError> {
        self.engine
            .left_click_drag_no_warp(start_x, start_y, end_x, end_y, allow_physical)
    }

    /// Scroll at a point without moving the real cursor there first.
    pub fn scroll_no_warp(
        &self,
        x: f64,
        y: f64,
        direction: &str,
        amount: f64,
        modifiers: Option<&str>,
        allow_physical: bool,
    ) -> Result<Option<InputOutcome>, AutomationError> {
        self.engine
            .scroll_no_warp(x, y, direction, amount, modifiers, allow_physical)
    }

    /// Press a key against the process the agent is working on.
    pub fn press_key_no_warp(
        &self,
        key_name: &str,
        modifier: Option<&str>,
        allow_physical: bool,
    ) -> Result<Option<InputOutcome>, AutomationError> {
        self.engine
            .press_key_no_warp(key_name, modifier, allow_physical)
    }

    /// Hold a key against the background target.
    pub fn hold_key_no_warp(
        &self,
        key: &str,
        duration_ms: Option<u64>,
        allow_physical: bool,
    ) -> Result<Option<InputOutcome>, AutomationError> {
        self.engine
            .hold_key_no_warp(key, duration_ms, allow_physical)
    }

    /// Release a key against the background target.
    pub fn release_key_no_warp(
        &self,
        key: &str,
        allow_physical: bool,
    ) -> Result<Option<InputOutcome>, AutomationError> {
        self.engine.release_key_no_warp(key, allow_physical)
    }

    /// Type text into the process the agent is working on.
    pub fn type_text_no_warp(
        &self,
        text: &str,
        allow_physical: bool,
    ) -> Result<Option<InputOutcome>, AutomationError> {
        self.engine.type_text_no_warp(text, allow_physical)
    }

    /// Post a mouse event directly to a specific process by PID without moving the cursor.
    pub fn post_mouse_event_to_pid(
        &self,
        pid: i32,
        event_type_str: &str,
        x: f64,
        y: f64,
    ) -> Result<(), AutomationError> {
        self.engine
            .post_mouse_event_to_pid(pid, event_type_str, x, y)
    }

    /// Post a key event directly to a specific process by PID without changing focus.
    pub fn post_key_event_to_pid(
        &self,
        pid: i32,
        keycode: u16,
        key_down: bool,
    ) -> Result<(), AutomationError> {
        self.engine.post_key_event_to_pid(pid, keycode, key_down)
    }

    /// Simulate a right click (down + up) at specified coordinates.
    pub fn right_click(
        &self,
        x: f64,
        y: f64,
        modifiers: Option<&str>,
    ) -> Result<(), AutomationError> {
        self.engine.right_click(x, y, modifiers)
    }

    /// Simulate a middle click (down + up) at specified coordinates.
    pub fn middle_click(
        &self,
        x: f64,
        y: f64,
        modifiers: Option<&str>,
    ) -> Result<(), AutomationError> {
        self.engine.middle_click(x, y, modifiers)
    }

    /// Simulate a double left click at the specified coordinates.
    pub fn double_click(
        &self,
        x: f64,
        y: f64,
        modifiers: Option<&str>,
    ) -> Result<(), AutomationError> {
        self.engine.double_click(x, y, modifiers)
    }

    /// Simulate a triple left click at the specified coordinates.
    pub fn triple_click(
        &self,
        x: f64,
        y: f64,
        modifiers: Option<&str>,
    ) -> Result<(), AutomationError> {
        self.engine.triple_click(x, y, modifiers)
    }

    /// Simulate dragging with the left mouse button from a start point to an end point.
    pub fn left_click_drag(
        &self,
        start_x: f64,
        start_y: f64,
        end_x: f64,
        end_y: f64,
    ) -> Result<(), AutomationError> {
        self.engine.left_click_drag(start_x, start_y, end_x, end_y)
    }

    /// Scroll at a specific position on screen
    pub fn scroll_at_position(
        &self,
        x: f64,
        y: f64,
        direction: &str,
        amount: f64,
    ) -> Result<(), AutomationError> {
        self.engine.scroll_at_position(x, y, direction, amount)
    }

    /// Scroll at the current mouse position
    pub fn scroll_at_current_position(
        &self,
        direction: &str,
        amount: f64,
    ) -> Result<(), AutomationError> {
        self.engine.scroll_at_current_position(direction, amount)
    }

    /// List all windows
    pub fn list_windows(&self) -> Result<Vec<UIElement>, AutomationError> {
        self.engine.list_windows()
    }

    /// Press a single key with an optional modifier
    pub fn press_key(&self, key_name: &str, modifier: Option<&str>) -> Result<(), AutomationError> {
        self.engine.press_key(key_name, modifier)
    }

    // --- New Methods for Agent Loop ---

    /// Returns the list of available tools for this Desktop instance.
    /// Used by: MCP server initialization and tool discovery.
    pub fn list_tools(&self) -> Vec<ToolDefinition> {
        let tools = vec![
            // --- OFFICIAL ANTHROPIC COMPUTER USE TOOLS ---
            // Following the official specification: https://docs.anthropic.com/en/docs/agents-and-tools/tool-use/computer-use-tool

            // Computer Tool (computer_20250124) - Single tool for all computer operations
            ToolDefinition {
                name: "computer".to_string(),
                description: "Use a mouse and keyboard to interact with a computer, and take screenshots. This is the official Anthropic Computer Use tool that handles all desktop interaction through action parameters.".to_string(),
                input_schema: ToolInputSchema {
                    type_: "object".to_string(),
                    properties: {
                        let mut props = HashMap::new();
                        props.insert(
                            "action".to_string(),
                            ToolParameter {
                                type_: "string".to_string(),
                                description: "The action to perform: screenshot, left_click, right_click, middle_click, double_click, triple_click, left_click_drag, mouse_move, left_mouse_down, left_mouse_up, type, key, hold_key, scroll, wait, cursor_position".to_string(),
                            },
                        );
                        props.insert(
                            "coordinate".to_string(),
                            ToolParameter {
                                type_: "array".to_string(),
                                description: "Array of [x, y] coordinates for click, mouse actions, and end position of drag actions. Required for mouse actions.".to_string(),
                            },
                        );
                        // Note: Removed legacy start_coordinate and end_coordinate parameters
                        // Following official Anthropic Computer Use specification:
                        // Drag operations use only 'coordinate' (end position) - start is current cursor position
                        props.insert(
                            "text".to_string(),
                            ToolParameter {
                                type_: "string".to_string(),
                                description: "Text to type or key combination to press (e.g., 'ctrl+s', 'Return').".to_string(),
                            },
                        );
                        props.insert(
                            "scroll_direction".to_string(),
                            ToolParameter {
                                type_: "string".to_string(),
                                description: "Direction to scroll: up, down, left, right.".to_string(),
                            },
                        );
                        props.insert(
                            "scroll_amount".to_string(),
                            ToolParameter {
                                type_: "number".to_string(),
                                description: "Amount to scroll (default: 3).".to_string(),
                            },
                        );
                        props.insert(
                            "duration_ms".to_string(),
                            ToolParameter {
                                type_: "number".to_string(),
                                description: "Duration in MILLISECONDS for wait or hold_key actions. The unit is in the name: this server takes milliseconds, unlike Anthropic's canonical computer tool where a bare 'duration' means seconds. A legacy 'duration' key is still accepted here and is also milliseconds.".to_string(),
                            },
                        );
                        props
                    },
                    // Note: Only action is universally required. Other parameters are conditionally required based on action.
                    // This matches the official Anthropic Computer Use specification.
                    required: vec!["action".to_string()],
                },
            },

            // Text Editor Tool (str_replace_based_edit_tool) - Official Anthropic tool
            ToolDefinition {
                name: "str_replace_based_edit_tool".to_string(),
                description: "Custom editing tool for viewing, creating and editing files".to_string(),
                input_schema: ToolInputSchema {
                    type_: "object".to_string(),
                    properties: {
                        let mut props = HashMap::new();
                        props.insert(
                            "command".to_string(),
                            ToolParameter {
                                type_: "string".to_string(),
                                description: "The command to run: view, create, str_replace, insert".to_string(),
                            },
                        );
                        props.insert(
                            "path".to_string(),
                            ToolParameter {
                                type_: "string".to_string(),
                                description: "Absolute path to file or directory".to_string(),
                            },
                        );
                        props.insert(
                            "file_text".to_string(),
                            ToolParameter {
                                type_: "string".to_string(),
                                description: "Content for create command".to_string(),
                            },
                        );
                        props.insert(
                            "old_str".to_string(),
                            ToolParameter {
                                type_: "string".to_string(),
                                description: "String to replace in str_replace command".to_string(),
                            },
                        );
                        props.insert(
                            "new_str".to_string(),
                            ToolParameter {
                                type_: "string".to_string(),
                                description: "Replacement string for str_replace or text for insert command".to_string(),
                            },
                        );
                        props.insert(
                            "insert_line".to_string(),
                            ToolParameter {
                                type_: "integer".to_string(),
                                description: "Line number for insert command. Use 0 to insert at beginning, 1 to insert after line 1, 2 to insert after line 2, etc.".to_string(),
                            },
                        );
                        props
                    },
                    required: vec!["command".to_string(), "path".to_string()],
                },
            },

            // Bash Tool - Official Anthropic tool
            ToolDefinition {
                name: "bash".to_string(),
                description: "Run commands in a bash shell".to_string(),
                input_schema: ToolInputSchema {
                    type_: "object".to_string(),
                    properties: {
                        let mut props = HashMap::new();
                        props.insert(
                            "command".to_string(),
                            ToolParameter {
                                type_: "string".to_string(),
                                description: "The bash command to run".to_string(),
                            },
                        );
                        props.insert(
                            "restart".to_string(),
                            ToolParameter {
                                type_: "boolean".to_string(),
                                description: "Set to true to restart the bash environment".to_string(),
                            },
                        );
                        props.insert(
                            "timeout".to_string(),
                            ToolParameter {
                                type_: "integer".to_string(),
                                description: "Optional wall-clock limit in SECONDS (minimum 1). The command is killed when it is exceeded. Omit it to run the command to completion with no limit.".to_string(),
                            },
                        );
                        props
                    },
                    required: vec!["command".to_string()],
                },
            },

            // --- DESKTOP ACCESSIBILITY TOOLS ---
            // These are MCP-specific tools for accessibility interface, not covered by Anthropic Computer Use

            ToolDefinition {
                name: "getUiTree".to_string(),
                description: "Gets the UI tree structure for accessibility analysis. This provides structured access to UI elements beyond what screenshot analysis can provide.".to_string(),
                input_schema: ToolInputSchema {
                    type_: "object".to_string(),
                    properties: {
                        let mut props = HashMap::new();
                        props.insert(
                            "application_name".to_string(),
                            ToolParameter {
                                type_: "string".to_string(),
                                description: "Optional name of the application to get the tree for. If omitted, uses the focused application.".to_string(),
                            },
                        );
                        props
                    },
                    required: vec![], // application_name is optional
                },
            },
            ToolDefinition {
                name: "captureScreenshot".to_string(),
                description: "Captures a screenshot of the entire screen and returns it as a base64 encoded PNG.".to_string(),
                input_schema: ToolInputSchema {
                    type_: "object".to_string(),
                    properties: HashMap::new(),
                    required: Vec::new(),
                },
            },
            ToolDefinition {
                name: "getClipboard".to_string(),
                description: "Gets the current content of the system clipboard.".to_string(),
                input_schema: ToolInputSchema {
                    type_: "object".to_string(),
                    properties: HashMap::new(), // No input parameters
                    required: Vec::new(),
                },
            },
            ToolDefinition {
                name: "setClipboard".to_string(),
                description: "Sets the system clipboard to the specified text content.".to_string(),
                input_schema: ToolInputSchema {
                    type_: "object".to_string(),
                    properties: [(
                        "content".to_string(),
                        ToolParameter {
                            type_: "string".to_string(),
                            description: "The text content to set the clipboard to.".to_string(),
                        },
                    )]
                    .iter()
                    .cloned()
                    .collect(),
                    required: vec!["content".to_string()],
                },
            },
            ToolDefinition {
                name: "findElementsBySelector".to_string(),
                description: "Finds UI elements matching a specified selector (e.g., role, title, description). Returns a list of element attributes.".to_string(),
                input_schema: ToolInputSchema {
                    type_: "object".to_string(),
                    properties: {
                        let mut props = HashMap::new();
                        props.insert(
                            "selector".to_string(),
                            ToolParameter {
                                type_: "string".to_string(),
                                description: "The selector string (e.g., 'button[title=\"OK\"]').".to_string(),
                            },
                        );
                        // Optional root element ID might be added here later
                        props
                    },
                    required: vec!["selector".to_string()],
                },
            },
            ToolDefinition {
                name: "getElementAttributes".to_string(),
                description: "Gets the accessibility attributes of a UI element specified by a selector.".to_string(),
                input_schema: ToolInputSchema {
                    type_: "object".to_string(),
                    properties: {
                        let mut props = HashMap::new();
                        props.insert(
                            "selector".to_string(),
                            ToolParameter {
                                type_: "string".to_string(),
                                description: "The selector string for the element (e.g., 'button[title=\"OK\"]').".to_string(),
                            },
                        );
                        // Optional root element ID might be added here later
                        props
                    },
                    required: vec!["selector".to_string()],
                },
            },

            // --- APPLICATION MANAGEMENT TOOLS ---
            // These are system-level tools not covered by Anthropic Computer Use

            ToolDefinition {
                name: "open_application".to_string(),
                description: "Opens an application specified by its name.".to_string(),
                input_schema: ToolInputSchema {
                    type_: "object".to_string(),
                    properties: [
                        ("app_name".to_string(), ToolParameter {
                            type_: "string".to_string(),
                            description: "The name of the application to open (e.g., 'Safari', 'Terminal').".to_string(),
                        })
                    ].iter().cloned().collect(),
                    required: vec!["app_name".to_string()],
                },
            },
            ToolDefinition {
                name: "open_url".to_string(),
                description: "Opens a URL in the default web browser or a specified one.".to_string(),
                input_schema: ToolInputSchema {
                    type_: "object".to_string(),
                    properties: [
                        ("url".to_string(), ToolParameter {
                            type_: "string".to_string(),
                            description: "The full URL to open (e.g., 'https://www.google.com').".to_string(),
                        }),
                        ("browser".to_string(), ToolParameter {
                            type_: "string".to_string(),
                            description: "Optional. The name of the browser to use (e.g., 'Safari', 'Chrome'). Defaults to the system default browser if not specified.".to_string(),
                        })
                    ].iter().cloned().collect(),
                    required: vec!["url".to_string()],
                },
            },
            ToolDefinition {
                name: "get_focused_element_info".to_string(),
                description: "Gets information about the currently focused UI element on the screen.".to_string(),
                input_schema: ToolInputSchema {
                    type_: "object".to_string(),
                    properties: HashMap::new(), // No parameters needed
                    required: Vec::new(),
                },
            },

            // --- UI ELEMENT INTERACTION TOOLS ---
            // These provide selector-based interaction, complementing the coordinate-based computer tool

            ToolDefinition {
                name: "find_element".to_string(),
                description: "Finds a UI element based on a CSS-like selector.".to_string(),
                input_schema: ToolInputSchema {
                    type_: "object".to_string(),
                    properties: [
                        ("selector".to_string(), ToolParameter {
                            type_: "string".to_string(),
                            description: "The CSS-like selector string (e.g., 'window[title=\"Calculator\"] button[label=\"1\"]').".to_string(),
                        })
                    ].iter().cloned().collect(),
                    required: vec!["selector".to_string()],
                },
            },
            ToolDefinition {
                name: "click".to_string(),
                description: "Clicks on a UI element specified by a selector. Note: For coordinate-based clicking, use the 'computer' tool with action 'left_click'.".to_string(),
                input_schema: ToolInputSchema {
                    type_: "object".to_string(),
                    properties: [
                        ("selector".to_string(), ToolParameter {
                            type_: "string".to_string(),
                            description: "The selector for the element to click.".to_string(),
                        })
                    ].iter().cloned().collect(),
                    required: vec!["selector".to_string()],
                },
            },
            ToolDefinition {
                name: "type_text".to_string(),
                description: "Types text into a UI element specified by a selector. Note: For general text typing, use the 'computer' tool with action 'type'.".to_string(),
                input_schema: ToolInputSchema {
                    type_: "object".to_string(),
                    properties: [
                        ("selector".to_string(), ToolParameter {
                            type_: "string".to_string(),
                            description: "The selector for the element to type into.".to_string(),
                        }),
                        ("text".to_string(), ToolParameter {
                            type_: "string".to_string(),
                            description: "The text to type.".to_string(),
                        }),
                    ].iter().cloned().collect(),
                    required: vec!["selector".to_string(), "text".to_string()],
                },
            },
             ToolDefinition {
                name: "scroll_element".to_string(),
                description: "Scrolls a UI element specified by a selector. Note: For general scrolling, use the 'computer' tool with action 'scroll'.".to_string(),
                input_schema: ToolInputSchema {
                    type_: "object".to_string(),
                    properties: [
                        ("selector".to_string(), ToolParameter {
                            type_: "string".to_string(),
                            description: "The selector for the element to scroll.".to_string(),
                        }),
                        ("direction".to_string(), ToolParameter {
                            type_: "string".to_string(),
                            description: "The direction to scroll ('up', 'down', 'left', 'right').".to_string(),
                        }),
                        ("amount".to_string(), ToolParameter {
                            type_: "number".to_string(),
                            description: "The amount to scroll (default: 3).".to_string(),
                        }),
                    ].iter().cloned().collect(),
                    required: vec!["selector".to_string(), "direction".to_string()],
                },
            },
            ToolDefinition {
                name: "typeText".to_string(),
                description: "Types text at the current cursor position or into the focused element. Note: This is an alias for compatibility - prefer using the 'computer' tool with action 'type'.".to_string(),
                input_schema: ToolInputSchema {
                    type_: "object".to_string(),
                    properties: HashMap::from([
                        ("text".to_string(), ToolParameter { type_: "string".to_string(), description: "The text to type.".to_string() }),
                    ]),
                    required: vec!["text".to_string()],
                },
            },
        ];

        // REMOVED: All individual mouse and keyboard tools that don't follow Anthropic Computer Use spec
        // The following tools have been consolidated into the unified 'computer' tool:
        // - holdKey, releaseKey, pressKey -> computer tool with action: "hold_key", "key"
        // - mouseMove -> computer tool with action: "mouse_move"
        // - leftMouseDown, leftMouseUp -> computer tool with action: "left_mouse_down", "left_mouse_up"
        // - leftClick, rightClick, middleClick -> computer tool with action: "left_click", "right_click", "middle_click"
        // - doubleClick, tripleClick -> computer tool with action: "double_click", "triple_click"
        // - leftClickDrag -> computer tool with action: "left_click_drag"
        // - cursorPosition -> computer tool with action: "cursor_position"
        // - wait -> computer tool with action: "wait"
        //
        // This consolidation ensures 100% compliance with the official Anthropic Computer Use specification
        // while maintaining all functionality through the unified computer tool interface.

        tools
    }

    /// Call a specific tool by name with given arguments
    pub fn call_tool(&self, name: &str, args: Value) -> Result<Value, AutomationError> {
        info!("Calling tool: {} with args: {}", name, args);

        match name {
            "open_application" => {
                let app_name = args
                    .get("app_name")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| {
                        AutomationError::InvalidArgument(
                            "Missing or invalid 'app_name' argument".to_string(),
                        )
                    })?;
                self.open_application(app_name)?;
                Ok(
                    serde_json::json!({"status": "success", "message": format!("Application '{}' opened.", app_name)}),
                )
            }
            "open_url" => {
                let url = args.get("url").and_then(|v| v.as_str()).ok_or_else(|| {
                    AutomationError::InvalidArgument(
                        "Missing or invalid 'url' argument".to_string(),
                    )
                })?;
                let browser = args.get("browser").and_then(|v| v.as_str()); // Optional
                self.open_url(url, browser)?;
                Ok(
                    serde_json::json!({"status": "success", "message": format!("URL '{}' opened.", url)}),
                )
            }
            "typeText" => {
                let text = args["text"].as_str().ok_or_else(|| {
                    AutomationError::InvalidArgument(
                        "Missing or invalid 'text' argument".to_string(),
                    )
                })?;
                self.type_text(text)?;
                Ok(Value::Null)
            }
            "get_focused_element_info" => {
                let element = self.focused_element()?;
                let attributes = element.attributes();
                // Convert attributes to JSON value
                let result_json = serde_json::to_value(attributes).map_err(|e| {
                    AutomationError::Internal(format!(
                        "Failed to serialize element attributes: {}",
                        e
                    ))
                })?;
                Ok(result_json)
            }
            "find_element" => {
                let selector_str =
                    args.get("selector")
                        .and_then(|v| v.as_str())
                        .ok_or_else(|| {
                            AutomationError::InvalidArgument(
                                "Missing or invalid 'selector' argument".to_string(),
                            )
                        })?;
                let selector: Selector = selector_str.into();
                let element_option = self.locator(selector).first()?;
                if let Some(element) = element_option {
                    let attributes = element.attributes();
                    let result_json = serde_json::to_value(attributes).map_err(|e| {
                        AutomationError::Internal(format!(
                            "Failed to serialize element attributes: {}",
                            e
                        ))
                    })?;
                    Ok(serde_json::json!({
                       "status": "success",
                       "element_found": true,
                       "attributes": result_json
                    }))
                } else {
                    Err(AutomationError::ElementNotFound(format!(
                        "Element not found for selector: {}",
                        selector_str
                    )))
                }
            }
            "click" => {
                let selector_str =
                    args.get("selector")
                        .and_then(|v| v.as_str())
                        .ok_or_else(|| {
                            AutomationError::InvalidArgument(
                                "Missing or invalid 'selector' argument".to_string(),
                            )
                        })?;
                let selector: Selector = selector_str.into();
                let element_option = self.locator(selector).first()?;
                if let Some(element) = element_option {
                    let click_result = element.click()?;
                    Ok(serde_json::json!({
                        "status": "success",
                        "message": format!("Clicked element matching selector '{}'. Method: {}, Details: {}", selector_str, click_result.method, click_result.details),
                        "coordinates": click_result.coordinates
                    }))
                } else {
                    Err(AutomationError::ElementNotFound(format!(
                        "Element not found for selector: {}",
                        selector_str
                    )))
                }
            }
            "type_text" => {
                let selector_str =
                    args.get("selector")
                        .and_then(|v| v.as_str())
                        .ok_or_else(|| {
                            AutomationError::InvalidArgument(
                                "Missing or invalid 'selector' argument".to_string(),
                            )
                        })?;
                let text_to_type = args.get("text").and_then(|v| v.as_str()).ok_or_else(|| {
                    AutomationError::InvalidArgument(
                        "Missing or invalid 'text' argument".to_string(),
                    )
                })?;
                let selector: Selector = selector_str.into();
                let element_option = self.locator(selector).first()?;
                if let Some(element) = element_option {
                    element.type_text(text_to_type)?;
                    Ok(serde_json::json!({
                        "status": "success",
                        "message": format!("Typed text into element matching selector '{}'", selector_str)
                    }))
                } else {
                    Err(AutomationError::ElementNotFound(format!(
                        "Element not found for selector: {}",
                        selector_str
                    )))
                }
            }
            "get_element_attributes" => {
                let selector_str =
                    args.get("selector")
                        .and_then(|v| v.as_str())
                        .ok_or_else(|| {
                            AutomationError::InvalidArgument(
                                "Missing or invalid 'selector' argument".to_string(),
                            )
                        })?;
                let selector: Selector = selector_str.into();
                let element_option = self.locator(selector).first()?;
                if let Some(element) = element_option {
                    let attributes = element.attributes();
                    let result_json = serde_json::to_value(attributes).map_err(|e| {
                        AutomationError::Internal(format!(
                            "Failed to serialize element attributes: {}",
                            e
                        ))
                    })?;
                    Ok(result_json)
                } else {
                    Err(AutomationError::ElementNotFound(format!(
                        "Element not found for selector: {}",
                        selector_str
                    )))
                }
            }
            "scroll_element" => {
                let selector_str =
                    args.get("selector")
                        .and_then(|v| v.as_str())
                        .ok_or_else(|| {
                            AutomationError::InvalidArgument(
                                "Missing or invalid 'selector' argument".to_string(),
                            )
                        })?;
                let direction =
                    args.get("direction")
                        .and_then(|v| v.as_str())
                        .ok_or_else(|| {
                            AutomationError::InvalidArgument(
                                "Missing or invalid 'direction' argument".to_string(),
                            )
                        })?;
                let amount = args.get("amount").and_then(|v| v.as_f64()).ok_or_else(|| {
                    AutomationError::InvalidArgument(
                        "Missing or invalid 'amount' argument".to_string(),
                    )
                })?;
                let selector: Selector = selector_str.into();
                let element_option = self.locator(selector).first()?;
                if let Some(element) = element_option {
                    element.scroll(direction, amount)?;
                    Ok(serde_json::json!({
                        "status": "success",
                        "message": format!("Scrolled element matching selector '{}'", selector_str)
                    }))
                } else {
                    Err(AutomationError::ElementNotFound(format!(
                        "Element not found for selector: {}",
                        selector_str
                    )))
                }
            }
            "getUiTree" => {
                #[derive(Deserialize)]
                struct GetUiTreeArgs {
                    application_name: Option<String>,
                }
                let parsed_args: GetUiTreeArgs = from_value(args).map_err(|e| {
                    AutomationError::InvalidArgument(format!(
                        "Failed to parse getUiTree args: {}",
                        e
                    ))
                })?;
                self.engine
                    .get_ui_tree(parsed_args.application_name.as_deref())
            }
            "findElementsBySelector" => {
                #[derive(Deserialize)]
                struct FindArgs {
                    selector: String,
                    // root_element_id: Option<String>, // TODO: Add support for root element ID
                }
                let parsed_args: FindArgs = from_value(args).map_err(|e| {
                    AutomationError::InvalidArgument(format!(
                        "Failed to parse findElementsBySelector args: {}",
                        e
                    ))
                })?;
                let selector = Selector::from_str(&parsed_args.selector)?;
                // TODO: Implement finding root element by ID if provided
                let elements = self.engine.find_elements(&selector, None)?;
                let element_attributes: Vec<_> =
                    elements.iter().map(|el| el.attributes()).collect();
                Ok(json!(element_attributes))
            }
            "getElementAttributes" => {
                #[derive(Deserialize)]
                struct GetAttributesArgs {
                    selector: String,
                    // root_element_id: Option<String>, // TODO: Add support for root element ID
                }
                let parsed_args: GetAttributesArgs = from_value(args).map_err(|e| {
                    AutomationError::InvalidArgument(format!(
                        "Failed to parse getElementAttributes args: {}",
                        e
                    ))
                })?;
                let selector = Selector::from_str(&parsed_args.selector)?;
                // TODO: Implement finding root element by ID if provided
                let element = self.engine.find_element(&selector, None)?;
                Ok(json!(element.attributes()))
            }
            "captureScreenshot" => {
                // No arguments expected for captureScreenshot
                self.capture_screenshot_base64()
                    .map(|base64_str| json!({ "screenshot_base64": base64_str }))
            }
            "getClipboard" => {
                let content = self.get_clipboard_content()?;
                Ok(json!({ "content": content }))
            }
            "setClipboard" => {
                #[derive(Deserialize)]
                struct SetClipboardArgs {
                    content: String,
                }
                let parsed_args: SetClipboardArgs = from_value(args).map_err(|e| {
                    AutomationError::InvalidArgument(format!(
                        "Failed to parse setClipboard args: {}",
                        e
                    ))
                })?;
                self.set_clipboard_content(&parsed_args.content)?;
                Ok(json!({ "status": "success" }))
            }
            "pressKey" => {
                #[derive(Deserialize)]
                struct PressKeyArgs {
                    key: String,
                    modifier: Option<String>,
                }
                let parsed_args: PressKeyArgs = from_value(args).map_err(|e| {
                    AutomationError::InvalidArgument(format!(
                        "Failed to parse pressKey args: {}",
                        e
                    ))
                })?;
                self.press_key(&parsed_args.key, parsed_args.modifier.as_deref())?;
                Ok(json!({
                    "status": "success",
                    "details": format!("Pressed key '{}' with modifier '{}'", parsed_args.key, parsed_args.modifier.as_deref().unwrap_or("none"))
                }))
            }
            "holdKey" => {
                let key = args["key"].as_str().ok_or_else(|| {
                    AutomationError::InvalidArgument(
                        "Missing or invalid 'key' argument for holdKey".to_string(),
                    )
                })?;
                let duration_ms = args.get("duration_ms").and_then(|v| v.as_u64());
                self.hold_key(key, duration_ms)?;
                Ok(Value::String(format!("Key '{}' held successfully.", key)))
            }
            "releaseKey" => {
                let key = args["key"].as_str().ok_or_else(|| {
                    AutomationError::InvalidArgument(
                        "Missing or invalid 'key' argument for releaseKey".to_string(),
                    )
                })?;
                self.release_key(key)?;
                Ok(Value::String(format!(
                    "Key '{}' released successfully.",
                    key
                )))
            }
            "wait" => {
                #[derive(Deserialize)]
                struct WaitArgs {
                    duration_ms: u64,
                }
                let args: WaitArgs = from_value(args).map_err(|e| {
                    AutomationError::InvalidArgument(format!("Error parsing wait args: {}", e))
                })?;
                self.wait(args.duration_ms)?;
                Ok(json!(null))
            }
            "cursorPosition" => {
                let (x, y) = self.cursor_position()?;
                Ok(json!({ "x": x, "y": y }))
            }
            "mouseMove" => {
                #[derive(Deserialize)]
                struct MouseMoveArgs {
                    x: f64,
                    y: f64,
                }
                let args: MouseMoveArgs = from_value(args).map_err(|e| {
                    AutomationError::InvalidArgument(format!("Error parsing mouseMove args: {}", e))
                })?;
                self.mouse_move(args.x, args.y)?;
                Ok(json!(null))
            }
            "leftMouseDown" => {
                #[derive(Deserialize)]
                struct MouseDownArgs {
                    x: f64,
                    y: f64,
                }
                let args: MouseDownArgs = from_value(args).map_err(|e| {
                    AutomationError::InvalidArgument(format!(
                        "Error parsing leftMouseDown args: {}",
                        e
                    ))
                })?;
                self.left_mouse_down(args.x, args.y)?;
                Ok(json!(null))
            }
            "leftMouseUp" => {
                #[derive(Deserialize)]
                struct MouseUpArgs {
                    x: f64,
                    y: f64,
                }
                let args: MouseUpArgs = from_value(args).map_err(|e| {
                    AutomationError::InvalidArgument(format!(
                        "Error parsing leftMouseUp args: {}",
                        e
                    ))
                })?;
                self.left_mouse_up(args.x, args.y)?;
                Ok(json!(null))
            }
            "leftClick" => {
                #[derive(Deserialize)]
                struct ClickArgs {
                    x: f64,
                    y: f64,
                }
                let args: ClickArgs = from_value(args).map_err(|e| {
                    AutomationError::InvalidArgument(format!("Error parsing leftClick args: {}", e))
                })?;
                self.left_click(args.x, args.y, None)?;
                Ok(json!(null))
            }
            "rightClick" => {
                #[derive(Deserialize)]
                struct ClickArgs {
                    x: f64,
                    y: f64,
                }
                let args: ClickArgs = from_value(args).map_err(|e| {
                    AutomationError::InvalidArgument(format!(
                        "Error parsing rightClick args: {}",
                        e
                    ))
                })?;
                self.right_click(args.x, args.y, None)?;
                Ok(json!(null))
            }
            "middleClick" => {
                #[derive(Deserialize)]
                struct ClickArgs {
                    x: f64,
                    y: f64,
                }
                let args: ClickArgs = from_value(args).map_err(|e| {
                    AutomationError::InvalidArgument(format!(
                        "Error parsing middleClick args: {}",
                        e
                    ))
                })?;
                self.middle_click(args.x, args.y, None)?;
                Ok(json!(null))
            }
            "doubleClick" => {
                #[derive(Deserialize)]
                struct ClickArgs {
                    x: f64,
                    y: f64,
                }
                let args: ClickArgs = from_value(args).map_err(|e| {
                    AutomationError::InvalidArgument(format!(
                        "Error parsing doubleClick args: {}",
                        e
                    ))
                })?;
                self.double_click(args.x, args.y, None)?;
                Ok(json!(null))
            }
            "tripleClick" => {
                #[derive(Deserialize)]
                struct ClickArgs {
                    x: f64,
                    y: f64,
                }
                let args: ClickArgs = from_value(args).map_err(|e| {
                    AutomationError::InvalidArgument(format!(
                        "Error parsing tripleClick args: {}",
                        e
                    ))
                })?;
                self.triple_click(args.x, args.y, None)?;
                Ok(json!(null))
            }

            // --- Text Editor Handlers ---
            "text_editor_view" => {
                #[derive(Deserialize)]
                struct TextViewArgs {
                    file_path: String,
                }
                let parsed_args: TextViewArgs = from_value(args).map_err(|e| {
                    AutomationError::InvalidArgument(format!(
                        "Error parsing text_editor_view args: {}",
                        e
                    ))
                })?;
                match fs::read_to_string(&parsed_args.file_path) {
                    Ok(content) => Ok(json!({ "content": content })),
                    Err(e) => Err(AutomationError::Internal(format!(
                        "Failed to read file '{}': {}",
                        parsed_args.file_path, e
                    ))),
                }
            }
            "text_editor_create" => {
                #[derive(Deserialize)]
                struct TextCreateArgs {
                    file_path: String,
                    content: Option<String>,
                }
                let parsed_args: TextCreateArgs = from_value(args).map_err(|e| {
                    AutomationError::InvalidArgument(format!(
                        "Error parsing text_editor_create args: {}",
                        e
                    ))
                })?;
                match fs::OpenOptions::new()
                    .write(true)
                    .create_new(true)
                    .open(&parsed_args.file_path)
                {
                    Ok(mut file) => {
                        use std::io::Write; // Import Write trait here
                        let content_to_write = parsed_args.content.unwrap_or_default();
                        match file.write_all(content_to_write.as_bytes()) {
                            Ok(_) => Ok(
                                json!({ "status": format!("File '{}' created successfully.", parsed_args.file_path) }),
                            ),
                            Err(e) => Err(AutomationError::Internal(format!(
                                "Failed to write initial content to file '{}': {}",
                                parsed_args.file_path, e
                            ))),
                        }
                    }
                    Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {
                        Err(AutomationError::Internal(format!(
                            "File '{}' already exists. Cannot create.",
                            parsed_args.file_path
                        )))
                    }
                    Err(e) => Err(AutomationError::Internal(format!(
                        "Failed to create file '{}': {}",
                        parsed_args.file_path, e
                    ))),
                }
            }
            "text_editor_str_replace" => {
                #[derive(Deserialize)]
                struct TextReplaceArgs {
                    file_path: String,
                    find: String,
                    replace: String,
                }
                let parsed_args: TextReplaceArgs = from_value(args).map_err(|e| {
                    AutomationError::InvalidArgument(format!(
                        "Error parsing text_editor_str_replace args: {}",
                        e
                    ))
                })?;
                let content = match fs::read_to_string(&parsed_args.file_path) {
                    Ok(c) => c,
                    Err(e) => {
                        return Err(AutomationError::Internal(format!(
                            "Failed to read file '{}' for replacement: {}",
                            parsed_args.file_path, e
                        )))
                    }
                };
                let new_content = content.replace(&parsed_args.find, &parsed_args.replace);
                match fs::write(&parsed_args.file_path, new_content) {
                    Ok(_) => Ok(
                        json!({ "status": format!("File '{}' updated successfully with replacements.", parsed_args.file_path) }),
                    ),
                    Err(e) => Err(AutomationError::Internal(format!(
                        "Failed to write updated content to file '{}': {}",
                        parsed_args.file_path, e
                    ))),
                }
            }
            "text_editor_insert" => {
                #[derive(Deserialize)]
                struct TextInsertArgs {
                    file_path: String,
                    text: String,
                    line: Option<usize>,
                }
                let parsed_args: TextInsertArgs = from_value(args).map_err(|e| {
                    AutomationError::InvalidArgument(format!(
                        "Error parsing text_editor_insert args: {}",
                        e
                    ))
                })?;
                let content = match fs::read_to_string(&parsed_args.file_path) {
                    Ok(c) => c,
                    Err(e) => {
                        return Err(AutomationError::Internal(format!(
                            "Failed to read file '{}' for insertion: {}",
                            parsed_args.file_path, e
                        )))
                    }
                };

                let mut lines: Vec<String> = content.lines().map(|s| s.to_string()).collect();
                let insertion_point = match parsed_args.line {
                    // Convert 1-based line to 0-based index, clamp to valid range (0 to lines.len())
                    Some(line_num_1_based) => line_num_1_based.saturating_sub(1).min(lines.len()),
                    None => lines.len(), // Append if line is null
                };

                lines.insert(insertion_point, parsed_args.text);

                // Join lines, ensuring newline at the end if original content had one or if inserting into empty file
                let new_content = lines.join("\n");
                // A simple heuristic: add newline if original content ended with one or if it was empty
                let final_content = if content.ends_with('\n') || content.is_empty() {
                    format!("{}\n", new_content)
                } else {
                    new_content
                };

                match fs::write(&parsed_args.file_path, final_content) {
                    Ok(_) => Ok(
                        json!({ "status": format!("Text inserted successfully into file '{}' at line {}.", parsed_args.file_path, insertion_point + 1) }),
                    ),
                    Err(e) => Err(AutomationError::Internal(format!(
                        "Failed to write updated content to file '{}': {}",
                        parsed_args.file_path, e
                    ))),
                }
            }
            // --- End Text Editor Handlers ---

            // --- Bash Handler ---
            "bash" => {
                #[derive(Deserialize)]
                struct BashArgs {
                    command: String,
                    /// Wall-clock limit in SECONDS. `None` means the command runs
                    /// to completion with no limit, which is what this tool has
                    /// always done and what callers that send nothing still get.
                    timeout: Option<u64>,
                }
                let parsed_args: BashArgs = from_value(args).map_err(|e| {
                    AutomationError::InvalidArgument(format!("Error parsing bash args: {}", e))
                })?;

                // A zero-second budget can only ever kill the command before it
                // does anything, so it is a caller error, not a request.
                if parsed_args.timeout == Some(0) {
                    return Err(AutomationError::InvalidArgument(
                        "bash 'timeout' must be at least 1 second; omit it to run without a timeout"
                            .to_string(),
                    ));
                }

                // Determine shell based on OS
                let shell_cmd = if cfg!(target_os = "windows") {
                    ("cmd", vec!["/C".to_string(), parsed_args.command])
                } else {
                    // Assume Unix-like shell (sh)
                    ("sh", vec!["-c".to_string(), parsed_args.command])
                };

                run_shell_command(shell_cmd.0, &shell_cmd.1, parsed_args.timeout)
            }
            // --- End Bash Handler ---

            // --- Computer Tool Handler (Official Anthropic Computer Use) ---
            "computer" => {
                #[derive(Deserialize)]
                struct ComputerArgs {
                    action: String,
                    coordinate: Option<Vec<f64>>,
                    // Note: For drag operations, coordinate represents the end position
                    // Drag starts from current cursor position as per Anthropic Computer Use specification
                    text: Option<String>,
                    scroll_direction: Option<String>,
                    scroll_amount: Option<f64>,
                    /// Milliseconds. The unit is in the name because a bare
                    /// `duration` means seconds in Anthropic's canonical computer
                    /// tool; the alias keeps older MCP clients working.
                    #[serde(alias = "duration")]
                    duration_ms: Option<u64>,
                }
                let parsed_args: ComputerArgs = from_value(args).map_err(|e| {
                    AutomationError::InvalidArgument(format!("Error parsing computer args: {}", e))
                })?;

                match parsed_args.action.as_str() {
                    "screenshot" => {
                        let base64_screenshot = self.capture_screenshot_base64()?;
                        Ok(json!({ "screenshot": base64_screenshot }))
                    }
                    "left_click" => {
                        let coords = parsed_args.coordinate.ok_or_else(|| {
                            AutomationError::InvalidArgument(
                                "coordinate required for left_click action".to_string(),
                            )
                        })?;
                        if coords.len() != 2 {
                            return Err(AutomationError::InvalidArgument(
                                "coordinate must be [x, y] array".to_string(),
                            ));
                        }
                        self.left_click(coords[0], coords[1], None)?;
                        Ok(json!({"status": "success"}))
                    }
                    "right_click" => {
                        let coords = parsed_args.coordinate.ok_or_else(|| {
                            AutomationError::InvalidArgument(
                                "coordinate required for right_click action".to_string(),
                            )
                        })?;
                        if coords.len() != 2 {
                            return Err(AutomationError::InvalidArgument(
                                "coordinate must be [x, y] array".to_string(),
                            ));
                        }
                        self.right_click(coords[0], coords[1], None)?;
                        Ok(json!({"status": "success"}))
                    }
                    "middle_click" => {
                        let coords = parsed_args.coordinate.ok_or_else(|| {
                            AutomationError::InvalidArgument(
                                "coordinate required for middle_click action".to_string(),
                            )
                        })?;
                        if coords.len() != 2 {
                            return Err(AutomationError::InvalidArgument(
                                "coordinate must be [x, y] array".to_string(),
                            ));
                        }
                        self.middle_click(coords[0], coords[1], None)?;
                        Ok(json!({"status": "success"}))
                    }
                    "double_click" => {
                        let coords = parsed_args.coordinate.ok_or_else(|| {
                            AutomationError::InvalidArgument(
                                "coordinate required for double_click action".to_string(),
                            )
                        })?;
                        if coords.len() != 2 {
                            return Err(AutomationError::InvalidArgument(
                                "coordinate must be [x, y] array".to_string(),
                            ));
                        }
                        self.double_click(coords[0], coords[1], None)?;
                        Ok(json!({"status": "success"}))
                    }
                    "triple_click" => {
                        let coords = parsed_args.coordinate.ok_or_else(|| {
                            AutomationError::InvalidArgument(
                                "coordinate required for triple_click action".to_string(),
                            )
                        })?;
                        if coords.len() != 2 {
                            return Err(AutomationError::InvalidArgument(
                                "coordinate must be [x, y] array".to_string(),
                            ));
                        }
                        self.triple_click(coords[0], coords[1], None)?;
                        Ok(json!({"status": "success"}))
                    }
                    "left_click_drag" => {
                        // Get current cursor position as start point (following Anthropic Computer Use specification)
                        let (start_x, start_y) = self.cursor_position()?;

                        // Get end coordinates from coordinate parameter
                        let end_coords = parsed_args.coordinate.ok_or_else(|| {
                            AutomationError::InvalidArgument(
                                "coordinate required for left_click_drag action".to_string(),
                            )
                        })?;

                        if end_coords.len() != 2 {
                            return Err(AutomationError::InvalidArgument(
                                "coordinate must be [x, y] array".to_string(),
                            ));
                        }

                        // Perform drag operation from current cursor position to specified coordinate
                        self.left_click_drag(start_x, start_y, end_coords[0], end_coords[1])?;
                        Ok(json!({
                            "status": "success",
                            "start": [start_x, start_y],
                            "end": end_coords
                        }))
                    }
                    "mouse_move" => {
                        let coords = parsed_args.coordinate.ok_or_else(|| {
                            AutomationError::InvalidArgument(
                                "coordinate required for mouse_move action".to_string(),
                            )
                        })?;
                        if coords.len() != 2 {
                            return Err(AutomationError::InvalidArgument(
                                "coordinate must be [x, y] array".to_string(),
                            ));
                        }
                        self.mouse_move(coords[0], coords[1])?;
                        Ok(json!({"status": "success"}))
                    }
                    "left_mouse_down" => {
                        let coords = parsed_args.coordinate.ok_or_else(|| {
                            AutomationError::InvalidArgument(
                                "coordinate required for left_mouse_down action".to_string(),
                            )
                        })?;
                        if coords.len() != 2 {
                            return Err(AutomationError::InvalidArgument(
                                "coordinate must be [x, y] array".to_string(),
                            ));
                        }
                        self.left_mouse_down(coords[0], coords[1])?;
                        Ok(json!({"status": "success"}))
                    }
                    "left_mouse_up" => {
                        let coords = parsed_args.coordinate.ok_or_else(|| {
                            AutomationError::InvalidArgument(
                                "coordinate required for left_mouse_up action".to_string(),
                            )
                        })?;
                        if coords.len() != 2 {
                            return Err(AutomationError::InvalidArgument(
                                "coordinate must be [x, y] array".to_string(),
                            ));
                        }
                        self.left_mouse_up(coords[0], coords[1])?;
                        Ok(json!({"status": "success"}))
                    }
                    "type" => {
                        let text = parsed_args.text.ok_or_else(|| {
                            AutomationError::InvalidArgument(
                                "text required for type action".to_string(),
                            )
                        })?;
                        self.type_text(&text)?;
                        Ok(json!({"status": "success"}))
                    }
                    "key" => {
                        let key = parsed_args.text.ok_or_else(|| {
                            AutomationError::InvalidArgument(
                                "text (key combination) required for key action".to_string(),
                            )
                        })?;
                        self.press_key(&key, None)?;
                        Ok(json!({"status": "success"}))
                    }
                    "hold_key" => {
                        let key = parsed_args.text.ok_or_else(|| {
                            AutomationError::InvalidArgument(
                                "text (key name) required for hold_key action".to_string(),
                            )
                        })?;
                        self.hold_key(&key, parsed_args.duration_ms)?;
                        Ok(json!({"status": "success"}))
                    }
                    "scroll" => {
                        let direction = parsed_args.scroll_direction.ok_or_else(|| {
                            AutomationError::InvalidArgument(
                                "scroll_direction required for scroll action".to_string(),
                            )
                        })?;
                        let amount = parsed_args.scroll_amount.unwrap_or(3.0);
                        if let Some(coords) = parsed_args.coordinate {
                            if coords.len() != 2 {
                                return Err(AutomationError::InvalidArgument(
                                    "coordinate must be [x, y] array".to_string(),
                                ));
                            }
                            self.scroll_at_position(coords[0], coords[1], &direction, amount)?;
                        } else {
                            self.scroll_at_current_position(&direction, amount)?;
                        }
                        Ok(json!({"status": "success"}))
                    }
                    "wait" => {
                        let duration_ms = parsed_args.duration_ms.ok_or_else(|| {
                            AutomationError::InvalidArgument(
                                "duration_ms (milliseconds) required for wait action".to_string(),
                            )
                        })?;
                        self.wait(duration_ms)?;
                        Ok(json!({"status": "success"}))
                    }
                    "cursor_position" => {
                        let (x, y) = self.cursor_position()?;
                        Ok(json!({"x": x, "y": y}))
                    }
                    _ => Err(AutomationError::InvalidArgument(format!(
                        "Unknown computer action: {}",
                        parsed_args.action
                    ))),
                }
            }
            // --- End Computer Tool Handler ---

            // --- str_replace_based_edit_tool Handler (Official Anthropic Tool) ---
            "str_replace_based_edit_tool" => {
                #[derive(Deserialize)]
                struct EditToolArgs {
                    command: String,
                    path: String,
                    file_text: Option<String>,
                    old_str: Option<String>,
                    new_str: Option<String>,
                    insert_line: Option<usize>,
                }
                let parsed_args: EditToolArgs = from_value(args).map_err(|e| {
                    AutomationError::InvalidArgument(format!(
                        "Error parsing str_replace_based_edit_tool args: {}",
                        e
                    ))
                })?;

                match parsed_args.command.as_str() {
                    "view" => match fs::read_to_string(&parsed_args.path) {
                        Ok(content) => Ok(json!({ "content": content })),
                        Err(e) => Err(AutomationError::Internal(format!(
                            "Failed to read file '{}': {}",
                            parsed_args.path, e
                        ))),
                    },
                    "create" => {
                        let content = parsed_args.file_text.ok_or_else(|| {
                            AutomationError::InvalidArgument(
                                "file_text required for create command".to_string(),
                            )
                        })?;
                        match fs::OpenOptions::new()
                            .write(true)
                            .create_new(true)
                            .open(&parsed_args.path)
                        {
                            Ok(mut file) => {
                                use std::io::Write;
                                match file.write_all(content.as_bytes()) {
                                    Ok(_) => Ok(
                                        json!({ "status": format!("File '{}' created successfully.", parsed_args.path) }),
                                    ),
                                    Err(e) => Err(AutomationError::Internal(format!(
                                        "Failed to write content to file '{}': {}",
                                        parsed_args.path, e
                                    ))),
                                }
                            }
                            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {
                                Err(AutomationError::Internal(format!(
                                    "File '{}' already exists. Cannot create.",
                                    parsed_args.path
                                )))
                            }
                            Err(e) => Err(AutomationError::Internal(format!(
                                "Failed to create file '{}': {}",
                                parsed_args.path, e
                            ))),
                        }
                    }
                    "str_replace" => {
                        let old_str = parsed_args.old_str.ok_or_else(|| {
                            AutomationError::InvalidArgument(
                                "old_str required for str_replace command".to_string(),
                            )
                        })?;
                        // new_str defaults to empty string if not provided (enables deletion)
                        let new_str = parsed_args.new_str.unwrap_or_default();
                        let content = match fs::read_to_string(&parsed_args.path) {
                            Ok(c) => c,
                            Err(e) => {
                                return Err(AutomationError::Internal(format!(
                                    "Failed to read file '{}' for replacement: {}",
                                    parsed_args.path, e
                                )))
                            }
                        };

                        // Count matches before replacement to provide feedback
                        let match_count = content.matches(&old_str).count();

                        if match_count == 0 {
                            return Err(AutomationError::Internal(format!(
                                "No matches found for '{}' in file '{}'",
                                old_str, parsed_args.path
                            )));
                        }

                        let new_content = content.replace(&old_str, &new_str);
                        match fs::write(&parsed_args.path, new_content) {
                            Ok(_) => Ok(json!({
                                "status": format!("File '{}' updated successfully. {} occurrence(s) of '{}' replaced with '{}'.",
                                    parsed_args.path, match_count, old_str, new_str),
                                "matches_replaced": match_count
                            })),
                            Err(e) => Err(AutomationError::Internal(format!(
                                "Failed to write updated content to file '{}': {}",
                                parsed_args.path, e
                            ))),
                        }
                    }
                    "insert" => {
                        let new_str = parsed_args.new_str.ok_or_else(|| {
                            AutomationError::InvalidArgument(
                                "new_str required for insert command".to_string(),
                            )
                        })?;
                        let insert_line = parsed_args.insert_line.ok_or_else(|| {
                            AutomationError::InvalidArgument(
                                "insert_line required for insert command".to_string(),
                            )
                        })?;
                        let content = match fs::read_to_string(&parsed_args.path) {
                            Ok(c) => c,
                            Err(e) => {
                                return Err(AutomationError::Internal(format!(
                                    "Failed to read file '{}' for insertion: {}",
                                    parsed_args.path, e
                                )))
                            }
                        };

                        let mut lines: Vec<String> =
                            content.lines().map(|s| s.to_string()).collect();

                        // Fix: insert_line should mean "insert after line N" for consistency with text editor expectations
                        // insert_line=1 means insert after line 1 (at index 1)
                        // insert_line=0 means insert at beginning (at index 0)
                        let insertion_point = if insert_line == 0 {
                            0
                        } else {
                            insert_line.min(lines.len())
                        };

                        lines.insert(insertion_point, new_str);

                        let new_content = lines.join("\n");
                        let final_content = if content.ends_with('\n') || content.is_empty() {
                            format!("{}\n", new_content)
                        } else {
                            new_content
                        };

                        match fs::write(&parsed_args.path, final_content) {
                            Ok(_) => Ok(
                                json!({ "status": format!("Text inserted successfully into file '{}' after line {}.", parsed_args.path, if insert_line == 0 { "beginning".to_string() } else { insert_line.to_string() }) }),
                            ),
                            Err(e) => Err(AutomationError::Internal(format!(
                                "Failed to write updated content to file '{}': {}",
                                parsed_args.path, e
                            ))),
                        }
                    }
                    _ => Err(AutomationError::InvalidArgument(format!(
                        "Unknown str_replace_based_edit_tool command: {}",
                        parsed_args.command
                    ))),
                }
            }
            // --- End str_replace_based_edit_tool Handler ---
            _ => {
                error!("Unknown tool called: {}", name);
                Err(AutomationError::ToolNotFound(name.to_string()))
            }
        }
    }

    // --- Screenshot Functionality ---
    #[cfg(target_os = "macos")]
    pub fn capture_screenshot_base64(&self) -> Result<String, AutomationError> {
        // Call the platform-specific function that now handles encoding
        platforms::macos::utils::capture_and_encode_screenshot()
    }

    #[cfg(not(target_os = "macos"))]
    pub fn capture_screenshot_base64(&self) -> Result<String, AutomationError> {
        Err(AutomationError::UnsupportedOperation(
            "Screenshot capture is only supported on macOS currently.".to_string(),
        ))
    }
    // --- End Screenshot Functionality ---

    // --- End New Methods ---
}

#[cfg(test)]
mod shell_timeout_tests {
    use super::run_shell_command;
    use serde_json::Value;
    use std::time::{Duration, Instant};

    /// How far past its budget a timed-out call may return and still count as
    /// bounded.
    ///
    /// The real cost after the budget expires is one poll interval (50ms), a
    /// `killpg`, and a thread handover: a few milliseconds. Five seconds is
    /// roughly a hundred times that, so the assertion will not flake on a box
    /// running several builds at once — while still catching the defect these
    /// tests exist for, where a 1s budget took 30.3s.
    const TIMEOUT_SLACK: Duration = Duration::from_secs(5);

    fn sh(command: &str) -> Vec<String> {
        vec!["-c".to_string(), command.to_string()]
    }

    /// Run `command` with a timeout and return the result with how long the
    /// call actually took.
    fn run_timed(command: &str, timeout_seconds: u64) -> (Value, Duration) {
        let started_at = Instant::now();
        let result = run_shell_command("sh", &sh(command), Some(timeout_seconds))
            .expect("a killed command still returns a result, not an error");
        (result, started_at.elapsed())
    }

    fn assert_within_budget(elapsed: Duration, timeout_seconds: u64) {
        let ceiling = Duration::from_secs(timeout_seconds) + TIMEOUT_SLACK;
        assert!(
            elapsed < ceiling,
            "a {}s timeout must bound the call: it returned after {:.2?}, past the {:.2?} ceiling",
            timeout_seconds,
            elapsed,
            ceiling
        );
    }

    /// Whether `pid` still names a live process.
    #[cfg(unix)]
    fn process_is_alive(pid: i32) -> bool {
        // SAFETY: signal 0 performs the permission and existence checks without
        // delivering anything. No memory is touched.
        unsafe { libc::kill(pid, 0) == 0 }
    }

    #[test]
    fn no_timeout_runs_to_completion_and_reports_no_timeout_field() {
        let result = run_shell_command("sh", &sh("printf hello"), None)
            .expect("command without a timeout should run");
        assert_eq!(result["stdout"], "hello");
        assert_eq!(result["success"], true);
        assert_eq!(result["exit_code"], 0);
        // The untimed path is byte-for-byte what it always was: no timeout was
        // asked for, so no timeout is reported and none was applied.
        assert!(result.get("timed_out").is_none());
    }

    #[test]
    fn a_timeout_the_command_beats_is_reported_as_not_timed_out() {
        let result = run_shell_command("sh", &sh("printf done"), Some(30))
            .expect("fast command should finish inside its budget");
        assert_eq!(result["stdout"], "done");
        assert_eq!(result["success"], true);
        assert_eq!(result["timed_out"], false);
    }

    #[test]
    fn a_command_that_outlives_its_timeout_is_killed() {
        // `sh -c 'sleep 30'` execs: the shell *becomes* the sleep, so killing
        // the direct child has always been enough here.
        let (result, elapsed) = run_timed("sleep 30", 1);
        assert_eq!(result["timed_out"], true);
        assert_eq!(result["success"], false);
        assert!(result["exit_code"].is_null());
        assert_within_budget(elapsed, 1);
    }

    #[test]
    fn output_written_before_the_timeout_survives_the_kill() {
        // Two commands, so the shell forks instead of exec'ing. The grandchild
        // inherits the write end of our stdout pipe; if it is left running, the
        // reader blocks on that pipe and the call returns in 30s, not 1s.
        let (result, elapsed) = run_timed("printf early; sleep 30", 1);
        assert_eq!(result["timed_out"], true);
        assert_eq!(result["stdout"], "early");
        assert_within_budget(elapsed, 1);
    }

    /// The shape the original tests missed: the budget has to bound the call
    /// even when what outlives it is a grandchild rather than the shell itself,
    /// and that grandchild has to actually die.
    #[cfg(unix)]
    #[test]
    fn a_forked_grandchild_is_killed_with_the_shell() {
        // The shell reports the pid of the process it forked, then waits on it,
        // so the grandchild is both identifiable and holding our pipes open.
        let (result, elapsed) = run_timed(r#"sleep 30 & printf '%s' "$!"; wait"#, 1);
        assert_eq!(result["timed_out"], true);
        assert_within_budget(elapsed, 1);

        let grandchild: i32 = result["stdout"]
            .as_str()
            .unwrap_or_default()
            .trim()
            .parse()
            .expect("the shell should have printed the pid of the process it forked");

        // Reparenting to launchd and reaping is not instant; poll rather than
        // guess a single sleep long enough to cover a loaded machine.
        let deadline = Instant::now() + Duration::from_secs(5);
        while process_is_alive(grandchild) && Instant::now() < deadline {
            std::thread::sleep(Duration::from_millis(50));
        }
        assert!(
            !process_is_alive(grandchild),
            "grandchild {} outlived the timeout that killed its shell",
            grandchild
        );
    }

    #[test]
    fn a_nonzero_exit_is_reported_as_failure_not_as_a_timeout() {
        let result = run_shell_command("sh", &sh("exit 3"), Some(30))
            .expect("a failing command is still a result");
        assert_eq!(result["exit_code"], 3);
        assert_eq!(result["success"], false);
        assert_eq!(result["timed_out"], false);
    }
}

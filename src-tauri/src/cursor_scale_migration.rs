//! Put back a pointer that an older Juno left enlarged.
//!
//! Juno used to signal "the agent is driving" by writing `mouseDriverCursorSize`
//! in `com.apple.universalaccess`, the same preference as System Settings →
//! Accessibility → Pointer Size. It remembered the original value only in
//! process memory, so a crash, a force quit, or the unguarded preview write in
//! Settings left the pointer enlarged for good, across reboots, with no record
//! of what it had been. Startup deliberately refused to fix it.
//!
//! Juno no longer touches that preference at all. The agent announces itself
//! with the cursor overlay, which cannot outlive the process that draws it.
//! This module exists only to clean up after the builds that did, and it runs
//! once.
//!
//! The hard part is telling "an older Juno left this big" from "this person
//! runs a large pointer on purpose", because the preference looks identical
//! either way. The evidence used here is `big_cursor_enabled` in Juno's own
//! settings file: only a build that scaled the pointer ever wrote that key. On
//! a machine that never ran one, nothing happens.

#[cfg(target_os = "macos")]
mod imp {
    use std::path::Path;
    use std::process::Command;
    use tracing::{info, warn};

    /// The key only an older, pointer-scaling build ever wrote.
    const EVIDENCE_KEY: &str = "big_cursor_enabled";
    /// Written once the cleanup has run, so it never runs twice.
    const MARKER: &str = "cursor-scale-restored";

    fn read_cursor_size() -> f64 {
        Command::new("defaults")
            .args(["read", "com.apple.universalaccess", "mouseDriverCursorSize"])
            .output()
            .ok()
            .and_then(|o| String::from_utf8(o.stdout).ok())
            .and_then(|s| s.trim().parse::<f64>().ok())
            .unwrap_or(1.0)
    }

    fn reset_cursor_size() {
        let wrote = Command::new("defaults")
            .args([
                "write",
                "com.apple.universalaccess",
                "mouseDriverCursorSize",
                "-float",
                "1.0",
            ])
            .output();
        if let Err(e) = wrote {
            warn!("[CursorRestore] Could not reset the pointer size: {e}");
            return;
        }
        // Nudge the accessibility subsystem so the change shows without a
        // logout. Both names are posted for cross-version coverage.
        for name in [
            "com.apple.accessibility.cache.cursor",
            "com.apple.universalaccess.prefChanged",
        ] {
            let _ = Command::new("notifyutil").args(["-p", name]).output();
        }
    }

    /// True when Juno's settings carry the key only a scaling build wrote.
    fn an_older_juno_scaled_here(config_dir: &Path) -> bool {
        let Ok(entries) = std::fs::read_dir(config_dir) else {
            return false;
        };
        entries.filter_map(Result::ok).any(|entry| {
            entry.path().extension().is_some_and(|e| e == "json")
                && std::fs::read_to_string(entry.path())
                    .is_ok_and(|text| text.contains(EVIDENCE_KEY))
        })
    }

    pub fn run(config_dir: &Path) {
        let marker = config_dir.join(MARKER);
        if marker.exists() {
            return;
        }

        let size = read_cursor_size();
        if size <= 1.0 {
            // Nothing to undo. Still record it, so a person who later enlarges
            // their pointer on purpose is never second-guessed by this code.
            let _ = std::fs::write(&marker, "nothing to restore\n");
            return;
        }

        if !an_older_juno_scaled_here(config_dir) {
            // A large pointer that Juno cannot take credit for. Leave it be,
            // and stop asking.
            info!("[CursorRestore] Pointer is {size:.1}x, but no Juno build scaled it here");
            let _ = std::fs::write(&marker, "not ours\n");
            return;
        }

        info!("[CursorRestore] An older Juno left the pointer at {size:.1}x, restoring it");
        reset_cursor_size();
        let _ = std::fs::write(&marker, format!("restored from {size:.2}\n"));
    }
}

#[cfg(not(target_os = "macos"))]
mod imp {
    pub fn run(_config_dir: &std::path::Path) {}
}

pub use imp::run;
